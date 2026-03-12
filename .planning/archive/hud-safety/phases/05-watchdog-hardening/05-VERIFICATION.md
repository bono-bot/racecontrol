---
phase: 05-watchdog-hardening
verified: 2026-03-12T05:30:00Z
status: passed
score: 12/12 must-haves verified
re_verification: false
gaps: []
human_verification: []
---

# Phase 5: Watchdog Hardening Verification Report

**Phase Goal:** Harden the pod supervision stack with escalating restart cooldowns (30s->2m->10m->30m) to prevent crash loops, post-restart self-tests that verify rc-agent health (WebSocket connected, lock screen responding) before declaring recovery successful, and email notifications to alert Uday when pods have persistent issues requiring manual intervention.

**Verified:** 2026-03-12T05:30:00Z
**Status:** PASSED
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | EscalatingBackoff produces correct cooldown durations: 30s, 2m, 10m, 30m (capped) | VERIFIED | `watchdog.rs` lines 43-46; 14 unit tests all pass |
| 2 | EscalatingBackoff resets to 30s after successful recovery | VERIFIED | `reset()` at line 68-71; `pod_monitor.rs` line 97 calls `backoff.reset()`; test `reset_clears_state` passes |
| 3 | EmailAlerter sends email via send_email.js shell-out | VERIFIED | `email_alerts.rs` lines 117-127: `tokio::process::Command::new("node")` with 15s timeout and `kill_on_drop(true)` |
| 4 | EmailAlerter rate-limits to 1 email per pod per 30 minutes | VERIFIED | `should_send()` lines 71-76; tests `should_send_returns_false_within_per_pod_cooldown` + `should_send_returns_true_after_per_pod_cooldown_expires` pass |
| 5 | EmailAlerter rate-limits to 1 venue-wide email per 5 minutes | VERIFIED | `should_send()` lines 78-84; tests `venue_wide_rate_limit_blocks_within_5_minutes` + `venue_wide_rate_limit_allows_after_5_minutes` pass |
| 6 | WatchdogConfig accepts email_recipient, email_enabled, email_script_path, escalation_steps from racecontrol.toml | VERIFIED | `config.rs` lines 213-235: all 6 fields present with serde defaults; 2 deserialization tests pass |
| 7 | AppState holds shared backoff state and email alerter accessible by both pod_monitor and pod_healer | VERIFIED | `state.rs` lines 60-62: `pod_backoffs: RwLock<HashMap<String, EscalatingBackoff>>` + `email_alerter: RwLock<EmailAlerter>` |
| 8 | Pod monitor uses escalating cooldowns (30s->2m->10m->30m) instead of fixed 120s restart cooldown | VERIFIED | `pod_monitor.rs` lines 147-166: reads from `state.pod_backoffs`, uses `backoff.ready(now)` gate |
| 9 | After restart command, pod monitor spawns a verification task checking process + WebSocket + lock screen at 5s, 15s, 30s, 60s | VERIFIED | `pod_monitor.rs` lines 278-280: `tokio::spawn` of `verify_restart()`; lines 382-447: checks at `[5u64, 15, 30, 60]` |
| 10 | Post-restart verification failure after 60s triggers email alert to Uday | VERIFIED | `pod_monitor.rs` lines 460-481: reads backoff state, calls `state.email_alerter.write().await.send_alert()` |
| 11 | Pod healer reads shared backoff state from AppState instead of its own fixed HealCooldown | VERIFIED | `pod_healer.rs` lines 243-248: `state.pod_backoffs.read().await`; `HealCooldown` struct and `HEAL_COOLDOWN_SECS` fully removed |
| 12 | Pod healer does NOT restart rc-agent independently — defers restart to pod_monitor | VERIFIED | No `restart_rc_agent` action in `execute_heal_action()`; line 199 defers via issues list instead |

**Score:** 12/12 truths verified

---

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/rc-common/src/watchdog.rs` | EscalatingBackoff state machine | VERIFIED | 221 lines; `pub struct EscalatingBackoff` with `new()`, `with_steps()`, `current_cooldown()`, `ready()`, `record_attempt()`, `reset()`, `exhausted()`, `attempt()`; 14 unit tests |
| `crates/rc-core/src/email_alerts.rs` | Email notification module with rate limiting | VERIFIED | 312 lines; `pub struct EmailAlerter` with dual rate limiting, async `send_alert()`, `format_alert_body()`; 9 unit tests |
| `crates/rc-core/src/config.rs` | Extended WatchdogConfig with email alert fields | VERIFIED | `WatchdogConfig` has `email_enabled`, `email_recipient`, `email_script_path`, `email_pod_cooldown_secs`, `email_venue_cooldown_secs`, `escalation_steps_secs` with sane defaults |
| `crates/rc-core/src/state.rs` | Shared watchdog state in AppState | VERIFIED | `pod_backoffs: RwLock<HashMap<String, EscalatingBackoff>>` and `email_alerter: RwLock<EmailAlerter>` at lines 60-62; initialized in `new()` at lines 94-99 |
| `crates/rc-core/src/pod_monitor.rs` | Escalating backoff + post-restart verification + email alerts | VERIFIED | Uses `state.pod_backoffs`, spawns `verify_restart()`, sends emails on exhaustion and verification failure |
| `crates/rc-core/src/pod_healer.rs` | Shared backoff consumption, no independent restarts | VERIFIED | Reads `state.pod_backoffs`, no `HealCooldown` struct, no `restart_rc_agent` action, defers via issues list |

---

### Key Link Verification

| From | To | Via | Status | Details |
|------|-----|-----|--------|---------|
| `email_alerts.rs` | `send_email.js` | `tokio::process::Command::new("node")` | WIRED | Lines 119-125: `Command::new("node").arg(&self.script_path)` with args, 15s timeout, `kill_on_drop(true)` |
| `state.rs` | `crates/rc-common/src/watchdog.rs` | `rc_common::watchdog::EscalatingBackoff` import | WIRED | Line 17: `use rc_common::watchdog::EscalatingBackoff;` — field `pod_backoffs: RwLock<HashMap<String, EscalatingBackoff>>` |
| `pod_monitor.rs` | `state.rs` | `state.pod_backoffs.write().await` | WIRED | Lines 93, 147, 241, 314, 418: multiple read/write access points |
| `pod_monitor.rs` | `email_alerts.rs` | `state.email_alerter.write().await.send_alert()` | WIRED | Lines 262-265, 332-335, 477-481: three call sites |
| `pod_monitor.rs` | `state.rs` | `state.agent_senders.read().await.contains_key()` for WebSocket check | WIRED | Line 395: `state.agent_senders.read().await.contains_key(&pod_id)` in `verify_restart()` |
| `pod_healer.rs` | `state.rs` | `state.pod_backoffs.read().await` for shared cooldown check | WIRED | Line 243: `let backoffs = state.pod_backoffs.read().await` |

---

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| WD-01 | 05-01 + 05-02 | Escalating restart cooldowns: 30s -> 2m -> 10m -> 30m per pod, resets on successful recovery | SATISFIED | `EscalatingBackoff` in `watchdog.rs`; wired in `pod_monitor.rs` via `state.pod_backoffs`; reset on heartbeat recovery (line 97) |
| WD-02 | 05-02 | Post-restart self-test: verify rc-agent process running, WebSocket reconnected, lock screen responsive within 60s | SATISFIED | `verify_restart()` in `pod_monitor.rs` checks all three: `check_process_running()` (tasklist), `agent_senders.contains_key()` (WebSocket), `check_lock_screen()` (port 18923) at 5/15/30/60s |
| WD-03 | 05-01 + 05-02 | Email notification to Uday when pod hits max escalation or post-restart verification fails | SATISFIED | `email_alerts.rs` has `send_alert()`; triggered at `backoff.exhausted()` (pod_monitor line 246) and verification failure after 60s (pod_monitor line 476) |
| WD-04 | 05-01 | Email rate limiting: max 1 per pod per 30 min, max 1 venue-wide per 5 min | SATISFIED | `EmailAlerter.should_send()` enforces both `pod_cooldown_secs=1800` and `venue_cooldown_secs=300`; 6 passing unit tests |
| WD-05 | 05-02 | Shared backoff state between pod_monitor and pod_healer to prevent duplicate restart attempts | SATISFIED | `AppState.pod_backoffs` used by both: pod_monitor (6 access points), pod_healer (2 access points); `HealCooldown` removed |
| WD-06 | 05-01 | Configurable alert settings in racecontrol.toml: email recipient, enable/disable, script path, cooldown durations | SATISFIED | 6 new `WatchdogConfig` fields with serde defaults; tests verify both default and explicit TOML deserialization |

All 6 requirements for this phase are SATISFIED. REQUIREMENTS.md traceability table correctly marks all WD-01 through WD-06 as Complete.

---

### Anti-Patterns Found

None. Scan of all 6 phase files (watchdog.rs, email_alerts.rs, config.rs, state.rs, pod_monitor.rs, pod_healer.rs):
- No `TODO`, `FIXME`, `PLACEHOLDER` comments
- No empty implementations (`return null`, `return {}`, `return []`, `=> {}`)
- No `.unwrap()` in production paths (test-only usage acceptable)
- No stale `HEAL_COOLDOWN_SECS` constant or `HealCooldown` struct
- No independent `restart_rc_agent` action in pod_healer

---

### Human Verification Required

None required. All behavioral contracts are verifiable through code inspection and unit tests.

Items that are operationally observable but do not block goal verification:
- Actual email delivery requires Node.js + Google credentials at runtime — isolated behind `email_enabled = false` default, safe for production
- Lock screen check depends on rc-agent running on pod at port 18923 — verified at the code path level, not runtime

---

### Test Results

| Suite | Command | Result |
|-------|---------|--------|
| rc-common watchdog tests | `cargo test -p rc-common -- watchdog` | 14/14 passed |
| rc-core email_alerts tests | `cargo test -p rc-core -- email_alerts::tests` | 9/9 passed (includes disabled + custom cooldowns) |
| rc-core config tests | `cargo test -p rc-core -- config::tests` | 2/2 passed (defaults + explicit TOML values) |
| rc-core build | `cargo build -p rc-core` | Clean (5 pre-existing warnings, no errors) |

Total: 25 unit tests across 2 crates, all green. Build clean.

---

### Commit Verification

All 4 implementation commits confirmed in git log:
- `02ad967` — feat(05-01): add EscalatingBackoff state machine in rc-common
- `c50e67a` — feat(05-01): add EmailAlerter, expand WatchdogConfig, wire AppState
- `3448a6c` — feat(05-02): rewrite pod_monitor with escalating backoff and post-restart verification
- `1a82cd7` — feat(05-02): modify pod_healer to use shared backoff and defer restarts to pod_monitor

---

## Summary

Phase 5 goal fully achieved. All three pillars of watchdog hardening are implemented, tested, and wired:

1. **Escalating cooldowns** — `EscalatingBackoff` state machine with configurable steps [30s, 2m, 10m, 30m], capped at max, resets on recovery, shared across subsystems via `AppState.pod_backoffs`.

2. **Post-restart self-tests** — `verify_restart()` spawned as a detached async task after every restart command, checks process running (tasklist), WebSocket connected (agent_senders), and lock screen responsive (port 18923) at 5s/15s/30s/60s. Partial recovery (Session 0 known limitation) is handled gracefully without false alerts.

3. **Email notifications** — `EmailAlerter` with per-pod (30min) and venue-wide (5min) rate limiting shells out to `send_email.js` via `tokio::process::Command` with 15s timeout. Alerts trigger on max escalation exhaustion and post-restart verification failure. Pod healer also alerts when 3+ issues detected simultaneously.

Coordination is correct: pod_monitor owns all rc-agent restarts; pod_healer defers by adding to the issues list. The active billing guard prevents restarts during customer sessions.

---

_Verified: 2026-03-12T05:30:00Z_
_Verifier: Claude (gsd-verifier)_
