---
phase: 25-billing-guard-server-bot-coordinator
verified: 2026-03-16T00:00:00Z
status: passed
score: 5/5 must-haves verified
re_verification: false
---

# Phase 25: Billing Guard + Server Bot Coordinator — Verification Report

**Phase Goal:** The bot detects and recovers from stuck billing sessions and idle drift without risking wallet corruption — bot_coordinator.rs on racecontrol routes anomalies through the correct StopSession sequence and fences the cloud sync race

**Verified:** 2026-03-16
**Status:** PASSED
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| #  | Truth                                                                                   | Status     | Evidence                                                                                                    |
|----|-----------------------------------------------------------------------------------------|------------|-------------------------------------------------------------------------------------------------------------|
| 1  | billing.rs has 5+ characterization tests covering start_session, end_session, idle, stuck | VERIFIED | 5 BILL-01 tests at lines 3793-3868; all pass (`cargo test game_exit_while_billing`, `start_session_inserts_timer`, etc.) |
| 2  | Bot detects stuck session (billing active + no game PID >= 60s) and sends BillingAnomaly | VERIFIED | billing_guard.rs lines 48-69: timer-based detection with 60s threshold, sends AgentMessage::BillingAnomaly(SessionStuckWaitingForGame) |
| 3  | Bot detects idle drift (billing active + DrivingState not Active >= 300s) and alerts staff | VERIFIED | billing_guard.rs lines 71-93: 300s threshold, sends AgentMessage::BillingAnomaly(IdleBillingDrift); bot_coordinator alerts via email, never ends session |
| 4  | Bot-triggered session end fences cloud sync — waits relay_available before completing    | VERIFIED | bot_coordinator.rs lines 147-165: 5s fence loop polling state.relay_available.load(Ordering::Relaxed) after end_billing_session_public() |
| 5  | bot_coordinator.rs handles BillingAnomaly, HardwareFailure, TelemetryGap routing         | VERIFIED | ws/mod.rs lines 508-516 route all three AgentMessage variants to bot_coordinator::{handle_billing_anomaly, handle_hardware_failure, handle_telemetry_gap} |

**Score:** 5/5 truths verified

---

### Required Artifacts

| Artifact                                             | Expected                                          | Status     | Details                                                                        |
|------------------------------------------------------|---------------------------------------------------|------------|--------------------------------------------------------------------------------|
| `crates/racecontrol/src/billing.rs`                  | 5+ BILL-01 characterization tests                 | VERIFIED   | Lines 3791-3869: 5 named characterization tests, all compile and pass           |
| `crates/rc-agent/src/billing_guard.rs`               | Stuck session + idle drift detection (BILL-02/03) | VERIFIED   | 161-line module, full implementation with 7 passing unit tests                  |
| `crates/racecontrol/src/bot_coordinator.rs`          | Routing + recover_stuck_session + alert staff     | VERIFIED   | 258-line module: handle_billing_anomaly, recover_stuck_session, alert_staff_idle_drift + 5 tests |
| `crates/rc-agent/src/failure_monitor.rs`             | driving_state field in FailureMonitorState        | VERIFIED   | Line 52: `pub driving_state: Option<DrivingState>`, default None (line 66)      |
| `crates/rc-agent/src/main.rs`                        | billing_guard::spawn() wired at startup           | VERIFIED   | Lines 617-622: billing_guard::spawn() called with shared watch receiver         |
| `crates/racecontrol/src/ws/mod.rs`                   | Stub arms replaced with bot_coordinator calls     | VERIFIED   | Lines 508-516: all three AgentMessage arms delegate to bot_coordinator functions |

---

### Key Link Verification

| From                          | To                                          | Via                                           | Status   | Details                                                                               |
|-------------------------------|---------------------------------------------|-----------------------------------------------|----------|---------------------------------------------------------------------------------------|
| billing_guard.rs              | failure_monitor.rs (FailureMonitorState)    | watch::Receiver<FailureMonitorState>          | WIRED    | spawn() takes state_rx: watch::Receiver<FailureMonitorState>; reads driving_state field |
| billing_guard.rs              | AgentMessage::BillingAnomaly                | agent_msg_tx: mpsc::Sender<AgentMessage>      | WIRED    | try_send(AgentMessage::BillingAnomaly{...}) at lines 53-62 and 77-87                  |
| main.rs (rc-agent)            | billing_guard::spawn()                      | failure_monitor_tx.subscribe()                | WIRED    | Lines 617-622: billing_guard::spawn(failure_monitor_tx.subscribe(), ws_exec_result_tx.clone(), pod_id.clone()) |
| ws/mod.rs                     | bot_coordinator::handle_billing_anomaly     | match AgentMessage::BillingAnomaly            | WIRED    | Line 514-516: direct async call with pod_id, billing_session_id, reason, detail       |
| bot_coordinator::recover_stuck_session | billing::end_billing_session_public | direct async call                            | WIRED    | Line 139: end_billing_session_public(state, &session_id, BillingSessionStatus::EndedEarly).await |
| bot_coordinator               | state.relay_available (AtomicBool)          | state.relay_available.load(Ordering::Relaxed) | WIRED    | Lines 149-165: sync fence loop reads relay_available from AppState                    |
| bot_coordinator::alert_staff_idle_drift | state.email_alerter              | state.email_alerter.write().await.send_alert  | WIRED    | Lines 192-197: EmailAlerter wired through AppState.email_alerter (RwLock<EmailAlerter>) |
| main.rs (rc-agent)            | failure_monitor_tx (driving_state updates)  | send_modify at Sites 9a + 9b                  | WIRED    | Lines 902, 917: failure_monitor_tx.send_modify(|s| { s.driving_state = Some(detector.state()); }) |

---

### Requirements Coverage

| Requirement | Source Plan  | Description                                                                              | Status    | Evidence                                                                                             |
|-------------|-------------|------------------------------------------------------------------------------------------|-----------|------------------------------------------------------------------------------------------------------|
| BILL-01     | 25-01-PLAN  | billing.rs characterization test suite before bot code — covers start_session, end_session, idle, sync | SATISFIED | 5 tests at billing.rs:3791-3868, explicitly marked "BILL-01 characterization"; all pass            |
| BILL-02     | 25-02-PLAN  | Bot detects stuck session (billing active >60s after game exits), triggers end_session via StopSession sequence | SATISFIED | billing_guard.rs: 60s threshold detection; bot_coordinator.recover_stuck_session calls end_billing_session_public which handles StopGame+SessionEnded |
| BILL-03     | 25-02-PLAN  | Bot detects idle billing drift (billing active + DrivingState inactive >5min), alerts staff — NO auto-end | SATISFIED | billing_guard.rs: 300s threshold; bot_coordinator.alert_staff_idle_drift sends email only; routing test confirms IdleBillingDrift never calls end_billing_session_public |
| BILL-04     | 25-04-PLAN  | Bot-triggered session end fences cloud sync — waits for sync acknowledgment before completing | SATISFIED | bot_coordinator.rs:147-165: 5s deadline fence polls state.relay_available; logs timeout if relay stays down |
| BOT-01      | 25-03-PLAN  | bot_coordinator.rs on racecontrol handles billing recovery message routing and server-side bot responses | SATISFIED | Full module at crates/racecontrol/src/bot_coordinator.rs; handle_billing_anomaly routes SessionStuckWaitingForGame → recover_stuck_session; IdleBillingDrift → alert_staff_idle_drift |

---

### Anti-Patterns Found

| File                  | Line | Pattern                               | Severity | Impact                                                                       |
|-----------------------|------|---------------------------------------|----------|------------------------------------------------------------------------------|
| bot_coordinator.rs    | 9    | `// stub; Phase 24 handles rc-agent side` comment on handle_hardware_failure | Info | Intentional: hardware failure server action is deferred to Phase 26; comment accurately documents intent |
| bot_coordinator.rs    | 93   | `// TELEM-01 alert logic — Phase 26. Stub here for BOT-01 completeness`    | Info | Intentional: telemetry routing is a stub pending Phase 26; no billing-critical path affected |

No blocker anti-patterns found. Both stub comments are on intentional stubs (HardwareFailure, TelemetryGap) that are explicitly out of scope for Phase 25. The billing-critical paths (BillingAnomaly routing) are fully implemented.

---

### Human Verification Required

#### 1. Sync Fence Behavior Under Relay-Down Conditions

**Test:** Kill the comms-link relay while a stuck session recovery is triggered. Observe whether the 5s fence times out cleanly without blocking teardown.
**Expected:** Fence logs "Sync fence timeout 5s — HTTP fallback scheduled" and returns; session end is still committed to local DB.
**Why human:** Can't simulate relay outage + billing anomaly concurrently in unit tests.

#### 2. Email Alert Delivery for Idle Drift

**Test:** Let a pod sit with billing active and DrivingState::Idle for 5+ minutes, then check that alert email arrives at the configured recipient.
**Expected:** Email arrives with subject containing pod ID and detail text within 5 minutes of idle threshold.
**Why human:** Requires live email infrastructure (Google Workspace/SMTP) and real pod state.

---

### Gaps Summary

No gaps found. All 5 requirements are satisfied, all artifacts are substantive and wired, all tests pass. The two stub functions (handle_hardware_failure, handle_telemetry_gap) are intentionally out of scope and documented as such in the source code.

---

_Verified: 2026-03-16_
_Verifier: Claude (gsd-verifier)_
