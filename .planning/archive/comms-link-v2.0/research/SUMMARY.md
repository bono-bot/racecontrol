# Project Research Summary

**Project:** comms-link v2.0
**Domain:** AI-to-AI communication infrastructure — process supervision, reliable messaging, remote execution, observability
**Researched:** 2026-03-20
**Confidence:** HIGH

## Executive Summary

comms-link v2.0 is a reliability and capability upgrade to an existing AI-to-AI WebSocket link between two known machines (James at 192.168.31.27 and Bono at 72.60.101.58). v1.0 is live and working but has three production gaps: the watchdog dies silently mid-session (caused a 15-hour blind outage Mar 17-18), messages are fire-and-forget with no delivery confirmation, and INBOX.md-based task routing has race conditions with git. The recommended v2.0 approach adds process supervision, ACK-based reliable delivery, and a transactional message queue — all without introducing heavy dependencies or external brokers. This is a 2-node, tens-of-messages-per-minute system; the correct tool for every reliability problem is Node.js stdlib plus one carefully chosen addition for durable storage.

The critical path is clear from combined research: protocol foundation first (ACK tracking plus durable queue), process supervisor second as an independent track that immediately addresses the live production gap, then wire ACK/queue into both sides of the daemon, then remote execution (the headline new feature), then observability. This order respects the dependency chain where remote execution requires reliable delivery, which requires the ACK/queue foundation. The architecture preserves all v1.0 patterns (dependency injection, EventEmitter, HTTP relay) so the existing 222 tests remain valid as integration checkpoints throughout.

The biggest risks are: (1) shell injection if remote command execution is implemented carelessly — use the array-args form of child process invocation with an enum-based allowlist, never the shell-string form; (2) ACK infinite loops if ACKs are themselves ACKed — control messages must never require ACKs; (3) NTFS file locking breaking atomic queue writes on Windows — use append-only WAL or add Defender exclusions; (4) duplicate command execution on reconnect replay — all remote commands must be idempotent. All four risks have proven mitigations documented in prior research.

## Key Findings

### Recommended Stack

v1.0's minimal footprint (ws plus Node.js stdlib) is a strength to preserve. There is one unresolved tension in the research: STACK.md recommends **better-sqlite3@^12.8.0** for the durable queue and metrics, while ARCHITECTURE.md and FEATURES.md independently recommend a **JSON WAL file** (append-only JSON lines) instead. Both solve the problem; the difference is compilation complexity vs querying power. This decision must be made at planning time before Phase 1 begins.

**Core technologies:**
- `ws@^8.19.0`: WebSocket transport — unchanged from v1.0, no action needed
- `better-sqlite3@^12.8.0` OR JSON WAL file: Durable queue and metrics store — SQLite gives queryable history and a clean API but requires C++ build tools on Windows and `build-essential` on the VPS; WAL file has zero native dependencies but is less queryable
- `node:child_process` (array-args form only): Remote command execution — already in use for watchdog and system-metrics; must NEVER use the shell-string form for remote commands
- `node:http.createServer`: Metrics endpoint on port 8766 — 15 lines, no Express needed
- `schtasks.exe` (via child process): Watchdog-of-watchdog supervisor scheduling — OS-level process, cannot crash

**What to explicitly avoid:** Redis/RabbitMQ/NATS (broker overkill for 2 nodes), prom-client/OTEL (one consumer doesn't justify it), NSSM as Windows service (Session 0 problem proven in v1.0), TypeScript (build step overhead for infrastructure glue code), shell-string child process form for remote commands, dotenv.

### Expected Features

**Must have (table stakes — v2.0 doesn't justify its existence without these):**
- Message ACK with sequence numbers — delivery confirmation for task and remote execution messages, retry with exponential backoff
- Transactional message queue — replaces INBOX.md file races, crash-resilient at-least-once delivery
- Bidirectional task routing — v1.0 is James-to-Bono only; Bono needs to initiate requests with correlation IDs
- Watchdog-of-watchdog via Task Scheduler — prevents repeat of the 15-hour blind outage
- Health snapshots in heartbeats — pod status, queue depth, deployment state visible to Bono

**Should have (high value, ship after stable core):**
- Remote command execution with approval — Bono sends "run X on James", three-tier approval (auto/notify/require), Claude Code approves dangerous commands
- Metrics export as structured JSON — uptime, reconnect count, ACK latency, queue depth
- Idempotency dedup via seen-message cache — prevents duplicate processing on reconnect replay

**Defer to v2.1+:**
- Protocol version negotiation — valuable when sides deploy independently; today they deploy together
- Graceful degradation modes — email fallback exists but needs E2E validation first
- External message broker — wrong tool for 2 nodes; never build this

### Architecture Approach

The v2.0 architecture multiplexes all new features over the single existing persistent WebSocket. Six new files are added (4 shared modules, 2 james modules), four existing files are modified (protocol.js, james/index.js, bono/index.js, bono/comms-server.js), and ping-heartbeat.js is replaced entirely by process-supervisor.js. The supervisor monitors the daemon via HTTP health check only — it does NOT open its own WebSocket connection (that would duplicate the daemon's auth, reconnect, and message handling). INBOX.md is demoted to human-readable audit log only, never read programmatically.

**Major components:**
1. `shared/ack-tracker.js` — Track sent messages awaiting ACK, retry with EscalatingCooldown; auto-send msg_ack on Bono side for exec/task messages; only data messages require ACKs, never control messages
2. `shared/message-queue.js` — Durable file-backed queue (enqueue/dequeue/ACK semantics) replacing INBOX.md programmatic use; atomic write with NTFS-safe retry fallback
3. `shared/exec-protocol.js` — Enum-based command allowlist, ApprovalMode (auto vs require-ack), array-args invocation only, sanitized environment, no shell-string form
4. `james/process-supervisor.js` — Spawns daemon as child process, monitors via HTTP /relay/health every 15s, respawns on failure using EscalatingCooldown; replaces ping-heartbeat.js
5. `james/exec-handler.js` — Receives exec_request, checks allowlist, runs via array-args child process, returns exec_result; approval gate for non-auto commands via HTTP relay routes
6. `shared/metrics-schema.js` — Metrics snapshot format and Prometheus text export for Bono's /metrics endpoint

**Patterns to preserve from v1.0:** Dependency injection (injectable functions for all external dependencies), EventEmitter for cross-component notifications, protocol envelope extension not replacement, HTTP relay for local service integration, atomic file write (write tmp then rename to target).

### Critical Pitfalls

1. **Shell injection via unsafe child process form** (Pitfall 18, CRITICAL) — Remote commands executed with the shell-string child process API allow metacharacter injection. The n8n CVE-2025-68613 (CVSS 9.4) is a recent real-world example. Prevention: always use the array-args form of child process invocation with an enum command allowlist. Never pass `shell: true`. Dangerous commands require explicit approval before execution.

2. **ACK-of-ACK infinite loop / ACK storm** (Pitfall 19, CRITICAL) — If ACKs trigger further ACKs, the connection saturates at wire speed. Prevention: define data messages (require ACK: exec_request, task_request) vs control messages (never ACKed: msg_ack, heartbeat_ack, exec_result). First line of receive handler must gate on message type before any ACK logic runs.

3. **Queue file corruption on crash and NTFS locking** (Pitfalls 21+25, CRITICAL) — `fs.writeFileSync` is not atomic on Windows; rename fails with EBUSY under Defender. Prevention: use append-only WAL (no rename needed, avoids NTFS locking entirely) OR implement rename with 3-retry exponential backoff plus Defender directory exclusion.

4. **Duplicate command execution on reconnect replay** (Pitfall 24, MODERATE) — Messages in-flight before disconnect are replayed on reconnect, causing double execution of remote commands. Prevention: receiver-side UUID dedup cache (last 1000 IDs or 1-hour TTL), sequence-based replay from last-ACKed-seq, all remote commands designed to be idempotent.

5. **Duplicate process instances from liveness-only watchdog** (Pitfall 23, MODERATE) — Process listed in tasklist does not mean the process is healthy or responsive. Prevention: health check (did heartbeat arrive within last 60s?) not just liveness check; PID lockfile prevents spawning a second instance alongside a hung one.

## Implications for Roadmap

Based on combined research, the build order is driven by a clear dependency chain. Each phase is independently testable and deployable. Phases 1 and 2 can proceed in parallel.

### Phase 1: Protocol Foundation
**Rationale:** Every v2.0 reliability feature depends on ACK tracking and the durable queue. These are pure library code with no side effects — safest to build and test first. Ship as new modules without wiring them into the daemon yet.
**Delivers:** `shared/ack-tracker.js`, `shared/message-queue.js`, protocol.js additions (msg_ack, exec_request, exec_result, health_snapshot, metrics_push message types). Target 30+ new unit tests.
**Addresses:** Message ACK with sequence numbers, transactional queue, retry with timeout.
**Avoids:** Pitfall 19 (design the control/data message split here before any code), Pitfall 20 (use monotonic integer counter, never Date.now() as sequence), Pitfall 21+25 (choose WAL vs SQLite here and lock in the atomic write strategy).

### Phase 2: Process Supervisor (parallel track)
**Rationale:** Independent of ACK/queue. Directly addresses the highest-priority live production gap (15-hour blind outage). Can ship standalone to immediately improve production reliability. James-only change, no Bono coordination required.
**Delivers:** `james/process-supervisor.js` (replaces ping-heartbeat.js), updated start-comms-link.bat. Reuses existing EscalatingCooldown from watchdog.js.
**Addresses:** Watchdog-of-watchdog requirement, mid-session crash recovery.
**Avoids:** Pitfall 23 (PID lockfile plus health check, not just liveness check; tasklist /FO CSV not wmic which is deprecated on Windows 11).

### Phase 3: Wire ACK and Queue into Daemon
**Rationale:** Foundation exists (Phase 1), supervision is solid (Phase 2). Wire the reliability layer into the actual message flow. This is the riskiest phase as it touches both sides simultaneously. Having the supervisor deployed means daemon deploy mistakes are self-healing.
**Delivers:** Modified james/index.js (queue.enqueue replaces appendFileSync, AckTracker wired in), modified bono/index.js (auto-sends msg_ack for exec/task messages, dedup cache added).
**Addresses:** Crash-resilient delivery, INBOX.md race elimination, bidirectional task routing with correlation IDs.
**Avoids:** Pitfall 24 (receiver UUID dedup cache wired here).
**Deploy order:** Bono first (backward compatible — sending ACKs for messages that don't expect them is harmless), then James.

### Phase 4: Remote Command Execution
**Rationale:** The headline v2.0 feature. Depends on reliable delivery (ACK plus queue from Phase 3) — exec_request must be reliably delivered before execution. Approval gate depends on queue for pending-approval persistence across daemon restarts.
**Delivers:** `shared/exec-protocol.js` (enum allowlist, ApprovalMode), `james/exec-handler.js` (array-args child process execution plus approval gate), HTTP relay routes for pending approvals and approval actions.
**Addresses:** Remote command execution with three-tier approval (auto-approve read-only, notify-and-execute moderate, require-approval dangerous).
**Avoids:** Pitfall 18 (array-args invocation with enum allowlist, never shell-string form), Pitfall 22 (sanitized env passed explicitly — only PATH/SYSTEMROOT/TEMP, never inherit full process env containing PSK and API keys).

### Phase 5: Health Snapshots and Metrics
**Rationale:** Observability layer. No other feature depends on it. Low risk — extends existing heartbeat payload and adds a simple HTTP endpoint. Metrics counters can accumulate from day one; export format ships last.
**Delivers:** `shared/metrics-schema.js`, extended system-metrics.js (pod status from rc-core :8080), metrics_push on 60s interval (no ACK required — next push supersedes), Bono's GET /metrics endpoint.
**Addresses:** Health snapshots in heartbeats, metrics export, Bono's full operational visibility into James's world.
**Avoids:** Pitfall 26 (bounded labels only: pod_id 1-8, message_type enum, state enum, error_category enum — never unbounded IDs or raw error strings as label values).

### Phase Ordering Rationale

- Phases 1 and 2 can be worked in parallel — Phase 2 addresses the live production gap immediately while Phase 1 builds the foundation.
- Phase 3 requires Phase 1 complete (cannot wire what doesn't exist) and benefits from Phase 2 being deployed (supervisor means daemon deploy mistakes are self-healing).
- Phase 4 requires Phase 3 stable (reliable delivery must exist before adding potentially dangerous remote execution).
- Phase 5 is safe to defer — no blockers, no other phase depends on it, incremental value only.
- Each phase has a clear deploy boundary: James-only (Phases 1, 2, 4 initial wiring) vs coordinated deploy (Phase 3 requires Bono updated first).

### Research Flags

Phases likely needing deeper research during planning:
- **Phase 3 (Wire ACK + Queue):** The Stack vs Architecture disagreement on SQLite vs WAL file must be explicitly resolved before Phase 1 begins. better-sqlite3 requires native compilation on both Windows and Linux VPS — verify build toolchain (Visual Studio Build Tools on James, build-essential on Bono's VPS) before committing to that path.
- **Phase 4 (Remote Execution):** The exact command allowlist needs to be defined as a product decision before writing exec-protocol.js. What specific commands does Bono actually need to run on James's machine? Define the enum before coding.

Phases with standard patterns (skip research-phase):
- **Phase 1 (Protocol Foundation):** Well-documented TCP-like ACK patterns. Existing codebase provides clear extension points with defined API shapes.
- **Phase 2 (Process Supervisor):** schtasks / Task Scheduler pattern proven in v1.0 ecosystem. EscalatingCooldown already implemented in watchdog.js — reuse directly.
- **Phase 5 (Metrics):** JSON metrics endpoint is trivial. Prometheus text format is a stable spec. Low-risk extension of existing heartbeat payload.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | Based on codebase inspection plus npm registry verification. One unresolved tension: better-sqlite3 (STACK.md) vs JSON WAL (ARCHITECTURE.md and FEATURES.md). Both valid; decision needed at planning before Phase 1. |
| Features | HIGH | Well-understood patterns (ACK, WAL, process supervision) from messaging and OS ecosystem research. Feature list validated against documented v1.0 production gaps. |
| Architecture | HIGH | Based on direct codebase analysis of all v1.0 source files. Component API shapes are fully specified with clear integration points. |
| Pitfalls | HIGH | All critical pitfalls have real-world evidence: v1.0 15-hour outage, npm CLI #9021 (NTFS locking), OWASP shell injection, Windows 15.6ms timer docs. Not speculative. |

**Overall confidence:** HIGH

### Gaps to Address

- **SQLite vs WAL file decision:** STACK.md recommends better-sqlite3; ARCHITECTURE.md and FEATURES.md independently recommend JSON WAL. Recommendation: use JSON WAL for the queue (zero native deps, avoids compilation on both platforms) and defer SQLite consideration to if/when metrics need long-term persistent storage. Decide and document before Phase 1 planning begins.
- **Exact remote command allowlist:** Phase 4 requires a defined enum of commands Bono will actually request. This is a product decision, not a technical one. Collect from Bono's operational needs before Phase 4 planning.
- **Bono's VPS Node.js version:** STACK.md notes better-sqlite3 requires Node >= 20. Bono's VPS version was not confirmed in research. Verify with `node --version` on the VPS if the SQLite option is chosen.
- **Email fallback E2E validation:** The graceful degradation mode (WS down to email fallback) is deferred but email has never been tested end-to-end (noted in FEATURES.md). Should be validated before it is relied on as a safety net in degradation mode design.

## Sources

### Primary (HIGH confidence)
- Direct codebase analysis — all comms-link v1.0 source files (james/, bono/, shared/, ping-heartbeat.js, 22 test files)
- [better-sqlite3 npm](https://www.npmjs.com/package/better-sqlite3) — v12.8.0, published March 2026
- [Node.js SQLite experimental docs](https://nodejs.org/api/sqlite.html) — confirmed experimental in Node 22.14.0, prints ExperimentalWarning
- [Node.js child_process docs](https://nodejs.org/api/child_process.html) — array-args vs shell-string invocation security model
- v1.0 production incident — 15-hour blind outage (Mar 17-18, 2026), PROJECT.md post-mortem
- Existing test suite — 22 test files, 222 tests using node:test
- `node --version` on James: 22.14.0 (confirmed)

### Secondary (MEDIUM confidence)
- [NATS Message Acknowledgment Patterns](https://oneuptime.com/blog/post/2026-02-02-nats-message-acknowledgment/view) — ACK types, at-least-once delivery
- [At-Least-Once Delivery Design](https://oneuptime.com/blog/post/2026-01-30-at-least-once-delivery/view) — persistent storage plus ACK tracking plus retry
- [IBM Watchdog-of-Watchdog Architecture](https://www.ibm.com/support/pages/itm-agent-insights-watchdog-service-monitoring-os-agents) — physical/logical watchdog pattern
- [Decision Gateway Pattern for Agent Authorization](https://medium.com/advisor360-com/designing-authorization-for-production-ai-agents-the-decision-gateway-pattern-59582093ccb8) — approval flow design
- [Agent Authorization Best Practices (Oso)](https://www.osohq.com/learn/best-practices-of-authorizing-ai-agents) — scoped permissions, human-in-the-loop
- npm CLI GitHub issue #9021 — write-file-atomic EBUSY on Windows (NTFS locking evidence)
- n8n CVE-2025-68613 (CVSS 9.4) — real-world expression injection bypassing sandboxes (shell injection evidence)
- Microsoft Windows documentation — Date.now() 15.6ms timer resolution

---
*Research completed: 2026-03-20*
*Ready for roadmap: yes*
