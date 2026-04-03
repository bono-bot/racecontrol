---
gsd_state_version: 1.0
milestone: v40.0
milestone_name: Game Launch Reliability
status: executing
last_updated: "2026-04-03T05:38:22.019Z"
last_activity: 2026-04-03 — Phase 314 Plan 01 complete (BATOM-01/02)
progress:
  total_phases: 8
  completed_phases: 8
  total_plans: 11
  completed_plans: 14
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-04-03)

**Core value:** Proactive game availability management — stop showing customers games they can't play, flag broken combos before launch, surface failures instantly through Meshed Intelligence.
**Current focus:** Defining requirements

## Current Position

Phase: 315 (Game Intelligence Shared Types)
Plan: 01 complete
Status: Executing
Last activity: 2026-04-03 — Phase 315 Plan 01 complete (v41.0 shared types foundation)

## Accumulated Context

- v40.0 Game Launch Reliability: Phase 311 complete, Phase 312 complete (b7359a02), Phase 313 Plan 01 complete (eb0db70b), Phase 314 Plan 01 complete (3de35d50)
- v41.0 Game Intelligence System: Phase 315 Plan 01 complete (4e6a2717) — shared types foundation in rc-common
- content_scanner.rs only scans AC content — Steam/non-Steam games invisible to system
- combo_reliability table + GamePresetWithReliability exist from Phase 298 (Config Management)
- Game Doctor (12-point check) exists but runs reactively at launch, not proactively
- Meshed Intelligence tier_engine.rs handles GameLaunchFail — needs new triggers (GameLaunchTimeout, CrashLoop)
- Not all pods have the same games (e.g., Forza Horizon 5) — showing unavailable games hurts business
- Per-combo reliability scoring infrastructure exists in preset_library.rs but isn't surfaced to kiosk

## Decisions

- Pod is always source of truth for game state, except Launching <30s (in-flight protection) — GSTATE-02
- 180s hard cap chosen to exceed any reasonable dynamic timeout (AC max ~120s) — GSTATE-01
- Backfill launched_at=None on first health tick rather than inline during reconciliation — GSTATE-01
- Per-pod lock: std::sync::Mutex<HashMap> for outer (brief hold), tokio::sync::Mutex for inner (held across .await) — BATOM-01
- [Phase 315]: All new types added to rc-common/types.rs to avoid cross-crate import cycles between rc-agent and racecontrol
