# Phase 5: Watchdog Hardening - Context

**Gathered:** 2026-03-12
**Status:** Ready for planning

<domain>
## Phase Boundary

Harden the existing ClaudeWatchdog (watchdog.js) with escalating cooldown on repeated crashes, post-restart self-test, WebSocket re-establishment to Bono, and email notification on successful restart. The watchdog-runner.js becomes the integration point for all of these.

</domain>

<decisions>
## Implementation Decisions

### Escalating Cooldown (WD-04)
- Steps: 5s, 15s, 30s, 60s, 5min — matching the roadmap success criteria
- Cap at 5min (last step) for attempts beyond step count — same clamping pattern as racecontrol's EscalatingBackoff
- Reset to step 0 when Claude Code is confirmed running after a restart (recovery)
- Track: attempt count, last attempt timestamp
- Watchdog poll loop checks cooldown.ready() before attempting restart — skip poll cycle if not ready

### Self-Test After Restart (WD-05)
- After restart_success event, verify Claude Code is actually responding — not just PID alive
- Use the existing 3-second post-spawn check in watchdog.js as the first gate
- Add a secondary check: re-run detect() after a short delay to confirm process didn't immediately die
- If self-test fails (process died within verification window), do NOT reset cooldown — let it escalate
- If self-test passes, reset cooldown to step 0

### WebSocket Re-establishment (WD-06)
- watchdog-runner.js should create and own a CommsClient instance alongside the ClaudeWatchdog
- On restart_success event (and self-test pass), call commsClient.connect() if not already connected
- CommsClient already has auto-reconnect with exponential backoff — the watchdog just needs to hold a reference and start it
- HeartbeatSender should be wired to the CommsClient (start on 'open', stop on 'close') — same pattern as index.js

### Email Notification (WD-07)
- On successful restart (self-test passed), email bono@racingpoint.in: "James is back online"
- Use existing send_email.js via execFile (not exec — avoids shell injection)
- Email body includes: restart timestamp, attempt count, cooldown step, Claude Code exe path
- No rate limiting needed — email only on successful restart, not on every crash detection
- Fire-and-forget: don't block watchdog loop on email delivery. Log error if send fails.

### Claude's Discretion
- Exact class structure for EscalatingCooldown (standalone class or integrated into ClaudeWatchdog)
- Whether to add the cooldown/email as constructor options or hardcode
- Test structure (extend existing watchdog tests or create new test file)

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `ClaudeWatchdog` (watchdog.js): EventEmitter with DI for detect/kill/spawn/findExe — extend with cooldown logic
- `CommsClient` (comms-client.js): Full auto-reconnect WebSocket client — just instantiate and connect
- `HeartbeatSender` (heartbeat-sender.js): Wires into CommsClient start/stop lifecycle
- `watchdog-runner.js`: Standalone entry point — becomes the integration hub
- `system-metrics.js`: collectMetrics() for heartbeat payload
- `send_email.js` (project root or racingpoint dir): `node send_email.js <to> <subject> <body>`

### Established Patterns
- ESM modules with Object.freeze enums and private class fields (#field)
- node:test built-in test runner (zero external test deps)
- DI via constructor options (detectFn, killFn, spawnFn, collectFn)
- EventEmitter for lifecycle events (crash_detected, restart_success, restart_failed)
- 2-second delay between kill and spawn for OS handle cleanup
- 3-second post-spawn detection as immediate death check

### Integration Points
- watchdog-runner.js: Add CommsClient + HeartbeatSender alongside ClaudeWatchdog
- ClaudeWatchdog: Add cooldown state that gates #restart() calls
- watchdog.on('restart_success'): Wire self-test → WebSocket connect → email send
- Environment: COMMS_PSK and COMMS_URL needed for WebSocket (same as index.js)

</code_context>

<specifics>
## Specific Ideas

- Pattern directly from racecontrol's Phase 5: EscalatingBackoff state machine with steps array, clamping to last element, ready() check, record_attempt(), reset()
- racecontrol used 30s→2m→10m→30m steps; comms-link uses 5s→15s→30s→60s→5min (faster initial recovery since it's one local process, not remote pods)
- Email via same send_email.js that James already uses for Bono communication
- Old PowerShell watchdog should be retired after this phase confirms stability (noted in STATE.md decision from Phase 4)

</specifics>

<deferred>
## Deferred Ideas

- WhatsApp notification to Uday on crash/recovery — that's Phase 6 (Alerting)
- Daily health summary — Phase 8
- LOGBOOK sync after restart — Phase 7

</deferred>

---

*Phase: 05-watchdog-hardening*
*Context gathered: 2026-03-12*
*Source: Adapted from racecontrol Phase 5 Watchdog Hardening patterns*
