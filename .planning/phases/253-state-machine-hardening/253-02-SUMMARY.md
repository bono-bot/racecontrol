---
phase: 253-state-machine-hardening
plan: 02
subsystem: billing-fsm / game-lifecycle
tags: [fsm, billing, crash-recovery, guard, invariant]
dependency_graph:
  requires: [252-financial-atomicity-core]
  provides: [phantom-billing-detection, free-gaming-guard, crash-recovery-atomicity, stop-game-fsm-coverage]
  affects: [ws/mod.rs, game_launcher.rs, rc-agent/event_loop.rs, rc-agent/ws_handler.rs]
tech_stack:
  added: []
  patterns: [per-pod Instant tracking in AppState, CrashRecoveryState match in StopGame, conditional ws_tx result check]
key_files:
  created: []
  modified:
    - crates/racecontrol/src/state.rs
    - crates/racecontrol/src/ws/mod.rs
    - crates/racecontrol/src/game_launcher.rs
    - crates/rc-agent/src/event_loop.rs
    - crates/rc-agent/src/ws_handler.rs
decisions:
  - "phantom_billing_start stored in AppState (RwLock<HashMap<String, Instant>>) not per-connection local — ensures persistence across WS reconnections for same pod"
  - "Phantom guard auto-pauses to PausedGamePause (not auto-end) — staff decides next action"
  - "FSM-03 free gaming guard: kept existing 'no active billing' in error message text for test compat, added FSM-03 prefix"
  - "FSM-04: BillingPaused send result now checked — WS failure triggers AutoEndPending instead of relaunch"
  - "FSM-05: crash_recovery reset at TOP of StopGame handler before any game.stop() call"
metrics:
  duration: 30min
  completed_date: "2026-03-28"
  tasks: 2
  files: 5
---

# Phase 253 Plan 02: Cross-FSM Guards and Crash Recovery Hardening Summary

Added cross-FSM invariant guards (phantom billing detection, free gaming prevention) and hardened crash recovery with atomic billing pause and complete StopGame state handling.

## Tasks Completed

### Task 1: Phantom Billing Guard (FSM-02) and Free Gaming Guard (FSM-03)

**FSM-02 — Phantom Billing Guard** (`state.rs`, `ws/mod.rs`)

Added `phantom_billing_start: RwLock<HashMap<String, Instant>>` to AppState to track when `billing=active + game=Idle` condition starts per pod.

In the Heartbeat handler (`ws/mod.rs`):
- Skips check if `game_state` is `None` (old agents may not send it)
- When `billing=Active` and `game_state=Idle`: records timestamp in `phantom_billing_start`
- After >30s: logs `ERROR "PHANTOM BILLING DETECTED: pod {} ..."`, sets timer to `PausedGamePause`
- When condition clears: removes the entry from `phantom_billing_start`

**FSM-03 — Free Gaming Guard** (`game_launcher.rs`)

The existing billing gate at lines 203-223 already enforced this invariant (introduced in an earlier phase as LAUNCH-02/03/04). Changes:
- Added FSM-03 tags/comments to the existing gate
- Updated error message to include "free gaming guard" prefix while preserving "no active billing" substring (test compatibility)
- Added WARN log "FREE GAMING GUARD: Rejected LaunchGame for pod {} ..."
- Added `TODO: FSM-03 exception for free trials` comment

**Commits:**
- `669dedd8` — feat(253-02): add phantom billing guard (FSM-02) and free gaming guard tag (FSM-03)

### Task 2: Crash Recovery Atomicity (FSM-04) and StopGame FSM Coverage (FSM-05)

**FSM-04 — Atomic Billing Pause on Crash** (`rc-agent/src/event_loop.rs`)

Verified existing code sends `BillingPaused` BEFORE setting `CrashRecoveryState::PausedWaitingRelaunch` (correct order). Enhanced:
- Added FSM-04 comment documenting the ordering guarantee
- Changed `let _ = ws_tx.send(BillingPaused)` to capture result
- If WS send fails: `tracing::error!(... "FSM-04: Failed to pause billing on crash — skipping relaunch, auto-ending session")` → sets `CrashRecoveryState::AutoEndPending`
- If WS send succeeds: normal path (PausedWaitingRelaunch + relaunch timer)

**FSM-05 — StopGame in All CrashRecoveryState Variants** (`rc-agent/src/ws_handler.rs`)

Added at the TOP of the `CoreToAgentMessage::StopGame` handler:
```rust
match &conn.crash_recovery {
    CrashRecoveryState::PausedWaitingRelaunch { attempt, .. } => {
        tracing::info!("StopGame received during crash recovery (attempt {}) — cancelling relaunch", attempt);
    }
    CrashRecoveryState::AutoEndPending => {
        tracing::info!("StopGame received during AutoEndPending — clearing");
    }
    CrashRecoveryState::Idle => {} // Normal case
}
conn.crash_recovery = CrashRecoveryState::Idle;
```

Verified: `SessionEnded` (line 275) and `SubSessionEnded` (line 751) already set `crash_recovery = CrashRecoveryState::Idle`.

**Commits:**
- `6dc17054` — feat(253-02): harden crash recovery atomicity (FSM-04) and StopGame handling (FSM-05)

## Verification

- `cargo check --workspace` — clean (1 pre-existing warning in main.rs, unrelated)
- `cargo test -p racecontrol-crate` — 590 passed, 1 pre-existing failure (`load_keys_valid_hex` env var isolation issue, passes in isolation)
- `cargo test --bin rc-agent -- crash` — 20 passed, 0 failed
- `cargo test --bin rc-agent -- billing` — 29 passed, 0 failed
- `grep -rn "FSM-02|FSM-03|FSM-04|FSM-05" crates/` — all four requirement tags present

## Deviations from Plan

**None - plan executed as written.**

The plan noted that FSM-03 (free gaming guard) might not exist yet. It was already implemented as LAUNCH-02/03/04 in a previous phase. The action taken was to add the FSM-03 tag, updated error message, and TODO comment rather than re-implementing.

## Known Stubs

None. All four guards are fully wired.

## Self-Check: PASSED

- `crates/racecontrol/src/state.rs` — FOUND phantom_billing_start field
- `crates/racecontrol/src/ws/mod.rs` — FOUND PHANTOM BILLING DETECTED log
- `crates/racecontrol/src/game_launcher.rs` — FOUND FSM-03 free gaming guard
- `crates/rc-agent/src/event_loop.rs` — FOUND FSM-04 billing_pause_sent check
- `crates/rc-agent/src/ws_handler.rs` — FOUND FSM-05 crash_recovery reset in StopGame
- Commits `669dedd8` and `6dc17054` — FOUND in git log
