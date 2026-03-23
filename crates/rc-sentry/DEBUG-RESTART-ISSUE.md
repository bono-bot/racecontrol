# BUG: rc-sentry restart_service() Cannot Launch rc-agent From Non-Interactive Context

**Date:** 2026-03-23
**Severity:** CRITICAL — pods stay dead after rc-agent crash until manual reboot
**Affects:** All 8 pods. rc-sentry build a6894d34, rc-agent build 82bea1eb/3bddbf5d
**Status:** ROOT CAUSE CONFIRMED, FIX NOT WORKING — needs parallel debug session

---

## Symptom

When rc-agent crashes on a pod, rc-sentry detects the crash (watchdog, 15s hysteresis) and calls `restart_service()`, but **rc-agent never actually starts**. The pod stays dead until manually rebooted or an admin runs `schtasks /Run /TN StartRCAgent` via rc-sentry's `/exec` endpoint.

## Root Cause Chain

1. rc-sentry runs as a **non-interactive process** (launched from HKLM Run key or schtasks, no interactive desktop session)
2. From this context, **no known method reliably launches a process into the interactive desktop on Windows**:
   - `cmd /C start + CREATE_NO_WINDOW (0x08000000)` — `.spawn()` returns Ok, child never starts
   - `PowerShell Start-Process + DETACHED_PROCESS (0x08)` — `.spawn()` returns Ok, child never starts
   - `schtasks /Create + /Run` via `Command::new("schtasks") + CREATE_NO_WINDOW` — task created, **but never runs**
3. The **same schtasks command works** when called through rc-sentry's `/exec` endpoint (which uses `cmd /C`)
4. This means the issue is specifically with **Rust's `std::process::Command` launching schtasks with `creation_flags`**

## What Was Tested (Cause Elimination)

| # | Method | Code Location | Result | Evidence |
|---|--------|---------------|--------|----------|
| 1 | `cmd /C start + CREATE_NO_WINDOW` | tier1_fixes.rs (build a7230e36) | **FAIL** — silently fails | Deployed on pods for weeks, never revived rc-agent |
| 2 | `PowerShell Start-Process + DETACHED_PROCESS` | tier1_fixes.rs (build 3bddbf5d) | **FAIL** — silently fails | E2E test: killed rc-agent, waited 50s, stayed dead |
| 3 | `schtasks via Command::new + CREATE_NO_WINDOW` | tier1_fixes.rs (build a6894d34) | **FAIL** — task created but never executes | E2E test: killed rc-agent, maintenance cleared, waited 35s, stayed dead |
| 4 | `schtasks via rc-sentry /exec endpoint` (cmd /C) | Manual via curl | **WORKS** | `schtasks /Run /TN StartRCAgent` → rc-agent starts in <5s |
| 5 | `start-rcagent.bat via schtasks (SYSTEM)` | Manual via SSH on .23 | **WORKS** | Server racecontrol.exe started via schtasks successfully |

## Key Difference: Method 3 vs 4

Both run `schtasks /Run /TN StartRCAgent`. The difference:

- **Method 3 (FAILS):** `std::process::Command::new("schtasks").args([...]).creation_flags(0x08000000).output()`
  - Runs inside rc-sentry's Rust process
  - Has CREATE_NO_WINDOW flag
  - rc-sentry itself was launched from HKLM Run key (non-interactive)

- **Method 4 (WORKS):** rc-sentry receives HTTP POST to `/exec` → `cmd /C schtasks /Run /TN StartRCAgent`
  - Runs via `rc_common::exec::run_cmd_sync()` which uses `cmd /C`
  - No CREATE_NO_WINDOW flag on the outer cmd process (or different process creation)

## Hypotheses for Parallel Session

**H1: CREATE_NO_WINDOW (0x08000000) flag blocks schtasks from running tasks**
- Test: Remove creation_flags entirely from `restart_service()`, rebuild, deploy to Pod 8, kill rc-agent, observe
- Expected: If this is the cause, schtasks will work without the flag

**H2: rc-sentry's process token lacks `SeIncreaseQuotaPrivilege` or `SeBatchLogonRight`**
- Test: Check rc-sentry's process token privileges on a pod
- `whoami /priv` from rc-sentry exec vs from interactive session

**H3: schtasks /Run requires interactive session for the calling process**
- Test: Run schtasks from a `schtasks /Create /RU SYSTEM` scheduled task (not from HKLM Run)
- This tests if the issue is the HKLM Run launch context vs SYSTEM service context

**H4: Use rc-sentry's own /exec endpoint to self-invoke schtasks (workaround)**
- rc-sentry's `/exec` endpoint → `cmd /C schtasks /Run /TN StartRCAgent`
- This is proven to work — if internal Command fails, make restart_service() call its own HTTP endpoint
- Code: `reqwest::blocking::Client::post("http://127.0.0.1:8091/exec").json({"cmd":"schtasks /Run /TN StartRCAgent"})`

**H5: The watchdog crash handler thread may be panicking before reaching restart_service()**
- Test: Add a file-write breadcrumb at the TOP of handle_crash() and at restart_service() entry
- `std::fs::write("C:\\RacingPoint\\sentry-breadcrumb-1.txt", "handle_crash entered")`
- If breadcrumb-1 exists but breadcrumb-2 (restart_service entry) doesn't, something between them panics

## Code Paths

- **Watchdog:** `crates/rc-sentry/src/watchdog.rs:172-244` — polls /health every 5s, 3-poll hysteresis, sends CrashContext to channel
- **Crash handler:** `crates/rc-sentry/src/main.rs:117-256` — receives CrashContext, runs tier1_fixes, Ollama, restart
- **handle_crash:** `crates/rc-sentry/src/tier1_fixes.rs:378-464` — maintenance check, sentinels, 6 fixes, escalation, restart
- **restart_service:** `crates/rc-sentry/src/tier1_fixes.rs:278-312` — the broken code
- **Sentry config:** `crates/rc-sentry/src/sentry_config.rs` — defaults to rc-agent on :8090, start script = start-rcagent.bat

## Sentinel Files That Block Restart

- `C:\RacingPoint\MAINTENANCE_MODE` — stops ALL restarts. Created after 3 restarts in 10 min.
- `C:\RacingPoint\GRACEFUL_RELAUNCH` — marks self-initiated restart (not a crash). Skips escalation counter.
- `C:\RacingPoint\rcagent-restart-sentinel.txt` — marks deploy restart. Skips escalation counter.

**IMPORTANT:** Before testing, always clear ALL three files on the target pod:
```
del C:\RacingPoint\MAINTENANCE_MODE C:\RacingPoint\GRACEFUL_RELAUNCH C:\RacingPoint\rcagent-restart-sentinel.txt 2>nul
```

## Current State of Pods

| Pod | rc-sentry build | rc-agent build | Notes |
|-----|----------------|----------------|-------|
| 1-8 | a6894d34 (schtasks) | 82bea1eb (old) or 3bddbf5d (Pod 8) | rc-agent-new.exe waiting on all pods |
| Pod 8 | a6894d34 | 3bddbf5d | Test pod — was killed and revived via manual schtasks |

## How to Test (Step by Step)

1. SSH or exec into a pod: `curl -s -X POST http://<pod_ip>:8091/exec -d '{"cmd":"..."}'`
2. Clear sentinels: `del C:\RacingPoint\MAINTENANCE_MODE C:\RacingPoint\GRACEFUL_RELAUNCH C:\RacingPoint\rcagent-restart-sentinel.txt 2>nul`
3. Verify rc-agent is running: `curl http://<pod_ip>:8090/health`
4. Verify rc-sentry is running: `curl http://<pod_ip>:8091/health`
5. Kill rc-agent: `curl -X POST http://<pod_ip>:8091/exec -d '{"cmd":"taskkill /F /IM rc-agent.exe"}'`
6. Wait 35s (15s hysteresis + 5s backoff + 15s buffer)
7. Check rc-agent: `curl http://<pod_ip>:8090/health` — should return health if fix works

## Standing Rule Lessons

1. **`.spawn().is_ok()` does NOT mean the child process started.** On Windows, spawn() returning Ok only means the OS accepted the CreateProcess call, not that the target executable is running. ALWAYS verify the actual process is alive after spawn (poll /health, check tasklist).

2. **Non-interactive Windows process context is fundamentally hostile to launching interactive processes.** There is no reliable single method. The workaround landscape: HKLM Run key (boot only), schtasks /RU SYSTEM (works from interactive callers), Windows Service SCM (requires service registration), named pipe to an interactive helper. Each has constraints.

3. **Test the EXACT restart path, not a proxy.** We tested `schtasks /Run` from the `/exec` endpoint and declared it working. But `/exec` runs via `cmd /C` which has different process creation flags than `Command::new("schtasks")` in Rust. The proxy test passed; the actual path failed.

4. **MAINTENANCE_MODE is a silent pod killer.** Once set, all restarts stop permanently until manually cleared. There is no timeout, no auto-clear, no alert. A pod can silently enter maintenance mode and stay dead indefinitely. The AI healer on James does not check for MAINTENANCE_MODE on pods.
