---
phase: 210-startup-enforcement-and-fleet-audit
plan: 02
subsystem: infra
tags: [audit, bat-drift, config-fallback, boot-resilience, sentinel, verification-chains, deploy]

# Dependency graph
requires:
  - phase: 210-01
    provides: bat-scanner.sh with bat_scan_pod, bat_scan_all, bat_validate_syntax functions
provides:
  - 5 new audit phases (61-65) covering v25.0 debug quality dimensions
  - deploy-pod.sh bat file sync step for every deploy
  - Debug Quality report section in audit report
affects: [audit-framework, deploy-pipeline, fleet-audit]

# Tech tracking
tech-stack:
  added: []
  patterns: [audit-phase-pattern, per-pod-emit-result, fleet-summary-emit, venue-state-aware-quiet]

key-files:
  created:
    - audit/phases/tier2/phase61.sh
    - audit/phases/tier2/phase62.sh
    - audit/phases/tier2/phase63.sh
    - audit/phases/tier3/phase64.sh
    - audit/phases/tier3/phase65.sh
  modified:
    - audit/audit.sh
    - scripts/deploy-pod.sh
    - audit/lib/report.sh

key-decisions:
  - "Phase 61 bat-drift uses bat_scan_pod_json for structured output parsing in audit context"
  - "Phase 62 config-fallback reads racecontrol.toml via rc-sentry /files endpoint to check for OBS-02 default values"
  - "Phase 64 sentinel-alerts is fleet-level (not per-pod) since active_sentinels is a fleet health response field"
  - "Phase 65 verification-chains uses server uptime as proxy when chain fields not yet in health response"
  - "deploy-pod.sh copies bat files to BINARY_DIR to reuse existing HTTP server instead of starting a second one"

patterns-established:
  - "v25.0 audit phases 61-65 follow existing emit_result pattern with venue-state awareness"
  - "Deploy pipeline includes bat sync as mandatory step between binary swap and restart"

requirements-completed: [BAT-03, BAT-04, AUDIT-01, AUDIT-02, AUDIT-03]

# Metrics
duration: 4min
completed: 2026-03-26
---

# Phase 210 Plan 02: Audit Integration & Deploy Bat Sync Summary

**5 audit phases (bat-drift, config-fallback, boot-resilience, sentinel-alerts, verification-chains) with deploy-pipeline bat sync and Debug Quality report section**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-26T06:54:26Z
- **Completed:** 2026-03-26T06:59:02Z
- **Tasks:** 2
- **Files modified:** 8

## Accomplishments
- Created 5 new audit phase scripts covering all v25.0 debug quality dimensions
- Registered phases 61-65 in audit.sh tier dispatch and mode-based run functions
- Added bat file sync step to deploy-pod.sh (after swap, before start)
- Added v25.0 Debug Quality per-pod summary table to audit report

## Task Commits

Each task was committed atomically:

1. **Task 1: Create 5 new audit phase scripts and register in audit.sh** - `cf803186` (feat)
2. **Task 2: Add bat sync to deploy-pod.sh and Debug Quality report section** - `88f7c54c` (feat)

## Files Created/Modified
- `audit/phases/tier2/phase61.sh` - Bat file drift detection per-pod via bat-scanner.sh
- `audit/phases/tier2/phase62.sh` - Config fallback detection checking for OBS-02 defaults
- `audit/phases/tier2/phase63.sh` - Boot resilience checking periodic_tasks in health
- `audit/phases/tier3/phase64.sh` - Sentinel alert wiring via fleet health active_sentinels
- `audit/phases/tier3/phase65.sh` - Verification chain health via server build and uptime
- `audit/audit.sh` - Updated dispatch for phases 61-65, usage comment updated to 65 phases
- `scripts/deploy-pod.sh` - Bat sync step + post-deploy bat verification
- `audit/lib/report.sh` - v25.0 Debug Quality section with per-pod summary table

## Decisions Made
- Phase 61 uses bat_scan_pod_json (not bat_scan_pod) for structured JSON output parsing in audit context
- Phase 62 fetches racecontrol.toml via rc-sentry /files endpoint rather than health endpoint parsing
- Phase 64 is fleet-level check (not per-pod) since active_sentinels is a fleet health response field
- Phase 65 falls back to uptime check as proxy when verification chain fields not exposed in health yet
- deploy-pod.sh reuses existing HTTP server by copying bat files to BINARY_DIR before server starts

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- All 5 audit phases ready for live fleet audit (requires pods online)
- Deploy pipeline bat sync ready for next deploy cycle
- Debug Quality section will appear in audit reports automatically when phases 61-65 run

---
*Phase: 210-startup-enforcement-and-fleet-audit*
*Completed: 2026-03-26*
