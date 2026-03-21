# Phase 8: Coordination & Daily Ops - Context

**Gathered:** 2026-03-12
**Status:** Ready for planning

<domain>
## Phase Boundary

Enable real-time structured coordination between James and Bono (task delegation, status queries, notifications), deliver twice-daily health summaries to Uday via WhatsApp + email, retire Bono's legacy [FAILSAFE] heartbeat, and document the full coordination protocol with Mermaid sequence diagrams.

</domain>

<decisions>
## Implementation Decisions

### Coordination Message Types
- Full coordination suite: task delegation, status queries, and one-way notifications
- Fully bidirectional -- both James and Bono can initiate any message type
- Hybrid message format: typed commands for common operations (deploy, check-status, restart) + generic `message` type for freeform coordination
- The existing `message` type in protocol.js is available for freeform; add new typed commands for structured operations

### Daily Health Summary
- Twice daily: morning (9:00 AM IST, covers overnight) + evening (11:00 PM IST, covers daytime)
- Metrics: uptime percentage, restart count, connection stability (reconnection count, longest disconnect, latency), pod/venue status
- Both channels: WhatsApp (minimal one-liner style per Phase 6) + email (detailed with tables)
- Both AIs contribute: Bono aggregates connection/uptime data from heartbeat monitoring, James adds pod/venue status. Bono computes and sends the combined summary to Uday
- James sends a `daily_report` coordination message to Bono with pod/venue data before each summary window

### [FAILSAFE] Retirement
- Claude's discretion on approach (clean replacement vs integration) based on functional overlap analysis
- Claude's discretion on scope (comms-link changes vs coordination instructions for Bono's VPS code)
- Must ensure no gap in monitoring coverage during transition -- new system must fully replace [FAILSAFE] before it's removed

### Protocol Documentation
- Claude's discretion on file location (PROTOCOL.md, JSDoc in protocol.js, or both)
- Must include Mermaid sequence diagrams showing message flows (task request -> ack -> result, etc.)
- Documentation must be agreed/usable by both AIs as a reference

### Claude's Discretion
- Task request flow pattern (immediate execute vs ack-then-execute) -- pick based on existing ack patterns in codebase
- Specific typed command names for protocol.js
- [FAILSAFE] retirement approach (clean replacement vs gradual integration)
- [FAILSAFE] scope (comms-link only vs both-sides coordination)
- Protocol doc format and location
- Health summary scheduling mechanism (cron-like timer, setInterval, etc.)
- How pod/venue status data is collected and transmitted from James to Bono

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `AlertManager` + `sendEvolutionText` (bono/alert-manager.js): WhatsApp delivery already implemented with DI, cooldown, fire-and-forget
- `HeartbeatMonitor` (bono/heartbeat-monitor.js): Tracks isUp, lastHeartbeat, lastPayload -- has uptime data for health summary
- `protocol.js`: MessageType already includes `message`, `status`, `recovery` -- extend with new coordination types
- `send_email.js` (racecontrol): Email delivery via execFile for detailed health summaries
- `system-metrics.js` (james/): collectMetrics() for CPU/memory/uptime -- already in heartbeat payload
- `wireBono()` / `wireRunner()`: Established wiring patterns for new message routing

### Established Patterns
- ESM modules with Object.freeze enums and private class fields (#field)
- DI via constructor options for testability (sendFn, collectFn, nowFn)
- EventEmitter for lifecycle events
- Fire-and-forget for non-critical operations (email, WhatsApp)
- node:test built-in test runner (178 tests across 16 files)
- `createMessage()` / `parseMessage()` for all WebSocket messages

### Integration Points
- `bono/index.js wireBono()`: Add coordination message routing alongside heartbeat/recovery
- `james/watchdog-runner.js wireRunner()`: Add coordination message handling alongside existing event wiring
- `shared/protocol.js MessageType`: Add new coordination message types
- `AlertManager`: Extend or create DailySummary class alongside for scheduled summaries

</code_context>

<specifics>
## Specific Ideas

- Pod/venue status should give Uday a quick "all pods healthy" or "Pod 3 offline" -- he checks this from his phone
- WhatsApp summary should be scannable in notification preview (emoji prefix like Phase 6 alerts)
- Email summary serves as daily archive -- Uday can search past summaries in Gmail
- The morning summary is especially important since Racing Point is closed overnight -- Uday wants to know if anything happened

</specifics>

<deferred>
## Deferred Ideas

- Web-based status dashboard for Uday (EM-01, v2)
- Historical uptime tracking and graphs (EM-02, v2)
- Connection latency monitoring (EM-03, v2)
- Sync additional shared files beyond LOGBOOK.md (AS-01, v2)

</deferred>

---

*Phase: 08-coordination-daily-ops*
*Context gathered: 2026-03-12*
