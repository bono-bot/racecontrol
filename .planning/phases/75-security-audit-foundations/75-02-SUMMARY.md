---
phase: 75-security-audit-foundations
plan: 02
subsystem: auth
tags: [jwt, secrets, env-vars, rand, config, security]

# Dependency graph
requires:
  - phase: 75-security-audit-foundations
    provides: "75-01 audit documents identified 6 secrets in config.rs needing migration"
provides:
  - "Env var overrides for all 6 secrets (JWT, terminal, relay, evolution, gmail x2)"
  - "resolve_jwt_secret() with env > config > auto-generate priority chain"
  - "Dangerous JWT default rejection and 256-bit random key generation"
affects: [auth-middleware, deployment, start-racecontrol-bat]

# Tech tracking
tech-stack:
  added: []
  patterns: ["env-var-first secret management via apply_env_overrides()", "resolve_jwt_secret 3-tier priority: env > config > random"]

key-files:
  created: []
  modified: ["crates/racecontrol/src/config.rs"]

key-decisions:
  - "Used rand 0.8 thread_rng().r#gen() (gen is reserved keyword in Rust 2024 edition)"
  - "Format hex manually with format!(\"{:02x}\") loop instead of adding hex crate dependency"
  - "Wrapped set_var/remove_var in unsafe blocks for Rust 2024 edition compliance"
  - "default_jwt_secret() retained for serde backward compatibility; resolve_jwt_secret catches it at runtime"

patterns-established:
  - "RACECONTROL_* env var naming convention for all secrets"
  - "Env var override pattern: check var -> if non-empty -> override config field"
  - "JWT secret resolution: env var > valid config > auto-generate with WARN log"

requirements-completed: [AUDIT-03, AUDIT-04]

# Metrics
duration: 5min
completed: 2026-03-20
---

# Phase 75 Plan 02: Secrets Env Var Migration Summary

**Env var overrides for 6 secrets (JWT, terminal, relay, evolution, gmail credentials) with cryptographic JWT key auto-generation rejecting the dangerous hardcoded default**

## Performance

- **Duration:** 5 min
- **Started:** 2026-03-20T12:11:43Z
- **Completed:** 2026-03-20T12:17:06Z
- **Tasks:** 1
- **Files modified:** 1

## Accomplishments
- All 6 secrets now loadable from RACECONTROL_* environment variables with config file fallback
- Dangerous default "racingpoint-jwt-change-me-in-production" automatically rejected and replaced with random 256-bit hex key
- WARN log emitted when auto-generating JWT key, naming RACECONTROL_JWT_SECRET env var for persistence
- 11 new unit tests covering env var override, config fallback, dangerous default rejection, and randomness

## Task Commits

Each task was committed atomically:

1. **Task 1: Add env var overrides for all 6 secrets + JWT auto-generation** - `fb6102e` (feat)

**Plan metadata:** pending (docs: complete plan)

## Files Created/Modified
- `crates/racecontrol/src/config.rs` - Added resolve_jwt_secret(), extended apply_env_overrides() with 6 env var checks, 11 new unit tests

## Decisions Made
- Used `rand::thread_rng().r#gen()` instead of `rand::rng().random()` because workspace uses rand 0.8 and `gen` is a reserved keyword in Rust 2024 edition
- Used `format!("{:02x}")` loop for hex encoding instead of adding hex crate -- avoids new dependency for a trivial operation
- Wrapped all `std::env::set_var`/`remove_var` in tests with `unsafe {}` blocks -- required by Rust 2024 edition for thread safety
- Kept `default_jwt_secret()` function -- still needed by serde `#[serde(default)]` for deserialization; the dangerous value is caught and replaced at runtime by `resolve_jwt_secret()`

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] rand 0.8 API differs from plan's rand 0.9 API**
- **Found during:** Task 1 (implementation)
- **Issue:** Plan specified `rand::rng().random()` which is rand 0.9 API; workspace uses rand 0.8
- **Fix:** Used `rand::thread_rng().r#gen()` (rand 0.8 equivalent, with raw identifier for Rust 2024 keyword)
- **Files modified:** crates/racecontrol/src/config.rs
- **Verification:** cargo test passes, random key generation works
- **Committed in:** fb6102e

**2. [Rule 3 - Blocking] Rust 2024 edition makes set_var/remove_var unsafe**
- **Found during:** Task 1 (test writing)
- **Issue:** `std::env::set_var` and `std::env::remove_var` are unsafe in Rust 2024 edition (workspace edition = 2024)
- **Fix:** Wrapped all env var mutations in tests with `unsafe {}` blocks, added SAFETY comment
- **Files modified:** crates/racecontrol/src/config.rs
- **Verification:** All 14 tests pass with --test-threads=1
- **Committed in:** fb6102e

---

**Total deviations:** 2 auto-fixed (2 blocking)
**Impact on plan:** Both fixes required for compilation. No scope creep -- same behavior, different API surface.

## Issues Encountered
None beyond the deviations documented above.

## User Setup Required

**Production deployment requires setting environment variables on the server.** The env vars are:
- `RACECONTROL_JWT_SECRET` -- JWT signing key (most critical; without it, a random key is generated on each restart)
- `RACECONTROL_TERMINAL_SECRET` -- cloud terminal auth
- `RACECONTROL_RELAY_SECRET` -- Bono relay auth
- `RACECONTROL_EVOLUTION_API_KEY` -- WhatsApp OTP via Evolution API
- `RACECONTROL_GMAIL_CLIENT_SECRET` -- Gmail OAuth
- `RACECONTROL_GMAIL_REFRESH_TOKEN` -- Gmail OAuth

Set as system-level env vars on server .23 (`setx /M RACECONTROL_JWT_SECRET "value"`). The bat file inherits system env vars.

## Next Phase Readiness
- Secrets infrastructure complete -- all 6 secrets can be managed via env vars
- Ready for auth middleware phase (Phase 76) which will enforce JWT on protected routes
- Production server needs env var setup before next deploy (see User Setup Required)

---
*Phase: 75-security-audit-foundations*
*Completed: 2026-03-20*
