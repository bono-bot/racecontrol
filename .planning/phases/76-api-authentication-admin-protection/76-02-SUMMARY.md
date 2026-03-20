---
phase: 76-api-authentication-admin-protection
plan: 02
subsystem: auth
tags: [argon2, admin-login, jwt, pin-hashing, spawn-blocking]

# Dependency graph
requires:
  - phase: 76-api-authentication-admin-protection
    plan: 01
    provides: "StaffClaims struct, create_staff_jwt helper, require_staff_jwt middleware"
provides:
  - "admin_login handler (POST /api/v1/auth/admin-login)"
  - "hash_admin_pin and verify_admin_pin argon2id utilities"
  - "admin_pin_hash field in AuthConfig with RACECONTROL_ADMIN_PIN_HASH env override"
affects: [76-04, 76-05, dashboard, admin-ui]

# Tech tracking
tech-stack:
  added: ["argon2 0.5 (argon2id password hashing)"]
  patterns: ["spawn_blocking for CPU-heavy argon2 verification", "PHC-format hash storage"]

key-files:
  created:
    - "crates/racecontrol/src/auth/admin.rs"
  modified:
    - "Cargo.toml"
    - "crates/racecontrol/Cargo.toml"
    - "crates/racecontrol/src/auth/mod.rs"
    - "crates/racecontrol/src/config.rs"

key-decisions:
  - "argon2 0.5 with default Argon2id params -- secure default, no custom tuning needed"
  - "spawn_blocking for PIN verification -- argon2 is CPU-heavy, must not block tokio runtime"
  - "admin_login returns sub=admin with role=staff -- reuses existing StaffClaims from Plan 01"
  - "503 when no admin_pin_hash configured -- explicit signal that admin login is not set up"

patterns-established:
  - "Argon2id PIN/password hashing pattern: hash_admin_pin for setup, verify_admin_pin for validation"
  - "CPU-heavy auth verification via tokio::task::spawn_blocking"

requirements-completed: [ADMIN-01, ADMIN-02]

# Metrics
duration: 4min
completed: 2026-03-20
---

# Phase 76 Plan 02: Admin PIN-to-JWT Login with Argon2id Verification Summary

**Admin login endpoint with argon2id PIN hashing, spawn_blocking verification, and 12-hour staff JWT issuance**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-20T13:02:26Z
- **Completed:** 2026-03-20T13:06:38Z
- **Tasks:** 1
- **Files modified:** 6

## Accomplishments
- Admin login endpoint validates PIN against argon2id hash, issues 12-hour staff JWT
- hash_admin_pin and verify_admin_pin utilities for CLI hash generation and runtime verification
- Argon2 verification runs on spawn_blocking (does not block tokio runtime)
- admin_pin_hash configurable via config file or RACECONTROL_ADMIN_PIN_HASH env var
- 8 unit tests covering hash format, verify correct/wrong/invalid, random salt, handler 401/200/503

## Task Commits

Each task was committed atomically:

1. **Task 1: Add argon2 dependency and create admin auth module with hash/verify/login** - `de5d5df` (feat)

## Files Created/Modified
- `crates/racecontrol/src/auth/admin.rs` - AdminLoginRequest/Response, hash_admin_pin, verify_admin_pin, admin_login handler, 8 unit tests
- `crates/racecontrol/src/auth/mod.rs` - Added pub mod admin and re-exports (admin_login, hash_admin_pin, verify_admin_pin)
- `crates/racecontrol/src/config.rs` - Added admin_pin_hash: Option<String> to AuthConfig with env var override
- `Cargo.toml` - Added argon2 = "0.5" to workspace dependencies
- `crates/racecontrol/Cargo.toml` - Added argon2 = { workspace = true }
- `Cargo.lock` - Updated with argon2 and transitive deps

## Decisions Made
- Used argon2 0.5 crate with default Argon2id parameters (secure defaults, no custom tuning needed for a single-user admin PIN)
- PIN verification runs on tokio::task::spawn_blocking since argon2 is CPU-intensive and would block the async runtime
- admin_login handler creates JWT with sub="admin" and role="staff", reusing StaffClaims from Plan 01 (no duplication)
- Returns 503 SERVICE_UNAVAILABLE when admin_pin_hash is not configured, making it clear that setup is required
- 12-hour JWT expiry (43200 seconds) aligns with shift-length limit

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
To enable admin login, set the admin PIN hash:
1. Generate hash: use `hash_admin_pin("your-pin")` or a CLI tool
2. Set env var: `RACECONTROL_ADMIN_PIN_HASH=$argon2id$...` or add `admin_pin_hash` to `[auth]` in racecontrol.toml

## Next Phase Readiness
- admin_login handler ready to be wired into route tree (Plan 04 or similar)
- hash_admin_pin available for generating initial admin PIN hash
- verify_admin_pin reusable for any future PIN-based auth flows

## Self-Check: PASSED

All files verified present. Commit de5d5df verified in git log.

---
*Phase: 76-api-authentication-admin-protection*
*Completed: 2026-03-20*
