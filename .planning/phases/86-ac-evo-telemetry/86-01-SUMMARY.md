---
phase: 86-ac-evo-telemetry
plan: 01
subsystem: telemetry
tags: [rust, shared-memory, winapi, sim-adapter, assetto-corsa-evo, tdd]

# Dependency graph
requires:
  - phase: 85-lmu-telemetry
    provides: LmuAdapter pattern — ShmHandle, zero-guard lap detection, SimAdapter trait
  - phase: 84-iracing-telemetry
    provides: IracingAdapter pattern — shared memory, warn-once flags, read_is_on_track
provides:
  - AssettoCorsaEvoAdapter implementing SimAdapter with graceful degradation
  - EVO sim type wired in main.rs (string match + adapter creation)
  - read_is_on_track() via physics speed/RPM for PlayableSignal
  - 7 unit tests covering TEL-EVO-01, TEL-EVO-02, TEL-EVO-03
affects:
  - event-loop
  - billing
  - pod-status

# Tech tracking
tech-stack:
  added: []
  patterns:
    - Warn-once pattern per failure type (warned_no_shm, warned_empty_graphics)
    - connect() returns Ok(()) with connected=false on missing SHM (not Err)
    - read_telemetry() returns Ok(None) on zero/empty state (not Err)
    - Zero-guard all lap emission: lap_ms > 0 and last_lap_count > 0
    - Physics-based PlayableSignal: speed > 5 km/h OR rpm > 500

key-files:
  created:
    - crates/rc-agent/src/sims/assetto_corsa_evo.rs
  modified:
    - crates/rc-agent/src/sims/mod.rs
    - crates/rc-agent/src/main.rs
    - crates/rc-common/src/types.rs
    - crates/racecontrol/src/api/routes.rs

key-decisions:
  - "AC EVO Early Access uses same acpmf_physics/graphics/static SHM names as AC1 — confirmed by motion sim community"
  - "connect() never returns Err — avoids repeated error logs since event loop retries on every tick when disconnected"
  - "read_telemetry() returns Ok(None) not Err on empty state — prevents event loop calling disconnect() on zero telemetry (Pitfall 5)"
  - "Separate warn-once flags: warned_no_shm vs warned_empty_graphics — each distinct failure logs exactly once per session"
  - "read_is_on_track() uses physics speed/RPM instead of graphics IsOnTrack — graphics struct empty in EVO Early Access"
  - "LapData.valid hardcoded to true — no reliable is_valid field in EVO EA graphics struct"

patterns-established:
  - "EVO adapter is ~80% copy-paste from AC1 with zero-guards added and sim_type changed"
  - "ShmHandle struct duplicated per-file (private, 4 lines) — same as iracing.rs, lmu.rs pattern"
  - "All lap detection gated on completed_laps > last_lap_count && last_lap_count > 0 && lap_ms > 0"

requirements-completed: [TEL-EVO-01, TEL-EVO-02, TEL-EVO-03]

# Metrics
duration: 14min
completed: 2026-03-21
---

# Phase 86 Plan 01: AC EVO Telemetry Summary

**AssettoCorsaEvoAdapter with warn-once zero-guard shared memory reads — AC1 struct offsets reused, graceful degradation when EVO Early Access SHM is absent or empty**

## Performance

- **Duration:** 14 min
- **Started:** 2026-03-21T07:07:17Z
- **Completed:** 2026-03-21T07:21:17Z
- **Tasks:** 2 of 2
- **Files modified:** 5

## Accomplishments
- New `assetto_corsa_evo.rs` implements `SimAdapter` with full graceful degradation (connect returns Ok even without EVO running, read_telemetry returns Ok(None) when disconnected)
- Zero-guard lap detection: no LapCompleted emitted with lap_time_ms = 0, last_lap_count guard prevents false-positive on first poll (Pitfall 4)
- Physics-based `read_is_on_track()` for earlier billing trigger: speed > 5 km/h OR rpm > 500
- EVO fully wired in main.rs: "assetto_corsa_evo"/"ac_evo"/"evo" config values map to AssettoCorsaEvoAdapter
- 7 unit tests all green; release build succeeds; rc-common + rc-agent test suites pass

## Task Commits

Each task was committed atomically:

1. **Task 1: AssettoCorsaEvoAdapter with zero-guard shared memory reads** - `b9cb7ba` (feat)
2. **Task 2: Wire EVO adapter into main.rs sim type matching and creation** - `79ff2b4` (feat)

## Files Created/Modified
- `crates/rc-agent/src/sims/assetto_corsa_evo.rs` - New EVO adapter: AssettoCorsaEvoAdapter, ShmHandle, offset modules, SimAdapter impl, 7 unit tests
- `crates/rc-agent/src/sims/mod.rs` - Added `pub mod assetto_corsa_evo;`
- `crates/rc-agent/src/main.rs` - EVO string match arm, AssettoCorsaEvoAdapter creation, AssettoCorsaEvoAdapter import, freedom_mode fix
- `crates/rc-common/src/types.rs` - freedom_mode field on PodInfo (pre-existing uncommitted work, committed here)
- `crates/racecontrol/src/api/routes.rs` - freedom_mode: None in PodInfo test fixtures (compilation fix)

## Decisions Made
- connect() returns Ok(()) with connected=false when SHM unavailable — prevents repeated error logs in the event loop's retry cycle
- Separate warn-once flags per failure mode (warned_no_shm, warned_empty_graphics) — each distinct issue logs exactly once
- Graphics and static handles are individually optional — physics-only mode is valid in EVO Early Access
- read_is_on_track() uses physics speed/RPM as PlayableSignal heuristic — graphics struct empty in EVO EA
- LapData.valid hardcoded to true — STATUS field in graphics is unreliable in EVO Early Access

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed missing `freedom_mode` field in PodInfo struct initializers**
- **Found during:** Task 2 (wiring main.rs — full test suite run)
- **Issue:** `freedom_mode: Option<bool>` was added to `PodInfo` in `rc-common/src/types.rs` (pre-existing uncommitted change) but was not added to the struct literal initializers in `main.rs` (1 location) and `racecontrol/src/api/routes.rs` (4 test fixtures), causing compile error E0063
- **Fix:** Added `freedom_mode: None` to all PodInfo initializers in affected files
- **Files modified:** `crates/rc-agent/src/main.rs`, `crates/racecontrol/src/api/routes.rs`, `crates/rc-common/src/types.rs` (committed as part of Task 2)
- **Verification:** `cargo build -p rc-agent-crate` and `cargo build -p racecontrol-crate` both succeed
- **Committed in:** `79ff2b4` (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (Rule 1 - compilation bug)
**Impact on plan:** Pre-existing struct field addition had broken compilation of main.rs and racecontrol tests. Fix required for cargo test to run. No scope creep.

## Issues Encountered
- `racecontrol-crate` integration tests for billing rates (test_billing_rates_create_inserts_and_cache_updates, test_billing_rates_update_invalidates_cache, test_billing_rates_delete_excludes_from_cost) fail with wrong tier counts. Pre-existing issue unrelated to EVO telemetry. Logged to deferred items.
- Package name is `rc-agent-crate` (not `rc-agent`) — discovered during test run, adjusted commands accordingly.

## Next Phase Readiness
- EVO adapter is complete and wired — set `pod.sim = "ac_evo"` in rc-agent.toml to activate
- No additional phases planned for this phase (phase 86 complete with plan 01)
- Billing accuracy: if EVO's graphics SHM remains empty in EA, billing falls through to 90s process-based fallback as designed

---
*Phase: 86-ac-evo-telemetry*
*Completed: 2026-03-21*

## Self-Check: PASSED

- `crates/rc-agent/src/sims/assetto_corsa_evo.rs` — FOUND
- `.planning/phases/86-ac-evo-telemetry/86-01-SUMMARY.md` — FOUND
- Commit `b9cb7ba` (Task 1) — FOUND
- Commit `79ff2b4` (Task 2) — FOUND
