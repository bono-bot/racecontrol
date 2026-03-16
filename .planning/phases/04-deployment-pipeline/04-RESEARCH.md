# Phase 4: Deployment Pipeline Hardening - Research

**Researched:** 2026-03-13
**Domain:** Windows binary deployment, process lifecycle, file locking, session-aware scheduling, Rust/Axum HTTP API, Next.js kiosk UI
**Confidence:** HIGH

---

## Summary

Phase 4 automates the manual deploy workflow into a reliable, repeatable pipeline managed from racecontrol. Today, deploying a new rc-agent binary to pods requires James (or a human) to manually run a Python script or curl commands. The deploy sequence (kill process, delete binary, download new binary, start process) has multiple failure modes on Windows: file lock errors when overwriting a running .exe, partial installs when downloads fail mid-way, no visibility into deploy progress, and no protection for pods with active customer sessions.

The phase introduces three layers:
1. **Shared protocol types** (DeployState enum, DeployCommand/DeployProgress messages) in rc-common for cross-crate communication.
2. **Deploy executor** in racecontrol that orchestrates the kill->verify-dead->download->size-check->start->verify-reconnect sequence for a single pod via pod-agent /exec calls.
3. **Rolling deploy controller** that deploys to Pod 8 (canary) first, verifies health, then rolls to remaining pods while skipping any pod with an active billing session.

Every building block exists: pod-agent /exec runs commands on pods, pod-agent /write pushes files, deploy_pod.py proves the sequence works manually, pod_monitor's verify_restart() shows how to confirm health post-restart, and billing.active_timers tracks active sessions. The work is integration — wiring these into an automated, observable pipeline with proper error handling.

---

## Current Deploy Workflow (Manual)

### Method 1: deploy_pod.py (primary)

File: `C:\Users\bono\racingpoint\deploy-staging\deploy_pod.py`

Steps:
1. Start HTTP server on James (.27): `python3 -m http.server 9998 --directory deploy-staging`
2. Run: `python deploy_pod.py <pod_number>` or `python deploy_pod.py all`
3. Script performs 5 steps per pod:
   - [1/5] Kill rc-agent.exe via `/exec` (`taskkill /F /IM rc-agent.exe`)
   - [2/5] Delete old rc-agent.toml via `/exec` (`del /Q C:\RacingPoint\rc-agent.toml`)
   - [3/5] Write new config via `/write` endpoint (generates from template per pod_number)
   - [4/5] Download new binary via `/exec` (`curl.exe -s -f -o C:\RacingPoint\rc-agent.exe <url>`)
   - [5/5] Start rc-agent via `/exec` (`cd /d C:\RacingPoint && start /b rc-agent.exe`)

### Method 2: deploy-cmd.json (ad-hoc curl)

File: `C:\Users\bono\racingpoint\deploy-staging\deploy-cmd.json`

Single compound command: `taskkill /F /IM rc-agent.exe & timeout /t 2 & del /F C:\RacingPoint\rc-agent.exe & timeout /t 1 & curl -s -o C:\RacingPoint\rc-agent.exe http://192.168.31.27:9998/rc-agent.exe & dir C:\RacingPoint\rc-agent.exe & start "" C:\RacingPoint\rc-agent.exe`

Problems:
- No verification between steps (process might not be dead when binary is deleted)
- No size check after download
- No WebSocket reconnection verification
- No session protection (will kill active customer sessions)
- Silent failure if any step fails (compound `&` doesn't check exit codes)

### Method 3: install.bat (USB pendrive)

File: `C:\Users\bono\racingpoint\deploy-staging\install.bat`

8-step batch script for initial setup or recovery. Must be run physically on the pod as admin. Used for fresh installs or when pod-agent is unreachable.

---

## Existing Code That Handles Deployment

### pod-agent /exec endpoint

- **Port:** 8090, HTTP POST
- **Request:** `{"cmd": "<shell command>", "timeout_ms": 10000}`
- **Response:** `{"success": true/false, "exit_code": 0, "stdout": "...", "stderr": "..."}`
- **Timeout:** Returns HTTP 500 + `success: false` + exit_code 124 on timeout
- **Important:** Field is `cmd`, NOT `command`
- **Semaphore:** Limits concurrent commands (prevents overload)
- **Used by:** pod_monitor restart, pod_healer diagnostics, manual deploy

### pod-agent /write endpoint

- **Request:** `{"path": "C:\\RacingPoint\\rc-agent.toml", "content": "<file content>"}`
- **Response:** `{"status": "ok", "path": "...", "bytes": N}`
- **Behavior:** Overwrites file atomically
- **Used by:** deploy_pod.py for config writes

### pod-agent /ping endpoint

- **GET request, returns 200 if pod-agent is alive**
- **Used by:** pod_monitor to check if pod-agent is reachable before restart

### pod_monitor verify_restart() (Phase 2)

- **File:** `crates/racecontrol/src/pod_monitor.rs` lines 498-683
- **Checks:** process alive (tasklist), WebSocket connected (is_closed()), lock screen responsive (/health on 18923)
- **Schedule:** 5s, 15s, 30s, 60s polling intervals
- **Result:** Full recovery (3/3 pass) -> WatchdogState::Healthy; failure -> RecoveryFailed + email alert
- **Reusable for:** Post-deploy verification (same 3 checks apply)

### wol::restart_pod()

- **File:** `crates/racecontrol/src/wol.rs` lines 56-74
- **Does:** `shutdown /r /f /t 0` via pod-agent /exec
- **Different from deploy restart:** This reboots the entire Windows machine, not just rc-agent

### API route: POST /pods/{id}/restart

- **File:** `crates/racecontrol/src/api/routes.rs` line 39
- **Calls:** `wol::restart_pod()` which reboots the entire machine
- **Not suitable for binary deploy:** We need to restart rc-agent only, not the whole pod

---

## Windows File Lock Issues

**Core problem:** A running `.exe` on Windows cannot be overwritten or deleted. Any attempt to `del` or `curl -o` to the same path while the process is alive results in "Access denied" or "The process cannot access the file because it is being used by another process."

**Current mitigation:** deploy_pod.py kills the process first (step 1), then deletes (step 2), then downloads (step 4). But there is no verification that the process is actually dead between steps 1 and 2 — `taskkill /F` sends the kill signal but the process may take a moment to exit.

**Robust solution needed:**
1. `taskkill /F /IM rc-agent.exe` — send kill signal
2. Poll: `tasklist /NH | findstr rc-agent` — wait until process is gone (up to 10s)
3. Verify file is unlocked: attempt `del /Q C:\RacingPoint\rc-agent.exe` — if fails, retry after 1s
4. Only then download the new binary

**Rename trick (alternative):** On Windows, you CAN rename a running .exe (but not delete/overwrite). Some deploy tools rename `rc-agent.exe` to `rc-agent.old.exe`, write new `rc-agent.exe`, then kill the old process. This avoids the file lock window entirely. However, it leaves a stale `.old` file that must be cleaned up.

**Recommended approach:** Kill-wait-verify-dead pattern. Simpler, proven in deploy_pod.py (just needs the wait/verify step added). The rename trick adds complexity for minimal benefit since we must kill the old process anyway to free the port (18923 lock screen, WebSocket connection).

---

## How Billing/Sessions Work (Session Protection Context)

### Active session tracking

- **Location:** `AppState.billing.active_timers: RwLock<HashMap<String, BillingTimer>>`
- **Key:** pod_id (e.g., "pod_3")
- **Value:** BillingTimer struct with session_id, driver_name, remaining_seconds, etc.
- **Check:** `state.billing.active_timers.read().await.contains_key(&pod_id)`

### Session lifecycle

1. Staff assigns customer via kiosk -> creates auth token
2. Customer enters PIN on lock screen -> billing starts
3. BillingTimer ticks every second, sends BillingTick to agent
4. Timer reaches 0 -> session ends, game stops, lock screen returns

### Why deploys must not disrupt active sessions

Killing rc-agent during a billing session means:
- Customer's game window may stay open but no billing tick updates
- Lock screen goes away (no PIN gate after session)
- Billing timer in racecontrol still runs but agent can't receive SessionEnded
- Customer sees "Disconnected" on overlay
- On rc-agent restart, billing resync sends current timer state, but game state may be inconsistent

### Phase 4 approach: Defer deploy for busy pods

For the rolling deploy:
- Check `active_timers.contains_key(pod_id)` before deploying to each pod
- If active session: skip pod, mark as "deferred" in deploy status
- After session ends (BillingSessionChanged event): deploy can proceed
- Alternatively: wait N seconds and re-check, or let staff manually trigger after session

---

## How Pod Health Is Verified After Restart (Phase 2)

Phase 2's `verify_restart()` in pod_monitor.rs provides the exact pattern needed for post-deploy verification:

1. **Process alive:** `tasklist /NH | findstr rc-agent` via pod-agent /exec
2. **WebSocket connected:** `sender.is_closed()` on the agent_senders channel
3. **Lock screen responsive:** PowerShell `Invoke-WebRequest http://127.0.0.1:18923/health` via pod-agent /exec
4. **Polling schedule:** 5s, 15s, 30s, 60s
5. **Success:** All 3 pass -> healthy
6. **Failure:** Any fail at 60s -> alert + report

The deploy executor can reuse this same verification logic. After starting the new binary, spawn a verification task that checks the same 3 conditions. On success, mark deploy as Complete. On failure, mark as Failed with reason and send email alert.

---

## Gap Analysis: What's Missing vs What Exists

### Exists (ready to use)

| Component | Location | Status |
|-----------|----------|--------|
| Pod-agent /exec (run commands on pods) | pod-agent (Node.js, port 8090) | Working on all 8 pods |
| Pod-agent /write (push files to pods) | pod-agent | Working |
| Pod-agent /ping (health check) | pod-agent | Working |
| Deploy template (rc-agent.template.toml) | deploy-staging/ | Working |
| Deploy script (deploy_pod.py) | deploy-staging/ | Working but manual |
| HTTP file server (python -m http.server) | James .27:9998 | Manual start required |
| Post-restart verification logic | pod_monitor.rs verify_restart() | Working (Phase 2) |
| Active billing check | billing.active_timers | Working |
| DashboardEvent broadcast (WS to kiosk) | dashboard_tx broadcast channel | Working |
| Email alerter | email_alerts.rs | Working (Phase 2) |
| WatchdogState FSM (Healthy/Restarting/etc) | state.rs | Working (Phase 2) |
| Kiosk WebSocket connection | kiosk/src/hooks/useKioskSocket.ts | Working |

### Missing (must build in Phase 4)

| Gap | Description | Plan |
|-----|-------------|------|
| DeployState enum | No shared type for deploy lifecycle states | 04-01 |
| DeployProgress protocol messages | No way to stream deploy progress to dashboard | 04-01 |
| DashboardEvent::DeployProgress | Dashboard has no deploy-related event variant | 04-01 |
| DashboardCommand::Deploy variants | Kiosk cannot trigger deploys | 04-01 |
| Deploy executor module | No code orchestrates kill->verify->download->start | 04-02 |
| POST /api/deploy/:pod_id endpoint | No API to trigger deploy for a single pod | 04-02 |
| Kill-wait-verify-dead logic | deploy_pod.py doesn't verify process is dead | 04-02 |
| Binary size check after download | No validation that downloaded binary is sane | 04-02 |
| Deploy state per pod in AppState | No tracking of which pods are deploying | 04-02 |
| Rolling deploy orchestrator | No automated multi-pod deploy with canary | 04-03 |
| Session-aware scheduling | No skip-busy-pod logic in deploy path | 04-03 |
| POST /api/deploy/rolling endpoint | No API for rolling deploy | 04-03 |
| GET /api/deploy/status endpoint | No API to query deploy state of all pods | 04-03 |
| Kiosk deploy UI | No buttons or progress display for deployments | 04-03 |

### Partially exists (needs adaptation)

| Component | Current State | Needed Change |
|-----------|---------------|---------------|
| verify_restart() | Checks process + WS + lock screen | Extract into reusable function callable from deploy executor |
| wol::restart_pod() | Reboots entire machine | Need separate rc-agent-only restart function |
| deploy_pod.py step sequence | kill -> del config -> write config -> download -> start | Port to Rust in deploy executor with proper error handling between steps |

---

## Standard Stack

### Core (already in use -- no new dependencies)

| Library | Purpose | Notes |
|---------|---------|-------|
| axum | HTTP endpoints for deploy API | Already in racecontrol |
| tokio | Async executor, spawn, sleep | Already in use |
| reqwest | HTTP client for pod-agent calls | Already in AppState.http_client |
| serde/serde_json | Serialize/deserialize protocol messages | Already in use |
| chrono | Timestamps for deploy events | Already in use |
| tracing | Structured logging | Already in use |

No new dependencies required.

---

## Architecture Patterns

### Pattern 1: DeployState FSM

```
Idle -> Killing -> WaitingDead -> Downloading -> Verifying -> Starting -> VerifyingHealth -> Complete
                                                                                         -> Failed
Any state -> Failed (on error)
```

Each state tracks what the executor is currently doing. The kiosk can display this to staff. Unlike WatchdogState (which is internal), DeployState is staff-visible.

### Pattern 2: Deploy Executor as Async Task

The deploy for a single pod runs as a tokio::spawn'd task. This is the same pattern as verify_restart() -- detached from the request handler so the HTTP response returns immediately with "deploy started" rather than blocking for 60+ seconds.

The executor:
1. Sets DeployState to Killing
2. Sends `taskkill /F /IM rc-agent.exe` via pod-agent /exec
3. Polls `tasklist | findstr rc-agent` every 2s up to 10s (WaitingDead)
4. Sends `del /F C:\RacingPoint\rc-agent.exe` via /exec
5. Sends `curl -s -f -o C:\RacingPoint\rc-agent.exe <url>` via /exec (Downloading)
6. Sends `dir C:\RacingPoint\rc-agent.exe` and validates size > minimum threshold (Verifying)
7. Sends `cd /d C:\RacingPoint && start /b rc-agent.exe` via /exec (Starting)
8. Runs verify_restart logic: poll process + WS + lock screen at 5s, 15s, 30s, 60s (VerifyingHealth)
9. On all-pass: DeployState::Complete; on failure: DeployState::Failed { reason }

At each step, the executor broadcasts DashboardEvent::DeployProgress so the kiosk UI updates in real time.

### Pattern 3: Rolling Deploy with Canary

1. POST /api/deploy/rolling with binary_url
2. Deploy to Pod 8 first (canary)
3. Wait for Pod 8 to reach DeployState::Complete (or fail)
4. If Pod 8 fails: abort rolling deploy, report failure
5. If Pod 8 succeeds: deploy to remaining pods, skipping those with active billing
6. For each skipped pod: mark as "deferred" in deploy status
7. Optionally: set up a listener for BillingSessionChanged events to auto-deploy to deferred pods when sessions end

### Anti-Patterns to Avoid

- **Blocking the HTTP handler:** Deploy to a pod takes 60+ seconds (kill + download + verify). The POST handler must return immediately and run the deploy as a background task.
- **Overwriting running .exe:** Always verify process is dead before downloading/writing the new binary.
- **Deploying without HTTP server running:** The binary download requires the HTTP server on James (.27:9998). The deploy executor should verify the binary is downloadable before starting the kill sequence.
- **Ignoring partial failures:** If download fails after killing the old process, the pod has NO rc-agent. The executor must report this clearly as Failed state.
- **Racing multiple deploys to the same pod:** Use the DeployState in AppState to prevent concurrent deploys to the same pod.

---

## Common Pitfalls

### Pitfall 1: Binary download fails after killing old process

**What goes wrong:** Deploy kills rc-agent, deletes the binary, then download from .27:9998 fails (HTTP server not running, network issue). Pod now has no rc-agent binary at all.

**How to avoid:** Two options: (a) verify binary URL is reachable BEFORE killing the old process, or (b) accept the risk but report clearly as Failed state with "binary missing" reason. Option (a) is safer.

### Pitfall 2: `start /b rc-agent.exe` exits immediately

**What goes wrong:** The `start /b` command spawns rc-agent in the background and returns immediately. Pod-agent's /exec reports success (exit code 0) but rc-agent may crash on startup (bad config, missing DLL). The "success" from /exec is meaningless for the start step.

**How to avoid:** The start step's exit code is unreliable. The real verification is the VerifyingHealth phase -- wait for process alive + WS connected + lock screen responsive.

### Pitfall 3: File lock on rc-agent.exe even after taskkill

**What goes wrong:** `taskkill /F` sends SIGKILL but the process may take a few seconds to fully release the file handle (OS flushes buffers, antivirus scans the dying process).

**How to avoid:** After taskkill, poll `tasklist | findstr rc-agent` every 2s up to 10s. If still alive after 10s, fail the deploy. Then attempt delete -- if delete fails, retry once after 2s.

### Pitfall 4: Rolling deploy starts before HTTP server

**What goes wrong:** Staff clicks "Deploy All" but the binary HTTP server on .27:9998 is not running. All 8 deploys fail after killing the old process.

**How to avoid:** The deploy executor should attempt to HEAD/GET the binary_url before starting any kills. If the binary is not reachable, fail immediately with "binary not available at <url>".

### Pitfall 5: Race between deploy and watchdog restart

**What goes wrong:** Deploy is in progress (killing old rc-agent), pod_monitor detects heartbeat stale, triggers its own restart cycle. Two competing processes try to manage the pod simultaneously.

**How to avoid:** Set WatchdogState to a new value or reuse Restarting while deploy is in progress. Pod_monitor already skips pods in Restarting/Verifying state. Alternatively, add a DeployState check in pod_monitor's skip logic.

---

## Open Questions

1. **HTTP file server automation**
   - What we know: Binary download requires `python3 -m http.server 9998` running on James (.27). Currently started manually.
   - What's unclear: Should the deploy executor auto-start the HTTP server? Or should it assume it's already running?
   - Recommendation: The deploy executor should check if the binary URL is reachable. If not, return an error. Starting the HTTP server is a separate concern (could be a systemd/scheduled task, or staff starts it manually). Don't over-automate.

2. **Config update during deploy**
   - What we know: deploy_pod.py always writes a new config from the template. The deploy executor could do the same.
   - What's unclear: Should every deploy update the config, or only when config changes?
   - Recommendation: Always write config on deploy (matches deploy_pod.py behavior). The template is the source of truth. Config-only deploys can use the existing deploy_pod.py --config-only.

3. **Binary source for deploy executor**
   - What we know: deploy_pod.py downloads from `http://192.168.31.27:9998/rc-agent.exe`. The executor in racecontrol could serve the binary itself or use the same HTTP server.
   - What's unclear: Should racecontrol serve the binary from its own port (8080), or rely on the separate HTTP server?
   - Recommendation: Use the existing HTTP server pattern (9998 on James). racecontrol's job is orchestration, not file serving. The binary_url is a parameter to the deploy endpoint.

4. **Minimum binary size threshold**
   - What we know: rc-agent.exe is typically 15-25MB (static CRT build). A corrupted download could be 0 bytes or a few KB.
   - What's unclear: What's the right minimum size?
   - Recommendation: 5MB minimum threshold. Any rc-agent.exe smaller than 5MB is almost certainly corrupted. This catches 0-byte files, partial downloads, and HTML error pages saved as .exe.

5. **Deploy and WatchdogState interaction**
   - What we know: Pod_monitor skips pods in Restarting/Verifying WatchdogState. Deploy kills rc-agent, which will trigger heartbeat staleness.
   - What's unclear: Should deploy set WatchdogState to Restarting, or use a new DeployState that pod_monitor also checks?
   - Recommendation: Add DeployState check to pod_monitor's skip logic. When a pod has DeployState != Idle, pod_monitor skips it entirely. This is cleaner than overloading WatchdogState with deploy semantics.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | cargo test (Rust built-in) |
| Quick run | `export PATH="$PATH:/c/Users/bono/.cargo/bin" && cargo test -p rc-common && cargo test -p racecontrol-crate` |
| Full suite | Same + `cargo test -p rc-agent-crate` |

### Phase Requirements -> Test Map

| Req ID | Behavior | Test Type | File Exists? |
|--------|----------|-----------|-------------|
| DEPLOY-02 | DeployState serde roundtrip | unit | No (04-01) |
| DEPLOY-02 | DeployProgress event serde roundtrip | unit | No (04-01) |
| DEPLOY-02 | Deploy executor kills process before download | unit (mock) | No (04-02) |
| DEPLOY-02 | Deploy fails if process still alive after 10s | unit | No (04-02) |
| DEPLOY-02 | Deploy fails if binary size < 5MB threshold | unit | No (04-02) |
| DEPLOY-05 | Rolling deploy starts with Pod 8 as canary | unit | No (04-03) |
| DEPLOY-05 | Rolling deploy skips pods with active billing | unit | No (04-03) |
| PERF-01 | Game launch timing (manual verification) | manual | N/A |
| PERF-02 | PIN entry response time (manual verification) | manual | N/A |

---

## Sources

### Primary (HIGH confidence)

- Direct codebase inspection:
  - `deploy-staging/deploy_pod.py` -- current 5-step deploy sequence
  - `deploy-staging/deploy-cmd.json` -- ad-hoc compound command
  - `deploy-staging/install.bat` -- USB pendrive installer
  - `deploy-staging/rc-agent.template.toml` -- config template
  - `crates/racecontrol/src/pod_monitor.rs` -- verify_restart() pattern, WatchdogState skip logic
  - `crates/racecontrol/src/pod_healer.rs` -- WatchdogState skip pattern, billing check
  - `crates/racecontrol/src/state.rs` -- AppState structure, WatchdogState enum
  - `crates/rc-common/src/protocol.rs` -- DashboardEvent, DashboardCommand, AgentMessage enums
  - `crates/racecontrol/src/ws/mod.rs` -- WebSocket handler, agent registration, billing resync
  - `crates/racecontrol/src/billing.rs` -- BillingTimer, active_timers
  - `crates/racecontrol/src/wol.rs` -- restart_pod (machine reboot, not rc-agent restart)
  - `crates/racecontrol/src/api/routes.rs` -- existing API routes, POST /pods/{id}/restart

### Secondary (MEDIUM confidence)

- MEMORY.md -- pod network map, deploy process, deployment rules
- Phase 1 and Phase 2 research -- patterns for AppState fields, WatchdogState, email alerts

---

## Metadata

**Confidence breakdown:**
- Current workflow analysis: HIGH -- all findings from direct source code inspection
- Gap analysis: HIGH -- verified against actual codebase
- Architecture patterns: HIGH -- based on proven patterns in the same codebase
- Pitfalls: HIGH -- identified from production deploy experience documented in MEMORY.md

**Research date:** 2026-03-13
**Valid until:** 2026-04-13
