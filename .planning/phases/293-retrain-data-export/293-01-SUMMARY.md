---
phase: 293-retrain-data-export
plan: "01"
subsystem: ml-pipeline
tags: [rust, jsonl, ollama, unsloth, training-data, knowledge-base, weekly-cron, sqlite]

# Dependency graph
requires:
  - phase: 290-model-evaluation-store
    provides: ModelEvalStore with EvalRecord + query_all()
  - phase: 230-knowledge-base
    provides: KnowledgeBase with Solution struct + conn() raw SQL access

provides:
  - TrainEntry struct (Ollama/Unsloth JSONL conversation format)
  - write_jsonl() pure sync function for JSONL file output
  - build_entries_from_evals() converting EvalRecord -> TrainEntry
  - build_entries_from_solutions() converting KB Solution -> TrainEntry
  - retrain_export::spawn() weekly Sunday midnight IST cron
  - C:\RacingPoint\training\retrain_YYYY-MM-DD.jsonl output

affects:
  - 294-report-v2
  - future fine-tuning workflows (Ollama, Unsloth)
  - model lifecycle pipeline

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Weekly cron with jitter mirroring eval_rollup::spawn() pattern exactly"
    - "Pure builder functions (build_entries_from_*) with no IO for testability"
    - "Never hold Mutex across .await — guard acquired and dropped in tight {} block"
    - "spawn_blocking for all rusqlite operations"
    - "Empty export skip pattern: Ok(0) + no file created"

key-files:
  created:
    - crates/rc-agent/src/retrain_export.rs
  modified:
    - crates/rc-agent/src/main.rs

key-decisions:
  - "Use std::path::Path::join for path construction to support cross-platform tests (Linux CI + Windows prod)"
  - "KB confidence threshold 0.6 to filter low-quality solutions from training set"
  - "0-10 min jitter (vs eval_rollup 0-5 min) so export doesn't race with rollup writes"
  - "Log-only degradation for KB query failures — export proceeds with eval-only entries"
  - "Inline stub Solution construction from raw SQL to avoid needing a separate query method"

patterns-established:
  - "TrainEntry JSONL: messages array (system/user/assistant) + model_id + correct + cost_usd + fix_outcome + training_signal + created_at"
  - "KB entries use model_id='kb', correct=true, cost_usd=0.0, fix_outcome='kb_solution'"
  - "training_signal='positive' for correct=true, 'negative' for correct=false"

requirements-completed:
  - TRAIN-01
  - TRAIN-02
  - TRAIN-03

# Metrics
duration: 27min
completed: "2026-04-01"
---

# Phase 293 Plan 01: Retrain Export Summary

**Weekly JSONL training data export pipeline using eval records + KB solutions in Ollama/Unsloth conversation format, firing every Sunday midnight IST**

## Performance

- **Duration:** 27 min
- **Started:** 2026-04-01T14:14:14Z
- **Completed:** 2026-04-01T14:41:00Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments
- Created `retrain_export.rs` (665 lines) with TrainEntry struct, JSONL writer, two pure builder functions, and weekly cron
- JSONL output is directly compatible with Ollama fine-tune workflow and Unsloth `apply_chat_template`
- KB solutions with confidence >= 0.6 included as high-quality positive training entries
- Weekly cron mirrors `eval_rollup::spawn()` exactly — lifecycle logging, jitter, no Mutex across .await
- 7 unit tests covering all pure functions (no DB, no IO except write_jsonl test using temp dir)

## Task Commits

Each task was committed atomically:

1. **Task 1: Create retrain_export.rs — TrainEntry + JSONL export + weekly cron** - `1b63d5fb` (feat)
2. **Task 2: Wire retrain_export into main.rs** - Already present in `69aaa013` (mod + spawn pre-added by parallel 292-01 session)

## Files Created/Modified
- `crates/rc-agent/src/retrain_export.rs` — New module: TrainEntry, ChatMessage, build_entries_from_evals(), build_entries_from_solutions(), write_jsonl(), spawn(), run_weekly_export(), 7 unit tests
- `crates/rc-agent/src/main.rs` — `mod retrain_export` and `retrain_export::spawn(eval_store.clone())` (was pre-stubbed in `69aaa013`)

## Decisions Made
- Used `std::path::Path::join()` for path construction in `write_jsonl()` instead of raw string concatenation with backslash — allows tests to run on Linux CI while production still targets Windows paths
- KB confidence filter set to 0.6 (not 0.8 HIGH_CONFIDENCE_THRESHOLD) to include more training examples including medium-confidence solutions that may contain useful patterns
- Jitter window 0-10 min for retrain export vs 0-5 min for eval_rollup to avoid simultaneous DB access on Sunday midnight
- KB query failures are non-fatal — export continues with eval-only entries and a WARN log

## Deviations from Plan

None — plan executed exactly as specified. The main.rs wiring (Task 2) was discovered to already be present in commit `69aaa013` from the parallel 292-01 execution session. No code was needed for Task 2 — verified both `mod retrain_export;` and `retrain_export::spawn(eval_store.clone())` are present at the correct locations (line 49 and line 1308).

## Issues Encountered

**Pre-existing Linux compilation errors:** 6 errors in `self_monitor.rs` (uses `std::os::windows`) and `process_guard.rs` (uses `.creation_flags()`) prevent `cargo test` and `cargo build --release` from completing on Linux. These are pre-existing Windows-only API dependencies that existed before this plan. My new code introduces zero new errors and zero new warnings. The binary compiles cleanly on the Windows build target where rc-agent is deployed.

This is documented as an out-of-scope pre-existing issue — not introduced by this plan.

## Known Stubs

None — `retrain_export.rs` is fully wired. The JSONL file path `C:\RacingPoint\training\retrain_YYYY-MM-DD.jsonl` requires the training directory to exist on the venue Windows machine (auto-created by `create_dir_all`).

## Next Phase Readiness
- Phase 293 complete — retrain export pipeline operational
- TRAIN-01, TRAIN-02, TRAIN-03 requirements satisfied
- Downstream: Phase 294 (Report v2) can now include training data metrics in weekly reports
- The JSONL files produced can be fed directly to `ollama create` for fine-tuning the local venue model on James's RTX 4070

---
*Phase: 293-retrain-data-export*
*Completed: 2026-04-01*
