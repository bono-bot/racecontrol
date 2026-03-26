---
phase: 210-startup-enforcement-and-fleet-audit
plan: 01
subsystem: infra
tags: [bash, audit, drift-detection, bat-files, fleet-management, rc-sentry]

requires:
  - phase: none
    provides: standalone script, no phase dependencies
provides:
  - bat_scan_pod() function for per-pod bat file drift detection via rc-sentry /files
  - bat_validate_syntax() for 5 known bat anti-pattern checks
  - bat_scan_all() for fleet-wide scan with summary
  - JSON output mode for audit framework integration
affects: [210-startup-enforcement-and-fleet-audit, audit-phases, deploy-pod]

tech-stack:
  added: []
  patterns: [source-able audit scripts with standalone CLI, SHA256 drift detection with line-level diff]

key-files:
  created: [scripts/bat-scanner.sh]
  modified: []

key-decisions:
  - "timeout /nobreak variant not flagged (works in HKLM Run startup context); bare timeout still flagged"
  - "ConspitLink2.0.exe added to bloatware skip list for taskkill-without-restart check (intentional singleton kill)"

patterns-established:
  - "Audit scanner pattern: source-able functions + standalone CLI with --json for audit integration"
  - "Drift detection: strip CR, SHA256 compare, diff on mismatch"

requirements-completed: [BAT-01, BAT-02]

duration: 4min
completed: 2026-03-26
---

# Phase 210 Plan 01: Bat Scanner Summary

**Bat file drift detection + syntax validation scanner for 8-pod fleet via rc-sentry /files endpoint with 5 anti-pattern checks**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-26T06:48:09Z
- **Completed:** 2026-03-26T06:52:01Z
- **Tasks:** 1
- **Files modified:** 1

## Accomplishments
- Created scripts/bat-scanner.sh with full drift detection (SHA256 + line-level diff) for start-rcagent.bat and start-rcsentry.bat
- Implemented 5 syntax validators: UTF-8 BOM, parentheses in if/else, /dev/null, timeout, taskkill-without-restart
- Standalone CLI with --pod, --validate, --all, --json, --help modes
- Source-able for audit integration (bat_scan_pod, bat_scan_all, bat_validate_syntax functions available when sourced)

## Task Commits

Each task was committed atomically:

1. **Task 1: Create bat-scanner.sh with drift detection and syntax validation** - `0d14976b` (feat)

## Files Created/Modified
- `scripts/bat-scanner.sh` - Bat file drift detection + syntax validation scanner (standalone + audit-callable)

## Decisions Made
- `timeout /nobreak` variant is not flagged as a violation since it works in the HKLM Run startup context where canonical bat files execute; bare `timeout` without `/nobreak` is still flagged
- ConspitLink2.0.exe added to the bloatware skip list for the taskkill-without-restart check, since ConspitLink is intentionally killed as a singleton guard before rc-agent starts it

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Adjusted timeout check to avoid false positive on canonical bat file**
- **Found during:** Task 1 (verification)
- **Issue:** Canonical start-rcagent.bat uses `timeout /t 3 /nobreak` which would trigger the timeout check, causing the canonical file to fail validation (contradicting acceptance criteria)
- **Fix:** Modified timeout check to skip lines containing `/nobreak` (works in HKLM Run context)
- **Files modified:** scripts/bat-scanner.sh
- **Verification:** `bash scripts/bat-scanner.sh --validate scripts/deploy/start-rcagent.bat` exits 0
- **Committed in:** 0d14976b

**2. [Rule 1 - Bug] Added ConspitLink2.0.exe to bloatware skip list**
- **Found during:** Task 1 (verification)
- **Issue:** ConspitLink2.0.exe is killed on line 19 of canonical bat without restart, which would trigger taskkill-without-restart false positive
- **Fix:** Added ConspitLink2.0.exe to the intentional-kill skip list
- **Files modified:** scripts/bat-scanner.sh
- **Verification:** Canonical bat passes validation with 0 violations
- **Committed in:** 0d14976b

---

**Total deviations:** 2 auto-fixed (2 bugs)
**Impact on plan:** Both fixes necessary for canonical bat to pass validation as required by acceptance criteria. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- bat-scanner.sh ready for use in audit phases (source the script and call functions)
- Fleet scan requires pods to be online with rc-sentry running on :8091
- JSON output available for integration with audit/lib/core.sh emit_result pattern

---
*Phase: 210-startup-enforcement-and-fleet-audit*
*Completed: 2026-03-26*
