---
phase: 255-legal-compliance
plan: 01
subsystem: payments
tags: [gst, accounting, invoicing, journal-entries, sqlite, rust, axum, legal-compliance]

# Dependency graph
requires:
  - phase: 252-financial-atomicity-core
    provides: post_journal_entry(), JournalLine, acc_wallet/acc_racing_rev accounts seeded
  - phase: 254-security-hardening
    provides: RBAC (staff_routes, manager+ access) required for invoice endpoint

provides:
  - 3-line GST-separated journal entries for all billing sessions (LEGAL-01)
  - invoices table with per-session GST records (invoice_number, GSTIN, SAC, CGST/SGST)
  - invoice_sequence table for monotonic invoice numbering
  - generate_invoice() function in accounting.rs
  - GET /billing/sessions/{id}/invoice (staff endpoint)
  - GET /customer/sessions/{id}/invoice (customer endpoint)
  - pricing_display_handler returns refund_policy, pricing_policy, gst_note (LEGAL-07)

affects: [256-game-specific-hardening, 257-billing-edge-cases, 258-staff-controls, 259-coupon-discount, 260-notifications]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "18% inclusive GST split: net_paise = amount * 100 / 118, gst_paise = amount - net_paise (integer arithmetic, no float)"
    - "CGST/SGST split for intra-state: cgst = gst / 2, sgst = gst - cgst"
    - "Invoice sequence via UPDATE...RETURNING for atomic number allocation in SQLite"
    - "Non-critical post-commit: invoice generation failure logs warn but does not fail billing session"

key-files:
  created: []
  modified:
    - crates/racecontrol/src/accounting.rs
    - crates/racecontrol/src/db/mod.rs
    - crates/racecontrol/src/api/routes.rs

key-decisions:
  - "18% inclusive GST (not exclusive): net = amount * 100 / 118 — customers see one price, tax is embedded"
  - "Invoice generation is non-critical: failure logs WARN, billing session continues — invoice is a record not a gate"
  - "VENUE_GSTIN hardcoded as placeholder constant with TODO — avoids new config struct dependency in this plan"
  - "post_session_debit remains as backward-compat wrapper calling post_session_debit_gst internally"
  - "Invoice generation wired into start_billing post-commit flow (not end_billing) — invoice is created at session start"

patterns-established:
  - "Pattern: post_session_debit_gst returns (entry_id, net_paise, gst_paise) for downstream use by invoice generator"
  - "Pattern: RETURNING clause for atomic sequence increment in SQLite (UPDATE...RETURNING next_number - 1)"

requirements-completed: [LEGAL-01, LEGAL-02, LEGAL-07]

# Metrics
duration: 25min
completed: 2026-03-28
---

# Phase 255 Plan 01: Legal Compliance — GST Accounting and Invoicing Summary

**18% inclusive GST split in 3-line journal entries, per-session GST invoices with GSTIN/SAC/CGST/SGST, and Consumer Protection Act pricing disclosure in the kiosk display endpoint**

## Performance

- **Duration:** ~25 min
- **Started:** 2026-03-28T23:20:00Z
- **Completed:** 2026-03-28T23:45:00Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- LEGAL-01: post_session_debit_gst() splits any session amount into 3 balanced journal lines (acc_wallet debit, acc_racing_rev credit net-of-GST, acc_gst_payable credit for 18% GST) using pure integer arithmetic
- LEGAL-02: invoices table + invoice_sequence + generate_invoice() + GET endpoints for staff and customer; invoice generation wired into start_billing post-commit flow
- LEGAL-07: pricing_display_handler returns refund_policy, pricing_policy, and gst_note alongside tier data — Consumer Protection Act 2019 compliance

## Task Commits

Each task was committed atomically:

1. **Task 1: GST separation in journal entries + invoices table** - `6791a153` (feat)
2. **Task 2: Invoice endpoint + pricing policy display** - `6e395bca` (feat)

## Files Created/Modified
- `crates/racecontrol/src/accounting.rs` - Added post_session_debit_gst(), generate_invoice(), VENUE_GSTIN/SAC_CODE constants
- `crates/racecontrol/src/db/mod.rs` - Added invoices table, invoice_sequence table, indexes
- `crates/racecontrol/src/api/routes.rs` - Added get_session_invoice, customer_session_invoice handlers, routes, pricing policy fields

## Decisions Made
- 18% inclusive GST split uses integer arithmetic: `net_paise = amount_paise * 100 / 118` avoids floating-point precision issues in financial calculations
- Invoice generation placed in post-commit of start_billing (not end_billing) — the invoice is the record of the contractual obligation at session start; refunds adjust the wallet but the original invoice stands
- Non-critical pattern: invoice failure does not block billing session (the session was committed atomically before this point; invoice is supplementary record)
- VENUE_GSTIN left as placeholder constant `29AABCU9603R1ZX` with clear TODO comment — reading from config.toml would require a config struct change (separate task, out of scope for this plan)
- post_session_debit kept as backward-compatible wrapper — existing callers (if any future plans add them) won't break

## Deviations from Plan

None — plan executed exactly as written. The TypedHeader approach for customer auth was replaced with the project's existing `extract_driver_id()` pattern (axum::http::HeaderMap), which was an implementation detail clarification, not a plan deviation.

## Issues Encountered
- TypedHeader/axum::headers not available in this axum version — used existing `extract_driver_id(headers)` pattern from other customer handlers. Fixed in Task 2 before final cargo check.
- Pre-existing integration test failures (test_wallet_credit_debit_balance etc.) confirmed not caused by these changes — they fail identically on the unmodified main branch (idempotency_key column migration issue in test DB).

## Known Stubs
- `VENUE_GSTIN = "29AABCU9603R1ZX"` — placeholder GSTIN constant in accounting.rs. The actual venue GSTIN must be configured before going live with GST filing. Wired everywhere as a constant, easy to update or migrate to config field.

## Next Phase Readiness
- GST accounting infrastructure complete — 255-02 and 255-03 can build on invoices table
- invoice_sequence row seeded at startup — invoice numbering works immediately without manual setup
- VENUE_GSTIN needs real value from Uday before production use

---
*Phase: 255-legal-compliance*
*Completed: 2026-03-28*
