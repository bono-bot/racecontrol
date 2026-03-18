---
quick_id: 260318-xbn
status: complete
---

# Summary: Kiosk Watchdog

Applied the watchdog pattern from 260318-wkg (web dashboard) to the kiosk app (port 3300).

## What Changed
- Created `start-kiosk-watchdog.ps1` — PowerShell infinite loop, monitors node.exe, restarts on crash with 5s backoff, logs to `kiosk-watchdog.log`
- Updated `start-kiosk.bat` — HKLM Run key launcher, starts PS1 hidden (no cmd window)
- Deployed to server .23 via SCP + SSH
- HKLM Run key `Kiosk` updated to use new bat file

## Server .23 Service Summary (all watchdog-protected)
| Service | Port | Watchdog | Log |
|---------|------|----------|-----|
| racecontrol.exe | 8080 | start-racecontrol-watchdog.ps1 | racecontrol-watchdog.log |
| web dashboard (node) | 3200 | start-web-watchdog.ps1 | web-dashboard.log |
| kiosk (node) | 3300 | start-kiosk-watchdog.ps1 | kiosk-watchdog.log |

## Verification
- Killed kiosk node process on :3300
- Watchdog restarted within 10 seconds
- HTTP 200 confirmed on /kiosk endpoint
