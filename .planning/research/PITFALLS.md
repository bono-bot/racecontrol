# Pitfalls Research

**Domain:** Pod management / process supervision / WebSocket hardening / deployment pipelines (Windows gaming venue)
**Researched:** 2026-03-13
**Confidence:** HIGH (derived from codebase inspection + archived Phase 05 research + targeted web research)

---

## Critical Pitfalls

### Pitfall 1: Declaring Recovery from HTTP 200 on Restart Command

**What goes wrong:**
`pod-agent /exec` returns HTTP 200 when the restart shell command is delivered. The monitor marks the pod recovered. But `start /b rc-agent.exe` exits immediately — it only launches the process. If rc-agent crashes on startup (bad config, port already bound, missing file), the HTTP 200 still arrived and recovery is falsely declared. The pod stays marked Online while it is actually dead.

**Why it happens:**
The restart command (`start /b rc-agent.exe`) is fire-and-forget. pod-agent reports success for command delivery, not for process health. The monitor has no follow-up check.

**How to avoid:**
After every restart command succeeds, spawn a separate tokio task that polls at +5s, +15s, +30s, +60s checking: (a) `tasklist | findstr rc-agent` via pod-agent exec, (b) `state.agent_senders.contains_key(&pod_id)` for WebSocket reconnect. Only declare recovery when both pass. See existing `check_rc_agent_health()` pattern in `pod_healer.rs` for the lock screen probe approach.

**Warning signs:**
Activity log shows "Agent Restarted" followed immediately by another offline detection cycle. Kiosk shows the pod bouncing between Online/Offline every 2 minutes.

**Phase to address:** WebSocket & watchdog hardening phase (covers post-restart verification requirement)

---

### Pitfall 2: File Lock on Binary Replace (Windows)

**What goes wrong:**
Deploying a new `rc-agent.exe` fails silently or with "access denied" because the old process still holds a file lock on the executable. The new binary never lands. pod-agent reports the download command succeeded, but `rc-agent.exe` on disk is still the old version. The pod restarts into the old binary.

**Why it happens:**
Windows locks executable files while they are running. Unlike Linux (where the inode is unlinked and the new file replaces it), Windows prevents overwriting a running `.exe`. `taskkill /F /IM rc-agent.exe` kills the process but the OS may not release the file handle instantly, especially if antivirus (Defender) is scanning the terminated process.

**How to avoid:**
Enforce the kill → wait → verify-dead → download sequence with an explicit delay after kill. Use `tasklist | findstr rc-agent` to confirm the process is gone before downloading. Add a 2-3 second sleep between kill and download in the deploy command chain. The current restart command already has `timeout /t 2` — keep it and ensure deploy scripts do the same. Verify binary size after download before starting.

**Warning signs:**
Deploy reports success but `rc-agent --version` output is unchanged. Binary size on pod matches old build. New features absent after "successful" deploy.

**Phase to address:** Deployment pipeline hardening phase

---

### Pitfall 3: Session 0 GUI Blindness After Remote Restart

**What goes wrong:**
pod-agent runs as SYSTEM. When it restarts rc-agent via `start /b rc-agent.exe`, the new process spawns in Windows Session 0 (the non-interactive SYSTEM session). All GUI surfaces — lock screen, overlay — are invisible to the customer sitting at the pod. The process is running and the WebSocket reconnects, so all monitoring shows green. The customer sees a blank screen.

**Why it happens:**
Windows isolates GUI from Session 0 as a security boundary (Session 0 Isolation, introduced Vista). Processes spawned from SYSTEM services inherit Session 0 and cannot draw to the user desktop (Session 1).

**How to avoid:**
Post-restart verification must treat "WebSocket connected but lock screen (port 18923) unresponsive" as a **partial recovery**, not full recovery. Log it as "Session 0 restart — GUI will restore on next login/reboot." Do NOT trigger an email alert for Session 0 partial recovery — it is expected and resolves itself. The HKLM Run key (`start-rcagent.bat`) handles Session 1 startup at next login. True failure is WebSocket also not connected.

**Warning signs:**
Lock screen health check fails (port 18923 returns 0) but WebSocket sender exists for that pod_id. Customer reports blank screen after pod restarts but staff kiosk shows pod as Online.

**Phase to address:** WebSocket & watchdog hardening phase (post-restart verification logic)

---

### Pitfall 4: WebSocket Drop During Game Launch CPU Spike

**What goes wrong:**
Launching a sim (Assetto Corsa, F1, Forza) causes a CPU spike on the pod (shader compilation, asset loading) lasting 5-30 seconds. During this spike, rc-agent's tokio runtime is starved. The WebSocket ping/pong cycle misses its deadline. rc-core sees a missed pong and closes the connection as stale. The kiosk briefly shows "disconnected" and may trigger a false offline detection.

**Why it happens:**
A single-threaded or under-resourced tokio runtime on the pod cannot service both game launch I/O and WebSocket keepalive simultaneously. The hyper/tungstenite default ping timeout is typically 20-30 seconds — tight enough to trip during heavy launch load.

**How to avoid:**
Two strategies, both needed:
1. **Server-side tolerance:** In rc-core, do not act on a single missed heartbeat. The existing 6s UDP heartbeat timeout is already a dead-pod threshold — preserve it but add a grace window (require 2-3 consecutive missed heartbeats before marking offline). Do not trigger a restart on first miss.
2. **Client-side keepalive:** In rc-agent's WebSocket loop, send application-level pings (not just relying on tungstenite's protocol-level ping) every 10s. This gives rc-core evidence the agent is alive even if the protocol-level ping races with game launch.
3. **Kiosk suppression:** If a pod transitions Online→Offline→Online within a short window (< 30s), suppress the "disconnected" flash in the kiosk by debouncing the status display.

**Warning signs:**
Activity log shows pods going offline at the same time customers launch games. "Disconnected" flashes in kiosk last 5-15 seconds then resolve without any manual action. UDP heartbeat gaps align with game launch events.

**Phase to address:** WebSocket connection resilience phase

---

### Pitfall 5: Concurrent Restart from Monitor + Healer

**What goes wrong:**
`pod_monitor.rs` (10s check interval) and `pod_healer.rs` (120s interval) both detect an unhealthy pod and both issue restart commands within seconds of each other. The pod gets killed and restarted twice. If billing was active, the second kill interrupts the first startup. Race conditions in the restart command chain (`taskkill → timeout → start`) compound.

**Why it happens:**
Monitor and healer run on independent tokio intervals with no shared lock or coordination state. Both check health conditions that can be true simultaneously (missed heartbeat + stale socket).

**How to avoid:**
Share a single `EscalatingBackoff` per pod in `AppState`. Both monitor and healer read from it before acting. Assign ownership: pod_monitor owns restart commands; pod_healer owns diagnostics (kill zombies, clear temp, check disk). Healer should check `last_restart_attempt` timestamp and skip restart if monitor acted in the last 60s.

**Warning signs:**
Activity log shows two "Agent Restarted" entries within 10 seconds for the same pod. rc-agent fails to start because it was killed mid-startup.

**Phase to address:** WebSocket & watchdog hardening phase (shared backoff state requirement)

---

### Pitfall 6: Email Storm on Venue-Wide Network Event

**What goes wrong:**
A router reboot, switch failure, or DHCP re-lease takes all 8 pods offline simultaneously. Each pod's independent escalation state triggers an email alert. Uday receives 8 emails within 60 seconds, or more if retry logic fires. He cannot determine whether it is a single venue-wide event or 8 independent failures.

**Why it happens:**
Per-pod email rate limiting (1 email/pod/30min) does not prevent simultaneous multi-pod emails. 8 pods × 1 email = 8 emails in one burst.

**How to avoid:**
Add a venue-level rate limit layer: max 1 email per 5 minutes across all pods. When multiple pods are offline within the same detection window (e.g., 3+ pods offline within 30s), aggregate into a single email: "Venue-wide outage detected: Pods 1, 3, 5, 7 offline." Include the count to signal it is likely infrastructure, not individual pod failures.

**Warning signs:**
Multiple pods going offline within the same 30-second monitor cycle. Email subject lines show multiple pod numbers arriving within seconds of each other.

**Phase to address:** Watchdog alerting phase

---

### Pitfall 7: Config Validation Failure is Silent

**What goes wrong:**
A deployed rc-agent binary starts successfully (process is alive, port opens) but silently uses default/fallback values because a required config field is missing or mis-typed in `rc-agent.toml`. Features that depend on missing config (billing rates, game UDP ports, pod ID) either do not work or use wrong values. Monitoring shows the pod as healthy.

**Why it happens:**
Rust `config` crate and `serde` with `#[serde(default)]` silently substitute defaults. A config file deployed with wrong field names (e.g., `billing_rate` vs `billing_rate_per_minute`) loads without error, using the default value of 0 or empty string.

**How to avoid:**
Add a `validate()` method called at startup that checks required fields are non-empty/non-zero and returns an error that causes rc-agent to exit with a non-zero code. Key fields to validate: `pod_id`, `core_url`, billing rates (must be > 0), pod IP. Log the config on startup at INFO level so deployed config is visible in logs. Never use `#[serde(default)]` on fields that would silently break billing if absent.

**Warning signs:**
Pod connects to WebSocket but billing shows ₹0 sessions. Pod ID shows as empty string in kiosk. Lock screen accepts any PIN.

**Phase to address:** Deployment & config validation phase

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Fixed restart cooldowns (120s/600s) | Simple to implement | Crash-looping pod restarts every 2 min forever, hammering a broken pod | Never — replace with escalating backoff |
| `tasklist` text parsing for health checks | Works without extra tooling | Breaks if process name changes, returns false positive if another process contains "rc-agent" in name | Only for MVP, replace with PID-based check |
| `start /b rc-agent.exe` from SYSTEM (Session 0 restart) | Reuses pod-agent exec infrastructure | Lock screen/overlay invisible until reboot | Acceptable as best-effort — document the limitation |
| Single-source heartbeat (UDP only) | Simple liveness check | UDP packets drop under CPU load — same spike that causes game launch issues also causes false offline detection | Never rely on UDP alone — require missed heartbeat count > 1 |
| `unwrap()` on config deserialization | Faster to write | Process panics on first malformed config, no useful error message | Never in production |

---

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| pod-agent `/exec` | Treating HTTP 200 as "command succeeded" | HTTP 200 = command delivered. Check actual output or follow-up with health poll |
| `send_email.js` shell-out | Assuming Node.js and credentials exist on Racing-Point-Server (.23) | Verify `node --version` on server before deploying. Use absolute paths to script and credential file. Gracefully swallow email failures — never let them block watchdog |
| `taskkill /F /IM rc-agent.exe` | Killing parent process only | Use `/T` flag to kill process tree, otherwise child processes (WebView2, game subprocesses) linger and hold file locks |
| Windows Defender real-time scanning | Defender scans newly downloaded binary, holds file lock for 1-3 seconds | Add `C:\RacingPoint\` to Defender exclusions (already done per MEMORY.md), verify exclusion is present before deploy |
| `state.agent_senders` as WebSocket health indicator | Sender exists but connection is actually broken (channel full, closed) | Verify sender is responsive by attempting to send a ping message and checking for error, not just `contains_key()` |

---

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Blocking tokio task in pod_monitor loop | All 8 pods paused waiting for one slow pod-agent exec call | Always use `tokio::spawn` for per-pod work; never `.await` pod-agent HTTP inside the main loop | First time a pod-agent call takes > 10s (network issue, slow pod) |
| Polling all 8 pods sequentially | Monitor cycle takes 8x longer than expected, last pod checked has stale data | Fan-out with `futures::join_all` or `tokio::spawn` per pod | Immediately visible if one pod's pod-agent is slow |
| Post-restart verification spawning unbounded tasks | 8 pods restart simultaneously → 8 verification tasks spawned → each spawns sub-tasks | Cap concurrent verification tasks with a semaphore; share state | All 8 pods offline simultaneously (venue-wide event) |
| Email shell-out blocking the alerter | `tokio::process::Command` called without timeout → alerter blocks indefinitely if Node.js hangs | Always add `.kill_on_drop(true)` and a timeout on the Command future | First time `send_email.js` encounters a network issue |

---

## Security Mistakes

| Mistake | Risk | Prevention |
|---------|------|------------|
| pod-agent `/exec` endpoint accepts arbitrary commands with no auth | Any process on the LAN can run arbitrary commands on pods | pod-agent already runs on LAN-internal ports — verify it is not exposed on public interfaces. Do not add auth complexity; keep it LAN-only |
| Config file contains credentials in plaintext (toml) | If config is accidentally committed to git, credentials leak | Keep sensitive fields (API keys, tokens) in environment variables or a separate secrets file not tracked by git |
| Restart commands logged verbatim including any embedded credentials | Log scraping reveals secrets | Never embed credentials in restart commands. Pass via config file, not command arguments |

---

## UX Pitfalls

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| Kiosk shows "Disconnected" flash during every game launch | Customer and staff panic thinking the pod is broken; interrupts immersion | Debounce the kiosk status display: only show "Disconnected" if offline state persists > 15s. Game launch spikes last 5-30s, so threshold eliminates false alarms |
| Email alert with raw pod_id (UUID) in subject | Uday cannot identify which physical pod is affected | Always include human-readable pod name ("Pod 3 — 192.168.31.28") in alerts alongside UUID |
| No differentiation between "restarting" and "offline" in kiosk | Staff cannot tell if a pod is being healed vs. hard-failed | Add a "Recovering" status distinct from "Offline" during the post-restart verification window |

---

## "Looks Done But Isn't" Checklist

- [ ] **Binary deploy:** Verify with `tasklist` that old process is dead AND new binary size matches expected before declaring success
- [ ] **Restart command:** rc-agent process running does not mean it is healthy — also check WebSocket reconnect in `agent_senders`
- [ ] **Config validation:** rc-agent starting without error does not mean config is correct — log effective config on startup and check billing rates are non-zero
- [ ] **Email alerting:** `send_email.js` running on James's machine does not mean it works on Racing-Point-Server (.23) — verify Node.js installed on server separately
- [ ] **Escalating backoff:** Backoff state reset on recovery must be tested — a pod that recovers and fails again should restart at 30s, not 30m
- [ ] **Defender exclusions:** `C:\RacingPoint\` excluded on all 8 pods — must be verified individually, not assumed from one pod's config
- [ ] **Process tree kill:** `/F /IM` kills only the named process — child processes (subprocesses spawned by rc-agent or games) may linger and hold locks unless `/T` is added

---

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| False recovery declaration (HTTP 200 but process dead) | LOW | Post-restart verification catches it within 60s; next monitor cycle will re-trigger restart with escalated cooldown |
| File lock on binary replace | LOW | Re-run deploy with explicit kill + 5s wait + verify-dead step; confirm Defender exclusions are in place |
| Session 0 blind restart | LOW | Log the partial recovery; wait for next customer login which triggers HKLM Run key and Session 1 startup |
| WebSocket drop during game launch | LOW | rc-agent reconnects automatically; debounce in kiosk prevents staff action. If persistent, check tokio worker thread count on pod |
| Concurrent monitor + healer restart | MEDIUM | Identify via activity log timestamps; shared backoff state prevents recurrence. If binary is corrupted mid-restart, run manual deploy via pendrive |
| Email storm on venue-wide event | LOW | Delete duplicate emails; add venue-level rate limiter before next occurrence |
| Silent config mismatch | MEDIUM | Check rc-agent startup logs for effective config dump; redeploy correct toml via pod-agent; verify billing rates in kiosk |

---

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| HTTP 200 false recovery | Post-restart verification (watchdog phase) | Unit test: mock pod-agent returning 200, confirm monitor waits for process + WS check before declaring healthy |
| File lock on binary replace | Deployment pipeline hardening | Deploy to Pod 8, verify new binary version via `rc-agent --version` output |
| Session 0 GUI blindness | Watchdog hardening (post-restart verification) | Confirm "partial recovery" log appears; confirm no email alert sent for Session 0 case |
| WebSocket drop on game launch | WebSocket connection resilience phase | Simulate game launch on Pod 8, verify kiosk shows no "Disconnected" flash |
| Concurrent restart (monitor + healer) | Shared backoff state (watchdog phase) | Unit test: trigger both monitor and healer conditions simultaneously, confirm only one restart fires |
| Email storm | Venue-level rate limiting (alerting phase) | Unit test: fire alerts for all 8 pods within 1s, confirm only 1 aggregated email sent |
| Silent config mismatch | Config validation phase | Unit test: start rc-agent with missing `pod_id` field, confirm process exits with error |

---

## Sources

- **Codebase inspection (HIGH):** `pod_monitor.rs`, `pod_healer.rs`, `udp_heartbeat.rs`, `pod-agent/src/main.rs`, `rc-agent/src/main.rs` — pitfalls derived from actual code paths
- **Archived Phase 05 research (HIGH):** `.planning/archive/hud-safety/phases/05-watchdog-hardening/05-RESEARCH.md` — Session 0 blindness, flapping, email storm, stale backoff state, concurrent restart, Node.js availability
- **MEMORY.md (HIGH):** Session 0 fix history, Defender exclusions, deploy sequence rules, `taskkill /T` tree-kill requirement
- **[Axum WebSocket discussions](https://github.com/tokio-rs/axum/discussions/1216) (MEDIUM):** No automatic reconnect in standard; backoff with jitter required; tungstenite auto-responds to pings but application-level ping still needed
- **[Microsoft: taskkill cannot stop process](https://support.microsoft.com/en-us/topic/you-cannot-stop-a-process-by-using-the-taskkill.exe-utility-in-windows-69bd6757-72de-6484-3503-359fd0c0d53c) (MEDIUM):** Windows file lock behavior; process tree kill requirement
- **[Kudu: Dealing with locked files during deployment](https://github.com/projectkudu/kudu/wiki/Dealing-with-locked-files-during-deployment) (MEDIUM):** Kill-before-replace pattern; file lock timing after process termination

---
*Pitfalls research for: RaceControl Reliability & Connection Hardening*
*Researched: 2026-03-13*
