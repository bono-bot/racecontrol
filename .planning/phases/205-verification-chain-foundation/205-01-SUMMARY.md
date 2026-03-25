---
phase: 205-verification-chain-foundation
plan: 01
subsystem: testing
tags: [rust, rc-common, verification, thiserror, tokio, tracing]

# Dependency graph
requires: []
provides:
  - VerifyStep trait with Input/Output associated types and name()/run() methods
  - VerificationError enum with 4 variants (InputParseError, TransformError, DecisionError, ActionError) each carrying step name + raw_value
  - ColdVerificationChain.execute_step() wrapping each step in tracing::info_span
  - HotVerificationChain (tokio-gated) fire-and-forget to ring buffer (capacity 64)
  - VerificationResult struct for ring buffer entries
  - spawn_periodic_refetch() generic async periodic re-fetch with lifecycle logging
affects: [206-observable-state, 207-process-guard-verification, 208-boot-resilience-consumers, 210-fleet-audit]

# Tech tracking
tech-stack:
  added: [thiserror = { workspace = true } in rc-common/Cargo.toml]
  patterns:
    - "VerifyStep trait with associated Input/Output types — all verification steps implement this"
    - "ColdVerificationChain.execute_step() wraps each step call in info_span(target=verification)"
    - "HotVerificationChain fires tokio::spawn, stores VerificationResult in Arc<Mutex<VecDeque>> cap 64"
    - "spawn_periodic_refetch generic over T/E/F/Fut — accepts any async closure returning Result"
    - "boot_resilience module #[cfg(feature = tokio)] — rc-sentry (no tokio) never sees it"

key-files:
  created:
    - crates/rc-common/src/verification.rs
    - crates/rc-common/src/boot_resilience.rs
  modified:
    - crates/rc-common/Cargo.toml
    - crates/rc-common/src/lib.rs

key-decisions:
  - "verification.rs is NOT feature-gated — verification types used by all crates including rc-sentry which has no tokio"
  - "boot_resilience.rs IS feature-gated behind #[cfg(feature = tokio)] — rc-sentry must not compile it"
  - "ColdVerificationChain uses execute_step() method pattern (not a builder chain) — simplest viable design that provides tracing spans per step without variadic generics"
  - "HotVerificationChain ring buffer capacity hardcoded at 64 — sufficient for diagnostics without unbounded growth"
  - "Mutex poisoning handled with unwrap_or_else(|p| p.into_inner()) in production code — ring buffer is diagnostic only, poisoning should not propagate"

patterns-established:
  - "VerificationError variants: always carry both step: String and raw_value: String — enables debugging without re-fetching source data"
  - "spawn_periodic_refetch: lifecycle events use structured tracing fields (resource, error, retry_count, downtime_ms) — parseable by log aggregators"

requirements-completed: [COV-01, BOOT-01]

# Metrics
duration: 15min
completed: 2026-03-26
---

# Phase 205 Plan 01: Verification Chain Foundation Summary

**VerifyStep trait, ColdVerificationChain, HotVerificationChain, VerificationError (4 typed variants), and spawn_periodic_refetch() added to rc-common with 13 tests passing and all downstream crates (rc-agent, racecontrol, rc-sentry) still compiling**

## Performance

- **Duration:** ~15 min
- **Started:** 2026-03-26T04:51:00Z
- **Completed:** 2026-03-26T05:06:49Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- Created verification.rs: VerifyStep trait + ColdVerificationChain + HotVerificationChain + VerificationError + VerificationResult — the foundational types Phases 206-208 compile against
- Created boot_resilience.rs: spawn_periodic_refetch() with full lifecycle logging (started/first_success/failed/self_healed/exit) — feature-gated behind tokio so rc-sentry never sees it
- cargo test -p rc-common (no tokio): 190 tests pass, boot_resilience module correctly skipped
- cargo test -p rc-common --features tokio: 195 tests pass (13 new: 10 verification + 3 boot_resilience + 1 doctest)
- All downstream binaries cargo check clean: rc-agent (warnings only), racecontrol (warnings only), rc-sentry (warnings only)

## Task Commits

Each task was committed atomically:

1. **Task 1: Create verification.rs** - `77c2b564` (feat)
2. **Task 2: Create boot_resilience.rs** - `b6c346d5` (feat)

**Plan metadata:** (see final commit below)

## Files Created/Modified
- `crates/rc-common/src/verification.rs` — VerifyStep trait, ColdVerificationChain, HotVerificationChain, VerificationError (4 variants), VerificationResult, 10 tests
- `crates/rc-common/src/boot_resilience.rs` — spawn_periodic_refetch() generic async periodic re-fetch with full lifecycle tracing, 3 tests
- `crates/rc-common/Cargo.toml` — added `thiserror = { workspace = true }` dependency
- `crates/rc-common/src/lib.rs` — added `pub mod verification;` and `#[cfg(feature = "tokio")] pub mod boot_resilience;`

## Decisions Made
- verification.rs is not feature-gated — VerificationError and VerifyStep are needed by all crates
- boot_resilience.rs is feature-gated behind tokio — rc-sentry has no async runtime
- ColdVerificationChain uses execute_step() method per-call rather than a builder — simpler and avoids Rust variadic generic limitations
- HotVerificationChain ring buffer capped at 64 with VecDeque::pop_front() eviction — deterministic memory bounds

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Created boot_resilience.rs during Task 1 to enable compilation**
- **Found during:** Task 1 (verification.rs creation)
- **Issue:** lib.rs was updated to declare `pub mod boot_resilience` but the file did not exist — cargo refused to compile verification tests
- **Fix:** Created boot_resilience.rs with full implementation (was planned for Task 2 anyway, just executed concurrently with Task 1 verification)
- **Files modified:** crates/rc-common/src/boot_resilience.rs
- **Verification:** cargo test -p rc-common --features tokio -- verification passed 10 tests after fix
- **Committed in:** b6c346d5 (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (Rule 3 - blocking issue)
**Impact on plan:** No scope creep — boot_resilience.rs was already planned for Task 2. Creating it during Task 1 to unblock compilation is purely ordering, not added scope.

## Issues Encountered
- boot_resilience.rs needed to exist before Task 1 verification tests could compile — lib.rs declared both modules simultaneously. Resolved by creating boot_resilience.rs immediately rather than waiting for Task 2.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Phase 206 (observable state) can now import `rc_common::verification::*` — types are stable
- Phase 207 (process guard verification) can import VerifyStep and implement concrete steps
- Phase 208 (boot resilience consumers) can call spawn_periodic_refetch() with the tokio feature enabled
- Phase 209 (bash tooling) has zero dependency on these Rust types — can develop in parallel
- Phase 210 (fleet audit) depends on outputs from Phases 206-209

---
*Phase: 205-verification-chain-foundation*
*Completed: 2026-03-26*
