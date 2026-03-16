---
phase: 25-billing-guard-server-bot-coordinator
verified: 2026-03-16T13:00:00Z
status: passed
score: 5/5 must-haves verified
re_verification:
  previous_status: passed
  previous_score: 5/5
  gaps_closed: []
  gaps_remaining: []
  regressions: []
---

# Phase 25: Billing Guard + Server Bot Coordinator Verification Report

**Phase Goal:** The bot detects and recovers from stuck billing sessions and idle drift without risking wallet corruption — bot_coordinator.rs on racecontrol routes anomalies through the correct StopSession sequence and fences the cloud sync wallet race
**Verified:** 2026-03-16T13:00:00Z
**Status:** PASSED
**Re-verification:** Yes — regression check after initial pass

## Goal Achievement

### Observable Truths

| #  | Truth                                                                                                      | Status     | Evidence                                                                                                                                                           |
|----|------------------------------------------------------------------------------------------------------------|------------|--------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| 1  | billing.rs has 5+ characterization tests covering start_session, end_session, idle, stuck (BILL-01 gate)  | VERIFIED   | 5 BILL-01 tests confirmed at billing.rs:3793-3868 — all 50 billing tests green (cargo test -p racecontrol-crate -- billing::tests: 50 passed)                     |
| 2  | Bot detects stuck session (billing active + no game PID >= 60s), sends BillingAnomaly(SessionStuckWaitingForGame) | VERIFIED | billing_guard.rs:48-69 — 60s threshold, STUCK_SESSION_THRESHOLD_SECS==60 asserted by test; sends AgentMessage::BillingAnomaly; 7 billing_guard tests pass          |
| 3  | Bot detects idle drift (billing active + DrivingState not Active >= 300s), alerts staff, never auto-ends  | VERIFIED   | billing_guard.rs:71-93 — 300s threshold, IDLE_DRIFT_THRESHOLD_SECS==300 asserted; bot_coordinator routes IdleBillingDrift to alert_staff_idle_drift only           |
| 4  | Bot-triggered session end fences cloud sync (BILL-04) — waits up to 5s for relay_available before completing | VERIFIED | bot_coordinator.rs:147-165 — 5s deadline loop polling state.relay_available.load(Ordering::Relaxed) inside the `if ended` block                                   |
| 5  | bot_coordinator.rs on racecontrol handles BillingAnomaly, HardwareFailure, TelemetryGap routing (BOT-01)  | VERIFIED   | ws/mod.rs:508-516 routes all three variants; handle_billing_anomaly, handle_hardware_failure, handle_telemetry_gap all wired; 5 bot_coordinator tests pass          |

**Score:** 5/5 truths verified

### Required Artifacts

| Artifact                                           | Expected                                         | Status   | Details                                                                                                   |
|----------------------------------------------------|--------------------------------------------------|----------|-----------------------------------------------------------------------------------------------------------|
| `crates/racecontrol/src/billing.rs`                | 5 BILL-01 characterization tests                 | VERIFIED | Tests at lines 3793-3868: game_exit_while_billing_ends_session, idle_drift_condition_check, end_session_removes_timer, stuck_session_condition, start_session_inserts_timer — all green |
| `crates/rc-agent/src/billing_guard.rs`             | spawn() + BILL-02/BILL-03 detection + 7 tests    | VERIFIED | 161-line module; spawn() fires tokio task; stuck_session and idle_drift detection loops; WIRED via mod billing_guard in main.rs |
| `crates/racecontrol/src/bot_coordinator.rs`        | handle_billing_anomaly, recover_stuck_session, alert_staff_idle_drift, handle_hardware_failure, handle_telemetry_gap | VERIFIED | 258-line module; recovery guard at entry; correct variant routing; 5 unit tests pass |
| `crates/rc-agent/src/failure_monitor.rs`           | driving_state: Option<DrivingState> field + Default None | VERIFIED | Line 52: pub driving_state; Default at line 64: driving_state: None; assertion test passes |
| `crates/rc-agent/src/main.rs`                      | billing_guard::spawn() call + driving_state send_modify at Sites 9a/9b | VERIFIED | Lines 617-622: billing_guard::spawn(failure_monitor_tx.subscribe(), ws_exec_result_tx.clone(), pod_id.clone()); Lines 902/917: send_modify driving_state updates |
| `crates/racecontrol/src/ws/mod.rs`                 | 3 bot stub arms replaced with real bot_coordinator calls | VERIFIED | Lines 508-516: HardwareFailure, TelemetryGap, BillingAnomaly all call bot_coordinator functions with .await |
| `crates/racecontrol/src/lib.rs`                    | pub mod bot_coordinator; declaration             | VERIFIED | Line 6: pub mod bot_coordinator; confirmed                                                                |

### Key Link Verification

| From                                | To                                        | Via                                              | Status | Details                                                                                                              |
|-------------------------------------|-------------------------------------------|--------------------------------------------------|--------|----------------------------------------------------------------------------------------------------------------------|
| billing_guard.rs spawn()            | FailureMonitorState (failure_monitor.rs)  | watch::Receiver<FailureMonitorState>             | WIRED  | Parameter state_rx: watch::Receiver<FailureMonitorState>; reads .billing_active, .game_pid, .driving_state, .recovery_in_progress |
| billing_guard.rs                    | AgentMessage::BillingAnomaly              | agent_msg_tx: mpsc::Sender<AgentMessage>         | WIRED  | try_send at lines 53-62 (SessionStuckWaitingForGame) and 77-87 (IdleBillingDrift)                                    |
| main.rs (rc-agent)                  | billing_guard::spawn()                    | failure_monitor_tx.subscribe()                   | WIRED  | Lines 617-622: billing_guard::spawn(failure_monitor_tx.subscribe(), ws_exec_result_tx.clone(), pod_id.clone())       |
| main.rs (rc-agent)                  | FailureMonitorState.driving_state         | failure_monitor_tx.send_modify at Sites 9a + 9b  | WIRED  | Lines 902/917: send_modify(|s| { s.driving_state = Some(detector.state()); }) at both signal and timeout DrivingState paths |
| ws/mod.rs                           | bot_coordinator::handle_billing_anomaly   | match AgentMessage::BillingAnomaly               | WIRED  | Line 514-515: crate::bot_coordinator::handle_billing_anomaly(&state, &pod_id, &billing_session_id, *reason, &detail).await |
| ws/mod.rs                           | bot_coordinator::handle_hardware_failure  | match AgentMessage::HardwareFailure              | WIRED  | Line 508-509: crate::bot_coordinator::handle_hardware_failure(&state, &pod_id, &reason, &detail).await               |
| ws/mod.rs                           | bot_coordinator::handle_telemetry_gap     | match AgentMessage::TelemetryGap                 | WIRED  | Line 511-512: crate::bot_coordinator::handle_telemetry_gap(&state, &pod_id, *gap_seconds as u64).await               |
| bot_coordinator::recover_stuck_session | billing::end_billing_session_public   | direct async call                                | WIRED  | Line 139: end_billing_session_public(state, &session_id, BillingSessionStatus::EndedEarly).await; active_timers gate at line 123 |
| bot_coordinator::recover_stuck_session | state.relay_available (AtomicBool)    | 5s fence loop polling load(Ordering::Relaxed)    | WIRED  | Lines 148-165: loop with 1s sleep, relay_available.load, Instant deadline; inside if ended block only (BILL-04)       |
| bot_coordinator::alert_staff_idle_drift | state.email_alerter (RwLock<EmailAlerter>) | write().await.send_alert()                  | WIRED  | Lines 192-197: state.email_alerter.write().await.send_alert(pod_id, &subject, &body).await                           |
| bot_coordinator::handle_billing_anomaly | is_pod_in_recovery (pod_healer.rs)    | WatchdogState read from pod_watchdog_states      | WIRED  | Lines 35-47: reads pod_watchdog_states, calls is_pod_in_recovery(), returns early if true                             |

### Requirements Coverage

| Requirement | Source Plan | Description                                                                                          | Status    | Evidence                                                                                                                           |
|-------------|------------|------------------------------------------------------------------------------------------------------|-----------|------------------------------------------------------------------------------------------------------------------------------------|
| BILL-01     | 25-01      | billing.rs characterization test suite before bot code — covers start_session, end_session, idle, sync | SATISFIED | 5 tests at billing.rs:3793-3868 marked "BILL-01 characterization"; 50 billing tests green                                        |
| BILL-02     | 25-02      | Bot detects stuck session (billing active >60s after game exits), triggers end_session via correct sequence | SATISFIED | billing_guard.rs: 60s threshold detection sends BillingAnomaly(SessionStuckWaitingForGame); bot_coordinator.recover_stuck_session calls only end_billing_session_public |
| BILL-03     | 25-02, 25-03 | Bot detects idle billing drift (billing active + DrivingState inactive >5min), alerts staff — no auto-end | SATISFIED | billing_guard.rs: 300s threshold; bot_coordinator routes IdleBillingDrift to alert_staff_idle_drift which only sends email; alert_not_end_session_for_idle_drift test confirms invariant |
| BILL-04     | 25-04      | Bot-triggered session end fences cloud sync — waits for sync ack before completing teardown          | SATISFIED | bot_coordinator.rs:147-165: 5s fence loop polls relay_available after end_billing_session_public returns true; logs timeout if relay stays down |
| BOT-01      | 25-03      | bot_coordinator.rs on racecontrol handles billing recovery message routing and server-side bot responses | SATISFIED | Full module at crates/racecontrol/src/bot_coordinator.rs; pub mod declared in lib.rs; all three handler functions wired in ws/mod.rs |

No orphaned requirements — all 5 Phase 25 requirements (BILL-01 through BILL-04, BOT-01) appear in plan frontmatter and are accounted for.

### Anti-Patterns Found

| File               | Line | Pattern                                                  | Severity | Impact                                                                                           |
|--------------------|------|----------------------------------------------------------|----------|--------------------------------------------------------------------------------------------------|
| bot_coordinator.rs | 76   | handle_hardware_failure is a stub (only tracing::warn!)  | Info     | Intentional — Phase 24 handles rc-agent side; Phase 26 adds server-side action. BOT-01 scopes this to routing completeness only |
| bot_coordinator.rs | 92   | handle_telemetry_gap is a stub (only tracing::warn!)     | Info     | Intentional — TELEM-01 is Phase 26. Stub satisfies BOT-01 routing completeness.                  |

No blocker anti-patterns. Both stubs are on non-billing-critical paths explicitly deferred to Phase 26. All billing-critical paths (BillingAnomaly routing, recover_stuck_session, alert_staff_idle_drift) are fully implemented and tested.

Note on variant names: Plan documents use `BillingStuckSession` / `IdleDriftDetected` but actual rc-common enum variants are `SessionStuckWaitingForGame` / `IdleBillingDrift`. The implementation correctly uses the actual enum names. This is a plan documentation mismatch, not an implementation issue.

### Human Verification Required

#### 1. Sync Fence Behavior Under Relay-Down Conditions

**Test:** Kill the comms-link relay (stop Bono's VPS relay service) while a stuck session recovery is triggered on a pod. Observe rc-agent logs on the server.
**Expected:** bot_coordinator logs "Sync fence timeout 5s for session=... — HTTP fallback scheduled" and exits cleanly. Session end is still committed to local DB. No hang.
**Why human:** Cannot simulate relay outage + billing anomaly concurrently in unit tests; requires live infrastructure.

#### 2. Email Alert Delivery for Idle Drift (BILL-03 end-to-end)

**Test:** Let a pod sit with billing active and DrivingState::Idle for 5+ minutes (or manually trigger by temporarily lowering IDLE_DRIFT_THRESHOLD_SECS in a test build).
**Expected:** Email arrives at james@racingpoint.in (or configured recipient) with subject "Racing Point Alert: Pod X idle while billing active" within 5 minutes of the threshold.
**Why human:** Requires live EmailAlerter (Google Workspace/SMTP) and real pod state; unit tests mock the condition but not the email delivery.

#### 3. Stuck Session Recovery End-to-End (BILL-02 end-to-end)

**Test:** Start a billing session on Pod 8, kill acs.exe without ending the session, wait 65 seconds, check racecontrol logs.
**Expected:** bot_coordinator logs "Recovering stuck session ... for pod=pod_8", end_billing_session_public returns true, lock screen appears on Pod 8 within 5 seconds.
**Why human:** Requires real pod + billing session + game process; async timer-based detection cannot be triggered in unit tests.

### Gaps Summary

No gaps found. Phase 25 is complete and all must-haves are verified against the actual codebase:

- All 5 required source files exist, are substantive (not stubs for the billing-critical paths), and are wired correctly
- All 5 requirements (BILL-01 through BILL-04, BOT-01) have verified implementation evidence
- Test suite: 50 billing tests green, 7 billing_guard tests green, 5 bot_coordinator tests green
- Key links verified by code inspection: billing_guard spawned from main.rs, driving_state send_modify at both update sites, ws/mod.rs delegates all three AgentMessage variants to bot_coordinator, recover_stuck_session uses only end_billing_session_public, BILL-04 relay fence is in place
- Two intentional stubs (handle_hardware_failure, handle_telemetry_gap) are correctly scoped to Phase 26 per BOT-01 requirements

Re-verification regression check: no regressions detected. All items that passed in the initial verification continue to pass.

---

_Verified: 2026-03-16T13:00:00Z_
_Verifier: Claude (gsd-verifier)_
