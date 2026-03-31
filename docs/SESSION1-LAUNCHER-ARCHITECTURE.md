# Session 1 Launcher Architecture

**Status:** PROPOSED (MMA audit recommendation, 6-model consensus, 2026-03-29)
**Priority:** HIGH — eliminates reboot-required deploys
**Effort:** Medium (new Rust binary + IPC + bat changes)

## Problem

rc-agent requires Session 1 (interactive desktop) for GUI operations: Edge browser,
game launching, ConspitLink, overlay HUD, taskbar control, keyboard hooks.

Currently, the only reliable way to start rc-agent in Session 1 is:
- **HKLM Run key** at boot (`start-rcagent.bat`)
- This means **every deploy requires a full Windows reboot**

All other restart paths land in Session 0:
- `RCWatchdog` service (uses WTSQueryUserToken + CreateProcessAsUser — silently fails)
- `schtasks /Run /TN StartRCAgent` (runs as SYSTEM)
- rc-sentry exec `start start-rcagent.bat` (inherits Session 0 from sentry service)

### Root Cause (MMA 6-model consensus)

WTSQueryUserToken + CreateProcessAsUser fails silently because:
1. **No active console session** — if console is locked/at login screen,
   `WTSGetActiveConsoleSessionId()` returns Session 0
2. **Missing privileges** — service needs `SeAssignPrimaryTokenPrivilege`,
   `SeIncreaseQuotaPrivilege`, and `SeTcbPrivilege`
3. **No DuplicateTokenEx** — raw token passed without primary token duplication
4. **lpDesktop not set** — must be `winsta0\default` for interactive GUI
5. **Silent fallback** — on failure, code falls back to CreateProcess in own session

## Proposed Architecture

### Components

```
+---------------------------+     Named Pipe / TCP     +-------------------------+
| RCWatchdog (Session 0)    | <---------------------> | RCAgentLauncher (Ses 1)  |
| Windows Service           |   IPC: restart/deploy    | User-space process       |
| Monitors rc-agent health  |                          | Owns rc-agent lifecycle  |
| Downloads new binaries    |                          | Binary swap + start      |
| Decides when to restart   |                          | Rollback on failure      |
+---------------------------+                          +------+------------------+
                                                              |
                                                              | spawn/kill
                                                              v
                                                    +-------------------+
                                                    | rc-agent (Ses 1)  |
                                                    | GUI operations    |
                                                    +-------------------+
```

### Session 1 Launcher (new binary: `rc-launcher.exe`)

**Lifecycle:**
1. Started by HKLM Run key at login (same as current start-rcagent.bat)
2. Runs as the interactive user in Session 1
3. Starts rc-agent as a child process
4. Listens on a named pipe (`\\.\pipe\RCAgentLauncher`) for commands
5. If rc-agent crashes, restarts it (already in Session 1 — no token issues)
6. If watchdog sends "deploy" command via pipe, performs binary swap + restart

**IPC Commands:**
- `RESTART` — kill rc-agent, run start-rcagent.bat, verify health
- `DEPLOY <hash>` — swap binary (hash → rc-agent.exe, old → prev), restart, verify
- `ROLLBACK` — swap prev → rc-agent.exe, restart
- `STATUS` — report rc-agent PID, session, uptime, build_id
- `SHUTDOWN` — graceful stop of rc-agent + launcher

**Key Properties:**
- Always runs in Session 1 (started at login)
- Singleton (named mutex: `Global\RCAgentLauncher`)
- Lightweight (~50KB binary, no GUI, no network)
- Crash-resilient: if launcher crashes, HKLM Run restarts it at next login
- RCWatchdog detects launcher health via named pipe ping

### RCWatchdog Changes

Current flow:
```
Watchdog detects rc-agent down → WTSQueryUserToken → CreateProcessAsUser → FAIL (Session 0)
```

New flow:
```
Watchdog detects rc-agent down → Send RESTART via named pipe → Launcher restarts in Session 1
```

If launcher is also dead:
```
Watchdog detects launcher dead → Reboot pod (HKLM Run restarts both)
```

### Deploy Flow (new)

```
deploy-pod.sh → download binary to pod via rc-sentry exec
             → send DEPLOY <hash> via named pipe to launcher
             → launcher swaps binary + restarts rc-agent
             → launcher reports new build_id via STATUS
             → deploy-pod.sh verifies build_id
```

No reboot required.

## Implementation Plan

### Phase 1: rc-launcher.exe (new crate)

Create `crates/rc-launcher/`:
- `main.rs` — Entry point, named pipe server, singleton mutex
- `ipc.rs` — Named pipe protocol (line-delimited text commands)
- `lifecycle.rs` — rc-agent spawn, kill, health check, binary swap
- `Cargo.toml` — Minimal deps: `winapi` (named pipes, process), `serde_json` (status)

Key Win32 APIs:
- `CreateNamedPipeW` for `\\.\pipe\RCAgentLauncher`
- `CreateProcessW` for rc-agent (inherits Session 1 from launcher)
- `CreateMutexW` for singleton (`Global\RCAgentLauncher`)

### Phase 2: RCWatchdog modifications

Modify `crates/rc-sentry/src/watchdog.rs`:
- Replace WTSQueryUserToken + CreateProcessAsUser path with named pipe client
- Add `ConnectNamedPipe` / `CallNamedPipeW` to send RESTART
- Fallback: if named pipe fails, schedule reboot

### Phase 3: Deploy script + bat updates

- Update `start-rcagent.bat` → `start-rclauncher.bat` (starts launcher, not agent directly)
- Update HKLM Run key: `RCAgent` → `RCLauncher`
- Update `deploy-pod.sh` to use named pipe DEPLOY command
- Preserve current bat-based flow as fallback for pods without launcher

### Phase 4: Fleet migration

1. Build rc-launcher.exe
2. Deploy to Pod 8 canary (alongside current setup)
3. Test: kill rc-agent → verify launcher restarts it in Session 1
4. Test: send DEPLOY command → verify binary swap without reboot
5. Roll out to fleet
6. Remove WTSQueryUserToken code from watchdog

## Alternative Approaches (rejected)

### Per-user scheduled task "At logon"
- Task Scheduler COM API can trigger a task in the user's session
- Rejected: fragile COM interop, task scheduler bugs on Windows 11

### Windows AssignedAccess / Kiosk Mode
- Eliminates explorer entirely, runs single app
- Rejected: too restrictive for debugging/remote access, hard to update

### Custom shell (replace explorer.exe)
- HKLM\...\Winlogon\Shell → rc-launcher.exe
- Rejected: loses file manager, DeskIn remote access, debugging tools

## Risks

1. **Named pipe security** — must validate caller (PID → service name check)
2. **Launcher crash** — loses rc-agent restart capability until reboot
3. **Migration complexity** — two restart paths during transition period
4. **Binary size** — another EXE to deploy and version

## Estimated Effort

| Component | Lines of Code | Time |
|-----------|--------------|------|
| rc-launcher crate | ~400 | 1 phase |
| Watchdog IPC client | ~100 | 0.5 phase |
| Deploy script updates | ~50 | 0.5 phase |
| Fleet migration + testing | — | 1 phase |
| **Total** | **~550** | **3 phases** |

## References

- MMA Deploy Audit (2026-03-29): 6 models, 2 iterations, convergence on Session 1 launcher
- Standing rule: "rc-agent MUST run in Session 1" (CLAUDE.md)
- Standing rule: "NEVER restart rc-agent via schtasks" (CLAUDE.md)
- Win32 API: WTSQueryUserToken, CreateProcessAsUser, CreateNamedPipeW
- Incident: 2026-03-29 deploy required 8 pod reboots due to Session 0 restart failure
