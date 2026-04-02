# Phase 292: Model Reputation Persistence - Context

**Gathered:** 2026-04-01
**Status:** Ready for planning
**Mode:** Infrastructure phase — discuss skipped

<domain>
## Phase Boundary

Per-model accuracy and run counts persist in SQLite across rc-agent restarts. Auto-demotion (< 30% accuracy) and promotion (> 90%) on 7-day rolling windows. Server API exposes per-model trends. Existing `model_reputation.rs` has in-memory sets from v32.0.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices at Claude's discretion. Key existing code: `crates/rc-agent/src/model_reputation.rs` (in-memory reputation), `crates/rc-agent/src/mma_engine.rs` (get_all_model_stats), `crates/rc-agent/src/model_eval_store.rs` (Phase 290 eval data).

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `model_reputation.rs` — existing `run_reputation_sweep()` with in-memory demotion/promotion
- `mma_engine.rs` — `get_all_model_stats()` returns per-model accuracy
- `model_eval_store.rs` — Phase 290 evaluation records (source for 7-day window calculations)
- `eval_rollup.rs` — Phase 290 weekly rollups with per-model accuracy

### Integration Points
- `model_reputation::run_reputation_sweep()` called from daily task in main.rs
- `mma_engine` roster — demotion/promotion modifies the active model list
- Server API — new endpoint `GET /api/v1/models/reputation`

</code_context>

<specifics>
## Specific Ideas
No specific requirements — infrastructure phase.
</specifics>

<deferred>
## Deferred Ideas
None.
</deferred>
