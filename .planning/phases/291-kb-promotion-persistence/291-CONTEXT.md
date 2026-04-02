# Phase 291: KB Promotion Persistence - Context

**Gathered:** 2026-04-01
**Status:** Ready for planning
**Mode:** Infrastructure phase — discuss skipped

<domain>
## Phase Boundary

KB promotion ladder (Shadow/Canary/Quorum/Hardened) persists in SQLite across rc-agent restarts. 6-hour cron evaluator automatically advances eligible candidates. Existing `kb_hardening.rs` has in-memory ladder from v32.0 — this phase adds SQLite persistence layer.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices at Claude's discretion. Key existing code: `crates/rc-agent/src/kb_hardening.rs` (in-memory ladder), `crates/rc-agent/src/knowledge_base.rs` (SQLite patterns). Reuse `mesh_kb.db` for persistence.

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `kb_hardening.rs` — existing 5-stage promotion logic (Observed→Shadow→Canary→Quorum→Hardened)
- `knowledge_base.rs` — SQLite rusqlite patterns, `mesh_kb.db` path
- `model_eval_store.rs` — newly created SQLite store pattern from Phase 290

### Integration Points
- `kb_hardening::spawn()` in main.rs — already spawned with fleet_bus_tx
- `knowledge_base.rs` solution_nodes table — tracks per-pod success counts

</code_context>

<specifics>
## Specific Ideas
No specific requirements — infrastructure phase.
</specifics>

<deferred>
## Deferred Ideas
None.
</deferred>
