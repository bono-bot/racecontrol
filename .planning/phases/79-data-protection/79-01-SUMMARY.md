---
phase: 79-data-protection
plan: 01
subsystem: crypto
tags: [aes-gcm, hmac-sha256, encryption, pii, field-cipher]

requires:
  - phase: none
    provides: n/a
provides:
  - FieldCipher struct with AES-256-GCM encrypt/decrypt and HMAC-SHA256 phone hashing
  - load_encryption_keys() from RACECONTROL_ENCRYPTION_KEY and RACECONTROL_HMAC_KEY env vars
  - AppState.field_cipher available in all Axum handlers via Arc<AppState>
  - test_field_cipher() helper for unit and integration test AppState construction
affects: [79-data-protection, database-migration, query-updates, data-rights]

tech-stack:
  added: [aes-gcm 0.10, hmac 0.12, sha2 0.10, hex 0.4]
  patterns: [field-level-encryption, deterministic-phone-hashing, env-var-key-loading]

key-files:
  created:
    - crates/racecontrol/src/crypto/mod.rs
    - crates/racecontrol/src/crypto/encryption.rs
  modified:
    - Cargo.toml
    - crates/racecontrol/Cargo.toml
    - crates/racecontrol/src/lib.rs
    - crates/racecontrol/src/state.rs
    - crates/racecontrol/src/main.rs
    - crates/racecontrol/src/api/routes.rs
    - crates/racecontrol/src/auth/admin.rs
    - crates/racecontrol/src/auth/middleware.rs
    - crates/racecontrol/src/game_launcher.rs
    - crates/racecontrol/tests/integration.rs

key-decisions:
  - "Fully-qualified <Hmac<Sha256> as Mac>::new_from_slice to disambiguate KeyInit vs Mac trait conflict from aes-gcm re-export"
  - "unsafe blocks for std::env::set_var/remove_var in tests (Rust 2024 edition requirement) with --test-threads=1 safety"
  - "test_field_cipher() NOT gated by #[cfg(test)] so integration tests (separate binaries) can access it"
  - "FieldCipher does not implement Clone -- shared via Arc<AppState> which is fine for all handler access"

patterns-established:
  - "Field-level encryption: base64(nonce || ciphertext) format for encrypted fields"
  - "Phone normalization: trim + strip +91 prefix before HMAC hashing"
  - "Env var key loading: fail-fast at startup with descriptive error messages"

requirements-completed: [DATA-01, DATA-02, DATA-06]

duration: 40min
completed: 2026-03-21
---

# Phase 79 Plan 01: Crypto Foundation Summary

**AES-256-GCM FieldCipher with deterministic HMAC-SHA256 phone hashing, env-var key loading, and AppState integration -- 10 unit tests green**

## Performance

- **Duration:** 40 min
- **Started:** 2026-03-21T01:30:23Z
- **Completed:** 2026-03-21T02:10:49Z
- **Tasks:** 2
- **Files modified:** 12

## Accomplishments
- FieldCipher struct with encrypt_field (random nonce AES-256-GCM) and decrypt_field (roundtrip verified)
- Deterministic HMAC-SHA256 phone hashing with +91 prefix normalization for lookup-without-decryption
- Server fail-fast startup when RACECONTROL_ENCRYPTION_KEY or RACECONTROL_HMAC_KEY env vars missing
- FieldCipher wired into AppState.field_cipher, accessible from any Axum handler via Arc<AppState>

## Task Commits

Each task was committed atomically:

1. **Task 1: Create crypto module with FieldCipher and unit tests** - `4e1cec6` (feat)
2. **Task 2: Wire FieldCipher into AppState and server startup** - `1111487` (feat)

## Files Created/Modified
- `crates/racecontrol/src/crypto/mod.rs` - Module declaration for encryption submodule
- `crates/racecontrol/src/crypto/encryption.rs` - FieldCipher struct with encrypt_field, decrypt_field, hash_phone, load_encryption_keys, test_field_cipher
- `Cargo.toml` - Workspace dependencies: aes-gcm, hmac, sha2, hex
- `crates/racecontrol/Cargo.toml` - Crate dependencies referencing workspace
- `crates/racecontrol/src/lib.rs` - Added `pub mod crypto`
- `crates/racecontrol/src/state.rs` - Added field_cipher: FieldCipher to AppState, updated new() signature
- `crates/racecontrol/src/main.rs` - load_encryption_keys() call at startup with fail-fast expect
- `crates/racecontrol/src/api/routes.rs` - Updated 3 test make_state() with test_field_cipher
- `crates/racecontrol/src/auth/admin.rs` - Updated test make_state() with test_field_cipher
- `crates/racecontrol/src/auth/middleware.rs` - Updated test make_state() with test_field_cipher
- `crates/racecontrol/src/game_launcher.rs` - Updated test make_state() with test_field_cipher
- `crates/racecontrol/tests/integration.rs` - Updated create_test_state() with test_field_cipher

## Decisions Made
- Fully-qualified `<Hmac<Sha256> as Mac>::new_from_slice` to disambiguate KeyInit vs Mac trait conflict from aes-gcm re-export
- `unsafe` blocks for `std::env::set_var/remove_var` in tests required by Rust 2024 edition; mitigated with `--test-threads=1`
- `test_field_cipher()` exposed without `#[cfg(test)]` gate so integration test binaries (compiled separately) can access it
- FieldCipher does not implement Clone; shared via `Arc<AppState>` which is the existing pattern for all handlers

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fully-qualified HMAC trait disambiguation**
- **Found during:** Task 1 (compilation)
- **Issue:** `HmacSha256::new_from_slice` ambiguous -- both `KeyInit` (re-exported by aes-gcm) and `hmac::Mac` provide it
- **Fix:** Used `<Hmac<Sha256> as Mac>::new_from_slice` fully-qualified syntax
- **Files modified:** crates/racecontrol/src/crypto/encryption.rs
- **Committed in:** 4e1cec6

**2. [Rule 3 - Blocking] Rust 2024 unsafe env var operations**
- **Found during:** Task 1 (compilation)
- **Issue:** `std::env::set_var` and `remove_var` are unsafe in Rust 2024 edition
- **Fix:** Wrapped in `unsafe {}` blocks with SAFETY comments (single-threaded test execution)
- **Files modified:** crates/racecontrol/src/crypto/encryption.rs
- **Committed in:** 4e1cec6

**3. [Rule 3 - Blocking] Debug trait requirement for unwrap_err()**
- **Found during:** Task 1 (compilation)
- **Issue:** `Result::unwrap_err()` requires `Debug` on Ok type; FieldCipher has no Debug impl
- **Fix:** Changed to `result.err().expect("should be Err")` which only needs Debug on the Err type (String)
- **Files modified:** crates/racecontrol/src/crypto/encryption.rs
- **Committed in:** 4e1cec6

**4. [Rule 3 - Blocking] test_field_cipher visibility for integration tests**
- **Found during:** Task 2 (integration test compilation)
- **Issue:** `#[cfg(test)]` makes function invisible to integration test binaries (separate compilation units)
- **Fix:** Removed `#[cfg(test)]` gate from `test_field_cipher()` -- deterministic keys pose no production risk
- **Files modified:** crates/racecontrol/src/crypto/encryption.rs
- **Committed in:** 1111487

---

**Total deviations:** 4 auto-fixed (4 blocking)
**Impact on plan:** All auto-fixes necessary for correct compilation. No scope creep.

## Issues Encountered
- Application Control policy blocks debug test binaries (os error 4551) -- release test binaries work fine
- All tests run with `--release` flag to work around this system policy

## User Setup Required

Before deploying, the server requires two new environment variables:
- `RACECONTROL_ENCRYPTION_KEY` -- 64 hex chars (32 bytes for AES-256-GCM). Generate: `openssl rand -hex 32`
- `RACECONTROL_HMAC_KEY` -- 64 hex chars (32 bytes for HMAC-SHA256). Generate: `openssl rand -hex 32`

These must be set in `start-racecontrol.bat` or the system environment on server .23.

## Next Phase Readiness
- FieldCipher available in AppState for database migration (Plan 02) and query updates (Plan 03)
- Phone hashing ready for customer lookup migration
- Encrypt/decrypt ready for PII column encryption

## Self-Check: PASSED

- All created files exist (crypto/mod.rs, crypto/encryption.rs)
- Both task commits found (4e1cec6, 1111487)
- All 12 acceptance criteria verified (struct, functions, env vars, dependencies, module, state wiring)
- 10 unit tests pass in release mode
- Release binary compiles clean

---
*Phase: 79-data-protection*
*Completed: 2026-03-21*
