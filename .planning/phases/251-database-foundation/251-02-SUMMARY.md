---
phase: 251-database-foundation
plan: 02
subsystem: billing
tags: [orphan-detection, resilience, fsm, whatsapp-alert]
requirements: [FSM-10, RESIL-03]
dependency_graph:
  requires: [251-01]
  provides: [orphan-detection-startup, orphan-detection-background]
  affects: [billing_sessions, activity_log, whatsapp_alerter]
tech_stack:
  added: []
  patterns: [startup-scan, background-task-spawn, lock-snapshot-before-await]
key_files:
  created: []
  modified:
    - crates/racecontrol/src/billing.rs
    - crates/racecontrol/src/main.rs
decisions:
  - WhatsApp alerts use whatsapp_alerter::send_whatsapp gated on config.alerting.enabled (same pattern as app_health_monitor)
  - log_pod_activity signature is (state, pod_id, category, action, details, source) — not (db, pod_id, event, details) as plan assumed
  - Background task has 300s initial delay to avoid double-alerting sessions caught by startup scan
  - Active_timers lock snapshotted into HashSet and dropped before DB query (standing rule compliance)
metrics:
  duration: 20min
  completed: "2026-03-28T19:26:15Z"
  tasks: 2
  files: 2
---

# Phase 251 Plan 02: Orphaned Session Detection Summary

**One-liner:** Startup scan + 5-min background job detect stale billing sessions via last_timer_sync_at, flag DB end_reason, and WhatsApp-alert staff (FSM-10, RESIL-03).

## What Was Built

Two public async functions in `crates/racecontrol/src/billing.rs`:

**`detect_orphaned_sessions_on_startup`** (FSM-10)
- Queries `billing_sessions` WHERE `status IN ('active', 'paused_manual', 'paused_disconnect')` AND `last_timer_sync_at IS NULL OR last_timer_sync_at < datetime('now', '-5 minutes')`
- Logs each orphan at ERROR level with session ID, pod ID, driver ID, last_sync timestamp, and driving seconds
- Updates `end_reason = 'orphan_flagged_startup'` for audit trail (WHERE end_reason IS NULL to be idempotent)
- Sends WhatsApp alert to staff via `whatsapp_alerter::send_whatsapp` when alerting is enabled
- Logs to activity feed via `log_pod_activity`

**`detect_orphaned_sessions_background`** (RESIL-03)
- Same query as startup but additionally filters out sessions present in `active_timers` in-memory map (those are NOT orphans)
- Lock on `active_timers` is snapshotted into `HashSet<String>` and dropped before DB query (no lock across .await)
- Flags with `end_reason = 'orphan_flagged_background'`
- WhatsApp alert + activity log on detection

**Wiring in `crates/racecontrol/src/main.rs`:**
- Startup call: `billing::detect_orphaned_sessions_on_startup(&state).await` immediately after `recover_active_sessions`
- Background task: `tokio::spawn` with 300s initial delay + 300s interval, logs lifecycle start

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Incorrect WhatsApp send pattern**
- **Found during:** Task 1 — reading whatsapp_alerter.rs
- **Issue:** Plan assumed `state.wa_alert_tx.send(msg)` (channel pattern). The actual pattern is `whatsapp_alerter::send_whatsapp(&state.config, &msg).await` with alerting gate.
- **Fix:** Used `whatsapp_alerter::send_whatsapp` directly, gated on `state.config.alerting.enabled`. Added `use crate::whatsapp_alerter;` import.
- **Files modified:** crates/racecontrol/src/billing.rs
- **Commit:** a86f4710

**2. [Rule 1 - Bug] Incorrect log_pod_activity signature**
- **Found during:** Task 1 — reading activity_log.rs
- **Issue:** Plan showed `log_pod_activity(&state.db, "server", "orphan_detection", &alert_msg)` (4 args, db pool). Actual signature is `log_pod_activity(state, pod_id, category, action, details, source)` (6 args, state reference).
- **Fix:** Called with correct 6-arg signature: `log_pod_activity(state, "server", "billing", "orphan_detection", &alert_msg, "startup")`.
- **Files modified:** crates/racecontrol/src/billing.rs
- **Commit:** a86f4710

## Verification Results

- `cargo check -p racecontrol-crate`: PASS (1 pre-existing warning about irrefutable let pattern in main.rs)
- `cargo test -p racecontrol-crate --lib`: PASS — 559 tests, 0 failures
- All acceptance criteria verified:
  - `detect_orphaned_sessions_on_startup` at line 1716 of billing.rs
  - `detect_orphaned_sessions_background` at line 1783 of billing.rs
  - `STARTUP ORPHAN DETECTION` ERROR log at line 1738
  - `BACKGROUND ORPHAN DETECTION` ERROR log at line 1819
  - `orphan_flagged_startup` end_reason at line 1753
  - `orphan_flagged_background` end_reason at line 1834
  - `ORPHAN ALERT` WhatsApp messages at lines 1762 and 1843
  - `last_timer_sync_at < datetime` queries at lines 1725 and 1798
  - Startup detection called after recover_active_sessions at main.rs:563
  - Background task with 300s intervals at main.rs:636-637

## Known Stubs

None. Both detection paths are fully wired with real DB queries, real WhatsApp alerts, and real activity logging.

## Commits

| Task | Commit | Message |
|------|--------|---------|
| Task 1 | a86f4710 | feat(251-02): add detect_orphaned_sessions_on_startup and detect_orphaned_sessions_background |
| Task 2 | 9ef6116e | feat(251-02): wire orphan detection into startup and background task loop |

## Self-Check: PASSED
