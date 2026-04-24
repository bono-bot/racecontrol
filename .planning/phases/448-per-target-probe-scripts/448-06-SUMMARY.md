---
phase: 448-per-target-probe-scripts
plan: "06"
subsystem: fleet-probe
tags: [probe, cloud-admin, cloud-rc, windows-python-fix, testing]
dependency_graph:
  requires: [448-01, 448-02]
  provides: [probe-cloud-admin.sh, probe-cloud-rc.sh, PROBE_PYTHON-pattern]
  affects: [448-07, 448-08]
tech_stack:
  added: [james_helpers.py, cloud_admin_helpers.py, cloud_rc_helpers.py]
  patterns: [PROBE_PYTHON-env-override, python-helper-files-over-heredocs]
key_files:
  created:
    - scripts/fleet-probe/probe-cloud-admin.sh
    - scripts/fleet-probe/probe-cloud-rc.sh
    - scripts/fleet-probe/lib/cloud_admin_helpers.py
    - scripts/fleet-probe/lib/cloud_rc_helpers.py
    - scripts/fleet-probe/lib/james_helpers.py
    - tests/fleet-probe/probe-cloud-admin.test.mjs
    - tests/fleet-probe/probe-cloud-rc.test.mjs
    - tests/fleet-probe/fixtures/responses/cloud-admin-health-ok.json
    - tests/fleet-probe/fixtures/responses/cloud-admin-health-gated.json
    - tests/fleet-probe/fixtures/responses/cloud-rc-health-ok.json
  modified:
    - scripts/fleet-probe/lib/probe-common.sh (_PROBE_PYTHON support)
    - scripts/fleet-probe/probe-james.sh (rewrite using james_helpers.py)
    - scripts/fleet-probe/probe-pod.sh (python3 -> $_PROBE_PYTHON)
    - scripts/fleet-probe/probe-pos.sh (python3 -> $_PROBE_PYTHON)
    - scripts/fleet-probe/probe-server.sh (python3 -> $_PROBE_PYTHON)
    - tests/fleet-probe/probe-cloud-admin.test.mjs (PROBE_PYTHON in spawn env)
    - tests/fleet-probe/probe-cloud-rc.test.mjs (PROBE_PYTHON in spawn env)
    - tests/fleet-probe/probe-james.test.mjs (PROBE_PYTHON in spawn env)
    - tests/fleet-probe/probe-pod.test.mjs (PROBE_PYTHON in spawn env)
    - tests/fleet-probe/probe-pos.test.mjs (PROBE_PYTHON in spawn env)
    - tests/fleet-probe/probe-server.test.mjs (PROBE_PYTHON in spawn env)
decisions:
  - "Used separate Python helper .py files (cloud_admin_helpers.py, cloud_rc_helpers.py, james_helpers.py) instead of heredoc inline python calls — eliminates Windows python3 Store stub hang in non-interactive Node.js spawn contexts"
  - "PROBE_PYTHON env var pattern adopted across all 8 probe scripts + probe-common.sh: tests pass PROBE_PYTHON=python (real Python 3.12 on Windows), production Linux defaults to python3"
  - "ADMIN_COMING_SOON_GATE encoded as scheduled_tasks entry (not probe_error) to distinguish intentional gate state from actual errors"
metrics:
  duration: "~3h (includes root-cause diagnosis of Windows python3 Store stub hang)"
  completed: "2026-04-24"
  tasks_completed: 2
  files_created: 10
  files_modified: 11
requirements: [PROBE-06, PROBE-07]
---

# Phase 448 Plan 06: probe-cloud-admin.sh + probe-cloud-rc.sh Summary

Wave 3 cloud probes with Python helper libs replacing heredoc/inline calls to eliminate Windows Store python3 stub hang.

## What Was Built

### Task 1: probe-cloud-admin.sh + 4 unit tests

`scripts/fleet-probe/probe-cloud-admin.sh` probes `admin.racingpoint.cloud` via two sub-probes:
- `GET /api/health` — captures `build_id`, `git_commit`, `pages_missing[]`
- `HEAD /` + redirect check — detects ADMIN_COMING_SOON_GATE (307 to /coming-soon)

The gate state is encoded as a `scheduled_tasks` entry `{name: "ADMIN_COMING_SOON_GATE", state: "active|inactive"}` — intentional state, not an error.

4 test cases: ok path (build_id captured), gate active (scheduled_tasks entry), pages_missing (partial + pages_probe error), /api/health 500 (probe_failed).

### Task 2: probe-cloud-rc.sh + 4 unit tests

`scripts/fleet-probe/probe-cloud-rc.sh` probes `racingpoint.cloud/api/v1/health` and captures `build_id`.

4 test cases: ok (build_id captured + schema-valid), 500 (probe_failed), malformed JSON 200 (partial + health_parse), missing build_id (partial + build_id error + null build_id).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Windows python3 Store stub hangs in non-interactive Node.js spawn**

- **Found during:** Task 1 — all 4 probe-cloud-admin tests timed out at 120s
- **Root cause:** On Windows, `python3` resolves to the App Execution Alias in `WindowsApps/` — a stub that opens the Microsoft Store UI in interactive sessions and hangs silently (no output, no exit) in non-interactive contexts such as `child_process.spawn("bash", ...)`. This affects `python3 -c "..."`, `python3 - <<'PYEOF'` heredocs, and even `which python3` (bash tries to exec the stub to verify it). The real Python 3.12 is at `python` (without the 3).
- **Diagnosis path:** Added debug HTTP server logging to confirm mock server WAS responding (GET /api/health served, GET / served) but probe script still hung — pointing to the python3 call inside the script, not curl or bash itself. Confirmed `python3 -c "print(1)"` hangs under Node.js spawn.
- **Fix:** Introduced `PROBE_PYTHON` env var pattern across the entire test suite:
  - `probe-common.sh` line 15: `_PROBE_PYTHON="${PROBE_PYTHON:-python3}"` — all other probe scripts inherit this after sourcing
  - All Python calls in probe-common.sh (json_escape, iso_ist_now fallback, write_manifest json.tool) changed to use `"$_PROBE_PYTHON"`
  - `probe-james.sh` fully rewritten to use `"$_PROBE_PYTHON" "$PY_HELPERS"` with new `lib/james_helpers.py` (eliminates all heredocs and `python3 -c` inline calls)
  - `probe-pod.sh`, `probe-pos.sh`, `probe-server.sh`: all `python3` occurrences replaced with `"$_PROBE_PYTHON"`
  - All `*.test.mjs` files: `const PYTHON_CMD = process.platform === "win32" ? "python" : "python3"` constant added; `PROBE_PYTHON: PYTHON_CMD` added to all spawn/spawnSync env objects
  - New Python helper libs: `lib/cloud_admin_helpers.py`, `lib/cloud_rc_helpers.py`, `lib/james_helpers.py` — all read from file paths (sys.argv), never from stdin
- **Files modified:** probe-common.sh, probe-cloud-admin.sh, probe-cloud-rc.sh, probe-james.sh, probe-pod.sh, probe-pos.sh, probe-server.sh, all 6 test files, 3 new lib/*.py files
- **Commit:** `e60b5bbe`
- **Impact:** Cross-cutting fix benefiting all 8 probe scripts and the entire test suite. The fix is production-safe: PROBE_PYTHON is only set in tests; on Linux production the default `python3` is correct.

**Note:** probe-relay.sh and probe-vps.sh python3 fixes were committed by the parallel 448-05 agent in commit `dfb01982` during the same root-cause investigation.

## Test Results

Full suite run after all fixes:

```
npm run test:fleet-probe
33/33 tests PASS (0 failures)

Test breakdown:
- probe-cloud-admin: 4/4 (ok, gate, pages_missing, 500)
- probe-cloud-rc:    4/4 (ok, 500, malformed, missing build_id)
- probe-james:       2/2 (ok + MANIFEST_TS unset)
- probe-pod:         8/8
- probe-pos:         3/3
- probe-relay:       3/3
- probe-server:      3/3
- probe-vps:         4/4
- schema helpers:    2/2

npm run test:fleet-drift
17/17 tests PASS (no regressions)
```

## Probe Script Inventory (after Plan 06)

All 8 per-target probes now exist plus orchestrator skeleton:

```
scripts/fleet-probe/
  probe-all.sh              # orchestrator skeleton (Plan 02)
  probe-cloud-admin.sh      # Plan 06 (new)
  probe-cloud-rc.sh         # Plan 06 (new)
  probe-james.sh            # Plan 02
  probe-pod.sh              # Plan 04
  probe-pos.sh              # Plan 04
  probe-relay.sh            # Plan 05
  probe-server.sh           # Plan 03
  probe-vps.sh              # Plan 05
  lib/
    cloud_admin_helpers.py  # Plan 06 (new)
    cloud_rc_helpers.py     # Plan 06 (new)
    james_helpers.py        # Plan 06 (new)
    probe-common.sh         # Plan 01 (modified)
```

9 .sh scripts (probe-all + 8 per-target). Wave 3 complete.

## Commits

| Hash | Description |
|------|-------------|
| `2f205f31` | feat(448-06): add probe-cloud-admin.sh + fixtures + unit tests (Task 1) |
| `3d658bad` | feat(448-06): add probe-cloud-rc.sh + fixture + unit tests (Task 2) |
| `e60b5bbe` | fix(448-06): replace python3 with _PROBE_PYTHON to fix Windows Store stub hang |

## Handoff to Plan 07

Plan 07 (orchestrator full wiring) can now wire all 8 per-target probes into `probe-all.sh`. All probes accept the same env contract:
- `MANIFEST_TS` (required) — timestamp prefix for manifest directory
- `PROBE_PYTHON` (optional) — Python interpreter override for non-Linux environments

All 8 produce schema-valid JSON at `state/fleet-manifest/$MANIFEST_TS/<target_id>.json`.

## Self-Check: PASSED

All key files found on disk. All 3 task commits verified in git log. 33/33 tests pass. 17/17 fleet-drift tests pass.
