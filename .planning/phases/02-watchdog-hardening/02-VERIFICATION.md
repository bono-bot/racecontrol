---
phase: 02-watchdog-hardening
verified: 2026-03-13T00:00:00Z
status: passed
score: 5/5 must-haves verified
re_verification: false
---

# Phase 2: Watchdog Hardening Verification Report

**Phase Goal:** pod_monitor uses escalating backoff per pod with exclusive restart ownership; pod_healer reads shared state and defers all restart commands; post-restart verification confirms process + WebSocket + lock screen before declaring recovery; email alerts fire when verification fails or backoff is exhausted
**Verified:** 2026-03-13
**Status:** PASSED
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | pod_monitor uses escalating backoff per pod with exclusive restart ownership | VERIFIED | `check_all_pods()` skips pods where WatchdogState is Restarting or Verifying (pod_monitor.rs:209-217); `is_ws_alive()` with `sender.is_closed()` replaces `contains_key` for WS liveness (pod_monitor.rs:80-86); backoff recorded via `record_attempt()` on restart (pod_monitor.rs:330) |
| 2 | pod_healer reads shared state and defers all restart commands | VERIFIED | `heal_pod()` reads `pod_watchdog_states` and returns early via `should_skip_for_watchdog_state()` for Restarting/Verifying states (pod_healer.rs:152-162); sets `needs_restart` flag in AppState instead of directly restarting (pod_healer.rs:218-221); `record_attempt()` removed from healer (pod_healer.rs:312 — comment only) |
| 3 | Post-restart verification confirms process + WebSocket + lock screen before declaring recovery | VERIFIED | `verify_restart()` polls at 5s/15s/30s/60s checking all 3 (pod_monitor.rs:523, 538-553); full recovery requires all 3 checks pass (pod_monitor.rs:555); partial recovery (process+WS ok, lock screen fail) falls through to failure path (pod_monitor.rs:596-606); check_lock_screen hits `/health` endpoint on port 18923 (pod_monitor.rs:712) |
| 4 | Email alerts fire when verification fails or backoff is exhausted | VERIFIED | verify_restart sends alert with `format_alert_body()` on 60s timeout (pod_monitor.rs:667-682); exhaustion path sends alert when `backoff.exhausted()` is true (pod_monitor.rs:359-380); both include failure_type, last_heartbeat, and next_action |
| 5 | Rate limiting: max 1 email per pod per 30min, 1 venue-wide per 5min | VERIFIED | `EmailAlerter.should_send()` enforces per-pod 1800s cooldown and venue-wide 300s cooldown (email_alerts.rs:65-87); tested via 6 unit tests all passing |

**Score:** 5/5 truths verified

---

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/racecontrol/src/state.rs` | WatchdogState enum, pod_watchdog_states, pod_needs_restart fields | VERIFIED | WatchdogState enum with 4 variants (lines 24-34); pod_watchdog_states and pod_needs_restart in AppState (lines 81-83); create_initial_watchdog_states() and create_initial_needs_restart() helpers (lines 258-274); wired in AppState::new() (lines 121-122) |
| `crates/rc-common/src/protocol.rs` | PodRestarting, PodVerifying, PodRecoveryFailed dashboard events | VERIFIED | All 3 variants present (lines 325-344); serde roundtrip tests pass (33/33 rc-common tests) |
| `crates/racecontrol/src/email_alerts.rs` | format_alert_body with failure_type, last_heartbeat, next_action | VERIFIED | Signature includes all 7 params including failure_type, last_heartbeat: Option<DateTime<Utc>>, next_action (lines 170-178); 4 dedicated tests pass for heartbeat None/Some and failure_type/next_action |
| `crates/rc-agent/src/lock_screen.rs` | /health endpoint returning HTTP 200 with JSON body | VERIFIED | GET /health handler at line 574 returns HTTP 200 always; delegates to `health_response_body()` pure function (line 576); health_response_body() at line 931; 6 unit tests pass covering ok/degraded states |
| `crates/racecontrol/src/pod_monitor.rs` | Rewritten restart lifecycle with WatchdogState management | VERIFIED | WatchdogState skip guard (lines 204-218); is_ws_alive() helper (lines 80-86); verify_restart with 3-check verification (lines 498-683); PodRestarting/PodVerifying/PodRecoveryFailed broadcasts present |
| `crates/racecontrol/src/pod_healer.rs` | WatchdogState-aware healer with needs_restart flag | VERIFIED | should_skip_for_watchdog_state() pure helper (lines 757-763); WatchdogState read in heal_pod() (lines 152-162); needs_restart flag set in Rule 2 no-WS path (lines 217-221); record_attempt() not called (line 312: comment only) |

---

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `state.rs` | `AppState::new()` | WatchdogState fields wired at construction | VERIFIED | pod_watchdog_states: RwLock::new(create_initial_watchdog_states()) at line 121; pod_needs_restart: RwLock::new(create_initial_needs_restart()) at line 122 |
| `pod_monitor.rs` | `state.rs` | pod_watchdog_states read/write at Restarting/Verifying/RecoveryFailed transitions | VERIFIED | Reads at line 206, writes at lines 342-347 (Restarting), 510-515 (Verifying), 636-641 (RecoveryFailed), 584-586 (Healthy on recovery), 157 (Healthy on natural recovery) |
| `pod_monitor.rs` | `protocol.rs` | DashboardEvent::PodRestarting broadcast | VERIFIED | `state.dashboard_tx.send(DashboardEvent::PodRestarting {...})` at line 351 |
| `pod_monitor.rs` | `protocol.rs` | DashboardEvent::PodVerifying broadcast | VERIFIED | `state.dashboard_tx.send(DashboardEvent::PodVerifying {...})` at line 518 |
| `pod_monitor.rs` | `protocol.rs` | DashboardEvent::PodRecoveryFailed broadcast | VERIFIED | `state.dashboard_tx.send(DashboardEvent::PodRecoveryFailed {...})` at line 644 |
| `pod_monitor.rs` | `email_alerts.rs` | format_alert_body with new parameters | VERIFIED | Called at lines 361-369 (exhaustion path) and 667-675 (verify_restart failure path); both pass failure_type, last_heartbeat (pod.last_seen / last_seen param), next_action |
| `pod_healer.rs` | `state.rs` | pod_watchdog_states read, pod_needs_restart write | VERIFIED | pod_watchdog_states read at line 153; pod_needs_restart write at lines 219-221 |
| `pod_healer.rs` | `pod_monitor.rs` | needs_restart flag consumed by monitor | VERIFIED | Healer sets at pod_healer.rs:220; monitor reads+clears via HashMap::remove().unwrap_or(false) at pod_monitor.rs:263-265 |
| `lock_screen.rs` | `/health endpoint` | health_response_body() called from serve_lock_screen() | VERIFIED | GET /health handler at line 574 calls health_response_body(&current) at line 576 |
| `pod_monitor.rs` | `lock_screen.rs /health` | check_lock_screen hits /health not root | VERIFIED | PowerShell command at line 712 targets `http://127.0.0.1:18923/health` |

---

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| WD-01 | 02-01, 02-02, 02-03 | Pod restart uses escalating backoff (30s→2m→10m→30m) instead of fixed cooldown | SATISFIED | EscalatingBackoff in AppState; pod_monitor calls record_attempt() and checks ready(); backoff_label() converts to 30s/2m/10m/30m |
| WD-03 | 02-01, 02-02, 02-03 | Post-restart verification confirms process running + WebSocket connected + lock screen responsive (60s window) | SATISFIED | verify_restart() polls 5s/15s/30s/60s with all 3 checks; /health endpoint on lock_screen.rs returns HTTP 200; partial recovery treated as FAILED |
| WD-04 | 02-01, 02-02 | Backoff resets to base on confirmed full recovery | SATISFIED | `backoff.reset()` called at pod_monitor.rs:578 on full recovery (all 3 checks pass); WatchdogState set to Healthy |
| ALERT-01 | 02-01, 02-02 | Email alert fires when post-restart verification fails or max escalation reached | SATISFIED | send_alert() called at pod_monitor.rs:678 (verification failure) and pod_monitor.rs:374 (exhaustion path) |
| ALERT-02 | 02-02 | Rate-limited: max 1 email per pod per 30min, 1 venue-wide per 5min | SATISFIED | EmailAlerter.should_send() enforces 1800s per-pod and 300s venue-wide cooldowns; venue_wide_rate_limit tests pass |

All 5 requirements satisfied. No orphaned requirements: REQUIREMENTS.md traceability table maps WD-01, WD-03, WD-04, ALERT-01, ALERT-02 to Phase 2 and marks them Complete.

---

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `pod_monitor.rs` | 252 | `contains_key` call | INFO | This is `billing.active_timers.contains_key(&pod.id)` — billing map lookup, NOT WS liveness. Correct usage. |
| `pod_healer.rs` | 754 | `contains_key` call | INFO | This is `billing timers.contains_key(pod_id)` inside `has_active_billing()` — billing map lookup, NOT WS liveness. Correct usage. |

No blockers. Both `contains_key` calls are for billing timer map lookups — correct and expected. WS liveness in both pod_monitor and pod_healer uses `sender.is_closed()` exclusively.

---

### Human Verification Required

None. All goal-critical behaviors are verifiable through code inspection and test results.

Items that would benefit from staging observation (not blocking):
- Email delivery end-to-end (Node.js script execution and SMTP relay) — not testable without a live pod failure
- Dashboard WebSocket event rendering in the frontend when PodRestarting/PodVerifying/PodRecoveryFailed arrive — frontend code not in scope for this phase

---

### Test Results Summary

| Suite | Tests | Result |
|-------|-------|--------|
| rc-common (all) | 33 | 33 passed, 0 failed |
| racecontrol (all) | 83 unit + 13 integration | 96 passed, 0 failed |
| rc-agent lock_screen | 6 | 6 passed, 0 failed |
| **Total** | **122** | **122 passed, 0 failed** |

Key test coverage for this phase:
- `state::tests` — 5 WatchdogState + 5 backoff tests
- `email_alerts::tests` — 11 tests including failure_type, last_heartbeat None/Some, next_action, rate limits
- `pod_monitor::tests` — 30 tests covering WatchdogState transitions, skip guards, failure reason logic, backoff_label, needs_restart flag consumption, partial recovery as failure
- `pod_healer::tests` — 9 tests covering skip/continue for all 4 WatchdogState variants and needs_restart decision tree
- `lock_screen::tests` — 6 tests covering health_response_body for ok/degraded states
- `protocol::tests` — 3 serde roundtrip tests for PodRestarting, PodVerifying, PodRecoveryFailed

---

### Gaps Summary

No gaps. All phase must-haves are verified at all three levels (exists, substantive, wired) across all 3 plans:

- **Plan 02-01**: Shared contracts all in place — WatchdogState enum (4 variants), AppState fields (pod_watchdog_states, pod_needs_restart pre-populated for pods 1-8), DashboardEvent variants (3 new watchdog events), format_alert_body signature extended (7 params), /health endpoint on lock_screen HTTP server
- **Plan 02-02**: pod_monitor fully rewritten — escalating backoff, WatchdogState skip guard, verify_restart with 3-check verification and partial-recovery-as-failure, PodRestarting/PodVerifying/PodRecoveryFailed broadcasts, email alerts on exhaustion and verification failure, is_ws_alive() using sender.is_closed()
- **Plan 02-03**: pod_healer boundary enforced — skip for Restarting/Verifying via should_skip_for_watchdog_state(), needs_restart flag set only for Rule 2 (lock screen down + no WS + no billing), record_attempt() removed from healer, WS liveness uses sender.is_closed()

---

*Verified: 2026-03-13*
*Verifier: Claude (gsd-verifier)*
