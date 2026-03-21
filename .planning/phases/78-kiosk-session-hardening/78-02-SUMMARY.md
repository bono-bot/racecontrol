---
phase: 78-kiosk-session-hardening
plan: 02
subsystem: api
tags: [axum, middleware, ip-classification, network-security, route-protection]

# Dependency graph
requires:
  - phase: 76-auth-hardening
    provides: "require_staff_jwt middleware, staff JWT infrastructure"
  - phase: 77-transport-security
    provides: "HTTPS listener, security headers"
provides:
  - "RequestSource enum (Pod/Staff/Customer/Cloud) IP classification"
  - "classify_source_middleware for global request tagging"
  - "require_non_pod_source guard middleware for staff route protection"
  - "kiosk_routes() separated from staff_routes() for pod-accessible endpoints"
affects: [78-kiosk-session-hardening, rc-agent-auth]

# Tech tracking
tech-stack:
  added: []
  patterns: [ip-based-source-classification, route-tier-separation]

key-files:
  created:
    - crates/racecontrol/src/network_source.rs
  modified:
    - crates/racecontrol/src/lib.rs
    - crates/racecontrol/src/main.rs
    - crates/racecontrol/src/api/routes.rs

key-decisions:
  - "kiosk_routes separated from staff_routes -- pods need JWT-protected kiosk endpoints but must not access admin routes"
  - "Layer order: JWT first (401 for unauth), then pod source check (403 for pods) -- prevents information leakage about which routes exist"
  - "Kiosk admin routes (create/update/delete experiences, update settings, allowlist) remain in staff_routes with pod block"

patterns-established:
  - "IP classification: classify_ip pure function + classify_source_middleware layer pattern"
  - "Route tier separation: kiosk_routes (pod-accessible) vs staff_routes (pod-blocked) with shared JWT requirement"

requirements-completed: [KIOSK-07, KIOSK-05]

# Metrics
duration: 9min
completed: 2026-03-21
---

# Phase 78 Plan 02: Network Source Tagging Summary

**IP-based request source classification with pod-blocked staff routes and pod-accessible kiosk endpoints**

## Performance

- **Duration:** 9 min
- **Started:** 2026-03-21T01:57:34Z
- **Completed:** 2026-03-21T01:06:57Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- RequestSource enum classifies all requests by origin IP (Pod, Staff, Customer, Cloud)
- Staff/admin routes now reject pod-originated requests with 403 Forbidden
- Kiosk-facing endpoints (experiences GET, settings GET, pod-launch, book-multiplayer) remain accessible from pods with valid JWT
- 12 unit+integration tests covering all IP classifications and guard behavior

## Task Commits

Each task was committed atomically:

1. **Task 1: Create network source tagging module with classify + guard middleware** - `e6d2ff4` (feat)
2. **Task 2: Wire source middleware and protect staff routes from pod access** - `62b603a` (feat)

## Files Created/Modified
- `crates/racecontrol/src/network_source.rs` - RequestSource enum, classify_ip, classify_source_middleware, require_non_pod_source
- `crates/racecontrol/src/lib.rs` - Added pub mod network_source declaration
- `crates/racecontrol/src/main.rs` - Global classify_source_middleware layer on app router
- `crates/racecontrol/src/api/routes.rs` - kiosk_routes() extracted, staff_routes() pod-blocked, api_routes() merges kiosk_routes

## Decisions Made
- Kiosk-facing routes separated into kiosk_routes() with JWT but no pod block -- pods authenticate via validate-pin and need these endpoints
- Admin kiosk routes (create/update/delete experiences, update settings, allowlist management) kept in staff_routes with pod block
- Layer order ensures JWT check runs first (401 for unauth) before pod source check (403) -- no information leakage
- Guard allows requests with missing RequestSource extension (graceful degradation if classify middleware not present)

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Network source tagging is live -- all requests tagged with RequestSource
- Staff routes are pod-blocked, kiosk routes are pod-accessible
- Ready for Plan 03 (session hardening) which can build on this source classification

---
*Phase: 78-kiosk-session-hardening*
*Completed: 2026-03-21*
