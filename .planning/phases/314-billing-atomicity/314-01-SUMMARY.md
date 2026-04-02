---
phase: 314-billing-atomicity
plan: 01
subsystem: billing
tags: [rust, tokio, mutex, concurrency, toctou, billing]

requires:
  - phase: 311-game-launch-reliability
    provides: "Billing infrastructure and routes"
provides:
  - "Per-pod billing start serialization lock (BATOM-01)"
  - "Dual pre-validation: active_timers + waiting_for_game (BATOM-02)"
affects: [billing, start_billing, concurrent-requests]

tech-stack:
  added: []
  patterns: ["Per-pod Arc<tokio::sync::Mutex<()>> via std::sync::Mutex<HashMap> for async-safe serialization"]

key-files:
  created: []
  modified:
    - crates/racecontrol/src/billing.rs
    - crates/racecontrol/src/api/routes.rs

key-decisions:
  - "std::sync::Mutex for outer HashMap (held briefly), tokio::sync::Mutex for per-pod lock (held across .await)"
  - "unwrap_or_else(|e| e.into_inner()) for poisoned mutex recovery instead of unwrap()"
  - "Arc split into separate let binding to avoid temporary value lifetime error"

patterns-established:
  - "Per-entity async serialization: std::sync::Mutex<HashMap<K, Arc<tokio::sync::Mutex<()>>>> pattern"

requirements-completed: [BATOM-01, BATOM-02]

duration: 18min
completed: 2026-04-03
---

# Phase 314 Plan 01: Billing Atomicity Summary

**Per-pod tokio::sync::Mutex serialization on start_billing with dual active_timers + waiting_for_game pre-validation to eliminate TOCTOU race condition**

## Performance

- **Duration:** 18 min
- **Started:** 2026-04-02T22:32:29Z
- **Completed:** 2026-04-02T22:50:00Z
- **Tasks:** 2/2
- **Files modified:** 2

## Accomplishments
- Added billing_start_locks field to BillingManager with get_billing_start_lock() helper
- start_billing now acquires per-pod lock before any validation or DB work
- Pre-validation checks BOTH active_timers AND waiting_for_game maps
- Two concurrent start_billing for same pod are serialized; different pods run in parallel
- DB UNIQUE index idx_billing_sessions_pod_active unchanged (defense-in-depth)

## Task Commits

Each task was committed atomically:

1. **Task 1+2: Per-pod billing start lock + dual pre-validation** - `3de35d50` (feat)

## Files Created/Modified
- `crates/racecontrol/src/billing.rs` - Added billing_start_locks field, initializer, and get_billing_start_lock() helper to BillingManager
- `crates/racecontrol/src/api/routes.rs` - Added per-pod lock acquisition before validation, added waiting_for_game pre-validation check

## Decisions Made
- Used std::sync::Mutex for outer HashMap (held only during HashMap lookup/insert, never across .await) and tokio::sync::Mutex for per-pod lock (designed to be held across .await)
- Used unwrap_or_else(|e| e.into_inner()) for poisoned std::sync::Mutex recovery per standing rule (no .unwrap() in production)
- Split Arc into separate let binding (billing_lock_arc) to satisfy Rust lifetime rules for temporary values

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed temporary value lifetime error in lock acquisition**
- **Found during:** Task 2 (routes.rs modification)
- **Issue:** `state.billing.get_billing_start_lock(&pod_id).lock().await` creates a temporary Arc that is dropped before the MutexGuard, causing E0716
- **Fix:** Split into two let bindings: `let billing_lock_arc = ...; let _billing_lock = billing_lock_arc.lock().await;`
- **Files modified:** crates/racecontrol/src/api/routes.rs
- **Verification:** cargo check passes, cargo test shows 71 pass (8 pre-existing failures unchanged)
- **Committed in:** 3de35d50

---

**Total deviations:** 1 auto-fixed (Rule 3 - blocking)
**Impact on plan:** Minor syntactic fix required by Rust lifetime rules. No scope creep.

## Issues Encountered
- 8 pre-existing test failures (test_lap_suspect_*, test_notification_*) confirmed unrelated to this change by running tests on stashed (unmodified) code

## User Setup Required
None - no external service configuration required.

## Known Stubs
None - all functionality is fully wired.

## Next Phase Readiness
- Billing atomicity is in place for same-pod serialization
- Ready for deployment with next server binary build
- No downstream API changes needed (same endpoints, same behavior for non-concurrent calls)

---
*Phase: 314-billing-atomicity*
*Completed: 2026-04-03*
