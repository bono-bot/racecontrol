---
phase: 13-leaderboard-core
plan: 04
subsystem: api
tags: [axum, driver-search, driver-profile, public-api, pii-exclusion, sqlite]

# Dependency graph
requires:
  - phase: 13-leaderboard-core
    provides: personal_bests table, laps table with suspect column, nickname display logic
provides:
  - /public/drivers?name=X search endpoint (case-insensitive, max 20, nickname-aware)
  - /public/drivers/{id} profile endpoint (stats, personal_bests, lap_history, no PII)
  - DriverSearchQuery struct with required name field
  - class_badge null placeholder in profile response
affects: [13-leaderboard-core, pwa-driver-pages, social-sharing]

# Tech tracking
tech-stack:
  added: []
  patterns: [pii-exclusion-by-construction, sector-zero-to-null, class-badge-placeholder]

key-files:
  created: []
  modified:
    - crates/racecontrol/src/api/routes.rs
    - crates/racecontrol/tests/integration.rs

key-decisions:
  - "PII exclusion by construction: SELECT only safe fields (never SELECT * then filter)"
  - "Sector times <= 0 mapped to SQL NULL via CASE expression, not application-level filtering"
  - "class_badge: null hardcoded in response — Phase 15 RAT-01 will populate with driver rating class"
  - "Search queries both name AND nickname columns for completeness"
  - "COLLATE NOCASE used for case-insensitive LIKE matching in SQLite"

patterns-established:
  - "Public profile: explicitly SELECT safe fields, never derive from staff endpoints"
  - "Sector null mapping: CASE WHEN sector_ms > 0 THEN sector_ms ELSE NULL END"
  - "Placeholder fields: include null fields for future features with clear Phase reference"

requirements-completed: [DRV-01, DRV-02, DRV-03, DRV-04]

# Metrics
duration: 4min
completed: 2026-03-15
---

# Phase 13 Plan 04: Public Driver Search & Profile Summary

**Public driver search (case-insensitive name/nickname, max 20) and profile endpoints (stats, personal bests, lap history) with zero PII exposure and sector-zero-to-null mapping**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-14T21:49:38Z
- **Completed:** 2026-03-14T21:53:22Z
- **Tasks:** 2 (TDD RED + GREEN)
- **Files modified:** 2

## Accomplishments
- Created /public/drivers?name=X endpoint with case-insensitive search across name and nickname columns, capped at 20 results
- Created /public/drivers/{id} endpoint returning driver stats, personal bests, and paginated lap history with zero PII fields
- Sector times <= 0 automatically mapped to JSON null via SQL CASE expressions
- class_badge: null placeholder included for future Phase 15 RAT-01 integration
- 7 integration tests covering search filtering, limit, PII exclusion, personal bests, sector nulling, nickname display, and class_badge placeholder
- All 209 library tests + 41 integration tests + 93 rc-common tests pass

## Task Commits

Each task was committed atomically:

1. **TDD RED: Failing tests for driver search and profile** - `f391847` (test)
2. **TDD GREEN: Implement handlers + route registration** - `cd0ed91` (feat)

_TDD REFACTOR phase not needed -- handlers are clean and follow existing patterns._

## Files Created/Modified
- `crates/racecontrol/src/api/routes.rs` - Added DriverSearchQuery struct, public_drivers_search and public_driver_profile handlers, registered 2 new routes
- `crates/racecontrol/tests/integration.rs` - Added 7 new tests (driver_search, search_limit, no_pii, class_badge_null, personal_bests, null_sectors, nickname)

## Decisions Made
- PII exclusion implemented by construction: the SQL SELECT statement only names safe columns (display_name, total_laps, total_time_ms, avatar_url, created_at). Email, phone, wallet, and billing data are never selected, making PII leakage structurally impossible.
- Sector zero-to-null mapping done at the SQL layer using CASE WHEN expressions rather than post-query filtering, keeping the handler code minimal.
- class_badge is hardcoded as null in the JSON response. This is a documented placeholder for Phase 15 RAT-01 which will implement driver rating classes (A/B/C/D based on percentile).
- Search covers both name and nickname columns with COLLATE NOCASE for case-insensitive matching across both fields.
- Driver profile returns 404 JSON error for non-existent driver IDs (not 500).

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Public driver search and profile endpoints ready for PWA consumption in Plan 05
- Social sharing loop enabled: shareable profile URLs return public stats
- All existing leaderboard endpoints unaffected
- class_badge placeholder ready for Phase 15 RAT-01 to populate

## Self-Check: PASSED

- Both modified files exist on disk
- Both commit hashes (f391847, cd0ed91) verified in git log
- All 209 library tests + 41 integration tests + 93 rc-common tests pass

---
*Phase: 13-leaderboard-core*
*Completed: 2026-03-15*
