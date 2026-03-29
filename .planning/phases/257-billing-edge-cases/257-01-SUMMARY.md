---
phase: 257-billing-edge-cases
plan: 01
subsystem: payments
tags: [billing, sqlite, tokio, axum, pwa, crash-recovery, timer]

# Dependency graph
requires:
  - phase: 252-financial-atomicity-core
    provides: atomic billing sessions, wallet debit/credit in transactions
  - phase: 253-state-machine-hardening
    provides: BillingSessionStatus FSM, PausedGamePause state
provides:
  - PWA game request TTL with 10-minute auto-expiry and GameRequestExpired broadcast
  - Extension pricing enforcement using current tier effective rate with validation
  - Billing start-time audit trail (billing_timer_started event + playable_signal_at column)
  - Crash recovery pause time tracking (PauseReason enum, recovery_pause_seconds field)
  - Dashboard-visible recovery time exclusion from billable seconds
affects:
  - 257-02 (inactivity + countdown) — shares BillingTimer struct; PauseReason enum is now established
  - 257-03 — any further billing edge cases build on PauseReason + recovery_pause_seconds
  - rc-agent event_loop — CrashRecovery pause reason flows from BillingPaused AgentMessage

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "PauseReason enum on BillingTimer for crash-vs-manual pause discrimination"
    - "Background TTL cleanup task (60s interval) with DashboardEvent broadcast for expired requests"
    - "ALTER TABLE ADD COLUMN wrapped in let _ = to be idempotent on existing databases"
    - "recovery_pause_seconds.saturating_sub() for billable time without underflow"

key-files:
  created: []
  modified:
    - crates/racecontrol/src/billing.rs
    - crates/racecontrol/src/api/routes.rs
    - crates/racecontrol/src/db/mod.rs
    - crates/racecontrol/src/main.rs
    - crates/racecontrol/src/ws/mod.rs
    - crates/racecontrol/tests/integration.rs
    - crates/rc-common/src/protocol.rs
    - crates/rc-common/src/types.rs

key-decisions:
  - "PauseReason enum (None/GamePause/CrashRecovery/Disconnect) on BillingTimer — distinguishes crash recovery pause from manual ESC pause so only crash time is excluded from billing"
  - "game_launch_requests INSERT is non-fatal (let _ =) — TTL enforcement proceeds even if INSERT fails; request_id is still returned to customer"
  - "cleanup_expired_game_requests runs every 60s and broadcasts GameRequestExpired per expired row — staff dashboard removes pending cards automatically"
  - "extension_rate_policy=current_tier_effective_rate logged on every extension — audit trail for pricing disputes"
  - "recovery_pause_seconds persisted in the existing 60-second timer DB sync — no additional write pressure"
  - "billing_timer_started audit event logged in start_billing_session — proves billing began at game-live signal, not staff launch click"

patterns-established:
  - "PauseReason enum pattern: future pause states (e.g. NetworkPartition) extend the enum rather than adding ad-hoc booleans"
  - "TTL cleanup task pattern: INSERT with expires_at column → periodic UPDATE WHERE status='pending' AND expires_at < now → DashboardEvent broadcast"

requirements-completed: [BILL-03, BILL-04, BILL-05, BILL-06]

# Metrics
duration: 45min
completed: 2026-03-29
---

# Phase 257 Plan 01: Billing Edge Cases Summary

**PWA game requests auto-expire after 10 min via server-side TTL, extensions enforce current tier rate, billing timer provably starts at game-live signal, and crash recovery pause time is excluded from billable seconds via PauseReason enum**

## Performance

- **Duration:** ~45 min
- **Started:** 2026-03-29T06:20:00Z
- **Completed:** 2026-03-29T07:05:00Z
- **Tasks:** 2 completed (Task 1: BILL-03+04, Task 2: BILL-05+06)
- **Files modified:** 8

## Accomplishments

- BILL-03: `game_launch_requests` table with `expires_at` column; `pwa_game_request()` inserts with `datetime('now', '+10 minutes')`; `GET /customer/game-request/{id}` returns `status=expired` when TTL exceeded; `cleanup_expired_game_requests()` background task (60s) marks expired rows and broadcasts `DashboardEvent::GameRequestExpired`
- BILL-04: `extend_billing()` handler validates `additional_seconds` in [60..=3600] and divisible by 60; `extend_billing_session()` logs tier rate used with `extension_rate_policy=current_tier_effective_rate` audit marker
- BILL-05: `billing_timer_started` billing event logged in `start_billing_session()` called from game-live path, creating auditable proof that timer began at `GameStateUpdate(Live)` not staff click; `playable_signal_at` and `billing_start_at` columns exist in `billing_sessions`
- BILL-06: `PauseReason` enum (None/GamePause/CrashRecovery/Disconnect) added to `BillingTimer`; `recovery_pause_seconds` increments on every `PausedGamePause` tick only when `pause_reason == CrashRecovery`; `current_cost()` uses `elapsed_seconds.saturating_sub(recovery_pause_seconds)` as billable time; field exposed in `BillingSessionInfo` for dashboard/receipt

## Task Commits

Each task was committed atomically:

1. **Task 1 + 2: PWA TTL, extension pricing, crash recovery (BILL-03/04/05/06)** - `4efc070f` (feat)

   Note: A parallel Claude session (`f27fd66b`, `7ce89fca`) had already committed billing.rs and routes.rs changes to HEAD before this session began. The `4efc070f` commit captured the genuinely new pieces: db/mod.rs migrations, main.rs spawn call, ws/mod.rs PauseReason assignment, integration test field additions, and protocol.rs GameRequestExpired event.

**Plan metadata:** (pending — this commit)

## Files Created/Modified

- `crates/racecontrol/src/billing.rs` — PauseReason enum, recovery_pause_seconds field on BillingTimer, tick() crash recovery increment, current_cost() billable subtraction, cleanup_expired_game_requests() background task, billing_timer_started event, extension tier rate logging
- `crates/racecontrol/src/api/routes.rs` — pwa_game_request() INSERT into game_launch_requests, GET /customer/game-request/{id} handler, extend_billing() additional_seconds validation
- `crates/racecontrol/src/db/mod.rs` — game_launch_requests CREATE TABLE IF NOT EXISTS, recovery_pause_seconds ALTER TABLE ADD COLUMN (idempotent)
- `crates/racecontrol/src/main.rs` — spawn_cleanup_expired_game_requests(state.clone()) after reconciliation spawn
- `crates/racecontrol/src/ws/mod.rs` — BillingPaused AgentMessage handler sets pause_reason = CrashRecovery
- `crates/racecontrol/tests/integration.rs` — Added recovery_pause_seconds: 0 and pause_reason: PauseReason::None to all 4 BillingTimer test constructors
- `crates/rc-common/src/protocol.rs` — GameRequestExpired { request_id: String } variant in DashboardEvent enum
- `crates/rc-common/src/types.rs` — recovery_pause_seconds: Option<u32> field in BillingSessionInfo (skip_serializing_if None)

## Decisions Made

- **PauseReason enum** over a boolean `is_crash_recovery_pause: bool`: enum is extensible (Disconnect will need different billing treatment in a future plan) and self-documenting
- **game_launch_requests INSERT non-fatal**: `let _ =` on the INSERT means TTL enforcement is best-effort; a DB error doesn't prevent the game request from proceeding. The request_id is still returned so the customer can poll for status
- **60-second cleanup interval** (same as timer sync): aligns with existing background task cadence, bounded write pressure
- **`billing_timer_started` as a billing event** (not a separate audit table): consistent with existing pattern for billing lifecycle events, queryable via `billing_events` table

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Missing `recovery_pause_seconds` in BillingSessionInfo struct literal**
- **Found during:** Task 2 (compiler error after adding field to BillingSessionInfo)
- **Issue:** WaitingForGame broadcast path built an inline `BillingSessionInfo { ... }` struct without the new field — compile error
- **Fix:** Added `recovery_pause_seconds: None` to the WaitingForGameEntry BillingSessionInfo literal
- **Files modified:** crates/racecontrol/src/billing.rs
- **Committed in:** `4efc070f`

**2. [Rule 3 - Blocking] Vec type annotation mismatch in billing snapshots**
- **Found during:** Task 2 (compiler error after adding recovery_pause_seconds to timer sync)
- **Issue:** `snapshots` Vec was typed as `Vec<(String, u32, u32, u32, String)>` (5-tuple) but the new map produced 6-tuples after adding recovery_pause_seconds
- **Fix:** Changed type annotation to `Vec<(String, u32, u32, u32, String, u32)>`
- **Files modified:** crates/racecontrol/src/billing.rs
- **Committed in:** `4efc070f`

**3. [Rule 3 - Blocking] Integration tests missing new BillingTimer struct fields**
- **Found during:** Task 2 (integration test compile errors)
- **Issue:** 4 BillingTimer struct literals in integration.rs were missing `recovery_pause_seconds` and `pause_reason` fields
- **Fix:** Added `recovery_pause_seconds: 0, pause_reason: racecontrol_crate::billing::PauseReason::None` to all 4 constructors
- **Files modified:** crates/racecontrol/tests/integration.rs
- **Committed in:** `4efc070f`

**4. [Rule 3 - Blocking] Test DB missing game_launch_requests table**
- **Found during:** Task 1 (unit test compile/runtime errors for BILL-03 tests)
- **Issue:** `create_test_db()` creates a minimal schema without the new table; BILL-03 tests failed with "no such table"
- **Fix:** Added inline `CREATE TABLE IF NOT EXISTS game_launch_requests` inside each BILL-03 test function
- **Files modified:** crates/racecontrol/src/billing.rs (test module)
- **Committed in:** `4efc070f`

---

**Total deviations:** 4 auto-fixed (all Rule 3 - blocking compile errors from new struct fields)
**Impact on plan:** All auto-fixes were mechanical field additions required by Rust's exhaustive struct construction. No scope creep.

## Issues Encountered

- **Parallel session conflict:** A background Claude agent had already committed billing.rs and routes.rs changes (commits `f27fd66b`, `7ce89fca`) before this session ran. The plan's two tasks were effectively split between sessions. This was handled by checking `git diff` before any writes, confirming which changes were genuinely absent (db migrations, spawn call, WS handler, integration fixes, protocol event), and committing only those.
- **Python regex over-matching:** An automated bulk-add of struct fields matched `WaitingForGameEntry` constructors that do not have PauseReason/recovery_pause_seconds. Those 5 constructors were manually reverted. The final compile verified correctness.

## User Setup Required

None - no external service configuration required. The `game_launch_requests` table is created automatically by the DB migration on next server start.

## Next Phase Readiness

- BILL-03/04/05/06 complete; PauseReason enum and recovery_pause_seconds infrastructure is in place for Phase 257-02 (inactivity + countdown)
- The `PauseReason::Disconnect` variant is defined but not yet wired — Phase 257-02 or 257-03 may use it for disconnect-billing logic
- Background agent `ae4cefa88f61c2c32` is executing 257-02 concurrently; that agent should see the new BillingTimer fields already in HEAD

---
*Phase: 257-billing-edge-cases*
*Completed: 2026-03-29*
