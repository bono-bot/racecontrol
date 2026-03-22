# Phase 161: Pod Monitor Merge - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Merge pod_monitor's restart/WoL logic into pod_healer as the single recovery authority. Add billing-aware WoL gate and graduated 4-step response (wait → Tier 1 → AI escalation → staff alert).

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase.

Key constraints:
- pod_monitor.rs exists in crates/racecontrol/src/ — currently handles pod liveness + WoL + restart
- pod_healer.rs exists in crates/racecontrol/src/ — handles healing actions
- Phase 159 delivered CascadeGuard + RecoveryAuthority — healer already wired to it
- PMON-01: check billing_active BEFORE WoL — never wake a pod in MAINTENANCE_MODE or deliberate shutdown
- PMON-02: pod_monitor logic moves INTO pod_healer — one recovery authority, one code path
- PMON-03: graduated response tracked per-pod: step 1 (wait 30s), step 2 (Tier 1 fix), step 3 (AI escalation), step 4+ (alert staff)
- Standing rule #10: WoL auto-wake must check whether pod was deliberately taken offline

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- racecontrol/src/pod_monitor.rs — current pod monitoring + WoL logic
- racecontrol/src/pod_healer.rs — healing actions + CascadeGuard integration (Phase 159)
- rc-common/src/recovery.rs — RecoveryAuthority::PodHealer, RecoveryLogger
- racecontrol/src/fleet_health.rs — PodFleetStatus with in_maintenance flag

### Integration Points
- racecontrol/src/pod_healer.rs — absorb pod_monitor's restart/WoL logic
- racecontrol/src/main.rs or server startup — remove pod_monitor spawn if it becomes redundant
- fleet_health.rs — billing_active and in_maintenance flags already available

</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
