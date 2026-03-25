---
phase: 181-standing-rules-gate
plan: 01
subsystem: infra
tags: [standing-rules, ota-pipeline, gate-check, registry, json]

requires:
  - phase: 179-ota-state-machine
    provides: OTA pipeline state machine and wave constants
provides:
  - Machine-readable standing-rules-registry.json with 76 classified rules
  - OTA Pipeline standing rules subsection in CLAUDE.md (6 new rules)
affects: [181-02 gate-check.sh, 182-admin-ota-ui, ota-pipeline]

tech-stack:
  added: []
  patterns: [standing-rules-registry JSON schema, AUTO/HUMAN-CONFIRM/INFORMATIONAL classification]

key-files:
  created: [standing-rules-registry.json]
  modified: [CLAUDE.md]

key-decisions:
  - "SR-ULTIMATE-001 classified as HUMAN-CONFIRM (requires manual E2E + visual verification, not fully automatable)"
  - "76 total rules (18 AUTO, 19 HUMAN-CONFIRM, 39 INFORMATIONAL) exceeds plan minimum of 50"
  - "All AUTO check_commands use exit-code-based checks (grep -q, wc -l | grep -q) for gate-check.sh integration"
  - "OTA rules use category 'ota-pipeline' distinct from existing 9 categories"

patterns-established:
  - "Standing rule registry schema: id, category, summary, type, check_command, checklist"
  - "AUTO rules must return exit 0 (pass) or non-zero (fail) for pipeline gating"
  - "HUMAN-CONFIRM rules must have checklist array for operator confirmation"

requirements-completed: [SR-01, SR-05]

duration: 6min
completed: 2026-03-25
---

# Phase 181 Plan 01: Standing Rules Registry & OTA Rules Summary

**76 standing rules classified into AUTO/HUMAN-CONFIRM/INFORMATIONAL registry with 6 new OTA Pipeline rules added to CLAUDE.md**

## Performance

- **Duration:** 6 min
- **Started:** 2026-03-25T10:17:36+05:30
- **Completed:** 2026-03-25T10:23:36+05:30
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments
- Created standing-rules-registry.json with 76 classified rules (18 AUTO, 19 HUMAN-CONFIRM, 39 INFORMATIONAL)
- All AUTO rules have exit-code-based check_commands ready for gate-check.sh consumption
- Added 6 OTA Pipeline standing rules to CLAUDE.md with _Why_ explanations
- 6 OTA-pipeline entries in registry (SR-OTA-001 through SR-OTA-006)

## Task Commits

Each task was committed atomically:

1. **Task 1: Create standing-rules-registry.json with all rules classified** - `634a99bf` (feat)
2. **Task 2: Add OTA Pipeline standing rules subsection to CLAUDE.md** - `80a5d21d` (feat)

## Files Created/Modified
- `standing-rules-registry.json` - Machine-readable registry of all 76 standing rules with classification
- `CLAUDE.md` - New "### OTA Pipeline" subsection with 6 standing rules after Security section

## Decisions Made
- SR-ULTIMATE-001 (three verification layers) classified as HUMAN-CONFIRM rather than AUTO since it requires manual E2E and visual verification steps that cannot be fully automated
- Pre-existing file had 72 entries; improved to 76 by adding missing AUTO rules (hardcoded IPs, force push, git config, standing rules sync)
- Fixed several AUTO check_commands that used `echo` or `head` instead of proper exit-code-returning checks (grep -q, wc -l | grep -q)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed AUTO check_commands that did not return proper exit codes**
- **Found during:** Task 1
- **Issue:** Several AUTO entries used `echo` or `head` which always return exit 0 regardless of match
- **Fix:** Replaced with proper exit-code checks using `wc -l | grep -q '^0$'` pattern
- **Files modified:** standing-rules-registry.json
- **Verification:** All 18 AUTO entries verified to have non-null check_command strings
- **Committed in:** 634a99bf

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** Fix was necessary for gate-check.sh to work correctly. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- standing-rules-registry.json ready for gate-check.sh (Plan 02) to consume
- CLAUDE.md OTA Pipeline section ready for Bono sync (standing rules sync rule applies)

---
*Phase: 181-standing-rules-gate*
*Completed: 2026-03-25*
