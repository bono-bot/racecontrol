---
phase: 112-rtsp-infrastructure-camera-pipeline
plan: 01
subsystem: infra
tags: [go2rtc, rtsp, camera, dahua, relay, streaming]

# Dependency graph
requires: []
provides:
  - "go2rtc RTSP relay at :8554 proxying 3 Dahua cameras"
  - "go2rtc API at :1984 for stream health checks"
  - "Auto-start via HKLM Run key on James (.27)"
affects: [rc-sentry-ai, people-tracker, face-detection]

# Tech tracking
tech-stack:
  added: [go2rtc v1.9.13]
  patterns: [RTSP relay fan-out, URL-encoded credentials in RTSP URIs]

key-files:
  created:
    - "C:\\RacingPoint\\go2rtc\\go2rtc.exe"
    - "C:\\RacingPoint\\go2rtc\\go2rtc.yaml"
    - "C:\\RacingPoint\\start-go2rtc.bat"
  modified: []

key-decisions:
  - "go2rtc runs independently via bat file, not managed by rc-sentry-ai"
  - "Firewall rule allows program-level access rather than port-specific"
  - "HKLM Run key for auto-start (consistent with rc-agent, racecontrol patterns)"

patterns-established:
  - "RTSP relay: go2rtc single binary at C:\\RacingPoint\\go2rtc\\ with YAML config"
  - "Camera auth: URL-encode special chars (Admin%40123) in RTSP URLs"

requirements-completed: [CAM-01, CAM-02]

# Metrics
duration: 2min
completed: 2026-03-21
---

# Phase 112 Plan 01: go2rtc RTSP Relay Setup Summary

**go2rtc v1.9.13 installed on James (.27) relaying sub-streams from 3 Dahua cameras (entrance .8, reception .15, reception_wide .154) at :8554 with API at :1984 and auto-start via HKLM Run key**

## Performance

- **Duration:** 2 min
- **Started:** 2026-03-21T15:04:10Z
- **Completed:** 2026-03-21T15:06:20Z
- **Tasks:** 2
- **Files created:** 3 (outside repo, at C:\RacingPoint\)

## Accomplishments
- go2rtc v1.9.13 downloaded and extracted to C:\RacingPoint\go2rtc\go2rtc.exe (18MB binary)
- go2rtc.yaml configured with 3 camera sub-streams using URL-encoded credentials
- Windows Firewall inbound rule "go2rtc RTSP Relay" added for program-level access
- API verified: curl http://127.0.0.1:1984/api/streams returns all 3 stream names
- Auto-start bat file created at C:\RacingPoint\start-go2rtc.bat with CRLF line endings
- HKLM Run key registered for boot-time auto-start

## Task Commits

Infrastructure files are outside the git repo (C:\RacingPoint\). No in-repo source changes for this plan.

1. **Task 1: Download go2rtc and create config + firewall rule** - infrastructure (no repo commit, files at C:\RacingPoint\go2rtc\)
2. **Task 2: Create auto-start bat file for go2rtc** - infrastructure (no repo commit, files at C:\RacingPoint\)

**Plan metadata:** included in docs commit below

## Files Created/Modified
- `C:\RacingPoint\go2rtc\go2rtc.exe` - go2rtc v1.9.13 RTSP relay binary (18MB)
- `C:\RacingPoint\go2rtc\go2rtc.yaml` - Stream config: entrance (.8), reception (.15), reception_wide (.154), API :1984, RTSP :8554
- `C:\RacingPoint\start-go2rtc.bat` - Auto-start script with CRLF line endings, cd /D to go2rtc dir

## Decisions Made
- go2rtc runs independently (not managed by rc-sentry-ai) -- simpler, more reliable, per research recommendation
- Firewall rule uses program-level allow (not port-specific) -- covers all go2rtc ports including WebRTC :8555
- HKLM Run key for auto-start -- consistent with existing rc-agent and racecontrol patterns on other machines

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
- Firewall rule and registry key required admin elevation -- used PowerShell Start-Process with -Verb RunAs

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- go2rtc running at :8554 (RTSP) and :1984 (API) on James (.27)
- Ready for Plan 02 (rc-sentry-ai crate) to connect via retina to relayed streams
- Ready for people tracker migration to use go2rtc relay URLs instead of direct camera connections
- go2rtc is currently running in this session; will auto-start on next boot via HKLM Run key

---
*Phase: 112-rtsp-infrastructure-camera-pipeline*
*Completed: 2026-03-21*
