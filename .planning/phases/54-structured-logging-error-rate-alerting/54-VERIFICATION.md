---
phase: 54-structured-logging-error-rate-alerting
verified: 2026-03-20T14:30:00+05:30
status: passed
score: 8/8 must-haves verified
re_verification: false
---

# Phase 54: Structured Logging + Error Rate Alerting — Verification Report

**Phase Goal:** racecontrol and rc-agent write structured JSON logs to daily-rotating files so incidents can be investigated with jq; racecontrol watches its own error rate and emails James and Uday when it exceeds a configurable threshold

**Verified:** 2026-03-20T14:30:00 IST
**Status:** PASSED
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|---------|
| 1 | racecontrol writes JSON log entries to a daily-rotating file in logs/ | VERIFIED | `RollingFileAppender::builder().rotation(Rotation::DAILY).filename_prefix("racecontrol-").filename_suffix("jsonl")` in main.rs line 306-310 |
| 2 | stdout remains human-readable text (no JSON on stdout) | VERIFIED | File layer has `.json()` (main.rs line 333); stdout layer `tracing_subscriber::fmt::layer().with_target(true)` has no `.json()` call (line 330) |
| 3 | racecontrol log files older than 30 days are deleted on startup | VERIFIED | `cleanup_old_logs(log_dir)` called at main.rs line 304 — before tracing init |
| 4 | rc-agent writes JSON log entries to a daily-rotating file | VERIFIED | `RollingFileAppender::builder().rotation(Rotation::DAILY).filename_prefix("rc-agent-").filename_suffix("jsonl")` in rc-agent/main.rs lines 527-530 |
| 5 | rc-agent JSON logs include pod_id field | VERIFIED | `tracing::info_span!("rc-agent", pod_id = %pod_id_str).entered()` at rc-agent/main.rs line 553 — entered after config load |
| 6 | racecontrol sends email when error rate exceeds configurable threshold | VERIFIED | ErrorCountLayer wired into tracing registry (main.rs line 338); alerter task spawned with both recipients (lines 360-366) |
| 7 | error rate alerter has a 30-minute cooldown | VERIFIED | `cooldown_secs: config.monitoring.error_rate_cooldown_secs` (default 1800) passed to ErrorCountLayer, enforced via `last_alerted: Option<Instant>` in error_rate.rs |
| 8 | threshold and window configurable via racecontrol.toml; email goes to both james@ and usingh@ | VERIFIED | `MonitoringConfig` in config.rs with serde defaults; main.rs lines 363-364 confirm both recipients |

**Score:** 8/8 truths verified

---

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `Cargo.toml` | tracing-subscriber json feature | VERIFIED | Line 26: `tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }` |
| `crates/racecontrol/src/main.rs` | JSON file layer + daily rotation + log cleanup | VERIFIED | `cleanup_old_logs`, `RollingFileAppender::builder()`, `.json()` on file layer, `ErrorCountLayer` in registry |
| `crates/rc-agent/src/main.rs` | JSON file layer + daily rotation + pod_id span | VERIFIED | All patterns confirmed: `.json()`, `Rotation::DAILY`, `info_span!(pod_id)`, `cleanup_old_logs` |
| `crates/racecontrol/src/error_rate.rs` | ErrorCountLayer + error_rate_alerter_task + unit tests | VERIFIED | File exists, 133 lines; `ErrorCountLayer`, `error_rate_alerter_task`, `try_send`, 4 `#[test]` functions present |
| `crates/racecontrol/src/config.rs` | MonitoringConfig with threshold/window/cooldown fields | VERIFIED | `MonitoringConfig` struct at line 282; all 4 fields with serde defaults; added to `Config` struct at line 29 |
| `crates/racecontrol/src/lib.rs` | `mod error_rate` declaration | VERIFIED | Line 21: `pub mod error_rate;` |

---

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `Cargo.toml` | `crates/racecontrol/src/main.rs` | `"json"` feature enables `.json()` method | WIRED | Feature confirmed at Cargo.toml line 26; `.json()` used at main.rs line 333 |
| `error_rate.rs` ErrorCountLayer | `tokio::sync::mpsc` | `try_send` in `on_event` (sync bridge) | WIRED | `self.alert_tx.try_send(()).is_ok()` at error_rate.rs line 85 |
| `error_rate.rs` alerter task | `email_alerts::EmailAlerter::send_alert` | `error_rate_alerter_task` receives from mpsc and calls `send_alert` | WIRED | `alerter.send_alert("server", subject, &body).await` at error_rate.rs line 126 |
| `crates/racecontrol/src/main.rs` | `error_rate.rs` | `ErrorCountLayer` added to tracing registry + alerter task spawned | WIRED | `use racecontrol_crate::error_rate::{ErrorCountLayer, ...}` at line 13; `.with(error_count_layer)` at line 338; `tokio::spawn(error_rate_alerter_task(...))` at line 366 |
| `crates/rc-agent/src/main.rs` | `config.pod.number` | `info_span!` with pod_id entered after config load | WIRED | Config loaded at line 510; tracing init at line 527; span entered at line 553 — ordering confirmed |

---

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|---------|
| MON-01 | 54-01 | racecontrol emits structured JSON logs via tracing-subscriber with daily file rotation | SATISFIED | `.json()` file layer + `Rotation::DAILY` + `racecontrol-YYYY-MM-DD.jsonl` naming in racecontrol/main.rs |
| MON-02 | 54-02 | rc-agent emits structured JSON logs via tracing-subscriber with daily file rotation | SATISFIED | Same pattern in rc-agent/main.rs; pod_id injected via `info_span!`; tracing deferred after config load |
| MON-03 | 54-03 | racecontrol triggers email alert when error rate exceeds N errors in M minutes (configurable threshold) | SATISFIED | `ErrorCountLayer` + `MonitoringConfig` + `error_rate_alerter_task` fully wired; both recipients configured |

All 3 requirements marked `[x]` in REQUIREMENTS.md — consistent with verification findings.

No orphaned requirements found for Phase 54. MON-04 through MON-07 map to later phases (55+) — not in scope.

---

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `crates/racecontrol/src/error_rate.rs` | 147-153 | `test_error_rate_below_threshold` tests empty state only — does not actually emit 4 errors via the layer | Info | Test coverage gap for below-threshold path, but other 3 tests use full layer + subscriber chain. Not a blocker. |

No TODO/FIXME/placeholder comments found in phase-modified files. No stub return values. No empty handlers. All key wiring is live code.

---

### Human Verification Required

#### 1. Email delivery on threshold breach

**Test:** Deploy racecontrol with `[monitoring] error_rate_email_enabled = true` and `error_rate_threshold = 3` in racecontrol.toml. Force 3 `tracing::error!` calls (e.g., via a test endpoint or by deliberately triggering an error condition). Wait up to 2 minutes.
**Expected:** Email received at james@racingpoint.in and usingh@racingpoint.in with subject "RaceControl: High Error Rate Alert" and body referencing `logs/racecontrol-*.jsonl`.
**Why human:** Email delivery depends on the `email_script_path` script and Google Workspace SMTP connectivity — cannot be verified statically.

#### 2. JSONL file format validity

**Test:** Run racecontrol on the server (.23). Let it log a few events, then run `jq . logs/racecontrol-$(date +%Y-%m-%d).jsonl` on the server.
**Expected:** Each line is a valid JSON object with `timestamp`, `level`, `target`, `fields.message` keys.
**Why human:** File creation only happens at runtime on the server; cannot verify format without actually running the binary.

#### 3. pod_id field in rc-agent JSONL

**Test:** After a pod deploy, run `jq '.span.pod_id // .fields.pod_id' rc-agent-$(date +%Y-%m-%d).jsonl | head -5` on a pod.
**Expected:** Returns `"pod_N"` (matching the pod number from config) for every log line.
**Why human:** Span field serialization format depends on runtime tracing-subscriber JSON output — field path (`span.pod_id` vs nested) can only be confirmed with actual output.

---

### Gaps Summary

No gaps. All 8 observable truths verified, all 3 requirements satisfied, all artifacts substantive and wired. Three items flagged for human confirmation but these are runtime/delivery checks, not code completeness issues.

---

_Verified: 2026-03-20T14:30:00 IST_
_Verifier: Claude (gsd-verifier)_
