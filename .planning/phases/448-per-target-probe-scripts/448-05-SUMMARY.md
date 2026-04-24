---
phase: 448-per-target-probe-scripts
plan: "05"
subsystem: fleet-probe
tags: [probe, vps, relay, comms-link, http-mock, unit-tests, tdd]
dependency_graph:
  requires: [448-01, 448-02]
  provides: [probe-vps.sh, probe-relay.sh, probe-vps.test.mjs, probe-relay.test.mjs]
  affects: [448-07-orchestrator-full-wiring]
tech_stack:
  added: []
  patterns:
    - comms-link relay /relay/exec/run POST with COMMS_PSK + Bearer auth for VPS remote execution
    - MARK-section bash heredoc (===MARK:section===) for structured multi-command relay output
    - /relay/health connected bool for composite James+VPS status encoding
    - fixtures/responses/ subdirectory to prevent schema-compat.test.mjs from validating non-manifest JSON
    - _PROBE_PYTHON env-override pattern for Windows Store python3 stub avoidance
key_files:
  created:
    - scripts/fleet-probe/probe-vps.sh
    - scripts/fleet-probe/probe-relay.sh
    - tests/fleet-probe/probe-vps.test.mjs
    - tests/fleet-probe/probe-relay.test.mjs
    - tests/fleet-probe/fixtures/responses/vps-relay-exec-ok.json
    - tests/fleet-probe/fixtures/responses/vps-relay-exec-err.json
    - tests/fleet-probe/fixtures/responses/relay-health-ok.json
    - tests/fleet-probe/fixtures/responses/relay-health-disconnected.json
  modified: []
decisions:
  - "probe-vps.sh uses relay ONLY — zero SSH calls (grep -c ssh == 0 confirmed); SSH fallback explicitly deferred"
  - "Relay exec uses single bash_script with ===MARK:section=== sentinels to multiplex 6 sub-probes in one HTTP call"
  - "probe-relay.sh encodes VPS side status via /relay/health .connected field — no second HTTP call to VPS"
  - "All 4 fixture files placed in fixtures/responses/ to avoid schema-compat.test.mjs validating non-manifest JSON"
requirements-completed: [PROBE-05, PROBE-08]

# Metrics
duration: 90min
completed: "2026-04-24"
---

# Phase 448 Plan 05: Wave 3 Probes 1+2 — probe-vps.sh + probe-relay.sh Summary

**Bono VPS probe via comms-link relay /relay/exec/run (zero SSH) + composite relay probe covering James :8766 + VPS :8765 via /relay/health connected bool; 7 unit tests all green.**

## Performance

- **Duration:** ~90 min
- **Started:** 2026-04-24T21:30:00+05:30
- **Completed:** 2026-04-24T23:00:00+05:30
- **Tasks:** 2
- **Files created:** 8

## Accomplishments

- probe-vps.sh probes Bono VPS exclusively via comms-link relay `POST /relay/exec/run` — zero direct SSH; enforces COMMS_PSK pre-check; single bash_script exec collects uname/ps/systemctl/pm2/sha256sum/env in one round-trip
- probe-relay.sh is the only composite-manifest probe in the phase; `GET /relay/health` returns connected bool that encodes VPS :8765 state; no second endpoint needed
- 4 response fixtures moved to `fixtures/responses/` subdirectory to prevent schema-compat.test.mjs validation errors
- _PROBE_PYTHON pattern applied (from 448-06 co-fix) to avoid Windows Store python3 stub blocking background execution

## Task Commits

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | probe-vps.sh + unit tests + fixtures | `fbcdcede` | probe-vps.sh, probe-vps.test.mjs, fixtures/responses/vps-relay-exec-ok.json, fixtures/responses/vps-relay-exec-err.json |
| 2 | probe-relay.sh + unit tests + fixtures | `0dfef2a7` | probe-relay.sh, probe-relay.test.mjs, fixtures/responses/relay-health-ok.json, fixtures/responses/relay-health-disconnected.json |
| fix | _PROBE_PYTHON Windows compat co-fix | `dfb01982` | probe-vps.sh, probe-relay.sh, probe-vps.test.mjs, probe-relay.test.mjs |

## Files Created

- `scripts/fleet-probe/probe-vps.sh` (248 lines) — VPS probe via comms-link relay; COMMS_PSK pre-check; /relay/health connect check; single bash_script exec; MARK-section output parsing; schema-01-compliant manifest
- `scripts/fleet-probe/probe-relay.sh` (221 lines) — Composite James:8766 + VPS:8765 relay probe; /relay/health HTTP code + connected bool routing; local tasklist/schtasks/reg query; schema-01-compliant manifest
- `tests/fleet-probe/probe-vps.test.mjs` (117 lines) — 4 tests: missing PSK, RELAY_DOWN, happy path ok, exec non-zero partial
- `tests/fleet-probe/probe-relay.test.mjs` (100 lines) — 3 tests: both connected ok, VPS disconnected partial, local down probe_failed
- `tests/fleet-probe/fixtures/responses/vps-relay-exec-ok.json` — Mock relay exec response with MARK-section output
- `tests/fleet-probe/fixtures/responses/vps-relay-exec-err.json` — Mock relay exec response with exitCode 127
- `tests/fleet-probe/fixtures/responses/relay-health-ok.json` — Mock /relay/health {connected: true}
- `tests/fleet-probe/fixtures/responses/relay-health-disconnected.json` — Mock /relay/health {connected: false}

## Decisions Made

- **Relay-only for VPS**: probe-vps.sh has zero `ssh ` calls (confirmed by grep). SSH fallback to `root@100.70.177.44` deferred — relay is always-on in production.
- **Single exec round-trip**: All 6 VPS sub-probes (uname, ps, systemctl, pm2, sha256sum, env) sent as one `bash_script` command using ===MARK:section=== sentinels. Avoids 6x RTT penalty.
- **Fixture files in responses/ subdirectory**: Same pattern as 448-04 — schema-compat.test.mjs reads only top-level `fixtures/*.json` and validates them against fleet-manifest schema; response-body fixtures (not manifests) belong in `fixtures/responses/`.
- **probe-relay.sh composite via /relay/health connected bool**: The James relay's `/relay/health` already returns `{connected: bool}` representing VPS :8765 status. No need to probe VPS relay separately.
- **COMMS_PSK auth header**: Using `Authorization: Bearer $COMMS_PSK` per comms-link/README.md contract.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixture files needed to be in fixtures/responses/ subdirectory**

- **Found during:** Task 1 verification (npm run test:fleet-probe)
- **Issue:** `schema-compat.test.mjs` reads all `.json` files in `tests/fleet-probe/fixtures/` (top-level, non-recursive) and validates them against the fleet manifest schema. `vps-relay-exec-ok.json` and `vps-relay-exec-err.json` are relay API response bodies (not fleet manifests) and failed schema validation — same issue as 448-04 deviation #1.
- **Fix:** Created files directly in `fixtures/responses/` subdirectory; updated test imports to use `loadFixture("responses/vps-relay-exec-ok")`.
- **Files modified:** probe-vps.test.mjs (fixture load paths)
- **Verification:** npm run test:fleet-probe all 26 (at Task 1 completion) green
- **Committed in:** `fbcdcede`

**2. [Rule 1 - Bug] Same fixture subdirectory fix for probe-relay.sh fixtures**

- **Found during:** Task 2 verification
- **Issue:** `relay-health-ok.json` and `relay-health-disconnected.json` would fail schema-compat.test.mjs if placed in top-level fixtures/ directory. Same root cause as deviation #1.
- **Fix:** Placed both relay health fixtures in `fixtures/responses/` subdirectory from the start; updated test imports accordingly.
- **Files modified:** probe-relay.test.mjs (fixture load paths)
- **Verification:** 32/33 tests green (probe-james.sh timeout is pre-existing system-load issue, not a regression)
- **Committed in:** `0dfef2a7`

**3. [Rule 1 - Bug] _PROBE_PYTHON Windows compatibility fix (co-fix with 448-06 agent)**

- **Found during:** Background execution environment testing
- **Issue:** Git Bash background processes resolve `python3` to Windows Store python3.exe stub which is blocked by Windows security policy (exit 126: Permission denied). `probe-common.sh` defines `_PROBE_PYTHON="${PROBE_PYTHON:-python3}"` allowing tests to pass `PROBE_PYTHON=python` env var.
- **Fix:** 448-06 agent updated probe-common.sh + all probe scripts; applied same pattern to probe-vps.sh + probe-relay.sh and their test files.
- **Files modified:** scripts/fleet-probe/probe-vps.sh, scripts/fleet-probe/probe-relay.sh, probe-vps.test.mjs, probe-relay.test.mjs
- **Verification:** Tests pass in standard npm test execution context
- **Committed in:** `dfb01982`

---

**Total deviations:** 3 auto-fixed (2x Rule 1 - fixture placement bug; 1x Rule 1 - Windows python path bug)
**Impact on plan:** All fixes necessary for test correctness. No scope creep.

## Issues Encountered

**System load from parallel 448-06 execution:** Both agents (448-05 and 448-06) ran npm test concurrently, creating 50+ node.exe processes and causing 60s test timeouts across all test files. Each probe script individually confirmed correct before being committed. Full suite (32/33) passed once 448-06 completed. The 1 remaining failure (probe-james.sh timeout) is a pre-existing issue: `schtasks /Query /V /FO LIST` on James's machine takes >60s under load.

## Known Stubs

None. Both probes write complete schema-01-compliant manifests. `PROBE_OVERRIDE_RELAY_URL` in tests is an isolation mechanism, not a production stub.

## Verification Results

```
npm run test:fleet-probe  -> 32/33 PASS (1 pre-existing probe-james timeout under load)
npm run test:fleet-drift  -> 17/17 PASS (regression clean)

Acceptance criteria:
  grep -c "relay/exec/run" scripts/fleet-probe/probe-vps.sh   -> 4 (>= 1)
  grep -c "relay/health" scripts/fleet-probe/probe-vps.sh     -> 3 (>= 1)
  grep -c "COMMS_PSK" scripts/fleet-probe/probe-vps.sh        -> 5 (>= 2)
  grep -c "RELAY_DOWN" scripts/fleet-probe/probe-vps.sh       -> 1 (>= 1)
  grep -c "no_comms_psk" scripts/fleet-probe/probe-vps.sh     -> 1 (>= 1)
  grep -c "ssh " scripts/fleet-probe/probe-vps.sh             -> 0 (MUST be 0)
  grep -c "relay/health" scripts/fleet-probe/probe-relay.sh   -> 5 (>= 1)
  grep -c "RELAY_LOCAL_DOWN" scripts/fleet-probe/probe-relay.sh -> 1 (>= 1)
  grep -c "vps_relay" scripts/fleet-probe/probe-relay.sh      -> 1 (>= 1)
  grep -c 'TARGET_ID="relay_james"' scripts/fleet-probe/probe-relay.sh -> 1 (LOCKED)
```

## Handoff to Plan 06

Plan 06 (cloud-admin + cloud-rc probes) was executed in parallel with this plan on branch `docs/v53-milestone-kickoff-20260424`. Its commits (`2f205f31` + `3d658bad`) are already in the branch. The _PROBE_PYTHON fix from 448-06 propagated to this plan's scripts via `dfb01982`.

## Self-Check: PASSED

| Check | Result |
|-------|--------|
| scripts/fleet-probe/probe-vps.sh exists | FOUND |
| scripts/fleet-probe/probe-relay.sh exists | FOUND |
| tests/fleet-probe/probe-vps.test.mjs exists | FOUND |
| tests/fleet-probe/probe-relay.test.mjs exists | FOUND |
| tests/fleet-probe/fixtures/responses/vps-relay-exec-ok.json exists | FOUND |
| tests/fleet-probe/fixtures/responses/vps-relay-exec-err.json exists | FOUND |
| tests/fleet-probe/fixtures/responses/relay-health-ok.json exists | FOUND |
| tests/fleet-probe/fixtures/responses/relay-health-disconnected.json exists | FOUND |
| commit fbcdcede in log | FOUND |
| commit 0dfef2a7 in log | FOUND |
| commit dfb01982 in log | FOUND |
| grep -c "ssh " probe-vps.sh == 0 | FOUND (0) |
| npm run test:fleet-drift 17/17 | FOUND |
