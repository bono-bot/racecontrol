---
phase: 178-agent-sentry-consumer
plan: 02
subsystem: rc-agent, rc-sentry
tags: [feature-flags, config-push, hot-reload, sentry-flags, billing-guard, game-launch]
dependency_graph:
  requires: [178-01]
  provides: [ConfigPush hot-reload, ConfigAck flow, sentry-flags bridge, game_launch gate, billing_guard gate]
  affects: [rc-agent ws_handler, rc-agent billing_guard, rc-sentry watchdog, rc-sentry HTTP]
tech_stack:
  added: []
  patterns:
    - "sentry-flags.json atomic write (tmp+rename) as file-based bridge between rc-agent and rc-sentry"
    - "pending_acks Vec<AgentMessage> drained from event loop after each WS message"
    - "Arc<RwLock<FeatureFlags>> created before AppState, shared with billing_guard"
key_files:
  created: []
  modified:
    - crates/rc-agent/src/feature_flags.rs
    - crates/rc-agent/src/event_loop.rs
    - crates/rc-agent/src/ws_handler.rs
    - crates/rc-agent/src/billing_guard.rs
    - crates/rc-agent/src/main.rs
    - crates/rc-sentry/src/main.rs
    - crates/rc-sentry/src/watchdog.rs
decisions:
  - "flags_arc created before AppState construction so billing_guard can share the same Arc (billing_guard::spawn at line 616 is before AppState at line 742)"
  - "restart_suppressed in watchdog gates the crash context channel send (not a post-send suppress) so tier1_fixes::handle_crash never fires during OTA deploy window"
  - "billing_guard tests updated to pass Arc<RwLock<FeatureFlags::new()>> — FeatureFlags::new() defaults all-enabled so existing tests are unaffected"
metrics:
  duration: "~20 minutes"
  completed_date: "2026-03-24"
  tasks_completed: 2
  tasks_total: 2
  files_modified: 7
  commits: 2
---

# Phase 178 Plan 02: ConfigPush, ConfigAck, sentry-flags bridge, and flag gates Summary

ConfigPush hot-reload pipeline with ConfigAck WS response, atomic sentry-flags.json bridge from rc-agent to rc-sentry, and feature flag gates at LaunchGame and billing_guard poll loop.

## What Was Built

### Task 1: ConfigPush handler + ConfigAck flow + sentry-flags bridge (commit `7a6ef5bf`)

**feature_flags.rs:**
- Added `write_sentry_flags()` — atomic write via tmp+rename to `C:\RacingPoint\sentry-flags.json`
- `apply_sync()` now calls `write_sentry_flags()` after `persist_to_disk()` — every FlagSync bridges to rc-sentry

**event_loop.rs:**
- Added `pending_acks: Vec<AgentMessage>` to `ConnectionState`
- After each `handle_ws_message` call, event loop drains `pending_acks` via `ws_tx.send()`

**ws_handler.rs:**
- Added `CoreToAgentMessage::ConfigPush` match arm with `HOT_RELOAD_FIELDS` and `NON_RELOAD_FIELDS` constants
- Non-reloadable fields (`port`, `ws_url`, `pod_number`, `pod_id`) log warning and are skipped
- `process_guard_whitelist` is hot-reloaded via `Arc<RwLock<MachineWhitelist>>`
- Other hot-reload fields (`billing_rates`, `game_limits`, `debug_verbosity`) log accepted and are wired as future extension points
- `ConfigAck` with `pod_id + sequence + accepted` queued into `conn.pending_acks`

### Task 2: rc-sentry flags reader, watchdog gate, LaunchGame gate, billing_guard gate (commit `1cdfcd47`)

**rc-sentry/main.rs:**
- `read_sentry_flags()` — reads `C:\RacingPoint\sentry-flags.json`, returns `Option<serde_json::Value>`
- `handle_flags()` — HTTP handler returning current flags JSON
- `/flags` endpoint added to route table (alongside /health, /version, etc.)

**rc-sentry/watchdog.rs:**
- At start of each watchdog tick: reads `sentry-flags.json` into `sentry_flags: Option<Value>`
- Extracts `kill_switches.kill_watchdog_restart` → `restart_suppressed: bool`
- When `restart_suppressed=true` in Crashed state: logs warning, transitions to Healthy, skips crash channel send (prevents tier1_fixes::handle_crash during OTA deploy)

**rc-agent/ws_handler.rs:**
- Added flag gate block at top of `CoreToAgentMessage::LaunchGame` handler
- Reads `flags.flag_enabled("game_launch")` — returns `HandleResult::Continue` without launching if disabled

**rc-agent/billing_guard.rs:**
- `spawn()` now takes `Arc<RwLock<FeatureFlags>>` as final parameter
- Checks `ff.flag_enabled("billing_guard")` on each poll tick, `continue` if disabled
- All 6 test call sites updated to pass `Arc::new(RwLock::new(FeatureFlags::new()))`

**rc-agent/main.rs:**
- `flags_arc` created early (before billing_guard::spawn at line 616) using `FeatureFlags::load_from_cache()`
- `billing_guard::spawn` receives `flags_arc.clone()`
- `AppState { flags: flags_arc, ... }` reuses the same Arc (not a second load)

## Verification

- `cargo check -p rc-agent-crate -p rc-sentry` — passes, no errors
- `cargo test -p rc-common` — 168 tests pass
- All 13 acceptance criteria pass (grep checks)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] billing_guard::spawn called before AppState — `state.flags` not in scope**

- **Found during:** Task 2 — first cargo check after adding `flags` parameter
- **Issue:** `billing_guard::spawn()` is called at main.rs:616, but `AppState` (which holds `flags`) is only created at line 742. `state.flags.clone()` compiled to error `E0425: cannot find value 'state'`.
- **Fix:** Created `flags_arc` via `Arc::new(RwLock::new(FeatureFlags::load_from_cache()))` at line 605, before the billing guard spawn. Passed `flags_arc.clone()` to spawn. Used `flags: flags_arc` in AppState (instead of creating a second Arc from disk). Net result: one load, one Arc, shared by both billing_guard and the event loop.
- **Files modified:** `crates/rc-agent/src/main.rs`
- **Commit:** `1cdfcd47`

## Self-Check: PASSED

- `crates/rc-agent/src/feature_flags.rs` — FOUND
- `crates/rc-agent/src/billing_guard.rs` — FOUND
- `crates/rc-sentry/src/watchdog.rs` — FOUND
- `.planning/phases/178-agent-sentry-consumer/178-02-SUMMARY.md` — FOUND
- commit `7a6ef5bf` — FOUND
- commit `1cdfcd47` — FOUND
