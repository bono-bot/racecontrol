# Phase 6 — Diagnostic Findings

**Collected:** 2026-03-13
**Method:** Remote via pod-agent (POST /exec on port 8090) from James (.27)

---

## DIAG-01: rc-agent Logs and Error Patterns

### Log File Status

| Pod | IP | Log File | rc-agent Running | Debug Server (18924) |
|-----|-----|----------|-----------------|---------------------|
| 1 | .89 | NO_LOG_FILE | Unknown | Timed out (unreachable) |
| 2 | .33 | NO_LOG_FILE | Unknown | Timed out (unreachable) |
| 3 | .28 | NO_LOG_FILE | Yes | OK (lock_screen_state: disconnected) |
| 4 | .88 | NO_LOG_FILE | Unknown | Timed out (unreachable) |
| 5 | .86 | NO_LOG_FILE | Unknown | Timed out (unreachable) |
| 6 | .87 | NO_LOG_FILE | Unknown | Timed out (unreachable) |
| 7 | .38 | NO_LOG_FILE | Unknown | Timed out (unreachable) |
| 8 | .91 | YES (full log captured) | Yes (with errors) | Failed to bind (port 10048) |

### Key Finding: No Persistent Log Files on Pods 1-7

Pods 1-7 have no `C:\RacingPoint\rc-agent-log.txt` file. rc-agent writes to stdout/stderr, which is only captured if the start script redirects output. Only Pod 8's `start-rcagent.bat` appears to redirect to a file.

**Impact:** Without log files, diagnosing past failures on Pods 1-7 requires either catching them live or deploying a logging start script.

### Pod 3 Debug Server Response

Pod 3 is the only pod with a responsive debug server:
```
{"pod":"Pod 3","pod_number":3,"lock_screen_state":"disconnected","debug_server":"ok"}
```
**Lock screen state: disconnected** — confirms the lock screen is showing the "disconnected" state, not connected to rc-core.

### Pod 8 Full Log Analysis (Critical Findings)

Pod 8 had a complete startup log captured at 2026-03-13T04:56:44Z. Key patterns:

**1. Successful startup sequence:**
- Config loaded from rc-agent.toml
- Pod #8 identified, sim: assetto_corsa
- Lock screen server bound to http://127.0.0.1:18923
- Kiosk mode ACTIVATED (Win/Alt+Tab/Alt+F4 blocked)
- Overlay: native Win32 mode

**2. UDP port conflicts (error 10048 — AddrInUse):**
- Port 5555 (LMU) — FAILED
- Port 9996 (AC) — FAILED
- Port 20777 (F1) — FAILED
- Port 5300 (Forza) — FAILED
- Port 6789 (iRacing) — FAILED

All 5 telemetry UDP ports failed with "Only one usage of each socket address" — suggests a previous rc-agent instance is still holding these ports, or another process is occupying them.

**3. Debug server failed to bind port 18924** — same error 10048. Previous instance still holding the port.

**4. rc-core unreachable:**
- WebSocket target: ws://192.168.31.23:8080/ws/agent
- Connection timed out on attempts 0-6+ with exponential backoff (1s, 1s, 1s, 2s, 4s, 8s, 16s)
- UDP heartbeat also reported rc-core unreachable after 6s

**5. Watchdog crash loop:**
- Watchdog detected pod-agent.exe not running and tried to restart
- pod-agent v0.4.0 started but immediately panicked with `unwrap()` on port 10048 — port 8090 already in use by the running instance
- This cycle repeated 3 times in 2 minutes

**6. Shared memory warning:**
- "Failed to open shared memory: Local\acpmf_physics" — expected when Assetto Corsa is not running

### Error Pattern Summary

| Pattern | Affected | Root Cause | Phase Fix |
|---------|----------|------------|-----------|
| No persistent log file | Pods 1-7 | start script doesn't redirect stdout | Deploy logging start script |
| rc-core unreachable (WebSocket timeout) | Pod 8 (likely all) | rc-core on .23:8080 not running or unreachable | Phase 7 (ensure rc-core auto-starts) |
| Lock screen state: disconnected | Pod 3 (confirmed), likely all | No WebSocket connection to rc-core | Phase 7 + Phase 8 |
| UDP port conflicts (10048) | Pod 8 | Multiple rc-agent instances or stale sockets | Existing auto-fix in ai_debugger.rs |
| Debug server port conflict | Pod 8 | Same as above — dual instance | Same |
| Watchdog crash loop (pod-agent) | Pod 8 | unwrap() on AddrInUse — pod-agent already running | Pod 8 has v0.4.0 (known bug, v0.5.0 fixes) |

---

## DIAG-03: Edge Browser Settings Baseline

### Edge Version

| Pod | IP | Edge Version |
|-----|-----|-------------|
| 1 | .89 | 145.0.3800.97 |
| 2 | .33 | 145.0.3800.97 |
| 3 | .28 | 145.0.3800.97 |
| 4 | .88 | 145.0.3800.97 |
| 5 | .86 | 145.0.3800.97 |
| 6 | .87 | 145.0.3800.97 |
| 7 | .38 | 145.0.3800.97 |
| 8 | .91 | 145.0.3800.97 |

**All 8 pods are on the same Edge version: 145.0.3800.97** — consistent fleet.

### Registry Policy Settings

| Pod | IP | StartupBoost (HKLM) | StartupBoost (HKCU) | BackgroundMode (HKLM) | BackgroundMode (HKCU) | EdgeUpdate Svc | edgeupdate Svc |
|-----|-----|---------------------|---------------------|----------------------|----------------------|----------------|----------------|
| 1 | .89 | not set | not set | not set | not set | STOPPED | STOPPED |
| 2 | .33 | not set | not set | not set | not set | STOPPED | STOPPED |
| 3 | .28 | not set | not set | not set | not set | STOPPED | STOPPED |
| 4 | .88 | not set | not set | not set | not set | STOPPED | STOPPED |
| 5 | .86 | not set | not set | not set | not set | STOPPED | STOPPED |
| 6 | .87 | not set | not set | not set | not set | STOPPED | STOPPED |
| 7 | .38 | not set | not set | not set | not set | STOPPED | STOPPED |
| 8 | .91 | not set | not set | not set | not set | STOPPED | STOPPED |

**Interpretation:**
- **"not set"** means the registry key `HKLM\SOFTWARE\Policies\Microsoft\Edge` does not contain the value — the setting is at its **default (ENABLED)**. This is NOT the same as "disabled."
- **EdgeUpdate service:** STOPPED on all pods but NOT disabled. It can restart on its own (e.g., at login, via scheduled task).
- **MicrosoftEdgeUpdate:** Does not exist as an installed service on any pod (error 1060).

### Phase 9 Remediation Needed

All 8 pods need the following Phase 9 changes:
1. **StartupBoostEnabled = 0** in `HKLM\SOFTWARE\Policies\Microsoft\Edge` (currently: not set = default enabled)
2. **BackgroundModeEnabled = 0** in `HKLM\SOFTWARE\Policies\Microsoft\Edge` (currently: not set = default enabled)
3. **EdgeUpdate service: DISABLED** via `sc config EdgeUpdate start= disabled` (currently: STOPPED but can auto-start)
4. **edgeupdate service: DISABLED** via `sc config edgeupdate start= disabled` (currently: STOPPED but can auto-start)

---

## DIAG-02: Server Port Audit

*Pending — requires RDP to Server (.23). See Plan 06-02.*

---

## DIAG-04: Server IP and MAC

*Pending — requires RDP to Server (.23). See Plan 06-02.*

---

## Phase 7 Prerequisites (Partial — awaiting DIAG-02/04)

| Prerequisite | Status | Value |
|-------------|--------|-------|
| Server MAC address | PENDING | Needs RDP to server |
| DHCP reservation needed? | PENDING | Needs RDP to server |
| Port 3300 free on server? | PENDING | Needs server port audit |
| rc-core running on 8080? | PENDING | Needs server port audit |
| Edge version consistent? | CONFIRMED | 145.0.3800.97 on all 8 pods |
| StartupBoost needs disabling? | CONFIRMED | All 8 pods (not set = default enabled) |
| BackgroundMode needs disabling? | CONFIRMED | All 8 pods (not set = default enabled) |
| EdgeUpdate needs disabling? | CONFIRMED | All 8 pods (STOPPED but not disabled) |
| rc-core reachable from pods? | NO | Pod 8 logs show WebSocket timeout to .23:8080 |
| Log files available? | NO | Only Pod 8 has logs; Pods 1-7 need logging script |
