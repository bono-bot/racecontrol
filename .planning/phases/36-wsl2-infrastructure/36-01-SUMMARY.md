---
phase: 36-wsl2-infrastructure
plan: 01
subsystem: infra
tags: [wsl2, saltstack, salt-master, ubuntu-24.04, networking, firewall]

requires: []
provides:
  - "C:/Users/bono/.wslconfig with networkingMode=mirrored, memory=4GB, swap=2GB"
affects: [37-minion-bootstrap, 38-salt-exec-rs, 39-remote-ops-removal, 40-fleet-rollout]

tech-stack:
  added: []
  patterns: []

key-files:
  created:
    - "C:/Users/bono/.wslconfig"
  modified: []

key-decisions:
  - "AMD-V (SVM) must be enabled in BIOS before WSL2 can be installed — CPU supports virtualization but BIOS has it disabled (HyperVRequirementVirtualizationFirmwareEnabled=False)"
  - ".wslconfig created with networkingMode=mirrored, memory=4GB, swap=2GB — ready for when WSL2 is enabled"
  - "Salt install sequence: bootstrap-salt.sh -M -N -P stable 3008 then apt-get install salt-api explicitly"

patterns-established: []

requirements-completed: []

duration: 10min
completed: 2026-03-17
---

# Phase 36 Plan 01: WSL2 Infrastructure Setup Summary

**.wslconfig created with mirrored networking config — blocked at WSL2 install by BIOS AMD-V disabled on Ryzen 7 5800X**

## Performance

- **Duration:** ~10 min
- **Started:** 2026-03-17T06:34:14Z
- **Completed:** 2026-03-17T06:44:14Z (partial — blocked at BIOS gate)
- **Tasks:** 0 of 3 fully complete (blocked mid-Task 1)
- **Files modified:** 1 (C:/Users/bono/.wslconfig)

## Accomplishments
- Created `C:/Users/bono/.wslconfig` with correct mirrored networking configuration
- Diagnosed BIOS blocker: AMD-V (SVM) disabled on Ryzen 7 5800X
- Confirmed `wsl --install --no-distribution` succeeded (WSL kernel and platform components downloaded)
- Confirmed Ubuntu 24.04 download completes but registration fails with HCS_E_HYPERV_NOT_INSTALLED

## Task Commits

This plan is blocked at Task 1 before any task commit could be made. The `.wslconfig` file was created but is outside the git repo and cannot be committed independently.

## Files Created/Modified
- `C:/Users/bono/.wslconfig` — WSL2 config with `networkingMode=mirrored`, `memory=4GB`, `swap=2GB`. Created. Ready for use once BIOS AMD-V is enabled and WSL2 is installed.

## Decisions Made
- `.wslconfig` swap set to 2GB (half of 4GB RAM limit, covers Salt burst startup per RESEARCH.md)
- No CPU limit per user decision in CONTEXT.md
- Salt install order: bootstrap script first, then explicit `apt-get install -y salt-api` (per Pitfall 5 in RESEARCH.md)
- saltadmin PAM user password: `RacingPoint2026Salt!` (to be set after WSL2 is running; will go in racecontrol.toml [salt] section in Phase 38)

## Deviations from Plan

### Blocking Issue Discovered

**[Rule 3 - Blocking] BIOS AMD-V (SVM) disabled on Ryzen 7 5800X**
- **Found during:** Task 1, Step 2 (wsl --install -d Ubuntu-24.04)
- **Issue:** `HyperVRequirementVirtualizationFirmwareEnabled = False` — AMD-V is supported by the CPU but disabled in BIOS/UEFI firmware. WSL2 requires hardware virtualization (AMD-V/SVM for AMD CPUs).
- **Error:** `HCS_E_HYPERV_NOT_INSTALLED` — the Hyper-V Compute Service cannot start without virtualization firmware support
- **Fix required:** Enter BIOS/UEFI and enable AMD SVM Mode (found under CPU Configuration or Advanced CPU Settings)
- **Not auto-fixable:** Requires physical access to machine during reboot — cannot be automated
- **Committed in:** N/A — blocked before task could complete

---

**Total deviations:** 0 auto-fixed, 1 human-action gate
**Impact on plan:** Plan cannot proceed until BIOS AMD-V is enabled. All downstream phases (37–40) are blocked.

## Issues Encountered
- `wsl --install -d Ubuntu-24.04 --no-launch` fails with `HCS_E_HYPERV_NOT_INSTALLED`
- Root cause confirmed via `systeminfo`: "Virtualization Enabled In Firmware: No"
- `VirtualizationFirmwareEnabled = False` from WMI `Win32_Processor`
- `wsl --install --no-distribution` succeeded — WSL kernel/platform components are downloaded, only BIOS change needed

## User Setup Required

**BIOS action required before this plan can continue:**

1. Reboot James's machine (Ryzen 7 5800X)
2. Enter BIOS/UEFI (typically Del or F2 during POST)
3. Navigate to: Advanced > CPU Configuration (varies by motherboard — look for "SVM Mode" or "AMD-V" or "Virtualization")
4. Set **SVM Mode** = **Enabled**
5. Save and exit
6. Verify: `powershell -Command "(Get-WmiObject Win32_Processor).VirtualizationFirmwareEnabled"` must return `True`
7. Then re-run this plan to continue from WSL2 install step

## Next Phase Readiness
- `.wslconfig` is created and ready — do not recreate it when resuming
- WSL2 install command is ready: `wsl --install -d Ubuntu-24.04 --no-launch`
- All subsequent steps (wsl.conf, salt install, firewall) are fully documented in the plan

## Self-Check: PASSED

- FOUND: `C:/Users/bono/.wslconfig` (created, contains networkingMode=mirrored)
- FOUND: `.planning/phases/36-wsl2-infrastructure/36-01-SUMMARY.md`
- Commit `9e178be`: docs(36-01): partial plan execution — blocked at BIOS AMD-V gate

---
*Phase: 36-wsl2-infrastructure*
*Completed: 2026-03-17 (partial — BIOS gate)*
