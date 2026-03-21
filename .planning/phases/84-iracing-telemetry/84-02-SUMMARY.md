---
phase: 84-iracing-telemetry
plan: 02
subsystem: telemetry
tags: [iracing, shared-memory, billing, playable-signal, sim-adapter]

# Dependency graph
requires:
  - phase: 84-01
    provides: IracingAdapter struct with SimAdapter impl including read_is_on_track override

provides:
  - IracingAdapter wired into rc-agent adapter creation match (SimType::IRacing arm)
  - read_is_on_track trait method on SimAdapter with default None implementation
  - Dedicated IRacing PlayableSignal arm in event_loop.rs using IsOnTrack shared memory signal
  - iRacing removed from 90s process-based fallback path

affects:
  - billing
  - event_loop
  - any future sim adapters that need per-sim PlayableSignal dispatch

# Tech tracking
tech-stack:
  added: []
  patterns:
    - Per-sim PlayableSignal arms in event_loop.rs match — each sim gets dedicated arm before catch-all
    - Default-None trait methods for sim-specific signals (read_ac_status, read_assist_state, read_is_on_track)

key-files:
  created: []
  modified:
    - crates/rc-agent/src/sims/mod.rs
    - crates/rc-agent/src/main.rs
    - crates/rc-agent/src/event_loop.rs

key-decisions:
  - "iRacing uses IsOnTrack shared-memory signal for billing trigger instead of 90s process fallback"
  - "read_is_on_track added as default-None trait method on SimAdapter following read_ac_status pattern"
  - "90s process fallback retained unchanged for LMU, EVO, WRC, Forza — only iRacing gets the shm path"

patterns-established:
  - "Per-sim PlayableSignal: add specific arm BEFORE catch-all Some(sim_type) in event_loop.rs match"
  - "Sim-specific signals: add as default-None trait method on SimAdapter, override in adapter impl"

requirements-completed:
  - TEL-IR-01
  - TEL-IR-03

# Metrics
duration: 15min
completed: 2026-03-21
---

# Phase 84 Plan 02: iRacing Wiring Summary

**IracingAdapter wired into rc-agent with IsOnTrack shared-memory billing trigger replacing the 90s process fallback**

## Performance

- **Duration:** 15 min
- **Started:** 2026-03-21T05:29:48Z
- **Completed:** 2026-03-21T05:44:47Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments

- `pub mod iracing;` registered in sims/mod.rs and `read_is_on_track()` default trait method added
- `IracingAdapter::new(pod_id)` wired into the adapter creation match in main.rs (SimType::IRacing arm)
- Dedicated `SimType::IRacing` arm added in event_loop.rs PlayableSignal dispatch, reading IsOnTrack from shared memory via trait dispatch
- iRacing removed from the 90s process-based fallback — other sims (LMU, EVO, WRC, Forza) unaffected

## Task Commits

Each task was committed atomically:

1. **Task 1: Register iracing module and add read_is_on_track to SimAdapter trait** - `dad850c` (included in docs commit for 99-01 — changes were already present in codebase at plan start)
2. **Task 2: Wire IracingAdapter creation in main.rs and PlayableSignal in event_loop.rs** - `5f27c06` (feat)

**Plan metadata:** (docs commit below)

## Files Created/Modified

- `crates/rc-agent/src/sims/mod.rs` - Added `pub mod iracing;` and `fn read_is_on_track() -> Option<bool> { None }` default trait method
- `crates/rc-agent/src/main.rs` - Added `use sims::iracing::IracingAdapter;` import + `SimType::IRacing => Some(Box::new(IracingAdapter::new(pod_id.clone())))` arm
- `crates/rc-agent/src/event_loop.rs` - Added `Some(rc_common::types::SimType::IRacing)` arm using `adapter.read_is_on_track()` before catch-all fallback

## Decisions Made

- iRacing billing uses IsOnTrack from shared memory (accurate — fires when player is actually on track) rather than the generic 90s process fallback (coarse — fires when process has been running 90s regardless of sim state).
- `read_is_on_track` follows the `read_ac_status` pattern — default None on the trait, overridden by IracingAdapter's `impl SimAdapter` block.
- The iRacing arm checks `state.adapter` via the `dyn SimAdapter` trait, so it naturally handles the case where no adapter exists (returns None from the `if let Some(ref adapter)` guard).

## Deviations from Plan

### Pre-existing Task 1 Changes

Task 1 changes (`pub mod iracing;` + `read_is_on_track` trait method in sims/mod.rs) were found already committed in the codebase at plan start. They had been included in the 99-01 docs commit (`dad850c`) rather than as a separate Task 1 commit. Since the changes were correct and passing `cargo check`, no re-commit was needed. Task 2 proceeded immediately.

**Total deviations:** 0 auto-fixes. Task 1 was pre-committed correctly; Task 2 executed as specified.
**Impact on plan:** None — all must_haves satisfied, build passes, tests pass.

## Issues Encountered

None — `cargo build --release --bin rc-agent` succeeded on first attempt. `cargo check` exited 0. All acceptance criteria met.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- iRacing is fully integrated: adapter creates on launch, reads telemetry every 100ms, detects lap completions, and triggers billing via IsOnTrack
- Phase 84 (2/2 plans) is complete — all TEL-IR-xx requirements satisfied
- iRacing pods are ready for deployment: build rc-agent, push to Pod 8, verify, then fleet

---
*Phase: 84-iracing-telemetry*
*Completed: 2026-03-21*
