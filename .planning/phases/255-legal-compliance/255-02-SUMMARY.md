---
phase: 255-legal-compliance
plan: 02
subsystem: auth
tags: [rust, axum, sqlx, sqlite, argon2, legal, minor-consent, waiver-gate, otp]

# Dependency graph
requires:
  - phase: 255-01
    provides: waiver_signed + dob columns on drivers, GST accounting layer, billing foundation
  - phase: 254
    provides: RBAC (cashier/manager/superadmin) — guardian endpoints require staff JWT
provides:
  - Waiver gate blocking billing for unsigned drivers (LEGAL-03)
  - Minor detection from DOB with conservative year/month/day comparison (LEGAL-04)
  - Guardian OTP send/verify endpoints using argon2 (LEGAL-04, SEC-08)
  - Guardian physical presence flag stored on billing sessions (LEGAL-05)
  - Indian Contract Act 1872 minor liability disclosure endpoint (LEGAL-06)
  - guardian_otp_code/expires_at/verified/verified_at columns on drivers
  - guardian_present/is_minor_session columns on billing_sessions
affects: [255-03, billing, kiosk-registration-flow, admin-driver-management]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - Guardian OTP reuses existing argon2 hash_otp/verify_otp_hash infrastructure (SEC-08 compliance by reuse)
    - Minor age check uses year/month/day comparison (no fractional year arithmetic)
    - guardian_otp_verified reset to 0 on every new send (prevents stale verifications)

key-files:
  created: []
  modified:
    - crates/racecontrol/src/api/routes.rs
    - crates/racecontrol/src/auth/mod.rs
    - crates/racecontrol/src/db/mod.rs

key-decisions:
  - "Waiver gate is a hard block — billing returns 400 with clear error if waiver_signed=0. Non-negotiable under Indian venue liability law."
  - "DOB-absent drivers treated as adults — avoids false minor flags for legacy records without DOB"
  - "Guardian OTP reuses send_otp_whatsapp() and hash_otp() — no new crypto primitives, SEC-08 compliance by reuse"
  - "guardian_otp_verified reset to 0 on every new send_guardian_otp call — prevents a verified OTP from a previous session being reused later"
  - "Minor disclosure endpoint is public (no auth) — kiosk must display it during registration before the guardian signs"
  - "guardian_present flag must come from the request body — staff must explicitly assert physical presence, cannot be inferred"

patterns-established:
  - "Legal compliance gates are pre-transaction checks — placed before wallet debit, after driver lookup"
  - "Staff-facing consent flows (guardian OTP) live in staff_routes — pod-inaccessible"

requirements-completed: [LEGAL-03, LEGAL-04, LEGAL-05, LEGAL-06]

# Metrics
duration: 35min
completed: 2026-03-29
---

# Phase 255 Plan 02: Legal Compliance — Waiver Gate + Minor Consent Summary

**Waiver gate in start_billing (Indian Contract Act 1872), guardian OTP consent flow for minors, and argon2-hashed guardian OTP send/verify via WhatsApp Evolution API**

## Performance

- **Duration:** ~35 min
- **Started:** 2026-03-28T23:30:01Z
- **Completed:** 2026-03-29T00:05:00Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments

- LEGAL-03: `start_billing` now rejects with a clear error if `waiver_signed=0` on the driver record — no billing without a signed waiver
- LEGAL-04/05: Minor detection from DOB (year/month/day comparison), guardian OTP must be verified and guardian must be physically present (staff-confirmed) before a minor can bill
- LEGAL-06: `GET /legal/minor-waiver-disclosure` returns Indian Contract Act 1872 disclosure text with guardian requirement flags
- Guardian OTP (`POST /guardian/send-otp` + `POST /guardian/verify-otp`) uses argon2 hashing (SEC-08) via existing `hash_otp`/`verify_otp_hash` infrastructure

## Task Commits

1. **Task 1: Waiver gate + minor detection + DB columns** — `12c1b62f` (feat)
2. **Task 2: Guardian OTP send/verify endpoints** — `5ea7d413` (feat)

## Files Created/Modified

- `crates/racecontrol/src/api/routes.rs` — start_billing waiver gate, minor detection block, guardian_present INSERT, disclosure endpoint + handler, guardian OTP HTTP handlers, route registrations
- `crates/racecontrol/src/auth/mod.rs` — `send_guardian_otp()` and `verify_guardian_otp()` functions
- `crates/racecontrol/src/db/mod.rs` — guardian_otp_code/expires_at/verified/verified_at on drivers; guardian_present/is_minor_session on billing_sessions

## Decisions Made

- Waiver gate is a hard block before any other billing logic — placed immediately after driver lookup, before trial check and wallet operations
- Age check uses conservative integer year/month/day comparison rather than `years_since()` or floating-point division — avoids off-by-one at exact birthday boundaries
- DOB-absent or unparseable records treated as adults (`false`) — avoids incorrectly flagging legacy customers who registered before DOB was captured
- Guardian OTP reuses `hash_otp()` and `verify_otp_hash()` from the existing OTP infrastructure — maintains SEC-08 compliance without duplicating crypto code
- `guardian_otp_verified` is reset to `0` whenever `send_guardian_otp()` is called — a new send invalidates any prior verification
- Minor waiver disclosure endpoint placed in `public_routes` — kiosk fetches it during the registration flow, before the guardian can sign, without requiring a staff JWT

## Deviations from Plan

**1. [Rule 1 - Bug] Added `use chrono::Datelike;` inside the if-let block**
- **Found during:** Task 1 (waiver gate implementation)
- **Issue:** `today.year()`, `today.month()`, `today.day()` are private without the `Datelike` trait in scope; cargo check returned E0624 (private method)
- **Fix:** Added `use chrono::Datelike;` scoped inside the `if let Ok(dob_date) = ...` block — avoids a top-level import conflict while fixing the compilation error
- **Files modified:** `crates/racecontrol/src/api/routes.rs`
- **Verification:** `cargo check` passed cleanly
- **Committed in:** `12c1b62f` (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (Rule 1 — compilation bug)
**Impact on plan:** No scope change. The fix was a missing trait import required for the plan's specified date comparison logic.

## Issues Encountered

- Parallel agent already committed Task 2 content (`5ea7d413`) before this agent's explicit commit step — Task 2 was already committed by the time the commit command ran. Both implementations were equivalent; the parallel agent's commit stands as the authoritative Task 2 commit.

## Known Stubs

None — all legal gates are wired to real DB columns. Guardian OTP sends via live WhatsApp Evolution API (falls back gracefully if API not configured).

## Next Phase Readiness

- Phase 255-03 (data retention + consent revocation) is already committed — minor consent and waiver gate foundation is in place
- Kiosk registration flow can now call `GET /legal/minor-waiver-disclosure` during waiver signing for minor customers
- Staff counter can call `POST /guardian/send-otp` and `POST /guardian/verify-otp` before starting a billing session for a minor

---
*Phase: 255-legal-compliance*
*Completed: 2026-03-29*
