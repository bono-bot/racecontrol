# Phase 205: Verification Chain Foundation - Context

**Gathered:** 2026-03-26
**Status:** Ready for planning

<domain>
## Phase Boundary

Create stable rc-common types (VerificationChain, VerifyStep trait, VerificationError enum) and boot_resilience module (spawn_periodic_refetch) that all three executables can consume. This phase produces library code only — no consumer integration.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase. Key constraints from requirements:
- VerifyStep trait with Input/Output/Error associated types
- VerificationError enum via thiserror 2 with InputParseError, TransformError, DecisionError, ActionError variants
- Hot-path (async fire-and-forget ring buffer) vs cold-path (synchronous) distinction
- spawn_periodic_refetch() with lifecycle logging (started, first_success, exit)
- notify 8.2.0 added to workspace deps for sentinel file watching

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `rc-common/src/exec.rs` — shared sync/async exec primitive with feature-gated tokio boundary
- `rc-common/src/recovery.rs` — RecoveryAuthority, ProcessOwnership, RecoveryLogger (JSONL)
- `rc-common/src/watchdog.rs` — EscalatingBackoff
- Existing process guard allowlist re-fetch pattern (commit 821c3031) as reference for boot_resilience

### Established Patterns
- thiserror 2 for typed errors throughout workspace
- tracing 0.1 with info_span! for structured logging
- tokio::spawn for background tasks with tokio::time::interval for periodic work
- Feature-gated tokio boundary in rc-common (rc-sentry is sync/std::net only)

### Integration Points
- verification.rs consumed by: racecontrol (pod_healer.rs), rc-agent (process_guard.rs, main.rs), rc-sentry (watchdog.rs)
- boot_resilience.rs consumed by: rc-agent (feature_flags.rs, process_guard.rs)
- rc-sentry does NOT use tokio — boot_resilience is rc-agent/racecontrol only

</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase. Types must be generic enough for all 4 critical chains identified in the audit (pod healer curl parse, config URL load, allowlist enforcement, spawn verification).

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>
