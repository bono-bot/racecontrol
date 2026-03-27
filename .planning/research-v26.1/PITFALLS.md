# Pitfalls Research

**Domain:** Event-driven mesh architecture added to existing hub-and-spoke sim racing venue system
**Researched:** 2026-03-27
**Confidence:** HIGH (critical pitfalls verified against multiple sources; Windows-specific items verified against official NATS docs and Microsoft Community)

---

## Critical Pitfalls

### Pitfall 1: NATS Server Windows Service Registration Conflict

**What goes wrong:**
When NATS server is launched as a child process inside an existing Windows service (such as rc-agent or rc-sentry), it attempts to register itself as a Windows service via `service_windows.go`. This causes an exit status 1 failure with an empty log file — the server silently fails to start without a clear error to the caller.

**Why it happens:**
NATS's Windows build includes logic that, when `Run(*Server)` is called, tries to register as a Windows service. A subprocess spawned from an existing service context conflicts with this logic. The project already has history with this failure pattern: rc-sentry restart using `cmd start + CREATE_NO_WINDOW` silently fails for the same category of reason (Windows service context + process isolation).

**How to avoid:**
Set the environment variable `NATS_DOCKERIZED=1` in the spawned NATS server process environment before launch. This bypasses service registration and calls `server.Start()` directly. Do NOT attempt to register NATS as a separate Windows service if launching from within rc-agent — use the environment variable approach or run NATS as a standalone service from the Windows Service Manager with an explicit log file path and correct directory permissions.

**Warning signs:**
- NATS server exits immediately after spawn with no log output
- `exit status 1` in the parent process log
- No NATS port (4222) listening after launch attempt

**Phase to address:**
Event Backbone phase (NATS + JetStream setup). Must be the very first integration test before any client code is written.

---

### Pitfall 2: NATS JetStream Store Directory — Windows Permissions and Path Format

**What goes wrong:**
JetStream defaults to storing data in `/tmp` — which maps to a temporary path that may be cleared on reboot or denied write access under certain Windows service accounts. Separately, if backslash paths (`C:\nats\data`) are used in the NATS config file, they may be misinterpreted depending on the config parser version, causing JetStream to silently fall back to in-memory mode and lose all durability on restart.

**Why it happens:**
NATS documentation examples universally use Unix-style forward slashes. The Windows service user (typically `SYSTEM` or `NetworkService`) often lacks write permission to arbitrary directories. When the JetStream write fails silently, the server continues in core NATS mode — no error is surfaced to clients, but stream persistence is gone.

**How to avoid:**
- Configure `store_dir` explicitly using forward slashes: `store_dir: "C:/nats/jetstream"`
- Create the directory before starting the server and set explicit ACL for the service account
- For the Windows service user, grant read on the config file and write on the store directory
- Verify JetStream is active post-start by checking the server info endpoint: `nats server info` should show `jetstream: true`
- Console logging is forbidden for `NetworkService` — always set an explicit log file

**Warning signs:**
- `nats server info` shows `jetstream: false` despite config enabling it
- Streams created appear to work but are empty after server restart
- Windows Event Log shows access denied for the NATS service account

**Phase to address:**
Event Backbone phase. Add a startup health check that verifies JetStream is active and the store directory is writable before any other component connects.

---

### Pitfall 3: mDNS Discovery Blocked by Windows Defender Firewall

**What goes wrong:**
Windows 11 has a built-in mDNS (UDP-In) firewall rule that is profile-aware. On pod machines, the network profile may be classified as "Public" rather than "Private" (common after reimaging or network changes), causing the mDNS inbound rule to be inactive. Pods appear to start up correctly but cannot discover each other — the discovery service returns empty peer lists with no error.

**Why it happens:**
The Windows Defender Firewall separates inbound mDNS (UDP 5353, multicast 224.0.0.251) rules by profile. "Public" profile disables mDNS by default as a security measure. Pod machines at the venue are on a LAN, but if Windows classifies the connection as "Public" (no Active Directory domain, no gateway responding to domain checks), the rule is off. The project already has documented history of silent Windows firewall interference.

**How to avoid:**
- Explicitly set the network profile on all pods to "Private" via PowerShell: `Set-NetConnectionProfile -InterfaceAlias "Ethernet" -NetworkCategory Private`
- Add an explicit inbound firewall rule (all profiles) for UDP 5353 multicast to the mDNS service executable
- Do not rely on the built-in Windows mDNS rule alone — add an application-specific rule for the Rust discovery binary
- Include firewall state in the fleet audit script (v23.0 audit framework already exists — extend it)

**Warning signs:**
- `nslookup pod1.local 224.0.0.251` returns no results from other pods
- mDNS service starts without error but peer count stays 0
- Firewall log shows blocked UDP 5353 packets

**Phase to address:**
Pod Mesh phase (mDNS/DNS-SD peer discovery). The audit script extension should be a prerequisite gate before the mesh goes live.

---

### Pitfall 4: mDNS Multicast Instability on Windows After Network Interface State Changes

**What goes wrong:**
There is a known Windows bug where multicast routing breaks after any WiFi or network interface disruption — including sleep/wake cycles, VPN connects, or Tailscale re-negotiation. The mDNS service continues running but stops sending or receiving multicast queries. The only reliable recovery is disabling and re-enabling the network interface, or rebooting. Since pods run Tailscale alongside the LAN adapter, Tailscale reconnections can silently kill mDNS multicast on the LAN interface.

**Why it happens:**
Windows multicast group membership is tied to interface state. When an interface transitions (VPN up/down, wake from sleep), the multicast socket's group membership is invalidated but the socket remains open. The Rust mDNS library does not automatically re-register with the multicast group after such a transition.

**How to avoid:**
- Implement an mDNS health watchdog in the pod mesh agent that periodically self-tests by sending a query and verifying at least one response (including from self)
- On detection of 0 peers for more than 60 seconds when the pod has been running for more than 5 minutes, trigger interface re-bind
- Do NOT restart the entire rc-agent — only rebind the mDNS socket
- Consider falling back to static IP peer table (192.168.31.x known IPs) when mDNS peer count drops to 0 unexpectedly

**Warning signs:**
- Peer count drops from N to 0 without pods going offline (health probes still passing)
- Windows Event Log shows interface state change events correlated with mDNS silence

**Phase to address:**
Pod Mesh phase. Must include a fallback static-IP discovery path as a degraded-mode option.

---

### Pitfall 5: Polling-to-Events Migration — Breaking Existing WebSocket Health Consumers

**What goes wrong:**
The current 30-second WebSocket health probe consumers (admin dashboard, control room, fleet grid) expect a specific JSON payload format and delivery timing. When migrating to event-driven delivery, the new system pushes events on state change rather than on a fixed schedule. Existing consumers that expected a heartbeat every 30 seconds interpret the absence of a packet as "server disconnected" and show stale or error state in the UI even when all pods are healthy.

**Why it happens:**
Polling consumers use the poll interval as an implicit heartbeat. Any gap longer than the expected interval triggers a "no data" error path. The migration introduces irregular delivery timing (event on change, not on schedule), violating the implicit contract existing consumers depend on.

**How to avoid:**
Use the dual-write / strangler fig approach: keep existing 30-second WebSocket health probes running for the duration of the migration. Add NATS event publishing as a parallel write — events flow to the mesh while the old HTTP/WS path continues serving existing consumers. Retire the polling path only after all consumers have been updated and verified. Never do a big-bang cutover.

**Warning signs:**
- Admin dashboard shows "connection lost" or stale pod data during migration
- Fleet grid flickers (already documented in project history as caused by timing changes)
- Control room health shows pods as offline while NATS confirms they are online

**Phase to address:**
Event Backbone phase (establish parallel write) AND the final migration phase (decommission polling). Each consumer must be explicitly updated and tested before the polling path is removed.

---

### Pitfall 6: Dual Write Atomicity — Database Update and NATS Event Not Atomic

**What goes wrong:**
When a pod state change updates the local SQLite database AND publishes a NATS event, these are two separate operations. If the SQLite write succeeds but NATS publish fails (NATS temporarily unreachable, network blip), the database is updated but no event is emitted. Downstream consumers never learn of the change. Conversely, if the event publishes but SQLite write fails, consumers receive a phantom state change.

**Why it happens:**
Distributed writes are inherently non-atomic. This is not a mistake in implementation — it is a fundamental property of adding a message bus alongside an existing database. The project already has documented SQLite on pods as the persistence layer, making this a concrete risk.

**How to avoid:**
Use the transactional outbox pattern: write the state change AND the pending event to SQLite in a single ACID transaction. A separate relay goroutine reads the outbox table and publishes to NATS, then marks the row as sent only after NATS confirms receipt. If NATS is down, events accumulate in the outbox and are replayed when connectivity restores. This is compatible with the existing SQLite spool design (v24.0 requirement for SQLite spooling already aligns with this).

**Warning signs:**
- NATS subscriber receives state X but pod's local database shows state Y
- After a NATS restart, downstream aggregate views show stale data for specific pods

**Phase to address:**
Event Backbone phase (define outbox pattern) AND Degraded Mesh phase (verify outbox replay on reconnect).

---

### Pitfall 7: Event Schema Evolution — No Version From Day One

**What goes wrong:**
Events published in Phase 1 of the mesh have no version field. By Phase 3, the schema has evolved. When the system replays historical events (after a server restart, new consumer catch-up, or aggregate rebuild), old events fail to deserialize because the new handler expects fields that did not exist in version 1. This breaks the fundamental replay guarantee of event sourcing.

**Why it happens:**
Schema versioning feels premature when writing the first event. It adds boilerplate to every event struct. Teams skip it, reasoning they can add it later. Adding it later requires a migration of all existing events in the JetStream stream — at which point there may be thousands of events and no clean path forward.

**How to avoid:**
Every event struct must include `version: u32` (or equivalent) from the first commit. Use Rust's `serde` with `#[serde(default)]` on new fields to maintain backward compatibility when deserializing old events. Event consumers must be version-aware — dispatch to a version-specific handler, not a single handler that assumes the latest schema. Start at version 1, not version 0 (to distinguish "no version field" from "version 1").

**Warning signs:**
- Deserialization panics or errors when replaying old events
- New subscriber fails to catch up from stream beginning
- Schema mismatch errors only appear during restart (works fine in steady state)

**Phase to address:**
Event Backbone phase. Version field must be in the event taxonomy definition before any events are published to production streams.

---

### Pitfall 8: JetStream Consumer Acknowledgment — Infinite Redelivery Loop

**What goes wrong:**
A JetStream consumer processes a message, performs a side effect (e.g., triggers a pod action), but the acknowledgment is delayed beyond `AckWait` (default behavior: immediate redeliver). The message is redelivered, the side effect is triggered again. Without `MaxDeliver` configured, the default is `-1` (redeliver forever). A single slow handler can trigger the same action dozens of times before anyone notices.

**Why it happens:**
NATS JetStream's at-least-once delivery guarantee means unacknowledged messages are retried. If handler logic takes longer than `AckWait` (e.g., due to a blocking pod operation over SSH), NATS assumes the message was lost and redelivers. The Rust `async-nats` client must explicitly acknowledge within the timeout window.

**How to avoid:**
- Set `MaxDeliver: 3` (or similar small number) on all consumers during initial development; increase only if justified
- Use `message.ack()` immediately after receipt and process asynchronously — acknowledge receipt, not completion
- For side-effecting consumers (AI agent command dispatch, pod actions), implement idempotency keys based on event ID before executing the action
- Use `message.term()` (negative terminal ack) for messages that can never succeed rather than letting them exhaust MaxDeliver
- Configure a dead-letter subject to capture terminated messages for alerting

**Warning signs:**
- Same pod action executes multiple times in rapid succession
- Consumer `NumRedelivered` metric climbs continuously
- Side effects (pod restarts, game launches) fire repeatedly for a single trigger event

**Phase to address:**
Event Backbone phase (consumer configuration standards) AND AI Agent Mesh phase (idempotency for agent-dispatched actions).

---

### Pitfall 9: AI Agent Runaway Loop — Cascading Reactive Triggers

**What goes wrong:**
James (on-site AI) observes a pod health event, publishes a remediation command. The command changes pod state. The pod state change publishes a new health event. James observes the new event and publishes another remediation command. The loop runs until budget or rate limits stop it. With 8 pods publishing health events, a single misconfigured agent handler can cascade across the entire fleet.

**Why it happens:**
Event-driven AI agents are reactive by design. Without explicit loop detection, an agent that "fixes" a problem can generate the event that triggers itself again. The project already has history of PowerShell orphan leaks from `relaunch_self()` — this is the same failure class at the orchestration layer.

**How to avoid:**
- Implement the blackboard pattern: the controller service (not the agent) executes actions. Agents propose intents to the blackboard; the controller deduplicates and rate-limits execution
- Add a per-pod, per-action cooldown: a given action (e.g., `restart_game`) cannot fire for the same pod more than once per 5 minutes
- Set `max_turns` or equivalent iteration limits on all agent reasoning loops
- Publish agent intents to a separate NATS subject that the controller subscribes to — agents never publish directly to the action subject
- Circuit breaker: if more than 3 actions are triggered for a single pod within 60 seconds, escalate to human (WhatsApp alert to Uday) rather than continuing automated remediation

**Warning signs:**
- NATS subscriber for action subjects receives burst of identical messages
- Pod enters restart loop with rc-sentry and rc-agent both logging rapid state changes
- WhatsApp alert and remediation action fire in tight alternation
- Agent reasoning log shows the same root cause analysis repeated verbatim multiple times

**Phase to address:**
AI Agent Mesh phase. The blackboard + intent-based command design (already in PROJECT.md Key Decisions) is the architectural prevention. Implement before connecting any AI agent to the event bus.

---

### Pitfall 10: Sensor Fusion False Positives from Clock Skew

**What goes wrong:**
Camera AI detection events arrive with the camera's system timestamp. Pod health events arrive with the pod's system timestamp. The fusion service joins them on a time window (e.g., "camera detects person in zone X within 2 seconds of pod Y going to billing state"). If the camera system clock and pod clock are skewed by more than the join window, all correlations fail silently — no error, just no fusion matches. Alternatively, stale camera events arrive late and correlate with wrong pod states, producing false positive occupancy readings.

**Why it happens:**
The 13 cameras use go2rtc via an NVR. NVR devices often have free-running clocks with significant drift. Pod clocks are set by Windows Time Service but may be minutes off from each other without NTP enforcement. Late fusion relies entirely on temporal alignment — if that alignment is wrong, the fusion output is worse than no fusion at all.

**How to avoid:**
- Enforce NTP synchronization on all pods and the server: verify all clocks are within 500ms of each other before enabling fusion
- Add a `source_timestamp` and `received_timestamp` to every event; use received_timestamp for initial fusion window, source_timestamp for audit
- Set the fusion time window conservatively wide (5 seconds minimum, not 500ms) to tolerate clock skew up to the NTP correction interval
- Implement a confidence degradation curve: correlations near the edge of the time window get lower confidence scores than those in the center
- Monitor cross-source clock skew as a metric; alert if any source drifts more than 1 second from server time

**Warning signs:**
- Fusion match rate is 0% or near-0% despite cameras and pods both generating events
- Fusion matches spike and drop in sync with NTP corrections
- Camera "person detected" events correlate with pod states from 3-5 minutes ago

**Phase to address:**
Camera Fusion phase. Clock skew measurement and NTP health check must be a prerequisite gate before fusion logic is written.

---

### Pitfall 11: Split Brain During Server Downtime — Conflicting Local State on Reconnection

**What goes wrong:**
The server at 192.168.31.23 goes offline (reboot, deploy, crash). Pods operate in degraded mode, spooling events locally to SQLite. Meanwhile, an admin uses the web dashboard (cloud VPS, Bono) to manually update a pod's configuration (e.g., change billing rate, update game list). When the server comes back, the pod has 47 spooled events representing local state changes. The server has 3 events from the admin dashboard representing different state changes. Neither side knows about the other's changes. Which version wins?

**Why it happens:**
This is the classic split-brain scenario: two partitions of the system independently accumulated writes during a network partition. Without a reconciliation protocol, the reconnecting merge is ambiguous. The existing project bug (pods table empty after server restart) demonstrates the server is already vulnerable to incomplete reconnection.

**How to avoid:**
- Define a clear conflict resolution policy before building degraded mode: for this venue, "server state wins for configuration, pod state wins for billing/session events" is a sensible rule
- Use vector clocks or sequence numbers (not wall-clock timestamps) to establish event ordering across the partition
- On reconnection, the pod sends its entire local spool with sequence numbers; the server applies only events it does not already have and rejects events that conflict with authoritative server state, logging conflicts for review
- Admin actions during server downtime should be clearly flagged in the UI as "pending — will sync on reconnect"
- Test reconnection explicitly with a chaos test: take down the server for 5 minutes, make changes on both sides, bring it back up, verify final state is deterministic

**Warning signs:**
- Pod state after reconnection differs from what was set via admin dashboard during downtime
- Billing session times are wrong after a server outage
- The existing "pods table empty after restart" bug recurs in a new form (different symptom, same root cause class)

**Phase to address:**
Degraded Mesh phase. Conflict resolution policy must be documented as an explicit decision in PROJECT.md before any code is written for this phase.

---

### Pitfall 12: Replay Storm on Reconnection — Consumer Overwhelm

**What goes wrong:**
After a 30-minute server outage, 8 pods each have a backlog of 100+ spooled events. When the server reconnects, all 8 pods attempt to replay their entire spool simultaneously. The fusion service, predictive layer, and admin dashboard all receive 800+ events in rapid succession. The admin dashboard WebSocket overwhelms the browser; the fusion service can't keep up and drops late-arriving events; the predictive layer builds incorrect models from out-of-order data.

**Why it happens:**
SQLite spool replay has no built-in backpressure. All accumulated events are sent as fast as possible after connectivity restores. The receiving side (NATS JetStream stream) may have its own redelivery backlog on top. Naive implementation produces O(pods × spool_depth) event burst on reconnect.

**How to avoid:**
- Implement replay rate limiting: each pod replays at most N events per second (e.g., 10/s), not as fast as possible
- Use JetStream's `StartTime` consumer option to allow consumers to request events only from a specific timestamp, not from the beginning of the stream
- Prioritize Tier 0 events (billing session completions, pod state) over Tier 1 (management) over Tier 2 (analytics) during catchup
- The fusion service must explicitly handle out-of-order events: use a sorted merge queue keyed by event timestamp rather than processing in arrival order

**Warning signs:**
- Admin dashboard becomes unresponsive after server reconnect
- JetStream consumer lag metric spikes immediately after reconnection events
- Fusion service drops events logged in its error output during reconnect window

**Phase to address:**
Degraded Mesh phase (replay rate limiting) AND Event Backbone phase (consumer priority configuration must be defined upfront).

---

### Pitfall 13: Predictive Analytics Cold Start and Overfitting on Small Dataset

**What goes wrong:**
The venue has 8 pods. Even with 6 months of operational data, the dataset for predictive maintenance is small by ML standards. A neural network or LSTM trained on this data overfits to venue-specific patterns (e.g., "Pod 3 always reboots on Tuesday at 18:00 because of the tournament") and produces useless predictions for any deviation from historical patterns. Worse, on initial deployment (cold start), there is no data at all — the model predicts everything as "healthy" until enough history accumulates.

**Why it happens:**
ML practitioners underestimate how small 8 pods × 6 months actually is for time-series prediction. Complex models need thousands of samples with diverse conditions. The venue's operational patterns are highly regular (daily opening/closing cycles, weekly tournaments), making models look accurate in backtesting while being brittle to novel failures.

**How to avoid:**
- Use rule-based and statistical methods (not neural networks) for the first version: threshold anomaly detection, rolling averages, Z-score on metrics — these work with 0 training data and have interpretable failure modes
- Implement explicit cold start handling: for the first 30 days, output "INSUFFICIENT DATA — monitoring only" rather than predictions
- Separate the pod failure predictor (sparse, high-stakes events) from demand forecasting (dense, recoverable events) — they need different approaches
- Tree-based models (gradient boosted) outperform LSTM on small tabular datasets and are more resistant to overfitting
- All predictions must include a confidence score; suppress alerts when confidence is below 0.7

**Warning signs:**
- Model accuracy looks high in backtesting but predictions never fire in production
- All pods predicted as "healthy" for 30 consecutive days (the model learned to always say healthy)
- Pod failure prediction fires for a pod that is operating normally

**Phase to address:**
Predictive Layer phase. The phase should explicitly start with rule-based baselines and introduce ML only after 60 days of event data are available.

---

## Technical Debt Patterns

Shortcuts that seem reasonable but create long-term problems.

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Skip event versioning for "simple" events | Less boilerplate in first phase | Replay breaks when schema changes; full stream migration required | Never — add `version: 1` from day one |
| Hard-code pod IP addresses instead of mDNS | Simpler early code | Breaks every time a pod NIC is replaced or IP changes; mDNS is the escape hatch | Only in unit tests, never in production code |
| Use wall-clock timestamps for event ordering | Intuitive, easy to implement | Clock skew causes incorrect merge order after reconnection; use monotonic sequence IDs instead | Acceptable for display/logging; never for ordering |
| Let AI agents publish directly to action subjects | Simpler architecture | Enables runaway loops; no deduplication or rate limiting | Never — always route through controller/blackboard |
| Disable antivirus exclusions "temporarily" | Faster JetStream I/O | Real-time AV scanning of JetStream store directory on Windows causes severe write amplification | Acceptable to exclude the store directory permanently; not acceptable to disable AV entirely |
| Single global NATS consumer for all event types | Less consumer management | All event types blocked by one slow handler; use per-type consumers | Never for production; acceptable in proof-of-concept |

---

## Integration Gotchas

Common mistakes when connecting to external services.

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| NATS server on Windows | Run as child of existing service without `NATS_DOCKERIZED=1` | Set env var or run NATS as standalone Windows Service; never embed inside rc-agent |
| NATS JetStream storage | Use default `/tmp` store_dir | Explicitly configure `store_dir` with forward-slash Windows path and verify write permission |
| mDNS on Windows 11 | Rely on built-in Windows mDNS rule in all firewall profiles | Add explicit application-level UDP 5353 inbound rule for all profiles; set network profile to Private |
| async-nats Rust client | Use deprecated synchronous `nats` crate | Use `async-nats` (Tokio-based); the synchronous crate is no longer maintained |
| JetStream consumer ACK | Ack after processing completes | Ack on receipt; process asynchronously to avoid redelivery on slow handlers |
| Camera NVR timestamps | Trust NVR source_timestamp for fusion joins | Add received_timestamp; use conservative join window; verify NTP sync first |
| SQLite spool replay | Replay all events as fast as possible on reconnect | Rate-limit replay; prioritize Tier 0 events; use JetStream StartTime consumer |
| Tailscale + mDNS | Assume Tailscale doesn't affect LAN mDNS | Tailscale tunnel reconnections can corrupt Windows multicast group membership on the LAN interface |

---

## Performance Traps

Patterns that work at small scale but fail as usage grows.

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| One JetStream consumer per event type per pod | Works fine for 8 pods | 8 pods × 20 event types = 160 consumers; NATS has consumer limits | At consumer count > 100 (NATS default limit) |
| No JetStream stream retention limits | Storage fills slowly; not noticed | Set `MaxAge` (e.g., 7 days) and `MaxBytes` on all streams from day one | When JetStream store fills the drive; no warning until disk full |
| Fusion join window stored in memory | Fast, simple | On replay/restart, all in-flight joins are lost; produces missed correlations | After every server restart |
| Predictive model rebuilt on every pod health event | Acceptable for 1 pod | CPU spikes every 30s; prediction latency grows; model thrashes | When model training time exceeds pod health event interval |
| mDNS peer list queried by polling from HTTP handlers | Works for 8 pods | Synchronous mDNS query in HTTP handler path blocks request if mDNS is slow | When mDNS takes >500ms (Windows multicast instability) |

---

## Security Mistakes

Domain-specific security issues beyond general web security.

| Mistake | Risk | Prevention |
|---------|------|------------|
| NATS server with no authentication on LAN | Any pod can publish to any subject; rogue process on compromised pod can inject false health events | Enable NATS user/password or NKey authentication even on LAN; the project already uses PSK pattern in comms-link |
| AI agent accepts commands from any NATS subject publisher | Attacker who gains pod access can publish admin commands | Validate command source; require signed command envelopes; agents only consume from authenticated subjects |
| JetStream store_dir accessible to all users | Any process on Windows can read/write billing event history | Set NTFS ACL on store directory to service account only; JetStream streams contain billing session data |
| mDNS leaks pod hostnames to network | Exposes fleet topology on any network the pods connect to | This is acceptable on venue LAN; document that pods should never connect to untrusted networks while mesh is active |
| Event sourcing log contains customer PII | Replay of billing events exposes customer names/IDs indefinitely | Encrypt PII fields in events; use customer ID (not name) in events; store PII only in database with retention policy |

---

## UX Pitfalls

Common user experience mistakes in this domain.

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| Admin dashboard shows event-driven "real-time" data but polling fallback shows stale data | Operators see inconsistent pod states on different screens | Clearly label data freshness; show "last updated" timestamp on all fleet tiles |
| AI agent takes autonomous action without notification | Uday sees pod restart with no explanation; creates distrust | All autonomous actions must appear in the activity feed before execution, not after |
| Degraded mode is invisible to staff | Staff continues selling pod time not knowing pod mesh is broken | Show a persistent "DEGRADED MODE - [X] minutes" banner in the control room when mesh connectivity is impaired |
| Predictive alert fires with no context | Staff sees "Pod 3 failure risk: HIGH" with no explanation | Every prediction must include the top 2 contributing factors in plain English |
| Split brain conflict shown as error | Staff escalates as a critical failure when it is a normal reconnection event | Frame as "syncing [N] pending changes" with a progress indicator, not an error state |

---

## "Looks Done But Isn't" Checklist

Things that appear complete but are missing critical pieces.

- [ ] **NATS event backbone:** Often appears working in happy-path testing — verify JetStream persistence survives a server restart before declaring phase done
- [ ] **mDNS peer discovery:** Often shows correct peer list in isolation — verify peer list updates correctly when a pod goes offline mid-session (not just at startup)
- [ ] **Dual write / outbox:** Often passes unit tests — verify behavior specifically when NATS is down during a pod state change (outbox must accumulate, not drop)
- [ ] **Degraded mode:** Often tested by gracefully stopping the server — verify behavior specifically when server disappears mid-transaction (hard kill, not graceful shutdown)
- [ ] **Reconnection replay:** Often only tested with small spool — verify with 500+ events spooled to confirm rate limiting and backpressure work
- [ ] **AI agent idempotency:** Often works in testing because test runs are isolated — verify the same event delivered twice produces exactly one action (not two)
- [ ] **Schema evolution:** Often only tested with current-version events — verify a consumer can replay events from the very first stream sequence after schema changes
- [ ] **Sensor fusion:** Often tested with synchronized test data — verify fusion handles events that arrive 10+ seconds late (simulate clock skew explicitly)
- [ ] **Antivirus exclusion:** JetStream store directory must be excluded from Windows Defender real-time scanning on server — verify I/O performance with exclusion confirmed active

---

## Recovery Strategies

When pitfalls occur despite prevention, how to recover.

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| NATS JetStream data lost (wrong store_dir) | HIGH | Stop NATS; fix store_dir; all historical events are gone; rebuild projections from existing PostgreSQL/SQLite state; accept gap in event history |
| mDNS multicast broken after network event | LOW | Disable and re-enable the LAN network interface on affected pod via PowerShell remote exec; do not reboot (disrupts customer session) |
| AI agent runaway loop | MEDIUM | Stop the agent immediately via relay exec; manually cancel pending actions in the blackboard; review and fix the trigger condition before restarting; check and cancel any duplicate side effects on affected pods |
| Split brain conflict after reconnection | MEDIUM | Review conflict log; apply the documented resolution policy (server wins for config, pod wins for billing); trigger a manual audit report via the v23.0 audit system |
| Replay storm overwhelms dashboard | LOW | Disconnect affected consumers temporarily; let NATS consumer lag drain; reconnect consumers; add rate limiting before next replay |
| Event schema deserialization failure | HIGH | Cannot replay affected events without a migration; write an upcaster that handles old event format; re-process the stream with the upcaster before deploying schema-breaking changes |
| Predictive model producing all-healthy false predictions | LOW | Disable ML predictions; revert to rule-based thresholds; accumulate 60 more days of data before re-enabling |

---

## Pitfall-to-Phase Mapping

How roadmap phases should address these pitfalls.

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| NATS Windows service registration conflict | Event Backbone | Integration test: launch NATS from within a Windows service context, verify it starts successfully |
| JetStream store_dir permissions and path | Event Backbone | Post-deploy check: `nats server info` shows `jetstream: true`; restart server, verify stream contents persist |
| mDNS blocked by Windows Firewall | Pod Mesh | Audit script extension: verify UDP 5353 inbound rule active on all pods; verify network profile is Private |
| mDNS multicast instability after interface changes | Pod Mesh | Chaos test: disconnect and reconnect Tailscale on a pod; verify mDNS peer list recovers within 90 seconds |
| Polling consumers broken during migration | Event Backbone | Parallel write test: verify existing health dashboard continues working while NATS events also flow |
| Dual write atomicity (outbox pattern) | Event Backbone | Fault injection: kill NATS mid-write; verify event appears in outbox; restore NATS; verify event is delivered |
| Event schema evolution breaking replay | Event Backbone | Day-one: every event struct has `version` field; replay test from sequence 1 after any schema change |
| JetStream infinite redelivery loop | Event Backbone | Consumer configuration audit: all consumers have `MaxDeliver` set; integration test confirms idempotency |
| AI agent runaway loop | AI Agent Mesh | Rate limit test: inject same event 10 times; verify only 1 action fires; verify cooldown enforced |
| Sensor fusion clock skew false positives | Camera Fusion | Prerequisite gate: NTP sync verified across all sources; fusion tested with synthetic 5-second clock skew |
| Split brain on reconnection | Degraded Mesh | Chaos test: documented conflict resolution policy applied correctly when both sides had writes |
| Replay storm on reconnection | Degraded Mesh | Load test: 8 pods × 100 events spooled; reconnect; verify replay takes >60 seconds (rate limited), not <5 seconds (unbounded) |
| Predictive cold start and overfitting | Predictive Layer | Cold start gate: model outputs "INSUFFICIENT DATA" for first 30 days; no predictions fire on insufficient data |

---

## Sources

- NATS Windows Service docs: https://docs.nats.io/running-a-nats-service/introduction/windows_srv
- NATS Server Windows service conflict issue: https://github.com/nats-io/nats-server/issues/1113
- JetStream consumer acknowledgment docs: https://docs.nats.io/nats-concepts/jetstream/consumers
- JetStream infinite redelivery issue: https://github.com/nats-io/nats-server/issues/4627
- async-nats Rust client: https://docs.rs/async-nats/latest/async_nats/
- NATS JetStream anti-patterns: https://www.synadia.com/blog/jetstream-design-patterns-for-scale
- Jepsen NATS 2.12.1 analysis (data loss under corruption): https://jepsen.io/analyses/nats-2.12.1
- mDNS on Windows 11 issues: https://community.start9.com/t/solved-mdns-on-windows-11-partially-works/1859
- mDNS in the Enterprise (Microsoft): https://techcommunity.microsoft.com/t5/networking-blog/mdns-in-the-enterprise/ba-p/3275777
- mDNS firewall configuration: https://learn.microsoft.com/en-us/answers/questions/101168/mdns-not-sending-queries-to-the-network
- Event-driven architecture 5 pitfalls (Wix): https://medium.com/wix-engineering/event-driven-architecture-5-pitfalls-to-avoid-b3ebf885bdb1
- Dual write problem (Confluent): https://www.confluent.io/blog/dual-write-problem/
- Transactional outbox pattern (AWS): https://docs.aws.amazon.com/prescriptive-guidance/latest/cloud-design-patterns/transactional-outbox.html
- Event sourcing schema evolution pitfalls: https://www.youngju.dev/blog/architecture/2026-03-07-architecture-event-sourcing-cqrs-production-patterns.en
- Event sourcing mistakes (Hacker News community): https://news.ycombinator.com/item?id=20324021
- AI agent infinite loop prevention: https://www.fixbrokenaiapps.com/blog/ai-agents-infinite-loops
- Multi-agent coordination strategies: https://galileo.ai/blog/multi-agent-coordination-strategies
- Sensor fusion time synchronization analysis: https://arxiv.org/abs/2209.01136
- Split brain distributed systems: https://dzone.com/articles/split-brain-in-distributed-systems
- Predictive analytics small datasets: https://www.sciencedirect.com/science/article/pii/S2772662225000293
- Strangler fig migration pattern: https://docs.aws.amazon.com/prescriptive-guidance/latest/cloud-design-patterns/strangler-fig.html
- Project-specific known issues: racecontrol `DEBUG-BLANKING-SCREEN.md`, `project_rcsentry_restart_bug.md`, `feedback_ssh_config_corruption.md`, `project_pod_healer_flicker.md`

---
*Pitfalls research for: Event-driven mesh architecture — Racing Point eSports v24.0*
*Researched: 2026-03-27*
