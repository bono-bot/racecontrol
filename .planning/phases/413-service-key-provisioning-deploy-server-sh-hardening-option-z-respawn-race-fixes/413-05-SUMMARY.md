---
phase: 413-service-key-provisioning-deploy-server-sh-hardening-option-z-respawn-race-fixes
plan: 05
subsystem: deploy-pipeline
tags: [deploy, schtasks, watchdog, race-condition, respawn, server]
one_liner: "deploy-server.sh schtasks disable/re-enable coverage expanded from 2 to 8 RC-related scheduled tasks (RCWatchdog + 5 others) to close the 03:13 IST respawn race"
dependency_graph:
  requires: []
  provides:
    - "deploy-server.sh symmetric 8-task schtasks disable + 2x re-enable (success + rollback)"
    - "Foundation for Plan 06 (DEPLOY_IN_PROGRESS → OTA_DEPLOYING sentinel rename)"
    - "Foundation for Plan 07 (WINDOWTITLE taskkill → WMIC commandline match)"
  affects:
    - scripts/deploy-server.sh
tech_stack:
  added: []
  patterns:
    - "schtasks /Change /TN <Name> /Disable|Enable chain (single curl exec line)"
    - "Symmetric disable→kill→swap→start→re-enable deploy pattern"
key_files:
  created: []
  modified:
    - scripts/deploy-server.sh
decisions:
  - "Keep the 3 curl /exec blocks structurally identical — symmetry means future audits can diff them"
  - "Stay strictly inside the schtasks chain — do not touch taskkill WINDOWTITLE (Plan 07) or DEPLOY_IN_PROGRESS sentinel (Plan 06)"
  - "Order the 8 task names identically across all 3 blocks for visual grepability"
metrics:
  duration_sec: 160
  completed: "2026-04-17T23:28:43Z"
  commits: 3
  tasks_completed: "3/3"
  files_modified: 1
---

# Phase 413 Plan 05: deploy-server.sh schtasks 8-Task Coverage Summary

## One-liner

Extend `deploy-server.sh` schtasks disable list from 2 to 8 tasks in all 3 locations (disable block, success re-enable block, rollback re-enable block) — closes the 2026-04-18 03:13 IST respawn race where `RCWatchdog` fired during the kill→swap→start window because it was never in the original disable list.

## The 8 RC-related Scheduled Tasks (Server .23)

| # | Task Name                | Role                                                                 | Previously Disabled in deploy? |
| - | ------------------------ | -------------------------------------------------------------------- | ------------------------------ |
| 1 | StartRCOnBoot            | Boot-time racecontrol launch                                         | Yes (original)                 |
| 2 | StartRCTemp              | On-demand racecontrol launch (used by deploy itself + operator runs) | Yes (original)                 |
| 3 | RCWatchdog               | PowerShell watchdog loop (THE smoking gun, fired at 03:13)           | No — now yes                   |
| 4 | RaceControlStartup       | Legacy startup task                                                  | No — now yes                   |
| 5 | StartRCDirect            | Direct racecontrol.exe launch (bypasses bat)                         | No — now yes                   |
| 6 | StartRaceControl         | Alternate launcher                                                   | No — now yes                   |
| 7 | StartRCWatchdog          | Alternate watchdog launcher                                          | No — now yes                   |
| 8 | StartFrontendWatchdog    | Frontend (kiosk/web/admin) watchdog                                  | No — now yes                   |

## Edits (3 blocks in `scripts/deploy-server.sh`)

| Block                                 | Line area  | Action  | Count before → after | Commit     |
| ------------------------------------- | ---------- | ------- | -------------------- | ---------- |
| Step 3a disable block                 | ~205       | Disable | 2 → 8                | `0fc38726` |
| Step 5b success-path re-enable block  | ~260       | Enable  | 2 → 8                | `e38a9e81` |
| Rollback-path re-enable block         | ~327       | Enable  | 2 → 8                | `7c7af7ec` |

## Invocation Counts (grep verification)

Each of the 8 tasks now has:

- **1 disable** invocation (Step 3a block only)
- **2 enable** invocations (Step 5b success + rollback)

Total `schtasks /Change /TN` invocations in `deploy-server.sh`: 24 across 3 lines (was 6 across 3 lines).

Verified via grep:

```
RCWatchdog=2 enables, 1 disable
RaceControlStartup=2 enables, 1 disable
StartRCDirect=2 enables, 1 disable
StartRaceControl=2 enables, 1 disable
StartRCWatchdog=2 enables, 1 disable
StartFrontendWatchdog=2 enables, 1 disable
StartRCOnBoot=2 enables, 1 disable
StartRCTemp=2 enables, 1 disable
```

## Deploy

- `rust_binary`: none — script-only change
- `frontend_rebuild`: none
- `config_change`: none
- `db_migration`: none
- `infrastructure`: none
- `data_files`: none
- `bat_file`: none
- `cloud_parity`: none (server-deploy-only script, cloud uses `deploy-cloud.sh`)
- `targets`: server-side deploy tooling on James `.27` (executor machine)

No runtime deploy needed — the script runs from James's box during the next deploy. First exercise of the new coverage: the next `bash scripts/deploy-server.sh` invocation (should be exercised before shipping Plan 06 + Plan 07).

## Preserved Fragments (untouched, as per plan guardrails)

- `taskkill /F /IM powershell.exe /FI "WINDOWTITLE eq *watchdog*" 2>nul` — remains in the Step 3a disable block. **Plan 07** replaces this with a `wmic ... commandline like '%start-racecontrol-watchdog.ps1%'` pattern.
- `del /Q C:\RacingPoint\DEPLOY_IN_PROGRESS 2>nul` — remains in both re-enable blocks. **Plan 06** renames `DEPLOY_IN_PROGRESS` → `OTA_DEPLOYING` in the same 3 blocks (and the Step 2c + Step 3a sentinel writes).

Both are verified present: `grep -c "DEPLOY_IN_PROGRESS"` returns 3 (Step 2c write — actually `MAINTENANCE_MODE`/`GRACEFUL_RELAUNCH` clear — plus Step 3a write of DEPLOY_IN_PROGRESS, plus Step 5b and rollback del); `grep -c 'taskkill /F /IM powershell.exe /FI'` returns 1.

## Verification

- `bash -n scripts/deploy-server.sh` → exit 0 (syntax valid) after each commit
- Per-task enable counts: 2 × 8 = 16 enables
- Per-task disable counts: 1 × 8 = 8 disables
- All 8 task names appear in every Disable block + every Enable block (total 3 blocks)
- No schtasks can fire and respawn racecontrol during the kill→swap→start window, assuming the set of 8 is complete on server .23 (see Caveats)

## Caveats

- **The 8-task set was enumerated from session evidence, not from `schtasks /Query` on server .23.** If server .23 has additional RC-related schtasks not in this list (human-created, installer residue, or created since the audit), they will still respawn racecontrol. The complete set enumeration should be part of Plan 08+ (verify via `schtasks /Query /FO CSV | findstr /I "rc\|racecontrol\|watchdog"` on server .23). For now, the 8-task set covers every task name mentioned in the 2026-04-18 session handoffs and the CLAUDE.md deploy protocol.
- **`StartRCTemp` being disabled during its own deploy run is subtle but safe** — the deploy script calls `schtasks /Run /TN StartRCTemp` in Step 5 **after** Step 5b re-enables it. Plan's original pattern preserved.
- **The disable/re-enable order is irrelevant** for idempotence (all are `/Change /TN /Enable|Disable`). Order preserved for visual symmetry.

## Deviations from Plan

None — plan executed exactly as written. All 3 acceptance-criteria grep counts match expected values (1 disable + 2 enables per new task, preserved StartRCOnBoot/StartRCTemp counts, taskkill + DEPLOY_IN_PROGRESS fragments intact, bash syntax valid).

## Commits

1. `0fc38726` — `fix(413-05): extend schtasks disable list to 8 tasks in deploy-server.sh` (Task 1)
2. `e38a9e81` — `fix(413-05): extend success-path schtasks re-enable to 8 tasks` (Task 2)
3. `7c7af7ec` — `fix(413-05): extend rollback-path schtasks re-enable to 8 tasks` (Task 3)

## Self-Check: PASSED

- `scripts/deploy-server.sh` present
- `413-05-SUMMARY.md` present
- All 3 commits (`0fc38726`, `e38a9e81`, `7c7af7ec`) present in `git log --oneline --all`
