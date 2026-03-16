# Phase 3: WebSocket Resilience - Context

**Gathered:** 2026-03-13
**Status:** Ready for planning

<domain>
## Phase Boundary

WebSocket connections survive game launch CPU spikes through ping/pong keepalive; rc-agent reconnects automatically with fast-then-backoff after any drop; kiosk debounces brief absences so staff never see a spurious "Disconnected" during normal game launch; WebSocket round-trips stay under 200ms; kiosk interactions respond within 100ms.

</domain>

<decisions>
## Implementation Decisions

### Kiosk disconnect UX
- Silent debounce: pod card stays green/online during the 15s debounce window. Staff only sees "Disconnected" after 15s confirmed absence. No false alarm flashing during game launches
- Single "Offline" state after debounce expires — no timed stages or age indicators. Activity log has timestamps if staff needs history
- Card color change only on offline — no toast notifications, no sound alerts. Uday already gets email alerts from Phase 2 watchdog
- Kiosk's OWN WebSocket connection also uses 15s debounce before showing disconnected in the header. Brief racecontrol restarts are invisible

### Pod screen during WS drop
- During active billing: customer sees NOTHING on WS drop. Game keeps running locally. Lock screen does not show "Disconnected". The drop is completely invisible to the customer
- During idle (no billing): lock screen shows "Disconnected" IMMEDIATELY on WS drop. No debounce for idle pods — staff needs to know unoccupied pods lost connection
- On reconnect after a drop during active billing: silent resume. No "Connection restored" toast or notification to the customer. They never knew anything happened
- Game keeps running during long WS drops (2+ minutes) — no warning overlay, no billing pause, no action on pod side. pod_monitor on racecontrol handles alerting staff via email
- Full re-register on every reconnect — pod sends fresh Register message with complete PodInfo. Same as initial connect. racecontrol gets accurate state immediately

### Reconnect aggressiveness
- rc-agent uses fast-then-backoff: first 3 attempts at 1s intervals (covers brief CPU spike blips), then exponential backoff 2s→4s→...→30s max
- Kiosk frontend keeps current 3s fixed retry interval — simple, fast enough for staff-facing LAN connection. 15s debounce hides brief drops anyway
- Both WS-level ping (from racecontrol) AND application-level heartbeat (from rc-agent at 5s) — belt-and-suspenders. WS ping keeps TCP alive during CPU spikes. App heartbeat carries pod state data
- racecontrol sends WS ping frames every 15s to all connected agents. Low overhead, frequent enough to prevent TCP idle timeout during shader compilation (typically 10-30s)

### Performance targets
- WS command round-trip (racecontrol → rc-agent → response) must complete under 200ms during normal operation on LAN
- ALL kiosk interactions — pod card clicks, page transitions, state updates, button responses — must respond within 100ms
- Log slow round-trips: tracing::warn! when WS command round-trip exceeds 200ms. No metrics dashboard, no Prometheus — just log lines for debugging
- No WebSocket message compression (permessage-deflate) — LAN bandwidth is not the bottleneck, pod state messages are small (~1-2KB), compression adds latency
- Optimize per-pod card updates: only re-render the specific pod card that changed (React.memo + stable keys), not all 8 cards on every WS message
- Use React 18 auto-batching for rapid-fire WebSocket messages — no custom requestAnimationFrame batching needed

### Claude's Discretion
- WS ping/pong implementation details (axum built-in vs manual frames)
- Exact debounce implementation in useKioskSocket.ts (setTimeout vs useRef timer)
- How to measure WS round-trip time for the tracing::warn! threshold
- React.memo granularity — which sub-components of pod cards to memoize
- Whether to add a pong timeout on racecontrol side (and what threshold)

</decisions>

<specifics>
## Specific Ideas

- Game launch shader compilation on pods typically takes 10-30s — the 15s WS ping interval must survive this window
- rc-agent already has a reconnection loop with exponential backoff (main.rs line ~417-462) — modify in place, don't rewrite
- useKioskSocket.ts (kiosk/src/hooks/) has a simple `setTimeout(connect, 3000)` in onclose — add debounce logic there
- Phase 2's `is_closed()` WS liveness check in pod_monitor/pod_healer must work correctly with the new ping/pong keepalive

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- `useKioskSocket.ts` (kiosk/src/hooks/): WebSocket hook with connect/reconnect, pod state management. Add debounce here
- rc-agent reconnection loop (main.rs ~417-462): Already has exponential backoff, modify to fast-then-backoff
- `agent_senders` + `agent_conn_ids` in AppState: Track connected agents, used for ping broadcasting
- `DashboardEvent::PodUpdate` (protocol.rs): Already broadcasts pod state changes to kiosk. Debounce on receive side
- `KioskHeader.tsx`: Shows connected/disconnected status — add debounce

### Established Patterns
- racecontrol ws/mod.rs: `handle_agent()` spawns send_task (mpsc → WS sender) + receive loop. WS ping goes in the send_task or a separate spawned task
- rc-agent main.rs: heartbeat_interval fires every 5s, sends Heartbeat AgentMessage. WS ping is separate (protocol-level frames)
- Kiosk: React functional components with hooks, Tailwind CSS, TypeScript interfaces in lib/types.ts

### Integration Points
- racecontrol ws/mod.rs: Add WS ping frame sending to agent connections (every 15s)
- rc-agent main.rs: Modify reconnect_delay logic (fast-then-backoff) + handle WS Ping frames (pong is automatic in tungstenite)
- kiosk/src/hooks/useKioskSocket.ts: Add 15s debounce timer for disconnect display + per-pod update optimization
- kiosk/src/components/KioskHeader.tsx: Debounce own connection indicator
- kiosk/src/components/KioskPodCard (or equivalent): Add React.memo wrapper

</code_context>

<deferred>
## Deferred Ideas

- Start billing timer on confirmed game launch (not at session creation) — billing/session management phase
- Billing timer pause during pod downtime — already captured as deferred in Phase 2
- WS message compression — explicitly decided against for LAN setup, revisit if multi-venue with WAN

</deferred>

---

*Phase: 03-websocket-resilience*
*Context gathered: 2026-03-13*
