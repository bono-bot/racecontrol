---
phase: 365-ai-behavior-validation-via-mma
plan: "01"
subsystem: database
tags: [sqlite, rust, ai-behavior, ac-server, feature-flags]

requires: []
provides:
  - ai_behavior_samples SQLite table (14 columns, 2 indexes)
  - tier_for_level() mapping function (ai_level 0-100 -> difficulty tier string)
  - collect_ai_behavior_samples() session-end collector hook
  - spawn_ai_behavior_batch() stub for Plan 02
  - phase365_mma_batch and phase365_anomaly_detection feature flags seeded
  - Module ai_behavior_batch registered in lib.rs
  - Collector wired into ac_server.rs collect_and_persist_ac_results()
affects: [365-02, 365-03]

tech-stack:
  added: []
  patterns:
    - "Feature-flag kill-switch pattern: flags.get(key).map(|f| f.enabled).unwrap_or(true)"
    - "AI car detection: driver_guid.is_empty() && best_lap > 0 && lap_count >= 3"
    - "Percentile stats: median/p25/p75 computed from sorted lap time vectors"

key-files:
  created:
    - crates/racecontrol/src/ai_behavior_batch.rs
  modified:
    - crates/racecontrol/src/db/mod.rs
    - crates/racecontrol/src/ac_server.rs
    - crates/racecontrol/src/lib.rs

key-decisions:
  - "D-01: ai_behavior_samples is a new table separate from laps (no is_ai column on laps)"
  - "D-02: AI cars identified in AcResultEntry by driver_guid.is_empty()"
  - "D-17: ai_behavior_samples NOT synced to cloud (venue-specific data)"
  - "DB migration is automatic via CREATE TABLE IF NOT EXISTS on server startup"

requirements-completed: [GLD-E-01]

duration: estimated
completed: 2026-04-11
---

# Phase 365 Plan 01: ai_behavior_samples schema + collector Summary

**New SQLite table + Rust collector that records per-(car, track, ai_level) median AI lap times after every AC session, using empty driver_guid as AI car discriminator with feature-flag kill-switch**

## Performance

- **Duration:** committed as part of prior agent session
- **Completed:** 2026-04-11
- **Tasks:** 3 (DB schema, module creation, lib.rs + ac_server.rs wiring)
- **Files modified:** 4

## Accomplishments

- Added `ai_behavior_samples` table with 14 columns (id, session_id, pod_id, sim_type, car, track, ai_level, difficulty_tier, lap_count, median_lap_ms, p25_lap_ms, p75_lap_ms, sampled_at, kb_batch_id) plus 2 indexes (combo on car/track/tier, sampled_at)
- Created `ai_behavior_batch.rs` module with `tier_for_level()` mapping (5 tiers: rookie/amateur/semi_pro/pro/alien), `AiLapSample` struct with median/p25/p75 computations, and full `collect_ai_behavior_samples()` async collector
- Seeded `phase365_mma_batch` and `phase365_anomaly_detection` feature flags via INSERT OR IGNORE
- Registered module in `lib.rs` and hooked collector into `ac_server.rs` `collect_and_persist_ac_results()` with config_json-based ai_level extraction and feature-flag guard
- 4 unit tests pass: tier_for_level all tiers, median_odd, median_even, ai_car_detection_via_empty_guid

## Task Commits

1. **Task 365-01-01+02+03: All tasks in single commit** - `773fff93` (feat)

## Files Created/Modified

- `crates/racecontrol/src/ai_behavior_batch.rs` - New module: tier mapping, AiLapSample, collector, weekly batch stub
- `crates/racecontrol/src/db/mod.rs` - Added ai_behavior_samples CREATE TABLE + 2 indexes + 2 feature flag seeds
- `crates/racecontrol/src/ac_server.rs` - Hook: collect_ai_behavior_samples() + check_and_broadcast_anomaly() at session end
- `crates/racecontrol/src/lib.rs` - Added `pub mod ai_behavior_batch`

## Decisions Made

- AI cars are identified by empty driver_guid in AcResultEntry (not a separate is_ai field on laps table)
- ai_behavior_samples table is NOT synced to cloud (venue-specific behavioral data)
- Feature flag kill-switch applied at collector entry: if flag disabled, skip silently
- DB migration is automatic (CREATE TABLE IF NOT EXISTS) — no migration script needed

## Deviations from Plan

None - the prior agent executed all 3 tasks as specified. Note: commit `773fff93` bundled all 3 plan tasks (DB schema, module, wiring) into a single commit rather than 3 separate commits — this was the prior agent's approach and is acceptable.

## Issues Encountered

None.

## Next Phase Readiness

- ai_behavior_samples table ready to receive data from session-end hooks
- `spawn_ai_behavior_batch` stub in place for Plan 02 to implement
- Feature flags seeded for both batch (Plan 02) and anomaly detection (Plan 03)

---
*Phase: 365-ai-behavior-validation-via-mma*
*Completed: 2026-04-11*
