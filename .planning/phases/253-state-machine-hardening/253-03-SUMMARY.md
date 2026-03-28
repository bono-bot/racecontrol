---
phase: 253-state-machine-hardening
plan: 03
subsystem: billing
tags: [fsm, split-sessions, billing, sqlite, cas, db-before-launch]

# Dependency graph
requires:
  - phase: 253-01
    provides: billing_fsm.rs with BillingEvent, validate_transition, authoritative_end_session
  - phase: 252-financial-atomicity-core
    provides: atomic billing start (FATM-01), CAS session finalization (FATM-04)
provides:
  - split_sessions table with UNIQUE(parent_session_id, split_number) constraint
  - SplitStatus enum in billing_fsm.rs (Pending/Active/Completed/Cancelled)
  - create_split_records(): N child splits with equal allocated_seconds + last-split remainder
  - get_next_pending_split(): lowest-numbered pending split query
  - transition_split(): CAS Active→Completed + next Pending→Active
  - cancel_pending_splits(): cancels all pending splits on parent cancellation
  - transition_to_next_split() in game_launcher.rs: DB-persist before launch command
  - FSM-08 DB-before-launch guard in launch_game(): rejects split 2+ without DB record
affects: [253-04, 254-security-hardening, 256-game-specific-hardening, 257-billing-edge-cases]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "CAS on split status (UPDATE WHERE status='active') prevents concurrent double-transition"
    - "DB-before-launch ordering: persist split → verify in DB → update in-memory → then send WS command"
    - "Lock snapshot before await: billing timer data copied then lock dropped before DB call"
    - "Last-split gets remainder seconds (total_seconds % split_count) ensuring sum = total"

key-files:
  created: []
  modified:
    - crates/racecontrol/src/billing_fsm.rs
    - crates/racecontrol/src/billing.rs
    - crates/racecontrol/src/db/mod.rs
    - crates/racecontrol/src/game_launcher.rs

key-decisions:
  - "SplitStatus defined in billing_fsm.rs (not billing.rs) — FSM module owns all status enums"
  - "split_sessions table added to db/mod.rs migration (not billing.rs) — DB schema lives in db/"
  - "create_split_records() failure on session start is non-fatal (logged at ERROR) — prevents blocking a valid customer session start due to a split bookkeeping failure; the parent session is already committed"
  - "FSM-08 guard snapshots timer data before await — no RwLock held across DB call (standing rule)"
  - "transition_to_next_split() does 3-step atomic sequence: CAS transition → DB verify → memory update"

patterns-established:
  - "DB-before-launch: always persist state to DB and verify before issuing irreversible commands"
  - "CAS on status transitions: UPDATE WHERE status='expected' + rows_affected check = concurrent guard"
  - "Split remainder: last split absorbs remainder seconds, ensuring sum of all splits = total session time"

requirements-completed: [FSM-07, FSM-08]

# Metrics
duration: 17min
completed: 2026-03-28
---

# Phase 253 Plan 03: State Machine Hardening Summary

**Split session parent+child entitlement model with CAS guards, FSM-08 DB-before-launch guard preventing orphaned game launches when no billing record exists**

## Performance

- **Duration:** 17 min
- **Started:** 2026-03-28T21:29:55Z
- **Completed:** 2026-03-28T21:46:04Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments

- FSM-07: `split_sessions` table in DB with `UNIQUE(parent_session_id, split_number)` constraint — concurrent insert guard at DB layer
- FSM-07: `SplitStatus` enum + 4 lifecycle functions with CAS guards (`create_split_records`, `get_next_pending_split`, `transition_split`, `cancel_pending_splits`) + 7 async tests all passing
- FSM-08: `transition_to_next_split()` in game_launcher.rs — 3-step atomic sequence (CAS DB transition → verify → memory update) before any launch command is issued
- FSM-08: DB-before-launch guard in `launch_game()` rejects split 2+ launches if split record not persisted as active

## Task Commits

Each task was committed atomically:

1. **Task 1: Split_sessions table, SplitStatus FSM, split lifecycle logic** - `5a105a8c` (feat)
2. **Task 2: DB-before-launch guard in game_launcher.rs** - `4d0d2e39` (feat)

## Files Created/Modified

- `crates/racecontrol/src/billing_fsm.rs` — Added `SplitStatus` enum (Pending/Active/Completed/Cancelled) with `as_str()`
- `crates/racecontrol/src/billing.rs` — Added 4 split lifecycle functions + 7 async tests; wired `create_split_records()` into `start_billing_session()` for split_count > 1
- `crates/racecontrol/src/db/mod.rs` — Added `split_sessions` table migration + 2 indexes (parent_id, parent_id+status)
- `crates/racecontrol/src/game_launcher.rs` — Added `transition_to_next_split()` + FSM-08 guard in `launch_game()`

## Decisions Made

- `SplitStatus` placed in `billing_fsm.rs` (not `billing.rs`) — FSM module owns all status enums, consistent with `BillingEvent`
- `split_sessions` table migration in `db/mod.rs` (not inline in billing.rs) — DB schema lives in the db/ module
- `create_split_records()` failure is non-fatal in `start_billing_session()` — logs at ERROR but doesn't block the customer session (parent session already committed to DB)
- FSM-08 guard snapshots timer lock before the async DB call — no RwLock held across `.await` (mandatory standing rule)

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- Pre-existing flaky test `crypto::encryption::tests::load_keys_valid_hex` fails intermittently when run with the full test suite (test isolation issue with environment variable contamination). Passes when run in isolation. Not related to this plan's changes — logged for deferred investigation.

## Known Stubs

None — all split lifecycle functions are fully implemented with real DB queries. No placeholder data.

## Next Phase Readiness

- FSM-07 and FSM-08 complete — split session modeling and DB-before-launch guard are production-ready
- Phase 253 complete (FSM-01 through FSM-08 all done across plans 01-03)
- Ready for Phase 254 (Security Hardening, SEC-01–10)

---
*Phase: 253-state-machine-hardening*
*Completed: 2026-03-28*
