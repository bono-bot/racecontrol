---
phase: 448-per-target-probe-scripts
plan: "07"
subsystem: testing
tags: [bash, python, node, fleet-probe, orchestrator, meta-index, canary, dry-run, windows-python-fix]

# Dependency graph
requires:
  - phase: 448-per-target-probe-scripts
    plan: "02"
    provides: "probe-all.sh skeleton with --dry-run, probe-james.sh"
  - phase: 448-per-target-probe-scripts
    plan: "03"
    provides: "probe-server.sh (SSH + HTTP)"
  - phase: 448-per-target-probe-scripts
    plan: "04"
    provides: "probe-pod.sh + probe-pos.sh"
  - phase: 448-per-target-probe-scripts
    plan: "05"
    provides: "probe-vps.sh + probe-relay.sh"
  - phase: 448-per-target-probe-scripts
    plan: "06"
    provides: "probe-cloud-admin.sh + probe-cloud-rc.sh + _PROBE_PYTHON pattern"
provides:
  - "scripts/fleet-probe/probe-all.sh -- FULL wiring: invokes all 8 probe scripts, sequential cluster + parallel pod fanout, --canary + --dry-run + --help modes, exits 0 always"
  - "scripts/fleet-probe/build-meta-index.py -- SCHEMA-03 _meta.json builder: ordered targets[], status_summary {ok/partial/probe_failed}, timing from orchestrator-start-epoch"
  - "tests/fleet-probe/smoke-orchestrator.sh -- 3-part integration smoke: dry-run 15 lines + --help + --canary end-to-end with mock network"
  - "tests/fleet-probe/orchestrator-dry-run.test.mjs -- 2 Node unit tests: --dry-run 15 targets + --help Usage:"
affects:
  - 448-08
  - phase-449

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Orchestrator always exits 0 -- probe_failed is a row in _meta.json, not an exit code"
    - "Pod fanout via & + wait with || true on each wait to absorb individual probe failures"
    - "Python timedelta arithmetic for IST timestamps -- avoids utcfromtimestamp() deprecation in Python 3.12+"
    - "cat pipe to python -c for reading manifests in shell -- avoids Unix-path vs Windows-path mismatch"
    - "PROBE_PYTHON propagated as export from orchestrator so all child subprobe invocations inherit the Windows-safe interpreter"

key-files:
  created:
    - scripts/fleet-probe/build-meta-index.py
    - tests/fleet-probe/smoke-orchestrator.sh
    - tests/fleet-probe/orchestrator-dry-run.test.mjs
  modified:
    - scripts/fleet-probe/probe-all.sh

key-decisions:
  - "Orchestrator exit 0 even on all-probe_failed -- the canary smoke confirms this: both server_23 + pod_8 return probe_failed but orchestrator exits 0 and writes _meta.json with target_count=2"
  - "Sequential cluster then parallel pods (not full parallel) -- server/pos/james/vps/cloud-admin/cloud-rc/relay sequential for auth rate-limit safety; pods 1-8 via & + wait"
  - "build-meta-index.py uses timedelta arithmetic (not utcfromtimestamp) -- Python 3.12 raises DeprecationWarning on utcfromtimestamp, fixed proactively"
  - "Smoke test uses PROBE_SSH mock + no SENTRY_KEY to force probe_failed -- avoids any real network call while still exercising the full orchestrator code path"
  - "cat pipe pattern for Python manifest reads in smoke test -- passes content via stdin rather than file path argument to avoid Git Bash Unix-path vs Windows Python path mismatch"

requirements-completed: [PROBE-09]

# Metrics
duration: 15min
completed: 2026-04-24
---

# Phase 448 Plan 07: Orchestrator Full Wiring + build-meta-index.py Summary

**Full probe-all.sh wiring: 8 probe scripts invoked (sequential cluster + parallel pod fanout), SCHEMA-03 _meta.json assembled by build-meta-index.py, canary+dry-run modes, orchestrator always exits 0**

## Performance

- **Duration:** ~15 min
- **Started:** 2026-04-24T18:00:16Z
- **Completed:** 2026-04-24T18:14:53Z
- **Tasks:** 2
- **Files created/modified:** 4

## Accomplishments

- `probe-all.sh` fully wired: invokes probe-server, probe-pos, probe-james, probe-vps, probe-cloud-admin, probe-cloud-rc, probe-relay (sequential) then pods 1-8 in parallel via `&` + `wait`
- `--canary` mode runs server_23 + pod_8 only (PACT-012 subset), writes 2 manifests + _meta.json
- `--dry-run` preserved exactly from Plan 02 (15-target enumeration, no network calls)
- `build-meta-index.py` reads per-target manifests, writes SCHEMA-03 `_meta.json` with `TARGET_ORDER`, `status_summary`, and `probe_duration_sec` from `--orchestrator-start-epoch`
- Smoke test proves end-to-end canary: both probes return `probe_failed` but orchestrator exits 0 and _meta.json has `target_count=2`
- `npm run test:fleet-probe` 35/35 pass (33 prior waves + 2 new orchestrator unit tests)
- `npm run test:fleet-drift` 17/17 still green

## Sample _meta.json from Canary Smoke Run

```json
{
  "schema_version": "1.0",
  "probe_run_id": "2026-04-24T23:39:26+05:30",
  "probed_at_ist": "2026-04-24T23:39:26+05:30",
  "probe_duration_sec": 1.0,
  "orchestrator": "scripts/fleet-probe/probe-all.sh",
  "orchestrator_version": "phase-448-v1",
  "target_count": 2,
  "targets": [
    {"target_id": "server_23", "role": "server", "probe_status": "probe_failed", "manifest_file": "server_23.json"},
    {"target_id": "pod_8",     "role": "pod",    "probe_status": "probe_failed", "manifest_file": "pod_8.json"}
  ],
  "status_summary": {"ok": 0, "partial": 0, "probe_failed": 2}
}
```

## Canary Wall-Clock Time

~1-2 seconds (both probes are probe_failed paths -- no real SSH or HTTP wait). Live canary with real SSH would be ~15-30s total.

## Task Commits

1. **Task 1: build-meta-index.py + full probe-all.sh wiring** -- `3635a496` (feat)
2. **Task 2: smoke-orchestrator.sh + orchestrator-dry-run.test.mjs** -- `eb729674` (feat)

## Files Created/Modified

- `scripts/fleet-probe/probe-all.sh` -- Replaced Plan 02 skeleton with full wiring (133 lines)
- `scripts/fleet-probe/build-meta-index.py` -- New SCHEMA-03 _meta.json builder (129 lines)
- `tests/fleet-probe/smoke-orchestrator.sh` -- New 3-part integration smoke test
- `tests/fleet-probe/orchestrator-dry-run.test.mjs` -- New Node unit tests (dry-run + help)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Python 3.12 DeprecationWarning in build-meta-index.py iso_ist_now()**
- **Found during:** Task 2 smoke test run
- **Issue:** `datetime.datetime.utcfromtimestamp()` emits `DeprecationWarning: datetime.datetime.utcfromtimestamp() is deprecated and scheduled for removal` in Python 3.12+. Would become an error in a future Python version.
- **Fix:** Replaced with `datetime.datetime(1970, 1, 1) + datetime.timedelta(seconds=ist_epoch)` -- same result, no deprecation warning, no future breakage risk.
- **Files modified:** scripts/fleet-probe/build-meta-index.py
- **Verification:** Re-ran smoke test -- clean output, no warnings
- **Committed in:** eb729674 (Task 2 commit)

**2. [Rule 1 - Bug] Git Bash Unix-path vs Windows Python path mismatch in smoke test**
- **Found during:** Task 2 first smoke run
- **Issue:** `"$PYTHON_CMD" -c "import json; m=json.load(open('$DIR/_meta.json'))..."` -- `$DIR` is a Git Bash Unix path (`/c/Users/.../`) but Python on Windows expects a Windows path (`C:\Users\...`), causing `FileNotFoundError`.
- **Fix:** Changed to `cat "$DIR/_meta.json" | "$PYTHON_CMD" -c "import json,sys; m=json.load(sys.stdin)..."` -- passes content via stdin rather than a file path, sidestepping the path format mismatch entirely.
- **Files modified:** tests/fleet-probe/smoke-orchestrator.sh
- **Verification:** Smoke test passes cleanly (smoke-orchestrator OK)
- **Committed in:** eb729674 (Task 2 commit)

---

**Total deviations:** 2 auto-fixed (Rule 1 bugs -- both discovered during smoke test execution)
**Impact on plan:** Zero scope change. Both fixes are correctness issues in the new code itself.

## Issues Encountered

- Git Bash path conversion: `$DIR` in smoke test needed `cat | python` pattern (same class as 448-02 ARG_MAX pattern -- Windows Python cannot resolve Git Bash Unix paths)

## Known Stubs

None -- orchestrator is fully wired. `probe-all.sh` no longer exits 3 for full/canary modes.

## Handoff to Plan 08

Plan 08 (access-gaps.md scaffold + Phase 449 handoff) can now:
- Run `bash scripts/fleet-probe/probe-all.sh --canary` to get a live 2-target manifest set
- Run `bash scripts/fleet-probe/probe-all.sh --dry-run` to enumerate all 15 targets
- Read `state/fleet-manifest/<ts>/_meta.json` for `status_summary` and `targets[]` with `probe_status`
- Phase 449 live-run execution gate: `bash scripts/fleet-probe/probe-all.sh` produces 15 per-target manifests + _meta.json in one invocation

All 8 per-target probe scripts (Plans 02-06) and the orchestrator (Plan 07) are committed on branch `docs/v53-milestone-kickoff-20260424`.

## Self-Check: PASSED
