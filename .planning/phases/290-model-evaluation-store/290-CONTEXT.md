# Phase 290: Model Evaluation Store - Context

**Gathered:** 2026-04-01
**Status:** Ready for planning
**Mode:** Infrastructure phase — discuss skipped (pure data layer)

<domain>
## Phase Boundary

Every AI diagnosis writes prediction, actual outcome, correctness, and cost to SQLite `model_evaluations` table. Weekly rollup computes per-model accuracy and cost-per-correct-diagnosis. Query API exposes evaluation data. Foundation for all v35.0 phases.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — infrastructure phase with clear success criteria. Use existing patterns from knowledge_base.rs (SQLite access) and mma_engine.rs (model stats). Hook into the existing MMA diagnosis result path.

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `mma_engine::get_all_model_stats()` — returns per-model accuracy data (in-memory)
- `knowledge_base.rs` — SQLite access patterns for rc-agent's local DB
- `fleet_kb.rs` on server side — SQLite patterns for racecontrol server DB
- `weekly_report.rs` — existing weekly cron scheduling pattern

### Established Patterns
- SQLite via rusqlite (rc-agent) or sqlx (racecontrol server)
- Cron via tokio::time::interval in spawned tasks
- API endpoints via Axum handlers in racecontrol/src/api/

### Integration Points
- MMA diagnosis result in `tier_engine.rs` → write evaluation record
- Weekly cron in `main.rs` → produce rollup
- Server API in `racecontrol/src/api/` → query endpoint

</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase. Refer to ROADMAP phase description and success criteria.

</specifics>

<deferred>
## Deferred Ideas

None — discuss phase skipped.

</deferred>
