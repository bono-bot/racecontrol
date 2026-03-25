---
phase: 202-config-validation-structural-fixes
plan: 02
subsystem: audit
tags: [bash, audit, evolution-api, oauth, display-resolution, bat]

requires:
  - phase: 202-01
    provides: TOML validation and server config checks
provides:
  - Evolution API live connection state check in Phase 30
  - OAuth token expiry proactive verification in Phase 31
  - Real display resolution query via rc-agent exec in Phase 19
  - start-rcsentry-ai.bat in repo with go2rtc warmup
affects: [audit-protocol, fleet-deploy, rc-sentry-ai]

tech-stack:
  added: []
  patterns: [safe_remote_exec for pod resolution queries, Evolution API connectionState polling, OAuth token expiry calculation]

key-files:
  created: [scripts/deploy/start-rcsentry-ai.bat]
  modified: [audit/phases/tier6/phase30.sh, audit/phases/tier6/phase31.sh, audit/phases/tier3/phase19.sh]

key-decisions:
  - "Used Get-CimInstance Win32_VideoController for resolution query (safer quoting than GetSystemMetrics P/Invoke)"
  - "Fixed 2>/dev/null to 2>nul in bat file for proper Windows syntax"
  - "Fallback to default Evolution API URL when TOML extraction fails"

patterns-established:
  - "Evolution API connection state check: query /api/instance/connectionState/{instance} for live status"
  - "OAuth token expiry: try multiple token file names, parse epoch ms/s/ISO formats"

requirements-completed: [CV-03, CV-04, SF-01, OP-01]

duration: 3min
completed: 2026-03-26
---

# Phase 202 Plan 02: Config Validation Structural Fixes Summary

**Live Evolution API connection check, OAuth token expiry verification, real display resolution queries, and start-rcsentry-ai.bat added to repo**

## Performance

- **Duration:** 3 min
- **Started:** 2026-03-25T23:23:00Z
- **Completed:** 2026-03-25T23:26:00Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- Phase 30 now tests Evolution API live connection state (not just config presence), emitting FAIL when unreachable
- Phase 31 proactively checks OAuth token expiry across multiple token file names, warns within 7 days
- Phase 19 queries actual display resolution from pods via Get-CimInstance Win32_VideoController instead of hardcoding 1920x1080
- start-rcsentry-ai.bat added to repo with go2rtc stream warmup before rc-sentry-ai launch

## Task Commits

Each task was committed atomically:

1. **Task 1: Fix WhatsApp and email audit checks (CV-03, CV-04)** - `95346ab5` (feat)
2. **Task 2: Fix display resolution hardcoding (SF-01) and add start-rcsentry-ai.bat (OP-01)** - `59544552` (feat)

## Files Created/Modified
- `audit/phases/tier6/phase30.sh` - Added Evolution API connectionState live check with URL extraction from TOML
- `audit/phases/tier6/phase31.sh` - Added OAuth token expiry proactive check across multiple token file names
- `audit/phases/tier3/phase19.sh` - Replaced hardcoded 1920x1080 with Get-CimInstance Win32_VideoController query
- `scripts/deploy/start-rcsentry-ai.bat` - New bat file with go2rtc warmup, log rotation, rc-sentry-ai launch

## Decisions Made
- Used Get-CimInstance Win32_VideoController for resolution query instead of GetSystemMetrics P/Invoke -- safer quoting through cmd.exe
- Fixed Windows bat syntax: 2>/dev/null to 2>nul, >/dev/null to 1>nul 2>nul in repo version
- Fall back to default Evolution API URL (srv1422716.hstgr.cloud:8080) when TOML extraction fails
- Try three possible token file names (gmail-token.json, google-credentials.json, oauth-token.json) before concluding no local token

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All 4 requirements (CV-03, CV-04, SF-01, OP-01) are now covered by updated audit scripts
- Scripts pass bash -n syntax validation
- start-rcsentry-ai.bat ready for deploy to James .27 (matches deployed version with Windows syntax fix)

---
*Phase: 202-config-validation-structural-fixes*
*Completed: 2026-03-26*
