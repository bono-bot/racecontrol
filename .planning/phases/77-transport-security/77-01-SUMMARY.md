---
phase: 77-transport-security
plan: 01
subsystem: infra
tags: [tls, rustls, rcgen, axum-server, self-signed-cert, https]

# Dependency graph
requires:
  - phase: 76-api-auth-admin-protection
    provides: "ServerConfig struct, Axum server startup"
provides:
  - "tls.rs module with load_or_generate_rustls_config() and generate_and_save()"
  - "ServerConfig extended with tls_port, cert_path, key_path"
  - "axum-server, rcgen, tower-helmet dependencies in Cargo.toml"
affects: [77-02 (dual-port wiring), kiosk HTTPS migration]

# Tech tracking
tech-stack:
  added: [axum-server 0.8 (tls-rustls), rcgen 0.14, tower-helmet 0.3]
  patterns: [auto-generate self-signed cert on first run, dual-port TLS config]

key-files:
  created: [crates/racecontrol/src/tls.rs]
  modified: [crates/racecontrol/Cargo.toml, crates/racecontrol/src/config.rs, crates/racecontrol/src/lib.rs]

key-decisions:
  - "rcgen generate_simple_self_signed takes string SANs (auto-detects IPs) -- no need for SanType enum directly"
  - "CertifiedKey has signing_key field (not key_pair) in rcgen 0.14 -- research examples were slightly outdated"

patterns-established:
  - "TLS cert auto-generation: check file existence, generate if missing, load via RustlsConfig::from_pem_file"
  - "Backward-compatible config extension: Option<T> fields with #[serde(default)] for zero-breakage TOML changes"

requirements-completed: [TLS-02, TLS-04]

# Metrics
duration: 11min
completed: 2026-03-20
---

# Phase 77 Plan 01: TLS Foundation Summary

**Self-signed cert generation via rcgen with IP SAN for 192.168.31.23, RustlsConfig loader, and backward-compatible ServerConfig extension for dual-port TLS**

## Performance

- **Duration:** 11 min
- **Started:** 2026-03-20T14:02:04Z
- **Completed:** 2026-03-20T14:13:04Z
- **Tasks:** 1
- **Files modified:** 5 (including Cargo.lock)

## Accomplishments
- tls.rs module with generate_and_save() for self-signed cert generation and load_or_generate_rustls_config() for RustlsConfig loading
- ServerConfig extended with tls_port, cert_path, key_path -- all Option<T> with serde(default), zero breakage for existing TOML configs
- 6 unit tests: PEM file creation, IP SAN verification, auto-create when missing, reuse existing certs, config deserialization with/without TLS fields
- All 325 crate unit tests pass, release build succeeds

## Task Commits

Each task was committed atomically:

1. **Task 1: Add TLS dependencies and create tls.rs module with cert generation** - `051839e` (feat)

## Files Created/Modified
- `crates/racecontrol/src/tls.rs` - TLS cert generation and RustlsConfig loader module
- `crates/racecontrol/Cargo.toml` - Added axum-server 0.8, rcgen 0.14, tower-helmet 0.3
- `crates/racecontrol/src/config.rs` - Extended ServerConfig with tls_port, cert_path, key_path + 2 config tests
- `crates/racecontrol/src/lib.rs` - Added pub mod tls
- `Cargo.lock` - Updated with new dependency tree

## Decisions Made
- rcgen 0.14's `generate_simple_self_signed()` accepts `Vec<String>` and auto-detects IP addresses as SanType::IpAddress -- simpler than manually constructing SanType enums
- `CertifiedKey` has `signing_key` field (not `key_pair` as in research examples) -- corrected during implementation
- Used `std::env::temp_dir()` with process ID for test isolation instead of adding `tempfile` dev dependency

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed rcgen 0.14 API mismatch from research**
- **Found during:** Task 1 (implementation)
- **Issue:** Research showed `CertifiedKey { cert, key_pair }` and `SanType::IpAddress` constructor, but rcgen 0.14 uses `CertifiedKey { cert, signing_key }` and `generate_simple_self_signed(Vec<String>)` with auto IP detection
- **Fix:** Used correct API: `signing_key.serialize_pem()` and string-based SANs
- **Files modified:** crates/racecontrol/src/tls.rs
- **Verification:** All 6 TLS tests pass
- **Committed in:** 051839e

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** API correction necessary for compilation. No scope creep.

## Issues Encountered
- Application Control policy intermittently blocks build scripts and test binaries in debug/release modes -- retrying resolves it. Pre-existing environment issue, not caused by this plan's changes.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- tls.rs module ready for Plan 02 to wire into main.rs dual-port startup
- ServerConfig tls_port field ready for racecontrol.toml configuration
- tower-helmet dependency available for security headers middleware in Plan 02

---
*Phase: 77-transport-security*
*Completed: 2026-03-20*
