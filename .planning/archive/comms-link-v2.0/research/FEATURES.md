# Feature Landscape: Comms Link v2.0

**Domain:** AI-to-AI Communication Link -- Reliable Messaging, Process Supervision, Observability
**Researched:** 2026-03-20
**Confidence:** HIGH (well-understood patterns from messaging, process supervision, and observability ecosystems, applied to a constrained 2-node system)

## Context

v1.0 is shipped and running. It provides WebSocket transport with PSK auth, heartbeat monitoring, Claude Code watchdog with escalating cooldown, WhatsApp/email alerting, LOGBOOK.md sync, and one-way INBOX.md task routing. v2.0 adds reliability guarantees, bidirectional communication, remote execution, deeper supervision, and observability. This research covers ONLY the new v2.0 features.

## Table Stakes

Features v2.0 must have to be considered a meaningful upgrade over v1.0. Without these, the milestone doesn't justify its existence.

| Feature | Why Expected | Complexity | Dependencies on v1.0 |
|---------|--------------|------------|----------------------|
| **Message ACK with sequence numbers** | v1.0 fires messages into the void -- no confirmation of delivery. For task routing and command execution, "did it arrive?" is non-negotiable. Sequence numbers detect gaps; ACKs confirm receipt. | MEDIUM | Extends existing `protocol.js` message envelope (add `seq`, `ack_seq` fields). Requires both sides to track sequence state. |
| **Transactional message queue (file-backed)** | v1.0's in-memory queue loses messages on crash. v1.0's INBOX.md has file races with git. A file-backed queue with atomic writes gives crash-resilient at-least-once delivery without an external broker. | MEDIUM | Replaces `INBOX.md` append pattern. Uses existing WebSocket transport for delivery. Needs a simple write-ahead log on disk (JSON lines file, not SQLite). |
| **Retry with timeout for unACKed messages** | ACK without retry is useless -- if the ACK never comes, the sender must redeliver. Timeout + bounded retries (3x with exponential backoff) prevent silent message loss. | LOW | Requires ACK tracking (above). Timer per outstanding message. |
| **Bidirectional task routing** | v1.0 INBOX.md is James-to-Bono only. Bono needs to send structured requests to James (and vice versa) with correlation IDs linking request to response. | MEDIUM | Extends existing `task_request`/`task_response` message types in protocol.js. Correlation via `id` + `reply_to` fields. |
| **Watchdog-of-watchdog (Task Scheduler)** | v1.0 watchdog dies silently if killed or crashes mid-session. The 15-hour blind outage (Mar 17-18) proved this. A Windows Task Scheduler periodic check (every 5min) that verifies the watchdog PID is alive and restarts it if not. | LOW | Registers a scheduled task via `schtasks.exe`. Checks for watchdog PID file or process name. Independent of the watchdog itself. |
| **Health snapshots in heartbeats** | Heartbeats currently carry basic system metrics. Adding pod status, deployment state, and comms-link queue depth gives Bono a complete picture of James's world without separate queries. | LOW | Extends existing `heartbeat` message payload. Reads from racecontrol API (localhost:8080) and local queue state. |

## Differentiators

Features that make v2.0 genuinely powerful beyond "basic reliability." Not blocking for launch but high value.

| Feature | Value Proposition | Complexity | Dependencies |
|---------|-------------------|------------|--------------|
| **Remote command execution with approval** | Bono sends "run X on James's machine" (or vice versa). Instead of Uday manually SSH-ing or using the web terminal, either AI can request the other to execute a command. The approval flow prevents runaway execution. | HIGH | Requires task routing (above) for request/response. Needs an allowlist of safe commands, a "pending approval" state, and a timeout for unapproved requests. Human approval via WhatsApp callback or auto-approve for allowlisted commands. |
| **Metrics export (structured counters)** | Uptime, reconnect count, message latency p50/p99, queue depth, ACK success rate -- exported as JSON over WebSocket for Bono to consume. Not Prometheus/OTEL (overkill for 2 nodes), just structured data Bono can log and chart. | LOW | Accumulate counters in-process. Send as part of heartbeat or on-demand via `status_query`. No external dependencies. |
| **Idempotency keys on messages** | Retry can cause duplicate processing. Messages already have UUID `id` fields (v1.0). Adding a seen-message cache (last 1000 IDs, 10min TTL) on the receiver deduplicates retried messages. | LOW | Uses existing `id` field from protocol.js. In-memory Set with TTL eviction. |
| **Graceful degradation modes** | When WebSocket is down, automatically fall back to email for critical messages (commands, alerts). When email is also down, buffer to disk queue. Explicit mode: REALTIME > EMAIL_FALLBACK > OFFLINE_QUEUE. | MEDIUM | Extends existing email fallback (v1.0 AL-03). Needs priority classification on messages (critical vs. routine). |
| **Protocol version negotiation** | v1.0 hard-rejects mismatched versions. v2.0 should negotiate -- server advertises supported versions on connect, client picks highest common. Enables rolling upgrades without coordinated deploys. | LOW | Handshake message after WebSocket open, before first heartbeat. Simple min/max version exchange. |

## Anti-Features

Features to explicitly NOT build for v2.0. Tempting but wrong for this system.

| Anti-Feature | Why It Seems Appealing | Why Avoid | What to Do Instead |
|--------------|------------------------|-----------|-------------------|
| **External message broker (Redis/RabbitMQ/NATS)** | "Real" message queues use brokers. At-least-once delivery is their bread and butter. | Two nodes. One sender, one receiver. Adding Redis means another service to monitor, another failure mode, another deploy target. The broker would have more infrastructure complexity than the entire comms-link. | File-backed write-ahead log (JSON lines). Append on send, delete on ACK. Survives crashes. Zero dependencies. |
| **OpenTelemetry / Prometheus stack** | Industry standard for observability. Collectors, exporters, dashboards. | Two nodes. One human consumer (Uday, via Bono). OTEL SDK adds ~15 npm packages. Prometheus needs a server. Grafana needs hosting. All for metrics that can be 20 lines of JSON in a heartbeat. | Export counters as JSON in heartbeat payload. Bono logs them. If Uday wants a chart, Bono generates one from the log. |
| **OAuth / JWT for command authorization** | Enterprise pattern for agent-to-agent auth. Token-based delegation, scoped permissions. | Two known, trusted AI agents on machines we own. PSK auth is already in place. OAuth adds token refresh, JWKS endpoints, and a whole identity layer for a system with exactly 2 principals. | PSK for transport auth (already done). Command-level auth via allowlist + human approval for dangerous commands. |
| **Consensus protocol (Raft/Paxos)** | "What if they disagree on state?" | Two nodes cannot run consensus (need odd number for quorum). They're not replicating state -- they're exchanging messages. Consensus solves a problem that doesn't exist here. | Designated authority per data type (already in v1.0: cloud-authoritative for drivers/pricing, local-authoritative for billing/laps). |
| **Persistent message database (SQLite/Postgres)** | "Store all messages for audit trail." | Messages are coordination ephemera. The valuable state is in LOGBOOK.md (synced) and racecontrol's database. Duplicating it in a message store creates two sources of truth. | Rotate log files. The WAL (write-ahead log) for the message queue is the only persistence needed, and it's cleaned up after ACK. |
| **Complex RBAC / permission system** | "Different permission levels for different command types." | Two agents. One human boss. The permission matrix is: "James can do everything on James's machine, Bono can do everything on Bono's VPS, cross-machine commands need approval." That's 3 rules, not a permission system. | Hardcoded allowlist per direction. Dangerous commands (reboot, kill process, delete files) require human approval. Safe commands (status, health check) auto-approve. |
| **Message ordering guarantees (total order)** | "Messages must be processed in exactly the order sent." | WebSocket already provides ordered delivery within a connection. The only ordering issue is across reconnects, and sequence numbers handle that. Total order across multiple channels (WS + email fallback) is a distributed systems nightmare for zero benefit. | Sequence numbers per session. On reconnect, replay from last ACKed sequence. Accept that email-fallback messages may arrive out of order (they're alerts, not transactions). |

## Feature Dependencies

```
[Message ACK + Sequence Numbers]
    |
    |--enables--> [Retry with Timeout]
    |--enables--> [Idempotency Dedup]
    |--enables--> [Metrics: ACK success rate]
    |
    |--required-by--> [Transactional Message Queue]
    |                      |
    |                      |--replaces--> INBOX.md (v1.0)
    |                      |--enables--> [Graceful Degradation Modes]
    |
    |--required-by--> [Bidirectional Task Routing]
                           |
                           |--enables--> [Remote Command Execution]
                           |                  |
                           |                  |--requires--> [Approval Flow]
                           |
                           |--enables--> [Structured Status Queries]

[Watchdog-of-Watchdog]
    |
    |--independent of--> WebSocket layer (uses Task Scheduler)
    |--monitors--> [Claude Watchdog (v1.0)]

[Health Snapshots]
    |
    |--extends--> [Heartbeat (v1.0)]
    |--includes--> [Queue Depth from Transactional Queue]
    |--includes--> [Metrics Counters]

[Protocol Version Negotiation]
    |
    |--extends--> [Protocol v1 (existing)]
    |--independent of--> all other v2 features
    |--should be FIRST--> enables rolling upgrades for everything else
```

### Critical Path

1. **Protocol version negotiation** -- must land first so v2 features can be added incrementally without breaking v1 connectivity
2. **Message ACK + sequence numbers** -- foundation for everything reliable
3. **Transactional message queue** -- replaces fragile INBOX.md
4. **Bidirectional task routing** -- structured request/response
5. **Remote command execution** -- the headline feature, depends on all above
6. **Watchdog-of-watchdog** -- independent track, can be built in parallel
7. **Health snapshots + metrics** -- lowest risk, extends existing heartbeat

## MVP Recommendation

### Must Ship (defines v2.0)

1. **Message ACK with sequence numbers** -- Without delivery confirmation, every other reliability feature is built on sand. This is the single most important addition.
2. **Transactional message queue (file-backed WAL)** -- Eliminates the INBOX.md file race and survives crashes. JSON lines file, append on send, truncate on ACK.
3. **Bidirectional task routing** -- Replaces one-way INBOX with structured request/response. Uses existing `task_request`/`task_response` types with correlation IDs.
4. **Watchdog-of-watchdog** -- Windows Task Scheduler task, runs every 5 minutes, checks if watchdog process is alive. Prevents a repeat of the 15-hour blind outage.
5. **Health snapshots in heartbeats** -- Low effort, high value. Extends existing heartbeat payload.

### Add After Stable (v2.1)

6. **Remote command execution with approval** -- High complexity, depends on all routing being stable first. Ship the plumbing (routing, ACK, queue) before adding the most dangerous feature.
7. **Metrics export** -- Useful but not urgent. Counters can accumulate from day one; export format can wait.
8. **Idempotency dedup** -- Retry-induced duplicates are a theoretical risk. Add when (if) it actually happens.

### Defer

- **Protocol version negotiation** -- Valuable but both sides deploy together today (James commits, Bono pulls). Add when independent deploy cadence becomes real.
- **Graceful degradation modes** -- Email fallback exists but is untested. Validate email E2E first (already in scope), then build degradation modes.

## Complexity Assessment

| Feature | Estimated Effort | Risk | Notes |
|---------|-----------------|------|-------|
| Message ACK + seq numbers | 2-3 phases | Low | Well-understood TCP-like pattern. Main risk: getting the replay-on-reconnect right. |
| File-backed message queue | 2 phases | Medium | File locking on Windows is the risk. Use rename-on-write (write to .tmp, rename to .wal) for atomicity. |
| Bidirectional task routing | 1-2 phases | Low | Protocol types already exist. Need correlation ID tracking and timeout. |
| Watchdog-of-watchdog | 1 phase | Low | `schtasks /create` one-liner. Check process, restart if missing. |
| Health snapshots | 1 phase | Low | Extend heartbeat payload. Read from localhost APIs. |
| Remote command execution | 3-4 phases | High | Approval flow is the hard part. Allowlist management, timeout handling, result streaming. |
| Metrics export | 1 phase | Low | In-memory counters, JSON serialization. |

## Implementation Patterns

### Message ACK Protocol

The standard pattern (from TCP, NATS, AMQP) is:
- Sender assigns monotonic sequence number per message
- Receiver ACKs with the sequence number it received
- Sender tracks outstanding (unACKed) messages with timeouts
- On timeout: retry up to N times, then mark as failed
- On reconnect: replay from `last_acked_seq + 1`

For this system: per-session sequence numbers (reset on reconnect) with a small outstanding window (max 10 unACKed). No need for persistent sequence numbers across sessions -- the WAL handles crash recovery.

### File-Backed Write-Ahead Log

Pattern: JSON Lines file (one JSON object per line).
- **Send:** Append message to `.wal` file, then send over WebSocket
- **ACK received:** Mark message as delivered in WAL (or rewrite WAL without it)
- **Crash recovery:** On startup, read WAL, resend all unACKed messages
- **Rotation:** After all messages ACKed, truncate WAL

Avoid SQLite -- it's a dependency for what is fundamentally "append line to file, delete line from file." JSON Lines with `fs.appendFileSync` (already used for INBOX.md) plus periodic compaction is sufficient.

### Approval Flow for Remote Commands

Three tiers:
1. **Auto-approve:** Read-only commands (status, health, version, disk space). Execute immediately, return result.
2. **Notify-and-execute:** Moderate commands (restart service, clear cache). Execute immediately, notify Uday via WhatsApp.
3. **Require approval:** Dangerous commands (reboot, kill process, delete files, deploy binary). Pause execution, notify Uday, wait for approval (WhatsApp reply or timeout). Default-deny on timeout.

The allowlist is a static config (JSON or TOML), not a database. Two agents, finite command set.

### Watchdog-of-Watchdog

The IBM Tivoli pattern: a "physical watchdog" (separate process) monitors the "logical watchdog." On Windows, the simplest reliable implementation is a Task Scheduler task:
- Trigger: every 5 minutes
- Action: PowerShell script that checks `tasklist /FI "IMAGENAME eq node.exe"` for the watchdog process
- If missing: start watchdog via `start-watchdog.bat`
- If present: exit silently

This is simpler and more reliable than a Windows Service (no service registration, no SCM complexity, survives across sessions).

## Sources

- [NATS Message Acknowledgment Patterns](https://oneuptime.com/blog/post/2026-02-02-nats-message-acknowledgment/view) -- ACK types, at-least-once delivery
- [At-Least-Once Delivery Design](https://oneuptime.com/blog/post/2026-01-30-at-least-once-delivery/view) -- persistent storage + ACK tracking + retry
- [IBM Watchdog-of-Watchdog Architecture](https://www.ibm.com/support/pages/itm-agent-insights-watchdog-service-monitoring-os-agents) -- physical/logical watchdog pattern
- [Decision Gateway Pattern for Agent Authorization](https://medium.com/advisor360-com/designing-authorization-for-production-ai-agents-the-decision-gateway-pattern-59582093ccb8) -- just-in-time approval, pause-and-wait
- [Agent Authorization Best Practices (Oso)](https://www.osohq.com/learn/best-practices-of-authorizing-ai-agents) -- scoped permissions, human-in-the-loop
- [Microsoft Copilot Studio Approvals](https://learn.microsoft.com/en-us/microsoft-copilot-studio/flows-advanced-approvals) -- multistage approval in agent flows
- [Octopus Deploy Service Watchdog](https://octopus.com/docs/administration/managing-infrastructure/service-watchdog) -- Task Scheduler-based process monitoring
- [C# Watchdog (thijse/Watchdog)](https://github.com/thijse/Watchdog) -- configurable process monitoring and restart

---
*Feature research for: Comms Link v2.0 -- Reliable AI-to-AI Communication*
*Researched: 2026-03-20*
