---
phase: 308
plan: "01"
subsystem: rbac
tags: [security, rbac, roles, jwt, admin, middleware]
dependency_graph:
  requires: [WSAUTH-01, WSAUTH-02]
  provides: [RBAC-01, RBAC-02, RBAC-03, RBAC-04]
  affects: [racecontrol]
tech_stack:
  added: []
  patterns: [role column on staff_members, JWT role claims, layered middleware (require_staff_jwt → require_role_*)]
key_files:
  created: []
  modified: []
decisions:
  - "No new code — all 4 RBAC requirements pre-built across prior milestones"
  - "staff_members.role column preferred over separate staff_roles table — simpler, functionally equivalent"
  - "Legacy 'staff' role mapped to 'cashier' — backward compatible"
metrics:
  duration: "0 minutes (verification only)"
  completed: "2026-04-02"
  tasks_completed: 0
  tasks_total: 0
  files_modified: 0
  files_created: 0
---

# Phase 308 Plan 01: RBAC for Admin Summary

All 4 RBAC requirements were already implemented across prior milestones. This phase is a verification pass.

## Pre-Built Implementation

### RBAC-01: Three Roles Defined
- `staff_members` table with `role TEXT DEFAULT 'staff'` column
- Constants: `ROLE_CASHIER = "cashier"`, `ROLE_MANAGER = "manager"`, `ROLE_SUPERADMIN = "superadmin"`
- Legacy "staff" → "cashier" mapping in `StaffClaims::normalized_role()`

### RBAC-02: JWT Role Claims
- `StaffClaims { sub, role, exp, iat }` — every staff JWT carries a role
- `create_staff_jwt_with_role(secret, sub, role, hours)` — role from DB
- `require_staff_jwt` middleware extracts claims and stores in request extensions
- Admin login → superadmin JWT; Kiosk staff login → role from `staff_members.role` column

### RBAC-03: Endpoint Enforcement
**Manager+ (require_role_manager):**
- Billing reports, accounting (trial-balance, P&L, balance-sheet), audit log
- Billing rates CRUD, reconciliation, dispute resolution, cash drawer, daily overrides

**Superadmin (require_role_superadmin):**
- Feature flags CRUD, config push, deploy (single/rolling), OTA
- Pipeline config

**Cashier (all staff routes without role gating):**
- Billing start/stop/pause, customer management, sessions, games, cafe, wallet operations
- Pod management, waivers, coupons, tournaments, gamification

### RBAC-04: Server-Side Enforcement
- 403 Forbidden returned for wrong role — not just UI-hidden
- `has_role()` method on StaffClaims for flexible role checking
- Tests: `manager_has_role_manager`, `superadmin_has_all_roles`, `cashier_cannot_access_manager`

## Commits

None — no new code required.
