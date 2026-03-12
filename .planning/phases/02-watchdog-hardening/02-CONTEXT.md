# Phase 2: Watchdog Hardening - Context

**Gathered:** 2026-03-13
**Status:** Ready for planning

<domain>
## Phase Boundary

pod_monitor uses escalating backoff (30s->2m->10m->30m) per pod with exclusive restart ownership; post-restart verification confirms process + WebSocket + lock screen via spawned async tasks; pod_healer reads shared state and defers all restarts by flagging AppState; email alerts fire when verification fails or max backoff is reached. Kiosk dashboard reflects detailed watchdog states.

</domain>

<decisions>
## Implementation Decisions

### Post-restart verification
- rc-agent exposes a /health endpoint on a local port; rc-core verifies lock screen responsiveness by hitting it via pod-agent /exec curl
- Verification polling uses escalating schedule: 5s, 15s, 30s, 60s after restart command sent (total 60s window)
- All 3 checks must pass: process alive + WebSocket connected + lock screen health endpoint responsive. If any check fails at 60s, declare failure
- Partial recovery (process + WS but no lock screen) is treated as FAILED — alert fires, backoff escalates. Customers can't use a pod without the lock screen
- Verification runs as a spawned async task (tokio::spawn) — pod_monitor continues checking other pods without blocking

### Kiosk status display
- Detailed states shown to staff: Online, Offline, Restarting (attempt N/4), Verifying Recovery, Recovery Failed
- New DashboardEvent types: PodRestarting, PodVerifying, PodRecoveryFailed — delivered via existing WebSocket protocol
- Show the backoff step ("Backoff: 2m") but not a live countdown timer — less visual noise
- Current state only — no restart history in the dashboard UI. Staff checks activity log for patterns

### Alert email content
- Actionable summary format, 10 lines max. Subject: "[RaceControl] Pod N — Recovery Failed" or "Pod N — Max Escalation Reached"
- Body includes: pod name, failure type (no WS / no lock screen / process dead), current backoff step, last heartbeat time, next action suggestion
- Recipient: Uday only (usingh@racingpoint.in) — James is on-site and sees the kiosk
- Alert fires on two triggers: (1) post-restart verification failure, (2) max backoff escalation reached (30m step)
- Uses existing Node.js send_email.js script via Command::new("node") — same Gmail OAuth path already implemented in EmailAlerter

### Healer vs Monitor boundaries
- pod_healer sets a `needs_restart: true` flag per pod in AppState when it detects restart-worthy issues. pod_monitor checks this flag on its next cycle and issues the restart
- pod_healer skips its entire diagnostic cycle for pods in Restarting or Verifying state — no conflicting actions during recovery
- WebSocket liveness uses belt-and-suspenders: heartbeat timestamp timeout as primary detection, channel send-ping-and-check-error as secondary confirmation before declaring dead
- Full backoff reset on recovery: attempt counter goes to 0, next failure starts at 30s. Clean slate — recovered pod is healthy until proven otherwise

### Claude's Discretion
- Exact /health endpoint response format on rc-agent side
- How to structure the `needs_restart` flag in AppState (bool, enum, or timestamp)
- DashboardEvent payload structure for new watchdog event types
- Email body template formatting
- How to handle verification task cleanup if pod_monitor cycle runs while verification is still pending

</decisions>

<specifics>
## Specific Ideas

- User wants billing timer paused during pod downtime with a grace period on recovery — captured as deferred idea (new capability, not watchdog scope)
- Existing EmailAlerter already has rate limits (30min/pod, 5min/venue) — this phase wires it into the actual watchdog flow
- STATE.md blocker: agent_senders channel liveness needs send-ping-and-check-error pattern — addressed in WS liveness decision

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- `EscalatingBackoff` (rc-common/src/watchdog.rs): `attempt()` advances step, `ready()` checks cooldown, `reset()` clears state. 5 tests passing.
- `EmailAlerter` (rc-core/src/email_alerts.rs): `send_alert()` with dual rate limits. Uses Node.js send_email.js. Ready to wire.
- `AppState.pod_backoffs` (rc-core/src/state.rs): Pre-populated for pod_1 through pod_8 (Phase 1). `RwLock<HashMap<String, EscalatingBackoff>>`.
- `DashboardEvent` (rc-common/src/protocol.rs): Existing WebSocket event enum — needs new variants for watchdog states.
- `activity_log::log_pod_activity()`: Already used by both pod_monitor and pod_healer for logging events.

### Established Patterns
- pod_monitor: spawned via `pub fn spawn(state: Arc<AppState>)`, loops on interval, checks all pods each cycle
- pod_healer: same spawn pattern, 2-minute cycle, uses pod-agent /exec for diagnostics
- Pod-agent /exec: Now returns honest HTTP status codes (500 for failure) — Phase 1 fix
- pod_backoffs keyed as "pod_{N}" (underscore not dash)

### Integration Points
- pod_monitor.rs: Add backoff usage, verification spawn, email alert calls
- pod_healer.rs: Add needs_restart flag setting, skip-during-verification logic
- state.rs: Add needs_restart field or verification-state tracking to AppState
- rc-agent main.rs or lock_screen.rs: Add /health endpoint for lock screen status
- protocol.rs: Add PodRestarting, PodVerifying, PodRecoveryFailed to DashboardEvent

</code_context>

<deferred>
## Deferred Ideas

- Billing timer pause during pod downtime with grace period on recovery — new capability, belongs in billing/session management phase
- Restart history UI in kiosk dashboard — future observability improvement
- Badge count for problem pods — nice-to-have for future dashboard enhancement

</deferred>

---

*Phase: 02-watchdog-hardening*
*Context gathered: 2026-03-13*
