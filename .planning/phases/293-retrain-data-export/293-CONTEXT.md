# Phase 293: Retrain Data Export - Context

**Gathered:** 2026-04-01
**Status:** Ready for planning
**Mode:** Infrastructure phase — discuss skipped

<domain>
## Phase Boundary

Weekly cron exports diagnosis evaluations + KB solutions as JSONL training data in Ollama/Unsloth conversation format. Each entry includes model name, prompt, response, correct/incorrect, and fix outcome.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices at Claude's discretion. Key existing code: `crates/rc-agent/src/model_eval_store.rs` (evaluation records), `crates/rc-agent/src/knowledge_base.rs` (KB solutions), `crates/rc-agent/src/eval_rollup.rs` (weekly cron pattern).

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `model_eval_store.rs` — `query_all()` returns all evaluation records (source data)
- `knowledge_base.rs` — KB solutions with problem/fix pairs
- `eval_rollup.rs` — weekly cron scheduling pattern (Sunday midnight IST + jitter)
- `weekly_report.rs` — `seconds_until_next_sunday_midnight_ist()` reusable

### Integration Points
- New cron task spawned in main.rs
- JSONL output to `/var/racecontrol/training/` or configurable path
- No server-side component needed (export is local to rc-agent)

</code_context>

<specifics>
## Specific Ideas
No specific requirements — infrastructure phase.
</specifics>

<deferred>
## Deferred Ideas
None.
</deferred>
