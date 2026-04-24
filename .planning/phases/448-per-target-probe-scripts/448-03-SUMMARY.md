---
phase: 448-per-target-probe-scripts
plan: 03
subsystem: testing
tags: [bash, node, fleet-probe, ssh, tailscale, q5-drift, config-hash, swaplog]

# Dependency graph
requires:
  - phase: 448-per-target-probe-scripts
    plan: 02
    provides: "probe-common.sh (10 functions), probe-james.sh pattern, mock-ssh-responder.sh, helpers.mjs"
provides:
  - "scripts/fleet-probe/probe-server.sh -- Server .23 probe via Tailscale SSH + SWAPLOG + Q5 three-way config_hash"
  - "tests/fleet-probe/probe-server.test.mjs -- 3 tests: ok path schema-valid + 3 config_hash keys; probe_failed + access_gap=SSH_23; exit 2 on missing MANIFEST_TS"
  - "tests/fleet-probe/fixtures/server-ssh-ok.txt -- section-marked mock SSH output for ok path"
  - "tests/fleet-probe/fixtures/server-ssh-timeout.txt -- exit 255 mock for probe_failed path"
affects:
  - 448-07-orchestrator
  - 449-first-full-fleet-probe-run
  - 452-diff-tool

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Section-marked SSH output (===MARK:section===) parsed with extract_section() awk -- one SSH handshake for all sub-probes"
    - "PROBE_SSH override: SSH_CMD=${PROBE_SSH:-ssh} lets tests inject mock-ssh-responder.sh without real network"
    - "Temp files for all sub-probe output; Python reads from sys.argv[1] file paths -- avoids ARG_MAX and heredoc-stdin problems (established in Plan 02)"
    - "probe_failed zeroes binary_sha256/config_hash/running_procs/scheduled_tasks/autostart_entries -- clean empty-object contract"
    - "SWAPLOG.md regex parse: grep '^| [0-9]' | tail -1 | awk -F'|' col2 -> python3 ISO-8601 +05:30 conversion"
    - "Q5 three-way config_hash keys locked: racecontrol.toml.server_live / .james_proxy / .git_head"

key-files:
  created:
    - scripts/fleet-probe/probe-server.sh
    - tests/fleet-probe/probe-server.test.mjs
    - tests/fleet-probe/fixtures/server-ssh-ok.txt
    - tests/fleet-probe/fixtures/server-ssh-timeout.txt
  modified: []

key-decisions:
  - "Section markers (===MARK:xxx===) in SSH script output -- enables awk-based section extraction from single multi-command SSH session"
  - "CONNECT_ERR flag separate from SUBPROBE_ERR -- probe_failed dominates, partial is for reachable-but-incomplete"
  - "james_proxy hash: /d/racecontrol.toml is the Git Bash path for D:\\racecontrol.toml on James -- live-tested correctly"
  - "PROBE_SKIP_HTTP=1 skips /api/v1/health for unit tests -- build_id stays null but schema still valid"
  - "config_hash is empty {} on probe_failed -- zeroed with other binary data"

requirements: [PROBE-01]

# Metrics
duration: 19min
completed: 2026-04-24
---

# Phase 448 Plan 03: probe-server.sh (Tailscale SSH + Q5 three-way drift) Summary

**Server .23 SSH probe capturing binary_sha256 + Q5 three-way config_hash + SWAPLOG last_deploy_ts + tasklist/schtasks/reg state; PROBE_SSH override makes unit tests fully offline**

## Performance

- **Duration:** ~19 min
- **Completed:** 2026-04-24
- **Tasks:** 2 (TDD RED + GREEN)
- **Files created:** 4

## Accomplishments

- `probe-server.sh` (389 lines) probes Server .23 via Tailscale SSH `ADMIN@100.125.108.37`
- Single SSH handshake with section-marked output (`===MARK:xxx===`) extracts 8 sub-probes
- Q5 three-way config_hash captured: `racecontrol.toml.server_live` (SSH certutil), `racecontrol.toml.james_proxy` (/d/racecontrol.toml), `racecontrol.toml.git_head` (repo file or git show HEAD)
- SWAPLOG.md parse extracts last_deploy_ts as ISO-8601 +05:30 from last data row
- probe_failed path: SSH timeout -> `probe_errors[0].sub_probe=ssh_connect`, `access_gap=SSH_23`, `binary_sha256={}`, `config_hash={}`
- `npm run test:fleet-probe` probe-server tests 3/3 GREEN; `npm run test:fleet-drift` 17/17 still green

## Task Commits

1. **Task 1: mock SSH fixtures + probe-server.test.mjs (TDD RED)** -- `f9d0f674`
2. **Task 2: probe-server.sh implementation (TDD GREEN)** -- `d0f2406e`

## Files Created

- `scripts/fleet-probe/probe-server.sh` -- 389-line SSH probe (PROBE_SSH override, section-marked output, temp files, Q5 config hash, SWAPLOG parse)
- `tests/fleet-probe/probe-server.test.mjs` -- 3 tests covering ok path (schema-valid + config_hash keys), probe_failed path (access_gap=SSH_23), MANIFEST_TS precondition
- `tests/fleet-probe/fixtures/server-ssh-ok.txt` -- section-marked mock SSH output (hostname/certutil_exe/certutil_toml/tasklist/schtasks/reg_hklm/reg_hkcu/env/end)
- `tests/fleet-probe/fixtures/server-ssh-timeout.txt` -- empty stdout + exit 255 (SSH timeout code)

## Sample OK Path Manifest (first 20 fields)

```json
{
    "schema_version": "1.0",
    "target_id": "server_23",
    "host": "Racing-Point-Server",
    "ip": "192.168.31.23",
    "role": "server",
    "probed_at_ist": "2026-04-24T20:58:05+05:30",
    "probe_status": "partial",
    "binary_sha256": {
        "racecontrol.exe": "aaaa...aaa"
    },
    "build_id": null,
    "config_hash": {
        "racecontrol.toml.server_live": "bbbb...bbb",
        "racecontrol.toml.james_proxy": "7881dac5fd86...",
        "racecontrol.toml.git_head":    "7ee002b40f8a..."
    },
    "running_procs": [...],
    "scheduled_tasks": [...],
    "autostart_entries": [...],
    "env_vars_hash": "e3b0c44...",
    "last_deploy_ts": null
}
```

Note: `partial` because PROBE_SKIP_HTTP=1 (no build_id) + SWAPLOG last row has tilde-prefixed timestamp. Live run will produce `ok` when /api/v1/health is reachable and SWAPLOG has a clean timestamp row.

## Sample probe_failed Manifest

```json
{
    "probe_status": "probe_failed",
    "binary_sha256": {},
    "build_id": null,
    "config_hash": {},
    "running_procs": [],
    "scheduled_tasks": [],
    "autostart_entries": [],
    "env_vars_hash": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    "last_deploy_ts": null,
    "probe_errors": [
        {
            "sub_probe": "ssh_connect",
            "error": "SSH to ADMIN@100.125.108.37 failed or truncated (exit=255)",
            "access_gap": "SSH_23"
        }
    ]
}
```

## Q5 Three-Way Config Hash Confirmation

All three keys are present and grep-verifiable:

```
grep -c "racecontrol.toml.server_live" scripts/fleet-probe/probe-server.sh  # 1
grep -c "racecontrol.toml.james_proxy" scripts/fleet-probe/probe-server.sh  # 1
grep -c "racecontrol.toml.git_head"    scripts/fleet-probe/probe-server.sh  # 1
```

Live probe (with `/d/racecontrol.toml` present on James and repo racecontrol.toml tracked) produces all 3 hashes in `config_hash`. Phase 452 diff tool will compare these to surface Q5 three-way drift.

## Decisions Made

- Section-marked SSH output (`===MARK:hostname===` through `===MARK:end===`) -- single handshake, awk parsing; more reliable than multi-SSH calls
- `PROBE_SSH=${PROBE_SSH:-ssh}` pattern (same as PROBE_SSH_SCENARIO passthrough) -- tests fully offline
- Temp files for all large output; manifest assembly via Python reading sys.argv file paths -- same ARG_MAX-safe pattern from Plan 02
- `CONNECT_ERR` vs `SUBPROBE_ERR` dual counter: connect-stage failures dominate to `probe_failed`; subprobe failures produce `partial`
- config_hash zeroed to `{}` on probe_failed (consistent with binary_sha256 empty contract)
- `/d/racecontrol.toml` is the correct Git Bash path for `D:\racecontrol.toml` -- verified live on James

## Deviations from Plan

### Auto-fixed Issues

None - plan executed exactly as specified. The section-marker approach in the SSH script output (plan specified `===MARK:xxx===`) worked as designed with the mock-ssh-responder.sh fixture format.

## Known Stubs

- `build_id` is always `null` when `PROBE_SKIP_HTTP=1` (unit tests) -- intentional for offline testing
- SWAPLOG last row with tilde-prefixed timestamps (`~02:10 IST`) will produce `last_deploy_ts: null` + partial error -- real deploy rows have clean timestamps and will parse correctly

## Handoff to Plan 04 (probe-pod.sh + probe-pos.sh, parallel Wave 2)

- Plans 03 (probe-server.sh) and 04 (probe-pod.sh + probe-pos.sh) run in parallel Wave 2
- Plan 04 owns: `scripts/fleet-probe/probe-pod.sh`, `scripts/fleet-probe/probe-pos.sh`, their tests and fixtures
- Plan 04 MUST NOT touch `scripts/fleet-probe/probe-server.sh` or `tests/fleet-probe/fixtures/server-ssh-*.txt`
- Orchestrator wiring (probe-all.sh) deferred to Plan 07 per wave structure

---
*Phase: 448-per-target-probe-scripts*
*Completed: 2026-04-24*
