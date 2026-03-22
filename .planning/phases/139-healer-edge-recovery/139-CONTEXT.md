# Phase 139: Healer Edge Recovery - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Add HealAction::RelaunchLockScreen to the pod healer when lock screen HTTP check fails. Send ForceRelaunchBrowser WS message to pod. rc-agent handles ForceRelaunchBrowser by calling close_browser + launch_browser. Gate by billing_active to prevent recovery system conflicts.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase.

Key constraints:
- Phase 137 already landed: close_browser() with safe mode gate, launch_browser() now pub
- Phase 138 already landed: idle health loop in event_loop.rs, IdleHealthFailed in protocol.rs
- ForceRelaunchBrowser is a new CoreToAgentMessage variant (server→pod direction)
- HealAction::RelaunchLockScreen is a new enum variant in pod_healer.rs
- Standing rule #10: check billing_active before dispatching relaunch — recovery must not fight active sessions
- The healer already has Rule 2 (line 204) checking lock screen HTTP on :18923 — extend this to dispatch relaunch

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- pod_healer.rs Rule 2 (line 204-255) — already checks lock screen HTTP, currently only logs
- pod_healer.rs HealAction enum — add RelaunchLockScreen variant
- pod_healer.rs execute_heal_action() (line 561-603) — dispatch new action
- protocol.rs CoreToAgentMessage — add ForceRelaunchBrowser variant
- ws_handler.rs — handle CoreToAgentMessage::ForceRelaunchBrowser

### Integration Points
- rc-common/src/protocol.rs — new CoreToAgentMessage variant
- racecontrol/src/pod_healer.rs — new HealAction + dispatch
- rc-agent/src/ws_handler.rs — handle ForceRelaunchBrowser message
- racecontrol/src/ws/mod.rs — send ForceRelaunchBrowser to pod via WS

</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
