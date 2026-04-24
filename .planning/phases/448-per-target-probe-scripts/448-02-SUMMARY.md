---
phase: 448-per-target-probe-scripts
plan: 02
subsystem: testing
tags: [bash, node, fleet-probe, localhost, orchestrator, dry-run]

# Dependency graph
requires:
  - phase: 448-per-target-probe-scripts
    plan: 01
    provides: "probe-common.sh (10 functions), validate-manifest-file.mjs, test helpers"
provides:
  - "scripts/fleet-probe/probe-james.sh -- James .27 localhost probe (tasklist, schtasks, HKLM/HKCU Run, startup folder)"
  - "scripts/fleet-probe/probe-all.sh -- Orchestrator SKELETON with --dry-run 15-target enumeration"
  - "tests/fleet-probe/probe-james.test.mjs -- 2 Node tests: schema-valid manifest + MANIFEST_TS precondition"
  - "tests/fleet-probe/smoke-james.sh -- Live bash smoke test with ajv validation"
affects:
  - 448-03-probe-server
  - 448-07-orchestrator

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Use cmd //c to invoke Windows builtins (tasklist, schtasks, reg) from Git Bash -- /FLAG args get POSIX-path-converted otherwise"
    - "Save large Windows command output to temp files; python3 reads from file path (sys.argv[1]) not stdin -- heredoc (python3 - <<'EOF') redirects stdin away from pipe, causing 0-line reads"
    - "Use mktemp + trap EXIT for temp dir cleanup in probe scripts"
    - "Pass small JSON configs as sys.argv; large data structures via temp files to stay within ARG_MAX"

key-files:
  created:
    - scripts/fleet-probe/probe-james.sh
    - scripts/fleet-probe/probe-all.sh
    - tests/fleet-probe/probe-james.test.mjs
    - tests/fleet-probe/smoke-james.sh
  modified: []

key-decisions:
  - "cmd //c required for Windows builtins in Git Bash (tasklist, schtasks) -- /V /FO /Query flags get interpreted as Unix paths without it"
  - "Temp files for large sub-probe outputs: heredoc form (python3 - <<'EOF') wins over pipe because it redirects stdin to the heredoc content, breaking piped input"
  - "probe-all.sh skeleton exits 3 for full/canary modes -- clean handoff to Plan 448-07 with no silent failure"
  - "15 targets, not 11 -- CONTEXT.md says 11 physical hosts but probe count is 15 (server=1, pod=8, pos=1, james=1, vps=1, cloud_admin=1, cloud_racecontrol=1, relay=1)"

requirements-completed: [PROBE-04, PROBE-09]

# Metrics
duration: 18min
completed: 2026-04-24
---

# Phase 448 Plan 02: probe-james.sh + Orchestrator Skeleton Summary

**James .27 localhost probe (always-available class) + 15-target orchestrator skeleton prove the Plan 01 shared lib end-to-end before any SSH/HTTP probe is written**

## Performance

- **Duration:** ~18 min
- **Started:** 2026-04-24T15:00:46Z
- **Completed:** 2026-04-24T15:12:56Z
- **Tasks:** 2
- **Files created:** 4

## Accomplishments

- `probe-james.sh` produces schema-valid james_27 manifest using Plan 01 shared lib (source + write_manifest + env_names_hash + iso_ist_now + probe_status_from_errors)
- Collects: running_procs (tasklist /V /FO CSV), scheduled_tasks (schtasks /Query /V /FO LIST), autostart_entries (HKLM/HKCU Run keys + Startup folder), env_vars_hash, config_hash (comms-link/config.toml if present)
- MANIFEST_TS precondition enforced: exits 2 with stderr "MANIFEST_TS not set"
- `probe-all.sh` skeleton enumerates exactly 15 targets in locked order, --dry-run exits 0 with no network calls
- `npm run test:fleet-probe` 4/4 pass (2 new probe-james tests + 2 existing schema-compat)
- `npm run test:fleet-drift` 17/17 still green (no regression)

## Sample probe-james.sh Output

Status line (stdout):
```
{"target_id":"james_27","probe_status":"ok","duration_ms":22517,"errors_count":0}
```

Manifest peek (first 8 fields from smoke test):
```json
{
    "schema_version": "1.0",
    "target_id": "james_27",
    "host": "JAMES-PC",
    "ip": "192.168.31.27",
    "role": "james",
    "probed_at_ist": "2026-04-24T20:36:38+05:30",
    "probe_status": "ok",
    "binary_sha256": {},
    "build_id": null,
    ...
}
```

## probe-all.sh --dry-run Output

```
target=server_23                role=server
target=pod_1                    role=pod
...
target=relay_james              role=relay
```
15 lines total. `wc -l` == 15, no duplicates.

## Task Commits

1. **Task 1: probe-james.sh + unit test + smoke test** -- `c6119937` (feat)
2. **Task 2: probe-all.sh orchestrator skeleton** -- `cc2029f0` (feat)

## Files Created

- `scripts/fleet-probe/probe-james.sh` -- 130+ line Windows localhost probe (cmd //c for builtins, temp files for large output, sys.argv for small data)
- `scripts/fleet-probe/probe-all.sh` -- 62-line skeleton with 15-target TARGETS array, --dry-run mode, exit 3 for Plan 07 handoff
- `tests/fleet-probe/probe-james.test.mjs` -- 2 tests: schema-valid manifest assertion (running_procs non-empty, env_vars_hash hex64, null fields) + MANIFEST_TS precondition
- `tests/fleet-probe/smoke-james.sh` -- bash smoke: runs probe-james.sh, asserts manifest written, calls ajv validator

## Decisions Made

- `cmd //c "..."` pattern for all Windows CLI tools in Git Bash -- `/V`, `/FO`, `/Query` flags get POSIX-path-converted otherwise (e.g. `/Query` becomes the path `/c/Program Files/Git/Query`)
- Python reads large command output from temp files (not from stdin) -- `python3 script.py file.txt` pattern avoids the heredoc stdin capture problem
- `mktemp -d` + `trap 'rm -rf "$WORK_DIR"' EXIT` ensures temp cleanup even on error
- probe-all.sh exits 3 (not 1) for unimplemented modes -- distinguishes "not yet wired" from "error"

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Git Bash path conversion of Windows CLI flags**
- **Found during:** Task 1 smoke test
- **Issue:** `tasklist /V /FO CSV` fails as Git Bash converts `/V` to Unix path; schtasks /Query similarly broken
- **Fix:** All Windows builtins wrapped in `cmd //c "..."` in probe-james.sh
- **Files modified:** scripts/fleet-probe/probe-james.sh
- **Committed in:** c6119937

**2. [Rule 1 - Bug] Python heredoc captures stdin, breaks pipe**
- **Found during:** Task 1 smoke test (exit 141 SIGPIPE)
- **Issue:** `cmd //c "..." | python3 - <<'PYEOF'` -- heredoc redirects Python's stdin to the heredoc content; the pipe is ignored; `sys.stdin` reads 0 lines from 8024-line schtasks output
- **Fix:** Save command output to temp files; Python receives file path as `sys.argv[1]` and reads with `open()`
- **Files modified:** scripts/fleet-probe/probe-james.sh
- **Committed in:** c6119937

**3. [Rule 1 - Bug] ARG_MAX exceeded for manifest assembly**
- **Found during:** Task 1 smoke test (exit 126 "Argument list too long")
- **Issue:** Running_procs JSON (470 processes) + schtasks JSON (8024 lines) exceeds shell ARG_MAX when passed as sys.argv to manifest assembly python call
- **Fix:** Manifest assembly python script also reads from files (running_procs_file, schtasks_file, autostart_file passed as argv); only small strings (target_id, probe_status, env_hash, etc.) passed as argv
- **Files modified:** scripts/fleet-probe/probe-james.sh
- **Committed in:** c6119937

---

**Total deviations:** 3 auto-fixed (Rule 1 bugs -- all Git Bash + Windows subprocess edge cases)
**Impact on plan:** Zero scope change. Implementation pattern established for all future Windows localhost probes: cmd //c + temp files + file-arg pattern.

## Issues Encountered

- Exit 141 (SIGPIPE): first symptom of heredoc-captures-stdin bug
- Exit 126 (ENOMEM/E2BIG): ARG_MAX exceeded, solved by file-based data passing

## Known Stubs

- `probe-all.sh` full/canary modes exit 3 -- intentional stub for Plan 448-07 wiring

## Open Handoff Items for Plan 03 (probe-server.sh)

- Plan 03 (probe-server.sh) is the first SSH probe; uses Tailscale SSH `ADMIN@100.125.108.37`
- Git Bash SSH quirks: use `2>nul` not `2>/dev/null` in REMOTE commands; host-side Windows commands via SSH need same `cmd //c` wrapping if running in a cmd.exe context
- Temp file pattern from probe-james.sh is directly reusable for SSH output storage
- `probe-all.sh` TARGETS array order is locked; Plan 03 can test its manifest independently without touching probe-all.sh

## Next Phase Readiness

- All 4 Plan 02 required artifacts on disk and committed
- `npm run test:fleet-probe` 4/4; `npm run test:fleet-drift` 17/17
- Plans 03-06 can source `lib/probe-common.sh`, reuse the temp-file pattern from probe-james.sh
- probe-all.sh --dry-run proves 15-target enumeration is correct; Plan 07 wiring is a drop-in

---
*Phase: 448-per-target-probe-scripts*
*Completed: 2026-04-24*
