---
phase: 253-state-machine-hardening
verified: 2026-03-28T17:30:00+05:30
status: gaps_found
score: 4/5 success criteria verified
re_verification: false
gaps:
  - truth: "When a game crashes, billing is paused atomically before any relaunch is attempted"
    status: partial
    reason: "The rc-agent path is correct: BillingPaused WS message sent BEFORE CrashRecoveryState::PausedWaitingRelaunch is set. However, the server-side BillingPaused handler (ws/mod.rs:835-840) is a log-only stub — it does not pause the billing timer. The actual server-side pause comes from AgentMessage::GameCrashed, which is a separate earlier message. Additionally, authoritative_end_session() is defined in billing_fsm.rs (FSM-06) but is never called anywhere — all end paths still go through end_billing_session() directly."
    artifacts:
      - path: "crates/racecontrol/src/ws/mod.rs"
        issue: "BillingPaused handler at line 835 only logs — does not pause billing timer. AgentMessage::GameCrashed handler does pause, but arrival order is non-deterministic across WS disconnects/reconnects."
      - path: "crates/racecontrol/src/billing_fsm.rs"
        issue: "authoritative_end_session() at line 130 is defined but never called (grep across crates confirms zero call sites). FSM-06 goal of convergence is met informally via end_billing_session() but the authoritative CAS path is dead code."
    missing:
      - "BillingPaused handler in ws/mod.rs should call validate_transition(CrashPause) on the timer, not just log"
      - "authoritative_end_session() should be called from end_billing_session() or replace it, otherwise the CAS guarantee is paper-only"
---

# Phase 253: State Machine Hardening — Verification Report

**Phase Goal:** Billing and game states are always consistent — phantom billing and free gaming are structurally impossible
**Verified:** 2026-03-28T17:30:00 IST
**Status:** gaps_found
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths (Success Criteria)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | An invalid state transition is rejected by the server with a logged error | VERIFIED (minor caveat) | `validate_transition()` in `billing_fsm.rs:104-120` returns `Err(msg)` and calls `tracing::warn!`. 8 invalid-transition tests pass. Caveat: logs at WARN not ERROR — "logged error" in SC-01 is ambiguous. |
| 2 | A billing session cannot remain active while the game is in Idle state | VERIFIED | Phantom billing guard in `ws/mod.rs:371-412` detects `billing=Active + game_state=Idle` via Heartbeat; after >30s logs `ERROR "PHANTOM BILLING DETECTED"` and sets `PausedGamePause`. `phantom_billing_start` field in `AppState` persists across WS reconnections. |
| 3 | A game in Running state cannot exist without an active billing session | VERIFIED (launch gate only) | Free gaming guard in `game_launcher.rs:269-279` rejects `LaunchGame` if no `active_timers` entry or no `waiting_for_game` entry exists. Returns `Err("FSM-03: Pod {} has no active billing session")`. No runtime invariant check for already-running games — protection is at launch boundary only. |
| 4 | When a game crashes, billing is paused atomically before any relaunch is attempted | PARTIAL | rc-agent path is correct: `event_loop.rs:625-656` sends `BillingPaused` WS message BEFORE setting `CrashRecoveryState::PausedWaitingRelaunch`. Server's `AgentMessage::GameCrashed` handler (`ws/mod.rs:553-564`) does pause billing. However: server's `BillingPaused` handler (`ws/mod.rs:835-840`) is log-only (no billing mutation). Server also has its own 5s relaunch spawn in `game_launcher.rs:905` that fires on `GameState::Error` — this spawn is blocked by the paused-billing gate at `launch_game()` line 282-289, but the two recovery systems (server 5s + agent 60s) operate independently with no coordination. |
| 5 | A split session is recorded to DB before any new launch command is issued | VERIFIED | `transition_to_next_split()` in `game_launcher.rs:175-220` does: (1) CAS DB transition, (2) SELECT COUNT(*) verify, (3) memory update — all BEFORE returning to caller. `launch_game()` FSM-08 guard at lines 294-333 rejects split 2+ if `SELECT COUNT(*) ... status='active'` returns 0. |

**Score: 4/5 truths verified** (SC-04 partial)

---

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/racecontrol/src/billing_fsm.rs` | FSM transition table, validate_transition(), authoritative_end_session() | VERIFIED | 390 lines. `TRANSITION_TABLE` has 20 entries. `validate_transition()` wired at 9 call sites in billing.rs + 1 in game_launcher (indirect). `authoritative_end_session()` exists but has zero call sites in production code. |
| `crates/racecontrol/src/billing.rs` | validate_transition() call sites gating all status mutations | VERIFIED | 9 confirmed call sites: LIVE resume (line 718), Pause (735), Replay/CrashPause (805), Disconnect (1006), timer expiry (1077), set_billing_status manual pause (2602), set_billing_status resume (2677), end_billing_session mapping (2782), end_billing_session (additional site at 2593) |
| `crates/racecontrol/src/game_launcher.rs` | Free gaming guard, FSM-08 DB-before-launch guard, transition_to_next_split() | VERIFIED | `launch_game()` billing gate at lines 269-279 (FSM-03). FSM-08 DB verify at lines 294-333. `transition_to_next_split()` at lines 175-220. |
| `crates/racecontrol/src/ws/mod.rs` | Phantom billing guard (FSM-02), GameCrashed billing pause (FSM-04) | PARTIAL | FSM-02 phantom guard at lines 371-412: VERIFIED. GameCrashed billing pause at lines 553-564: VERIFIED. BillingPaused handler at lines 835-840: LOG-ONLY stub, does not pause timer. |
| `crates/racecontrol/src/state.rs` | phantom_billing_start field in AppState | VERIFIED | `phantom_billing_start: RwLock<HashMap<String, Instant>>` at line 212. |
| `crates/rc-agent/src/event_loop.rs` | BillingPaused sent before CrashRecoveryState set | VERIFIED | Lines 625-657: `GameCrashed` sent (625), `BillingPaused` sent (636-640), THEN `CrashRecoveryState::PausedWaitingRelaunch` set (651). Ordering is correct. |
| `crates/rc-agent/src/ws_handler.rs` | StopGame clears crash_recovery in all states | VERIFIED | Lines 76-86 (from SUMMARY): `match &conn.crash_recovery { PausedWaitingRelaunch => log, AutoEndPending => log, Idle => {} }` then `conn.crash_recovery = Idle`. |
| `split_sessions` DB table | parent+child entitlement model with UNIQUE constraint | VERIFIED | Migration in `db/mod.rs`. CAS lifecycle functions in `billing.rs`: `create_split_records`, `get_next_pending_split`, `transition_split`, `cancel_pending_splits`. 7 async tests pass. |

---

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `billing.rs` status mutations | `billing_fsm::validate_transition()` | direct call | WIRED | 9 production call sites confirmed via grep |
| `game_launcher.rs::launch_game()` | active_timers billing gate | direct read | WIRED | Lines 273-279, 384-393 |
| `ws/mod.rs` Heartbeat handler | `phantom_billing_start` AppState | direct write | WIRED | Lines 383-410 |
| `ws/mod.rs` GameCrashed handler | `active_timers` billing pause | direct write | WIRED | Lines 555-564 |
| `ws/mod.rs` BillingPaused handler | `active_timers` | NOT WIRED | STUB | Lines 835-840: handler only logs, does not mutate timer |
| `billing_fsm::authoritative_end_session()` | Any end path in billing.rs | call | NOT WIRED | Zero call sites in production code — dead code |
| `game_launcher::transition_to_next_split()` | `split_sessions` DB + memory | DB CAS + verify | WIRED | Lines 181-219: DB CAS, SELECT verify, timer update |
| `launch_game()` FSM-08 guard | `split_sessions` DB | SELECT query | WIRED | Lines 311-333 |

---

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|--------------------|--------|
| `billing_fsm::validate_transition()` | `TRANSITION_TABLE` | const array | N/A (pure logic) | FLOWING |
| FSM-02 phantom guard | `phantom_billing_start` | `Instant::now()` per pod | Yes — real per-pod timestamps | FLOWING |
| FSM-08 DB-before-launch | `split_sessions` rows | `transition_split()` CAS DB write | Yes — real sqlx queries with rowcount check | FLOWING |
| FSM-03 free gaming guard | `active_timers` | BillingManager hashmap | Yes — populated by `start_billing_session()` | FLOWING |

---

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| FSM-01: Invalid transition rejected | `cargo test -p racecontrol-crate -- billing_fsm` | 26 tests pass (per SUMMARY) | PASS (per SUMMARY — not re-run) |
| FSM-07: Split CAS tests | `cargo test -p racecontrol-crate -- split` | 7 tests pass (per SUMMARY) | PASS (per SUMMARY) |
| FSM-04 rc-agent tests | `cargo test --bin rc-agent -- crash` | 20 passed (per SUMMARY) | PASS (per SUMMARY) |

Step 7b: SKIPPED (no server running locally; behavioral checks rely on SUMMARY-reported test results)

---

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| FSM-01 | 253-01 | Billing state transitions validated via allowed-transitions table | SATISFIED | `billing_fsm.rs` TRANSITION_TABLE + `validate_transition()` at 9 sites |
| FSM-02 | 253-02 | Cross-FSM: billing=active requires game≠Idle | SATISFIED | Phantom guard in `ws/mod.rs:371-412`, 30s detection window |
| FSM-03 | 253-02 | Cross-FSM: game=Running requires billing≠cancelled | SATISFIED (launch gate) | `launch_game()` billing gate lines 269-279 |
| FSM-04 | 253-02 | Crash recovery atomically pauses billing before relaunch | PARTIAL | rc-agent ordering correct; server-side BillingPaused handler is log-only stub |
| FSM-05 | 253-02 | StopGame handled in every recovery FSM state | SATISFIED | `ws_handler.rs` StopGame clears `crash_recovery` in all variants |
| FSM-06 | 253-01 | Billing pause timeout and crash auto-end share single authoritative trigger | PARTIAL | Both paths use `end_billing_session()` (same function = same trigger). `authoritative_end_session()` defined but dead — CAS guarantee is not enforced at runtime. |
| FSM-07 | 253-03 | Split session = parent order + child entitlements with immutable duration | SATISFIED | `split_sessions` table, `SplitStatus` enum, 4 lifecycle functions with CAS guards |
| FSM-08 | 253-03 | Split transition persisted to DB before launch command | SATISFIED | `transition_to_next_split()` + FSM-08 guard in `launch_game()` |

---

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `ws/mod.rs` | 835-840 | `AgentMessage::BillingPaused` handler is log-only — no timer mutation | Warning | FSM-04 relies on the separate GameCrashed handler for actual pause. If message order changes (WS reconnect), billing could remain Active during relaunch window. |
| `billing_fsm.rs` | 130-212 | `authoritative_end_session()` is dead code — zero call sites | Warning | FSM-06 claims CAS convergence but the function is never invoked. All end paths go through `end_billing_session()` which has its own FSM gate but not the full CAS-on-in-progress-states logic. |
| `game_launcher.rs` | 905-939 | Server has independent 5s relaunch spawn on GameState::Error, separate from rc-agent's 60s CrashRecoveryState | Info | Two independent recovery systems. Server spawn is harmlessly blocked by billing gate (paused status check). No coordination failure, but adds complexity. |
| `ws/mod.rs` | 396-399 | Phantom guard direct assignment `timer.status = BillingSessionStatus::PausedGamePause` bypasses `validate_transition()` | Warning | Inconsistent with FSM-01 mandate that all status mutations go through the transition table. |

---

### Human Verification Required

#### 1. SC-03 Runtime Invariant

**Test:** Start a billing session and launch a game. Wait for game to reach Running state. Then end the billing session via API while the game is still running. Observe whether the game continues running (free gaming) or is automatically stopped.
**Expected:** Game should be stopped within one heartbeat cycle, or a server-side guard should detect Running+no-billing and issue StopGame.
**Why human:** The free gaming guard only fires at launch time. There is no runtime scan that detects Running+no-billing after launch. This scenario requires actually running the system.

#### 2. SC-04 Message Ordering Under WS Reconnect

**Test:** Trigger a game crash while the WS connection is at high latency. Observe whether `GameCrashed` arrives before or after the server's relaunch spawn fires.
**Expected:** Billing should be paused before any LaunchGame is sent.
**Why human:** The 5s server delay provides a practical buffer, but the correctness guarantee depends on message ordering across the WS stream — not verifiable by static analysis alone.

---

### Gaps Summary

**2 structural gaps block full goal achievement:**

**Gap 1 — BillingPaused handler is a stub (SC-04/FSM-04 partial):**
The server's `AgentMessage::BillingPaused` handler in `ws/mod.rs` at lines 835-840 only writes a log entry. It does not call `validate_transition(CrashPause)` or update `timer.status`. The actual billing pause comes from `AgentMessage::GameCrashed` (lines 553-564) which arrives on the same WS stream and precedes the rc-agent's relaunch timer. In normal operation, the ordering is safe due to: (1) the 5s server-side relaunch delay, (2) the `GameCrashed` message being sent before `BillingPaused` by the rc-agent, and (3) the paused-billing gate in `launch_game()` that blocks any relaunch attempt when billing is PausedGamePause. The gap is that `BillingPaused` as a signal does nothing — the two-message protocol has a redundant, inert handler.

**Gap 2 — authoritative_end_session() is dead code (FSM-06):**
The function at `billing_fsm.rs:130` is well-written with CAS semantics, but has zero call sites. FSM-06 claims "billing pause timeout and crash recovery auto-end share a single authoritative end-session trigger" — they do share `end_billing_session()`, which is now FSM-gated via `validate_transition()`. The intent of the authoritative function (CAS on in-progress states) is not enforced at runtime. The double-end protection in `end_billing_session()` relies on `validate_transition()` rejecting terminal→terminal transitions, not on a database-level CAS.

**These gaps are non-critical for normal operation** — the primary correctness goals (phantom billing detection, free gaming prevention, split DB ordering) are all working. The gaps represent incomplete wiring of defensive layers, not broken primary paths.

---

_Verified: 2026-03-28T17:30:00 IST_
_Verifier: Claude (gsd-verifier)_
