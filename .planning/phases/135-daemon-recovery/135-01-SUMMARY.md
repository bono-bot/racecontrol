---
phase: 135-daemon-recovery
plan: "01"
subsystem: comms-link-watchdog
tags: [watchdog, daemon-recovery, task-scheduler, powershell, notifications]
dependency_graph:
  requires: []
  provides: [comms-daemon-watchdog, daemon-auto-restart, bono-crash-notifications]
  affects: [comms-link-daemon, james-bono-relay]
tech_stack:
  added: []
  patterns: [polling-health-check, idempotent-watchdog, crash-notification, task-scheduler-repeat]
key_files:
  created:
    - C:\Users\bono\.claude\james_watchdog.ps1
    - C:\Users\bono\racingpoint\comms-link\scripts\register-comms-watchdog.js
  modified:
    - C:\Users\bono\racingpoint\racecontrol\LOGBOOK.md
    - C:\Users\bono\racingpoint\comms-link\INBOX.md
decisions:
  - "Used /sc MINUTE /mo 2 (every 2 min) for Task Scheduler trigger — simpler than ONLOGON + repeat combo"
  - "Restart via start-comms-link.bat (not launching node directly) — preserves all env vars and supervisor"
  - "No -Wait on start-comms-link.bat (it uses start /min and never exits)"
  - "Send-BonoMessage wraps Start-Process in try/catch so notification failure doesn't abort restart"
metrics:
  duration_minutes: 15
  completed_date: "2026-03-22"
  tasks_completed: 2
  files_created: 2
  commits: 1
---

# Phase 135 Plan 01: CommsLink Daemon Watchdog Summary

**One-liner:** PowerShell watchdog that polls localhost:8766/relay/health every 2 min and auto-restarts the comms-link daemon with crash/recovery notifications to Bono.

---

## What Was Built

### james_watchdog.ps1 (`C:\Users\bono\.claude\`)

PowerShell watchdog script run by Task Scheduler every 2 minutes. Logic:

1. `Test-DaemonHealth` — `Invoke-WebRequest -Uri http://localhost:8766/relay/health -TimeoutSec 5 -UseBasicParsing -ErrorAction SilentlyContinue`. Returns `$true` if HTTP 200, `$false` on any exception or non-200.
2. If UP: logs `OK -- CommsLink daemon is healthy` and exits cleanly.
3. If DOWN:
   - Logs `DOWN` with IST timestamp to `C:\Users\bono\.claude\comms-watchdog.log`
   - Sends `[CRASH] CommsLink daemon down on James - restarting` to Bono via `send-message.js`
   - Launches `start-comms-link.bat` via `Start-Process -WindowStyle Hidden` (no `-Wait`)
   - Sleeps 10 seconds
   - Re-checks health
   - If recovered: logs `RECOVERED` + sends `[RECOVERED] CommsLink daemon back online on James`
   - If still down: logs `FAILED` and exits (Task Scheduler will retry in 2 min)

IST timestamps: computed as `UTC + 5h30m` using `[System.DateTime]::UtcNow.AddHours(5).AddMinutes(30)`.

### register-comms-watchdog.js (`comms-link/scripts/`)

One-shot ESM Node.js script (matches register-watchdog.js pattern) that registers `CommsLink-DaemonWatchdog` in Windows Task Scheduler:

```
schtasks /create /tn CommsLink-DaemonWatchdog
  /tr "powershell.exe -NoProfile -ExecutionPolicy Bypass -File C:\Users\bono\.claude\james_watchdog.ps1"
  /sc MINUTE /mo 2 /rl HIGHEST /f
```

After creating, queries and prints the task details for confirmation. Requires Administrator privileges.

---

## Key Implementation Decisions

| Decision | Rationale |
|----------|-----------|
| `/sc MINUTE /mo 2` for schedule | Simpler than ONLOGON + repeat trigger combo; achieves same result (detects crash within 2 min) |
| Restart via `start-comms-link.bat` | Preserves all required env vars (COMMS_PSK, COMMS_URL, LOGBOOK_PATH) and also starts supervisor-runner.js |
| No `-Wait` on bat | `start-comms-link.bat` uses `start /min` which backgrounds immediately — blocking `-Wait` would hang forever |
| `try/catch` around `Send-BonoMessage` | Notification failure must not abort the restart sequence |
| Health endpoint: `localhost:8766/relay/health` | Existing relay health endpoint; tests actual daemon connectivity |
| Log append mode | `Add-Content` appends by default; log persists across watchdog runs for diagnostics |

---

## Deviations from Plan

None — plan executed exactly as written.

---

## Commits

| Repo | Hash | Message |
|------|------|---------|
| comms-link | `6420871` | feat(135-01): add CommsLink daemon watchdog + scheduler registration |
| comms-link | `c657462` | chore(135-01): inbox entry — daemon watchdog shipped |
| racecontrol | `5cd3e70` | chore(135-01): logbook entry for comms daemon watchdog |

---

## Activation Required

The Task Scheduler task is NOT yet registered. To activate:

```
# Run as Administrator on James machine (192.168.31.27):
cd C:\Users\bono\racingpoint\comms-link
node scripts/register-comms-watchdog.js
```

Once registered, the task will run every 2 minutes automatically. Verify with:
```
schtasks /query /tn CommsLink-DaemonWatchdog /fo LIST /v
```

## Self-Check: PASSED

- [x] `C:\Users\bono\.claude\james_watchdog.ps1` — exists, syntax OK
- [x] `C:\Users\bono\racingpoint\comms-link\scripts\register-comms-watchdog.js` — exists, syntax OK
- [x] Commit `6420871` — verified in comms-link repo
- [x] Both scripts match key_links in plan frontmatter (localhost:8766/relay/health, send-message.js)
