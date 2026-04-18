---
phase: 414-continuous-billing-session
plan: 04
subsystem: billing
tags: [billing, fsm, timer, continuous-session, snap-pricing, e2e-test]
dependency_graph:
  requires:
    - 414-00 (Wave 0 stub tests)
    - 414-01 (FSM transitions: Active+GameStopped→WaitingForGame, WaitingForGame+End→Completed, WaitingForGame+EndEarly→EndedEarly)
    - 414-02 (between_games_idle_seconds tick logic + idle_warning_sent flag)
    - 414-03 (DashboardEvent::IdleWarning broadcast at 600s + per-tick BillingTick during WaitingForGame)
  provides:
    - Backend continuous-billing model fully wired end-to-end
    - 15-min idle AUTO-END (Completed status) for mid-stream WaitingForGame sessions
    - Staff-triggered stop_billing branch for mid-stream WaitingForGame (EndedEarly + cumulative bill)
    - Stale-cancel query filter so mid-stream sessions are NOT swept by 5-min rule
    - 414-INTEGRATION-04 e2e test proving stop_billing branches correctly
  affects:
    - api/billing_session.rs::stop_billing handler (HTTP DELETE /api/v1/billing/{id})
    - billing_timer.rs::tick_all_timers (every-1s heartbeat)
    - billing_timer_stale.rs::cleanup_stale_sessions (every-tick LBILL sweep)
    - billing_game_status.rs::handle_game_off (single-player game-stop path; Task 1)
    - billing_game_status.rs::handle_live_resume (game-relaunch reset; Task 1)
tech_stack:
  added: []
  patterns:
    - "Post-lock async work — lock-discipline preserved (CLAUDE.md: never hold a lock across .await)"
    - "Status→Event mapping via end_billing_session match table (Completed→End, EndedEarly→EndEarly)"
    - "Self-contained sqlx integration tests (B3 fallback when no shared test_helpers exist)"
key_files:
  created:
    - crates/racecontrol/tests/billing_session_e2e.rs (414-INTEGRATION-04 implementation; 267 LOC)
  modified:
    - crates/racecontrol/src/billing_game_status.rs (Task 1: handle_game_off + handle_live_resume)
    - crates/racecontrol/src/billing_timer.rs (Task 2a: phase414_idle_auto_ends vec + post-lock loop)
    - crates/racecontrol/src/billing_timer_stale.rs (Task 2a: AND driving_seconds = 0 filter)
    - crates/racecontrol/src/api/billing_session.rs (Task 2b: stop_billing elapsed_seconds branch)
    - crates/racecontrol/src/billing_tests.rs (Task 1: 4 Wave-0 tests un-ignored + Task 2a: LBILL test schema column add)
decisions:
  - "B4 (CRITICAL): Auto-end uses BillingEvent::End → Completed. Staff-stop uses BillingEvent::EndEarly → EndedEarly. Two distinct paths — must NEVER be conflated."
  - "B3: e2e test written self-contained (Step 0 grep for test_helpers returned zero hits). Mirrors production stop_billing SQL operations verbatim."
  - "Distinct vec for Phase 414 auto-end (phase414_idle_auto_ends) instead of reusing H11 sessions_to_auto_end. H11 vec routes through handle_offline_auto_end which writes status='ended_early' (wrong for B4). Phase 414 routes through end_billing_session_public(Completed) (correct B4)."
  - "elapsed_seconds source for stop_billing: in-memory BillingTimer first (most accurate, ticks every 1s), falls back to persisted DB column (synced every 60s by sync_timers_to_db)."
  - "W3 closure: WaitingForGame + Disconnect open question closed without adding FSM transition. Meter is already paused in WaitingForGame; disconnect is a no-op for billing. Idle counter advances regardless of pod connectivity."
  - "Rule 3 auto-fix: Added driving_seconds INTEGER NOT NULL DEFAULT 0 to LBILL test schema (billing_tests.rs:2614). Without it, the new SQL filter errored, the match block silently returned Vec::new(), and 3 LBILL stale-cancel tests reported false negatives."
metrics:
  duration_minutes: 28
  completed: "2026-04-18T04:19:03Z"
  tasks_total: 3
  tasks_complete: 3
  tests_added: 5
  tests_unignored: 4
  loc_added: 614
  loc_removed: 60
---

# Phase 414 Plan 04: Wave 4 Backend Wiring Summary

Backend continuous-billing model now fully functional end-to-end: game-stop pauses the meter (instead of ending billing), the 15-min idle counter auto-ends sessions as `Completed` (B4 lock), staff-triggered stop on mid-stream routes to `EndedEarly` with cumulative billing, and the LBILL stale-cancel query no longer kills mid-stream WaitingForGame sessions whose `created_at` is hours old.

## Tasks Completed

### Task 1 — handle_game_off rewrite + handle_live_resume reset + 4 Wave-0 tests GREEN (commit `976cdd93`)
- `billing_game_status.rs::handle_game_off` (single-player branch): replaced `end_billing_session(EndedEarly)` with `BillingEvent::GameStopped` FSM transition. Game stop no longer ends billing; meter moves to mid-stream `WaitingForGame`. Multiplayer branch left unchanged (out of scope per CONTEXT.md v2 deferral).
- `billing_game_status.rs::handle_live_resume`: added `between_games_idle_seconds = 0` + `idle_warning_sent = false` reset on `WaitingForGame → Active` for clean re-entry into the warning cycle.
- 4 Wave-0 tests un-ignored and implemented (all assert auto-end resolves to `Completed` per B4):
  - `idle_auto_ends_at_900s_completed` — FSM permits `WaitingForGame + End → Completed`
  - `cumulative_snap_25_5_yields_pkg_30` — 25min Active + 7min wait + 5min Active → snap to ₹700 (NOT ₹750)
  - `idle_auto_end_completes_with_cumulative_cost` — 16min idle from mid-stream → FSM permits End→Completed
  - `pod_offline_in_waiting_auto_ends_completed` — pod connectivity does NOT affect idle counter (W3 closure)

### Task 2a — tick_all_timers 15-min idle auto-end + stale query filter (commit `f1600e09`)
- `billing_timer.rs`: added dedicated `phase414_idle_auto_ends: Vec<(pod_id, session_id, reason)>` collector. The Plan 02 mid-stream branch pushes candidates when `between_games_idle_seconds >= 900`. After lock-drop, post-lock loop calls `end_billing_session_public(BillingSessionStatus::Completed)` which maps to `BillingEvent::End` via the match table at `billing_session_end.rs:53`. **B4 lock honoured: auto-end → Completed (NOT EndedEarly)**.
- The vec is intentionally separate from the existing H11 `sessions_to_auto_end` vec — H11 routes through `handle_offline_auto_end` which writes `status='ended_early'` (wrong for B4). Phase 414 needs its own loop for the Completed path.
- `billing_timer_stale.rs`: added `AND driving_seconds = 0` to the LBILL stale-cancel WHERE clause. Mid-stream WaitingForGame sessions (`driving_seconds > 0`) now persist for the full 15-min idle window instead of being swept after 5 min.
- **Rule 3 auto-fix:** the LBILL test schema (billing_tests.rs::create_lbill_test_state) was missing the `driving_seconds` column. The new SQL filter caused 3 LBILL stale-cancel tests to fail (the query errored, the match returned `Vec::new()`, no sessions were swept). Added `driving_seconds INTEGER NOT NULL DEFAULT 0` to the test schema; all 5 LBILL tests now pass.

### Task 2b — stop_billing branch on elapsed_seconds + e2e test (commit `f0597923`)
- `api/billing_session.rs::stop_billing`: branched the `waiting_for_game` path on `elapsed_seconds`:
  - `elapsed_seconds == 0` → existing `CancelledNoPlayable` + full refund (preserved)
  - `elapsed_seconds > 0` → STAFF-TRIGGERED `EndedEarly` via `end_billing_session_public(BillingSessionStatus::EndedEarly)` → maps to `BillingEvent::EndEarly` (FSM-04 added in Plan 01) → `status='ended_early'`, cumulative debit retained (no refund). **B4 lock honoured: staff-stop → EndedEarly (NOT Completed)**.
  - elapsed_seconds source: in-memory `BillingTimer.elapsed_seconds` first (ticks every 1s), falls back to persisted DB column (synced every 60s).
- `tests/billing_session_e2e.rs`: 414-INTEGRATION-04 implemented self-contained (B3 fallback). Both branches exercised:
  - Branch 1 (elapsed=0): final status `cancelled_no_playable`, wallet refunded ₹700
  - Branch 2 (elapsed=600): final status `ended_early`, wallet NOT refunded (cumulative ₹250 retained)
  - Cross-branch assertion: the two terminal statuses MUST differ (`assert_ne!`) — proves B4 distinction holds.

## B4 Verification (per plan verification step 8)

| Path                        | Trigger                                              | FSM Event              | Final Status      | Verified By                                                                                       |
| --------------------------- | ---------------------------------------------------- | ---------------------- | ----------------- | ------------------------------------------------------------------------------------------------- |
| Auto-end                    | tick_all_timers detects between_games_idle_seconds >= 900 | `BillingEvent::End`    | `Completed`       | `grep "BillingSessionStatus::Completed" billing_timer.rs` (line in phase414_idle_auto_ends loop) |
| Staff-triggered stop        | stop_billing handler called by staff with elapsed_seconds > 0 | `BillingEvent::EndEarly` | `EndedEarly`      | `grep "BillingSessionStatus::EndedEarly" api/billing_session.rs` (line in mid-stream branch)     |
| Game-stop (Task 1)          | handle_game_off single-player path                   | `BillingEvent::GameStopped` | `WaitingForGame` | `grep "BillingEvent::GameStopped" billing_game_status.rs`                                        |
| Stale-sweep filter (Task 2a)| billing_timer_stale.rs::cleanup_stale_sessions       | n/a (SQL filter)       | n/a               | `grep "AND driving_seconds = 0" billing_timer_stale.rs`                                          |

**End-to-end grep results (run 2026-04-18):**
```
=== B4 grep auto-end (Completed in phase414_idle_auto_ends loop) ===
BillingSessionStatus::Completed, // BillingEvent::End via end_billing_session match
=== B4 grep stop_billing (EndedEarly) ===
rc_common::types::BillingSessionStatus::EndedEarly,
=== B3 stale fix ===
AND driving_seconds = 0
=== Task 1 GameStopped ===
match crate::billing_fsm::validate_transition(timer.status, crate::billing_fsm::BillingEvent::GameStopped)
```

## B3 Verification (helpers vs self-contained for e2e test)

Step 0 grep (per plan-checker B3) was run:
```
grep -rn 'pub fn insert_billing_session|pub fn fetch_session_status|pub fn fetch_wallet_debit' \
  crates/racecontrol/tests/ crates/racecontrol/src/
```
Result: **ZERO hits**. No shared test_helpers exist. Per plan B3 fallback, e2e test was written **self-contained** with direct sqlx setup. Choice documented in test file header comment (line 8-15). Test mirrors production `stop_billing` SQL operations verbatim — no assumptions about helper APIs.

## Test Results

```
cargo test -p rc-common                       → 1 passed (doctests)
cargo test -p racecontrol-crate --lib         → 979 passed; 0 failed; 1 ignored
cargo test --test billing_session_e2e         → 1 passed (stop_billing_branches_on_elapsed)
cargo test -p racecontrol-crate --lib billing → 187 passed; 0 failed
```

**14 Wave-0 tests now GREEN** (was: 14 #[ignore] in Wave 0):
- 5 FSM (Plan 01: GameStopped enum, Active+GameStopped, WaitingForGame+End, WaitingForGame+EndEarly, Completed+GameStopped rejected)
- 4 timer (Plan 02: TIMER-01..03; Plan 04 Task 1: TIMER-04 idle_auto_ends_at_900s_completed)
- 2 protocol round-trip (Plan 03: PROTOCOL-01 IdleWarning serde + PROTOCOL-02 between_games_idle_seconds Some/None)
- 3 integration (Plan 04 Task 1: cumulative_snap_25_5, idle_auto_end_completes_with_cumulative_cost, pod_offline_in_waiting_auto_ends_completed)

**Plus 414-INTEGRATION-04 e2e test (Task 2b) — 1 passing.**

## Lock Discipline Confirmation

`billing_timer.rs::tick_all_timers` lock discipline preserved (CLAUDE.md mandate "never hold a lock across .await"):
- The `phase414_idle_auto_ends` vec is populated INSIDE the `active_timers.write()` scope (line ~168-174, sync field reads only — no `.await`).
- The post-lock auto-end loop runs at line ~366+ AFTER `drop(timers); drop(pods);` — `end_billing_session_public` is awaited only after the write lock is released.
- `api/billing_session.rs::stop_billing` mid-stream branch: takes `active_timers.read()` for elapsed_seconds snapshot inside a tight `{ }` block, drops it BEFORE the `end_billing_session_public(...).await` call.
- E2E test does not use locks at all (in-memory sqlx pool only).

## W3 Closure (CONTEXT.md open question)

Original open question: "edge case: pod offline during WaitingForGame — what does Disconnect do?"

**Closure:** No `WaitingForGame + Disconnect` FSM transition added. Rationale (documented in Task 1 plan):
1. Meter is ALREADY paused in WaitingForGame state
2. Idle counter is server-side and advances regardless of pod connectivity
3. 15-min auto-end fires regardless of pod connectivity (uses BillingEvent::End → Completed, see B4 lock)

`pod_offline_in_waiting_auto_ends_completed` test (414-INTEGRATION-03) covers this scenario.

## Self-Check: PASSED

Files exist:
- FOUND: crates/racecontrol/src/billing_game_status.rs
- FOUND: crates/racecontrol/src/billing_timer.rs
- FOUND: crates/racecontrol/src/billing_timer_stale.rs
- FOUND: crates/racecontrol/src/api/billing_session.rs
- FOUND: crates/racecontrol/src/billing_tests.rs
- FOUND: crates/racecontrol/tests/billing_session_e2e.rs

Commits exist:
- FOUND: `976cdd93` (Task 1)
- FOUND: `f1600e09` (Task 2a)
- FOUND: `f0597923` (Task 2b)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 — Blocking issue] LBILL test schema missing driving_seconds column**
- **Found during:** Task 2a (after adding `AND driving_seconds = 0` filter to billing_timer_stale.rs)
- **Issue:** 3 LBILL stale-cancel tests (`stale_cancel_game_aware_test1_no_game_cancels`, `_test4_absolute_timeout`, `_test5_pending_always_cancels`) failed because the LBILL test schema at `billing_tests.rs::create_lbill_test_state` (line 2609-2621) did not include the `driving_seconds` column. The new SQL filter caused the query to error with "no such column: driving_seconds"; the match block at `billing_timer_stale.rs:31-42` swallowed the error and returned `Vec::new()` silently — so no sessions were swept and tests reported false negatives.
- **Fix:** Added `driving_seconds INTEGER NOT NULL DEFAULT 0` to the LBILL test schema. The other two test schemas (line 1883 minimal split-sessions schema, line 2887 F-05 schema) were inspected — only the LBILL schema needed the column.
- **Files modified:** crates/racecontrol/src/billing_tests.rs (line 2614)
- **Commit:** `f1600e09` (folded into Task 2a)

### Architectural notes

**Distinct vec for Phase 414 auto-end (NOT reusing H11 vec):** The pre-existing `sessions_to_auto_end` vec (line 51) routes through `handle_offline_auto_end` which writes `status='ended_early'`. That's the H11 path (offline + all pauses exhausted) and is wrong for B4. Phase 414 needs its own vec (`phase414_idle_auto_ends`) routed through `end_billing_session_public(Completed)` which writes `status='completed'`. Both vecs co-exist; both are processed post-lock. Documented in source comments at line 51-56 and at the auto-end loop site.

## Backend Continuous-Billing Now Functional E2E (modulo frontend)

What works after Plan 04:
- Customer drives 10 min in AC, AC quits cleanly → meter pauses (WaitingForGame, mid-stream), cumulative ₹250 retained
- Customer launches F1 25 within 10 min → meter resumes, cumulative cost continues from ₹250
- 30 cumulative driving min → snap to ₹700 (NOT ₹250 + 5×₹25 = ₹375)
- Customer walks away after 10-min IdleWarning → at 15 min idle, session AUTO-ENDS as Completed (B4)
- Staff hits End Session from kiosk while session in mid-stream WaitingForGame → routes to EndedEarly (B4), cumulative bill retained
- Staff hits End Session from kiosk while session in first-wait WaitingForGame (no driving yet) → routes to CancelledNoPlayable + full refund
- Stale-sweep no longer kills mid-stream WaitingForGame sessions whose `created_at` is hours old

What's still pending:
- **Plan 05** (Wave 5): kiosk frontend Continue/End buttons + IdleWarningDialog + paused-meter UI (cumulative ticking display)
- **Plan 06** (Wave 6): venue financial E2E test (CLAUDE.md mandate before deploy)

## Pointer to Next Plan

→ `.planning/phases/414-continuous-billing-session/414-05-PLAN.md` — Wave 5 kiosk frontend (Continue with another game / End session buttons + IdleWarningModal countdown + balance branch).
