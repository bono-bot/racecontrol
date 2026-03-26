# Phase 204: Cross-Service and UI End-to-End - Context

**Gathered:** 2026-03-26
**Status:** Ready for planning

<domain>
## Phase Boundary

Add 5 cross-service dependency checks and UI rendering verifications to audit phase scripts. Requirements: XS-01, XS-02, UI-01, UI-02, UI-03.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion -- pure infrastructure phase. All changes are bash script edits to existing audit phase scripts.

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- http_get in audit/lib/core.sh -- curl wrapper with timeout
- safe_remote_exec in audit/lib/core.sh -- remote exec via rc-agent :8090
- emit_result in audit/lib/results.sh -- standardized result emission

### Integration Points
- Phase 35: audit/phases/tier7/phase35.sh (cloud sync)
- Phase 36: audit/phases/tier7/phase36.sh (data integrity)
- Phase 07: audit/phases/tier1/phase07.sh (process guard -- already modified in 203)
- Phase 09: audit/phases/tier1/phase09.sh (self-monitor -- already modified in 203)
- Phase 20: audit/phases/tier3/phase20.sh (kiosk browser)
- Phase 26: audit/phases/tier5/phase26.sh (game catalog)
- Phase 44: audit/phases/tier9/phase44.sh (face detection -- already modified in 203)

### Key URLs for cross-service checks
- Venue drivers: http://192.168.31.23:8080/api/v1/drivers?limit=1
- Cloud drivers: http://100.70.177.44:8080/api/v1/drivers?limit=1
- Kiosk static: http://192.168.31.23:3300/kiosk (extract _next/static/ URL then verify)
- Cameras page: http://192.168.31.27:3200/cameras (Next.js cameras page)
- Game selection: http://192.168.31.23:3300/kiosk/games or embedded in kiosk page

</code_context>

<specifics>
## Specific Ideas

No specific requirements -- infrastructure phase.

</specifics>

<deferred>
## Deferred Ideas

None -- discussion stayed within phase scope.

</deferred>
