# Phase 267: Survival Foundation - Context

**Gathered:** 2026-03-30
**Status:** Ready for planning
**Mode:** Auto-generated (infrastructure phase — discuss skipped)

<domain>
## Phase Boundary

All 5 existing recovery systems coordinate via shared sentinel protocol and structured types so they cannot fight each other over the same patient. Defines HEAL_IN_PROGRESS sentinel, server-arbitrated heal lease, structured action_id logging, survival types in rc-common (SurvivalReport, HealLease, BinaryManifest, DiagnosisContext + OpenRouter client trait), and updates all 5 existing recovery paths (rc-sentry, RCWatchdog, self_monitor, pod_monitor, WoL) to check sentinels before acting.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase. Key constraints from research:
- OpenRouter client: define TRAIT in rc-common only (no reqwest dependency in shared crate)
- HEAL_IN_PROGRESS sentinel: JSON file at C:\RacingPoint\HEAL_IN_PROGRESS with {layer, started_at, action, ttl_secs}
- Server heal lease: POST /api/v1/pods/{id}/heal-lease endpoint
- All existing recovery systems must check both HEAL_IN_PROGRESS and OTA_DEPLOYING before acting
- action_id: UUID v4 generated at diagnosis start, propagated through all cross-layer operations

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- rc-common/src/mesh_types.rs — MeshSolution, SolutionStatus, FixType, DiagnosisTier
- rc-common/src/recovery.rs — RecoveryAction, RecoveryAuthority, RecoveryDecision, RecoveryLogger
- rc-common/src/verification.rs — ColdVerificationChain, VerifyStep
- rc-common/src/types.rs — WatchdogCrashReport, PodInfo, PodStatus
- rc-common/src/watchdog.rs — EscalatingBackoff

### Established Patterns
- Sentinel files at C:\RacingPoint\ (MAINTENANCE_MODE, OTA_DEPLOYING, sentry-restart-breadcrumb.txt)
- JSON payloads for sentinel files (MAINTENANCE_MODE already uses JSON with timestamp)
- serde derive for all shared types
- tracing for structured logging

### Integration Points
- rc-watchdog/src/service.rs — checks MAINTENANCE_MODE and sentry breadcrumb before restart
- racecontrol/src/pod_monitor.rs — heartbeat detection, delegates to pod_healer
- racecontrol/src/pod_healer.rs — graduated AI-driven recovery
- rc-agent/src/self_monitor.rs — self-health monitoring loop

</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase. Refer to ROADMAP phase description and success criteria.

MMA consensus findings to address:
- HEAL_IN_PROGRESS stale lock risk: TTL must be mandatory, auto-expire on TTL exceeded
- Server-arbitrated heal lease: server grants exclusive heal lease to one layer at a time
- Budget persistence: budget_state.json written on every OpenRouter call

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
