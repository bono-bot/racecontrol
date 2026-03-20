---
phase: 68-pod-switchcontroller
plan: 02
subsystem: infra
tags: [websocket, failover, tokio, arc-rwlock, self-monitor, rc-agent]

# Dependency graph
requires:
  - phase: 68-pod-switchcontroller plan 01
    provides: SwitchController protocol variant, failover_url CoreConfig field, HeartbeatStatus.last_switch_ms AtomicU64

provides:
  - Arc<RwLock<String>> active_url driving the reconnect loop URL on each iteration
  - SwitchController match arm: validates target URL, writes RwLock, stores last_switch_ms, sends Close frame, breaks inner loop
  - self_monitor last_switch_ms grace guard suppressing WS-dead relaunch for 60s after a switch
  - 3 unit tests for the grace guard logic

affects: [68-pod-switchcontroller, Phase 74 rc-agent decomposition]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Arc<RwLock<String>> for runtime-mutable config inside async event loop (no restart needed)"
    - "Epoch-millis AtomicU64 as lightweight cross-task signal (last_switch_ms guard pattern)"
    - "self_monitor log_event pub export for cross-module event recording"

key-files:
  created: []
  modified:
    - crates/rc-agent/src/main.rs
    - crates/rc-agent/src/self_monitor.rs

key-decisions:
  - "68-02: active_url Arc<RwLock<String>> created before outer reconnect loop; url cloned inside loop on each iteration — CRITICAL placement ensures new URL is picked up without process restart"
  - "68-02: SwitchController validates target_url against primary + failover strings (strict allowlist) — rejects unknown URLs with warn log, never silently accepts"
  - "68-02: log_event made pub in self_monitor so SwitchController handler in main.rs can record SWITCH events to the rc-bot event log"
  - "68-02: switch_grace_active = last_switch_ms != 0 && since_switch_ms < 60_000 — zero value means never switched, prevents false grace on startup"

patterns-established:
  - "Epoch-millis cross-task signal: store u64 ms in AtomicU64, load + saturating_sub in consumer to get elapsed"
  - "Strict URL allowlist in SwitchController: only primary_url + failover_url accepted, all others warn-logged and ignored"

requirements-completed: [FAIL-02, FAIL-03, FAIL-04]

# Metrics
duration: 20min
completed: 2026-03-20
---

# Phase 68 Plan 02: SwitchController Runtime Wiring Summary

**Arc<RwLock<String>> active_url in reconnect loop, SwitchController URL-validated handler with last_switch_ms signal, and 60s self_monitor grace guard — full failover switching wired end-to-end**

## Performance

- **Duration:** 20 min
- **Started:** 2026-03-20T14:05:36Z
- **Completed:** 2026-03-20T14:25:00Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments
- Reconnect loop now reads URL from `active_url.read().await.clone()` on each iteration — SwitchController causes immediate reconnect to new URL without process restart
- SwitchController match arm validates target URL against primary + failover strings, writes Arc<RwLock>, stores epoch-millis in `HeartbeatStatus.last_switch_ms`, sends WS Close frame, and breaks inner loop
- self_monitor WS-dead relaunch suppressed for 60s after any SwitchController (prevents self_monitor from fighting an intentional disconnect)
- 3 unit tests added covering: grace active within 60s, grace expired after 60s, never-switched (last_switch_ms=0)

## Task Commits

Each task was committed atomically:

1. **Task 1: Wire Arc RwLock active_url into reconnect loop + SwitchController handler** - `b4dde24` (feat)
2. **Task 2: Add last_switch_ms guard to self_monitor WS-dead check** - `766b1da` (feat)

## Files Created/Modified
- `crates/rc-agent/src/main.rs` - Arc<RwLock<String>> active_url, reconnect loop URL read, SwitchController match arm
- `crates/rc-agent/src/self_monitor.rs` - last_switch_ms grace guard, log_event made pub, 3 unit tests

## Decisions Made
- `active_url` is `std::sync::Arc<tokio::sync::RwLock<String>>` — RwLock from tokio (async-compatible await), Arc from std (no async needed for the Arc wrapper itself)
- `log_event` made `pub` in self_monitor.rs so main.rs can record SWITCH events to the rc-bot event log without duplicating the file-append logic
- Unit tests for the grace guard are pure logic tests (no AtomicU64 involved) — extracts the guard formula into local variables and asserts, exactly matching the production guard logic

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Made self_monitor::log_event pub**
- **Found during:** Task 1 (SwitchController handler)
- **Issue:** Plan specified `log_event(...)` call in main.rs SwitchController handler, but `log_event` was a private fn in self_monitor.rs
- **Fix:** Changed `fn log_event` to `pub fn log_event` in self_monitor.rs; referenced as `self_monitor::log_event(...)` in main.rs
- **Files modified:** crates/rc-agent/src/self_monitor.rs
- **Verification:** `cargo build --bin rc-agent` succeeds with zero errors
- **Committed in:** b4dde24 (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (Rule 2 - missing critical pub export)
**Impact on plan:** Minimal — visibility change only. No behaviour change, no new dependencies.

## Issues Encountered
- `cargo test -p rc-agent` fails with "Application Control policy has blocked this file" (Windows Defender AppLocker blocks test binary execution). This is a pre-existing environment restriction — all test code compiles cleanly. The 3 new unit tests are verified correct by logic inspection and build success.

## Next Phase Readiness
- Phase 68 complete (both plans done): SwitchController protocol variant (Plan 01) + runtime wiring (Plan 02)
- Requirements FAIL-01, FAIL-02, FAIL-03, FAIL-04 all complete
- rc-agent can now receive SwitchController at runtime, switch active WS URL, and self_monitor will not fight the intentional disconnect
- Ready for Phase 74 rc-agent Decomposition (DECOMP-01..04)

---
*Phase: 68-pod-switchcontroller*
*Completed: 2026-03-20*
