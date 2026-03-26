---
phase: 215-self-improving-intelligence
plan: 02
subsystem: intelligence
tags: [bash, jq, suggestion-engine, proposals, relay-exec, pattern-analysis, self-improvement]

# Dependency graph
requires:
  - phase: 215-self-improving-intelligence-01
    provides: pattern-tracker.sh with update_pattern_db(), suggestions.jsonl JSONL schema, trend-analyzer.sh with TREND_OUTLIER entries

provides:
  - scripts/intelligence/suggestion-engine.sh with run_suggestion_engine() and get_suggestions_json()
  - audit/results/proposals/ directory with per-proposal JSON files (id, category, bug_type, pod_ip, confidence, evidence, status, created_ts, total_count, fix_success_rate)
  - get_suggestions relay exec command registered via pattern-tracker.sh export
  - auto-detect.sh wired to run suggestion engine after trend analysis post-run

affects:
  - 215-self-improving-intelligence-03
  - relay exec API (get_suggestions command now available)
  - auto-detect.sh pipeline (suggestion engine runs every auto-detect run)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Proposal deduplication: scan existing PROPOSALS_DIR/*.json for status==pending before creating new"
    - "Relay exec via exported bash functions: export -f enables function dispatch by name"
    - "Lazy-load pattern: relay alias loads sourced dependency if not already in scope"
    - "Category mapping from fix rates: fix_success_rate < 0.5 + fix_applied_rate > 0.5 = new_autofix_candidate"
    - "Confidence score: min(1.0, total_count / 10.0) provides normalized signal strength"

key-files:
  created:
    - scripts/intelligence/suggestion-engine.sh
  modified:
    - scripts/auto-detect.sh
    - scripts/intelligence/pattern-tracker.sh

key-decisions:
  - "Proposal deduplication at write time (not read time) — scan pending proposals before creating new one to prevent duplicate accumulation across runs"
  - "get_suggestions registered in pattern-tracker.sh not suggestion-engine.sh — pattern-tracker is always sourced first, ensures relay alias available even if suggestion-engine not yet sourced"
  - "TREND_OUTLIER entries processed as threshold_tune proposals with fixed confidence 0.50 — separate from frequency-based regular proposals to avoid feedback loop"
  - "MIN_FREQUENCY read from auto-detect-config.json field suggestion_min_frequency (default 3) — operator-configurable without code change"

patterns-established:
  - "Relay command registration: export function in sourced script, function name = command name"
  - "Lazy-load relay alias: alias loads dependency script if primary function not in scope"

requirements-completed: [LEARN-02, LEARN-03, LEARN-06]

# Metrics
duration: 15min
completed: 2026-03-26
---

# Phase 215 Plan 02: Self-Improving Intelligence — Suggestion Engine Summary

**Suggestion engine that converts raw suggestions.jsonl pattern data into categorized JSON proposal files (6 categories, confidence scoring, deduplication) with relay exec inbox query via get_suggestions command**

## Performance

- **Duration:** ~15 min
- **Started:** 2026-03-26T03:51 IST
- **Completed:** 2026-03-26T04:06 IST
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments

- Created suggestion-engine.sh with run_suggestion_engine() that groups by bug_type+pod_ip, applies MIN_FREQUENCY threshold, assigns one of 6 categories, computes confidence, and writes structured proposal JSON files
- Registered get_suggestions as relay exec command via export in pattern-tracker.sh with lazy-load fallback
- Wired suggestion engine into auto-detect.sh pipeline: sourced in source block, called after run_trend_analysis in generate_report_and_notify()
- Integration smoke test confirmed: 5 injected rc_agent_crash_loop entries produced new_autofix_candidate proposal, readable via get_suggestions_json()

## Task Commits

Each task was committed atomically:

1. **Task 1: Create suggestion-engine.sh — analyze patterns and generate proposals** - `58cb55e7` (feat)
2. **Task 2: Wire suggestion engine into auto-detect.sh and register relay command** - `5d89a43c` (feat)

**Plan metadata:** (see final commit after SUMMARY)

## Files Created/Modified

- `scripts/intelligence/suggestion-engine.sh` — Exports run_suggestion_engine() (proposal generation from suggestions.jsonl) and get_suggestions_json() (sorted JSON array of all proposals)
- `scripts/auto-detect.sh` — Added suggestion-engine.sh source line; added run_suggestion_engine guard-wrap call after run_trend_analysis
- `scripts/intelligence/pattern-tracker.sh` — Added get_suggestions() relay alias with lazy-load pattern and export -f

## Decisions Made

- Proposal deduplication at write time: scan PROPOSALS_DIR/*.json for status==pending before creating new proposal — prevents duplicate proposals accumulating across repeated runs
- get_suggestions registered in pattern-tracker.sh not suggestion-engine.sh: pattern-tracker is always sourced first by auto-detect.sh, ensuring relay alias is available even if suggestion-engine was not yet loaded
- TREND_OUTLIER entries processed as threshold_tune proposals with fixed confidence 0.50, separate pass from regular frequency-based entries — avoids feedback loop from inflating regular counts
- MIN_FREQUENCY defaults to 3, readable from auto-detect-config.json.suggestion_min_frequency field

## Deviations from Plan

None — plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

- Proposal files land at audit/results/proposals/ on every auto-detect run once patterns accumulate
- get_suggestions relay exec command is registered and callable: `curl -s -X POST http://localhost:8766/relay/exec/run -H "Content-Type: application/json" -d '{"command":"get_suggestions","reason":"check proposals"}'`
- Phase 215-03 (approval loop / self-patch) can build on proposals dir and the get_suggestions API

---
*Phase: 215-self-improving-intelligence*
*Completed: 2026-03-26*
