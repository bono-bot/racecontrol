# Phase 290: Wire Metric Producers - Context

**Gathered:** 2026-04-01
**Status:** Ready for planning
**Mode:** Auto-generated (gap closure phase)

<domain>
## Phase Boundary

Wire real metric producers into the MetricsSender channel created in Phase 285. Currently `_metrics_tx` is created but never used — TSDB is empty at runtime. This phase clones the sender and feeds it real data from existing server loops (pod health scores, WS connection counts, billing revenue, game session counts, etc.).

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices at Claude's discretion — gap closure infrastructure phase. Key constraints:
- Clone `_metrics_tx` (remove underscore prefix) and pass to existing loops
- Use `MetricSample` struct from `metrics_tsdb.rs` with `record_sample()` or channel send
- Capture at least: pod health score, WS connections, game session count, billing revenue
- CPU/GPU temp come from pod reports via WS — extract from existing `handle_pod_status_update`
- Do NOT create new background loops if data is already available in existing loops — just add a `.send()` call

</decisions>

<canonical_refs>
## Canonical References

- `crates/racecontrol/src/metrics_tsdb.rs` — MetricsSender type, MetricSample struct, record_sample()
- `crates/racecontrol/src/main.rs` — Line 711: `_metrics_tx` creation, existing spawn points
- `crates/racecontrol/src/ws/mod.rs` — Pod status updates with health scores, WS connection tracking
- `crates/racecontrol/src/billing.rs` — Billing session events, revenue tracking
- `.planning/v34.0-MILESTONE-AUDIT.md` — Gap 1 details

</canonical_refs>

<code_context>
## Existing Code Insights

### Integration Points
- `main.rs:711` — `let _metrics_tx = spawn_metrics_ingestion(...)` — rename to `metrics_tx`, clone to consumers
- WS handler receives pod heartbeats with health_score, cpu_usage, gpu_temp — add `.send()` there
- Fleet health loop already iterates pods — add metrics recording there
- Billing events already fire on session start/end — add revenue metric there

</code_context>

<specifics>
## Specific Ideas

No specific requirements — straightforward producer wiring.

</specifics>

<deferred>
## Deferred Ideas

None.

</deferred>
