# Phase 348: Auth Resilience — Context

**Gathered:** 2026-04-10 (updated 2026-04-11 via --auto)
**Status:** Code-complete (not deployed)
**Milestone:** v47.0 Admin Dashboard Venue-Ready Hardening

<domain>
## Phase Boundary

Harden admin/staff authentication against brute-force attacks and provide emergency access when normal login is unavailable. Five success criteria:

1. Per-staff-id lockout across multiple IPs (not just per-IP)
2. Lockout survives server restart (DB-backed)
3. Staff JWT valid 12h+ (already 24h — pre-existing)
4. Multi-device login doesn't invalidate first (already no revocation — pre-existing)
5. Break-glass token with WhatsApp alert

</domain>

<decisions>
## Implementation Decisions

### Lockout Strategy
- **D-01:** Dual lockout — per-IP in-memory (10 failures → 15min lockout, matches kiosk_redeem_pin pattern) + per-staff-id DB-backed (10 failures in 5min → account lock)
- **D-02:** `staff_login_attempts` DB table with indexes on `(staff_id, attempted_at)` and `(source_ip, attempted_at)` for efficient window queries
- **D-03:** Lockout window is time-based (5-minute sliding window) — no manual admin unlock needed, account auto-recovers after window passes

### Break-Glass Emergency Access
- **D-04:** `POST /api/v1/auth/break-glass` endpoint with pre-shared secret from config (`break_glass_secret` field in AuthConfig)
- **D-05:** Issues 1-hour superadmin JWT with `staff_id="break-glass"` — shorter than regular 24h JWT for security
- **D-06:** WhatsApp alert on BOTH success and failure attempts (via Evolution API) — failure alerts catch unauthorized attempts
- **D-07:** Returns 404 if not configured (doesn't reveal endpoint exists to attackers)
- **D-08:** Requires `reason` field for audit trail — every emergency access must be justified

### JWT & Multi-Device
- **D-09:** SC-3 (12h JWT) — ALREADY SATISFIED, JWT lifetime is 24h. No sliding refresh implemented; 24h is sufficient for venue ops.
- **D-10:** SC-4 (multi-device) — ALREADY SATISFIED, no session revocation exists. Staff can login on multiple devices simultaneously.

### Audit Trail
- **D-11:** Every login attempt (success/failure) recorded in `staff_login_attempts` with source_ip, staff_id, success flag, timestamp
- **D-12:** Break-glass usage logged via `accounting::log_admin_action` for compliance audit

### Claude's Discretion
- Per-IP lockout threshold (10 attempts) aligned with existing `kiosk_redeem_pin` pattern for consistency
- DB table indexes chosen for time-window query efficiency

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Auth Implementation
- `crates/racecontrol/src/auth/admin.rs` — admin_login + break_glass handlers (+83 lines in 348-03)
- `crates/racecontrol/src/auth/mod.rs` — auth module exports
- `crates/racecontrol/src/auth/middleware.rs` — JWT middleware, role extraction

### API Routes
- `crates/racecontrol/src/api/routes.rs:12765+` — staff_validate_pin with lockout logic (+139/-1 in 348-01)

### Database
- `crates/racecontrol/src/db/mod.rs:3606+` — admin_lockout + staff_login_attempts table creation (+15 in 348-01)

### Config
- `crates/racecontrol/src/config.rs:461-486` — AuthConfig with break_glass_secret field (+7 in 348-03)

### Prior Phase Dependencies
- `.planning/phases/343-staff-pin-hardening/343-CONTEXT.md` — Cloud-authority 409 guard (precursor to auth hardening)
- `.planning/phases/345-backend-resilience/345-CONTEXT.md` — JWT literal removal + webhook secret rejection

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `kiosk_redeem_pin` lockout pattern — 10-attempt threshold, in-memory, used as template for per-IP lockout
- `accounting::log_admin_action` — existing audit logging, reused for break-glass events
- Evolution API WhatsApp integration — already wired for alerting, reused for break-glass alerts

### Established Patterns
- DB table creation in `db/mod.rs` with `CREATE TABLE IF NOT EXISTS` + `CREATE INDEX IF NOT EXISTS`
- AuthConfig struct in `config.rs` with serde deserialization from TOML
- Route registration in `api/routes.rs` with Axum extractors

### Integration Points
- Break-glass endpoint registered in the admin auth router (same auth layer as regular login)
- WhatsApp alerting via existing Evolution API helpers
- Staff login attempts table joins with existing staff tables via staff_id

</code_context>

<specifics>
## Specific Ideas

No specific requirements beyond success criteria — all decisions driven by security best practices and existing codebase patterns.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 348-auth-resilience*
*Context gathered: 2026-04-10, updated 2026-04-11*
