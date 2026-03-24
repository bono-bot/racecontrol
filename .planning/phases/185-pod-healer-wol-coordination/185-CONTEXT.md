# Phase 185: pod_healer WoL Coordination - Context

**Gathered:** 2026-03-25
**Status:** Ready for planning

<domain>
## Phase Boundary

Server-side pod_healer changes: query recovery events API before WoL, write WOL_SENT sentinel, enforce ProcessOwnership registry at all restart call sites, implement recovery-intent.json with 2-min TTL. Changes to racecontrol crate (pod_healer.rs, pod_monitor.rs) and any sentinel handling.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — infrastructure phase. Key constraints from ROADMAP success criteria:
- WoL skipped when sentry restarted with spawn_verified=true within 60s
- WOL_SENT sentinel written via rc-sentry /exec before magic packet sent
- ProcessOwnership registry enforced at all four call sites (rc-sentry, self_monitor, pod_monitor, rc-watchdog)
- GRACEFUL_RELAUNCH sentinel distinguishes intentional restarts from crashes
- recovery-intent.json with 2-min TTL prevents simultaneous restarts from different authorities

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `pod_healer.rs` — existing pod healing logic with WoL
- `pod_monitor.rs` — heartbeat monitoring, pod status tracking
- `rc-common/src/recovery.rs` — ProcessOwnership registry, RecoveryAuthority enum
- `racecontrol/src/recovery.rs` — GET /api/v1/recovery/events endpoint (Phase 183)
- `racecontrol/src/state.rs` — AppState with recovery event store

### Established Patterns
- pod_healer uses AppState to access pod status and fleet health
- HTTP requests from server to pods via rc-sentry :8091/exec for remote commands
- Sentinel files at C:\RacingPoint\ on each pod

### Integration Points
- pod_healer queries GET /api/v1/recovery/events?pod_id=X&since_secs=60 (local, same binary)
- WOL_SENT written to pod via rc-sentry /exec endpoint before magic packet
- ProcessOwnership checks at restart call sites in pod_healer and pod_monitor

</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
