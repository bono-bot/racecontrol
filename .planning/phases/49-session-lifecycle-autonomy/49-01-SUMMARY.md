---
phase: 49-session-lifecycle-autonomy
plan: 01
subsystem: billing
tags: [rust, tokio, reqwest, billing, websocket, lock-screen, http-retry]

# Dependency graph
requires:
  - phase: 48-dynamic-kiosk-allowlist
    provides: FailureMonitorState watch channel pattern, kiosk billing state tracking
  - phase: 46-crash-safety-panic-hook
    provides: StartupReport protocol extension pattern, OnceLock HTTP client pattern
provides:
  - SessionAutoEnded, BillingPaused, BillingResumed AgentMessage variants in protocol.rs
  - billing_paused + active_billing_session_id fields in FailureMonitorState
  - Configurable orphan auto-end threshold (auto_end_orphan_session_secs) in AgentConfig
  - billing_guard orphan detection with HTTP POST + 3-retry backoff [5s, 15s, 30s]
  - end_reason column on billing_sessions table with cloud_sync push support
  - show_idle_pin_entry() "Ready" screen on lock_screen — replaces ScreenBlanked after session end
  - blank_timer target changed to show_idle_pin_entry() at 30s (was show_blank_screen() at 15s)
affects: [50-llm-self-test-fleet-health, SESSION-03-billing-pause-resume]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - OnceLock static reqwest client for orphan HTTP calls (mirrors self_monitor.rs OLLAMA_CLIENT pattern)
    - Two-tier timer pattern: BILL-02 fires at 60s, SESSION-01 orphan fires at configurable threshold (default 300s) using same game_gone_since timer
    - Idle PinEntry as blank/empty LockScreenState::PinEntry variant — driver_name.is_empty() detection in render

key-files:
  created: []
  modified:
    - crates/rc-common/src/protocol.rs
    - crates/rc-agent/src/failure_monitor.rs
    - crates/rc-agent/src/billing_guard.rs
    - crates/rc-agent/src/lock_screen.rs
    - crates/rc-agent/src/main.rs
    - crates/racecontrol/src/billing.rs
    - crates/racecontrol/src/db/mod.rs
    - crates/racecontrol/src/cloud_sync.rs
    - crates/racecontrol/src/ws/mod.rs

key-decisions:
  - "auto_end_orphan_session_secs in AgentConfig (serde default 300s) — per-pod configurable without code rebuild"
  - "SESSION-01 orphan check shares game_gone_since timer with BILL-02 — two-tier escalation from 60s alert to 300s auto-end"
  - "SessionAutoEnded WS message sent regardless of HTTP retry outcome — server notified even if billing state is stale"
  - "show_blank_screen() kept ONLY for disconnect cleanup — all post-session and post-crash paths use show_idle_pin_entry()"
  - "end_reason stored via silent ALTER TABLE ADD COLUMN migration — idempotent on redeploy"

patterns-established:
  - "Idle PinEntry pattern: LockScreenState::PinEntry with empty driver_name/token_id signals idle/ready state to renderer"
  - "Two-tier billing anomaly: BILL-02 alert (60s) + SESSION-01 auto-end (300s) share same elapsed timer"

requirements-completed: [SESSION-01, SESSION-02]

# Metrics
duration: 14min
completed: 2026-03-19
---

# Phase 49 Plan 01: Session Lifecycle Autonomy Summary

**rc-agent autonomously detects and HTTP-ends orphaned billing sessions (5min configurable) + transitions pods to idle PinEntry "Ready" screen after session end instead of blank screen**

## Performance

- **Duration:** 14 min
- **Started:** 2026-03-19T03:09:23Z (IST 08:39:23)
- **Completed:** 2026-03-19T03:23:00Z (IST 08:53:00)
- **Tasks:** 2
- **Files modified:** 9

## Accomplishments
- SESSION-01: billing_guard detects billing_active + no game_pid for configurable threshold (default 300s) and HTTP POSTs to `/api/v1/billing/session/{id}/end?reason=orphan_timeout` with 3-retry backoff [5s, 15s, 30s], sends SessionAutoEnded WS message regardless of HTTP outcome
- SESSION-02: blank_timer target changed from show_blank_screen() to show_idle_pin_entry() at 30s delay — pod shows "Ready" screen after any session end, not a black screen
- Protocol extended with 3 new AgentMessage variants (SessionAutoEnded, BillingPaused, BillingResumed), billing_sessions table gains end_reason column with cloud sync support

## Task Commits

Each task was committed atomically:

1. **Task 1: Protocol + schema + FailureMonitorState + billing.rs end_reason + cloud_sync** - `12e254f` (feat)
2. **Task 2: Configurable orphan timeout, orphan auto-end, idle PinEntry, blank_timer 30s** - `39876e4` (feat)
3. **Auto-fix: SelfTestResult match arm + StartupReport wildcard in ws/mod.rs** - `cdab89a` (fix)

## Files Created/Modified
- `crates/rc-common/src/protocol.rs` - Added SessionAutoEnded, BillingPaused, BillingResumed variants; StartupReport gains startup_self_test_verdict + startup_probe_failures (Phase 50 prep)
- `crates/rc-agent/src/failure_monitor.rs` - Added billing_paused: bool and active_billing_session_id: Option<String> to FailureMonitorState
- `crates/rc-agent/src/billing_guard.rs` - Orphan auto-end: OnceLock HTTP client, attempt_orphan_end(), spawn() takes core_base_url + orphan_end_threshold_secs, billing_paused suppression, tokio::spawn retry loop
- `crates/rc-agent/src/lock_screen.rs` - Added show_idle_pin_entry() method + idle PinEntry rendering ("Ready" heading, QR scan subheading) when driver_name is empty
- `crates/rc-agent/src/main.rs` - AgentConfig gains auto_end_orphan_session_secs (serde default 300), billing_guard::spawn call updated, active_billing_session_id wired in BillingStarted/SessionEnded/BillingStopped handlers, blank_timer changed to show_idle_pin_entry() at 30s
- `crates/racecontrol/src/billing.rs` - end_billing_session_public() signature extended with end_reason: Option<&str>
- `crates/racecontrol/src/db/mod.rs` - Migration: ALTER TABLE billing_sessions ADD COLUMN end_reason TEXT (silent error on duplicate)
- `crates/racecontrol/src/cloud_sync.rs` - billing_sessions push payload includes end_reason
- `crates/racecontrol/src/ws/mod.rs` - Match arms for SessionAutoEnded, BillingPaused, BillingResumed, SelfTestResult; StartupReport pattern uses .. wildcard for future fields

## Decisions Made
- `auto_end_orphan_session_secs` is a top-level AgentConfig field (not nested in [billing]) — simpler TOML config, matches existing pattern for flat fields like `pod.id`
- Orphan detection shares the `game_gone_since` timer with BILL-02 stuck session detection — avoids a separate timer, creates natural two-tier escalation: alert at 60s, auto-end at 300s
- SessionAutoEnded WS is sent regardless of whether HTTP retry succeeded — server gets notified and can reconcile, better than silent failure

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Non-exhaustive AgentMessage pattern in ws/mod.rs after protocol.rs extended with SelfTestResult**
- **Found during:** Final verification (cargo check --workspace)
- **Issue:** protocol.rs had SelfTestResult variant (added by Phase 50 prep) not matched in ws/mod.rs AgentMessage match; StartupReport pattern also lacked wildcard for new Phase 50 fields, causing E0004/E0027 compile errors
- **Fix:** Added SelfTestResult match arm (tracing::info + `let _ = report`) and `..` wildcard to StartupReport pattern
- **Files modified:** crates/racecontrol/src/ws/mod.rs
- **Verification:** `cargo check --workspace` exits 0; `cargo test -p rc-agent-crate -- lock_screen` 30/30 pass
- **Committed in:** cdab89a

---

**Total deviations:** 1 auto-fixed (Rule 1 — bug)
**Impact on plan:** Auto-fix was necessary for correctness — pre-existing partial Phase 50 protocol extension broke the exhaustive pattern match. No scope creep.

## Issues Encountered
- Phase 50 Plan 02 had uncommitted work in the working tree (self_test.rs untracked + main.rs + protocol.rs modified) that introduced compile errors. The SelfTestResult variant was already in protocol.rs but ws/mod.rs had no handler. Fixed via ws/mod.rs match arm addition. The Phase 50 uncommitted changes (main.rs startup_self_test additions, self_test.rs) are pre-existing and out of scope for this plan.

## User Setup Required
None - no external service configuration required. The `auto_end_orphan_session_secs` config field has a `serde(default)` of 300 and is optional in the TOML — existing pod configs work without modification.

## Next Phase Readiness
- SESSION-01 + SESSION-02 complete. Pods now self-heal orphaned sessions without manual intervention.
- SESSION-03 (billing pause/resume during crash recovery) can proceed — BillingPaused/BillingResumed variants are already in protocol.rs and ws/mod.rs handles them
- Phase 50 LLM self-test work is partially in working tree (self_test.rs, main.rs changes) — needs to be committed as Phase 50 Plan 01/02 proceeds

---
*Phase: 49-session-lifecycle-autonomy*
*Completed: 2026-03-19*
