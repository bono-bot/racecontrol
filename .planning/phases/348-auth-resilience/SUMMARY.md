# Phase 348: Auth Resilience — SUMMARY

**Status:** CODE-COMPLETE (not deployed)
**Milestone:** v47.0 Admin Dashboard Venue-Ready Hardening
**Completed:** 2026-04-10

## What Was Built

### 348-01: Per-IP + Per-Staff-ID Lockout (`da0fb590`)
- **Per-IP in-memory lockout:** 10 failed attempts → 15-minute lockout (matches kiosk_redeem_pin pattern)
- **Per-staff-id DB lockout:** `staff_login_attempts` table tracks all login attempts with source_ip, staff_id, success, timestamp. 10 failures for a staff_id in 5 minutes locks that account
- **Audit trail:** Every login attempt (success/failure) recorded in DB with indexes on (staff_id, attempted_at) and (source_ip, attempted_at)
- **Files:** `crates/racecontrol/src/api/routes.rs` (+139/-1), `crates/racecontrol/src/db/mod.rs` (+15)

### 348-02: JWT + Multi-Device — SKIPPED
- SC-3 (12h JWT): Already satisfied — JWT lifetime is 24h
- SC-4 (multi-device): Already satisfied — no session revocation on second login

### 348-03: Break-Glass Emergency Access (`a051c5d7`)
- **POST /api/v1/auth/break-glass** — emergency access when normal admin login unavailable
- Validates pre-shared secret from config (`break_glass_secret` field in AuthConfig)
- Issues 1-hour superadmin JWT with staff_id="break-glass"
- WhatsApp alert on BOTH success and failure attempts (via Evolution API)
- Full audit trail via `accounting::log_admin_action`
- Returns 404 if not configured (doesn't reveal endpoint exists)
- Requires `reason` field for audit trail
- **Files:** `crates/racecontrol/src/auth/admin.rs` (+83), `crates/racecontrol/src/api/routes.rs` (+1), `crates/racecontrol/src/config.rs` (+7)

## Success Criteria Verification

| SC | Criteria | Status | Evidence |
|----|----------|--------|----------|
| SC-1 | Per-staff-id lockout across multiple IPs | CODE-COMPLETE | DB-backed `staff_login_attempts` table, 10-in-5min threshold |
| SC-2 | Lockout survives server restart | CODE-COMPLETE | DB-persisted (not in-memory) |
| SC-3 | Staff JWT valid 12h+ | ALREADY SATISFIED | JWT lifetime = 24h (pre-existing) |
| SC-4 | Multi-device login doesn't invalidate first | ALREADY SATISFIED | No session revocation (pre-existing) |
| SC-5 | Break-glass token with WhatsApp alert | CODE-COMPLETE | `/auth/break-glass` endpoint + WhatsApp on success/failure |

## Not Deployed / Not Runtime-Tested

- Runtime lockout behavior (needs server deploy)
- ConnectInfo extraction in production
- WhatsApp delivery latency for break-glass alerts
- Rate limiting on break-glass endpoint (inherits auth rate limit layer)
- `break_glass_secret` config field needs to be set in production `racecontrol.toml`

## Tests

894/894 cargo tests pass on both commits.

## Deploy Requirements

- **Rust binary:** Rebuild + deploy racecontrol to server (.23) and cloud (Bono VPS)
- **Config:** Add `break_glass_secret` to `racecontrol.toml` on server (and cloud if applicable)
- **DB migration:** `staff_login_attempts` table created via `CREATE TABLE IF NOT EXISTS` on startup
- **No frontend changes**
