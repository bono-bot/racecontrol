---
phase: 207-boot-resilience
plan: 01
subsystem: infra
tags: [feature-flags, boot-resilience, periodic-refetch, self-healing, tokio]

requires:
  - phase: 205-verification-chain-foundation
    provides: spawn_periodic_refetch generic pattern in rc-common::boot_resilience
provides:
  - Feature flags HTTP periodic re-fetch (5-min interval) via spawn_periodic_refetch
  - FeatureFlags::fetch_from_server() async method for HTTP GET /api/v1/flags
  - Boot resilience standing rule in CLAUDE.md with resource checklist
affects: [207-02, 208-gate-protocol, 210-fleet-audit]

tech-stack:
  added: []
  patterns: [spawn_periodic_refetch for any startup-fetched resource]

key-files:
  created: []
  modified:
    - crates/rc-agent/src/feature_flags.rs
    - crates/rc-agent/src/main.rs
    - crates/rc-agent/Cargo.toml
    - CLAUDE.md

key-decisions:
  - "Feature-gated fetch_from_server behind #[cfg(feature = http-client)] to match existing reqwest gating pattern"
  - "Enabled rc-common tokio feature in rc-agent Cargo.toml to access boot_resilience module"
  - "Mock HTTP server in tests uses raw TcpListener with read-before-write pattern for reqwest compatibility"

patterns-established:
  - "Boot resilience pattern: any startup-fetched remote data must use spawn_periodic_refetch for self-healing"

requirements-completed: [BOOT-02, BOOT-03]

duration: 8min
completed: 2026-03-26
---

# Phase 207 Plan 01: Feature Flags Periodic Re-fetch Summary

**Feature flags self-heal via HTTP GET /api/v1/flags every 5 minutes using spawn_periodic_refetch, with CLAUDE.md standing rule banning single-fetch-at-boot patterns**

## Performance

- **Duration:** 8 min
- **Started:** 2026-03-26T05:02:01Z
- **Completed:** 2026-03-26T05:10:00Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- Added FeatureFlags::fetch_from_server() async method that fetches flags via HTTP, parses Vec<FeatureFlagRow>, and applies via apply_sync
- Wired spawn_periodic_refetch in main.rs with 5-minute interval for feature flag self-healing
- Added boot resilience standing rule to CLAUDE.md with resource checklist (allowlist DONE, flags DONE, billing/camera CHECK)
- 3 new tests covering HTTP fetch success, unreachable URL error handling, and flag application

## Task Commits

Each task was committed atomically:

1. **Task 1: Add HTTP fetch + periodic re-fetch for feature flags** - `3e82bba1` (feat)
2. **Task 2: Add boot resilience standing rule to CLAUDE.md** - `f26e98ee` (docs)

## Files Created/Modified
- `crates/rc-agent/src/feature_flags.rs` - Added fetch_from_server() async method and 3 tests
- `crates/rc-agent/src/main.rs` - Added spawn_periodic_refetch block after core_http_base derivation
- `crates/rc-agent/Cargo.toml` - Enabled rc-common tokio feature for boot_resilience module access
- `CLAUDE.md` - Added boot resilience standing rule with resource re-fetch checklist

## Decisions Made
- Feature-gated fetch_from_server behind `#[cfg(feature = "http-client")]` to match existing reqwest gating pattern in rc-agent
- Enabled rc-common `tokio` feature from rc-agent Cargo.toml since boot_resilience module is gated behind it
- Used raw TcpListener mock server in tests with explicit read-before-write to ensure reqwest compatibility

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Enabled rc-common tokio feature in Cargo.toml**
- **Found during:** Task 1 (compilation)
- **Issue:** rc_common::boot_resilience is behind `#[cfg(feature = "tokio")]` but rc-agent did not enable the tokio feature on rc-common
- **Fix:** Changed `rc-common = { workspace = true }` to `rc-common = { workspace = true, features = ["tokio"] }` in rc-agent Cargo.toml
- **Files modified:** crates/rc-agent/Cargo.toml
- **Verification:** cargo check -p rc-agent-crate compiles without errors
- **Committed in:** 3e82bba1 (Task 1 commit)

**2. [Rule 1 - Bug] Fixed mock HTTP server test flakiness**
- **Found during:** Task 1 (test execution)
- **Issue:** Mock TcpListener did not read the HTTP request before writing response, causing reqwest connection errors
- **Fix:** Added AsyncReadExt::read() before response write, plus Connection: close header and stream.shutdown()
- **Files modified:** crates/rc-agent/src/feature_flags.rs (test module only)
- **Verification:** All 3 tests pass consistently
- **Committed in:** 3e82bba1 (Task 1 commit)

---

**Total deviations:** 2 auto-fixed (1 blocking, 1 bug)
**Impact on plan:** Both auto-fixes necessary for compilation and test correctness. No scope creep.

## Issues Encountered
None beyond the auto-fixed deviations above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Feature flags periodic re-fetch is wired and tested
- Ready for 207-02 (next boot resilience plan)
- Billing rates and camera config flagged as CHECK for future boot resilience verification

---
*Phase: 207-boot-resilience*
*Completed: 2026-03-26*
