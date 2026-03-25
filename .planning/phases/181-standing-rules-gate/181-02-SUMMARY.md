---
phase: 181-standing-rules-gate
plan: 02
subsystem: infra
tags: [standing-rules, gate-check, ota-pipeline, shell-script, pre-deploy, post-wave]

requires:
  - phase: 181-01
    provides: standing-rules-registry.json with 76 classified rules (AUTO/HUMAN-CONFIRM/INFORMATIONAL)
provides:
  - test/gate-check.sh with --pre-deploy and --post-wave enforcement modes
  - Pipeline gating with exit codes 0 (pass), 1 (fail/rollback), 2 (HUMAN-CONFIRM pause)
  - Automated execution of all 18 AUTO standing rule check_commands
  - Operator checklists for HUMAN-CONFIRM rules triggered by diff context
affects: [181-03 pipeline integration, 182-admin-ota-ui, ota-pipeline, deploy workflow]

tech-stack:
  added: []
  patterns: [suite-based gate script following comms-link run-all.sh pattern, node-based JSON registry parsing, diff-aware HUMAN-CONFIRM triggering]

key-files:
  created: [test/gate-check.sh]
  modified: []

key-decisions:
  - "HUMAN-CONFIRM rules are context-aware: display/visual rules trigger only when display-affecting files change in diff"
  - "Suite ordering: comms-link E2E (0) -> cargo tests (1) -> AUTO standing rules (2) -> diff analysis (3) -> HUMAN-CONFIRM checklist (4)"
  - "Post-wave mode has different suites: comms-link E2E (0) -> build ID verify (1) -> fleet health (2) -> AUTO standing rules (3)"
  - "Exit code priority: failures (exit 1) take precedence over HUMAN-CONFIRM pending (exit 2)"

patterns-established:
  - "Gate script extends comms-link run-all.sh as Suite 0 (SYNC-06 compliance)"
  - "AUTO rules parsed from standing-rules-registry.json via node require() — no jq dependency"
  - "Triple-separator (|||) for parsing multi-field lines in shell"

requirements-completed: [SR-02, SR-03, SR-06, SYNC-06]

duration: 2min
completed: 2026-03-25
---

# Phase 181 Plan 02: Gate Check Script Summary

**gate-check.sh with 5 pre-deploy suites (comms-link E2E, cargo tests, 18 AUTO standing rules, diff analysis, HUMAN-CONFIRM checklists) and 4 post-wave suites (comms-link E2E, build ID verification, fleet health, AUTO standing rules)**

## Performance

- **Duration:** 2 min
- **Started:** 2026-03-25T10:27:25+05:30
- **Completed:** 2026-03-25T10:29:45+05:30
- **Tasks:** 1
- **Files modified:** 1

## Accomplishments
- Created 479-line gate-check.sh that enforces all standing rules as pipeline gates
- Pre-deploy mode runs 5 suites: comms-link E2E framework, cargo workspace tests, 18 AUTO standing rule checks from registry, diff analysis (unwrap/any/manifest), and HUMAN-CONFIRM operator checklists
- Post-wave mode runs 4 suites: comms-link E2E, build ID from release manifest, fleet health endpoint with ws_connected verification, and AUTO standing rules
- Exit code contract: 0=pass, 1=fail (rollback), 2=HUMAN-CONFIRM pending (pause pipeline)
- No --force or --skip-gate flags exist anywhere in the script (SR-04 partial compliance)

## Task Commits

Each task was committed atomically:

1. **Task 1: Create test/gate-check.sh with pre-deploy and post-wave modes** - `bd7e5447` (feat)

## Files Created/Modified
- `test/gate-check.sh` - Standing rules enforcement gate script with --pre-deploy and --post-wave modes

## Decisions Made
- Used node require() to parse standing-rules-registry.json instead of jq (node is always available on James's machine, jq may not be)
- HUMAN-CONFIRM rules are diff-context-aware: display/visual rules only trigger when display-affecting files change, guard/filter rules trigger when guard files change, ultimate and deploy/OTA rules always trigger
- Comms-link run-all.sh is called as Suite 0 in both modes, with a graceful skip (warn, not fail) if comms-link repo is not found on the machine

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- gate-check.sh ready for OTA pipeline integration (Plan 03)
- Script verified: syntax check passes, runs successfully, all acceptance criteria met
- Both --pre-deploy and --post-wave modes implemented and functional

---
*Phase: 181-standing-rules-gate*
*Completed: 2026-03-25*
