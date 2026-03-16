# Phase 25: Billing Guard + Server Bot Coordinator — Research

**Researched:** 2026-03-16
**Domain:** Rust/Tokio async billing state machine, WebSocket message routing, cloud sync CRDT wallet fence
**Confidence:** HIGH

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| BILL-01 | `billing.rs` characterization test suite written before any billing bot code — covers start_session, end_session, idle detection, sync paths | billing.rs already has 27 tests; BILL-01 means expanding with characterization tests that cover the exact paths bot code will touch (game-exit detection, DrivingState idle, end_billing_session side-effects) |
| BILL-02 | Bot detects stuck session (billing active >60s after game process exits) and triggers safe `end_session()` via correct StopSession → SessionUpdate::Finished order | billing_guard.rs new file in rc-agent; uses FailureMonitorState.billing_active + game_pid absence; routes through AgentMessage::BillingAnomaly to server |
| BILL-03 | Bot detects idle billing drift (billing active + DrivingState inactive > 5 minutes) and alerts staff rather than auto-ending | billing_guard.rs reads DrivingState from FailureMonitorState; sends staff alert via email_alerts (server side), does NOT call end_session |
| BILL-04 | Bot-triggered session end fences cloud sync — waits for sync acknowledgment before completing teardown to prevent wallet CRDT race | bot_coordinator.rs on server; must call cloud_sync mechanism or check sync state before declaring teardown complete |
| BOT-01 | `bot_coordinator.rs` on racecontrol handles billing recovery message routing and server-side bot responses | New file in racecontrol-crate; receives BillingAnomaly/TelemetryGap/HardwareFailure from ws/mod.rs match arms (currently tracing::info! stubs) |
</phase_requirements>

---

## Summary

Phase 25 splits billing protection across two layers: the agent (`billing_guard.rs`) detects stuck sessions and idle drift, the server (`bot_coordinator.rs`) routes those anomaly reports to the correct recovery handlers. The two never duplicate work — the agent sends an `AgentMessage::BillingAnomaly` report, the server owns the `end_session()` call and the cloud sync fence.

The BILL-01 prerequisite gate is the most important sequencing constraint. `billing.rs` already has 27 tests (BillingTimer unit tests, cost calculation, multiplayer wait, WhatsApp receipt formatting) but has **zero tests** covering the paths the bot will exercise: game-exit-while-billing-active, idle drift detection, and the server-side end_billing_session side-effect chain (StopGame + SessionEnded + wallet refund + cloud sync). Characterization tests MUST be green before any bot code is added.

The wallet CRDT race is the second hardest problem. The cloud wallets table uses `MAX(updated_at)` semantics (cloud authoritative). If `end_session()` runs locally, debits the wallet locally, and the cloud sync pushes a stale balance at the next 2-second relay tick, the balance update can be clobbered. The fence pattern is: set a `sync_pending` flag after `end_billing_session()`, check `relay_available` AtomicBool (already in AppState), and only declare teardown complete after one successful `push_via_relay()` or `sync_once_http()` cycle.

**Primary recommendation:** Wave 0 = BILL-01 characterization tests (billing.rs, no new files). Wave 1 = billing_guard.rs (rc-agent) + bot_coordinator.rs skeleton (racecontrol). Wave 2 = cloud sync fence + integration test. Deploy to Pod 8 for live verification.

---

## Standard Stack

### Core (already in project)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| tokio::sync::watch | (tokio bundle) | FailureMonitorState broadcast to billing_guard | Already used in failure_monitor.rs; zero-copy borrow pattern |
| tokio::sync::mpsc | (tokio bundle) | AgentMessage send from billing_guard to WS sender | Established agent message channel in main.rs |
| rc_common::protocol::AgentMessage | local | BillingAnomaly variant already defined | PROTO-02 delivered this in Phase 23 |
| rc_common::types::PodFailureReason | local | BillingStuckSession / IdleDriftDetected variants | PROTO-01 covers these |
| crate::billing::end_billing_session | local (private) | Server-side session teardown | The canonical end path; must not be bypassed |
| crate::email_alerts | local | Staff alert for BILL-03 idle drift | Already used in pod_healer.rs for 3+ issues |
| crate::pod_healer::is_pod_in_recovery | local | Guard against concurrent fix races | Already in pod_healer.rs, public fn |

### Architecture Notes

**billing_guard.rs lives in rc-agent.** It polls `FailureMonitorState` (same watch channel as `failure_monitor.rs`) and sends `AgentMessage::BillingAnomaly` through the existing `agent_msg_tx` mpsc. It does NOT call `end_session()` directly — that would bypass the server's wallet management.

**bot_coordinator.rs lives in racecontrol-crate (server side).** It is called from `ws/mod.rs` in the `BillingAnomaly`, `TelemetryGap`, and `HardwareFailure` match arms which currently contain `tracing::info!` stubs. It receives `Arc<AppState>` and routes to the correct handler function.

**Installation:** No new Cargo dependencies required. All needed functionality is already in the project.

---

## Architecture Patterns

### Existing Pattern: FailureMonitorState Watch Channel

`failure_monitor.rs` already polls a `watch::Receiver<FailureMonitorState>` every 5s. `billing_guard.rs` uses the SAME watch channel (same sender, additional receiver). This avoids a second background task for basic detection.

`FailureMonitorState` already contains:
- `billing_active: bool` — set/cleared in main.rs at BillingStarted/SessionEnded/SubSessionEnded/BillingStopped
- `game_pid: Option<u32>` — current game process PID
- `recovery_in_progress: bool` — blocks autonomous actions

**Missing from FailureMonitorState for Phase 25:** `driving_state: Option<DrivingState>` — needs to be added (see Pitfalls). There is no `driving_state` field currently.

### Pattern: billing_guard.rs Detection Rules

```rust
// Source: failure_monitor.rs pattern (Phase 24)
// In billing_guard spawn() loop:

// BILL-02: Stuck session (game exited but billing still active)
if state.billing_active && state.game_pid.is_none() {
    // elapsed since first game_pid went None while billing was active
    if elapsed_game_gone > Duration::from_secs(60) && !stuck_fired {
        stuck_fired = true;
        let msg = AgentMessage::BillingAnomaly {
            pod_id: pod_id.clone(),
            billing_session_id: "unknown".to_string(), // server resolves
            reason: PodFailureReason::BillingStuckSession,
            detail: format!("game_pid=None for {}s while billing active", elapsed),
        };
        let _ = agent_msg_tx.try_send(msg);
    }
} else {
    stuck_fired = false;
    elapsed_game_gone reset;
}

// BILL-03: Idle drift (DrivingState not Active for 5 minutes)
if state.billing_active && !is_driving_active(&state) {
    if idle_elapsed > Duration::from_secs(300) && !idle_fired {
        idle_fired = true;
        let msg = AgentMessage::BillingAnomaly {
            pod_id: pod_id.clone(),
            billing_session_id: "unknown".to_string(),
            reason: PodFailureReason::IdleDriftDetected,
            detail: format!("DrivingState inactive for {}s while billing", idle_elapsed),
        };
        let _ = agent_msg_tx.try_send(msg);
    }
} else {
    idle_fired = false;
    idle_elapsed reset;
}
```

**Key:** `stuck_fired` and `idle_fired` are task-local booleans (same as `launch_timeout_fired` in failure_monitor.rs) to prevent duplicate fires per incident. Reset when condition clears.

### Pattern: bot_coordinator.rs Routing

```rust
// Source: ws/mod.rs stub pattern (Phase 23)
// bot_coordinator.rs in racecontrol-crate

pub async fn handle_billing_anomaly(
    state: &Arc<AppState>,
    pod_id: &str,
    billing_session_id: &str,
    reason: PodFailureReason,
    detail: &str,
) {
    // Guard: skip if pod is in recovery
    let wd_state = state.pod_watchdog_states.read().await
        .get(pod_id).cloned().unwrap_or(WatchdogState::Healthy);
    if is_pod_in_recovery(&wd_state) {
        tracing::info!("[bot-coord] BillingAnomaly for {} skipped — pod in recovery", pod_id);
        return;
    }

    match reason {
        PodFailureReason::BillingStuckSession => {
            recover_stuck_session(state, pod_id).await;
        }
        PodFailureReason::IdleDriftDetected => {
            alert_staff_idle_drift(state, pod_id, detail).await;
        }
        _ => {
            tracing::warn!("[bot-coord] Unhandled BillingAnomaly reason {:?}", reason);
        }
    }
}
```

### Pattern: recover_stuck_session() — Correct Order

The correct teardown sequence is documented in ws/mod.rs (AcStatus::Off handler):

```
1. end_billing_session(state, &session_id, BillingSessionStatus::EndedEarly) // sets status, does DB write, sends StopGame + SessionEnded
2. // end_billing_session internally sends: StopGame, then SessionEnded (with driving_seconds)
3. // The agent receives SessionEnded → shows 15s summary → engages lock screen (LIFE-03)
4. // cloud sync fence: confirm wallet delta synced before returning
```

`end_billing_session` is a **private** async fn. The public wrapper is `end_billing_session_public`. `bot_coordinator.rs` must call `end_billing_session_public(state, &session_id, BillingSessionStatus::EndedEarly)`.

**Session ID resolution:** When the agent sends `BillingAnomaly`, the `billing_session_id` may be "unknown" (agent doesn't always know server's session UUID). Server resolves by looking up the active timer for the pod: `state.billing.active_timers.read().await.get(pod_id).map(|t| t.session_id.clone())`.

### Pattern: Cloud Sync Fence (BILL-04)

The cloud sync concern from STATE.md: wallet is authoritative in cloud. If `end_billing_session()` debits locally and the cloud hasn't synced, a subsequent sync could overwrite the local debit.

**What actually syncs:** Cloud sync tables include `wallets` (in `SYNC_TABLES` constant). The sync pushes rows where `updated_at > last_push`. After `end_billing_session()` updates the wallet, the next `push_via_relay()` cycle (2s) or `sync_once_http()` cycle (30s) will include the wallet delta.

**Fence approach:** After calling `end_billing_session_public()`, bot_coordinator waits for the next sync cycle to complete. The `relay_available` AtomicBool is on `state`. Pattern:

```rust
// After end_billing_session_public() returns:
let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
loop {
    tokio::time::sleep(Duration::from_secs(2)).await;
    // Check if relay sync cycle fired since we called end_session
    // Simple: just wait one 2s relay tick (sufficient for real-time relay path)
    // If relay down, accept 30s HTTP fallback was scheduled
    if state.relay_available.load(Ordering::Relaxed) || deadline.elapsed().is_ok() {
        break;
    }
}
```

**Simpler approach (acceptable):** Log a billing_event with type `bot_recovery` immediately after `end_billing_session_public()`. The cloud sync will pick this up in its next cycle. The wallet debit happens atomically in `end_billing_session()` via `wallet::refund()`. The test for BILL-04 is: trigger a stuck session, check wallet balance locally, wait 5 seconds for sync cycle, confirm no balance discrepancy.

### Anti-Patterns to Avoid

- **Direct timer mutation from bot_coordinator:** Never access `state.billing.active_timers` directly in bot_coordinator. Always call `end_billing_session_public()` — this preserves the side-effect chain (StopGame, SessionEnded, wallet refund, dashboard broadcast, pod status reset).
- **Calling end_billing_session() from rc-agent:** The agent cannot call server billing functions. Agent sends `BillingAnomaly`; server acts. Never short-circuit this boundary.
- **Auto-ending on idle drift:** BILL-03 explicitly says alert, not end. Do NOT call `end_billing_session_public()` in `alert_staff_idle_drift()`. Staff decides.
- **Bypassing is_pod_in_recovery():** Every bot_coordinator handler must check this guard. The pod healer uses the same guard; reuse the pattern exactly.
- **Adding billing_guard.rs without a separate spawn site:** billing_guard::spawn() is a separate tokio task from failure_monitor::spawn(). They share the watch receiver but run independently.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Session teardown sequence | Custom teardown in bot_coordinator | `end_billing_session_public()` | Already handles StopGame, SessionEnded, wallet refund, dashboard broadcast, pod status — 200+ lines of battle-tested logic |
| Recovery concurrency guard | Another mutex | `is_pod_in_recovery()` from pod_healer | Phase 23 delivered this; reusing it is the whole point of PROTO-03 |
| Staff alert delivery | Custom email sender | `state.email_alerter` | Already used in pod_healer.rs; sends via Google Workspace SMTP |
| Wallet debit | Direct SQL UPDATE | `wallet::refund()` in wallet.rs | Handles atomicity, syncs with cloud authoritative balance |
| Cloud sync coordination | Manual HTTP calls | Wait for existing sync cycle via relay_available | The 2s relay tick already fires; bot just needs to wait for the next cycle |

---

## Common Pitfalls

### Pitfall 1: DrivingState Not in FailureMonitorState
**What goes wrong:** billing_guard.rs tries to read `state.driving_state` but the field doesn't exist in `FailureMonitorState`. Compile error.
**Why it happens:** Phase 24 added `billing_active`, `game_pid`, `hid_connected`, `launch_started_at` to FailureMonitorState but NOT `driving_state`.
**How to avoid:** Wave 0 must add `driving_state: Option<DrivingState>` to `FailureMonitorState` AND add a `send_modify` update site in main.rs wherever DrivingStateUpdate is received (around line 885-900 in rc-agent/src/main.rs). The test for this is in the characterization test suite.
**Warning signs:** Compile error "no field `driving_state`" in billing_guard.rs.

### Pitfall 2: session_id Resolution — Agent Doesn't Know Server's UUID
**What goes wrong:** `BillingAnomaly.billing_session_id` sent from agent is empty/wrong; `end_billing_session_public()` looks it up, finds nothing, session persists.
**Why it happens:** The agent knows `billing_active=true` (from BillingStarted message) but the server-generated session UUID is not sent back to the agent in `CoreToAgentMessage::BillingStarted`.
**How to avoid:** In `bot_coordinator.handle_billing_anomaly()`, ALWAYS resolve session_id from `state.billing.active_timers.read().await.get(pod_id)`. Ignore whatever billing_session_id the agent sent — use it only for logging.
**Warning signs:** `end_billing_session_public()` returns false (session not found).

### Pitfall 3: DebugMemory bypass — billing gate must be INSIDE billing_guard functions
**What goes wrong:** billing_guard checks billing_active at the spawn loop level but not inside the fix function body. DebugMemory instant_fix() can call the fix function directly, bypassing the call-site guard.
**Why it happens:** Same root cause as fix_frozen_game (documented in Phase 24). BILL-02 recovery triggers end_session which is destructive; must not fire when billing is already stopped.
**How to avoid:** `recover_stuck_session()` must read `state.billing.active_timers.read().await.contains_key(pod_id)` at function entry. If no active timer, return early with log. This is the server-side equivalent of the `snapshot.billing_active` gate in rc-agent fix functions.
**Warning signs:** Test for double-fire recovery produces error in log.

### Pitfall 4: StopGame Sent Before end_billing_session
**What goes wrong:** bot_coordinator sends `StopGame` to the agent, then calls `end_billing_session_public()`. Agent closes game → billing timer is still ticking. Race: agent sends `GameCrashed` → ws/mod.rs calls `handle_game_status_update(Off)` → `end_billing_session()` fires AGAIN → double-end.
**Why it happens:** Misunderstanding of the correct teardown order. `end_billing_session()` already sends `StopGame` internally.
**How to avoid:** `bot_coordinator.recover_stuck_session()` calls ONLY `end_billing_session_public()`. That function handles `StopGame` + `SessionEnded` internally. Never send `StopGame` separately in bot_coordinator.
**Warning signs:** Log shows two `end_billing_session` calls for the same session_id.

### Pitfall 5: Cloud Sync CRDT Race on Wallet
**What goes wrong:** Bot calls `end_billing_session_public()`, which calls `wallet::refund()` locally. 2 seconds later, cloud pushes a `wallets` row with an older `updated_at` timestamp. The cloud row overwrites the local refund.
**Why it happens:** Cloud is authoritative for wallets. The sync uses `MAX(updated_at)` — if cloud's record has a newer `updated_at` than local (possible if a previous sync from cloud was received but our local refund hasn't synced out yet), the cloud row wins.
**How to avoid:** After `end_billing_session_public()`, log a `billing_recovery` event with `event_type='bot_recovery'` immediately. This creates a fresh `updated_at` on `billing_events`. Then wait for sync cycle (2s relay tick) before declaring teardown complete. The wallet refund from `wallet::refund()` also updates the wallet's `updated_at` — ensuring our outgoing push includes the new balance.
**Warning signs:** Balance discrepancy after stuck session recovery in integration test (BILL-04 test).

### Pitfall 6: Bot Fires During server-Commanded Recovery
**What goes wrong:** Server is already restarting rc-agent (pod_monitor recovery cycle). billing_guard sees billing_active + game_pid=None and fires BillingAnomaly. Server processes BillingAnomaly and tries to end a session that was already being handled.
**Why it happens:** Agent-side FailureMonitorState has `recovery_in_progress` but it might not be set if the recovery is server-initiated.
**How to avoid:** Both guards matter: (1) rc-agent: check `state.recovery_in_progress` before sending BillingAnomaly (same guard as failure_monitor.rs line 101). (2) Server: `bot_coordinator.handle_billing_anomaly()` checks `is_pod_in_recovery()`.
**Warning signs:** Double session-end logs during pod restart cycles.

---

## Code Examples

### BillingAnomaly match arm in ws/mod.rs (current stub)

```rust
// Source: crates/racecontrol/src/ws/mod.rs lines 514-516
AgentMessage::BillingAnomaly { pod_id, billing_session_id, reason, detail } => {
    tracing::info!("[bot] BillingAnomaly pod={} session={} reason={:?}: {}", pod_id, billing_session_id, reason, detail);
}
```

This stub gets replaced with a call to `bot_coordinator::handle_billing_anomaly(state, pod_id, billing_session_id, reason, detail).await`.

### end_billing_session_public signature

```rust
// Source: crates/racecontrol/src/billing.rs lines 1835-1841
pub async fn end_billing_session_public(
    state: &Arc<AppState>,
    session_id: &str,
    end_status: BillingSessionStatus,
) -> bool {
    end_billing_session(state, session_id, end_status).await
}
```

This is the only correct entry point for bot_coordinator to end a session.

### is_pod_in_recovery signature

```rust
// Source: crates/racecontrol/src/pod_healer.rs lines 775-780
pub fn is_pod_in_recovery(wd_state: &WatchdogState) -> bool {
    matches!(
        wd_state,
        WatchdogState::Restarting { .. } | WatchdogState::Verifying { .. }
    )
}
```

### relay_available AtomicBool (for BILL-04 fence)

```rust
// Source: state.rs (inferred from cloud_sync.rs line 152)
state.relay_available.store(effective_relay_up, Ordering::Relaxed);
// Read in bot_coordinator:
let relay_up = state.relay_available.load(Ordering::Relaxed);
```

### FailureMonitorState fields relevant to Phase 25

```rust
// Source: crates/rc-agent/src/failure_monitor.rs lines 34-63
pub struct FailureMonitorState {
    pub game_pid: Option<u32>,
    pub last_udp_secs_ago: Option<u64>,
    pub hid_connected: bool,
    pub launch_started_at: Option<Instant>,
    pub billing_active: bool,
    pub recovery_in_progress: bool,
    // MISSING: driving_state — must be ADDED in Wave 0
}
```

### Email alert pattern from pod_healer.rs

```rust
// Source: crates/racecontrol/src/pod_healer.rs lines 367-373
state.email_alerter
    .write()
    .await
    .send_alert(&pod.id, &subject, &body)
    .await;
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Manual staff monitoring of billing | Bot detects stuck sessions automatically | Phase 25 (this phase) | Staff freed from watching timers |
| BillingAnomaly stub in ws/mod.rs | bot_coordinator.rs with real routing | Phase 25 (this phase) | Anomaly reports actually trigger recovery |
| No idle drift detection | 5-min idle alerts to staff | Phase 25 (this phase) | Prevents silent billing when customer walks away |

**Deferred (v6.0):**
- `DBG-01`: DebugMemory pattern keys include billing context — deferred
- `DBG-02`: Bot action log in staff dashboard — deferred
- `BILL-D1`: Auto-refund partial credit on bot termination — explicitly out of scope (too risky without human review)

---

## Open Questions

1. **DrivingState field addition to FailureMonitorState**
   - What we know: Field does not exist. main.rs sends `DrivingStateUpdate` around line 885 and 900; `detector.state()` is the source.
   - What's unclear: Whether `FailureMonitorState` is the right place, or if billing_guard.rs should watch a separate channel.
   - Recommendation: Add `driving_state: Option<DrivingState>` to FailureMonitorState (simplest, consistent with existing pattern). Add send_modify site at DrivingStateUpdate handler in main.rs. This is a Wave 0 task alongside BILL-01 tests.

2. **Wallet sync fence implementation depth**
   - What we know: relay_available AtomicBool already exists. cloud_sync runs every 2s on relay path, 30s on HTTP fallback.
   - What's unclear: Whether a simple 5-second wait is sufficient or a real acknowledgment from cloud is needed.
   - Recommendation: The STATE.md lists this as a pending blocker. For Phase 25, implement the minimal fence: after `end_billing_session_public()`, wait max 5s for relay_available to be true, then log completion. The integration test (BILL-04) verifies the balance. Full CRDT acknowledgment (waiting for cloud to echo back) is deferred to v6.0.

3. **billing_session_id in BillingAnomaly sent from agent**
   - What we know: `CoreToAgentMessage::BillingStarted` sends session_id to agent. Agent stores it.
   - What's unclear: Whether agent currently stores the billing_session_id from BillingStarted in a field accessible to billing_guard.
   - Recommendation: Add `billing_session_id: Option<String>` to `FailureMonitorState`. Set it in main.rs at BillingStarted handler (already updates billing_active). Clear at session end. This allows billing_guard to send the correct session_id. If this adds complexity to Wave 0, send "unknown" and let server resolve — already documented in Pitfall 2.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in test harness (`cargo test`) |
| Config file | Cargo.toml per crate; no separate test config |
| Quick run command | `cargo test -p racecontrol-crate -- billing` |
| Full suite command | `cargo test -p rc-common && cargo test -p rc-agent-crate && cargo test -p racecontrol-crate` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| BILL-01 | BillingTimer tick behavior, start/end session paths, idle detection conditions | unit (pure) | `cargo test -p racecontrol-crate -- billing::tests` | ✅ (extend existing mod tests) |
| BILL-01 | end_billing_session side-effect chain (StopGame sent, timer removed, pod status reset) | unit (mock) | `cargo test -p racecontrol-crate -- billing::tests::characterization` | ❌ Wave 0 |
| BILL-01 | Game-exit-while-billing: AcStatus::Off triggers end_billing_session | unit (pure logic) | `cargo test -p racecontrol-crate -- billing::tests::game_exit_ends_session` | ❌ Wave 0 |
| BILL-02 | billing_guard: game_pid=None for 60s while billing_active=true triggers BillingAnomaly | unit (pure logic) | `cargo test -p rc-agent-crate -- billing_guard::tests` | ❌ Wave 1 |
| BILL-02 | bot_coordinator: BillingAnomaly(BillingStuckSession) routes to recover_stuck_session | unit | `cargo test -p racecontrol-crate -- bot_coordinator::tests` | ❌ Wave 1 |
| BILL-02 | recover_stuck_session returns early if no active timer for pod | unit | `cargo test -p racecontrol-crate -- bot_coordinator::tests::recover_no_timer_noop` | ❌ Wave 1 |
| BILL-03 | billing_guard: DrivingState inactive 300s while billing triggers BillingAnomaly(IdleDrift) | unit | `cargo test -p rc-agent-crate -- billing_guard::tests::idle_drift_fires_at_5min` | ❌ Wave 1 |
| BILL-03 | bot_coordinator: BillingAnomaly(IdleDriftDetected) routes to alert, NOT end_session | unit | `cargo test -p racecontrol-crate -- bot_coordinator::tests::idle_drift_alerts_not_ends` | ❌ Wave 1 |
| BILL-04 | Wallet balance consistent after stuck session recovery + sync wait | integration | `cargo test -p racecontrol-crate -- integration::billing_bot_sync_fence` | ❌ Wave 2 |
| BOT-01 | bot_coordinator receives BillingAnomaly/TelemetryGap/HardwareFailure and routes each to handler | unit | `cargo test -p racecontrol-crate -- bot_coordinator::tests::routes_all_three_variants` | ❌ Wave 1 |

### Sampling Rate

- **Per task commit:** `cargo test -p racecontrol-crate -- billing::tests`
- **Per wave merge:** `cargo test -p rc-common && cargo test -p rc-agent-crate && cargo test -p racecontrol-crate`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps (BILL-01 Prerequisite Gate)

These test additions to `billing.rs` are the BILL-01 gate. NO billing bot code may be written until these pass:

- [ ] `billing.rs tests::game_exit_while_billing_ends_session` — pure logic test: AcStatus::Off path in `handle_game_status_update()` removes timer from active_timers. Tests the `if let Some(session_id) = session_id` branch at billing.rs line 572.
- [ ] `billing.rs tests::idle_drift_condition_check` — pure logic test: billing_active=true + DrivingState not Active for >300s means alert (not auto-end). Characterizes what BILL-03 is protecting against.
- [ ] `billing.rs tests::end_session_removes_timer` — pure logic test: `BillingTimer` removed from `active_timers` HashMap after end. Uses `BillingTimer::dummy()` helper already in billing.rs.
- [ ] `billing.rs tests::stuck_session_condition` — pure logic test: billing_active=true + game_pid=None for 60s matches BILL-02 detection condition.
- [ ] `FailureMonitorState` add `driving_state: Option<DrivingState>` field — compile gate before billing_guard.rs can be written.
- [ ] `failure_monitor.rs` update `send_modify` at DrivingStateUpdate site in main.rs — needed for billing_guard to see driving state.

*(Note: billing.rs already has 27 passing tests. Wave 0 adds ~5 characterization tests targeting the bot-facing paths specifically.)*

---

## Sources

### Primary (HIGH confidence)

- Direct codebase reading: `crates/racecontrol/src/billing.rs` (3,791 lines, 27 existing tests)
- Direct codebase reading: `crates/rc-agent/src/failure_monitor.rs` (Phase 24 output, billing_active field confirmed)
- Direct codebase reading: `crates/rc-agent/src/ai_debugger.rs` (billing gate pattern in fix_frozen_game confirmed)
- Direct codebase reading: `crates/racecontrol/src/ws/mod.rs` (BillingAnomaly/TelemetryGap/HardwareFailure stub arms confirmed at lines 508-516)
- Direct codebase reading: `crates/rc-common/src/protocol.rs` (BillingAnomaly variant with fields confirmed)
- Direct codebase reading: `crates/racecontrol/src/pod_healer.rs` (is_pod_in_recovery() public fn confirmed)
- Direct codebase reading: `crates/racecontrol/src/cloud_sync.rs` (relay_available pattern confirmed)
- `.planning/REQUIREMENTS.md` — BILL-01 through BILL-04, BOT-01 requirements
- `.planning/STATE.md` — Accumulated decisions including wallet sync fence concern

### Secondary (MEDIUM confidence)

- `.planning/ROADMAP.md` — Phase 25 goal, success criteria, dependency on Phase 24
- rc-agent/src/main.rs grep results — billing_active send_modify sites (8 confirmed update sites)

### Tertiary (LOW confidence)

- Wallet CRDT race mechanics — inferred from cloud_sync.rs SYNC_TABLES list and normalize_timestamp() behavior. The exact race window and MAX(updated_at) semantics are described in STATE.md CONCERNS.md reference but CONCERNS.md was not read directly.

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all libraries already in project, confirmed by direct code reading
- Architecture patterns: HIGH — billing_guard pattern directly modeled on failure_monitor.rs; bot_coordinator modeled on pod_healer.rs
- Pitfalls: HIGH — billing gate (Pitfall 3) confirmed by Phase 24 decisions in STATE.md; DrivingState gap (Pitfall 1) confirmed by reading FailureMonitorState definition
- Cloud sync fence: MEDIUM — mechanism identified, exact CRDT semantics not fully verified (CONCERNS.md not read)

**Research date:** 2026-03-16
**Valid until:** 2026-04-16 (stable internal codebase; no external dependencies)
