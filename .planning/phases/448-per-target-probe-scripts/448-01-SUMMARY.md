---
phase: 448-per-target-probe-scripts
plan: 01
subsystem: testing
tags: [bash, node, ajv, fleet-probe, schema-validation, test-helpers]

# Dependency graph
requires:
  - phase: 447-manifest-schema-scope-lock
    provides: "fleet-manifest.schema.json (LOCKED), ajv 8.17.1 + ajv-formats installed, 17/17 validate-manifest.test.mjs green"
provides:
  - "scripts/fleet-probe/lib/probe-common.sh -- 10-function shared Bash library consumed by all Plans 02-08"
  - "scripts/fleet-probe/validate-manifest-file.mjs -- ajv CLI wrapper for FLEET_PROBE_VALIDATE=1 gate"
  - "tests/fleet-probe/helpers.mjs -- ESM test helpers (startMockHttpServer, makeMockSshEnv, loadFixture, validateAgainstSchema)"
  - "tests/fleet-probe/mock-ssh-responder.sh -- scenario-file SSH mock for offline probe unit tests"
  - "tests/fleet-probe/mock-http-server.mjs -- thin re-export of startMockHttpServer"
  - "3 schema-valid fixtures: server_23_ok, pod_1_partial, pos_130_probe_failed"
  - "tests/fleet-probe/schema-compat.test.mjs -- asserts all fixtures pass ajv validation"
  - "package.json: test:fleet-probe script (node --test tests/fleet-probe/*.test.mjs)"
affects:
  - 448-02-probe-james
  - 448-03-probe-server
  - 448-04-probe-pod-pos
  - 448-05-probe-vps-relay
  - 448-06-probe-cloud
  - 448-07-orchestrator
  - 448-08-access-audit-docs

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "UTC_EPOCH+19800 for IST timestamps in Bash (TZ env override silently fails on Git Bash)"
    - "write_manifest uses FLEET_PROBE_VALIDATE=1 env gate for optional schema validation"
    - "Scenario-file SSH mock: lines before ---EXIT--- to stdout; line after to exit code"
    - "ajv/dist/2020.js import pattern inherited from Phase 447"

key-files:
  created:
    - scripts/fleet-probe/lib/probe-common.sh
    - scripts/fleet-probe/validate-manifest-file.mjs
    - tests/fleet-probe/helpers.mjs
    - tests/fleet-probe/mock-ssh-responder.sh
    - tests/fleet-probe/mock-http-server.mjs
    - tests/fleet-probe/fixtures/server_23_ok.json
    - tests/fleet-probe/fixtures/pod_1_partial.json
    - tests/fleet-probe/fixtures/pos_130_probe_failed.json
    - tests/fleet-probe/schema-compat.test.mjs
  modified:
    - package.json

key-decisions:
  - "json_escape uses python3 for correctness (handles all Unicode + control chars safely)"
  - "write_manifest validates JSON via python3 json.tool before optional ajv check"
  - "mock-ssh-responder.sh uses ---EXIT--- sentinel to separate stdout from exit code in scenario files"
  - "fixtures use e3b0c44298... (sha256 of empty string) as env_vars_hash sentinel when probe_status=probe_failed"

patterns-established:
  - "Pattern: all probe scripts source lib/probe-common.sh and call write_manifest"
  - "Pattern: unit tests use startMockHttpServer for HTTP targets; makeMockSshEnv for SSH targets"
  - "Pattern: fixture names encode target_id + status class (server_23_ok, pod_1_partial, pos_130_probe_failed)"

requirements-completed: [PROBE-09]

# Metrics
duration: 25min
completed: 2026-04-24
---

# Phase 448 Plan 01: Wave 0 Scaffolding Summary

**Shared Bash probe library (10 functions) + ajv CLI validator + ESM test helpers + 3 schema-valid fixtures establish the offline-capable foundation for all 8 per-target probe scripts**

## Performance

- **Duration:** ~25 min
- **Started:** 2026-04-24T14:47:00Z
- **Completed:** 2026-04-24T14:51:34Z
- **Tasks:** 2
- **Files modified:** 11 (10 created + 1 package.json edit)

## Accomplishments

- Shared Bash library (`probe-common.sh`) with all 10 required functions; uses UTC_EPOCH+19800 IST pattern, certutil for Windows SHA256, python3 for JSON escaping
- CLI validator (`validate-manifest-file.mjs`) wraps Phase 447 ajv 2020 pattern; exits 0/1/2 as per contract; used by `write_manifest` under `FLEET_PROBE_VALIDATE=1`
- ESM test helpers providing startMockHttpServer (ephemeral 127.0.0.1:0), makeMockSshEnv (scenario-file SSH mock), loadFixture, and validateAgainstSchema
- 3 schema-valid fixtures covering all 3 probe_status classes (ok, partial, probe_failed)
- `npm run test:fleet-probe`: 2/2 pass; `npm run test:fleet-drift`: 17/17 still green (no regression)

## Task Commits

1. **Task 1: probe-common.sh + validate-manifest-file.mjs** - `dfdc14d1` (feat)
2. **Task 2: test helpers + fixtures + schema-compat + package.json** - `b7ec5cf5` (feat)

## Files Created/Modified

- `scripts/fleet-probe/lib/probe-common.sh` - 10-function shared library (json_escape, write_manifest, sha256_of, sha256_of_stdin, sha256_of_remote_file, iso_ist_now, probe_status_from_errors, env_names_hash, env_names_hash_remote, cmdline_hash)
- `scripts/fleet-probe/validate-manifest-file.mjs` - ajv 2020 CLI: exit 0=valid, 1=invalid, 2=usage error
- `tests/fleet-probe/helpers.mjs` - ESM: startMockHttpServer, makeMockSshEnv, loadFixture, validateAgainstSchema
- `tests/fleet-probe/mock-ssh-responder.sh` - reads PROBE_SSH_SCENARIO, emits stdout+exit code via ---EXIT--- separator
- `tests/fleet-probe/mock-http-server.mjs` - re-exports startMockHttpServer for direct import
- `tests/fleet-probe/fixtures/server_23_ok.json` - schema-valid ok-class fixture (probe_status=ok)
- `tests/fleet-probe/fixtures/pod_1_partial.json` - schema-valid partial-class fixture with probe_errors[]
- `tests/fleet-probe/fixtures/pos_130_probe_failed.json` - schema-valid probe_failed fixture with access_gap
- `tests/fleet-probe/schema-compat.test.mjs` - asserts all 3+ fixtures pass schema validation
- `package.json` - added test:fleet-probe alongside preserved test:fleet-drift

## Decisions Made

- Used python3 for `json_escape` (correctness over complexity -- handles all Unicode, control chars, newlines)
- `write_manifest` validates JSON is well-formed via `python3 -m json.tool` before optional schema gate; removes .tmp on any failure
- mock-ssh-responder.sh uses `---EXIT---` sentinel line to separate stdout body from exit code -- awk-based, no head/tail fragility
- Fixtures use sha256 of empty string (`e3b0c44298...`) as env_vars_hash sentinel when probe_status=probe_failed (schema requires non-empty 64-char hex)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Removed TZ=Asia/Kolkata from probe-common.sh comment**
- **Found during:** Task 1 acceptance criteria check
- **Issue:** Plan acceptance criterion says `grep -c "TZ=Asia/Kolkata" == 0`; the comment documented the banned pattern using the exact string
- **Fix:** Rephrased comment to "TZ env override silently fails on Git Bash" (no banned string)
- **Files modified:** scripts/fleet-probe/lib/probe-common.sh
- **Verification:** `grep -c "TZ=Asia/Kolkata"` returns 0
- **Committed in:** dfdc14d1

---

**Total deviations:** 1 auto-fixed (Rule 1 - comment contained banned grep pattern)
**Impact on plan:** Zero scope change. One-line comment reword to satisfy acceptance criterion.

## Issues Encountered

None - both tasks executed cleanly.

## Known Stubs

None - all fixtures contain concrete values; no TBD_LIVE placeholders in probe-common.sh.

## Open Handoff Items for Plan 02 (probe-james.sh)

- Plan 02 sources `lib/probe-common.sh` via `source "$(dirname "$0")/lib/probe-common.sh"`
- Plan 02 uses `iso_ist_now`, `write_manifest`, `env_names_hash`, `cmdline_hash` from probe-common.sh
- Plan 02 can run `FLEET_PROBE_VALIDATE=1 MANIFEST_TS=test npm run ... && node scripts/fleet-probe/validate-manifest-file.mjs` to gate output
- `makeMockSshEnv` in helpers.mjs is NOT needed for probe-james.sh (localhost, no SSH) but startMockHttpServer is available if probe-james.sh probes local HTTP endpoints

## Next Phase Readiness

- All Wave 0 files from `448-VALIDATION.md` Wave 0 Requirements exist on disk
- `npm run test:fleet-probe` exits 0; `npm run test:fleet-drift` exits 0
- Plans 02-08 can source `lib/probe-common.sh` and use `tests/fleet-probe/helpers.mjs` immediately
- No blockers for Plan 02

---
*Phase: 448-per-target-probe-scripts*
*Completed: 2026-04-24*
