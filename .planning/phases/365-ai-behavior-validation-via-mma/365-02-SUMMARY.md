---
phase: 365-ai-behavior-validation-via-mma
plan: "02"
subsystem: api
tags: [rust, openrouter, toml, mma, weekly-batch, tokio]

requires:
  - phase: 365-01
    provides: ai_behavior_samples table + ai_behavior_batch.rs module stub
provides:
  - Weekly MMA batch job (604800s interval, 1h initial delay) spawned from main.rs
  - OpenRouter 5-model consensus logic for lap time band derivation
  - KB TOML files written to .planning/kb/ai-behavior/{car}-{track}.toml
  - TierBand / KbEntry structs with TOML serialization
  - POST /api/v1/admin/ai-behavior-batch/run manual trigger endpoint
  - compute_consensus(), bands_agree(), slugify(), write_kb_entry() public functions
affects: [365-03]

tech-stack:
  added: []
  patterns:
    - "MMA consensus: 5-model pool, >=3 agree within 5% p50 tolerance = consensus"
    - "KB TOML format: [tier_name] sections with p10_ms/p50_ms/p90_ms/consensus_models/samples_used"
    - "Slug format: lowercase, spaces -> dashes, non-alphanum stripped"
    - "Weekly batch: tokio::time::interval(604800s) with 3600s initial delay to avoid boot congestion"

key-files:
  created: []
  modified:
    - crates/racecontrol/src/ai_behavior_batch.rs
    - crates/racecontrol/src/main.rs
    - crates/racecontrol/src/api/routes.rs

key-decisions:
  - "5 OpenRouter models: claude-3.5-sonnet, gpt-4o, gemini-1.5-pro, mistral-large, deepseek-chat"
  - "Consensus threshold: >=3/5 models within 5% of p50 median"
  - "MAX_TUPLES_PER_BATCH=20, MIN_SAMPLES_PER_TUPLE=10 (last 30 days)"
  - "OpenRouter key: OPENROUTER_KEY env var first, then data/openrouter-mma-key.txt"
  - "KB files at .planning/kb/ai-behavior/{car_slug}-{track_slug}.toml (directory auto-created)"

requirements-completed: [GLD-E-02, GLD-E-03]

duration: estimated
completed: 2026-04-11
---

# Phase 365 Plan 02: Weekly MMA batch + KB file format Summary

**Weekly OpenRouter 5-model MMA consensus batch that queries ai_behavior_samples, derives expected lap time bands per (car, track, difficulty_tier), and writes KB TOML files to .planning/kb/ai-behavior/**

## Performance

- **Duration:** committed as part of prior agent session
- **Completed:** 2026-04-11
- **Tasks:** 2 (consensus logic + OpenRouter call/KB write/batch spawner)
- **Files modified:** 3

## Accomplishments

- Implemented `TierBand` and `KbEntry` structs with TOML serialization (`to_toml_string()`, `file_path()`, `slugify()`)
- Implemented `bands_agree()` (5% p50 tolerance) and `compute_consensus()` (largest agreeing group >= 3 = consensus, averaged band)
- Implemented `query_model_for_band()` OpenRouter HTTP call with JSON response parsing and sanity validation (p10 < p50 < p90, all positive, < 10 min)
- Implemented `write_kb_entry()` that creates `.planning/kb/ai-behavior/` directory and writes TOML file
- Implemented `run_ai_behavior_batch_cycle()` full batch: queries tuples with >= 10 samples from last 30 days (max 20), calls 5 models per tier, writes consensus KB files
- Replaced `spawn_ai_behavior_batch()` stub with full implementation: 1h initial delay + 604800s (7-day) interval loop
- Spawned batch task from `main.rs` after `spawn_data_retention_job`
- Added `POST /api/v1/admin/ai-behavior-batch/run` manual trigger endpoint with admin auth
- 4 new tests pass: consensus_3_of_5_agree, consensus_2_of_5_no_consensus, slugify, toml_output_format

## Task Commits

1. **Tasks 365-02-01+02: Consensus logic + batch spawner** - `ced70634` (feat)

## Files Created/Modified

- `crates/racecontrol/src/ai_behavior_batch.rs` - Added TierBand/KbEntry/ModelBandResponse structs, consensus logic, OpenRouter caller, KB writer, full batch cycle and spawner
- `crates/racecontrol/src/main.rs` - Added tokio::spawn of ai_behavior_batch::spawn_ai_behavior_batch
- `crates/racecontrol/src/api/routes.rs` - Added POST /api/v1/admin/ai-behavior-batch/run endpoint

## Decisions Made

- MMA here means analytics batch (OpenRouter 5-model consensus), NOT the Unified MMA Protocol Q1-Q4 incident gate
- Batch grouped by (car, track) to produce one TOML file per pair with multiple tier sections
- No KB file written if no tier achieves consensus (batch logs the skip)
- Feature flag `phase365_mma_batch` kill-switch checked at batch cycle start

## Deviations from Plan

None - plan executed as specified. All acceptance criteria met: TierBand, compute_consensus, bands_agree, slugify, to_toml_string, spawn_ai_behavior_batch, run_ai_behavior_batch_cycle, MMA_MODELS const, write_kb_entry, main.rs spawn, routes.rs endpoint all present.

## Issues Encountered

None.

## Next Phase Readiness

- KB TOML files will be populated on first batch run (requires OPENROUTER_KEY or data/openrouter-mma-key.txt)
- Plan 03 anomaly detector can read KB files via read_kb_entry() already implemented in 365-01

---
*Phase: 365-ai-behavior-validation-via-mma*
*Completed: 2026-04-11*
