---
phase: 258-staff-controls-deployment-safety
plan: 01
subsystem: api
tags: [rust, axum, billing, rbac, audit-log, discount, cash-drawer]

# Dependency graph
requires:
  - phase: 254-security-hardening
    provides: Three-tier RBAC (cashier/manager/superadmin), require_role_manager middleware
  - phase: 252-financial-atomicity-core
    provides: wallet credit/debit primitives, audit_log table
  - phase: 257-billing-edge-cases
    provides: billing_sessions table with discount_paise/discount_reason columns

provides:
  - STAFF-01: POST /billing/{id}/discount with manager approval gate for discounts above Rs.50
  - STAFF-02: confirmed complete via existing SEC-05 self-topup block
  - STAFF-03: GET /admin/reports/daily-overrides returning discounts/refunds/tier-changes with actor_id
  - STAFF-04: GET /admin/reports/cash-drawer and POST /admin/reports/cash-drawer/close with discrepancy audit

affects: [phase-259-coupon-discount, phase-260-notifications-resilience-ux]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - manager_approval_code validated against staff_members table by role (manager/superadmin)
    - All financial override actions inserted to audit_log via accounting::log_admin_action
    - IST-aware date defaults using chrono::FixedOffset::east_opt(19800)

key-files:
  created: []
  modified:
    - crates/racecontrol/src/billing.rs
    - crates/racecontrol/src/api/routes.rs

key-decisions:
  - "STAFF-01 manager approval code validated by PIN lookup against staff_members.role IN ('manager','superadmin') — reuses existing PIN auth pattern"
  - "Below-threshold discounts proceed without manager approval but still require reason_code and insert audit_log entry"
  - "Cash drawer total uses topup_cash type OR notes LIKE '%cash%' — catches both structured and ad-hoc cash transactions"
  - "STAFF-02 marked complete via existing SEC-05 self-topup block at routes.rs:7028 — no new implementation needed"
  - "DISCOUNT_APPROVAL_THRESHOLD_PAISE constant placed in billing.rs near other billing config (not AppState) — simple constant, future config migration can move it to DB"

patterns-established:
  - "Financial controls (discount gate, cash drawer) follow same audit_log pattern as other financial operations"
  - "Manager approval code validation reuses staff PIN lookup without issuing a JWT — ephemeral approval, not a session"

requirements-completed: [STAFF-01, STAFF-02, STAFF-03, STAFF-04]

# Metrics
duration: 22min
completed: 2026-03-29
---

# Phase 258 Plan 01: Staff Financial Controls Summary

**Four-endpoint staff financial controls: discount approval gate with manager PIN validation above Rs.50 threshold, daily override audit report (discounts/refunds/tier changes), and cash drawer reconciliation with discrepancy logging**

## Performance

- **Duration:** 22 min
- **Started:** 2026-03-29T08:28:00Z
- **Completed:** 2026-03-29T08:50:42Z
- **Tasks:** 1 (monolithic task covering STAFF-01 through STAFF-04)
- **Files modified:** 2

## Accomplishments

- POST /billing/{id}/discount validates manager approval code for discounts above Rs.50 threshold (STAFF-01)
- STAFF-02 confirmed complete via existing SEC-05 self-topup block (no new code)
- GET /admin/reports/daily-overrides returns all discounts, manual refunds, and tier change audit entries with actor_id for a given day (STAFF-03)
- GET /admin/reports/cash-drawer returns system cash total; POST /admin/reports/cash-drawer/close logs physical vs system discrepancy (STAFF-04)
- All four endpoints wired with correct RBAC: discount endpoint in cashier+ section, reports in manager+ merged sub-router

## Task Commits

Each task was committed atomically:

1. **Task 1: Discount approval gate and daily override report** - `3257b077` (feat)

**Auto-fix commit:** `11926c97` (fix: .unwrap() on always-valid FixedOffset constants)

**Plan metadata:** (this SUMMARY + STATE.md update)

## Files Created/Modified

- `crates/racecontrol/src/billing.rs` - Added `DISCOUNT_APPROVAL_THRESHOLD_PAISE` constant (Rs.50 = 5000 paise)
- `crates/racecontrol/src/api/routes.rs` - Added route wiring + four handler functions (apply_billing_discount, daily_overrides_report, cash_drawer_status, cash_drawer_close)

## Decisions Made

- Manager approval code validated by PIN lookup against `staff_members` WHERE `role IN ('manager', 'superadmin')` — reuses existing PIN auth pattern from `staff_validate_pin`, no new auth mechanism needed
- Below-threshold discounts insert audit_log entry without requiring manager approval
- Cash drawer query uses `type = 'topup_cash' OR notes LIKE '%cash%'` to catch both structured and ad-hoc cash entries
- IST date defaults use `chrono::FixedOffset::east_opt(19800)` (19800 = 5*3600+30*60 seconds = UTC+5:30)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Replaced .unwrap() on FixedOffset with .expect() to comply with CLAUDE.md no-.unwrap() rule**
- **Found during:** Task 1 (post-commit pre-commit hook warning)
- **Issue:** Three occurrences of `.east_opt(0).unwrap()` in IST fallback code triggered pre-commit warning
- **Fix:** Replaced with `.expect("UTC offset 0 is always valid")` — semantically identical (0 is always valid) but avoids the banned pattern
- **Files modified:** crates/racecontrol/src/api/routes.rs
- **Verification:** cargo check passes, pre-commit passes with no warnings
- **Committed in:** 11926c97

---

**Total deviations:** 1 auto-fixed (Rule 1 - code quality)
**Impact on plan:** Minor cleanup, no functional change. All .unwrap() were on a constant that can never return None.

## Issues Encountered

None — plan executed successfully. STAFF-02 was already complete (SEC-05), as documented in plan.

## Known Stubs

None — all four endpoints are fully implemented and wired. No hardcoded empty values flow to callers.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- STAFF-01 through STAFF-04 complete. Phase 258 Plan 02 (deployment safety — DEPLOY-01 through DEPLOY-05) can proceed.
- The discount approval code lookup references `staff_members.role` — ensure the staff table has correct role values (manager/superadmin) for approval codes to work.

---
*Phase: 258-staff-controls-deployment-safety*
*Completed: 2026-03-29*
