# Phase 273: Event Pipeline & Safety Foundation - Context

**Gathered:** 2026-04-01
**Status:** Ready for planning
**Mode:** Auto-generated (infrastructure phase — discuss skipped)

<domain>
## Phase Boundary

All anomaly detection and fix application flows through an event-driven pipeline with safety guardrails that prevent runaway autonomous actions. This phase creates the foundational event bus (broadcast + mpsc hybrid), wires predictive alerts as first-class FleetEvents, implements blast radius limiting (max 2/10 nodes), per-action circuit breakers, idempotency keys, and ensures every resolved issue is recorded in KB.

Requirements: PRO-01, PRO-02, PRO-03, PRO-04, PRO-05, PRO-06, SAFE-01, SAFE-02, SAFE-03

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase. Key patterns from MMA council research:
- broadcast<FleetEvent> for fan-out + mpsc<Incident> for work queues into tier engine
- DashMap<NodeId, ActiveFix> + RAII FixGuard for blast radius limiting
- circuitbreaker-rs or hand-rolled CircuitBreaker (existing pattern in tier_engine.rs)
- Idempotency keys: node_id + rule_version + incident_fingerprint

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `tier_engine.rs` — existing 5-tier decision tree, CircuitBreaker struct, dedup map
- `diagnostic_engine.rs` — existing anomaly detection with 9+ trigger types, 5-min scan interval
- `mma_engine.rs` — 4-step convergence engine with domain rosters
- `knowledge_base.rs` — SQLite KB with solution storage and lookup
- `budget_tracker.rs` — per-node cost tracking
- `predictive_maintenance.rs` — 9 threshold-based alert checks (currently log-only)
- `mesh_gossip.rs` — solution propagation via WebSocket

### Established Patterns
- tokio::spawn for background tasks with lifecycle logging
- mpsc channels for inter-module communication (diagnostic_engine → tier_engine)
- DiagnosticEvent + DiagnosticTrigger for anomaly detection
- TierResult enum for fix outcomes
- CircuitBreaker struct in tier_engine.rs (consecutive failures + cooldown)

### Integration Points
- diagnostic_engine emits DiagnosticEvent via mpsc → tier_engine receives
- predictive_maintenance runs independently, alerts logged but not fed to tier_engine (GAP)
- tier_engine runs tiers 1-5, but Tier 5 is a stub (GAP)
- knowledge_base.rs has lookup but no KB-first gate before model calls (GAP)
- No blast radius limiter exists yet (GAP)
- No idempotency tracking exists yet (GAP)

</code_context>

<specifics>
## Specific Ideas

- Wire predictive_maintenance alerts as DiagnosticTrigger variants fed to tier_engine
- Add KB lookup gate in tier_engine BEFORE Tier 3/4 model calls (Q1 protocol)
- Blast radius limiter as a shared Arc<BlastRadiusLimiter> passed to tier_engine
- Idempotency via HashMap<String, Instant> with TTL cleanup (like existing dedup_map)
- Every TierResult::Fixed must call knowledge_base.record_solution()

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
