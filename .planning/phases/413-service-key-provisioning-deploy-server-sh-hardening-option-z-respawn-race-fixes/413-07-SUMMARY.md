---
phase: 413-service-key-provisioning-deploy-server-sh-hardening-option-z-respawn-race-fixes
plan: 07
subsystem: deploy-pipeline
tags: [deploy, watchdog, wmic, race-condition, server, factor-3]
one_liner: "deploy-server.sh watchdog kill swapped from WINDOWTITLE-wildcard taskkill to WMIC commandline match on start-racecontrol-watchdog.ps1 — catches empty-title PS instances launched via `start \"\" /B` that the old filter missed"
dependency_graph:
  requires:
    - 413-05 (8-task schtasks coverage — same disable block)
    - 413-06 (OTA_DEPLOYING sentinel rename — same block, orthogonal substring)
  provides:
    - "deploy-server.sh watchdog kill matches every PowerShell instance running start-racecontrol-watchdog.ps1 regardless of window title"
    - "Factor 3 of the 2026-04-18 03:13 IST deploy abort closed"
    - "All 3 factors (schtasks coverage + sentinel rename + watchdog match) now co-resident in the Step 3a disable block"
  affects:
    - scripts/deploy-server.sh
tech_stack:
  added: []
  patterns:
    - "WMIC commandline substring match replacing Windows taskkill WINDOWTITLE wildcard filter"
    - "Match the launcher artifact (ps1 filename), not transient window-title strings"
key_files:
  created:
    - .planning/phases/413-service-key-provisioning-deploy-server-sh-hardening-option-z-respawn-race-fixes/413-07-SUMMARY.md
  modified:
    - scripts/deploy-server.sh
decisions:
  - "WMIC (deprecated in Win11 24H2+ but present on server .23) chosen over Get-CimInstance for consistency with the plan's explicit spec; Plan 08 audit will flag 24H2 risk if applicable"
  - "Double-percent `%%start-racecontrol-watchdog.ps1%%` in the WMIC LIKE literal so cmd.exe on the /exec handler collapses to a single `%` — same escape pattern the rest of deploy-server.sh uses"
  - "No fallback to the old taskkill filter — a fallback masks bugs; Plan 10's integration test catches a WMIC failure in its own right"
  - "Reworded the Factor 3 explanatory comment to drop the literal `WINDOWTITLE eq *watchdog*` substring (same Rule-1 deviation shape as Plan 06) — the acceptance-criterion `grep -c ... = 0` and the prescribed comment text contradicted; invariant preserved, prose reworded to `the window-title taskkill filter` / `the old taskkill wildcard filter`"
metrics:
  duration_sec: 180
  completed: "2026-04-18T00:00:00Z"
  commits: 1
  tasks_completed: "1/1"
  files_modified: 1
---

# Phase 413 Plan 07: deploy-server.sh WMIC Commandline Match (Factor 3) Summary

## One-liner

Replace the `taskkill /F /IM powershell.exe /FI "WINDOWTITLE eq *watchdog*"` fragment inside the Step 3a disable block with `wmic process where "name='powershell.exe' and commandline like '%%start-racecontrol-watchdog.ps1%%'" delete`. The launcher at `scripts/deploy/start-racecontrol.bat:26` spawns the watchdog via `start "" /B powershell ... -File ... start-racecontrol-watchdog.ps1` — the empty `""` leaves the window title unset, so the old wildcard filter matched zero instances and zombie watchdogs survived every deploy's kill step. Commandline match via WMIC catches every PS process running the ps1 file, regardless of title.

## Before / After of the Disable-Block Kill Fragment

Before (Plan 05 + Plan 06 state, line 208):

```
... & taskkill /F /IM powershell.exe /FI "WINDOWTITLE eq *watchdog*" 2>nul & echo WATCHDOG_DISABLED
```

After (Plan 07, line 211):

```
... & wmic process where "name='powershell.exe' and commandline like '%%start-racecontrol-watchdog.ps1%%'" delete 2>nul & echo WATCHDOG_DISABLED
```

The rest of the disable-block payload (8 `schtasks /Change /TN <Name> /Disable` fragments from Plan 05) is unchanged and stays to the left of the new WMIC invocation. The `echo WATCHDOG_DISABLED` tail stays to the right.

## Why the Old Filter Missed

`start-racecontrol.bat:26`:

```bat
start "" /B powershell -ExecutionPolicy Bypass -WindowStyle Hidden -File C:\RacingPoint\start-racecontrol-watchdog.ps1
```

- `""` is the title argument to the Windows `start` command. Empty string = no window title set.
- `/B` means "no new console window" anyway.
- `-WindowStyle Hidden` keeps any PS UI surfaces invisible.

Net: the spawned `powershell.exe` has neither a console window nor a window title containing `watchdog`. `taskkill /FI "WINDOWTITLE eq *watchdog*"` needs *something* matching the `*watchdog*` pattern in a window title column, which WMIC/tasklist queries don't populate for these instances. Result: 0 matches, taskkill reports success, zombie watchdog survives and fires `schtasks /Run /TN StartRCDirect` (or equivalent) immediately after the swap — 03:13 IST was the textbook example.

## Why WMIC commandline match works

WMIC exposes `Win32_Process.CommandLine`, which DOES contain the full argv of the spawned process, including the `-File C:\RacingPoint\start-racecontrol-watchdog.ps1` token. A `LIKE '%start-racecontrol-watchdog.ps1%'` filter reliably identifies every watchdog instance regardless of how the parent launched it (empty title, hidden window, `/B`, or fully interactive).

## Escape-Rule Decision: `%%` vs `%`

The WMIC LIKE wildcard is `%`. cmd.exe treats `%` as variable-expansion syntax. Inside a curl JSON `-d '{...}'` payload, the fragment goes: bash single-quotes → JSON string → cmd.exe (on the /exec handler side, which runs `cmd /C "..."`).

| Layer | Sees | Reason |
|---|---|---|
| Bash (single-quoted) | `%%start-racecontrol-watchdog.ps1%%` literal | single quotes suppress bash expansion |
| JSON parser | `%%start-racecontrol-watchdog.ps1%%` (no escape needed) | `%` is not a JSON special char |
| cmd.exe (via `cmd /C` on server side) | `%start-racecontrol-watchdog.ps1%` (one `%` per pair) | cmd.exe collapses `%%` → `%` |
| WMIC | `%start-racecontrol-watchdog.ps1%` | wildcards around the ps1 filename |

Decision: **use `%%`** per CONTEXT.md's spec. This matches the escape pattern elsewhere in the deploy-server.sh /exec chain. If Plan 10's integration test shows `%%` arrives at wmic literally (because a specific /exec path doesn't go through cmd /C), we reduce to single `%`. Conservative default was to match the plan's explicit guidance.

## Counts (grep verification — post-edit)

| Grep | Before | After | Target | Status |
|---|---|---|---|---|
| `WINDOWTITLE eq \*watchdog\*` | 1 | 0 | 0 | PASS |
| `wmic process where` | 0 | 1 | ≥1 | PASS |
| `start-racecontrol-watchdog.ps1` | 1 | 3 | ≥1 | PASS (1 WMIC line + 2 new comment mentions) |
| `commandline like` | 0 | 1 | ≥1 | PASS |
| `taskkill /F /IM powershell` | 1 | 0 | 0 | PASS |
| `schtasks /Change /TN RCWatchdog /Disable` | 1 | 1 | 1 | PASS (Plan 05 preserved) |
| `schtasks /Change /TN RCWatchdog /Enable` | 2 | 2 | 2 | PASS (Plan 05 preserved) |
| `OTA_DEPLOYING` | 5 | 5 | ≥3 | PASS (Plan 06 preserved) |
| `DEPLOY_IN_PROGRESS` | 0 | 0 | 0 | PASS (Plan 06 preserved) |
| `bash -n scripts/deploy-server.sh` | exit 0 | exit 0 | exit 0 | PASS |

## The 3-Factor Disable Block (Composite View — Plan 05 + 06 + 07)

```bash
# Phase 413 Factor 1: Extended schtasks disable from 2 to 8 tasks. RCWatchdog fired
# during the 2026-04-18 03:13 abort — it was not in the original disable list.
# Phase 413 Factor 3: Replaced the window-title taskkill filter with a WMIC
# commandline match. start-racecontrol.bat launches the watchdog via
# `start "" /B powershell ... -File start-racecontrol-watchdog.ps1` — the
# empty "" leaves the window title unset, so the old taskkill wildcard filter
# matched zero instances and zombie watchdogs survived the "kill" step.
# Commandline match via WMIC catches every PS instance running the ps1.
# See .planning/phases/413-*/413-CONTEXT.md for full rationale.
info "Disabling watchdog to prevent restart race..."
curl -s --max-time 15 "http://${SERVER_IP}:${SENTRY_PORT}/exec" \
    -H "$AUTH_HEADER" \
    -H "Content-Type: application/json" \
    -d '{"cmd":"schtasks /Change /TN StartRCOnBoot /Disable 2>nul & schtasks /Change /TN StartRCTemp /Disable 2>nul & schtasks /Change /TN RCWatchdog /Disable 2>nul & schtasks /Change /TN RaceControlStartup /Disable 2>nul & schtasks /Change /TN StartRCDirect /Disable 2>nul & schtasks /Change /TN StartRaceControl /Disable 2>nul & schtasks /Change /TN StartRCWatchdog /Disable 2>nul & schtasks /Change /TN StartFrontendWatchdog /Disable 2>nul & wmic process where \"name='powershell.exe' and commandline like '%%start-racecontrol-watchdog.ps1%%'\" delete 2>nul & echo WATCHDOG_DISABLED"}' > /dev/null 2>&1
# Also write a deploy sentinel to block any remaining watchdog instance
# Phase 413 Factor 2: Sentinel name unified on OTA_DEPLOYING (matches
# start-racecontrol-watchdog.ps1:61). [...]
```

All 3 factors now live in adjacent lines of the same block. Future audits can diff the block as a unit.

## Deploy

- `rust_binary`: none — shell-script-only change
- `frontend_rebuild`: none
- `config_change`: none
- `db_migration`: none
- `infrastructure`: none
- `data_files`: none
- `bat_file`: none
- `cloud_parity`: none (server-deploy tooling; cloud uses `deploy-cloud.sh`)
- `targets`: server-deploy pipeline on James `.27`

First live-exercise: the next `bash scripts/deploy-server.sh` invocation. Runtime validation is Plan 10 (integration) and Plan 11 (fleet). No pre-verification possible from James's box — the WMIC command runs inside the server's cmd.exe context, which can only be reached via the /exec endpoint.

## Deviations from Plan

**1. [Rule 1 — Documentation bug] Reworded Factor 3 comment to drop the literal `WINDOWTITLE eq *watchdog*` substring**

- **Found during:** Task 1 acceptance-criteria grep after the initial Edit.
- **Issue:** The plan's described comment would naturally quote the old filter `\`/FI \"WINDOWTITLE eq *watchdog*\"\`` to explain why it missed. But acceptance criterion #1 is `grep -c "WINDOWTITLE eq \*watchdog\*" scripts/deploy-server.sh` = **0**. A comment containing the literal string inflates the count to 1. Same deviation shape as Plan 06 (Rule 1, documentation bug where prose + invariant contradicted).
- **Fix:** Reworded the comment to paraphrase — `the window-title taskkill filter` and `the old taskkill wildcard filter` — preserving the explanatory intent (what was replaced and why) without the banned literal substring.
- **Files modified:** `scripts/deploy-server.sh` lines 203-208
- **Commit:** `bee5d207` (same atomic commit as the WMIC swap — single-task plan)

No other deviations. Factor 3 implemented exactly as specified (WMIC command, `%%` escape, no fallback, single block edit).

## Preserved Fragments (unchanged, out of scope)

| Fragment | Count | Notes |
|---|---|---|
| `schtasks /Change /TN RCWatchdog /Disable` | 1 | Plan 05 — disable block |
| `schtasks /Change /TN RCWatchdog /Enable` | 2 | Plan 05 — success + rollback re-enable blocks |
| `OTA_DEPLOYING` | 5 | Plan 06 — 3 functional (write + 2 deletes) + 2 comment mentions |
| `DEPLOY_IN_PROGRESS` | 0 | Plan 06 — fully removed |
| `C:\\RacingPoint\\MAINTENANCE_MODE` | ≥1 | Different sentinel, different semantics — untouched |
| `C:\\RacingPoint\\GRACEFUL_RELAUNCH` | ≥1 | Different sentinel, different semantics — untouched |
| `start-racecontrol-watchdog.ps1` checker (.ps1 file itself) | 2 hits in ps1 | PS watchdog file NOT modified — the checker still reads `OTA_DEPLOYING` at ps1:61 |

## Commits

1. `bee5d207` — `fix(413-07): replace WINDOWTITLE taskkill with WMIC commandline match` (Task 1)

## Self-Check: PASSED

- `scripts/deploy-server.sh` present
- `.planning/phases/413-service-key-provisioning-deploy-server-sh-hardening-option-z-respawn-race-fixes/413-07-SUMMARY.md` present
- Commit `bee5d207` present in `git log --oneline -5`
- All 8 acceptance-criteria grep counts match expected values
- `bash -n` exits 0
- Plan 05 (8-task coverage) and Plan 06 (OTA_DEPLOYING sentinel) invariants preserved

## Next

All 3 factors of the 2026-04-18 03:13 IST deploy abort are now resolved in source:

1. Factor 1 (Plan 05) — 8-task schtasks coverage
2. Factor 2 (Plan 06) — OTA_DEPLOYING sentinel
3. Factor 3 (Plan 07) — WMIC commandline match

Plan 08 is the MMA audit of the 3-factor change. Plan 10 is the live integration test (first deploy exercising the new block). Plan 11 is fleet deploy.
