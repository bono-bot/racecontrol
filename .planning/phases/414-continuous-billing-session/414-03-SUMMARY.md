---
phase: 414-continuous-billing-session
plan: "03"
subsystem: billing
tags: [protocol, serde, typescript, cascade, idle-warning, dashboard-event]
dependency_graph:
  requires: ["414-00", "414-01", "414-02"]
  provides: ["DashboardEvent::IdleWarning", "BillingSessionInfo.between_games_idle_seconds", "TS BillingSession.between_games_idle_seconds"]
  affects: ["kiosk", "web", "admin", "dashboard_ws"]
tech_stack:
  added: []
  patterns: ["tagged-union serde round-trip", "post-lock async DB query", "TS optional field cascade"]
key_files:
  created: []
  modified:
    - crates/rc-common/src/protocol.rs
    - crates/rc-common/src/types.rs
    - crates/racecontrol/src/billing.rs
    - crates/racecontrol/src/billing_timer.rs
    - crates/racecontrol/src/billing_tests.rs
    - packages/shared-types/src/billing.ts
    - packages/contract-tests/src/billing.contract.test.ts
    - web/src/lib/api.ts
decisions:
  - "IdleWarning serializes as 'idle_warning' (snake_case) per DashboardEvent #[serde(rename_all = snake_case)] — test asserts this exact tag"
  - "between_games_idle_seconds: Option<u32> with #[serde(default, skip_serializing_if = Option::is_none)] for backward-compat"
  - "B2 fix: per-tick BillingTick emitted INSIDE the WaitingForGame mid-stream branch (sync, no lock violation)"
  - "Wallet balance query (.await) placed AFTER lock drop — CLAUDE.md lock discipline preserved"
  - "web/src/lib/api.ts BillingSession redeclaration cascaded per CLAUDE.md cascade rule"
metrics:
  duration_minutes: 20
  completed_date: "2026-04-18"
  tasks_completed: 3
  tasks_total: 3
  files_changed: 8
---

# Phase 414 Plan 03: Wave 3 — Protocol Additions + Cascade Summary

**One-liner:** Added `DashboardEvent::IdleWarning` (5 fields) + `BillingSessionInfo.between_games_idle_seconds: Option<u32>` to rc-common; wired real wallet-balance IdleWarning broadcast post-lock; added per-tick BillingTick in WaitingForGame branch (B2); cascaded TS type to shared-types + web; 4 Wave-0 protocol/contract tests GREEN.

## Tasks Completed

| Task | Name | Commit | Key Files |
|------|------|--------|-----------|
| 1 | Add IdleWarning variant + between_games_idle_seconds; un-ignore round-trip tests | `894420c9` | protocol.rs, types.rs, billing.rs, billing_timer.rs, billing_tests.rs |
| 2 | Wire DashboardEvent::IdleWarning broadcast + B2 per-tick BillingTick in WaitingForGame | `9382f77a` | billing_timer.rs, billing.rs |
| 3 | TS cascade + un-skip CONTRACT-01 test | `d0db978e` | shared-types/billing.ts, billing.contract.test.ts, web/api.ts |

## Protocol Additions

### DashboardEvent::IdleWarning (new variant)

Added to `crates/rc-common/src/protocol.rs` after `CommandError`:

```rust
IdleWarning {
    pod_id: String,
    session_id: String,
    balance_paise: u64,
    seconds_remaining: u32,
    can_continue: bool,
}
```

Serializes as `{ "event": "idle_warning", "data": { ... } }` (snake_case per existing enum attribute).

DashboardEvent variant count: was N variants, now N+1 (IdleWarning appended).

### BillingSessionInfo.between_games_idle_seconds (new field)

Added to `crates/rc-common/src/types.rs`:

```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
pub between_games_idle_seconds: Option<u32>,
```

BillingSessionInfo field count: +1 additive optional field. Old clients ignore unknown JSON keys — backward-compat preserved.

## Wave-0 Tests Now GREEN (PROTOCOL-01 + PROTOCOL-02)

| Test ID | Test Name | File | Result |
|---------|-----------|------|--------|
| PROTOCOL-01 | `test_idle_warning_serde_roundtrip` | protocol.rs | PASS |
| PROTOCOL-02 | `test_billing_info_idle_seconds_roundtrip` | types.rs | PASS |
| Backward-compat | `billing_session_info_without_optional_fields_backward_compat` | types.rs | PASS (no regression) |
| CONTRACT-01 | `Phase 414 — BillingSession.between_games_idle_seconds field` | billing.contract.test.ts | PASS (describe.skip removed) |
| CONTRACT-02 | `billing_tick_between_games has elapsed_seconds > 0 and between_games_idle_seconds set` | ws-dashboard.contract.test.ts | PASS (was already passing from Wave 0) |

**rc-common test suite:** 254 passed, 0 failed, 0 ignored (was 252 + 2 ignored before this plan).

**racecontrol-crate --lib billing:** 183 passed, 0 failed, 4 ignored (unchanged — no regression).

**vitest contract tests:** 54 passed, 0 failed (was 53 passed, 1 skipped).

## B2 Fix Verification

Per plan-checker B2 requirement: `DashboardEvent::BillingTick(timer.to_info(&rate_tiers))` is now emitted inside the WaitingForGame mid-stream branch so the kiosk paused-meter UI receives live `between_games_idle_seconds` countdown data every second.

```
grep -n "DashboardEvent::BillingTick" crates/racecontrol/src/billing_timer.rs
109: PausedDisconnect branch
139: PausedGamePause/PausedCrashRecovery branch
171: WaitingForGame mid-stream branch (NEW — B2 fix)
271: Active branch (existing)
499: waiting_for_game map broadcast (existing)
```

5 total BillingTick emits — line 171 is the new B2 fix inside the WaitingForGame branch.

## IdleWarning Broadcast Wiring

The Plan 02 placeholder log loop was replaced with a real broadcast:

1. After `active_timers` write lock drops — CLAUDE.md lock discipline preserved
2. `SELECT balance_paise FROM wallets WHERE driver_id = ?` — same pattern as per-minute debit
3. `unwrap_or(0)` — no `.unwrap()` per CLAUDE.md, graceful degradation for missing wallet
4. `can_continue = balance_paise >= rate_paise_per_minute`
5. `state.dashboard_tx.send(DashboardEvent::IdleWarning { ... })` — existing broadcast channel

No `.await` inside any lock guard scope. Lock discipline: `let mut timers = ..write().await` → work inside lock (sync only) → `drop(timers)` → async wallet query → broadcast.

## BillingTimer.to_info() Population

`billing.rs` `to_info()` now populates:

```rust
between_games_idle_seconds: if self.status == BillingSessionStatus::WaitingForGame && self.elapsed_seconds > 0 {
    Some(self.between_games_idle_seconds)
} else {
    None
},
```

`Some(N)` only during mid-stream WaitingForGame — kiosk renders countdown from this value.

## TS Cascade Summary

| File | Change | Method |
|------|--------|--------|
| `packages/shared-types/src/billing.ts` | Added `between_games_idle_seconds?: number` + `recovery_pause_seconds?: number` | Primary type definition |
| `packages/contract-tests/src/billing.contract.test.ts` | Removed `describe.skip`, implemented round-trip assertion | Test activation |
| `web/src/lib/api.ts` | Added `between_games_idle_seconds?: number` + `recovery_pause_seconds?: number` to local BillingSession redeclaration | Cascade rule — redeclaration must match |

Kiosk and admin both import `BillingSession` from `@racingpoint/types` (shared-types) — automatically pick up the new optional field. No code change needed in those apps. Plan 05 handles kiosk consumption of the field for the paused-meter UI.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 — Missing critical field] BillingTimer.to_info() between_games_idle_seconds added in Task 1**
- **Found during:** Task 1 build verification after adding the field to BillingSessionInfo
- **Issue:** `cargo build -p racecontrol-crate` failed with `E0063: missing field 'between_games_idle_seconds' in initializer of BillingSessionInfo` — billing.rs `to_info()` needed the new field
- **Fix:** Added the field population logic to `billing.rs` `to_info()` in Task 1 commit (was Task 2 work, pulled forward to unblock compile)
- **Files modified:** `crates/racecontrol/src/billing.rs`
- **Commit:** `894420c9`

**2. [Rule 1 — Test bug] IdleWarning serde tag is snake_case 'idle_warning' not 'IdleWarning'**
- **Found during:** Task 1 test run — `test_idle_warning_serde_roundtrip` FAILED with `expected event tag "IdleWarning", got: {"event":"idle_warning"...}`
- **Issue:** `DashboardEvent` has `#[serde(rename_all = "snake_case")]` — `IdleWarning` → `"idle_warning"` in JSON. Plan spec showed `"IdleWarning"` but actual serde output is `"idle_warning"`.
- **Fix:** Updated test assertion to `assert!(json.contains("\"event\":\"idle_warning\""))` — matches actual serde behavior, consistent with all other DashboardEvent variants
- **Files modified:** `crates/rc-common/src/protocol.rs`
- **Commit:** `894420c9`

## Known Stubs

None — no hardcoded empty values or placeholder text that prevent this plan's goal. Plan 04 owns the 15-min auto-end (900s threshold); Plan 05 owns the kiosk frontend consumption. Both are future plans, not stubs in this plan's deliverables.

## Next Plan

**414-04 (Wave 4):** `handle_game_off` rewrite — removes the auto-end-on-game-stop behavior, wires `BillingEvent::GameStopped` from game-state transitions, adds 900s auto-end trigger, adds 3 surface fixes (stop_billing branches on elapsed_seconds, WaitingForGame+Disconnect transition, integration tests TIMER-04 + INTEGRATION-01..04 GREEN).

## Self-Check: PASSED

| Check | Result |
|-------|--------|
| protocol.rs exists | FOUND |
| types.rs exists | FOUND |
| billing_timer.rs exists | FOUND |
| billing.ts exists | FOUND |
| 414-03-SUMMARY.md exists | FOUND |
| Commit 894420c9 (task 1) | FOUND |
| Commit 9382f77a (task 2) | FOUND |
| Commit d0db978e (task 3) | FOUND |
| `IdleWarning {` in protocol.rs | FOUND |
| `pub between_games_idle_seconds: Option<u32>` in types.rs | FOUND |
| `DashboardEvent::IdleWarning {` in billing_timer.rs | FOUND |
| Plan 02 placeholder removed | CONFIRMED |
| `between_games_idle_seconds` in shared-types/billing.ts | FOUND |
| `describe.skip` removed from billing.contract.test.ts | CONFIRMED |
