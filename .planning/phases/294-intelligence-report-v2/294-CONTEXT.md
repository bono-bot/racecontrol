# Phase 294: Intelligence Report v2 - Context

**Gathered:** 2026-04-01
**Status:** Ready for planning
**Mode:** Infrastructure phase — discuss skipped

<domain>
## Phase Boundary

Enhance the existing weekly_report.rs (v32.0 Phase 279) with per-model accuracy rankings, KB promotion counts, cost savings from hardened rules, and model accuracy trend analysis. Depends on Phase 290 (eval store), Phase 291 (KB promotions), Phase 292 (model reputation).

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All choices at Claude's discretion. Enhance existing `weekly_report.rs` — do not create a new module. Use data from `model_eval_store.rs` (eval records), `kb_promotion_store.rs` (promotion counts), `model_reputation_store.rs` (accuracy trends). Report sent via existing EscalationPayload/WhatsApp path.

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `weekly_report.rs` — existing Sunday midnight cron, `collect_report()`, `format_whatsapp_message()`, sends via EscalationPayload
- `model_eval_store.rs` — `query_all()` / `query_by_model()` for evaluation data
- `eval_rollup.rs` — `ModelEvalRollupStore` with per-model accuracy rollups
- `kb_promotion_store.rs` — `all_candidates()` for promotion pipeline data
- `model_reputation_store.rs` — `load_all()` for per-model trends

### Integration Points
- `weekly_report::spawn()` already in main.rs — enhance, don't add new spawn
- Pass eval_store, promo_store, rep_store to weekly_report::spawn()
- Enhance `format_whatsapp_message()` with new sections

</code_context>

<specifics>
## Specific Ideas
No specific requirements — infrastructure phase.
</specifics>

<deferred>
## Deferred Ideas
None.
</deferred>
