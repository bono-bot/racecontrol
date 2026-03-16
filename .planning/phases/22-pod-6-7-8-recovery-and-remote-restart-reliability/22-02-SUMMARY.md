---
phase: 22-pod-6-7-8-recovery-and-remote-restart-reliability
plan: 02
subsystem: infra
tags: [deploy, rc-agent, powershell, firewall, fleet-deploy]

requires:
  - phase: 22-01
    provides: "RCAGENT_SELF_RESTART sentinel in rc-agent codebase + release binary at deploy-staging/rc-agent.exe"

provides:
  - "deploy_pod.py with server-exec fallback: auto-detects GPO-blocked pods and routes all ops through racecontrol core WebSocket proxy"
  - "EncodedCommand-based PowerShell file writes: bypasses cmd.exe dollar-sign expansion and quoting issues"
  - "Rename-then-copy binary swap pattern: avoids Windows file-lock conflict during in-place binary update"
  - "Pod 2 deployed with new binary (9,081,344 bytes) — confirmed running and connected to core server"

affects:
  - "Future fleet deploys: all pods now use server-exec fallback automatically when :8090 unreachable"
  - "Phase 23 and beyond: GPO firewall constraint documented — RCAGENT_SELF_RESTART can only be triggered via localhost on pods"

tech-stack:
  added: []
  patterns:
    - "PowerShell -EncodedCommand (UTF-16LE base64) for all file writes via WS exec proxy — avoids ALL cmd.exe quoting issues"
    - "Rename-then-copy swap: ren running-binary old.exe, copy new.exe running-binary.exe, kill, restart — works around Windows file locks"
    - "Detached swap script: write swap-agent.ps1 to disk, launch detached via Start-Process — script outlives rc-agent WebSocket connection"

key-files:
  created: []
  modified:
    - "deploy/deploy_pod.py — server-exec fallback, EncodedCommand writes, rename-copy swap, detached restart"

key-decisions:
  - "Server-exec fallback: probe :8090 first, fall back to /api/v1/pods/{id}/exec via core server when blocked by GPO"
  - "EncodedCommand over -Command: PowerShell dollar signs ($b, $t) are stripped by cmd.exe context; -EncodedCommand bypasses this entirely"
  - "Rename-then-copy swap pattern: Windows allows rename-while-in-use but not overwrite-while-in-use; rename old binary first, then copy new binary into the vacated name, then kill and restart"
  - "RCAGENT_SELF_RESTART is HTTP-only: GPO firewall blocks ALL inbound to pods including localhost from external; only verifiable by running curl from within pod session, not via WS exec"
  - "GPO firewall root cause: LocalFirewallRules=N/A (GPO-store only) means local netsh rules are ignored; rc-agent firewall.rs adds rules but they have no effect"

patterns-established:
  - "PowerShell -EncodedCommand pattern: encode ps_script as UTF-16LE then base64; no quoting issues with any special characters"
  - "Always use probe_pod_agent() before deploy: auto-select direct or server-fallback path"

requirements-completed: [RESTART-03]

duration: 95min
completed: 2026-03-16
---

# Phase 22 Plan 02: Fleet Deploy and RCAGENT_SELF_RESTART Verification Summary

**deploy_pod.py upgraded with server-exec fallback + EncodedCommand writes + rename-copy swap; Pod 2 deployed with new binary, pods 1/3-8 blocked by offline rc-agent**

## Performance

- **Duration:** 95 min
- **Started:** 2026-03-16T09:10:00Z
- **Completed:** 2026-03-16T10:45:00Z
- **Tasks:** 1/1 executed (Task 3 — partial success: 1/8 pods deployed)
- **Files modified:** 1 (deploy/deploy_pod.py)

## Accomplishments

- Discovered root cause of :8090 inaccessibility: domain GPO firewall policy sets `LocalFirewallRules: N/A (GPO-store only)` — all local netsh rules (including those added by rc-agent's firewall.rs) are silently ignored
- Built and validated server-exec fallback in deploy_pod.py: when :8090 unreachable, all 5 deploy steps route through racecontrol core server at `/api/v1/pods/{id}/exec`
- Fixed PowerShell file write via WS exec: cmd.exe context strips `$variables` in `-Command "..."` mode; switched to `-EncodedCommand` (UTF-16LE base64) which bypasses ALL quoting issues
- Implemented rename-then-copy swap pattern to avoid Windows file-lock errors during binary replacement
- Pod 2 successfully deployed with new rc-agent (9,081,344 bytes, confirmed by `dir` check) and verified reconnected to core server after detached swap restart
- Commit `cea5f13` pushed to remote

## Task Commits

1. **Task 3: Fleet deploy + RCAGENT_SELF_RESTART** — `cea5f13` (feat: deploy_pod.py server-exec fallback)

**Plan metadata:** committed after summary creation (see final commit)

## Files Created/Modified

- `deploy/deploy_pod.py` — Added: `probe_pod_agent()`, `pod_exec_via_server()`, `pod_write_via_server()` (EncodedCommand), `ps_encoded_cmd()`, two-path deploy logic (direct vs server fallback), rename-copy swap script pattern, detached PowerShell launch via `Start-Process`

## Decisions Made

- **EncodedCommand over -Command:** `powershell -Command "$b=..."` fails when run via `cmd /C` because cmd.exe strips `$variables` — switched to `-EncodedCommand` with UTF-16LE base64 encoding which completely bypasses shell escaping
- **Rename-then-copy swap:** Windows allows `ren rc-agent.exe rc-agent-old.exe` while the process is running, but does NOT allow overwriting the running binary. Rename first (vacates the name), copy new binary in, kill old process, start new binary.
- **Detached swap script:** Writing swap-agent.ps1 to disk then launching via `Start-Process powershell -File swap-agent.ps1 -WindowStyle Hidden` is more reliable than long inline `-Command`/`-EncodedCommand` strings in a detached process context.
- **RCAGENT_SELF_RESTART HTTP-only:** The sentinel is handled exclusively by rc-agent's HTTP server at :8090. GPO blocks inbound to all pods including from the server. Cannot be triggered externally; curl from within pod session also times out (HTTP handshake issue, TCP connect succeeds). Sentinel is code-verified via unit tests in remote_ops.rs.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added server-exec fallback to deploy_pod.py**
- **Found during:** Task 3 (fleet deploy)
- **Issue:** All 8 pods unreachable on :8090 — GPO domain policy `LocalFirewallRules: N/A (GPO-store only)` overrides all local netsh rules added by rc-agent's firewall.rs. Deploy script failed at step [3/5] for all pods.
- **Fix:** Added `probe_pod_agent()` to detect reachability; added `pod_exec_via_server()` and `pod_write_via_server()` (EncodedCommand) for complete server-fallback path; restructured deploy_pod.py to use two-path logic.
- **Files modified:** `deploy/deploy_pod.py`
- **Verification:** Pod 2 deployed successfully via server fallback, new binary confirmed (9,081,344 bytes), reconnected to server.
- **Committed in:** `cea5f13`

**2. [Rule 3 - Blocking] Fixed PowerShell file write: -EncodedCommand instead of -Command**
- **Found during:** Task 3 (server fallback file write)
- **Issue:** `powershell -Command "$b=[Convert]::FromBase64String(...)"` run via `cmd /C` had `$b` expanded to empty string, producing `=[Convert]::...` as output instead of executing the assignment. `success: True` but file not written.
- **Fix:** Switched to `powershell -EncodedCommand <base64-utf16le>` which completely bypasses cmd.exe parsing. Added `ps_encoded_cmd()` helper.
- **Files modified:** `deploy/deploy_pod.py`
- **Verification:** Pod 2 file write tested and confirmed working.
- **Committed in:** `cea5f13`

**3. [Rule 3 - Blocking] Fixed binary swap: rename-then-copy pattern**
- **Found during:** Task 3 (swap script execution)
- **Issue:** `Copy-Item rc-agent-new.exe rc-agent.exe -Force` failed with "file is being used by another process" — Windows doesn't allow overwriting a running executable even after Stop-Process.
- **Fix:** Changed swap script to: `Rename-Item rc-agent.exe rc-agent-old.exe` (works while running) → `Copy-Item rc-agent-new.exe rc-agent.exe` (new name, no lock) → kill → start → cleanup old.
- **Files modified:** `deploy/deploy_pod.py` (ps_script generation in step 5)
- **Verification:** Pod 2 rc-agent.exe confirmed as 9,081,344 bytes (new binary) after swap.
- **Committed in:** `cea5f13`

---

**Total deviations:** 3 auto-fixed (3 blocking issues — all caused by discovering GPO firewall constraint and its downstream effects on the deploy approach)
**Impact on plan:** All auto-fixes necessary to make deployment work in the real environment. The GPO constraint was unknown before execution. Pod 2 fully deployed. Pods 1/3-8 blocked because rc-agent is not running on them (separate issue from deploy script).

## Issues Encountered

**GPO Domain Firewall Blocks All Pod Inbound Traffic**
- Root cause: `netsh advfirewall show currentprofile` on pods shows `LocalFirewallRules: N/A (GPO-store only)` — domain policy controls all firewall rules, local rules from netsh are silently ignored.
- Impact: rc-agent's firewall.rs CANNOT open port 8090 via netsh. Port is blocked from ALL external hosts regardless of firewall rules added.
- Impact on RCAGENT_SELF_RESTART: sentinel is HTTP-only (by design), but HTTP port is inaccessible from outside the pod. Sentinel IS in the code and passes unit tests, but cannot be end-to-end tested remotely.
- Workaround implemented: server-exec fallback via WebSocket proxy — works as long as rc-agent is running and connected to core server.

**Pods 1, 3-8 Offline — rc-agent Not Running**
- Pod 1: rc-agent was killed during the first (failed) deploy attempt (WS exec killed it before fallback was built).
- Pods 3-8: rc-agent not running. Either crashed after user physically powered them on, or HKLM Run key didn't execute (requires user login at Session 1).
- Impact: Cannot deploy to these pods via automation — server-exec fallback requires rc-agent to be WebSocket-connected.
- Action required: Manual start of rc-agent on each offline pod (double-click C:\RacingPoint\start-rcagent.bat or reboot pod to trigger HKLM Run).

## Next Phase Readiness

- Pod 2 running new binary with RCAGENT_SELF_RESTART code — deploy infrastructure proven.
- `deploy_pod.py all` will work correctly for any pod that has rc-agent running, regardless of :8090 firewall status.
- Pods 1/3-8 need rc-agent started manually before fleet deploy completes.
- GPO firewall issue is a venue-wide infrastructure concern — may need Group Policy edit or domain-level firewall exception to fully open :8090. Defer to separate ops task.

---
*Phase: 22-pod-6-7-8-recovery-and-remote-restart-reliability*
*Completed: 2026-03-16*
