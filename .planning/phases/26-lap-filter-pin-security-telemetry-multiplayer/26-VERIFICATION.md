---
phase: 26-lap-filter-pin-security-telemetry-multiplayer
verified: 2026-03-16T14:00:00Z
status: passed
score: 7/7 must-haves verified
re_verification:
  previous_status: passed
  previous_score: 7/7
  gaps_closed: []
  gaps_remaining: []
  regressions: []
---

# Phase 26: Lap Filter, PIN Security, Telemetry & Multiplayer Verification Report

**Phase Goal:** Implement bot-side enforcement of lap validity, per-track minimum lap time floors, PIN lockout security, telemetry gap email alerting, and multiplayer disconnect safe teardown — completing the RC Bot Expansion v5.0 feature set.
**Verified:** 2026-03-16T14:00:00Z
**Status:** passed
**Re-verification:** Yes — independent source-level confirmation of all 7 requirements and all test results. This run read every artifact directly and executed cargo test for all three crates.

## Goal Achievement

### Observable Truths

| #   | Truth                                                                                          | Status     | Evidence                                                                                                                                                                                                  |
| --- | ---------------------------------------------------------------------------------------------- | ---------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | LAP-01: invalid laps (valid=false) never reach the leaderboard                                 | VERIFIED   | `lap_tracker.rs` line 29: `if lap.lap_time_ms == 0 \|\| !lap.valid { return false; }` — no INSERT path. All leaderboard SQL in `routes.rs` filters `WHERE valid = 1` (confirmed at lines 7839, 8344, 8357, 8424, 8509, 8512, 8528, 8572, 8575, 8742). |
| 2   | LAP-02: per-track minimum lap time floors flag suspicious laps with review_required=1          | VERIFIED   | `catalog.rs` lines 34-36: spa=`Some(120_000)`, monza=`Some(80_000)`, ks_silverstone=`Some(90_000)`. `get_min_lap_time_ms_for_track()` at line 79. `lap_tracker.rs` lines 106-117: calls catalog function, issues `UPDATE laps SET review_required = 1` when lap below floor. |
| 3   | LAP-03: LapData.session_type populated by both AC and F1 25 adapters                          | VERIFIED   | `types.rs` line 222: `pub session_type: SessionType` (non-optional). `assetto_corsa.rs` line 309: `session_type: rc_common::types::SessionType::Practice` (AC always returns Practice). `f1_25.rs` lines 292-297: match on `self.session_type` byte maps 1-4=Practice, 5-8=Qualifying, 9-11=Race, 12=Hotlap; line 314: wired into LapData construction. |
| 4   | PIN-01: customer and staff PIN failure counters are separate HashMaps                          | VERIFIED   | `state.rs` lines 108, 111: two distinct `RwLock<HashMap<String, u32>>` fields (`customer_pin_failures`, `staff_pin_failures`); initialized lines 177-178 in `AppState::new`. `auth/mod.rs`: customer path writes line 435; staff path writes line 1465. |
| 5   | PIN-02: staff PIN never locked out by customer failures                                        | VERIFIED   | `auth/mod.rs` line 1455: explicit `PIN-02 invariant` comment. `validate_employee_pin` reads/writes only `staff_pin_failures`. No lockout ceiling on staff counter. `CUSTOMER_PIN_LOCKOUT_THRESHOLD=5` (line 24) applies exclusively to the customer path (line 404). |
| 6   | TELEM-01: 60s UDP silence during active billing + game Running sends staff email               | VERIFIED   | `failure_monitor.rs` line 31: `const TELEM_GAP_SECS: u64 = 60`. Line 94: `telem_gap_fired` task-local flag. Lines 145-158: detection + `try_send(TelemetryGap)`. `ws/mod.rs` lines 511-512: routes TelemetryGap to `handle_telemetry_gap`. `bot_coordinator.rs` lines 101-153: `GameState::Running` guard + billing guard before `email_alerter.send_alert`. |
| 7   | MULTI-01: multiplayer disconnect triggers BlankScreen, end_billing, log in order              | VERIFIED   | `bot_coordinator.rs` `handle_multiplayer_failure`: BlankScreen line 198 (step 1), `end_billing_session_public` with `EndedEarly` lines 207-212 (step 2), group cascade DB query via `group_session_members` lines 223-280 (step 3), `log_pod_activity` lines 283-293 (step 4). `ws/mod.rs` lines 520-526: MultiplayerFailure arm calls handler. |

**Score:** 7/7 truths verified

---

## Required Artifacts

| Artifact                                                       | Expected                                                     | Status   | Details                                                                                                                                        |
| -------------------------------------------------------------- | ------------------------------------------------------------ | -------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| `crates/rc-common/src/types.rs`                                | LapData.session_type field + SessionType enum                | VERIFIED | `LapData` struct line 222: `pub session_type: SessionType`. SessionType enum lines 126-131: Practice, Qualifying, Race, Hotlap. Non-optional field forces all construction sites to set it. |
| `crates/racecontrol/src/catalog.rs`                            | TrackEntry.min_lap_time_ms + get_min_lap_time_ms_for_track() | VERIFIED | `TrackEntry.min_lap_time_ms: Option<u32>` lines 17-20. spa=`Some(120_000)`, monza=`Some(80_000)`, ks_silverstone=`Some(90_000)`. `get_min_lap_time_ms_for_track` pub fn lines 79-84. |
| `crates/racecontrol/src/lap_tracker.rs`                        | Invalid lap gate + session_type binding + review_required    | VERIFIED | Line 29: early return on `!lap.valid`. Line 78: INSERT includes `session_type` column. Line 96: `.bind(format!("{:?}", lap.session_type)...)`. Lines 106-117: `get_min_lap_time_ms_for_track` call gating review_required UPDATE. |
| `crates/rc-agent/src/sims/assetto_corsa.rs`                    | session_type wired in LapData construction                   | VERIFIED | Line 309: `session_type: rc_common::types::SessionType::Practice` — AC has no session type API, Practice is correct and intentional. |
| `crates/rc-agent/src/sims/f1_25.rs`                            | session_type wired with byte-to-enum mapping                 | VERIFIED | Lines 292-297: match on `self.session_type` byte (from packet data offset 6). Line 314: `session_type: lap_session_type` wired into LapData construction. |
| `crates/racecontrol/src/state.rs`                              | customer_pin_failures + staff_pin_failures fields            | VERIFIED | Lines 108-111: two distinct `RwLock<HashMap<String, u32>>` fields. Lines 177-178: initialized to empty HashMaps in `AppState::new`. |
| `crates/racecontrol/src/auth/mod.rs`                           | Separate counter logic + lockout at 5 (customer only)        | VERIFIED | `CUSTOMER_PIN_LOCKOUT_THRESHOLD=5` line 24. Customer: lockout check line 402-407, increment line 435, reset line 555. Staff: increment line 1465, reset line 1472. PIN-02 invariant comment line 1455. |
| `crates/rc-agent/src/failure_monitor.rs`                       | TELEM_GAP_SECS=60, telem_gap_fired flag, TelemetryGap send  | VERIFIED | Line 31: `const TELEM_GAP_SECS: u64 = 60`. Line 94: `let mut telem_gap_fired = false`. Lines 145-158: detection + send. Lines 162-168: flag reset when data resumes or billing stops. |
| `crates/racecontrol/src/bot_coordinator.rs`                    | handle_telemetry_gap + handle_multiplayer_failure            | VERIFIED | `handle_telemetry_gap` lines 96-153: GameState::Running guard + billing guard + email send. `handle_multiplayer_failure` lines 163-294: BlankScreen → end_billing → group cascade via DB → log event. |
| `crates/racecontrol/src/ws/mod.rs`                             | TelemetryGap and MultiplayerFailure arms wired to handlers   | VERIFIED | Lines 511-513: TelemetryGap arm calls `handle_telemetry_gap`. Lines 520-526: MultiplayerFailure arm calls `handle_multiplayer_failure` with `session_id.as_deref()`. |

---

## Key Link Verification

| From                                       | To                                          | Via                                  | Status | Details                                                                                 |
| ------------------------------------------ | ------------------------------------------- | ------------------------------------ | ------ | --------------------------------------------------------------------------------------- |
| `failure_monitor.rs`                        | `AgentMessage::TelemetryGap`               | `agent_msg_tx.try_send(msg)`         | WIRED  | Lines 153-158: constructs TelemetryGap with pod_id, sim_type, gap_seconds; sends via try_send |
| `ws/mod.rs` TelemetryGap arm               | `bot_coordinator::handle_telemetry_gap`    | direct async call                    | WIRED  | Lines 511-512: matches TelemetryGap arm, calls handle_telemetry_gap with &state, &pod_id, gap_seconds as u64 |
| `bot_coordinator::handle_telemetry_gap`    | `state.email_alerter.send_alert`           | `email_alerter.write().await`        | WIRED  | Lines 147-152: send_alert called after both GameState::Running and billing guards pass  |
| `ws/mod.rs` MultiplayerFailure arm         | `bot_coordinator::handle_multiplayer_failure` | direct async call                 | WIRED  | Lines 520-526: matches MultiplayerFailure, passes &state, &pod_id, &reason, session_id.as_deref() |
| `handle_multiplayer_failure`               | `CoreToAgentMessage::BlankScreen`          | `agent_senders.get(pod_id).send`     | WIRED  | Lines 196-200: BlankScreen sent to triggering pod before billing end                   |
| `handle_multiplayer_failure`               | `end_billing_session_public`               | direct async call                    | WIRED  | Lines 207-212: called with EndedEarly after BlankScreen; sends StopGame which zeroes FFB on agent |
| `persist_lap`                              | `catalog::get_min_lap_time_ms_for_track`   | function call                        | WIRED  | Line 106: called with `&lap.track`; result gates `UPDATE laps SET review_required = 1 WHERE id = ?` |
| `persist_lap`                              | laps DB table (session_type column)        | INSERT bind                          | WIRED  | Line 96: `.bind(format!("{:?}", lap.session_type).to_lowercase())` — maps enum variant to string |

---

## Requirements Coverage

| Requirement | Source Plan | Description                                                                    | Status    | Evidence                                                                                                                                              |
| ----------- | ----------- | ------------------------------------------------------------------------------ | --------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| LAP-01      | 26-02-PLAN  | valid=false laps excluded from leaderboard (never inserted into DB)            | SATISFIED | `persist_lap` line 29: early return without INSERT when `!lap.valid`. All leaderboard SQL filters `WHERE valid = 1` (9+ query sites confirmed in routes.rs). Unit test `lap_invalid_flag_prevents_persist` passes. |
| LAP-02      | 26-02-PLAN  | Per-track minimum lap time in catalog; laps below floor get review_required=1  | SATISFIED | catalog.rs floors for spa/monza/silverstone confirmed. persist_lap lines 106-117 set review_required via UPDATE. Tests `lap_review_required_below_min_floor` and `lap_not_flagged_above_min_floor` pass. |
| LAP-03      | 26-02-PLAN  | LapData session_type field populated by both AC and F1 25 adapters             | SATISFIED | types.rs LapData line 222 has `pub session_type: SessionType` (non-optional). AC sets Practice (line 309). F1 25 maps 4 enum variants from packet byte (lines 292-314). Test `lap_data_carries_session_type` passes. |
| PIN-01      | 26-03-PLAN  | Customer and staff PIN failure counters tracked separately                     | SATISFIED | Two distinct `RwLock<HashMap<String,u32>>` in AppState (state.rs 108/111). Separate write paths in auth/mod.rs (435 for customer, 1465 for staff). Tests `customer_and_staff_counters_are_separate` and `customer_failures_do_not_affect_staff_counter` pass. |
| PIN-02      | 26-03-PLAN  | Staff PIN never locked out by customer failures                                | SATISFIED | `validate_employee_pin` only touches `staff_pin_failures` (explicit PIN-02 invariant comment line 1455). No lockout ceiling on staff counter. Test `staff_pin_succeeds_when_customer_counter_maxed` passes. |
| TELEM-01    | 26-04-PLAN  | 60s UDP silence during active billing + game Running triggers staff email       | SATISFIED | TELEM_GAP_SECS=60 in failure_monitor.rs. telem_gap_fired prevents duplicate sends. bot_coordinator guards on GameState::Running + billing_active before emailing. 7 unit tests in failure_monitor all pass. 2 bot_coordinator unit tests pass. |
| MULTI-01    | 26-04-PLAN  | AC multiplayer disconnect triggers BlankScreen, end_billing, log in order      | SATISFIED | `handle_multiplayer_failure` in bot_coordinator.rs: BlankScreen (step 1), end_billing_session_public with EndedEarly (step 2), group cascade via group_session_members DB query (step 3), log_pod_activity (step 4). ws/mod.rs MultiplayerFailure arm wired. Tests `multiplayer_failure_triggers_lock_end_billing_log_in_order` and `multiplayer_failure_noop_when_billing_inactive` pass. |

No orphaned requirements: all 7 IDs (LAP-01, LAP-02, LAP-03, PIN-01, PIN-02, TELEM-01, MULTI-01) appear in plans, are implemented, and are marked Complete in REQUIREMENTS.md traceability table. REQUIREMENTS.md contains exactly 7 Phase 26 entries — no extras.

---

## Anti-Patterns Found

| File                   | Line | Pattern                                                                              | Severity | Impact                                                                                                                         |
| ---------------------- | ---- | ------------------------------------------------------------------------------------ | -------- | ------------------------------------------------------------------------------------------------------------------------------ |
| `failure_monitor.rs`   | 155  | `sim_type: SimType::AssettoCorsa, // TODO: read from state.sim_type when available`  | Info     | SimType in TelemetryGap is hardcoded. Staff email alert fires correctly regardless — only the sim label in the alert is wrong for non-AC sims. No runtime impact. |
| `bot_coordinator.rs`   | —    | Inline step comment says "Engage lock screen" but doc comment step list says step 1 is lock / step 3 is cascade. Plan describes group cascade as step 3 which matches the implementation. Stale detail in doc comment. | Info | No runtime impact. |

No blocker or warning-level anti-patterns found.

---

## Test Results (Executed This Verification Run)

All three crates tested against actual source:

**rc-common:** 112 tests — all pass. Includes `test_telemetry_gap_roundtrip` and `test_multiplayer_failure_roundtrip` confirming protocol structs exist.

**rc-agent-crate (failure_monitor):** 14 tests — all pass:
- `telem_gap_fires_when_billing_active_game_pid_and_60s_silence` (TELEM-01)
- `telem_gap_does_not_fire_when_billing_inactive` (TELEM-01)
- `telem_gap_does_not_fire_when_game_pid_none` (TELEM-01)
- `telem_gap_does_not_fire_below_60s_threshold` (TELEM-01)
- `telem_gap_fired_flag_prevents_duplicate_sends` (TELEM-01)
- `telem_gap_flag_resets_when_udp_resumes` (TELEM-01)
- Plus 8 existing CRASH/USB/launch-timeout tests — no regressions

**racecontrol-crate (targeted):** 32 tests — all pass:
- `lap_tracker::lap_invalid_flag_prevents_persist` (LAP-01)
- `lap_tracker::lap_review_required_below_min_floor` (LAP-02)
- `lap_tracker::lap_not_flagged_above_min_floor` (LAP-02)
- `lap_tracker::lap_data_carries_session_type` (LAP-03)
- `auth::customer_and_staff_counters_are_separate` (PIN-01)
- `auth::customer_failures_do_not_affect_staff_counter` (PIN-01)
- `auth::staff_pin_succeeds_when_customer_counter_maxed` (PIN-02)
- `bot_coordinator::telemetry_gap_skipped_when_game_not_running` (TELEM-01)
- `bot_coordinator::telemetry_gap_alerts_when_game_running_and_billing_active` (TELEM-01)
- `bot_coordinator::multiplayer_failure_triggers_lock_end_billing_log_in_order` (MULTI-01)
- `bot_coordinator::multiplayer_failure_noop_when_billing_inactive` (MULTI-01)
- Plus 21 existing auth/bot_coordinator tests — no regressions

---

## Human Verification Required

### 1. TELEM-01 End-to-End Alert Delivery

**Test:** Run a billing session on a pod with the game in Running state. Block UDP traffic for 65+ seconds (e.g., kill acs.exe without triggering other detectors or recovery paths).
**Expected:** Staff email arrives within the next 5-second poll cycle after the 60-second threshold.
**Why human:** Email delivery through the node alert script requires a live SMTP relay. Cannot verify delivery programmatically.

### 2. MULTI-01 Lock Screen Sequencing

**Test:** Start a multiplayer AC session, disconnect the AC server mid-session, observe the pod display.
**Expected:** Pod goes blank (lock screen via BlankScreen command) before the billing receipt fires. The sequence BlankScreen → StopGame (FFB zero) → SessionEnded should be visible in logs in that order.
**Why human:** BlankScreen command travels over WebSocket to rc-agent — confirming the visual blank occurs before billing debit requires live observation or log correlation.

---

## Gaps Summary

No gaps. All 7 requirements are satisfied with substantive, wired implementations verified directly against source code in this run. Cargo tests were executed and confirm all 7 requirement-specific test cases pass. No regressions in any of the three crates.

---

_Verified: 2026-03-16T14:00:00Z_
_Verifier: Claude (gsd-verifier) — re-verification, all artifacts read from source + cargo test executed_
