---
phase: 112-rtsp-infrastructure-camera-pipeline
plan: 04
subsystem: infra
tags: [rtsp, go2rtc, people-tracker, opencv, camera-relay]

requires:
  - phase: 112-01
    provides: "go2rtc RTSP relay running on James (.27) at localhost:8554"
provides:
  - "People tracker reads all 3 camera streams via go2rtc relay instead of direct camera connections"
  - "rtsp_url override pattern in CameraProcessor for flexible stream sourcing"
affects: [112-03, rc-sentry-ai]

tech-stack:
  added: []
  patterns: ["rtsp_url config override with direct-connection fallback"]

key-files:
  created: []
  modified:
    - "people-tracker/config.yaml"
    - "people-tracker/main.py"

key-decisions:
  - "Added rtsp_url as optional override field per camera — preserves direct-connection fallback when rtsp_url is absent"
  - "Kept camera_auth and ip fields for fallback and documentation purposes"

patterns-established:
  - "RTSP URL override: config.yaml rtsp_url field takes priority over auto-constructed URL from ip + auth"

requirements-completed: [CAM-04]

duration: 8min
completed: 2026-03-21
---

# Phase 112 Plan 04: People Tracker RTSP Migration Summary

**People tracker migrated to read all 3 camera RTSP streams from go2rtc relay at localhost:8554, eliminating direct camera connections**

## Performance

- **Duration:** 8 min
- **Started:** 2026-03-21T20:30:00+05:30
- **Completed:** 2026-03-21T20:44:00+05:30
- **Tasks:** 2 (1 auto + 1 human-verify checkpoint)
- **Files modified:** 2

## Accomplishments
- People tracker config.yaml updated with rtsp_url overrides for all 3 cameras (entrance, reception, reception_wide) pointing at go2rtc relay
- CameraProcessor.__init__ extended with optional rtsp_url parameter — uses relay URL when present, falls back to direct camera connection when absent
- CameraProcessor instantiation updated to pass rtsp_url from config
- Human verification confirmed people counting works correctly via go2rtc relay

## Task Commits

Each task was committed atomically:

1. **Task 1: Add rtsp_url override to people tracker config and code** - `87d4ced` (feat) [in people-tracker repo]
2. **Task 2: Verify people tracker works via go2rtc relay** - checkpoint:human-verify (approved)

**Plan metadata:** (this commit) (docs: complete plan)

## Files Created/Modified
- `people-tracker/config.yaml` - Added rtsp_url fields pointing at go2rtc relay (127.0.0.1:8554) for all 3 cameras
- `people-tracker/main.py` - Added rtsp_url parameter to CameraProcessor.__init__ with override logic and updated instantiation

## Decisions Made
- Used optional rtsp_url config field rather than replacing the URL construction entirely, preserving backward compatibility
- Kept camera_auth and ip fields intact for fallback and reference

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- People tracker fully migrated to go2rtc relay
- Plan 112-03 (rc-sentry-ai camera pipeline) can proceed with the same relay pattern
- All direct camera connections from people tracker eliminated, freeing RTSP connection slots

## Self-Check: PASSED

- FOUND: people-tracker/config.yaml
- FOUND: people-tracker/main.py
- FOUND: 112-04-SUMMARY.md
- FOUND: commit 87d4ced

---
*Phase: 112-rtsp-infrastructure-camera-pipeline*
*Completed: 2026-03-21*
