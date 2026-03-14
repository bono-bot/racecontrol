---
phase: 04-pwa-session-results
plan: 01
subsystem: api
tags: [axum, sqlx, whatsapp, evolution-api, billing-events, public-api]

# Dependency graph
requires:
  - phase: 01-billing-cloud-sync
    provides: billing_events table synced to cloud
provides:
  - customer_session_detail includes events timeline
  - public_session_summary endpoint (no auth)
  - WhatsApp receipt via Evolution API on session end
affects: [04-pwa-session-results plan 02, PWA session detail page, public share page]

# Tech tracking
tech-stack:
  added: []
  patterns: [format_wa_phone helper for Evolution API phone formatting, format_receipt_message for testable message construction]

key-files:
  created: []
  modified:
    - crates/rc-core/src/api/routes.rs
    - crates/rc-core/src/billing.rs

key-decisions:
  - "WhatsApp receipt sent directly via Evolution API (not via Bono webhook) per user decision"
  - "Public session summary shows first name only (split_whitespace().next()) per privacy decision"
  - "Receipt formatting extracted to testable helper functions (format_wa_phone, format_receipt_message)"
  - "5-second timeout on receipt HTTP call to prevent blocking session end"
  - "Events query uses unwrap_or_default() for graceful empty array on error"

patterns-established:
  - "Public endpoints: no auth, privacy-safe (first name only, no billing amounts)"
  - "WhatsApp messaging: format_wa_phone() for phone normalization, Evolution API pattern"
  - "Testable helpers: extract query/format logic into pure functions for unit testing"

requirements-completed: [PWA-03, PWA-04, PWA-05]

# Metrics
duration: 6min
completed: 2026-03-14
---

# Phase 4 Plan 01: PWA Session Results Backend Summary

**Events timeline in session detail, public shareable endpoint, and WhatsApp receipt via Evolution API on session end**

## Performance

- **Duration:** 6 min
- **Started:** 2026-03-14T05:36:17Z
- **Completed:** 2026-03-14T05:42:38Z
- **Tasks:** 3
- **Files modified:** 2

## Accomplishments
- customer_session_detail now returns { session, laps, events } with billing events timeline ordered by created_at ASC
- GET /public/sessions/{id} returns privacy-safe session summary (first name only, no billing amounts, no auth required)
- post_session_hooks sends WhatsApp receipt via Evolution API with duration, cost, best lap, and wallet balance
- 10 new tests added (2 events, 2 public session, 6 WhatsApp receipt) -- all 207 rc-core tests pass

## Task Commits

Each task was committed atomically:

1. **Task 1: Add events timeline to customer_session_detail** - `16aa329` (feat)
2. **Task 2: Add public session summary endpoint** - `d45a4a5` (feat)
3. **Task 3: Add WhatsApp receipt to post_session_hooks** - `b3ed73d` (feat)

_TDD: tests written alongside implementation for each task (query logic + helpers tested in isolation)_

## Files Created/Modified
- `crates/rc-core/src/api/routes.rs` - Added events query in customer_session_detail, public_session_summary handler, 4 tests
- `crates/rc-core/src/billing.rs` - Added format_wa_phone, format_receipt_message, send_whatsapp_receipt, hook call in post_session_hooks, 6 tests

## Decisions Made
- WhatsApp receipt delivered directly via Evolution API (same pattern as OTP in auth/mod.rs) -- not via Bono webhook. Per user decision from planning phase.
- Public session summary exposes first name only (privacy). No billing amounts, no phone, no email.
- Receipt formatting extracted to pure testable functions rather than testing HTTP calls.
- 5-second HTTP timeout on receipt delivery -- best-effort, never blocks session end.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required. Evolution API config is already present in racecontrol.toml (evolution_url, evolution_api_key, evolution_instance).

## Next Phase Readiness
- Backend APIs ready for PWA Plan 04-02 (session detail page with timeline, public share page)
- customer_session_detail returns events array for timeline rendering
- public_session_summary provides data for shareable link page
- WhatsApp receipt will fire automatically on next session end (no deployment needed on venue yet -- rc-core rebuild required)

---
*Phase: 04-pwa-session-results*
*Completed: 2026-03-14*
