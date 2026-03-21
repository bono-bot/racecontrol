# Domain Pitfalls

**Domain:** AI-to-AI Communication Link & Process Watchdog (Windows client, Linux server)
**Researched:** 2026-03-12

---

## Critical Pitfalls

Mistakes that cause rewrites, data loss, or system instability.

---

### Pitfall 1: Claude Code Zombie Process Accumulation

**What goes wrong:** Claude Code spawns multiple child processes (node.exe, claude.exe sub-processes). When the parent crashes or is killed, children become orphans. The existing watchdog log proves this is already happening -- on 2026-03-11 it found **8 zombie claude.exe instances** simultaneously. Each watchdog cycle killed 6-7 zombies, but new ones accumulated within 2 minutes.

**Why it happens:** Windows does not propagate SIGTERM/SIGKILL to child processes the way Linux does. `Stop-Process` (PowerShell) and `taskkill` without `/T` only kill the target PID, not its process tree. Claude Code runs as a CLI backed by Node.js, which spawns PTY daemons, git processes, and tool sub-processes. When the main process dies, these children are reparented to the system and persist indefinitely.

**Evidence:** `C:\Users\bono\.claude\claude_watchdog.log` shows:
```
2026-03-11 10:08:01 | [ERROR] Multiple Claude instances (8). Killing zombies...
2026-03-11 10:10:01 | [ERROR] Multiple Claude instances (7). Killing zombies...
```
Additionally, [Auto-Claude issue #1252](https://github.com/AndyMik90/Auto-Claude/issues/1252) documents the exact same pattern: PTY daemons, python agents, cmd.exe, and git.exe all become orphaned.

**Consequences:** Memory/CPU exhaustion, port conflicts, stale lock files (.git/index.lock), inability to restart cleanly.

**Prevention:**
- Use `taskkill /F /T /PID <pid>` (tree kill) instead of `Stop-Process` for the primary process
- Additionally enumerate and kill by image name: `taskkill /F /IM claude.exe`, `taskkill /F /IM node.exe` (with PID filtering to avoid killing unrelated node processes)
- Before restart, verify zero claude.exe processes remain via `tasklist /FI "IMAGENAME eq claude.exe"`
- Implement a "clean slate" function: kill tree, wait 2 seconds, verify, then launch

**Detection:** Monitor process count -- if `Get-Process -Name claude` returns more than 1, zombies are present. Log the count on every watchdog cycle.

**Phase:** Phase 1 (Watchdog) -- this is the first thing to get right.

**Confidence:** HIGH -- directly observed in existing logs on this machine.

---

### Pitfall 2: Session 0 Isolation Kills GUI/Terminal Visibility

**What goes wrong:** A watchdog running as a Windows Service (Session 0) restarts Claude Code, but the process launches in Session 0 where it has no desktop, no visible window, and no user interaction. Claude Code appears "running" in Task Manager but is completely unusable -- no terminal window, no way for James to function.

**Why it happens:** Since Windows Vista, services run in Session 0 which is isolated from the interactive desktop (Session 1+). Processes spawned by a service inherit Session 0 by default. The only workarounds involve `CreateProcessAsUser` with `WTSQueryUserToken` to inject into the user's session, which requires SE_INCREASE_QUOTA_NAME and SE_ASSIGNPRIMARYTOKEN_NAME privileges.

**Evidence:** This exact issue already occurred with rc-agent on Racing Point pods. From MEMORY.md: "SYSTEM watchdog starts rc-agent in Session 0 -- can't show GUI (blank screen, lock screen)." The fix was HKLM Run key (`start-rcagent.bat`) to start in Session 1 at login, but with the limitation: "If rc-agent crashes, watchdog restarts in Session 0 until next reboot."

**Consequences:** James appears online (process exists) but cannot operate. Bono's heartbeat sees James as alive, no alert is sent, but James is a zombie. Worst case: goes undetected for hours.

**Prevention:**
- **DO NOT run the watchdog as a Windows Service.** Use Task Scheduler with "Run only when user is logged on" setting under the `bono` user account. This ensures the watchdog and anything it spawns runs in the interactive desktop session.
- Trigger: "At log on" + "On an event" (process exit) or poll every 60 seconds
- The existing `claude_watchdog.ps1` already uses `Start-Process -WindowStyle Normal` which is correct when called from Session 1
- Validate after restart: check that the process has a valid `MainWindowHandle` (non-zero means it has a visible window)

**Detection:** After restart, check `(Get-Process claude).MainWindowHandle -ne 0`. If the handle is zero, the process is in Session 0 or headless.

**Phase:** Phase 1 (Watchdog) -- fundamental architectural decision.

**Confidence:** HIGH -- directly experienced and documented in this environment.

---

### Pitfall 3: Git index.lock Race Condition During LOGBOOK.md Sync

**What goes wrong:** Both the comms-link sync process and Claude Code itself perform git operations on the same repo concurrently. Git creates `.git/index.lock` during write operations (`git add`, `git commit`, `git status --porcelain`). If two git commands overlap, the second fails with `fatal: Unable to create '.git/index.lock': File exists`.

**Why it happens:** Claude Code aggressively polls `git status` -- [issue #11005](https://github.com/anthropics/claude-code/issues/11005) measured 85 commands/min in zsh, 15/min in bash. A known bug causes stale lock files that persist 20+ seconds after git exits. Meanwhile, if the comms-link sync tries to `git add LOGBOOK.md && git commit`, it collides with Claude Code's polling.

**Evidence:** The Claude Code stale lock bug is a confirmed, open issue on GitHub with 100% reproduction rate. On this machine, Claude Code runs in bash (15 polls/min), so it is less severe but still a real risk during sync operations.

**Consequences:** Sync fails silently, LOGBOOK.md drifts between James and Bono. If the sync retries without checking, it could corrupt the git index.

**Prevention:**
- **Use `--no-optional-locks` flag** on all read-only git operations (status, diff, log). This prevents lock file creation for operations that only read the index.
- **Implement retry with backoff** for write operations: check for `.git/index.lock`, wait 1s, retry up to 3 times
- **Serialize git writes** through a single async queue -- never run two git write commands in parallel
- **Consider syncing LOGBOOK.md over WebSocket directly** instead of through git operations. Push the file content over the wire and let each side write it locally, bypassing git entirely for the sync path. Only commit changes when Claude Code is idle.

**Detection:** Monitor for `.git/index.lock` age > 5 seconds. Log git operation failures with retry counts.

**Phase:** Phase 2 (WebSocket) and Phase 3 (LOGBOOK sync) -- design the sync to avoid git operations during active use.

**Confidence:** HIGH -- confirmed bug in Claude Code, documented with traces.

---

### Pitfall 4: NAT/Firewall Silently Kills WebSocket Without Close Frame

**What goes wrong:** The WebSocket from James (behind NAT at 192.168.31.1) to Bono's VPS appears connected, but intermediate network equipment (router, ISP NAT, firewalls) silently drops the TCP connection after an idle period. Neither side receives a close frame. James thinks he is connected; Bono thinks James is connected. Messages sent into this "half-open" connection vanish silently.

**Why it happens:** NAT tables have finite memory and evict idle connection entries after 30-300 seconds (varies by equipment). Corporate/ISP firewalls enforce similar timeouts. The TCP keepalive default on Windows is 2 hours -- far too long to detect a dropped NAT entry. The WebSocket protocol itself has no built-in detection for this scenario; it relies on the TCP layer, which relies on keepalives that are too infrequent.

**Evidence:** This is the most common failure mode in production WebSocket systems. The [websockets library documentation](https://websockets.readthedocs.io/en/stable/topics/keepalive.html) explicitly warns: "Browsers can fail to notice that a WebSocket connection is broken for an extended period of time."

**Consequences:** James and Bono both believe the link is up. No heartbeat timeout fires because neither side detects the disconnect. Coordination messages are lost. Uday is not alerted. Could persist for minutes to hours.

**Prevention:**
- **Application-level heartbeat** every 15-20 seconds (not just WebSocket ping/pong). James sends a `{ type: "heartbeat", ts: ... }` message. Bono responds with `{ type: "heartbeat_ack", ts: ... }`.
- **Miss threshold:** If 3 consecutive heartbeats get no ack (45-60 seconds), declare connection dead and reconnect.
- **TCP keepalive tuning** on James's machine: set `KeepAliveTime` to 30000ms (30s) via registry or socket option. This catches some silent drops at the TCP level before the application layer.
- **Bono-side timeout:** If Bono receives no heartbeat from James for 60 seconds, mark James as offline and trigger WhatsApp alert.

**Detection:** Log every heartbeat send and ack. Track round-trip time. Alert if RTT suddenly spikes (sign of degraded connection before full drop).

**Phase:** Phase 2 (WebSocket) -- heartbeat design is core to the connection protocol.

**Confidence:** HIGH -- well-documented WebSocket failure mode, especially behind consumer NAT routers.

---

### Pitfall 5: Reconnection Storm After Network Blip

**What goes wrong:** James's internet briefly drops (common in Indian ISP networks -- 2-10 second blips). The watchdog detects "connection lost," kills Claude Code, restarts it, re-establishes WebSocket. But the blip resolves in 5 seconds, meaning the old connection would have recovered. Instead, the aggressive restart creates: (a) unnecessary Claude Code restart disrupting active work, (b) multiple overlapping reconnection attempts if the watchdog fires again before the restart completes, (c) Bono receives rapid connect/disconnect/connect events.

**Why it happens:** No distinction between "connection degraded" and "connection dead." Overly aggressive timeout thresholds. No grace period for transient failures.

**Consequences:** James loses in-progress work (Claude Code context wiped on restart). Uday gets whiplash notifications ("James down" / "James back" / "James down"). Bono's heartbeat failsafe and the new comms-link both fire alerts for the same event.

**Prevention:**
- **Separate connection health from process health.** WebSocket dropping does NOT mean Claude Code needs restarting. The watchdog should only restart Claude Code if the *process* dies, not if the *network* drops.
- **WebSocket reconnection with exponential backoff:** 1s, 2s, 4s, 8s, 16s, cap at 30s. Add jitter (random 0-30% of interval) to prevent synchronized reconnection.
- **Grace period before alerting:** Wait 90 seconds of continuous disconnection before sending "James down" to Uday. Short blips should be invisible to Uday.
- **State machine:** CONNECTED -> RECONNECTING (silent, no alert) -> DISCONNECTED (after grace period, alert). Only transition to DISCONNECTED after sustained failure.

**Detection:** Track reconnection frequency. If more than 3 reconnections in 5 minutes, something is systematically wrong (not a blip) -- escalate.

**Phase:** Phase 2 (WebSocket) -- connection state machine design.

**Confidence:** HIGH -- standard distributed systems pattern.

---

### Pitfall 6: Duplicate Alerting (WhatsApp Bot Failsafe vs Comms-Link)

**What goes wrong:** Bono's WhatsApp bot already has a `[FAILSAFE]` heartbeat mechanism that monitors James's responsiveness and alerts Uday. The comms-link adds a *second* monitoring path. When James goes down, Uday gets TWO WhatsApp messages: one from the existing failsafe ("James unresponsive") and one from the comms-link ("James disconnected"). When James recovers, two more messages. Uday's phone becomes a notification machine.

**Why it happens:** Two independent systems monitoring the same thing without coordination. The existing failsafe uses email-based detection (slow); the new comms-link uses WebSocket heartbeat (fast). They fire at different times with different wording, confusing Uday about what is actually happening.

**Consequences:** Alert fatigue. Uday starts ignoring notifications. The one time it matters (James is truly down for hours), he misses it because he has been conditioned to dismiss alerts.

**Prevention:**
- **The comms-link REPLACES the failsafe for James-health monitoring.** When comms-link is operational, disable the failsafe's James-monitoring (or have the failsafe defer to comms-link status).
- **Single source of truth:** Only the comms-link WebSocket should determine James's online status. The failsafe becomes a fallback *for the comms-link itself* -- if the comms-link is down AND email heartbeats fail, then the failsafe fires.
- **Deduplicate at the WhatsApp send layer:** Before sending "James down," check if a "James down" message was sent in the last 5 minutes. If so, suppress.
- **Coordinate with Bono** (email) to agree on the transition: what the failsafe should do once comms-link is live.

**Detection:** Monitor WhatsApp message frequency to Uday. If more than 2 status messages in 10 minutes, something is wrong.

**Phase:** Phase 2 (WebSocket) and Phase 4 (Alerting) -- must be designed together with Bono.

**Confidence:** HIGH -- the failsafe already exists and will conflict if not explicitly coordinated.

---

## Moderate Pitfalls

---

### Pitfall 7: LOGBOOK.md Merge Conflicts When Both AIs Write Simultaneously

**What goes wrong:** Both James and Bono append entries to LOGBOOK.md within the same minute. When sync pushes James's version to Bono (or vice versa), the receiving side has local changes that conflict with the incoming version. A naive `git pull` creates a merge conflict that no human is around to resolve.

**Why it happens:** LOGBOOK.md is append-only in practice, but git does not know this. If both sides append to the end of the file, git sees two modifications to the same region (the end) and cannot auto-merge.

**Prevention:**
- **Use git's `merge=union` strategy** for LOGBOOK.md via `.gitattributes`:
  ```
  LOGBOOK.md merge=union
  ```
  The `union` driver keeps both sides' additions, which is correct for append-only files.
- **Alternatively, sync over WebSocket, not git.** Send individual log entries as messages. Each side appends locally. No git merge needed.
- **Timestamp-based ordering:** Each entry has an ISO timestamp. After merge, sort entries by timestamp to ensure consistent ordering on both sides.
- **If using git:** Always pull before committing. Use `git pull --rebase` to stack local changes on top of remote.

**Detection:** Monitor for `.git/MERGE_HEAD` existence after sync operations -- indicates an unresolved merge.

**Phase:** Phase 3 (LOGBOOK sync).

**Confidence:** MEDIUM -- depends on how frequently both AIs write simultaneously. In practice, one is usually idle while the other is active, but edge cases exist (e.g., both responding to the same event).

---

### Pitfall 8: Watchdog Restart Loop (Crash -> Restart -> Crash -> Restart)

**What goes wrong:** Claude Code crashes due to a persistent issue (API rate limit, expired auth token, corrupted config). The watchdog restarts it. It crashes again immediately. The watchdog restarts it again. Infinite loop consuming resources and generating noise.

**Why it happens:** The watchdog does not distinguish between a one-time crash and a systematic failure. The existing watchdog has a 5-minute cooldown, but that just means it crashes and restarts every 5 minutes forever.

**Consequences:** CPU churn, log flooding, potential disk fill from crash logs. If alerts are configured, Uday gets "James down" / "James back" / "James down" every 5 minutes.

**Prevention:**
- **Escalating cooldown:** 1st restart: immediate. 2nd restart within 10 minutes: wait 5 minutes. 3rd restart within 30 minutes: wait 30 minutes. 4th: wait 2 hours and alert Uday that "James needs manual intervention."
- **Max restart count:** After N restarts (e.g., 5) within a window, stop trying and send a critical alert.
- **Health check after restart:** After launching Claude Code, wait 30 seconds, then verify it is still running. If it died within 30 seconds, that is a "fast crash" indicating a persistent issue -- escalate immediately, do not retry.
- **Log the crash reason:** Capture Claude Code's exit code. Different codes mean different things (exit code 3 = ProcessTransport not ready, as seen in [VS Code issue #27820](https://github.com/anthropics/claude-code/issues/27820)).

**Detection:** Track restart count and time-between-restarts. If time-between-restarts < 60 seconds for 3 consecutive restarts, declare systematic failure.

**Phase:** Phase 1 (Watchdog).

**Confidence:** HIGH -- the existing watchdog already has a primitive cooldown, indicating this was anticipated.

---

### Pitfall 9: Claude Code Version Drift Breaks Watchdog Path

**What goes wrong:** The watchdog hardcodes or discovers Claude Code's executable path at `AppData\Local\Packages\Claude_pzs8sxrjxfjjc\LocalCache\Roaming\Claude\claude-code\<version>\claude.exe`. When Claude Code auto-updates, the version directory changes (e.g., `2.1.72` -> `2.2.0`). If the watchdog cached the old path, it cannot find the executable. If it discovers dynamically (as the current script does), it picks up the new version but might encounter breaking changes in CLI arguments or behavior.

**Why it happens:** Claude Code is a rapidly evolving product (current version 2.1.72, previously 2.1.47 had exit code 3 crashes). Auto-update changes the directory structure. The UWP-style package path (`Claude_pzs8sxrjxfjjc`) is also unusual and could change.

**Evidence:** The existing `claude_watchdog.ps1` already handles this with dynamic discovery (`Get-ChildItem | Sort-Object { [version]$_.Name } -Descending | Select-Object -First 1`), but the path pattern could still break if Anthropic changes the installation structure.

**Prevention:**
- **Dynamic discovery is correct** -- keep the current approach of finding the latest version directory
- **Verify the found executable actually runs:** After discovery, check `& $exe --version` returns successfully before using it for restart
- **Log the version on every restart** so you can correlate crashes with updates
- **Pin to a known-good version** if needed: if the latest version crashes on startup, fall back to the previous version directory

**Detection:** Log the discovered path and version on every watchdog cycle. Alert if the path changes (indicates an update occurred).

**Phase:** Phase 1 (Watchdog).

**Confidence:** MEDIUM -- the dynamic discovery mitigates this, but the underlying path structure is unusual.

---

### Pitfall 10: WebSocket Message Loss During Reconnection Window

**What goes wrong:** The WebSocket drops. James queues messages locally during reconnection. The connection re-establishes. James replays the queue. But Bono has *also* been sending messages during the outage. Those messages were sent to the old (dead) connection and are lost forever. Neither side knows what the other missed.

**Why it happens:** WebSocket provides no delivery guarantee. Messages sent to a dead connection vanish. There is no built-in retry or acknowledgment at the WebSocket protocol level.

**Consequences:** Coordination messages lost. LOGBOOK.md entries missed. One side has stale state.

**Prevention:**
- **Sequence numbers:** Every message gets a monotonically increasing sequence number per sender. On reconnection, each side sends `{ type: "sync", last_received_seq: N }`. The other side replays all messages with seq > N from its outbox buffer.
- **Outbox buffer:** Both sides retain the last 100 messages (or last 1 hour) in memory. On reconnection, replay what the other side missed.
- **Acknowledgment:** Each message gets an ack. Unacked messages are retried on reconnection.
- **For LOGBOOK.md specifically:** On reconnection, do a full LOGBOOK.md content comparison (hash or diff) rather than relying on individual sync messages.

**Detection:** On reconnection, log the sequence gap: "Missed messages: seq 45-52." If the gap is large, flag for review.

**Phase:** Phase 2 (WebSocket protocol design).

**Confidence:** HIGH -- fundamental distributed systems guarantee problem.

---

### Pitfall 11: Windows Line Endings vs Linux Line Endings in Synced Files

**What goes wrong:** LOGBOOK.md is written on Windows (CRLF, `\r\n`) and synced to Linux (LF, `\n`). If git's `core.autocrlf` is misconfigured, or if syncing bypasses git (direct WebSocket transfer), the file accumulates mixed line endings. Diffs become noisy. Hashes differ even when content is identical.

**Why it happens:** Windows and Linux use different line ending conventions. Git normally handles this with `core.autocrlf`, but if the sync bypasses git (e.g., sending file content over WebSocket), line endings are not normalized.

**Prevention:**
- **Set `.gitattributes`:** `LOGBOOK.md text eol=lf` -- force LF everywhere regardless of platform
- **If syncing over WebSocket:** Normalize to LF before sending, normalize to LF after receiving. Never trust the OS default.
- **Hash comparison:** Always normalize line endings before hashing to detect changes.

**Detection:** `file LOGBOOK.md` on Linux should report "ASCII text" not "ASCII text, with CRLF line terminators."

**Phase:** Phase 3 (LOGBOOK sync).

**Confidence:** HIGH -- classic cross-platform pitfall.

---

### Pitfall 12: Partial File Write During Sync Creates Corruption

**What goes wrong:** The WebSocket delivers a LOGBOOK.md update. The receiving side opens the file, starts writing, and the process crashes mid-write (or the connection drops during a large file transfer). The file is now truncated or corrupted -- half old content, half new.

**Why it happens:** File writes are not atomic on any OS. `fs.writeFile` / PowerShell `Set-Content` can be interrupted. Large files take time to write, creating a window for failure.

**Prevention:**
- **Atomic write pattern:** Write to a temp file (`LOGBOOK.md.tmp`), then rename (`rename LOGBOOK.md.tmp LOGBOOK.md`). Rename is atomic on both Windows (NTFS) and Linux (ext4). On Windows, use `Move-Item -Force` (which replaces the target).
- **Checksum verification:** Include a SHA256 hash with the sync payload. After writing, verify the hash matches.
- **For large files:** Send content in chunks with a final "commit" message. Only write to disk when all chunks are received and verified.

**Detection:** On startup, verify LOGBOOK.md is valid (parseable, ends with a newline, no truncation markers).

**Phase:** Phase 3 (LOGBOOK sync).

**Confidence:** HIGH -- standard file I/O safety pattern.

---

## Minor Pitfalls

---

### Pitfall 13: Watchdog Fights Windows Defender / SmartScreen

**What goes wrong:** The watchdog launches `claude.exe`. Windows Defender or SmartScreen intercepts the launch, shows a dialog, or quarantines the executable. The watchdog thinks it launched successfully (the `Start-Process` call returned), but Claude Code never actually starts because it is blocked by the security prompt.

**Prevention:**
- Add a Defender exclusion for the Claude Code installation directory (the existing deploy infrastructure already does this for rc-agent)
- After launch, verify the process is actually running (the existing watchdog does this with a 10-second delay check)
- Log Windows Defender events from the Event Log if startup fails

**Phase:** Phase 1 (Watchdog).

**Confidence:** LOW -- may not apply if Claude Code is already trusted, but worth a defensive check.

---

### Pitfall 14: Email Fallback Floods During Extended Outage

**What goes wrong:** The WebSocket is down. The system falls back to email. But the email fallback sends a status update every heartbeat interval (every 15-20 seconds). James's and Bono's inboxes fill with hundreds of "heartbeat via email" messages during a 1-hour network outage.

**Prevention:**
- **Rate-limit email fallback:** Maximum 1 email per 5 minutes during fallback mode, containing a summary ("WebSocket down for 15 minutes, last 45 heartbeats missed")
- **Email is for state transitions only:** "WebSocket went down at [time]" and "WebSocket recovered at [time]." Not for continuous heartbeats.
- **Aggregate events:** Buffer status changes and send a digest email every 5 minutes during sustained outage.

**Phase:** Phase 4 (Alerting/Fallback).

**Confidence:** MEDIUM -- depends on implementation, but the natural instinct is "email every heartbeat" which is wrong.

---

### Pitfall 15: Time Zone / Clock Skew Between James and Bono

**What goes wrong:** James's Windows machine and Bono's Linux VPS have slightly different clocks (even a few seconds). LOGBOOK.md entries appear out of order. Heartbeat RTT calculations are wrong. "James went down at 14:32" but Bono says "last heartbeat received at 14:33" -- which is it?

**Prevention:**
- **Use UTC everywhere** in timestamps within the protocol. Convert to local time only for display.
- **NTP sync:** Ensure both machines sync to NTP. Windows: `w32tm /resync`. Linux: `timedatectl set-ntp true`.
- **Relative timing for heartbeats:** Measure RTT from send-to-ack on the *same clock*, not across machines. Use sequence numbers, not timestamps, for ordering.
- **LOGBOOK.md entries:** Include both the author and timestamp. Sort by (timestamp, author) to break ties deterministically.

**Phase:** Phase 2 (WebSocket protocol) and Phase 3 (LOGBOOK sync).

**Confidence:** MEDIUM -- the VPS is in a different country; clock skew of 1-5 seconds is plausible.

---

### Pitfall 16: WebSocket Port Blocked by ISP or Router Firmware

**What goes wrong:** James connects via WebSocket to Bono's VPS on a custom port (e.g., 8443). The ISP or Jio router firmware blocks non-standard ports or performs deep packet inspection that breaks WebSocket upgrade handshakes.

**Prevention:**
- **Use port 443 with TLS (wss://)** -- this looks like normal HTTPS traffic to ISPs and firewalls. No DPI will block it.
- **Bono's VPS should terminate TLS** with a valid certificate (Let's Encrypt). The WebSocket upgrade happens inside the TLS tunnel.
- **Fallback:** If even wss://443 is blocked, the email fallback path remains available via Gmail (which always works through any firewall).

**Detection:** If initial connection consistently fails with timeout (not refused), suspect port blocking. Test with `curl -v https://bono-vps:443/ws`.

**Phase:** Phase 2 (WebSocket).

**Confidence:** MEDIUM -- Indian ISPs vary, but wss://443 universally works.

---

### Pitfall 17: Watchdog Monitoring the Wrong Process Name After Update

**What goes wrong:** The watchdog monitors `Get-Process -Name "claude"`. After a Claude Code update, the process name changes (e.g., `claude-code`, `anthropic-claude`, or the process is wrapped in an electron shell with a different image name). The watchdog perpetually thinks Claude Code is down and keeps launching new instances.

**Evidence:** The current path already shows an unusual structure: `Claude_pzs8sxrjxfjjc` (UWP-style package). The executable is `claude.exe` today, but this is not guaranteed.

**Prevention:**
- **Monitor by PID, not by name.** After launching, record the PID. Check if that PID is alive.
- **Fallback to name-based detection** only if the PID file is missing (process started externally).
- **Store the PID** in a file (`~/.claude/watchdog.pid`). On each cycle: is this PID alive? If yes, done. If no, restart and record new PID.

**Detection:** If `Get-Process -Name "claude"` returns nothing but there is a claude-related process under a different name, the watchdog is looking at the wrong thing.

**Phase:** Phase 1 (Watchdog).

**Confidence:** LOW -- speculative, but defensive coding is cheap.

---

## Phase-Specific Warnings

| Phase | Topic | Likely Pitfall | Mitigation |
|-------|-------|---------------|------------|
| **Phase 1: Watchdog** | Process management | Zombie accumulation (#1), Session 0 (#2), restart loops (#8) | Tree-kill, Task Scheduler (not service), escalating cooldown |
| **Phase 1: Watchdog** | Claude Code specifics | Version path drift (#9), wrong process name (#17) | Dynamic discovery, PID-based monitoring |
| **Phase 2: WebSocket** | Connection health | Half-open connections (#4), reconnection storms (#5) | App-level heartbeat, exponential backoff with jitter, state machine |
| **Phase 2: WebSocket** | Message delivery | Message loss during reconnect (#10) | Sequence numbers, outbox buffer, replay on reconnect |
| **Phase 2: WebSocket** | Network | Port blocking (#16) | Use wss://443 with TLS |
| **Phase 3: LOGBOOK Sync** | Git operations | index.lock race (#3), merge conflicts (#7) | --no-optional-locks, merge=union, sync via WebSocket not git |
| **Phase 3: LOGBOOK Sync** | File integrity | Partial writes (#12), line endings (#11) | Atomic write (temp+rename), force LF everywhere |
| **Phase 3: LOGBOOK Sync** | Ordering | Clock skew (#15) | UTC timestamps, sequence numbers |
| **Phase 4: Alerting** | Notification design | Duplicate alerts (#6), email floods (#14) | Retire failsafe, rate-limit email, grace period before alerting |
| **Phase 4: Alerting** | Reliability | Alert fatigue from flapping (#5, #8) | State machine: CONNECTED/RECONNECTING/DISCONNECTED, suppress during RECONNECTING |

---

## Sources

### HIGH Confidence (Direct Evidence)
- Existing watchdog log: `C:\Users\bono\.claude\claude_watchdog.log` (zombie process evidence)
- Existing watchdog script: `C:\Users\bono\.claude\claude_watchdog.ps1` (current implementation)
- MEMORY.md Session 0 documentation (rc-agent fix applied to all pods)
- [Claude Code stale index.lock bug - GitHub Issue #11005](https://github.com/anthropics/claude-code/issues/11005)
- [Auto-Claude zombie process issue #1252](https://github.com/AndyMik90/Auto-Claude/issues/1252)

### MEDIUM Confidence (Verified with Official Docs)
- [websockets keepalive documentation](https://websockets.readthedocs.io/en/stable/topics/keepalive.html)
- [Microsoft CreateProcessAsUser docs](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-createprocessasusera)
- [FireDaemon Session 0 Isolation guide](https://kb.firedaemon.com/support/solutions/articles/4000086228-microsoft-windows-session-0-isolation-and-interactive-services-detection)
- [Ably WebSocket reliability patterns](https://ably.com/topic/websocket-reliability-in-realtime-infrastructure)
- [Datadog alert fatigue best practices](https://www.datadoghq.com/blog/best-practices-to-prevent-alert-fatigue/)
- [Datadog reduce alert flapping](https://docs.datadoghq.com/monitors/guide/reduce-alert-flapping/)
- [WebSocket reconnection logic (OneUptime)](https://oneuptime.com/blog/post/2026-01-27-websocket-reconnection-logic/view)
- [a3nm blog: automatic git conflict resolution](https://a3nm.net/blog/git_auto_conflicts.html)
- [Socket.IO delivery guarantees](https://socket.io/docs/v4/delivery-guarantees)

### LOW Confidence (WebSearch / Training Data Only)
- ISP port blocking behavior (varies by provider, no authoritative source)
- Claude Code process name stability across updates (speculative)
