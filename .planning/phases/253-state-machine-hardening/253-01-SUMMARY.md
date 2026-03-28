---
phase: 253-state-machine-hardening
plan: 01
subsystem: billing
tags: [rust, billing, fsm, state-machine, cas, axum, sqlx]

# Dependency graph
requires:
  - phase: 252-financial-atomicity-core
    provides: CAS finalization pattern, compute_refund(), atomic billing start

provides:
  - billing_fsm.rs module with TRANSITION_TABLE (20 transitions), validate_transition(), authoritative_end_session()
  - All billing.rs status mutations gated through validate_transition()
  - Invalid transitions rejected with tracing::warn (active->active, cancelled->ended, etc.)
  - Single authoritative CAS-protected end path for all session endings

affects: [254-security-hardening, 257-billing-edge-cases, any phase touching billing status mutations]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "FSM transition table as const array of (from, event, to) tuples — single source of truth"
    - "validate_transition() returns Result<new_status, String> — callers handle Err gracefully"
    - "authoritative_end_session() CAS: UPDATE WHERE status IN (active, paused_*, waiting_for_game)"
    - "All timer.status mutations in production code go through validate_transition()"
    - "Test code uses direct struct field assignment for test setup (exempted from FSM gate)"

key-files:
  created:
    - crates/racecontrol/src/billing_fsm.rs
  modified:
    - crates/racecontrol/src/billing.rs
    - crates/racecontrol/src/lib.rs

key-decisions:
  - "authoritative_end_session() in billing_fsm.rs does CAS + timer removal + deploy trigger; end_billing_session() retains downstream work (refunds, agent notify, pod state) and gates its mutation through validate_transition"
  - "TRANSITION_TABLE is a const &[(from, event, to)] slice — O(n) scan is acceptable for 20 transitions, no HashMap overhead"
  - "Resume event covers PausedGamePause, PausedDisconnect, and PausedManual -> Active (single event, multiple valid from-states)"
  - "CrashPause and Pause both produce PausedGamePause but are separate events for audit trail clarity"
  - "Test-only direct timer.status assignments (lines 3684-4200) exempted from FSM gate — test setup, not production mutation paths"

patterns-established:
  - "FSM-gate pattern: match validate_transition(timer.status, event) { Ok(s) => timer.status = s, Err(e) => warn }"
  - "Rejected transitions log at WARN, not ERROR — they indicate protocol violations but are handled gracefully"

requirements-completed: [FSM-01, FSM-06]

# Metrics
duration: 35min
completed: 2026-03-28
---

# Phase 253 Plan 01: State Machine Hardening — FSM Transition Table Summary

**Server-side billing FSM with 20-rule TRANSITION_TABLE, validate_transition() gates all 9 status mutation sites in billing.rs, and authoritative_end_session() provides single CAS-protected end path**

## Performance

- **Duration:** 35 min
- **Started:** 2026-03-28T21:02:11Z
- **Completed:** 2026-03-28T21:37:00Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments

- Created `billing_fsm.rs` with `BillingEvent` enum (11 variants), `TRANSITION_TABLE` const (20 transitions), `validate_transition()` function, and `authoritative_end_session()` CAS-protected end path
- Wired `validate_transition()` into all 9 production billing.rs status mutation points (LIVE resume, Pause, CrashPause/Replay, Disconnect, manual pause/resume, natural expiry, end_billing_session)
- 26 unit tests verify all valid transitions and 8 classes of invalid transitions (active->active, cancelled->ended, completed->cancel, etc.) — all pass

## Task Commits

1. **Task 1: Create billing_fsm.rs — transition table, validation, and authoritative end-session** - `679dd8d9` (feat)
2. **Task 2: Wire validate_transition() into all billing.rs mutation points** - `4ea66610` (feat)

## Files Created/Modified

- `crates/racecontrol/src/billing_fsm.rs` — New module: BillingEvent, TRANSITION_TABLE (20 entries), validate_transition(), authoritative_end_session(), 26 unit tests
- `crates/racecontrol/src/billing.rs` — 9 validate_transition() call sites replacing direct timer.status assignments in production code paths
- `crates/racecontrol/src/lib.rs` — Added `pub mod billing_fsm`

## Decisions Made

- `authoritative_end_session()` in billing_fsm.rs handles CAS + timer removal + deploy trigger; `end_billing_session()` retains refund calculation, agent notifications, and pod state cleanup — this preserves the complex downstream logic while gating the actual status mutation
- `Resume` event covers all paused states (PausedGamePause, PausedDisconnect, PausedManual) → Active with a single event variant; the transition table has separate rows for each from-state ensuring all three are explicitly allowed
- Test-only direct `timer.status = BillingSessionStatus::X` assignments at lines 3684-4200 are inside `#[cfg(test)]` block — exempted from FSM gate (test setup, not production mutations)

## Deviations from Plan

None — plan executed exactly as written.

## Issues Encountered

- Cargo package name is `racecontrol-crate` (not `racecontrol`) — test command adjusted from `cargo test -p racecontrol` to `cargo test -p racecontrol-crate`. Pre-existing.
- `test_billing_pause_timeout_refund` integration test fails with missing `idempotency_key` column — pre-existing DB schema issue in test environment, not related to FSM changes. `crypto::encryption::tests::load_keys_valid_hex` has a race condition when run in parallel — also pre-existing.

## Next Phase Readiness

- FSM-01 (transition table) and FSM-06 (authoritative end-session) are complete
- Ready for Phase 253 Plan 02 (FSM-02–FSM-05: cross-FSM invariants, phantom billing guard, etc.)
- `validate_transition()` is available to any future phase that needs to make billing status mutations

## Self-Check

- `crates/racecontrol/src/billing_fsm.rs`: FOUND
- `crates/racecontrol/src/billing.rs` (modified): FOUND
- Commit `679dd8d9`: FOUND
- Commit `4ea66610`: FOUND

## Self-Check: PASSED

---
*Phase: 253-state-machine-hardening*
*Completed: 2026-03-28*
