# Domain Pitfalls: v2.0 Comms-Link Features

**Domain:** Adding reliability, remote execution, and observability to existing AI-to-AI communication system
**Researched:** 2026-03-20
**Context:** v1.0 shipped with 17 documented pitfalls. This document covers NEW pitfalls specific to adding v2.0 features (process supervision, message ACK, transactional queue, remote command execution, metrics). Cross-references v1.0 pitfalls where they interact.

---

## Critical Pitfalls

Mistakes that cause security breaches, data loss, or multi-hour outages.

---

### Pitfall 18: Remote Command Execution -- Shell Injection via Unsafe Child Process APIs

**What goes wrong:** Bono sends a command like `{ cmd: "git status" }` over WebSocket. James executes it using the shell-based child process API (the one that accepts a single string and passes it to cmd.exe). An attacker who compromises Bono's side (or a bug in message construction) sends `{ cmd: "git status & del /s /q C:\\" }` and the shell interprets the ampersand as a command separator. Even without malicious intent, commands containing user-derived strings (pod names, file paths) can accidentally inject shell metacharacters.

**Why it happens:** The shell-based child process API spawns `cmd.exe` on Windows and passes the entire command string to it. Shell metacharacters (`&`, `|`, `;`, `` ` ``, `$()`, `>`, `<`) are interpreted. This is the single most common Node.js security vulnerability in command execution -- documented extensively in OWASP, Snyk, and the Node.js security book by Liran Tal.

**Consequences:** Arbitrary code execution on James's machine. Full compromise of the venue network (James is at .27 with access to all 8 pods). Potential data exfiltration, ransomware, or silent backdoor installation.

**Prevention:**
- **ALWAYS use `execFile()` (or the project's `execFileNoThrow` utility) which does NOT spawn a shell** -- arguments are passed as an array, and metacharacters are treated as literal strings.
- **Command allowlist:** Define a strict enum of permitted commands (e.g., `git_status`, `pod_health`, `restart_agent`). Each maps to a hardcoded `execFile` call with predefined arguments. Remote side sends the command NAME, not the command STRING.
- **Argument validation:** If commands accept parameters (e.g., pod number), validate against a regex allowlist (`/^[1-8]$/` for pod numbers). Reject anything with shell metacharacters.
- **No `shell: true` option:** Even with `execFile`/`spawn`, the `{ shell: true }` option re-enables shell interpretation. Never set it.
- **Approval flow for dangerous commands:** Commands like `restart`, `deploy`, `kill` require explicit confirmation (second message within 30s timeout). Read-only commands (`status`, `health`) can auto-execute.

**Detection:** Log every remote command execution with: sender, command name, arguments, exit code, stdout/stderr (truncated). Alert on any command that is not in the allowlist.

**Phase:** Remote Command Execution phase -- this is the FIRST thing to design before writing any execution code.

**Confidence:** HIGH -- shell injection is the most documented Node.js vulnerability. The n8n CVE-2025-68613 (CVSS 9.4) is a recent real-world example of expression injection bypassing sandboxes.

---

### Pitfall 19: ACK-of-ACK Infinite Loop (ACK Storm)

**What goes wrong:** James sends message M1. Bono sends ACK(M1). James's protocol layer treats the ACK as a new message requiring acknowledgment, so James sends ACK(ACK(M1)). Bono's protocol layer does the same: ACK(ACK(ACK(M1))). Infinite ping-pong at wire speed, saturating the WebSocket and CPU.

**Why it happens:** The ACK protocol does not distinguish between "messages that require ACKs" and "control messages (ACKs) that do not." If every inbound message triggers an outbound ACK, and ACKs are themselves messages, the loop is inevitable. This is a well-documented TCP-level vulnerability (RFC-documented ACK storm attack) that also manifests in application-level protocols.

**Consequences:** WebSocket saturated with ACK traffic. CPU pegged parsing/generating ACK messages. Legitimate messages blocked or dropped. Connection may be killed by the OS due to send buffer overflow.

**Prevention:**
- **ACKs MUST NOT be ACKed.** Define a clear split: `data messages` (heartbeat, task_request, file_sync, command) require ACKs. `control messages` (heartbeat_ack, task_response, ack) NEVER require ACKs.
- **In the protocol envelope:** Add an `ack_required: boolean` field, or simpler: ACK message types are defined in a Set and the receive handler skips ACK logic for those types.
- **Implementation guard:** In the message handler, first line: `if (isControlMessage(msg.type)) return;` before any ACK logic runs.

**Detection:** Monitor messages-per-second. If rate exceeds 50/s sustained for 5s, something is looping. Kill the WebSocket and reconnect with a fresh sequence.

**Phase:** Message ACK Protocol phase -- protocol design must address this before any code is written.

**Confidence:** HIGH -- this is a textbook distributed systems mistake. The Wireshark community documents it as a common diagnostic finding.

---

### Pitfall 20: Sequence Number Wrap-Around and Timestamp-as-Sequence Antipattern

**What goes wrong:** The real danger is not integer overflow (JavaScript Number is safe to 2^53) but using `Date.now()` as a sequence number. Two messages sent in the same millisecond get the same "sequence." On reconnect replay, the receiver cannot distinguish them. Duplicate delivery results.

**Why it happens:** Developers use timestamps because they seem monotonically increasing and unique. They are neither on Windows -- `Date.now()` resolution is 15.6ms (the Windows timer tick), meaning messages sent within the same tick get identical timestamps.

**Consequences:** Duplicate command execution (dangerous for remote commands -- "restart pod 3" runs twice). Duplicate file syncs overwriting concurrent changes. Lost messages when receiver says "I have everything up to seq X" but actually missed a message with the same timestamp.

**Prevention:**
- **Use a simple integer counter starting at 0.** Increment by 1 per message. Store as JavaScript Number (safe up to 2^53).
- **Per-sender sequence:** James has his own counter, Bono has his own. On reconnect: "My last sent was 47, your last I received was 31."
- **Never use timestamps as sequence numbers.** Timestamps are metadata, not ordering primitives.
- **Persist the counter:** Write the current sequence number to a file on clean shutdown. On restart, read it back and continue from there (not from 0, which would cause the receiver to think all messages are "old").

**Detection:** On receive, check `msg.seq > last_received_seq`. If `msg.seq <= last_received_seq`, it is a duplicate or replay -- log and discard. If `msg.seq > last_received_seq + 1`, messages were lost -- request replay of the gap.

**Phase:** Message ACK Protocol phase.

**Confidence:** HIGH -- Windows timer resolution is documented by Microsoft. The timestamp-as-sequence antipattern is well-known.

---

### Pitfall 21: Transactional Queue File Corruption on Power Loss / Crash

**What goes wrong:** The transactional queue writes a message to a JSON file (replacing INBOX.md). The write consists of: read file, parse JSON, append message, serialize JSON, write file. If the process crashes or power is lost between "write file" start and completion, the file contains partial JSON -- the queue is now unreadable. All pending messages are lost.

**Why it happens:** `fs.writeFileSync()` and `fs.writeFile()` are not atomic. The OS may write partial data to disk. Even writeFileSync followed by fsyncSync is not sufficient if the crash occurs between the write and the fsync. On NTFS, the situation is complicated by Windows Defender and Windows Search Indexer holding file handles, which can cause `EBUSY` or `EPERM` on rename operations.

**Evidence:** The existing INBOX.md already has this problem (documented in PROJECT.md: "appendFileSync collides with git operations"). The npm CLI has a known issue (#9021) where `write-file-atomic` using `fs.rename()` fails on Windows due to transient file locks from antivirus.

**Consequences:** Queue file corrupted. All pending messages lost. System must recover from empty state. If the queue file is also the persistence layer (no WAL), messages that were "accepted" but not yet delivered are permanently lost.

**Prevention:**
- **Write-ahead log (WAL) pattern:** Instead of rewriting the entire queue file, append new entries to a log file (one JSON line per message). On startup, replay the log to reconstruct the queue state. Appends are much safer than rewrites -- a partial append only corrupts the last entry, not the entire file.
- **Atomic write with retry:** Write to `.queue.tmp`, fsync, then rename to `.queue.json`. If rename fails (EBUSY from Defender), retry 3 times with 100ms delay.
- **Exclude queue directory from Windows Defender real-time scanning:** Add the comms-link data directory to Defender exclusions (the existing deploy infrastructure already does this for rc-agent).
- **Exclude from Windows Search Indexer:** The indexer locks files it is processing. Either exclude the directory or use a file extension the indexer ignores (e.g., `.wal` instead of `.json`).
- **Checksums:** Store a CRC32 at the end of each WAL entry. On replay, skip entries with invalid checksums (partial writes from crashes).

**Detection:** On startup, attempt to parse the queue file. If parse fails, log the corruption, attempt recovery from WAL, and alert. Never silently start with an empty queue.

**Phase:** Transactional Message Queue phase -- this is the core reliability feature.

**Confidence:** HIGH -- file corruption on crash is a fundamental problem. The Dan Luu "Files are hard" article documents how even databases get this wrong.

---

### Pitfall 22: Remote Command Privilege Escalation via Inherited Environment

**What goes wrong:** James executes a remote command using `execFile`. The child process inherits James's full environment: `COMMS_PSK` (the WebSocket secret), `PATH`, any API keys in env vars, and the `LOCALAPPDATA` path giving access to Claude's auth tokens. A seemingly harmless command like `env` or `set` leaks all secrets back to the caller over the WebSocket.

**Why it happens:** `execFile()` inherits the parent's environment by default. The output (stdout/stderr) is sent back over the WebSocket as the command result. Even if the allowlisted command itself is safe, its output may contain sensitive data.

**Consequences:** PSK leaked -- attacker can impersonate either side. API keys leaked -- access to WhatsApp, Evolution, Google Workspace. Auth tokens leaked -- full access to Claude, GitHub.

**Prevention:**
- **Sanitize the environment:** Pass an explicit `env` option to `execFile` with only the variables the command needs. Start with an empty env and add back only `PATH`, `SYSTEMROOT`, `TEMP`, `TMP`.
- **Output filtering:** Before sending stdout/stderr back over WebSocket, scan for known secret patterns (64-char hex strings, Bearer tokens, API keys). Redact or reject if found.
- **Never allowlist `env`, `set`, `printenv`, `Get-ChildItem Env:`,** or any command that dumps environment variables.
- **Never allowlist `type`, `cat`, `Get-Content`** with arbitrary file paths -- these could read config files containing secrets.
- **Principle of least privilege:** Each allowlisted command should document exactly what environment variables and file access it needs.

**Detection:** Log command output size. If output exceeds expected size for the command type (e.g., `git status` should be < 10KB), flag for review. Monitor for known secret patterns in outbound messages.

**Phase:** Remote Command Execution phase -- design the execution sandbox alongside the allowlist.

**Confidence:** HIGH -- environment inheritance is default behavior in Node.js, explicitly documented.

---

## Moderate Pitfalls

---

### Pitfall 23: Process Supervision -- Mid-Session Recovery Creates Duplicate Instances

**What goes wrong:** The watchdog detects Claude Code crashed and starts a new instance. But Claude Code did not actually crash -- it is in a hung state (consuming 0% CPU, holding open handles, responding to `tasklist` as "running" but not processing any work). The watchdog's detection returns `true` (running), so it does nothing. Alternatively: the detection was momentarily `false` due to high CPU load causing `tasklist` to timeout (5s timeout in current code), so the watchdog spawns a SECOND instance alongside the hung one.

**Why it happens:** Process liveness (PID exists) does not equal process health (doing useful work). The current watchdog only checks if `claude.exe` is in the process list. It has no health check (e.g., "did Claude respond to a ping within 10s?"). The 5-second `tasklist` timeout (line 31 of watchdog.js) means a heavily loaded system can trigger false negatives.

**Consequences:** Two Claude Code instances fighting over the same git repo, creating index.lock races (Pitfall 3 from v1.0). Two WebSocket connections to Bono -- Bono sees duplicate heartbeats, message routing is ambiguous. CPU/memory doubled.

**Prevention:**
- **Health check, not just liveness check:** After confirming the process is running, send a probe (e.g., check if the comms-link WebSocket is connected and received a heartbeat_ack in the last 60s). If the process is running but unhealthy for 3 consecutive checks, THEN kill and restart.
- **PID file with lock:** On startup, write PID to a lockfile. Before spawning a new instance, check if the PID in the lockfile is alive. If alive, do NOT spawn another. This prevents the duplicate-instance race.
- **Increase tasklist timeout:** 5s is too short under load. Use 15s, or switch to `tasklist /FI "PID eq <known_pid>"` which is faster than scanning all processes by image name.
- **Replace `tasklist` with PowerShell `Get-Process -Id $pid`:** Faster and more reliable for single-PID checks. Avoids the wmic deprecation issue.

**Detection:** On startup, check for existing lockfile. If found and PID is alive, log "Existing instance detected, skipping spawn" and exit.

**Phase:** Process Supervision phase.

**Confidence:** HIGH -- the current watchdog already has the liveness-only check (line 252 of watchdog.js). The hung-process scenario is common with Claude Code based on community reports.

---

### Pitfall 24: Duplicate Message Delivery on Reconnect Replay

**What goes wrong:** James disconnects. During the outage, James queued 5 messages (seq 10-14). James reconnects and replays the queue. But Bono had already received seq 10-12 before the connection dropped (they were in flight). Now Bono processes seq 10-12 twice.

**Why it happens:** The current queue implementation (`#queue` in CommsClient) pushes messages when `send()` fails. But `ws.send()` succeeds if the message enters the kernel send buffer -- it does not mean the remote end received it. The WebSocket `close` event fires AFTER some messages have been "sent" (buffered) but before the remote acknowledged them. On replay, these already-delivered messages are sent again.

**Consequences:** Duplicate task execution. Duplicate file syncs (possibly benign, but wastes bandwidth). Duplicate log entries. For remote commands, a "restart pod 3" executes twice -- the second restart hits a process that just started, causing unnecessary disruption.

**Prevention:**
- **Receiver-side deduplication:** Every message has a UUID (`id` field in the current protocol). The receiver maintains a Set of recently-seen IDs (last 1000 or last 1 hour). If a message ID was already seen, discard it.
- **Sequence-based replay:** On reconnect, the receiver reports `last_received_seq`. The sender replays only messages with `seq > last_received_seq`. This is more efficient than UUID dedup but requires both sides to track sequences.
- **Idempotent command design:** All remote commands MUST be idempotent. "Restart pod 3" should check if pod 3 was restarted in the last 60s and skip if so. "Sync file X" should compare hashes before overwriting.
- **At-least-once + idempotency is better than attempting exactly-once:** Do not try to build exactly-once delivery. It is provably impossible in distributed systems with crash-recovery. Instead, deliver at-least-once and make handlers idempotent.

**Detection:** Log duplicate message receipt with the duplicate ID and original receipt time. If duplicates are frequent, the replay window is too wide.

**Phase:** Message ACK Protocol phase.

**Confidence:** HIGH -- this is the fundamental exactly-once impossibility in distributed systems. The current `#queue` replay in CommsClient (line 182) has exactly this vulnerability.

---

### Pitfall 25: NTFS File Locking Breaks Atomic Queue Writes

**What goes wrong:** The transactional queue uses the atomic write pattern: write to `.tmp`, rename to `.json`. On Windows, the rename fails with `EPERM` or `EBUSY` because another process has the target file open. Common culprits: Windows Defender real-time protection, Windows Search Indexer, VS Code / editor file watchers, the comms-link itself reading the queue file.

**Why it happens:** NTFS enforces mandatory file locking. If any process has a file handle open (even for reading), `fs.rename()` targeting that file fails. This is fundamentally different from Linux/ext4 where rename succeeds even with open readers (they keep their old file descriptor). The npm CLI documents this exact issue (GitHub #9021) with `write-file-atomic`.

**Evidence:** This machine runs Windows Defender (required for pod security) and VS Code (Claude Code's environment). Both will touch files in the comms-link directory.

**Consequences:** Queue writes fail intermittently. If the code does not retry, messages are silently lost. If it retries indefinitely, the message processing pipeline stalls.

**Prevention:**
- **WAL approach avoids this entirely:** Append-only log files do not need rename. New entries are appended to the end. No file replacement needed.
- **If using rename pattern:** Retry with exponential backoff: 50ms, 100ms, 200ms, 400ms, cap at 1s. 3 retries. Log each failure with the locking process name if detectable.
- **Defender exclusion:** Add the queue directory to Defender exclusions. The existing deploy infrastructure already does this for `C:\RacingPoint\`.
- **Use `fs.copyFile` + `fs.unlink` as fallback:** If `fs.rename` fails after retries, fall back to copy-then-delete. Less atomic but succeeds through locks.
- **File extension strategy:** Use `.wal` or `.dat` instead of `.json` -- Defender and Search Indexer are less aggressive with unknown extensions.

**Detection:** Log `EBUSY`/`EPERM` errors with timestamps. If they cluster around Defender scan schedules (typically on file change), the pattern is clear.

**Phase:** Transactional Message Queue phase.

**Confidence:** HIGH -- directly evidenced by npm CLI issue #9021 and this machine running Defender.

---

### Pitfall 26: Metrics Cardinality Explosion from Dynamic Labels

**What goes wrong:** The metrics export includes labels like `pod_id`, `message_id`, `command_name`, `error_message`. Each unique combination of label values creates a new time series. With 8 pods, 5 command types, and varied error messages, this seems manageable. But then someone adds `session_id` (unbounded) or uses raw error strings as labels (each unique stack trace = new series). Memory usage grows linearly with unique label combinations. After days of operation, the metrics consumer (Bono's dashboard) runs out of memory or becomes unresponsive.

**Why it happens:** Prometheus-style metrics (counters, gauges, histograms) are indexed by their label set. Each unique label combination is a separate time series stored in memory. Developers add labels for debuggability without considering cardinality. The problem manifests gradually -- hours to days after deployment -- making it hard to catch in testing.

**Consequences:** Bono's dashboard becomes slow or crashes. Metrics storage fills disk. The metrics endpoint becomes a memory leak that degrades the entire system.

**Prevention:**
- **Bounded labels only:** `pod_id` (1-8), `message_type` (enum), `connection_state` (enum), `command_result` (success/failure). Never use IDs, timestamps, or free-text as label values.
- **Error categorization:** Instead of `error_message: "ECONNREFUSED 192.168.31.89:8090"`, use `error_category: "connection_refused"`. Map errors to a fixed set of categories.
- **Cardinality budget:** Before adding a metric, calculate: how many unique series will this create? If > 100, reconsider the label design. For this system (8 pods, ~10 message types, ~5 states), total series should be < 500.
- **Use exemplars for high-cardinality data:** Instead of labeling a metric with `session_id`, attach a trace/session ID as an exemplar on a low-cardinality metric. This preserves debuggability without cardinality cost.
- **No histograms unless needed:** Histograms create multiple series per label combination (one per bucket). Use summaries or simple gauges for latency unless you need percentile calculations.

**Detection:** Periodically log the total number of unique metric series. Alert if growth rate exceeds 10 new series/hour (indicates unbounded labels).

**Phase:** Metrics/Observability phase.

**Confidence:** HIGH -- cardinality explosion is the #1 observability pitfall, extensively documented by Grafana, Datadog, and Prometheus communities.

---

### Pitfall 27: wmic Removal Breaks Health Snapshots on Windows 11 25H2

**What goes wrong:** The health snapshot feature collects system metrics (CPU, memory, disk, process list). Existing code uses `wmic` commands. Windows 11 25H2 (shipping 2025, already in preview) REMOVES `wmic` entirely -- not just deprecated, physically removed from the OS. Health snapshots start failing silently or throwing "command not found" errors.

**Why it happens:** Microsoft has been deprecating `wmic` since Windows 10 21H2 (2021). As of Windows 11 24H2, it is disabled by default but available as a Feature on Demand. In 25H2, it is removed entirely and will NOT be available even as an optional feature in future updates.

**Evidence:** PROJECT.md already flags this: "wmic deprecated: Health check uses deprecated Windows API." Microsoft's official blog confirms complete removal in the next feature update after 25H2 (2026).

**Consequences:** Health snapshots return empty/error data. Bono's dashboard shows James as "unknown" health. Monitoring blind spot -- the exact scenario that caused the 15-hour v1.0 outage.

**Prevention:**
- **Replace ALL wmic usage NOW, not later.** Use:
  - CPU: `powershell -c "Get-CimInstance Win32_Processor | Select LoadPercentage"` or `os.cpus()` from Node.js
  - Memory: `os.freemem()` / `os.totalmem()` from Node.js (no external command needed)
  - Disk: `powershell -c "Get-CimInstance Win32_LogicalDisk"`
  - Processes: `tasklist` (NOT deprecated) or `Get-Process` via PowerShell
- **Prefer Node.js `os` module** over shelling out where possible. `os.cpus()`, `os.freemem()`, `os.totalmem()`, `os.uptime()` are cross-platform and do not depend on any external tool.
- **Test on a machine with wmic removed:** Create a test that verifies health snapshot works without wmic in PATH.

**Detection:** On startup, check if wmic exists in PATH. If yes, log a deprecation warning. If no, verify the replacement commands work.

**Phase:** Health Snapshots phase -- address early as it affects monitoring reliability.

**Confidence:** HIGH -- Microsoft's official announcement confirms removal. This is not speculative.

---

### Pitfall 28: Detached Process Handle Leak on Windows Prevents Clean Restart

**What goes wrong:** The watchdog spawns Claude Code with `spawn(exePath, [], { detached: true, stdio: 'ignore' })` and calls `child.unref()`. On Linux, this cleanly detaches. On Windows, the child process sometimes inherits handles from the parent (stdout, stderr, socket handles, file handles). When the watchdog tries to restart (kill old + spawn new), the old process holds inherited handles that prevent the new process from binding to the same resources (e.g., a port, a file lock).

**Why it happens:** Node.js on Windows uses `CREATE_NEW_PROCESS_GROUP` for detached processes but does not set `HANDLE_FLAG_INHERIT = FALSE` on all handles. Multiple open Node.js issues document this (#51018, #5614, #5146, #36808): detached child processes on Windows do not behave the same as on Linux. `stdio: 'ignore'` helps with stdio handles but does not prevent other handle types from leaking.

**Consequences:** Port 8765 (WebSocket) stays bound by the zombie process. New instance cannot start the WebSocket server -- "EADDRINUSE." Or: `.git/index.lock` held by inherited handle, blocking all git operations.

**Prevention:**
- **Use `stdio: 'ignore'` (already done in current code -- good).**
- **Kill with tree kill:** `taskkill /F /T /PID <pid>` to ensure all inherited-handle child processes are also killed.
- **Wait after kill:** Current code waits 2s (line 279 of watchdog.js). Increase to 3s on Windows to allow handle cleanup by the OS.
- **Verify port availability before starting:** Before spawning the new instance, check if port 8765 is free. If not, identify and kill the process holding it.
- **Consider Windows Job Objects:** Job Objects can contain a process and all its children, and killing the job kills everything. Node.js does not expose this natively, but the `windows-kill` npm package or a small native addon can provide it.

**Detection:** After killing the old process, run `netstat -ano | findstr :8765` to verify the port is free before spawning. If still in use, log the holding PID and kill it.

**Phase:** Process Supervision phase.

**Confidence:** MEDIUM -- the current code uses `stdio: 'ignore'` which mitigates the most common case. But handle inheritance for non-stdio handles is a known Node.js Windows limitation.

---

### Pitfall 29: Approval Flow Timeout Race Creates Stuck Commands

**What goes wrong:** Bono sends a dangerous command (e.g., "restart pod 3") that requires James's approval. James sends an approval prompt to Uday via WhatsApp. The approval has a 30-second timeout. Uday is busy and does not respond. The command times out and is rejected. But the timeout cleanup has a race: if Uday approves at second 29 and the approval message arrives at second 31 (network delay), the command was already rejected but the approval is now orphaned. Worse: if the approval is late-matched to a DIFFERENT pending command, the wrong command executes.

**Why it happens:** Distributed timeout + async approval creates a window where the state on different nodes disagrees. The approval message is in flight when the timeout fires.

**Consequences:** Commands that should execute do not (Uday approved but timeout won). Commands that should not execute do (late approval matched to wrong pending command). Uday loses trust in the system ("I approved it and nothing happened").

**Prevention:**
- **Unique approval tokens:** Each command gets a cryptographically random token. The approval must include this exact token. Late approvals for expired tokens are discarded, never matched to other commands.
- **Grace period:** Timeout is 30s, but acceptance window is 35s (5s grace). If approval arrives in the grace window, it is still honored.
- **Acknowledgment to approver:** When a command is approved, send confirmation to Uday: "Command 'restart pod 3' approved and executing." When it times out: "Command 'restart pod 3' expired -- no action taken."
- **Idempotency check:** Before executing an approved command, verify the command is still relevant. "Restart pod 3" should check if pod 3 needs restarting.

**Detection:** Log all approval flow events: command_requested, approval_sent, approval_received, command_executed, command_timed_out. Alert if approval_received occurs after command_timed_out.

**Phase:** Remote Command Execution phase.

**Confidence:** MEDIUM -- the approval flow is a new feature, so the exact timeout value is not yet determined. But timeout races are a universal distributed systems problem.

---

### Pitfall 30: Metrics Memory Leak from Unbounded In-Process Accumulation

**What goes wrong:** Metrics are accumulated in-memory JavaScript objects (Maps, arrays) on James's side for export to Bono. If Bono is disconnected for hours (network outage, VPS restart), the metrics buffer grows without bound. Each heartbeat adds a metrics snapshot (~1KB). After 12 hours at 15s intervals, that is 2,880 snapshots (~3MB). Seems small, but if metrics include per-pod histograms, per-message-type counters with history, and command execution logs, it grows to tens of MB.

**Why it happens:** Metrics are designed for continuous export. When the export target is unavailable, the producer must either drop old metrics or buffer them. Buffering without a cap is a memory leak by definition.

**Consequences:** James's Node.js process grows in memory. Eventually triggers GC pressure, then OOM.

**Prevention:**
- **Ring buffer with fixed size:** Keep the last N metrics snapshots (e.g., 200 = ~50 minutes at 15s intervals). When full, oldest is evicted. On reconnect, Bono gets the last 50 minutes of metrics -- sufficient for dashboarding.
- **Aggregation before buffering:** Do not buffer raw snapshots. Maintain running aggregates: min, max, avg, count. On export, send the aggregates. Reset after successful export. This is O(1) memory regardless of outage duration.
- **Export timeout:** If metrics export has not succeeded in 1 hour, reset the buffer to prevent unbounded growth. Log the data loss.
- **Monitor buffer size:** Expose `metrics_buffer_size` as a metric itself. Alert if it exceeds 80% of the cap.

**Detection:** Periodically log `process.memoryUsage().heapUsed`. If it grows monotonically between GC cycles, there is a leak. Compare with `metrics_buffer_size` to identify if metrics are the source.

**Phase:** Metrics/Observability phase.

**Confidence:** HIGH -- unbounded buffers are a classic memory leak pattern. The existing `#queue` in CommsClient has a `#maxQueueSize` cap (good precedent -- apply the same to metrics).

---

## Minor Pitfalls

---

### Pitfall 31: Sequence Number Reset on Process Restart Causes Replay Confusion

**What goes wrong:** James's comms-link restarts (crash, update, manual restart). The in-memory sequence counter resets to 0. James sends message with seq=1. Bono's `last_received_seq` from James was 47. Bono sees seq=1, which is < 47, and discards it as a duplicate/replay. All messages from James are now silently dropped until James's counter exceeds 47.

**Why it happens:** Sequence numbers are in-memory only. Process restart loses the counter state.

**Prevention:**
- **Persist sequence number to disk.** On each send, write the current seq to a file (or every 10 sends, with the understanding that up to 10 messages may be re-sent on crash recovery).
- **Epoch-based sequences:** `seq = (epoch << 32) | counter`. Epoch increments on every restart (persisted to disk). Bono sees a new epoch and resets its `last_received_seq` for James. Counter within each epoch starts at 0.
- **Reconnect handshake:** On WebSocket connect, both sides exchange their current seq and last_received_seq. This resolves any ambiguity from restarts.

**Detection:** If the receiver sees `incoming_seq < last_received_seq` AND the gap is large (> 10), suspect a restart rather than a duplicate. Request a full state sync.

**Phase:** Message ACK Protocol phase.

**Confidence:** HIGH -- any in-memory counter has this problem on restart.

---

### Pitfall 32: Health Snapshot Size Bloat Saturates WebSocket

**What goes wrong:** Health snapshots are designed to include pod status, deployment state, process lists, disk usage, network stats, and recent log excerpts. A comprehensive snapshot easily reaches 50-100KB. Sent every 30 seconds in the heartbeat, this is 100-200KB/min of overhead on the WebSocket.

**Prevention:**
- **Differential snapshots:** Send full snapshot once on connect, then send only changes (deltas) in subsequent heartbeats. If nothing changed, the delta is empty.
- **Separate health from heartbeat:** Heartbeats are tiny (< 200 bytes) and frequent (15-20s). Health snapshots are large and infrequent (every 5 minutes, or on-demand when Bono requests).
- **Compression:** gzip the snapshot before sending. JSON compresses well (5-10x ratio). Node.js `zlib.gzipSync()` is fast for sub-100KB payloads.
- **Size cap:** If a snapshot exceeds 50KB after compression, truncate optional fields (log excerpts, full process lists) and include a `truncated: true` flag.

**Phase:** Health Snapshots phase.

**Confidence:** MEDIUM -- depends on what data is included. Start minimal, expand as needed.

---

### Pitfall 33: Task Routing Without Request-Response Correlation Causes Mismatched Responses

**What goes wrong:** James sends two task_requests in quick succession: `{ id: "abc", task: "pod_status" }` and `{ id: "def", task: "restart_agent" }`. Bono processes them concurrently. The restart takes 10 seconds; the status takes 1 second. Bono sends `task_response` for status first, then restart. If the response does not include the original request ID, James cannot match responses to requests.

**Why it happens:** Without explicit correlation (request_id in the response), responses are matched by order. But concurrent/async processing means order is not guaranteed.

**Prevention:**
- **Mandatory `request_id` in every task_response.** Copy the `id` from the `task_request` into the `task_response`. The current protocol already has an `id` field (UUID) -- use it for correlation.
- **Request registry:** James maintains a Map of `pending_requests: { [request_id]: { resolve, reject, timeout } }`. On response, look up by `request_id` and resolve/reject. On timeout (30s), reject with timeout error and clean up.
- **Timeout cleanup:** If a request is not resolved in 60s, remove it from the map and log a warning. This prevents the map from growing indefinitely with orphaned requests.

**Phase:** Bidirectional Task Routing phase.

**Confidence:** HIGH -- request-response correlation is a standard pattern. The existing protocol has the `id` field to support it.

---

## Integration Pitfalls (v1.0 + v2.0 Interactions)

---

### Pitfall 34: New ACK Protocol Conflicts with Existing Queue Replay

**What goes wrong:** v1.0 has a queue replay mechanism (CommsClient `#flushQueue`, line 182). v2.0 adds ACK-based replay. On reconnect, BOTH mechanisms fire: the v1.0 queue replays all buffered messages, AND the ACK protocol replays all unacked messages. Every message is sent twice.

**Prevention:**
- **Replace, do not layer.** The v2.0 ACK protocol REPLACES the v1.0 queue replay. Remove `#flushQueue` and `#queue` from CommsClient. The ACK outbox buffer subsumes their function.
- **Migration:** During the transition, keep v1.0 queue as fallback for message types that have not yet been converted to ACK protocol. But this must be temporary and tracked.
- **Test:** Write an integration test that simulates disconnect-reconnect and verifies exactly one delivery of each message.

**Phase:** Message ACK Protocol phase -- must explicitly address the replacement of the existing queue.

**Confidence:** HIGH -- the existing code (CommsClient lines 126-185) has both the send-queue and flush-on-reconnect logic that will conflict.

---

### Pitfall 35: Process Supervision and Remote Command Execution Create a Circular Dependency

**What goes wrong:** The process supervisor monitors and restarts the comms-link. Remote command execution runs through the comms-link. If the comms-link crashes, the supervisor restarts it. But if someone sends a "restart comms-link" remote command, the comms-link must kill itself -- which the supervisor immediately revives. Or: the supervisor itself needs updating via remote command, but it cannot receive commands because it IS the thing being updated.

**Prevention:**
- **The supervisor MUST be independent of the comms-link.** The supervisor is a separate process (the existing watchdog design is correct). It does not receive commands over WebSocket. It only monitors and restarts.
- **"Restart comms-link" command:** The comms-link sends an ACK ("restarting in 5s"), then exits. The supervisor detects the exit and restarts with the new version. The 5s delay allows the ACK to be transmitted.
- **"Update supervisor" command:** This requires a two-phase approach. (1) Download new supervisor code to a staging location. (2) Comms-link tells the supervisor to self-replace via a local IPC mechanism (e.g., a file flag that the supervisor checks on its next poll cycle). (3) Supervisor replaces itself and restarts.
- **Never allow remote commands to kill the supervisor directly.** The supervisor is the last line of defense.

**Phase:** This spans Process Supervision + Remote Command Execution. Design both phases with this interaction in mind.

**Confidence:** HIGH -- circular dependency between supervisor and supervised process is a classic ops architecture mistake.

---

## Phase-Specific Warnings

| Phase Topic | Likely Pitfall | Severity | Mitigation |
|-------------|---------------|----------|------------|
| **Process Supervision** | Duplicate instances from liveness-only check (#23) | Critical | Health check + PID lockfile |
| **Process Supervision** | Handle leak prevents clean restart (#28) | Moderate | Tree kill + port verification + 3s wait |
| **Process Supervision** | Supervisor-comms circular dependency (#35) | Critical | Independent supervisor process, no remote control |
| **Message ACK Protocol** | ACK storm infinite loop (#19) | Critical | ACKs never ACKed, control message type set |
| **Message ACK Protocol** | Timestamp-as-sequence antipattern (#20) | Critical | Integer counter, persisted to disk |
| **Message ACK Protocol** | Duplicate delivery on reconnect (#24) | Moderate | UUID dedup + idempotent handlers |
| **Message ACK Protocol** | Sequence reset on restart (#31) | Minor | Epoch-based sequences, reconnect handshake |
| **Message ACK Protocol** | Conflicts with v1.0 queue replay (#34) | Moderate | Replace, do not layer |
| **Transactional Queue** | File corruption on crash (#21) | Critical | WAL pattern, not full-file rewrite |
| **Transactional Queue** | NTFS locking breaks atomic rename (#25) | Moderate | WAL avoids rename, Defender exclusion |
| **Remote Command Execution** | Shell injection via unsafe APIs (#18) | Critical | execFile + command allowlist + no shell |
| **Remote Command Execution** | Environment variable leak (#22) | Critical | Sanitized env, output filtering |
| **Remote Command Execution** | Approval timeout race (#29) | Moderate | Unique tokens, grace period, idempotency |
| **Remote Command Execution** | Circular dep with supervisor (#35) | Critical | Independent supervisor, staged restart |
| **Health Snapshots** | wmic removed in Win 11 25H2 (#27) | Moderate | Node.js os module + Get-CimInstance |
| **Health Snapshots** | Snapshot size bloat (#32) | Minor | Differential snapshots, separate from heartbeat |
| **Metrics/Observability** | Cardinality explosion (#26) | Moderate | Bounded labels only, cardinality budget < 500 |
| **Metrics/Observability** | Unbounded metrics buffer (#30) | Moderate | Ring buffer with fixed cap |
| **Task Routing** | Mismatched request-response (#33) | Moderate | Mandatory request_id correlation |

---

## Pitfall Priority for Implementation Order

The pitfalls suggest a specific implementation order to avoid cascading failures:

1. **Process Supervision first** -- without reliable supervision, nothing else matters (15-hour outage lesson)
2. **Message ACK Protocol second** -- the queue and remote execution depend on reliable delivery
3. **Transactional Queue third** -- replaces INBOX.md, required before task routing
4. **Remote Command Execution fourth** -- requires queue + ACK + supervision to be solid
5. **Health Snapshots + Metrics last** -- observability is valuable but not load-bearing

---

## Sources

### HIGH Confidence (Official Documentation / Direct Evidence)
- [Node.js child_process documentation](https://nodejs.org/api/child_process.html) -- execFile vs shell-based APIs, handle inheritance, detached behavior
- [Microsoft WMIC removal announcement](https://techcommunity.microsoft.com/blog/windows-itpro-blog/wmi-command-line-wmic-utility-deprecation-next-steps/4039242) -- confirmed removal in 25H2
- [Microsoft WMIC removal support article](https://support.microsoft.com/en-us/topic/windows-management-instrumentation-command-line-wmic-removal-from-windows-e9e83c7f-4992-477f-ba1d-96f694b8665d)
- [npm CLI issue #9021](https://github.com/npm/cli/issues/9021) -- NTFS rename failures with write-file-atomic
- [Node.js detached process issues](https://github.com/nodejs/node/issues/51018) -- Windows handle inheritance problems
- Existing v1.0 code: CommsClient queue replay (comms-client.js lines 126-185), Watchdog detection (watchdog.js lines 25-45)
- PROJECT.md production incidents: 15-hour outage, wmic deprecation, INBOX.md races

### MEDIUM Confidence (Verified with Multiple Sources)
- [Node.js command injection prevention](https://www.nodejs-security.com/book/command-injection) -- Liran Tal's security guide
- [Auth0 command injection prevention](https://auth0.com/blog/preventing-command-injection-attacks-in-node-js-apps/)
- [Dan Luu "Files are hard"](https://danluu.com/file-consistency/) -- file consistency pitfalls across OS/FS
- [Dan Luu fsyncgate](https://danluu.com/fsyncgate/) -- fsync error handling
- [Grafana cardinality management](https://grafana.com/blog/how-to-manage-high-cardinality-metrics-in-prometheus-and-kubernetes/)
- [Prometheus best practices](https://betterstack.com/community/guides/monitoring/prometheus-best-practices/)
- [ClickHouse high-cardinality analysis](https://clickhouse.com/resources/engineering/high-cardinality-slow-observability-challenge)
- [CVE-2025-68613 n8n expression injection](https://www.resecurity.com/blog/article/cve-2025-68613-remote-code-execution-via-expression-injection-in-n8n-2) -- real-world RCE via code injection
- [Confluent exactly-once semantics](https://www.confluent.io/blog/exactly-once-semantics-are-possible-heres-how-apache-kafka-does-it/)
- [Brave New Geek distributed messaging tradeoffs](https://bravenewgeek.com/what-you-want-is-what-you-dont-understanding-trade-offs-in-distributed-messaging/)

### LOW Confidence (WebSearch Only / Training Data)
- Windows timer tick resolution (15.6ms) for Date.now() -- commonly cited but not verified against current Node.js version
- Windows Job Objects as process containment mechanism -- known Windows API but no Node.js library verified
