# Phase 308: RBAC for Admin - Context

**Gathered:** 2026-04-02
**Status:** Pre-built — all 4 requirements already implemented across prior milestones

<domain>
## Phase Boundary

Staff access limited by role — cashier/manager/superadmin. JWT role claims enforced on every endpoint. Admin dashboard gated by role (server enforces, UI hides).

</domain>

<decisions>
## Pre-Built Assessment

All 4 RBAC requirements are already implemented:

### RBAC-01: Three roles in system
- `staff_members` table has `role TEXT DEFAULT 'staff'` column (ALTER migration in db/mod.rs:1903)
- Constants: `ROLE_CASHIER`, `ROLE_MANAGER`, `ROLE_SUPERADMIN` in auth/middleware.rs
- Legacy "staff" role mapped to "cashier" via `normalized_role()`

### RBAC-02: JWT role claim enforcement
- `StaffClaims { sub, role, exp, iat }` in auth/middleware.rs
- `create_staff_jwt_with_role()` encodes role from DB into JWT
- `require_staff_jwt` middleware extracts and validates on every request
- Kiosk staff login reads role from DB: `role_opt.as_deref().unwrap_or("cashier")`

### RBAC-03: Cashier restrictions
- Manager+ routes (routes.rs:548-573): billing reports, accounting, audit log, billing rates, disputes, cash drawer — `require_role_manager`
- Superadmin routes (routes.rs:575-591): flags, config push, deploy, OTA, pipeline — `require_role_superadmin`
- All other staff routes: billing, customers, sessions, games, cafe — accessible to all roles

### RBAC-04: Server-enforced UI gating
- Server returns 403 for wrong role (not just UI-hidden)
- Admin dashboard (racingpoint-admin) is separate repo — UI gating there is a frontend task
- Kiosk already reads staff role from JWT and displays role in response

</decisions>

<code_context>
## Key Files
- `crates/racecontrol/src/auth/middleware.rs` — ROLE_*, StaffClaims, require_role_*, has_role()
- `crates/racecontrol/src/api/routes.rs:548-596` — Manager+ and Superadmin route groups
- `crates/racecontrol/src/db/mod.rs:1882-1903` — staff_members table + role column migration
- `crates/racecontrol/src/api/routes.rs:11085-11115` — Kiosk staff login with DB role → JWT

</code_context>

<specifics>
No new code needed. Phase 308 requirements already met by prior implementations.
</specifics>

<deferred>
- Admin dashboard UI role gating (frontend, racingpoint-admin repo) — separate from this Rust backend milestone
- Dedicated `staff_roles` lookup table — unnecessary; `staff_members.role` column with constant validation is simpler and equivalent
</deferred>
