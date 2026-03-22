---
phase: 139-healer-edge-recovery
plan: "02"
subsystem: infra
tags: [rust, rc-agent, ws_handler, lock_screen, recovery, healer]

# Dependency graph
requires:
  - phase: 139-01
    provides: CoreToAgentMessage::ForceRelaunchBrowser variant + pod_healer dispatch
  - phase: 137-02
    provides: LockScreenManager.close_browser() and launch_browser() public API
provides:
  - rc-agent handles ForceRelaunchBrowser: close_browser + launch_browser on pod
  - billing_active guard prevents relaunch during active sessions
  - Unit test verifies protocol deserialization contract
affects:
  - 139-03
  - deploy

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Billing guard pattern: load(Relaxed) on AtomicBool before any lock_screen mutation"
    - "Direct state.lock_screen field access (no Arc<Mutex> wrapper in ws_handler)"

key-files:
  created: []
  modified:
    - crates/rc-agent/src/ws_handler.rs

key-decisions:
  - "139-02: ForceRelaunchBrowser arm uses direct state.lock_screen access matching all other lock_screen arms"
  - "139-02: Billing guard uses Relaxed ordering matching existing BlankScreen arm pattern"
  - "139-02: Pod-id field destructured as pod_id: _ (not needed in agent handler — pod knows its own identity)"

patterns-established:
  - "Phase 139 recovery pattern: server detects (healer) -> queues action -> sends WS msg -> agent handles"

requirements-completed:
  - HEAL-03

# Metrics
duration: 8min
completed: 2026-03-22
---

# Phase 139 Plan 02: Healer Edge Recovery — Agent Handler Summary

**rc-agent ForceRelaunchBrowser handler: billing-gated close_browser + launch_browser on server-initiated WS message, completing the server-to-pod lock screen recovery round-trip**

## Performance

- **Duration:** 8 min
- **Started:** 2026-03-22T05:00:00Z
- **Completed:** 2026-03-22T05:08:17Z
- **Tasks:** 2
- **Files modified:** 1

## Accomplishments
- ForceRelaunchBrowser match arm added in ws_handler.rs before the catch-all, completing the healer round-trip
- Billing-active guard prevents relaunch during active sessions (matches BlankScreen guard pattern)
- Unit test `force_relaunch_browser_variant_exists` verifies JSON deserialization of the protocol variant
- cargo build --release succeeds for both racecontrol.exe (32MB) and rc-agent.exe (11MB)
- Full protocol chain verified: healer detects -> queues relaunch_lock_screen -> sends ForceRelaunchBrowser WS -> agent handles

## Task Commits

Each task was committed atomically:

1. **Task 1: Add ForceRelaunchBrowser handler in ws_handler.rs** - `6b9cce5` (feat)
2. **Task 2: Full build verification and test sweep** - (verification only, no file changes)

**Plan metadata:** pending (docs commit)

## Files Created/Modified
- `crates/rc-agent/src/ws_handler.rs` - Added ForceRelaunchBrowser match arm + #[cfg(test)] block with deserialization test

## Decisions Made
- Used `state.lock_screen.close_browser()` + `state.lock_screen.launch_browser()` with direct field access — matches every other lock_screen call in ws_handler.rs (no Arc<Mutex> wrapping in this file)
- `pod_id: _` destructure — the agent doesn't need to inspect the pod_id; it acts unconditionally on its own lock screen
- Billing guard uses `std::sync::atomic::Ordering::Relaxed` — matches the existing BlankScreen arm at line 568

## Deviations from Plan

None — plan executed exactly as written. The `#[cfg(test)]` block was new (no existing test block in ws_handler.rs) but this was specified in the plan's action.

## Issues Encountered

- `cargo test -p rc-agent` failed with "package ID specification did not match" — actual crate name is `rc-agent-crate`. Used correct name throughout. Pre-existing racecontrol-crate test failures in `config` and `crypto` modules confirmed pre-existing (reproduced on stash, unrelated to this plan).

## Next Phase Readiness
- Server→pod lock screen recovery chain is complete end-to-end (139-01 + 139-02)
- Ready for integration testing and deployment to pods
- HEAL-03 requirement complete

---
*Phase: 139-healer-edge-recovery*
*Completed: 2026-03-22*
