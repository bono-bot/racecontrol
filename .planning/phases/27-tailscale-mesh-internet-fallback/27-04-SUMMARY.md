---
phase: 27-tailscale-mesh-internet-fallback
plan: "04"
subsystem: infra
tags: [tailscale, winrm, powershell, fleet-deploy, mesh-networking]

requires:
  - phase: 27-01
    provides: BonoConfig struct with relay_port=8099 in racecontrol.toml; WinRM confirmed open on all pods

provides:
  - WinRM fleet deploy script for Tailscale on all 8 pods and Racing Point server
  - Canary-first rollout pattern (Pod 8 first, manual 'yes' to proceed)
  - PREAUTH_KEY and ADMIN_PASS guard rails preventing accidental runs with placeholders

affects:
  - 27-05 (racecontrol.toml update with Tailscale IPs after this script runs)
  - operations (fleet bootstrapping — run once before Phase 27 goes live)

tech-stack:
  added: [Tailscale MSI, WinRM Invoke-Command, msiexec silent install]
  patterns:
    - Canary-first fleet deploy: Pod 8 isolated, manual confirmation gate before fleet rollout
    - Download-inside-WinRM: avoids UNC double-hop by running Invoke-WebRequest inside the remote session
    - Service startup grace: 5s sleep between msiexec exit and tailscale up (tailscaled needs time to register)

key-files:
  created:
    - scripts/deploy-tailscale.ps1
  modified: []

key-decisions:
  - "Download MSI inside WinRM session via Invoke-WebRequest -- avoids UNC double-hop that breaks with WinRM CredSSP not configured"
  - "Pod 8 is the designated canary for all fleet operations in Phase 27"
  - "CRLF line endings enforced on all .ps1 files per MEMORY.md standing rule"

patterns-established:
  - "Guard-rail pattern: script exits early with clear error message if placeholder values not replaced"
  - "4-step install sequence: download, msiexec /quiet, 5s sleep, tailscale up --reset"

requirements-completed: [TS-DEPLOY]

duration: 2min
completed: 2026-03-16
---

# Phase 27 Plan 04: Deploy Script Summary

**WinRM PowerShell fleet deploy script for Tailscale on 8 pods + server with canary-first rollout and placeholder guard rails**

## Performance

- **Duration:** 2 min
- **Started:** 2026-03-16T11:38:00Z
- **Completed:** 2026-03-16T11:39:42Z
- **Tasks:** 1
- **Files modified:** 1

## Accomplishments

- Created `scripts/deploy-tailscale.ps1` — production-ready WinRM deploy script for bootstrapping Tailscale across the entire Racing Point fleet
- Canary pattern: Pod 8 (192.168.31.91) deployed first; interactive 'yes' gate before fleet rollout to Pods 1-7 + server
- Guard rails prevent accidental execution with placeholder PREAUTH_KEY or ADMIN_PASS values
- 4-step install sequence per pod: HTTP download inside WinRM session, msiexec silent install with TS_UNATTENDEDMODE=always, 5s service startup grace, tailscale up with --unattended --auth-key --hostname --reset
- Verifies assigned Tailscale IP (100.x.x.x) on each device after enrollment
- CRLF line endings applied for Windows PowerShell compatibility

## Task Commits

1. **Task 1: Create scripts/deploy-tailscale.ps1** - `8286bdb` (feat)

**Plan metadata:** (docs commit follows)

## Files Created/Modified

- `scripts/deploy-tailscale.ps1` - WinRM fleet deploy script; 220 lines; canary-first, 4-step install, IP verification, next-steps for Plan 05

## Decisions Made

- Download MSI inside WinRM session (Invoke-WebRequest) rather than UNC path — avoids credential double-hop issue with WinRM basic auth
- Pod 8 (192.168.31.91) is the designated canary for all Phase 27 fleet operations
- CRLF line endings applied per MEMORY.md standing rule for all .ps1 files
- PREAUTH_KEY placeholder guard exits at line 58 — operator must replace before running

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

Before running the script, operator must:
1. Download Tailscale MSI to `C:\Users\bono\racingpoint\deploy-staging\tailscale-setup-latest-amd64.msi`
2. Start HTTP server: `python -m http.server 9998 --directory C:\Users\bono\racingpoint\deploy-staging --bind 0.0.0.0`
3. Generate a reusable pre-auth key from Tailscale Admin Console and replace `PREAUTH_KEY_REPLACE_ME`
4. Replace `ADMIN_PASSWORD_REPLACE_ME` with the pod Windows admin password (ask Uday)

## Next Phase Readiness

- Script is ready to run once Tailscale pre-auth key is generated and MSI is staged
- After running: note Tailscale IPs for server (racing-point-server) and all pods
- Plan 05 (racecontrol.toml update) requires the server's Tailscale IP and Bono's VPS Tailscale IP

---
*Phase: 27-tailscale-mesh-internet-fallback*
*Completed: 2026-03-16*
