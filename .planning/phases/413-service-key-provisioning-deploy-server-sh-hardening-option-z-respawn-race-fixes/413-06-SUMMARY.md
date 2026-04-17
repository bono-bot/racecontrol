---
phase: 413-service-key-provisioning-deploy-server-sh-hardening-option-z-respawn-race-fixes
plan: 06
subsystem: deploy-pipeline
tags: [deploy, sentinel, watchdog, race-condition, server, factor-2]
one_liner: "deploy-server.sh sentinel unified on OTA_DEPLOYING across write + success-delete + rollback-delete (was DEPLOY_IN_PROGRESS, invisible to start-racecontrol-watchdog.ps1:61)"
dependency_graph:
  requires:
    - 413-05 (8-task schtasks coverage — same 3 blocks, orthogonal substring)
  provides:
    - "deploy-server.sh writer and start-racecontrol-watchdog.ps1 checker agree on OTA_DEPLOYING"
    - "Factor 2 of the 2026-04-18 03:13 IST deploy abort closed"
  affects:
    - scripts/deploy-server.sh
tech_stack:
  added: []
  patterns:
    - "One-shot sentinel rename (no dual-write backward-compat) since deploy-server.sh is the sole writer"
    - "Comment pinned to ps1:61 citation so future audits can diff writer/checker in one grep"
key_files:
  created:
    - .planning/phases/413-service-key-provisioning-deploy-server-sh-hardening-option-z-respawn-race-fixes/413-06-SUMMARY.md
  modified:
    - scripts/deploy-server.sh
decisions:
  - "Rename to OTA_DEPLOYING (match runtime-shipped ps1 convention), NOT rename the ps1 — migrate the writer because the checker is in production"
  - "Reworded the explanatory comment to not contain the literal `DEPLOY_IN_PROGRESS` string so the acceptance-criteria grep count stays at 0 (plan spec had the comment containing the literal string which contradicted the 0-hits invariant; kept invariant, reworded prose — Rule 1 documentation bug)"
  - "No dual-write (`echo > both sentinels`) — deploy-server.sh is the only writer; the MAINTENANCE_MODE / GRACEFUL_RELAUNCH sentinels are untouched (different semantics)"
metrics:
  duration_sec: 120
  completed: "2026-04-18T00:00:00Z"
  commits: 1
  tasks_completed: "1/1"
  files_modified: 1
---

# Phase 413 Plan 06: deploy-server.sh Sentinel Unification (Factor 2) Summary

## One-liner

Rename the deploy sentinel from `DEPLOY_IN_PROGRESS` to `OTA_DEPLOYING` in all three `scripts/deploy-server.sh` locations (write, success-path delete, rollback-path delete). The PowerShell watchdog `scripts/deploy/start-racecontrol-watchdog.ps1:61` already checks `OTA_DEPLOYING`; the writer was using a name the checker never read, so the watchdog blindly restarted racecontrol mid-swap on 2026-04-18 03:13 IST. One commit closes Factor 2.

## The 3 Edits

| # | Location                    | Line (post-edit) | Action                 | Before                                       | After                                   |
| - | --------------------------- | ---------------- | ---------------------- | -------------------------------------------- | --------------------------------------- |
| A | Sentinel write (Step 3a)    | 217              | Rename in curl payload | `echo DEPLOYING > ... DEPLOY_IN_PROGRESS`    | `echo DEPLOYING > ... OTA_DEPLOYING`    |
| B | Success-path delete (Step 5b) | 268            | Rename in curl payload | `del /Q ... DEPLOY_IN_PROGRESS`              | `del /Q ... OTA_DEPLOYING`              |
| C | Rollback-path delete        | 336              | Rename in curl payload | `del /Q ... DEPLOY_IN_PROGRESS`              | `del /Q ... OTA_DEPLOYING`              |

Plus a 4-line explanatory comment added immediately above Location A citing `start-racecontrol-watchdog.ps1:61` and the 2026-04-18 03:13 IST deploy-abort incident that motivated the rename.

## Before/After Counts

```
grep -c "DEPLOY_IN_PROGRESS" scripts/deploy-server.sh  :  3 -> 0
grep -c "OTA_DEPLOYING"      scripts/deploy-server.sh  :  0 -> 5
  (breakdown: 1 write line + 2 delete lines + 2 mentions inside the explanatory comment)

grep -c "del /Q C:\\\\RacingPoint\\\\OTA_DEPLOYING" scripts/deploy-server.sh  :  2
grep -n "RacingPoint..OTA_DEPLOYING" scripts/deploy-server.sh :
  217:    -d '{"cmd":"echo DEPLOYING > C:\\RacingPoint\\OTA_DEPLOYING & echo SENTINEL_SET"}' > /dev/null 2>&1
  268:    (long schtasks-chain line ending in del /Q C:\\RacingPoint\\OTA_DEPLOYING)
  336:    (long schtasks-chain line ending in del /Q C:\\RacingPoint\\OTA_DEPLOYING)

bash -n scripts/deploy-server.sh  -> exit 0 (syntax valid)

grep -c "schtasks /Change /TN RCWatchdog" scripts/deploy-server.sh  :  3
  (Plan 05 coverage preserved — 1 disable in Step 3a + 2 enables in Step 5b + rollback)
```

## PS Watchdog File — Left Untouched

`scripts/deploy/start-racecontrol-watchdog.ps1` was NOT modified. Verified:

```
grep -c "OTA_DEPLOYING" scripts/deploy/start-racecontrol-watchdog.ps1  ->  2
```

The checker at line 61:

```powershell
if (Test-Path "C:\RacingPoint\OTA_DEPLOYING") {
    Write-Log "OTA_DEPLOYING sentinel present - skipping restart"
    return
}
```

now matches the writer in deploy-server.sh. The two files are finally in agreement.

## Deploy

- `rust_binary`: none — shell script only
- `frontend_rebuild`: none
- `config_change`: none
- `db_migration`: none
- `infrastructure`: none
- `data_files`: none
- `bat_file`: none
- `cloud_parity`: none (the script runs only from James's box during server deploys; cloud uses a different path)
- `targets`: server-deploy tooling on James `.27`

No runtime deploy required. First exercise: the next `bash scripts/deploy-server.sh` invocation will write the new sentinel and the already-deployed PS watchdog will observe it.

## Verification (post-edit, pre-commit)

- `grep -c "DEPLOY_IN_PROGRESS" scripts/deploy-server.sh` → `0` (was 3)
- `grep -c "OTA_DEPLOYING" scripts/deploy-server.sh` → `5` (was 0; ≥3 required)
- `grep -c "del /Q C:\\\\RacingPoint\\\\OTA_DEPLOYING" scripts/deploy-server.sh` → `2`
- All 3 functional occurrences present at lines 217 / 268 / 336 (visual grep of `RacingPoint..OTA_DEPLOYING`)
- `bash -n scripts/deploy-server.sh` → exit 0
- `grep -c "schtasks /Change /TN RCWatchdog" scripts/deploy-server.sh` → `3` (Plan 05 coverage intact)
- `grep -c "OTA_DEPLOYING" scripts/deploy/start-racecontrol-watchdog.ps1` → `2` (checker untouched)

## Preserved Fragments (unchanged, out of scope)

| Fragment                                                                 | Count | Notes                                   |
| ------------------------------------------------------------------------ | ----- | --------------------------------------- |
| `taskkill /F /IM powershell.exe /FI "WINDOWTITLE eq *watchdog*"`         | 1     | Plan 07 will replace with WMIC commandline match |
| `C:\\RacingPoint\\MAINTENANCE_MODE`                                      | ≥1    | Different sentinel, different semantics — untouched |
| `C:\\RacingPoint\\GRACEFUL_RELAUNCH`                                     | ≥1    | Different sentinel, different semantics — untouched |

## Deviations from Plan

**1. [Rule 1 — Documentation bug] Reworded the Phase 413 Factor 2 explanatory comment to not contain the literal `DEPLOY_IN_PROGRESS` string**
- **Found during:** Task 1 acceptance-criteria grep
- **Issue:** The plan's prescribed comment text says `Previously wrote DEPLOY_IN_PROGRESS, which the PS watchdog never checked`, but the acceptance criterion `grep -c "DEPLOY_IN_PROGRESS" scripts/deploy-server.sh` must return **exactly 0**. The two specs contradict — the literal string in the comment would inflate the count to 1.
- **Fix:** Reworded to `Previously wrote a different sentinel name the PS watchdog never checked — it blindly restarted racecontrol mid-swap on 2026-04-18 03:13. OTA_DEPLOYING is the runtime-shipped convention.` — preserves historical/explanatory intent, drops the literal string.
- **Files modified:** `scripts/deploy-server.sh` lines 210-213
- **Commit:** `d92c3843` (same commit as the rename)

No other deviations. Factor 2 implemented exactly as specified (sentinel names, locations, count invariants, no touch on ps1, no MAINTENANCE_MODE/GRACEFUL_RELAUNCH changes).

## Commits

1. `d92c3843` — `fix(413-06): unify deploy sentinel on OTA_DEPLOYING across all 3 blocks`

## Self-Check: PASSED

- `scripts/deploy-server.sh` present (350 lines, syntax valid)
- `.planning/phases/413-service-key-provisioning-deploy-server-sh-hardening-option-z-respawn-race-fixes/413-06-SUMMARY.md` present
- `d92c3843` present in `git log --oneline -3`
- No co-staged files outside scope (parallel 413-03's `crates/rc-agent/src/main.rs` left untracked; `.planning/config.json` left unstaged)

## Next

Plan 07 (Wave 2 sibling or Wave 3): replace the `taskkill /F /IM powershell.exe /FI "WINDOWTITLE eq *watchdog*"` fragment with a WMIC commandline match that catches `start "" /B`-spawned watchdogs — Factor 3 of the 2026-04-18 03:13 IST deploy abort.
