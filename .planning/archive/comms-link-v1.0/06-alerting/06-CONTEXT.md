# Phase 6: Alerting - Context

**Gathered:** 2026-03-12
**Status:** Ready for planning

<domain>
## Phase Boundary

Notify Uday via WhatsApp (through Bono's Evolution API) when James goes down or comes back online, with email as fallback when WebSocket is unavailable. Flapping suppression prevents alert floods during rapid crash/restart cycles.

</domain>

<decisions>
## Implementation Decisions

### Alert Origin & Routing
- Bono orchestrates ALL WhatsApp alerting -- both down-alerts (heartbeat timeout) and back-online (receives recovery signal from James)
- James sends recovery signal + status updates (crash count, cooldown state) to Bono over WebSocket
- Email fallback goes to BOTH Uday (usingh@racingpoint.in) AND Bono (bono@racingpoint.in)
- James sends down-email only after hitting 5min cooldown cap (final escalation step) -- not on every crash detection
- Evolution API details (instance name, API key, Uday's WhatsApp number) not yet available -- plan with env var placeholders, coordinate with Bono before execution

### WhatsApp Message Content
- Minimal one-liner style with status emoji
- Down alert: `🚨 James DOWN 14:32 (restart attempt 5, cooldown 5min)` -- includes crash attempt info
- Back-online: `✅ James UP 14:35 (down 3min, 2 restarts)` -- includes downtime duration and restart count
- Bono constructs these messages from heartbeat timeout / recovery signal data

### Flapping Suppression
- Both sides suppress independently: James suppresses email alerts locally, Bono suppresses WhatsApp alerts
- Email fallback alerts are also suppressed (one "James is down" email per suppression window)
- Cooldown approach (escalating vs fixed window) at Claude's discretion

### Email Fallback
- Check CommsClient state at event time: if DISCONNECTED → email immediately, if CONNECTED → send via WebSocket
- No try-then-fallback -- use current state to decide channel upfront
- Email has MORE detail than WhatsApp: include system metrics, cooldown history, log context
- WhatsApp stays minimal one-liner
- Subject lines typed for inbox scanning: `[ALERT] James DOWN` / `[RECOVERED] James UP`
- Use existing send_email.js via execFile pattern (from Phase 5)

### Claude's Discretion
- Recovery signal format: new message type vs enriched heartbeat (pick what fits existing protocol)
- Status update delivery: part of heartbeat payload vs separate messages on state change
- Flapping suppression implementation: EscalatingCooldown reuse vs simple fixed window
- Whether to queue recovery signal for WS delivery when connection resumes after email fallback
- Alert suppression window duration(s)

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `EscalatingCooldown` (watchdog.js): 5-step delay with ready()/recordAttempt()/reset() -- potential reuse for alert suppression
- `wireRunner()` (watchdog-runner.js): Already fires email on self_test_passed -- extend with alert logic
- `send_email.js` (racecontrol repo): CJS script, 3 CLI args (to, subject, body) via execFile
- `CommsClient` (comms-client.js): State machine with CONNECTED/RECONNECTING/DISCONNECTED -- check state for fallback decision
- `HeartbeatSender` (heartbeat-sender.js): Sends system metrics every 15s -- could carry watchdog state

### Established Patterns
- ESM modules with Object.freeze enums and private class fields (#field)
- DI via constructor options for testability
- EventEmitter for lifecycle events
- Fire-and-forget for email (don't block watchdog loop)
- node:test built-in test runner (zero deps)
- execFile not exec (avoids shell injection)

### Integration Points
- watchdog-runner.js: Add alert logic alongside existing email-to-Bono on self_test_passed
- ClaudeWatchdog events: crash_detected, self_test_passed, self_test_failed -- wire alert triggers
- CommsClient.send(): Send recovery/status messages to Bono
- EscalatingCooldown.step: Check if at 5min cap for down-email threshold

</code_context>

<specifics>
## Specific Ideas

- Down-email threshold: only when cooldown hits 5min cap (step index >= last step) -- James is genuinely stuck, not a quick blip
- Email body richer than WhatsApp: include CPU/memory from system-metrics.js, cooldown step history, exe path
- WhatsApp messages scannable in phone notification preview -- emoji prefix makes alert type visible at a glance
- Typed email subjects (`[ALERT]` / `[RECOVERED]`) for inbox filtering rules

</specifics>

<deferred>
## Deferred Ideas

- Daily health summary (uptime %, restart count, connection stability) -- Phase 8 (AL-05)
- Web dashboard for Uday -- v2 (EM-01)
- WhatsApp notification for pod issues at Racing Point (not comms-link scope)

</deferred>

---

*Phase: 06-alerting*
*Context gathered: 2026-03-12*
