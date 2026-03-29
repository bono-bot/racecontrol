# Phase 257: Billing Edge Cases - Context

**Gathered:** 2026-03-29
**Status:** Ready for planning
**Mode:** Auto-generated (discuss skipped)

<domain>
## Phase Boundary

Edge cases in session lifecycle are handled correctly — inactivity, timeouts, extensions, and disputes all have defined behaviors. This phase addresses the remaining billing gaps identified by the MMA audit.

Requirements: BILL-01 (inactivity detection), BILL-02 (session countdown), BILL-03 (PWA timeout), BILL-04 (extension pricing), BILL-05 (billing start-time), BILL-06 (recovery time exclusion), BILL-07 (multiplayer billing), BILL-08 (dispute portal)

Depends on: Phase 252 (financial atomicity), Phase 253 (FSM integrity)

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
Key guidance:
- BILL-01: Agent-side inactivity monitor — track last input event (steering/pedal/button). If no input for configurable N minutes (default 10), send InactivityAlert to server → staff notification. Do NOT auto-end — just alert.
- BILL-02: Agent-side session countdown overlay. At 5 min remaining: yellow warning banner. At 1 min: red warning. Uses duration_minutes from launch args + billing timer elapsed.
- BILL-03: Server-side TTL on game_launch_requests table. PWA requests auto-expire after 10 minutes. Expired requests return "expired" status to PWA. Staff dashboard hides expired requests.
- BILL-04: Document that extensions use the current tier effective rate. Add extension_rate_policy field to billing config.
- BILL-05: Billing timer should start counting when GameStateUpdate(Running) is received from agent, not when staff clicks launch. Adjust the billing start flow.
- BILL-06: During crash recovery (CrashRecoveryState != Idle), billing timer is paused (already done in Phase 253). Add explicit tracking of recovery_pause_seconds.
- BILL-07: Add multiplayer_sessions table linking billing sessions to a shared room. When AC server crashes, pause ALL linked sessions simultaneously.
- BILL-08: Add dispute_requests table. PWA endpoint POST /customer/dispute. Staff review endpoint GET /admin/disputes. Staff can approve (trigger refund) or deny with reason.

</decisions>

<code_context>
## Existing Code Insights

### Key Files
- `crates/racecontrol/src/billing.rs` — BillingTimer, billing states, FSM from Phase 253
- `crates/racecontrol/src/api/routes.rs` — billing endpoints, game launch
- `crates/rc-agent/src/event_loop.rs` — agent event loop, crash recovery FSM
- `crates/rc-agent/src/ws_handler.rs` — LaunchGame handler, game state updates
- `crates/rc-agent/src/session_enforcer.rs` — session timer from Phase 256

</code_context>

<specifics>
## Specific Ideas

No specific requirements — refer to ROADMAP success criteria.

</specifics>

<deferred>
## Deferred Ideas

None.

</deferred>
