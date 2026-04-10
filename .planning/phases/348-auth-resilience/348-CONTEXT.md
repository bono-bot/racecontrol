# Phase 348: Auth Resilience — Context

**Gathered:** 2026-04-10
**Status:** Code-complete
**Milestone:** v47.0 Admin Dashboard Venue-Ready Hardening

<domain>
## Phase Boundary

Harden auth against brute-force and provide emergency access when normal login is unavailable.

Five success criteria:
1. Per-staff-id lockout across multiple IPs (not just per-IP)
2. Lockout survives server restart (DB-backed)
3. Staff JWT valid 12h+ (already 24h)
4. Multi-device login doesn't invalidate first (already no revocation)
5. Break-glass token with WhatsApp alert
</domain>

<decisions>
- SC-1+2: staff_login_attempts DB table + per-IP in-memory lockout (348-01)
- SC-3+4: Already satisfied (24h JWT, no session revocation)
- SC-5: break_glass_secret config field + POST /auth/break-glass endpoint (348-03)
- JWT sliding refresh: NOT implemented — 24h is sufficient for venue ops
</decisions>

<canonical_refs>
- crates/racecontrol/src/auth/admin.rs — admin_login + break_glass handlers
- crates/racecontrol/src/api/routes.rs:12765+ — staff_validate_pin with lockout
- crates/racecontrol/src/db/mod.rs:3606+ — admin_lockout + staff_login_attempts tables
- crates/racecontrol/src/config.rs:461-486 — AuthConfig with break_glass_secret
</canonical_refs>
