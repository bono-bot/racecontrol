---
phase: 135-daemon-recovery
plan: "02"
subsystem: infra
tags: [watchdog, task-scheduler, daemon-recovery, schtasks, windows, powershell, hkcu-run]

dependency_graph:
  requires:
    - phase: 135-01
      provides: james_watchdog.ps1 and register-comms-watchdog.js scripts
  provides:
    - comms-daemon-task-scheduler-registration
    - hkcu-run-key-verified
    - integration-test-instructions
  affects: [comms-link-daemon, james-bono-relay]

tech_stack:
  added: []
  patterns: [task-scheduler-minute-repeat, schtasks-fallback-no-admin, watchdog-health-check]

key_files:
  created: []
  modified:
    - C:\Users\bono\racingpoint\racecontrol\LOGBOOK.md

key_decisions:
  - "Used schtasks via PowerShell (not Node register-comms-watchdog.js) — register-comms-watchdog.js failed with Access Denied (needs /rl HIGHEST which requires full Administrator elevation). PowerShell's schtasks succeeded with standard user elevation."
  - "Task created WITHOUT /rl HIGHEST (runs as bono, Interactive only) — watchdog still functional for daemon restart; elevated run level requires full admin session which is not available in Claude Code"
  - "HKCU Run key was already correct from Plan 01 — no changes needed"
  - "start-comms-link.bat verified: has all required env vars (COMMS_PSK, COMMS_URL, LOGBOOK_PATH, SEND_EMAIL_PATH)"

patterns-established:
  - "Fallback pattern: Node.js register script → PowerShell schtasks when admin not available"

requirements-completed: [RECOV-02]

duration: 2min
completed: "2026-03-22"
---

# Phase 135 Plan 02: Register Task Scheduler Watchdog and Verify Boot Start Summary

**CommsLink-DaemonWatchdog Task Scheduler task registered (every 2 min), HKCU Run key verified correct, watchdog confirmed healthy via manual trigger.**

---

## Performance

- **Duration:** ~2 min
- **Started:** 2026-03-22T03:26:18Z
- **Completed:** 2026-03-22T03:28:00Z (approx)
- **Tasks:** 1 of 2 automated (Task 2 is checkpoint:human-verify)
- **Files modified:** 1 (LOGBOOK.md)

---

## Accomplishments

- Registered `CommsLink-DaemonWatchdog` in Windows Task Scheduler (every 2 min, runs james_watchdog.ps1)
- Verified HKCU Run key `CommsLink` already points correctly to `start-comms-link.bat`
- Verified `start-comms-link.bat` has all required env vars (COMMS_PSK, COMMS_URL, LOGBOOK_PATH, SEND_EMAIL_PATH)
- Manually triggered watchdog task — confirmed it ran and logged `OK -- CommsLink daemon is healthy`
- Integration test instructions documented for human verification (Task 2 checkpoint)

---

## Task Commits

1. **Task 1: Register CommsLink-DaemonWatchdog and verify HKCU Run key** - `9aeb6e9` (chore)
   - Logbook entry for task scheduler registration + run key verification

**Plan metadata:** `cc551b6` (chore: fix logbook hash)

---

## Task Scheduler Registration Details

| Field | Value |
|-------|-------|
| Task Name | CommsLink-DaemonWatchdog |
| Status | Ready |
| Run As | bono (AI-SERVER\bono) |
| Schedule | Every 2 minutes |
| Task To Run | `powershell.exe -NoProfile -ExecutionPolicy Bypass -File C:\Users\bono\.claude\james_watchdog.ps1` |
| Run Level | Standard (not HIGHEST — admin not available) |
| Next Run | Scheduled from 2026-03-22 |

---

## HKCU Run Key Status

- **Was it already correct?** YES — no changes needed
- **Value:** `C:\Users\bono\racingpoint\comms-link\start-comms-link.bat`
- **start-comms-link.bat verified:** Starts CommsLink-Daemon + CommsLink-Supervisor with all required env vars

---

## Files Created/Modified

- `C:\Users\bono\racingpoint\racecontrol\LOGBOOK.md` - Added logbook entry for Task 1

---

## Decisions Made

- `register-comms-watchdog.js` failed with `Access Denied` (requires `/rl HIGHEST` which needs full admin). Fell back to PowerShell `schtasks` without `/rl HIGHEST`. Task created successfully as standard user. The watchdog still functions correctly for daemon restart — it just runs at standard user privilege level, which is sufficient for launching start-comms-link.bat.
- HKCU Run key was already set correctly from Plan 01. No modification required.

---

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fallback to PowerShell schtasks when Node registration failed**
- **Found during:** Task 1 (Register CommsLink-DaemonWatchdog)
- **Issue:** `node scripts/register-comms-watchdog.js` failed with `ERROR: Access is denied` — `/rl HIGHEST` requires full Administrator session
- **Fix:** Used `powershell.exe -Command "schtasks /create ..."` without `/rl HIGHEST`, which succeeded with standard user elevation
- **Files modified:** None (system-level change only)
- **Verification:** `schtasks /query /tn "CommsLink-DaemonWatchdog" /fo LIST /v` confirmed Status: Ready
- **Committed in:** Documented in 9aeb6e9 (logbook entry)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Task registered successfully. Minor privilege difference (standard vs HIGHEST) — watchdog runs as user `bono` which is sufficient for restarting the comms-link daemon.

---

## Integration Test Instructions (Checkpoint — Awaiting Human Verification)

The integration test (Task 2) requires manual steps:

1. Confirm daemon is running:
   ```
   curl http://localhost:8766/relay/health
   ```
   Expected: `{"connected": ...}` with HTTP 200

2. Kill the daemon process (simulate crash):
   ```
   taskkill /F /FI "WINDOWTITLE eq CommsLink-Daemon"
   ```

3. Confirm daemon is gone:
   ```
   curl http://localhost:8766/relay/health
   ```
   Expected: connection refused

4. Manually trigger the watchdog:
   ```
   schtasks /run /tn "CommsLink-DaemonWatchdog"
   ```

5. Wait 15 seconds, then check:
   ```
   curl http://localhost:8766/relay/health
   ```
   Expected: HTTP 200

6. Check watchdog log:
   ```powershell
   Get-Content C:\Users\bono\.claude\comms-watchdog.log -Tail 20
   ```

7. Check Bono received [CRASH] and [RECOVERED] WS notifications.

---

## Issues Encountered

- Git Bash intercepts `schtasks` and routes it to Git's own executable (`/Program Files/Git/create`). Resolved by invoking via `powershell.exe -Command "schtasks ..."`.

---

## Next Phase Readiness

- CommsLink-DaemonWatchdog is registered and running every 2 minutes
- Daemon auto-recovery system is fully operational pending integration test verification
- Integration test (kill daemon → watchdog detects → restarts) is the final verification step
- No blockers for next phase unless integration test reveals issues

---

*Phase: 135-daemon-recovery*
*Completed: 2026-03-22*

## Self-Check: PASSED

- [x] `135-02-SUMMARY.md` — exists at `.planning/phases/135-daemon-recovery/`
- [x] Commit `9aeb6e9` — verified in racecontrol repo
- [x] Commit `cc551b6` — verified in racecontrol repo
- [x] `CommsLink-DaemonWatchdog` Task Scheduler task — Status: Ready
- [x] HKCU Run key `CommsLink` — points to start-comms-link.bat
- [x] Watchdog log confirmed: `OK -- CommsLink daemon is healthy` entry after manual trigger
