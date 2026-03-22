---
phase: 156-promotions-engine
plan: 01
subsystem: database, api
tags: [sqlite, sqlx, axum, rust, crud, promotions, cafe]

# Dependency graph
requires: []
provides:
  - cafe_promos SQLite table with CHECK constraint on promo_type
  - idx_cafe_promos_active index for fast active-promo queries
  - cafe_promos.rs Rust module with CafePromo, CreateCafePromoRequest, UpdateCafePromoRequest types
  - Five admin CRUD endpoints: GET/POST /cafe/promos, PUT/DELETE /cafe/promos/{id}, POST /cafe/promos/{id}/toggle
affects: [157-checkout-integration, promotions, cafe]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Dynamic UPDATE SQL via Vec<String> set_clauses with sequential .bind() calls"
    - "sqlx::query_as re-fetch after INSERT/UPDATE to return fresh DB state"
    - "promo_type application-level validation mirroring SQLite CHECK constraint"

key-files:
  created:
    - crates/racecontrol/src/cafe_promos.rs
  modified:
    - crates/racecontrol/src/db/mod.rs
    - crates/racecontrol/src/lib.rs
    - crates/racecontrol/src/api/routes.rs

key-decisions:
  - "stacking_group column on cafe_promos table (not a separate cafe_promo_stacking table) — sufficient for mutual-exclusivity logic Phase 157 needs"
  - "promo_type validated at application layer AND SQLite CHECK constraint for defense-in-depth"
  - "config stored as JSON string (TEXT column) allowing flexible schema per promo_type without separate tables"

patterns-established:
  - "Dynamic partial-update pattern: build set_clauses Vec, then sequential .bind() matching the same field order"
  - "Re-fetch after write pattern: INSERT/UPDATE returns fresh row via SELECT, not from request data"

requirements-completed: [PROMO-01, PROMO-02, PROMO-03, PROMO-04]

# Metrics
duration: 15min
completed: 2026-03-22
---

# Phase 156 Plan 01: Promotions Engine Summary

**SQLite cafe_promos table + five Axum admin CRUD endpoints for combo/happy_hour/gaming_bundle promos with stacking group support**

## Performance

- **Duration:** ~15 min
- **Started:** 2026-03-22T13:30:00Z
- **Completed:** 2026-03-22T13:45:00Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments

- cafe_promos table with CHECK constraint on promo_type (combo/happy_hour/gaming_bundle), config JSON column, and stacking_group for mutual-exclusivity
- idx_cafe_promos_active index enabling fast active-promo lookups at checkout time
- Full CRUD module (cafe_promos.rs) with all five handlers, zero .unwrap() calls
- Routes wired into admin_router() behind require_non_pod_source + require_staff_jwt middleware

## Task Commits

1. **Task 1: DB migration — cafe_promos table and active index** - `a4aff594` (feat)
2. **Task 2: cafe_promos.rs module + routes registration** - `8ce96368` (feat)

**Plan metadata:** (docs commit below)

## Files Created/Modified

- `crates/racecontrol/src/cafe_promos.rs` - New module: CafePromo type, 3 request types, validation helper, 5 CRUD handlers
- `crates/racecontrol/src/db/mod.rs` - Added cafe_promos CREATE TABLE IF NOT EXISTS + idx_cafe_promos_active after cafe_orders indexes block
- `crates/racecontrol/src/lib.rs` - Added `pub mod cafe_promos;` after `pub mod cafe_alerts;`
- `crates/racecontrol/src/api/routes.rs` - Added `use crate::cafe_promos;`, `delete` routing import, and 3 route registrations in admin_router()

## Decisions Made

- Used stacking_group column on cafe_promos (not a separate cafe_promo_stacking table) per plan note — single column is sufficient for Phase 157's mutual-exclusivity check
- promo_type validated at Rust application layer (validate_promo_type fn) in addition to SQLite CHECK constraint — defense-in-depth; returns clean 400 JSON error rather than raw SQLite constraint error
- config stored as TEXT (JSON string) — allows different schema shapes per promo_type (combo/happy_hour/gaming_bundle) without separate tables or JSONB

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added `delete` to axum routing imports**
- **Found during:** Task 2 (routes.rs registration)
- **Issue:** `delete` routing method was not imported in routes.rs, only `get`, `post`, `put` — needed for DELETE /cafe/promos/{id}
- **Fix:** Added `delete` to `routing::{delete, get, post, put}` import line
- **Files modified:** crates/racecontrol/src/api/routes.rs
- **Verification:** `cargo build --release --bin racecontrol` succeeds with zero errors
- **Committed in:** `8ce96368` (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Necessary import addition — no scope creep.

## Issues Encountered

- Pre-existing test failures (6 billing_rates + notification tests, 1 config fallback test) were present before this plan and are unrelated to promotions. 449 tests pass.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Phase 157 (checkout integration) can now import CafePromo and query cafe_promos table
- All five endpoints are accessible via admin auth at /api/v1/cafe/promos
- stacking_group column is ready for Phase 157's mutual-exclusivity logic
- No blockers

---
*Phase: 156-promotions-engine*
*Completed: 2026-03-22*
