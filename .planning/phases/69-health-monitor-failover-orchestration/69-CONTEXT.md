# Phase 69: Health Monitor & Failover Orchestration - Context

**Gathered:** 2026-03-21
**Status:** Ready for planning

<domain>
## Phase Boundary

James (.27) runs a health probe loop against server .23, uses a hysteresis state machine to confirm sustained outage (not transient CPU spike), coordinates with Bono via comms-link to activate cloud racecontrol, broadcasts SwitchController to all pods, and notifies Uday via email + WhatsApp. Pods individually verify .23 is unreachable before honoring the switch (split-brain guard). Failback is Phase 70 — this phase handles outage detection + failover only.

</domain>

<decisions>
## Implementation Decisions

### Health Probe Loop (HLTH-01, HLTH-03)
- Runs on James's machine (.27) as a new module in comms-link (NOT in racecontrol on .23 — can't monitor yourself)
- Probes server .23 via HTTP GET to `http://192.168.31.23:8080/api/v1/health` every 5s
- Also probes rc-agent at `http://192.168.31.23:8090/ping` as secondary check
- Probe timeout: 3s per request — if no response in 3s, that probe counts as a failure
- Both probes must fail for a cycle to count as "down" — HTTP-only failure (e.g., racecontrol crashed but server OS is fine) is still a "down" for failover purposes since pods can't connect
- Use Tailscale IP (100.71.226.83) as fallback probe path if LAN probe fails — if Tailscale also fails, it's a real outage

### Hysteresis State Machine (HLTH-02, HLTH-03)
- Reuse the cloud_sync.rs pattern: consecutive failure/success counters with thresholds
- DOWN threshold: 12 consecutive failures at 5s interval = 60s sustained outage (matches SC-2 requirement)
- UP threshold: 2 consecutive successes = confirmed recovery (for failback in Phase 70)
- States: `Healthy` → `Degraded` (1-11 failures) → `Down` (12+ failures, triggers failover)
- A single successful probe resets the failure counter to 0 (conservative — avoids false failover)
- Log every state transition with timestamp for audit trail

### Failover Orchestration Sequence (ORCH-01, ORCH-02)
- When state machine hits `Down`:
  1. James sends `task_request` to Bono via comms-link: `{ task: "activate_failover", reason: "server .23 unreachable for 60s" }`
  2. Bono receives task_request, runs `activate_failover` command (pm2 start racecontrol on VPS)
  3. James waits for `task_response` from Bono confirming cloud racecontrol is running
  4. James sends HTTP POST to cloud racecontrol's SwitchController broadcast endpoint (new endpoint on Bono's racecontrol)
  5. Cloud racecontrol broadcasts `SwitchController { target_url: "ws://100.70.177.44:8080/ws/agent" }` to all connected pods
  6. If step 4 fails (cloud racecontrol not ready), retry 3 times with 5s interval
- Alternative: James can also use the new `/relay/exec/send` endpoint (Phase 66-04) to send `activate_failover` exec_request

### Pod-Side Split-Brain Guard (ORCH-03)
- When rc-agent receives SwitchController, before switching it does a quick LAN probe: HTTP GET `http://192.168.31.23:8090/ping` with 2s timeout
- If .23 responds: pod REJECTS the SwitchController and logs "split-brain guard: .23 still reachable, ignoring switch"
- If .23 does not respond: pod ACCEPTS the SwitchController and proceeds with the URL switch
- This is per-pod individual decision — no quorum. Each pod independently verifies .23 is down from its own perspective
- The guard runs in rc-agent's existing SwitchController handler (Phase 68) — add the probe before the URL write

### Bono as Secondary Watchdog (HLTH-04)
- Bono monitors comms-link heartbeat from James — if James's heartbeat stops for 5 minutes, Bono independently checks server .23
- Bono probes server .23 via Tailscale: `http://100.71.226.83:8090/ping`
- If both James heartbeat AND server .23 are down, Bono auto-activates cloud racecontrol as primary
- This handles the case where James's machine (.27) is also offline (power outage at venue)
- Implementation: add heartbeat-gap detection in Bono's comms-link `bono/index.js`

### Notifications (ORCH-04)
- Email: use existing `send_email.js` shell-out pattern (CLAUDE.md)
- WhatsApp: use existing Evolution API via `whatsapp_alerter.rs` pattern or `sendEvolutionText` in comms-link
- Content: "FAILOVER ACTIVATED — Server .23 unreachable. Pods switched to cloud (100.70.177.44). Time: {IST timestamp}. Pods connected: {count}/8."
- Send notification AFTER pods have switched (not before) — so the count is accurate
- Rate limit: max 1 failover notification per 10 minutes (prevent spam if server flaps)

### Claude's Discretion
- Whether health probe runs as a standalone Node.js module or integrated into james/index.js
- Exact hysteresis counter implementation (class vs plain object)
- Whether to use the existing racecontrol SwitchController broadcast or build a new endpoint on Bono's VPS
- Retry strategy for failed SwitchController broadcasts
- WhatsApp message formatting (plain text vs template)
- Bono watchdog implementation details (timer-based vs heartbeat-gap detection)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Health Monitoring
- `crates/racecontrol/src/cloud_sync.rs` lines 26-27, 145-147 — RELAY_DOWN_THRESHOLD/UP_THRESHOLD hysteresis pattern to reuse
- `crates/racecontrol/src/email_alerts.rs` — existing email notification pattern
- `crates/racecontrol/src/whatsapp_alerter.rs` — existing WhatsApp alerting

### Failover Mechanics
- `crates/rc-agent/src/main.rs` lines 2742-2768 — SwitchController handler with URL allowlist + last_switch_ms (Phase 68)
- `crates/rc-common/src/protocol.rs` line 403 — SwitchController variant definition
- `crates/rc-agent/src/self_monitor.rs` lines 84-96 — last_switch_ms guard (Phase 68)

### Comms-Link Coordination
- `C:/Users/bono/racingpoint/comms-link/shared/protocol.js` — task_request, task_response, exec_request message types
- `C:/Users/bono/racingpoint/comms-link/shared/exec-protocol.js` — activate_failover, deactivate_failover commands (Phase 66)
- `C:/Users/bono/racingpoint/comms-link/james/index.js` — relay server, exec/send endpoint (Phase 66-04)
- `C:/Users/bono/racingpoint/comms-link/bono/index.js` — ExecHandler, heartbeat monitor

### Research
- `.planning/research/ARCHITECTURE.md` — Failover architecture, build order
- `.planning/research/PITFALLS.md` — Split-brain prevention, false positive risks, AC launch CPU spikes
- `.planning/research/FEATURES.md` — Hysteresis pattern, failover trigger mechanics, 60s outage window

### Phase 66 Discovery
- Server Tailscale IP: 100.71.226.83
- Bono VPS Tailscale IP: 100.70.177.44
- Server LAN: 192.168.31.23, rc-agent :8090, racecontrol :8080

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `cloud_sync.rs` hysteresis pattern (3-down/2-up) — adapt thresholds to 12-down/2-up for 60s window
- `email_alerts.rs` — `send_email()` function for Uday notification
- `whatsapp_alerter.rs` — `send_whatsapp()` or Evolution API pattern
- `comms-link exec_request` — can trigger `activate_failover` on Bono VPS
- `comms-link task_request` — structured coordination with Bono

### Established Patterns
- Health probe: HTTP GET with timeout, similar to fleet health polling in racecontrol
- Notification rate limiting: email_alerts.rs already has rate limiting logic
- SwitchController broadcast: cloud_sync.rs already references pod WebSocket connections

### Integration Points
- James comms-link → Bono comms-link: task_request/exec_request for failover activation
- Bono racecontrol → pods: SwitchController broadcast via WebSocket
- rc-agent SwitchController handler: add LAN probe guard before URL switch (Phase 68 code)
- James health monitor → notification system: email + WhatsApp on state transition

</code_context>

<specifics>
## Specific Ideas

- The health monitor is on James (.27), NOT on the server — a process cannot reliably detect its own death
- 60s outage window (12 x 5s probes) matches the ROADMAP success criterion exactly
- Split-brain guard is per-pod, not quorum — simpler, each pod decides independently
- Bono secondary watchdog handles the "James is also down" edge case (venue power outage)
- Notifications go to Uday only AFTER pods have switched — so the pod count in the message is accurate

</specifics>

<deferred>
## Deferred Ideas

- Failback (server recovery detection + switch back to .23) — Phase 70
- Session data reconciliation after failover — Phase 70
- Grafana dashboard for failover events — Future requirement MON-01
- Automatic config sync on failover — already handled by Phase 67

</deferred>

---

*Phase: 69-health-monitor-failover-orchestration*
*Context gathered: 2026-03-21*
