# Pitfalls Research

**Domain:** Adding crash-diagnosing watchdog to existing Rust binary on Windows — RC Sentry AI Debugger (v11.2)
**Researched:** 2026-03-21
**Confidence:** HIGH — drawn from direct codebase knowledge (rc-sentry v11.0, rc-agent architecture), Windows process management internals, EAC/iRacing anti-cheat documentation, and verified community sources

---

## Critical Pitfalls

### Pitfall 1: Anti-Cheat Ban From Process Inspection — The Line Is Not Where You Think It Is

**What goes wrong:**
F1 25 uses EasyAntiCheat EOS (Epic Games variant, kernel driver `easyanticheat_eos.sys`). iRacing uses its own EAC variant (migrated from Kamu EAC to Epic EOS in recent seasons). Both run at kernel level and perform continuous scanning of the process list, memory regions, and system drivers. The mistake is assuming the line between "safe" and "banned" is at OpenProcess/ReadProcessMemory. It is not — EAC's scan is aggressive and flag-then-investigate: activities like iterating the process list with `CreateToolhelp32Snapshot`, calling `QueryFullProcessImageName()` on game-owned PIDs, or reading the game's working set size via `GetProcessMemoryInfo()` on the game PID can trigger heuristic flags. ASUS Aura Sync (a legitimate background app) was flagged by EAC for using a driver that EAC interpreted as suspicious. The rc-sentry name and behavior profile (external binary, polling, process enumeration) fits the same threat signature as a cheat tool's loader.

**Why it happens:**
Developers assume that because rc-sentry does not inject code or modify game memory, EAC will not care. EAC does not know rc-sentry's intent — it classifies behavior, not intent. A sentry binary that opens handles to game processes, reads their memory size, or even sits adjacent to an active EAC session with unusual API call patterns can trigger a report.

**How to avoid:**
The constraint already stated in PROJECT.md is the correct answer and must be treated as a hard limit with no exceptions: no process inspection, no debug APIs, no `OpenProcess()` on game PIDs, no enumeration of handles belonging to the game process. The sole safe mechanism for crash detection is the health endpoint poll — `GET localhost:8090/health` returning 200 or timing out. Do not add any "helpful" supplementary check that touches the game process. If the health endpoint goes silent, that is the only crash signal. Specifically:
- Never call `OpenProcess()` with `PROCESS_VM_READ` or `PROCESS_QUERY_INFORMATION` while a game is running.
- Never call `CreateToolhelp32Snapshot()` with `TH32CS_SNAPMODULE` (module enumeration flags EAC kernel scanner).
- `TH32CS_SNAPPROCESS` for process list enumeration is lower risk than module enumeration, but still avoid querying specific game PIDs.
- rc-sentry's own HTTP polling loop must not vary its behavior based on which game is running — consistent behavior is harder to flag than conditional behavior.

**Warning signs:**
- rc-sentry code has a `find_process_by_name()` or equivalent function that is called while games may be running.
- Log lines show "checking if game PID is running" distinct from the health endpoint poll.
- A new "supplementary crash detection" mechanism is added that uses any WinAPI other than TCP socket connect.

**Phase to address:**
Phase 1 (Health Polling Watchdog) — the API surface is defined here. Document the anti-cheat constraint explicitly in the module's doc comment. No process inspection path should exist in the code even as dead code.

---

### Pitfall 2: Watchdog Restart Loop — Fixing the Crash That Keeps Crashing

**What goes wrong:**
rc-sentry detects rc-agent crash → runs Tier 1 fixes (clear sockets, kill zombies, repair config) → restarts rc-agent → rc-agent crashes again within 30 seconds from the same cause (Tier 1 fixes did not address it) → rc-sentry detects crash again → applies same fixes → restarts again → crash loop at full speed. With a 5-second health poll interval and no restart rate limiting, this loop runs at ~10 restarts per minute. Consequences: (1) if a billing session is active, it is torn down and restarted repeatedly, creating ghost billing records; (2) Ollama is queried every cycle with the same crash context → James .27:11434 is hammered with requests; (3) the fleet dashboard shows the pod as perpetually "restarting" with no actionable signal for staff.

**Why it happens:**
The watchdog is designed to fix and restart. Without explicit restart storm detection, "fix failed, try again" and "infinite restart loop" look identical to the watchdog. The backoff the existing rc-agent watchdog uses (EscalatingBackoff in rc-common) must be carried forward to rc-sentry's restart logic — but developers often write the new crash-restart path without importing the existing backoff infra.

**How to avoid:**
Reuse the existing `EscalatingBackoff` from rc-common (it is already there from v1.0 WD-01/WD-02). Apply it to the restart decision in rc-sentry: after each restart, back off before the next attempt (30s → 60s → 120s → cap at 300s). Track restart count per session. At 3 restarts within 5 minutes, escalate to "block pod" state and alert staff instead of continuing to restart. Reset the backoff counter only when rc-agent has been healthy for at least 2 minutes (the same post-restart verification pattern from WD-03/WD-04).

The crash pattern memory (`debug-memory.json`) must include a `last_fix_applied` and `fix_success_count` field per pattern. If the same fix is applied 3 times without recovery, the pattern is classified as "fix ineffective" and Tier 3 Ollama is invoked regardless of pattern match.

**Warning signs:**
- The restart path in rc-sentry does not import or reference `EscalatingBackoff`.
- restart decision is made immediately after crash detection with no delay.
- Fleet dashboard shows pod cycling between "crashed" and "restarting" states at regular intervals.
- Ollama query log shows the same crash signature queried multiple times per minute.

**Phase to address:**
Phase 2 (Tier 1 Deterministic Fixes + Restart Logic) — backoff is a first-class requirement, not an improvement to add later.

---

### Pitfall 3: Crash Log Is Gone When You Read It — Timing and File Lifecycle

**What goes wrong:**
rc-agent crashes. rc-sentry detects the crash via health poll timeout. rc-sentry calls Tier 1 fixes, which include "kill zombie processes" — one of which may be the previous rc-agent process that still holds an open file handle to the startup log or stderr capture file. `TerminateProcess()` on the zombie releases the handle and flushes the file buffer. If rc-sentry reads the log before the handle is released (i.e., before killing the zombie), it reads a truncated or still-locked file. On Windows, a file held open with `GENERIC_WRITE | FILE_FLAG_NO_BUFFERING` cannot be read by another process even with `GENERIC_READ` unless the original opener used `FILE_SHARE_READ`.

**Why it happens:**
The natural ordering is: detect crash → read logs → apply fixes → restart. But the log may be held by the crashed process (which is still technically alive as a zombie). The correct ordering for log capture is: detect crash → kill zombie → wait for handle release → read logs → apply other fixes → restart.

**How to avoid:**
Log reading must happen after zombie termination. The Tier 1 sequence must be:
1. Kill zombie rc-agent process (with handle release).
2. Wait 500ms for OS to flush and release file handles.
3. Read startup_log, stderr capture, and panic output files.
4. Apply remaining fixes (socket cleanup, config repair, shader cache clear).
5. Restart.

For the stderr capture specifically: rc-agent's stderr must be redirected to a file with `FILE_SHARE_READ` in the spawn call so rc-sentry can read it without waiting for zombie termination. Document this requirement in rc-sentry's process spawn code.

**Warning signs:**
- Log parsing happens before zombie kill in the Tier 1 fix sequence.
- rc-sentry logs "could not read startup_log: access denied" or "file locked" errors.
- rc-sentry consistently reads empty or truncated logs despite rc-agent clearly logging before crash.

**Phase to address:**
Phase 3 (Post-Crash Log Analysis) — the file access ordering and `FILE_SHARE_READ` requirement are implementation details that must be in the spec, not discovered during testing.

---

### Pitfall 4: Pattern Memory JSON Corruption From Concurrent Access

**What goes wrong:**
rc-sentry runs as a long-lived service with a health polling loop (every 5s) and a separate Axum HTTP server serving existing endpoints. The debug-memory.json file is read at crash detection time (to check for known patterns) and written after each Ollama query result (to store new patterns). If a concurrent HTTP request hits a `/debug-memory` endpoint while a crash analysis write is in progress, two writes can interleave, producing a partial JSON file that fails to deserialize on the next read. The next crash falls through to Ollama even for a known pattern. In the worst case, the JSON file is left at 0 bytes due to a truncate-before-write that interrupted.

**Why it happens:**
`tokio::fs::write()` is not atomic on Windows. It truncates the file then writes the new content. If the process crashes, is killed, or a second writer races between truncate and write, the file is empty. This is a known failure mode for any "write JSON file atomically" pattern.

**How to avoid:**
Never write to `debug-memory.json` directly. Write to `debug-memory.json.tmp`, then call `std::fs::rename()` (atomic on the same volume on Windows) to replace the old file. In Rust on Windows, `std::fs::rename()` maps to `MoveFileExW` with `MOVEFILE_REPLACE_EXISTING`, which is atomic at the filesystem level. Protect all reads and writes behind a `tokio::sync::RwLock<DebugMemory>` held in AppState — the same pattern already used for URL switching in rc-sentry v11.0. Memory-first: load into the RwLock at startup, write-through to disk on mutation.

**Warning signs:**
- `debug-memory.json` is read from disk on every crash event (no in-memory cache behind a RwLock).
- Write path uses `tokio::fs::write()` or `std::fs::write()` directly without a `.tmp` + rename sequence.
- No lock guards the memory file access path.
- rc-sentry logs "failed to parse debug-memory.json: unexpected end of file" after a crash during analysis.

**Phase to address:**
Phase 4 (Crash Pattern Memory) — atomic write and RwLock are design requirements, not implementation details to figure out during coding.

---

### Pitfall 5: Ollama Cold Start Blocks the Restart Sequence

**What goes wrong:**
Ollama on James .27:11434 serves `qwen3:0.6b`. The model is kept in memory while active but unloads after an idle period (default 5-minute `OLLAMA_KEEP_ALIVE`). If the first crash of the day triggers an Ollama query and the model is not loaded, Ollama must load the model before responding. Model load time for qwen3:0.6b on the RTX 4070 is 10-30 seconds. The rc-sentry Tier 3 query is made from within the post-crash analysis path that is blocking the restart sequence. The pod stays down for 30+ seconds while Ollama cold-starts, even though the restart decision could have been made without an LLM.

The second failure mode: if Ollama is unreachable (James .27 is offline, network blip, Ollama service not running), `reqwest` with no timeout set will hang indefinitely. The health polling loop for rc-agent is blocked behind the Ollama request, meaning crash detection for other pods stops while one Ollama query hangs.

**Why it happens:**
Developers set a timeout for the HTTP client but forget the Ollama endpoint is on a separate machine. Connection timeout and read timeout are different: `reqwest::ClientBuilder::connect_timeout()` covers the TCP handshake but `reqwest::ClientBuilder::timeout()` is the total request time. Forgetting the total timeout means a connected but slow Ollama response hangs the caller indefinitely.

**How to avoid:**
Tier 3 Ollama query must run in a detached `tokio::spawn()` that does not block the restart sequence. The restart decision and the Ollama query must be decoupled: restart immediately after Tier 1/2 fail, then fire the Ollama query asynchronously, then update debug-memory.json when the result arrives. The Ollama result is used to annotate the next crash, not to gate the current restart.

Set both timeouts on the reqwest client for Ollama:
- `connect_timeout`: 5 seconds (if .27 is unreachable, fail fast).
- `timeout`: 45 seconds total (covers model load cold start up to 30s + inference up to 15s).

If the Ollama query times out or returns a network error, log the failure, increment a `ollama_failure_count` counter, and proceed as if no LLM result was available. Do not retry the same query in a loop.

**Warning signs:**
- Ollama query is `await`ed in the same future chain as the restart call.
- `reqwest::Client` for Ollama has no `timeout()` set.
- Pod restart latency in logs is 30+ seconds on the first crash of the day.
- Crash analysis for pod 2 is blocked while pod 1's Ollama query is pending.

**Phase to address:**
Phase 5 (Tier 3 Ollama Integration) — decoupled async Ollama path and dual timeout are defined before a single line of Ollama query code is written.

---

### Pitfall 6: Windows Process Kill Leaves Stale TCP Sockets — Tier 1 Fix Incomplete

**What goes wrong:**
rc-agent holds open TCP sockets: the WebSocket connection to racecontrol server, the HTTP server on port 8090, and potentially CLOSE_WAIT sockets from previous sessions (the CLOSE_WAIT leak was a known issue patched in v8.0, but crash conditions can bypass the cleanup path). When rc-agent crashes, these sockets enter TCP TIME_WAIT or CLOSE_WAIT. Tier 1 fix "clean stale sockets" must handle this correctly — but the common mistake is running `taskkill /IM rc-agent.exe /F` and then immediately attempting to start rc-agent. Port 8090 is still bound by the OS for the TCP TIME_WAIT period (default 2 minutes on Windows, configurable via `TcpTimedWaitDelay` registry key). rc-agent fails to bind port 8090 at startup, logs "address already in use", and rc-sentry interprets this as another crash.

**Why it happens:**
taskkill kills the process but does not flush the TCP stack. On Windows, `SO_REUSEADDR` semantics differ from Linux — it does not override TIME_WAIT sockets in Windows the way `SO_REUSEPORT` would. rc-agent's Axum listener binding without `SO_REUSEADDR` will fail if the previous port is in TIME_WAIT.

**How to avoid:**
Tier 1 socket cleanup must do more than kill the process. After killing the zombie rc-agent:
1. Use `netstat -ano` (or the WinAPI equivalent `GetExtendedTcpTable`) to check whether port 8090 is still in any state (TIME_WAIT, CLOSE_WAIT, LISTEN).
2. If still bound, wait up to 10 seconds polling every 1 second for the port to free.
3. If still bound after 10 seconds, attempt `SetTcpEntry()` to force close lingering CLOSE_WAIT sockets before restart.
4. Log the port state at each step so the crash analysis has visibility into socket lifecycle.

rc-agent's Axum binding should already use `SO_REUSEADDR` (the existing socket hygiene work from v8.0). Verify it does; if not, this is a separate fix required before rc-sentry socket cleanup is reliable.

**Warning signs:**
- Tier 1 fix sequence does not include a port readiness check after process kill.
- rc-agent startup logs show "address already in use: port 8090" after a Tier 1 fix.
- rc-sentry counts two crash events for what was one actual crash (restart failed, counted as second crash).

**Phase to address:**
Phase 2 (Tier 1 Deterministic Fixes) — socket cleanup is a multi-step sequence, not a one-line `taskkill`. The port readiness check is part of the fix, not optional.

---

### Pitfall 7: Panic Output Is Not Where You Think It Is on Windows

**What goes wrong:**
On Windows, a Rust binary built without `#![windows_subsystem = "windows"]` sends panic output to stderr. rc-agent is a console subsystem binary (service), so panic output goes to stderr. But when started via the Windows Service framework (HKLM Run → `start-rcagent.bat`), stderr is not connected to a file unless the bat script explicitly redirects it: `rc-agent.exe 2>>C:\RacingPoint\rc-agent-stderr.log`. The panic output is silently discarded. rc-sentry finds the log file empty or absent and cannot determine the crash cause. The crash is misclassified as "unknown" and Ollama is queried with no crash context.

A second subtlety: `RUST_BACKTRACE=1` must be set in the environment of the rc-agent process for backtraces to appear in panic output. If the bat script does not set this environment variable, the panic message contains only the panic location with no call stack, which is not enough for Tier 2 memory matching on complex crashes.

**Why it happens:**
Developers test rc-agent interactively (from a terminal where stderr is visible) and never notice the bat script does not capture it. The missing redirect is invisible during development.

**How to avoid:**
The `start-rcagent.bat` must include:
```
set RUST_BACKTRACE=1
rc-agent.exe >> C:\RacingPoint\rc-agent.log 2>> C:\RacingPoint\rc-agent-stderr.log
```
rc-sentry's log analysis must check the stderr log file path as the primary panic source, not rc-agent's tracing log (which only captures events that made it through the tracing subscriber before the panic unwind). Both paths should be checked:
1. `C:\RacingPoint\rc-agent-stderr.log` — tail last 4KB for panic message and backtrace.
2. `C:\RacingPoint\rc-agent.log` — tail last 8KB for tracing events leading up to the crash.

The bat file must be updated as part of the rc-sentry AI Debugger implementation. This is a prerequisite, not a follow-up.

**Warning signs:**
- `start-rcagent.bat` does not have `2>>` stderr redirection.
- `RUST_BACKTRACE` is not set in the bat script environment.
- rc-sentry log analysis consistently finds empty panic output and falls through to Ollama for every crash.
- Panic output visible when running rc-agent from a terminal but never seen in log files.

**Phase to address:**
Phase 3 (Post-Crash Log Analysis) — bat file update is a prerequisite task listed before the log parsing code is written.

---

### Pitfall 8: Tier 1 Fixes Applied When rc-agent Is Still Running — Race Between Poll and Fix

**What goes wrong:**
Health endpoint poll times out at T=0 (poll sent, no response in 3 seconds). rc-sentry classifies this as a crash. rc-sentry begins Tier 1 fix sequence: kills zombie rc-agent process, clears sockets, repairs config. Meanwhile, rc-agent was not crashed — it was briefly unresponsive due to a GC pause, a large tokio task blocking the runtime, or a temporary network blip between rc-sentry and port 8090. rc-agent recovers and responds to the next poll at T=5. rc-sentry has already killed it and is mid-way through applying Tier 1 fixes to a healthy process. The active billing session is interrupted.

**Why it happens:**
A single poll timeout is an unreliable crash signal. The health endpoint poll has a 3-second timeout (reasonable for normal operation), but transient load spikes on a gaming pod (shader compilation, audio driver stalls, FFB firmware communication) can cause the HTTP server to be unresponsive for 3-8 seconds without a true crash.

**How to avoid:**
Require N consecutive poll failures before classifying as crashed. The PROJECT.md spec says "every 5s" — this means: poll every 5 seconds, declare crashed only after 2 consecutive timeouts (10 seconds of silence) before initiating any Tier 1 action. Three consecutive timeouts (15 seconds) before process kill. This gives transient hangs time to resolve without triggering the fix sequence. The existing EscalatingBackoff pattern from rc-common captures this concept — apply the same hysteresis model used in v10.0 health monitoring (2 failures before state transition).

**Warning signs:**
- Crash detection logic fires fix sequence after a single timeout.
- rc-sentry logs show Tier 1 fixes applied to a pod where rc-agent was recovered and running within the same log minute.
- Billing sessions show spurious interruptions on pods that have no rc-agent crash in their own logs.

**Phase to address:**
Phase 1 (Health Polling Watchdog) — the crash classification threshold (N consecutive failures) is defined in the watchdog spec before implementation.

---

### Pitfall 9: Adding HTTP Endpoints to rc-sentry Breaks the 4-Slot Concurrency Cap

**What goes wrong:**
rc-sentry v11.0 has a hard 4-slot concurrency cap on exec endpoints (the `concurrency_guard` with a semaphore). The new crash analysis endpoints added by v11.2 (e.g., `/debug/status`, `/debug/memory`, `/crash-report` relay to server) consume slots from the same semaphore if added naively as regular Axum handlers. During a fleet-wide event (all 8 pods report crashes simultaneously), the racecontrol server may invoke rc-sentry's existing endpoints and the new crash endpoints at the same time, exhausting the 4-slot cap and causing the health polling loop itself to be blocked waiting for a slot.

**Why it happens:**
The 4-slot cap was designed for the exec endpoints (which spawn external processes and have high resource cost). Informational endpoints like `/debug/status` do not need that guard but get lumped in if the developer applies the middleware at the router level rather than per-route.

**How to avoid:**
Apply the concurrency semaphore only to endpoints that invoke external processes (`/exec`, `/files` with write operations). Read-only endpoints (`/health`, `/version`, `/debug/status`, `/debug/memory`) must be outside the semaphore. The health polling loop's internal poll (localhost HTTP call) must never compete with the semaphore-gated endpoints. The Axum router structure should be:
```
Router::new()
  .route("/health", get(health_handler))       // no semaphore
  .route("/debug/status", get(debug_status))   // no semaphore
  .route("/exec", post(exec_handler           // semaphore-gated
      .layer(semaphore_middleware)))
```

**Warning signs:**
- Concurrency semaphore middleware applied at the top-level router instead of per-route.
- `/debug/status` endpoint acquires an exec slot during a GET request.
- During a crash storm test, health polling latency increases above the 3-second timeout threshold.

**Phase to address:**
Phase 6 (Fleet API Reporting) — endpoint routing structure reviewed before new endpoints are added. The concurrency guard placement is explicitly documented in the endpoint spec.

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Single poll timeout → immediate crash classification | Simpler state machine | Spurious fix sequences on transient hangs; billing session interruptions | Never — N-consecutive threshold is required from day one |
| Blocking `await` on Ollama query in restart path | Simpler linear code | Pod stays down 30+ seconds on model cold start; cascading delay if .27 is slow | Never — Ollama must be fire-and-forget from the restart path |
| Write `debug-memory.json` directly without atomic rename | Simpler file write | Partial JSON on interrupt = zero-byte file = all patterns lost | Never — atomic write is a one-line change with critical correctness benefit |
| Skip stderr redirect in bat file "for now" | Faster first deploy | All panic output silently discarded; crash analysis always falls through to Ollama | Never — bat file update is a prerequisite |
| Apply concurrency semaphore to all rc-sentry routes | Consistent middleware | Informational endpoints block under load; health poll competes with exec slots | Never — semaphore is resource-gated, not blanket |
| Add a `find_by_name_and_kill()` helper for "convenience" | Easier zombie cleanup | Single function that can be misused to inspect game PIDs → EAC risk | Only if constrained to rc-agent PID exclusively, never game PIDs |

---

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| Ollama `:11434` over LAN | Set only `connect_timeout`, not total `timeout` | Set both: `connect_timeout(5s)` and `timeout(45s)` via `reqwest::ClientBuilder` |
| Ollama model cold start | Treat 30s+ response as a timeout error and retry | Model is loading; extend timeout to 60s, do not retry during load, fire-and-forget from restart path |
| Windows stderr capture | Assume stderr goes to a log file | Explicitly redirect with `2>>` in `start-rcagent.bat`; verify redirect exists before implementing log parser |
| `taskkill /IM rc-agent.exe /F` | Assume port 8090 is free immediately after kill | Port stays in TIME_WAIT; poll for port free with 10s timeout before restart |
| `debug-memory.json` first read | Open the file directly on crash event | Lock not held, file may be mid-write from previous analysis; always read through the in-memory RwLock |
| rc-sentry existing 6 endpoints | Add new crash endpoints to same Axum router with same middleware stack | Audit semaphore placement; crash/debug endpoints must not consume exec semaphore slots |
| Fleet API relay to racecontrol server | Open a new reqwest client per crash report | Reuse a shared `reqwest::Client` stored in AppState; connection pool keeps LAN connections warm |

---

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Health poll loop awaits Ollama query | Pod restart delayed 30-60s; other pods' crash detection stalls | Spawn Ollama query as detached task; restart path is independent | First crash on any pod after Ollama idle timeout |
| Tailing log files without size limit | On a heavily logging pod, reading 10MB log file to find last 4KB of panic output | Always read from file end: seek to `file_len - 4096`, read backward. Never read entire file | After 24h of continuous rc-agent operation |
| `debug-memory.json` grows unbounded | Slow startup as JSON deserialization time grows; RwLock held longer during parse | Cap at 100 patterns; evict least-recently-used entries when full | After ~50 unique crash patterns accumulate |
| Polling every 5s unconditionally when game is active | 5s poll during active session generates background network noise visible to EAC as unusual traffic | Poll interval remains 5s — but ensure HTTP client has keepalive so each poll reuses the TCP connection rather than opening a new one | Not a hard break, but new TCP connections per poll look more like scanning than persistent monitoring |
| Log parsing on each crash event reads all three log files sequentially | 3 file reads block the crash analysis path | Read all three files concurrently with `tokio::join!` | On slow disk I/O with large log files |

---

## Security Mistakes

| Mistake | Risk | Prevention |
|---------|------|------------|
| Logging full crash context including config values | racecontrol.toml may contain API keys, JWT secrets, or encryption keys; log files are accessible to pod user | Redact config values from crash reports before logging: replace secret fields with `[REDACTED]` |
| Sending full crash log to fleet API without size limit | A crafted or very large log file could produce an oversized API request | Truncate crash report payload at 16KB before sending to server |
| Ollama query contains full log file contents | Log file may contain customer session data (phone hash, billing amount) | Sanitize log content before Ollama query: strip any field matching phone number patterns or INR/credit amounts |
| `debug-memory.json` stored in world-readable location | Pattern memory reveals which crash patterns the system knows how to exploit | Store in `C:\RacingPoint\` (same dir as rc-agent.toml), not in a temp or user directory |

---

## "Looks Done But Isn't" Checklist

- [ ] **Stderr capture:** Verify `start-rcagent.bat` has `2>>` redirect before any log parsing test runs. Open the log file after a test crash and confirm panic output is present.
- [ ] **N-consecutive threshold:** Kill rc-agent, confirm rc-sentry waits for 2 consecutive poll failures before entering fix sequence, not 1.
- [ ] **Atomic JSON write:** Corrupt `debug-memory.json` mid-write in a test. Confirm next read succeeds from the last valid snapshot, not a zero-byte file.
- [ ] **Ollama timeout:** Take .27 offline, trigger a crash. Confirm pod restarts within 15 seconds without waiting for Ollama. Confirm Ollama failure is logged but does not block restart.
- [ ] **Restart backoff:** Crash rc-agent 4 times in 5 minutes. Confirm the 4th restart is delayed (backoff active) and a staff alert fires instead of another restart attempt.
- [ ] **Port readiness:** After Tier 1 process kill, confirm Tier 1 fix waits for port 8090 to clear before restart. Induce TIME_WAIT manually and verify the wait logic triggers.
- [ ] **No process inspection during game:** Start F1 25, let EAC initialize, then trigger rc-agent crash. Confirm rc-sentry detects via health poll only. No `OpenProcess()` on any game PID in logs.
- [ ] **Concurrency semaphore:** Under load, confirm `/debug/status` responds while all 4 exec semaphore slots are occupied.
- [ ] **Pattern memory hit:** Store a known pattern in `debug-memory.json`. Trigger a crash that matches the pattern. Confirm Tier 2 memory match fires and Ollama is NOT queried.
- [ ] **Crash report to server:** Trigger a crash, confirm the fleet API receives a crash report with pod number, crash time, log excerpt, and fix applied — all within 30 seconds of crash detection.

---

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Restart loop at full speed | MEDIUM | Push config update: set `max_restarts_per_window = 0` (disable auto-restart) on affected pod; investigate root cause manually; re-enable when fixed |
| debug-memory.json zero bytes after crash-during-write | LOW | Delete the file; rc-sentry recreates empty on next start; all learned patterns lost but no functional breakage |
| Ollama query hanging; crash analysis blocked | LOW | Restart rc-sentry on affected pod to clear the hung reqwest connection; fix: add total timeout to reqwest client |
| bat file missing stderr redirect; all panics lost | MEDIUM | Add redirect to bat file, redeploy via fleet exec endpoint; historical crash cause for the current incident is unrecoverable — must re-trigger or wait for next crash |
| EAC false flag from process inspection | HIGH | Stop rc-sentry immediately; do not restart until the offending API call is identified and removed from the binary; check EAC ban status on iRacing account linked to pod; contact EAC support with timeline if ban issued |
| Tier 1 fix applied to healthy rc-agent (false crash) | MEDIUM | rc-agent self-restarts via Windows Service; check billing session integrity on affected pod; adjust N-consecutive threshold upward if pod has sustained high load |

---

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| Anti-cheat ban from process inspection | Phase 1 — Health Polling | No `OpenProcess()` on game PIDs in codebase; explicit doc comment in health polling module |
| Restart loop without backoff | Phase 2 — Tier 1 Fixes + Restart Logic | 4 crashes in 5 min triggers backoff and staff alert, not 4th restart |
| Log timing race (zombie holds file) | Phase 3 — Post-Crash Log Analysis | Fix sequence orders: kill zombie → wait 500ms → read logs |
| Pattern memory JSON corruption | Phase 4 — Crash Pattern Memory | Atomic write (tmp + rename) and RwLock in AppState |
| Ollama cold start blocks restart | Phase 5 — Tier 3 Ollama Integration | Ollama query is detached; pod restarts in under 10s regardless of Ollama state |
| Stale TCP sockets block restart | Phase 2 — Tier 1 Fixes | Port readiness check in fix sequence; Tier 1 test with induced TIME_WAIT |
| Panic output not captured | Phase 3 — Post-Crash Log Analysis | bat file update is prerequisite task in phase spec |
| Single poll timeout triggers false crash | Phase 1 — Health Polling | N-consecutive threshold defined and tested before implementation |
| Concurrency semaphore on debug endpoints | Phase 6 — Fleet API Reporting | Semaphore placement audited when new endpoints are added |

---

## Sources

- Direct codebase knowledge: rc-sentry v11.0 (6 endpoints, 4-slot semaphore, TCP read fix), rc-common EscalatingBackoff (WD-01/WD-02/WD-03/WD-04), v10.0 health monitoring hysteresis FSM, v8.0 CLOSE_WAIT socket leak fix
- `PROJECT.md`: v11.2 milestone spec, anti-cheat constraints, Ollama at James .27:11434, existing 4-tier debug order
- `CLAUDE.md`: start-rcagent.bat architecture, HKLM Run key, Session 1 GUI requirement, port 8090 binding, Windows Service context
- [EasyAntiCheat kernel driver incompatibility with Kernel-Mode Hardware-Enforced Stack Protection](https://learn.microsoft.com/en-us/answers/questions/3962392/easy-anti-cheat-driver-incompatible-with-kernel-mo) — Microsoft Learn (HIGH confidence — confirms EAC kernel mode scope)
- [EasyAntiCheat EOS causes Kernel Panic](https://learn.microsoft.com/en-us/answers/questions/3894697/easyanticheat-eos-causes-kernel-panic) — Microsoft Learn (HIGH confidence — confirms easyanticheat_eos.sys version 6.1, January 2025)
- [EasyAntiCheat kernel scan loop bug report](https://forums.ea.com/discussions/apex-legends-technical-issues-en/bug-report-%E2%80%93-easyanticheat-eos-sys-kernel-scan-loop/13034346) — EA Forums (MEDIUM confidence — confirms continuous scanning behavior)
- [iRacing anti-cheat migration to EOS EAC](https://support.iracing.com/support/solutions/articles/31000173103-anticheat-not-installed-uninstalling-eac-and-installing-eos-) — iRacing support (HIGH confidence — confirms iRacing uses EOS variant of EAC)
- [Anti-cheat false positives: ASUS Aura Sync flagged by EAC](https://gamespace.com/all-articles/news/easy-anti-cheat-download-install/) — GameSpace (MEDIUM confidence — confirms legitimate background apps trigger EAC)
- [Rust std::process::Child::kill fails on Windows with stale handles](https://github.com/rust-lang/rust/issues/112423) — rust-lang/rust GitHub (HIGH confidence — confirms Windows handle lifecycle issue)
- [taskkill /T /F required for child process tree on Windows](https://www.techbloat.com/kill-stubborn-programs-and-processes-with-taskkill/) — TechBloat (MEDIUM confidence — confirms tree kill required)
- [Ollama KEEP_ALIVE and model loading latency](https://markaicode.com/troubleshooting-ollama-tool-execution-timeouts/) — Markaicode (MEDIUM confidence — confirms 30-90s cold start for model reload)
- [reqwest timeout configuration](https://webscraping.ai/faq/reqwest/how-can-i-set-a-request-in-reqwest) — WebScraping.AI (HIGH confidence — connect_timeout vs total timeout distinction)
- [Rust panic hook stderr on Windows windowed subsystem](https://github.com/rust-lang/rust/issues/25643) — rust-lang/rust GitHub (HIGH confidence — confirms stderr behavior on Windows)
- [RUST_BACKTRACE required for stack trace in panic output](https://doc.rust-lang.org/book/ch09-01-unrecoverable-errors-with-panic.html) — Rust Book (HIGH confidence)

---
*Pitfalls research for: RC Sentry AI Debugger — crash-diagnosing watchdog added to existing Rust binary on Windows gaming pods (v11.2)*
*Researched: 2026-03-21 IST*
