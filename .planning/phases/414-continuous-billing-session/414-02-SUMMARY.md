---
phase: 414-continuous-billing-session
plan: "02"
subsystem: billing
tags: [tdd, wave-2, billing-timer, idle-counter, tick-branch, continuous-billing]
dependency_graph:
  requires:
    - phase: 414-01
      provides: "3 FSM transitions incl. Active+GameStopped→WaitingForGame"
  provides:
    - "BillingTimer.between_games_idle_seconds: u32 — in-memory idle counter for mid-stream WaitingForGame"
    - "BillingTimer.idle_warning_sent: bool — one-shot flag to prevent double-fire of IdleWarning"
    - "tick() WaitingForGame branch: mid-stream (elapsed_seconds>0) increments idle counter; first-wait no-op preserved"
    - "tick_all_timers WaitingForGame branch: sets idle_warning_sent inside lock at 600s; collects candidate post-lock"
    - "3 Wave-0 timer tests GREEN: TIMER-01, TIMER-02, TIMER-03"
  affects: [414-03, 414-04, billing.rs, billing_timer.rs]
tech-stack:
  added: []
  patterns:
    - "collect-inside-lock-emit-after-drop: idle_warnings_to_emit Vec populated inside active_timers write lock, placeholder-logged after drop"
    - "guard-before-dispatch: idle_warning_sent flag set inside lock (one-shot) before post-lock emission"
key-files:
  created: []
  modified:
    - crates/racecontrol/src/billing.rs
    - crates/racecontrol/src/billing_timer.rs
    - crates/racecontrol/src/billing_tests.rs
    - crates/racecontrol/src/billing_session_start.rs
    - crates/racecontrol/src/billing_orphan.rs
    - crates/racecontrol/src/billing_session_lifecycle.rs
key-decisions:
  - "between_games_idle_seconds is in-memory only — NOT persisted (customer-favourable on server restart per D-CLOUD-SYNC)"
  - "idle_warning_sent flag is set inside the active_timers write lock to prevent double-fire on concurrent ticks"
  - "tick() does NOT set idle_warning_sent — only tick_all_timers does (separation of concerns: timer.tick is sync pure-mutation; flag check needs the full loop context)"
  - "WaitingForGame branch uses continue so the Active-specific offline-check and debit logic are skipped entirely"
metrics:
  duration: "~20 minutes"
  completed: "2026-04-18T08:45 IST"
  tasks_completed: 2
  tasks_total: 2
  files_created: 1
  files_modified: 6
requirements:
  - 414-TIMER-01
  - 414-TIMER-02
  - 414-TIMER-03
---

# Phase 414 Plan 02: Wave 2 BillingTimer Idle Counter + tick_all_timers Candidate Collection Summary

**Two fields added to BillingTimer + tick() WaitingForGame arm extended + tick_all_timers IdleWarning candidate collection — 3 Wave-0 timer tests turn GREEN (183 billing tests pass, 975 total lib tests pass)**

---

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Add 2 fields to BillingTimer + update tick() WaitingForGame arm | `c5d45d44` | billing.rs, billing_tests.rs, billing_session_start.rs, billing_session_lifecycle.rs, billing_orphan.rs |
| 2 | tick_all_timers — collect IdleWarning candidates inside lock, log post-lock | `8a271ecf` | billing_timer.rs |

---

## Fields Added to BillingTimer

**Before:** 27 fields  
**After:** 29 fields (+2)

| Field | Type | Default | Purpose |
|-------|------|---------|---------|
| `between_games_idle_seconds` | `u32` | `0` | Seconds elapsed in mid-stream WaitingForGame. NOT persisted — resets to 0 on server restart (customer-favourable). |
| `idle_warning_sent` | `bool` | `false` | One-shot flag: prevents the 10-min IdleWarning from double-firing. Set by `tick_all_timers` inside the write lock. |

---

## tick() WaitingForGame Branch Logic

**Before (single arm):**
```rust
BillingSessionStatus::WaitingForGame => false,
```

**After (two guard arms):**
```rust
BillingSessionStatus::WaitingForGame if self.elapsed_seconds > 0 => {
    // Phase 414: Mid-stream between-games. Increment idle counter; cost is FROZEN.
    self.between_games_idle_seconds += 1;
    false  // not actively billing
}
BillingSessionStatus::WaitingForGame => false, // first-wait (elapsed_seconds == 0) — existing behavior
```

The `elapsed_seconds > 0` guard is the key invariant: it ensures the idle counter only runs AFTER the customer has driven at least 1 second. The first-wait path (never-driven session waiting for first game) is unchanged.

---

## tick_all_timers Branch Logic

New block added inside the for-loop, **after PausedGamePause/PausedCrashRecovery handler, before the Active-only offline-check**:

```rust
if timer.status == BillingSessionStatus::WaitingForGame && timer.elapsed_seconds > 0 {
    if timer.between_games_idle_seconds == 600 && !timer.idle_warning_sent {
        timer.idle_warning_sent = true;           // set INSIDE lock — one-shot guard
        idle_warnings_to_emit.push((...));        // collect INSIDE lock
        tracing::info!(..., "Phase 414 Plan 02: IdleWarning threshold hit");
    }
    // Plan 04 will add: 900s auto-end
    // Plan 03 will add: BillingTick broadcast for paused-meter UI
    continue;  // skip Active-only paths (offline detection, debit, 5min/1min warnings)
}
```

Post-lock placeholder log (Plan 03 replaces with wallet lookup + DashboardEvent::IdleWarning broadcast):
```rust
for (pod_id, session_id, wallet_owner_id, rate) in &idle_warnings_to_emit {
    tracing::info!(..., "Phase 414 Plan 02: Would emit DashboardEvent::IdleWarning");
}
```

---

## Reset Point Identified

The `between_games_idle_seconds` and `idle_warning_sent` fields reset to 0/false on every `WaitingForGame → Active` transition. This happens in **Plan 04** which wires the production reset path in `handle_live_resume` (the function that processes `GameLive` events when the customer starts a new game while in WaitingForGame state).

For Plan 02, the unit test `timer_idle_counter_resets_on_resume` demonstrates the field is mutable from outside tick() — the actual production reset wiring is Plan 04's responsibility.

---

## Lock Discipline Confirmation

Verified: **zero `.await` calls inside the new WaitingForGame branch**.

The branch only:
1. Reads `timer.between_games_idle_seconds` and `timer.idle_warning_sent` (sync field reads)
2. Mutates `timer.idle_warning_sent = true` (sync field write)
3. Calls `idle_warnings_to_emit.push(...)` (sync Vec push with `.clone()` on String fields)
4. Calls `tracing::info!(...)` (sync macro)
5. `continue` (no branch left)

The post-lock placeholder log loop runs AFTER `drop(timers)` — no lock held. CLAUDE.md "Never hold a lock across .await" is satisfied.

---

## Test Results

### Wave-0 Timer Tests: 3 of 4 now GREEN

```
test billing::tests::timer_idle_counter_advances_only_in_waiting ... ok   ← was #[ignore] (TIMER-01)
test billing::tests::timer_idle_counter_resets_on_resume ... ok            ← was #[ignore] (TIMER-02)
test billing::tests::idle_warning_fires_at_600s_once ... ok                ← was #[ignore] (TIMER-03)
test billing::tests::idle_auto_ends_at_900s_completed ... ignored          ← Plan 04 (TIMER-04)
```

### Full billing test suite

```
cargo test -p racecontrol-crate --lib billing:
  183 passed; 0 failed; 4 ignored
```

### Full lib test suite (pre-commit gate)

```
cargo test -p rc-common:           1 passed; 0 failed; 0 ignored
cargo test -p racecontrol-crate --lib:  975 passed; 0 failed; 5 ignored
```

### FSM regression check (Wave 1 intact)

```
cargo test -p racecontrol-crate --lib billing_fsm:
  35 passed; 0 failed; 0 ignored
```

---

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Missing fields in 3 struct literal initializers**
- **Found during:** Task 1 — first compile attempt after adding fields to BillingTimer
- **Issue:** `billing_session_start.rs:291`, `billing_session_lifecycle.rs:311`, and `billing_orphan.rs:62` all use explicit struct literal syntax (enumerating every field) rather than `..Default::default()`. Adding 2 new fields to BillingTimer caused E0063 compile errors on all 3.
- **Fix:** Added `between_games_idle_seconds: 0` and `idle_warning_sent: false` to each initializer with a `// Phase 414:` comment explaining the intent.
- **Files modified:** billing_session_start.rs, billing_session_lifecycle.rs, billing_orphan.rs
- **Commit:** `c5d45d44`

---

## Known Stubs

None introduced. The Plan 04 stubs (TIMER-04, INTEGRATION-01/02/03) remain `#[ignore]`'d as intentional scaffolding — unchanged from Wave 0.

---

## Plan 03 Pointer

Plan 03 (protocol additions + IdleWarning broadcast wire-up) can now proceed. With Wave 2 landed:
- `timer.between_games_idle_seconds` reaches 600 exactly once per between-games gap
- `timer.idle_warning_sent` is set to `true` inside the lock at that threshold — no double-fire risk
- `idle_warnings_to_emit` Vec is populated inside the lock — Plan 03 should add wallet balance lookup + `DashboardEvent::IdleWarning` emission AFTER the lock drops (replacing the placeholder log loop)
- The `idle_warning_sent` reset to `false` on resume is Plan 04's job (along with `between_games_idle_seconds = 0`)

---

## Self-Check

Files created:
- `.planning/phases/414-continuous-billing-session/414-02-SUMMARY.md` — this file

Files modified (verified via `git show`):
- `crates/racecontrol/src/billing.rs` — `c5d45d44`
- `crates/racecontrol/src/billing_tests.rs` — `c5d45d44`
- `crates/racecontrol/src/billing_session_start.rs` — `c5d45d44`
- `crates/racecontrol/src/billing_session_lifecycle.rs` — `c5d45d44`
- `crates/racecontrol/src/billing_orphan.rs` — `c5d45d44`
- `crates/racecontrol/src/billing_timer.rs` — `8a271ecf`

## Self-Check: PASSED
