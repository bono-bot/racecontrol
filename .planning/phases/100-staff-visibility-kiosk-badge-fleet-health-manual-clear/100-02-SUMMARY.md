---
phase: 100-staff-visibility-kiosk-badge-fleet-health-manual-clear
plan: 02
subsystem: kiosk
tags: [fleet-health, maintenance, kiosk, badge, pin, next-js, typescript]

# Dependency graph
requires:
  - phase: 100-01
    provides: in_maintenance + maintenance_failures fields in fleet health API JSON; POST /pods/{id}/clear-maintenance endpoint

provides:
  - PodFleetStatus TypeScript type includes in_maintenance and maintenance_failures fields
  - api.clearMaintenance(podId) method calling POST /pods/{id}/clear-maintenance
  - Fleet page shows Racing Red Maintenance badge on pods with in_maintenance=true
  - Clicking badge opens PIN-gated modal showing maintenance_failures list
  - Clear Maintenance button calls api.clearMaintenance and closes modal on success
  - Badge disappears on next 5s poll cycle after maintenance is cleared

affects: [staff-visibility, kiosk-badge, fleet-page, kiosk-ui]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Status functions accept maintenance param — maintenance takes priority over WS/HTTP status in statusBorder/statusLabel/statusLabelColor"
    - "IIFE pattern for conditional modal render: {condition && (() => { ... })()}"
    - "Client-side PIN gate (any 4-digit input) for casual venue TV protection; actual security is the JWT-protected API endpoint"

key-files:
  created: []
  modified:
    - kiosk/src/lib/types.ts
    - kiosk/src/lib/api.ts
    - kiosk/src/app/fleet/page.tsx

key-decisions:
  - "PIN gate accepts any 4-digit input — goal is to prevent casual viewing on venue TV, not strict security; actual ClearMaintenance endpoint requires staff JWT auth"
  - "statusBorder/statusLabel/statusLabelColor: maintenance check runs first so Maintenance state always overrides Healthy/WS Only/HTTP Only/Offline visuals"
  - "offline opacity-50 only applied when !maintenance — maintenance pods show full opacity so the Racing Red badge stands out"

requirements-completed: [STAFF-01, STAFF-02]

# Metrics
duration: 14min
completed: 2026-03-21
---

# Phase 100 Plan 02: Kiosk Fleet Maintenance Badge + PIN-gated Clear Summary

**Racing Red Maintenance badge on fleet pod cards with PIN-gated detail modal showing failure checks and Clear Maintenance button calling POST /pods/{id}/clear-maintenance**

## Performance

- **Duration:** 14 min
- **Started:** 2026-03-21T06:10:55Z
- **Completed:** 2026-03-21T06:24:44Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments

- PodFleetStatus TypeScript interface gains `in_maintenance: boolean` and `maintenance_failures: string[]` fields matching Plan 01 server-side API output
- `api.clearMaintenance(podId)` method added to api.ts, calling POST `/pods/${podId}/clear-maintenance`
- Fleet page status helper functions (`statusBorder`, `statusLabel`, `statusLabelColor`) updated to accept `maintenance` param — Racing Red `#E10600` when in maintenance, takes priority over WS/HTTP status
- Racing Red "Maintenance" badge button appears on pod cards where `in_maintenance=true`; clicking opens modal
- Modal shows PIN input gate (4-digit, any PIN accepted — casual venue TV protection); after PIN, shows `maintenance_failures` list
- "Clear Maintenance" button in modal calls `api.clearMaintenance(pod.pod_id!)` and closes modal on success; badge disappears on next 5s poll
- TypeScript compiles with 0 errors

## Task Commits

Each task was committed atomically:

1. **Task 1: Update types and API client** - `4b9e6a8` (feat)
2. **Task 2: Fleet page maintenance badge with PIN-gated details and clear button** - `af45623` (feat)

**Plan metadata:** (docs commit to follow)

## Files Created/Modified

- `kiosk/src/lib/types.ts` - PodFleetStatus gains `in_maintenance: boolean` and `maintenance_failures: string[]`
- `kiosk/src/lib/api.ts` - `api.clearMaintenance(podId)` method calls POST /pods/{podId}/clear-maintenance
- `kiosk/src/app/fleet/page.tsx` - statusBorder/statusLabel/statusLabelColor accept maintenance param; Maintenance badge; PIN-gated modal with failure list and Clear Maintenance button

## Decisions Made

- PIN gate accepts any 4-digit input: prevents casual viewing on venue TV; strict security handled by JWT-protected server endpoint
- maintenance check runs first in status helpers so `in_maintenance=true` always overrides WS/HTTP visual state
- Pod cards with `in_maintenance=true` render at full opacity so the Racing Red badge is prominent (offline opacity-50 only when !maintenance)

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None. TypeScript compiled cleanly on first attempt.

## User Setup Required

None - no external service configuration required. Kiosk fleet page reads in_maintenance from the existing /api/v1/fleet/health endpoint updated in Plan 01.

## Next Phase Readiness

- STAFF-01 (staff see maintenance badge at a glance) and STAFF-02 (manual clear from dashboard) are now complete end-to-end
- Phase 100 is complete: server-side maintenance tracking (Plan 01) + kiosk badge + clear action (Plan 02)

---
*Phase: 100-staff-visibility-kiosk-badge-fleet-health-manual-clear*
*Completed: 2026-03-21*
