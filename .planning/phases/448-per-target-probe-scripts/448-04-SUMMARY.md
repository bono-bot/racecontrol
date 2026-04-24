---
phase: 448-per-target-probe-scripts
plan: "04"
subsystem: fleet-probe
tags: [probe, pod, pos, rc-sentry, ssh, wmi, partial-probe, unit-tests]
dependency_graph:
  requires: [448-01, 448-02]
  provides: [probe-pod.sh, probe-pos.sh, probe-pod.test.mjs, probe-pos.test.mjs]
  affects: [448-07-orchestrator-full-wiring]
tech_stack:
  added: []
  patterns:
    - rc-sentry /exec POST with X-Service-Key for Windows process/registry data
    - MARK-section SSH batch (single session, awk extraction) for POS probe
    - Temp-file Python parsing to avoid heredoc+pipe anti-pattern
    - async spawn (not spawnSync) for in-process mock HTTP server tests
    - PROBE_OVERRIDE_URL / PROBE_SSH / PROBE_SSH_SCENARIO env overrides for test isolation
key_files:
  created:
    - scripts/fleet-probe/probe-pod.sh
    - scripts/fleet-probe/probe-pos.sh
    - tests/fleet-probe/probe-pod.test.mjs
    - tests/fleet-probe/probe-pos.test.mjs
    - tests/fleet-probe/fixtures/pos-ssh-partial.txt
    - tests/fleet-probe/fixtures/responses/pod-exec-ok.json
    - tests/fleet-probe/fixtures/responses/pod-exec-401.json
  modified: []
decisions:
  - Moved fixture response files to fixtures/responses/ subdirectory to avoid schema-compat.test.mjs validating non-manifest JSON
  - Used async spawn (not spawnSync) for probe-pod tests to allow in-process HTTP mock server to serve connections
  - Used temp-file pattern for Python parsing in both probe-pod.sh and probe-pos.sh (heredoc+pipe breaks stdin)
metrics:
  duration_minutes: 120
  completed_date: "2026-04-24T21:14:25+05:30"
  tasks_completed: 2
  tasks_total: 2
  files_created: 7
  files_modified: 0
---

# Phase 448 Plan 04: Wave 2 Probes 2+3 of 3 — probe-pod.sh + probe-pos.sh Summary

**One-liner:** Pod probe via rc-sentry :8091/exec with 8-pod IP table and positional N arg; POS probe via Tailscale SSH with WMI-denied partial degradation; both fully unit-tested with mock servers.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | probe-pod.sh + tests + fixtures | `6735a67f` | probe-pod.sh (431 lines), probe-pod.test.mjs, fixtures/responses/pod-exec-ok.json, fixtures/responses/pod-exec-401.json |
| 2 | probe-pos.sh + tests + fixture | `f7495c59` | probe-pos.sh (341 lines), probe-pos.test.mjs, fixtures/pos-ssh-partial.txt; also fixed heredoc+pipe bug in probe-pod.sh |

## What Was Built

### probe-pod.sh (Task 1)

Probes a single pod (N = 1..8) via rc-sentry :8091/exec:

- **Positional N arg** with validation: exit 2 for N outside 1..8 or missing
- **8-pod IP/hostname case statement**: maps N to `{IP, HOST}` (pod_1=RCPOD-1/192.168.31.89 … pod_8=RCPOD-8/192.168.31.91)
- **SENTRY_KEY pre-check**: missing key → probe_failed + `no_sentry_key` access_gap immediately
- **pod_exec() helper**: writes JSON payload to temp file, calls `curl -d @payload_file`, detects 401 → probe_failed + `stale_sentry_key`
- **Single /exec call per sub-probe** for: `certutil` (binary SHA256), `tasklist /V /FO CSV`, `schtasks /Query /FO LIST`, `reg query HKLM Run`, `reg query HKCU Run`, `set` (env vars)
- **Python temp-file parsing** for tasklist CSV → `running_procs[]`, schtasks LIST → `scheduled_tasks[]`, reg output → `autostart_entries[]`
- **rc-agent :8090/health** for `build_id`, `env_vars_hash`
- **PROBE_OVERRIDE_URL / PROBE_OVERRIDE_PORT_SENTRY / PROBE_OVERRIDE_PORT_AGENT** env vars for test isolation
- **Output**: `state/fleet-manifest/$MANIFEST_TS/pod_N.json` (schema-01-compliant)

### probe-pos.sh (Task 2)

Probes POS1 (192.168.31.130 / Tailscale pos1@100.95.211.1) via SSH:

- **Single SSH session** with MARK-delimited batch: hostname, certutil (config.json), tasklist, schtasks, reg_hklm, reg_hkcu, env, end
- **extract_section() awk helper** for clean section parsing
- **WMI-denied detection**: `grep -qiE 'WMI|ERROR:|Unable to connect'` on tasklist → `probe_errors[{sub_probe:"tasklist", error:"WMI access denied…"}]` + `running_procs=[]` but continues (partial, not probe_failed)
- **SSH failure detection**: exit 1 from SSH + "timed out"/"Connection refused" → probe_failed + `POS_SSH_DOWN` access_gap
- **PROBE_SSH / PROBE_SSH_SCENARIO / PROBE_SKIP_HTTP** overrides for test isolation (skips kiosk :3300 HTTP in unit tests)
- **Python temp-file parsing** for schtasks LIST → `scheduled_tasks[]`, reg output → `autostart_entries[]`
- **Output**: `state/fleet-manifest/$MANIFEST_TS/pos_130.json` (schema-01-compliant)

## Verification Results

```
npm run test:fleet-probe  → 18/18 PASS
npm run test:fleet-drift  → 17/17 PASS (regression clean)
```

Test breakdown:
- Tests 1-4: probe-pod.sh preconditions (exit 2 for pod 9, pod 0, no arg, no MANIFEST_TS)
- Tests 5-7: no SENTRY_KEY → probe_failed + no_sentry_key
- Tests 8-9: pod_1 and pod_8 IP/host mapping verified
- Test 10: 401 from rc-sentry → probe_failed + stale_sentry_key
- Test 11: POS partial path (WMI-denied tasklist) → partial + sub_probe error + scheduled_tasks >= 1 + autostart_entries >= 1
- Test 12: POS SSH timeout → probe_failed + POS_SSH_DOWN
- Test 13: POS MANIFEST_TS unset → exit 2
- Tests 14-16: probe-server.sh (carried from 448-03, regression confirmed)
- Tests 17-18: schema-compat + validateAgainstSchema

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Moved fixture response files to responses/ subdirectory**

- **Found during:** Task 1 verification
- **Issue:** `schema-compat.test.mjs` reads all `.json` files in `tests/fleet-probe/fixtures/` and validates them against the fleet manifest schema. `pod-exec-ok.json` and `pod-exec-401.json` are rc-sentry response bodies (not manifests) and failed schema validation — test 17 "all fixtures are schema-valid" failed.
- **Fix:** Created `tests/fleet-probe/fixtures/responses/` subdirectory; moved both files there. `schema-compat.test.mjs` uses `readdirSync(FIXTURES_DIR)` (top-level only, no subdirectory recursion), so the response fixtures are never validated.
- **Files modified:** test file import paths updated
- **Commit:** `6735a67f`

**2. [Rule 1 - Bug] Fixed spawnSync blocks Node event loop for mock HTTP tests**

- **Found during:** Task 1 verification (test 10)
- **Issue:** `spawnSync` blocks Node.js single-threaded event loop, preventing the in-process `http.createServer` from accepting connections while the bash subprocess runs. curl got http_code=000 (connection refused) for the 401 path test.
- **Fix:** Rewrote `runProbeWithMock()` to use async `spawn` with stream event listeners (`child.stdout.on('data')`, `child.on('close')`) and a Promise-based wrapper with 60s timeout guard.
- **Files modified:** `tests/fleet-probe/probe-pod.test.mjs`
- **Commit:** `6735a67f`

**3. [Rule 1 - Bug] Fixed heredoc+pipe anti-pattern in probe-pod.sh and probe-pos.sh**

- **Found during:** Task 2 verification (test 11 — `scheduled_tasks.length >= 1` got 0)
- **Issue:** `printf '%s' "$DATA" | python3 - "$OUTFILE" <<'PYEOF'` is broken: the `<<'PYEOF'` heredoc redirects Python's stdin to the heredoc content (the script itself), not the pipe data. Result: `sys.stdin.read()` returns "" (empty), producing empty arrays. Same bug class documented in 448-02 SUMMARY as "Rule 1 Bug #2".
- **Fix (both scripts):** Save section data to temp files (`$WORK_DIR/tasklist.txt`, `$WORK_DIR/schtasks.txt`, `$WORK_DIR/reg-combined.txt`), then pass as `sys.argv[1]` and `sys.argv[2]` to Python. Heredoc is now the Python source only; data is read via `open(sys.argv[1])`.
- **Files modified:** `scripts/fleet-probe/probe-pod.sh` (all 3 parsing blocks), `scripts/fleet-probe/probe-pos.sh` (schtasks + reg blocks)
- **Commit:** `f7495c59`

## Known Stubs

None. Both probes write complete manifests. `PROBE_SKIP_HTTP=1` in tests is a test-isolation flag, not a production stub — production runs without it.

## Self-Check: PASSED

| Check | Result |
|-------|--------|
| scripts/fleet-probe/probe-pod.sh exists | FOUND |
| scripts/fleet-probe/probe-pos.sh exists | FOUND |
| tests/fleet-probe/probe-pod.test.mjs exists | FOUND |
| tests/fleet-probe/probe-pos.test.mjs exists | FOUND |
| tests/fleet-probe/fixtures/pos-ssh-partial.txt exists | FOUND |
| tests/fleet-probe/fixtures/responses/pod-exec-ok.json exists | FOUND |
| tests/fleet-probe/fixtures/responses/pod-exec-401.json exists | FOUND |
| commit 6735a67f in log | FOUND |
| commit f7495c59 in log | FOUND |
