---
phase: 05-kiosk-pin-launch
plan: 01
subsystem: api
tags: [rust, axum, sqlite, pin-redemption, rate-limiting, billing, game-launch]

# Dependency graph
requires:
  - phase: 04-remote-booking-pin-generation
    provides: reservations table, PIN generation, create/cancel/modify reservation functions
provides:
  - POST /api/v1/kiosk/redeem-pin endpoint
  - redeem_pin() function in reservation.rs
  - Per-IP lockout tracking for PIN redemption attempts
affects: [05-kiosk-pin-launch, kiosk-pwa, pod-agent]

# Tech tracking
tech-stack:
  added: []
  patterns: [in-handler lockout with LazyLock static HashMap, atomic SQL UPDATE with RETURNING for double-redeem prevention]

key-files:
  created: []
  modified:
    - crates/racecontrol/src/reservation.rs
    - crates/racecontrol/src/api/routes.rs

key-decisions:
  - "Route placed in auth_rate_limited_routes (tower-governor 5/min) since customers use it directly without staff JWT"
  - "Pod availability checked BEFORE consuming PIN to avoid losing reservation on full venue"
  - "Lockout uses std::sync::LazyLock + Mutex static rather than AppState field to avoid modifying shared state struct"
  - "Pricing tier resolved via kiosk_experiences table rather than debit_intents to avoid complex joins"

patterns-established:
  - "PIN redemption lockout: static LazyLock<Mutex<HashMap<IpAddr, PinLockoutState>>> with auto-prune at 1000 entries"
  - "Reservation redemption: check pending_debit first, then pod availability, then atomic UPDATE with RETURNING"

requirements-completed: [KIOSK-02, KIOSK-03, KIOSK-04, KIOSK-05]

# Metrics
duration: 5min
completed: 2026-03-21
---

# Phase 05 Plan 01: Kiosk PIN Redemption Summary

**POST /api/v1/kiosk/redeem-pin endpoint with atomic double-redeem prevention, pod assignment, billing defer, game launch, and per-IP lockout after 10 failures**

## Performance

- **Duration:** 5 min
- **Started:** 2026-03-21T13:27:19Z
- **Completed:** 2026-03-21T13:32:25Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments
- Full PIN redemption flow: validate PIN -> check pod availability -> atomic mark redeemed -> assign pod -> defer billing -> clear lock screen -> launch game -> broadcast dashboard event
- pending_debit PINs return distinct "being processed" message without consuming
- No pods available returns error without consuming PIN
- Per-IP lockout: 10 consecutive failures trigger 5-minute cooldown with remaining attempts counter
- Route protected by tower-governor rate limiting (5 req/min per IP)

## Task Commits

Each task was committed atomically:

1. **Task 1: Add redeem_pin() and kiosk_redeem_pin handler** - `cad596f` (feat)
2. **Task 2: Add per-IP rate limiting and lockout** - `d092cf6` (feat)

## Files Created/Modified
- `crates/racecontrol/src/reservation.rs` - Added redeem_pin() function (~160 lines) with full redemption flow
- `crates/racecontrol/src/api/routes.rs` - Added PinLockoutState, kiosk_redeem_pin handler with lockout tracking, route registration

## Decisions Made
- Route placed in auth_rate_limited_routes (tower-governor 5/min) since customers use it directly without staff JWT
- Pod availability checked BEFORE consuming PIN to avoid losing reservation on full venue
- Lockout uses std::sync::LazyLock + Mutex static rather than AppState field to avoid modifying shared state struct
- Pricing tier resolved via kiosk_experiences table rather than debit_intents to avoid complex joins

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
- routes.rs already contained the kiosk_redeem_pin handler stub and route registration from a prior session; only the lockout logic was new code for routes.rs

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- Redeem-pin endpoint ready for kiosk PWA integration (Plan 05-02)
- Lockout and rate limiting active for production security
- rc-sentry crate has pre-existing compilation error (creation_flags) unrelated to this plan

---
*Phase: 05-kiosk-pin-launch*
*Completed: 2026-03-21*
