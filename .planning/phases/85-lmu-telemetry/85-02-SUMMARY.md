---
phase: 85-lmu-telemetry
plan: 02
subsystem: telemetry
tags: [rust, lmu, le-mans-ultimate, rf2, shared-memory, billing, sim-adapter]

# Dependency graph
requires:
  - phase: 85-lmu-telemetry-plan-01
    provides: LmuAdapter struct with rF2 shared memory read_is_on_track() via SimAdapter trait

provides:
  - LmuAdapter wired into rc-agent main.rs adapter creation for SimType::LeMansUltimate
  - LMU PlayableSignal arm in event_loop.rs using read_is_on_track() instead of 90s process fallback
  - pub mod lmu confirmed in sims/mod.rs (added in plan 01)

affects: [85-lmu-telemetry, rc-agent deployment, billing accuracy for LMU pods]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "SimAdapter pattern extended: new sim type added via 3-file integration (mod.rs, main.rs, event_loop.rs)"
    - "PlayableSignal dispatch: dedicated match arm per sim type using read_is_on_track() trait method"

key-files:
  created: []
  modified:
    - crates/rc-agent/src/sims/mod.rs
    - crates/rc-agent/src/main.rs
    - crates/rc-agent/src/event_loop.rs

key-decisions:
  - "LMU PlayableSignal arm mirrors iRacing arm exactly — uses adapter.read_is_on_track() via dyn SimAdapter dispatch, no special casing needed"
  - "pub mod lmu was already added in plan 01 (deviation auto-fix for test compilation) — Task 1 verified this and skipped redundant edit"

patterns-established:
  - "New sim integration checklist: (1) pub mod in sims/mod.rs (2) use + match arm in main.rs (3) PlayableSignal arm in event_loop.rs"

requirements-completed: [TEL-LMU-01, TEL-LMU-03]

# Metrics
duration: 12min
completed: 2026-03-21
---

# Phase 85 Plan 02: LMU Wiring Summary

**LmuAdapter wired into rc-agent: SimType::LeMansUltimate creates adapter in main.rs, dedicated PlayableSignal arm in event_loop.rs replaces 90s process fallback with rF2 shared memory IsOnTrack**

## Performance

- **Duration:** 12 min
- **Started:** 2026-03-21T08:45:00+05:30
- **Completed:** 2026-03-21T08:57:00+05:30
- **Tasks:** 2 completed
- **Files modified:** 2 (mod.rs already done in plan 01)

## Accomplishments

- Verified `pub mod lmu;` already in sims/mod.rs from plan 01 deviation fix — skipped redundant edit
- Added `use sims::lmu::LmuAdapter` import and `SimType::LeMansUltimate => LmuAdapter::new(pod_id)` arm in main.rs
- Added dedicated `SimType::LeMansUltimate` PlayableSignal arm in event_loop.rs using `adapter.read_is_on_track()` — LMU no longer falls through to 90s process-based fallback
- Full test suite: rc-common 135 passed, rc-agent compiles clean, release build succeeds

## Task Commits

Each task was committed atomically:

1. **Task 1: Register LMU module and create adapter in main.rs** - `e32391f` (feat)
2. **Task 2: Add LMU PlayableSignal arm in event_loop.rs** - `3fa9de5` (feat)

**Plan metadata:** (docs commit below)

## Files Created/Modified

- `crates/rc-agent/src/sims/mod.rs` - `pub mod lmu;` already present (plan 01), verified only
- `crates/rc-agent/src/main.rs` - Added `use sims::lmu::LmuAdapter` + `SimType::LeMansUltimate` match arm
- `crates/rc-agent/src/event_loop.rs` - Added LMU PlayableSignal dispatch arm, updated fallback comments

## Decisions Made

- LMU PlayableSignal arm mirrors the iRacing arm exactly — `dyn SimAdapter::read_is_on_track()` trait dispatch works without any special casing
- Updated catch-all comment to remove LMU from "LMU, EVO, WRC, Forza" list since LMU now has its own arm

## Deviations from Plan

None — plan executed exactly as written. The pre-noted condition (pub mod lmu already in mod.rs) was verified and confirmed before any edit attempt.

## Issues Encountered

Two pre-existing test failures in `racecontrol-crate` (`config::tests::config_fallback_preserved_when_no_env_vars` and `crypto::encryption::tests::load_keys_wrong_length`) — unrelated to LMU wiring, not introduced by this plan. Out of scope per deviation rules.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- LMU is now fully wired: adapter created, PlayableSignal dispatched via shared memory IsOnTrack
- Billing for LMU pods will trigger accurately on IsOnTrack=true instead of waiting 90s
- Ready for Phase 85 verification or deployment to LMU-configured pods

---
*Phase: 85-lmu-telemetry*
*Completed: 2026-03-21*
