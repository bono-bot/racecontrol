---
quick_id: 260318-xbn
description: Apply watchdog pattern to kiosk Node.js app on server .23
wave: 1
---

# Plan 260318-xbn-01: Kiosk Watchdog

## Objective
Apply the same PowerShell watchdog pattern from the web dashboard (260318-wkg) to the kiosk app on port 3300.

## Tasks

### Task 1: Create kiosk watchdog scripts
- `deploy-staging/start-kiosk-watchdog.ps1` — PowerShell loop: start node, wait for exit, log, restart with 5s backoff
- `deploy-staging/start-kiosk.bat` — HKLM Run key launcher (ASCII CRLF), starts PS1 hidden

### Task 2: Deploy to server .23 via SSH
- SCP scripts to `C:\RacingPoint\`
- Update HKLM Run key `Kiosk` → `start-kiosk.bat`
- Kill old bare kiosk node process
- Start kiosk via watchdog
- Verify crash recovery: kill node, confirm auto-restart within 10s
