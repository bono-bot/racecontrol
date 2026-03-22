---
phase: 162-james-watchdog-migration
verified: 2026-03-22T22:30:00+05:30
status: passed
score: 8/8 must-haves verified
re_verification: false
human_verification:
  - test: "Confirm Task Scheduler CommsLink-DaemonWatchdog runs rc-watchdog.exe every 2 minutes on James (.27)"
    expected: "schtasks /Query /TN CommsLink-DaemonWatchdog /FO LIST shows rc-watchdog.exe as Task To Run with MINUTE/2 schedule"
    why_human: "Cannot query Windows Task Scheduler registry programmatically from this environment"
  - test: "Confirm recovery-log.jsonl contains james_monitor entries after first run"
    expected: "C:\\Users\\bono\\racingpoint\\recovery-log.jsonl has entries with machine=james and authority=james_monitor"
    why_human: "Log file is on James (.27) local disk — not in the repo"
  - test: "Confirm watchdog-state.json was created at C:\\Users\\bono\\.claude\\watchdog-state.json"
    expected: "File exists with valid JSON counts object"
    why_human: "Runtime artifact on James (.27) local disk — not in the repo"
---

# Phase 162: James Watchdog Migration — Verification Report

**Phase Goal:** Rust binary replaces james_watchdog.ps1, pattern memory, graduated response, Bono alert on repeated failures
**Verified:** 2026-03-22T22:30:00+05:30
**Status:** PASSED (with minor warning)
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | rc-watchdog binary compiles targeting James (.27) — standalone exe via Task Scheduler | VERIFIED | `cargo test -p rc-watchdog` → 29 passed; binary at `target/release/rc-watchdog.exe` (3.7MB) and `deploy-staging/rc-watchdog.exe` (3.7MB) |
| 2 | Each service (Ollama :11434, comms-link :8766, kiosk :3300, webterm :9999, Claude Code process) is checked via HTTP GET or process name | VERIFIED | `james_monitor.rs` lines 37-70: all 5 `ServiceConfig` entries confirmed with correct URLs and check types |
| 3 | Failure counts persist between Task Scheduler runs in watchdog-state.json | VERIFIED | `failure_state.rs`: atomic tmp+rename write, `FailureState::load()` returns default on missing/corrupt; roundtrip test confirmed |
| 4 | First failure: log warn and write updated failure count — no restart attempted | VERIFIED | `graduated_action(1)` returns `(Restart, "first_failure_wait_retry")`; `attempt_restart` only called when `action==Restart && count==2` (james_monitor.rs:199) |
| 5 | Second failure: restart of that service attempted | VERIFIED | `attempt_restart(&svc)` called in run_monitor when `action == RecoveryAction::Restart && count == 2` (james_monitor.rs:199-201) |
| 6 | Third+ failure: Bono alerted via node send-message.js with service name and failure count | VERIFIED | `bono_alert::alert_bono()` called with `[WATCHDOG] {service} DOWN on James (failure #{n})` when `matches!(action, RecoveryAction::AlertStaff)` (james_monitor.rs:213-221) |
| 7 | Every decision logged to RECOVERY_LOG_JAMES via RecoveryLogger with RecoveryAuthority::JamesMonitor | VERIFIED | `RecoveryLogger::new(RECOVERY_LOG_JAMES)` in `run_monitor()` (james_monitor.rs:162); `logger.log(&d)` called for every failure and recovery path |
| 8 | Recovery resets count to 0 and OK is logged | VERIFIED | `state.reset(svc.name)` after logging recovered decision (james_monitor.rs:183); recovery decision uses `RecoveryAction::Restart` with `reason="recovered"` |

**Score:** 8/8 truths verified

---

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/rc-watchdog/src/failure_state.rs` | Persistent failure-count state across invocations | VERIFIED | 137 lines; `FailureState::load/save/increment/reset/count` all implemented; 6 unit tests |
| `crates/rc-watchdog/src/bono_alert.rs` | Bono alert via node send-message.js spawn | VERIFIED | 64 lines; `alert_bono_with_exe` extracted for testability; returns `Ok(())` on missing node; COMMS_PSK/COMMS_URL set; 2 unit tests |
| `crates/rc-watchdog/src/james_monitor.rs` | Core monitor logic — ServiceConfig list, check_service(), run_monitor() | VERIFIED | 297 lines; 5 services defined; `graduated_action` pub(crate) fn; full `run_monitor()` loop; 7 unit tests |
| `crates/rc-watchdog/src/main.rs` | Entry point — branches on --service vs james_monitor mode | VERIFIED | 62 lines; `--service` → Windows service dispatcher; no args → `james_monitor::run_monitor()` |
| `scripts/register-james-watchdog.bat` | One-shot registration script | VERIFIED* | CRLF line endings confirmed; correct schtasks/reg commands; goto-label error handling. *See warning below |
| `deploy-staging/rc-watchdog.exe` | Deployed binary at staging path | VERIFIED | 3.7MB binary present; matches `target/release/rc-watchdog.exe` (same size/date) |

---

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `main.rs` | `james_monitor::run_monitor()` | call when no --service arg | WIRED | Line 52: `james_monitor::run_monitor();` in else branch |
| `james_monitor.rs` | `failure_state::FailureState` | `FailureState::load()` and `.save()` on each run | WIRED | Lines 163, 224: load at start, save after all service checks |
| `james_monitor.rs` | `rc_common::recovery::RecoveryLogger` | `RecoveryLogger::new(RECOVERY_LOG_JAMES)` | WIRED | Line 162: logger created with `RECOVERY_LOG_JAMES` const |
| `james_monitor.rs` | `bono_alert::alert_bono()` | called when failure_count >= 3 | WIRED | Line 218: `alert_bono(&alert_msg)` called inside `if matches!(action, RecoveryAction::AlertStaff)` |
| `rc-watchdog` binary | `rc-common::recovery` types | workspace dependency | WIRED | `Cargo.toml` line 13: `rc-common = { workspace = true }` |
| `register-james-watchdog.bat` | `deploy-staging/rc-watchdog.exe` | Task Scheduler registration | WIRED | Script references `C:\Users\bono\racingpoint\deploy-staging\rc-watchdog.exe`; human checkpoint confirmed task registered |

---

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| JWAT-01 | 162-01, 162-02 | Replace james_watchdog.ps1 with a Rust-based monitor using AI debugger pattern memory | SATISFIED | rc-watchdog.exe deployed to deploy-staging; Task Scheduler task registered (human-verified checkpoint in 162-02); pattern memory = FailureState persistent JSON |
| JWAT-02 | 162-01 | James monitor checks Ollama, Claude Code, comms-link, webterm with graduated response (not blind restart) | SATISFIED | All 4 services (+ kiosk) checked via HTTP/process; graduated_action FSM: count=1 wait, count=2 restart, count=3+ alert; no blind restart loop |
| JWAT-03 | 162-01 | James monitor alerts Bono via comms-link WS on repeated failures instead of silent restart | SATISFIED | `alert_bono()` sends via `node send-message.js` with COMMS_PSK/COMMS_URL; triggered at count>=3 via AlertStaff action |

---

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `scripts/register-james-watchdog.bat` | 11 | `2>/dev/null` instead of `2>nul` | WARNING | In cmd.exe, `2>/dev/null` redirects to a file literally named `/dev/null` rather than suppressing output. This only affects the silent deletion step (`schtasks /Delete` on a non-existent task). Task registration, Run key, and immediate run steps are unaffected. The human checkpoint confirms the script ran successfully — so this did not block deployment. |

No stub patterns detected. No TODO/FIXME/placeholder comments in any source file. No empty return implementations.

---

### Commit Verification

All claimed commits verified in git log:

| Commit | Description |
|--------|-------------|
| `ca187c3c` | feat(162-01): add failure_state.rs and bono_alert.rs |
| `3d67f8b5` | feat(162-01): implement james_monitor.rs with graduated response + main.rs wiring |
| `b7cad99c` | feat(162-02): add registration script + deploy rc-watchdog.exe to staging |

---

### Human Verification Required

These items were confirmed via human checkpoint in plan 162-02 but cannot be programmatically re-verified from this environment:

#### 1. Task Scheduler Registration

**Test:** `schtasks /Query /TN CommsLink-DaemonWatchdog /FO LIST` on James (.27)
**Expected:** Task To Run shows `C:\Users\bono\racingpoint\deploy-staging\rc-watchdog.exe`; Schedule Type is MINUTE with Interval 2
**Why human:** Task Scheduler registry is on James (.27) Windows machine — not accessible from this verifier

#### 2. Recovery Log Entries

**Test:** `type C:\Users\bono\racingpoint\recovery-log.jsonl` on James (.27)
**Expected:** JSON entries with `machine: "james"`, `authority: "james_monitor"` present after at least one run
**Why human:** Runtime log file on James (.27) local disk

#### 3. watchdog-state.json Existence

**Test:** `type C:\Users\bono\.claude\watchdog-state.json` on James (.27)
**Expected:** Valid JSON with `{"counts": {...}}` structure
**Why human:** Runtime artifact on James (.27) local disk

Note: All three were confirmed by the human checkpoint step in plan 162-02 (run confirmed successful with empty counts = all services healthy at first run).

---

### Gaps Summary

No gaps blocking goal achievement. All 8 observable truths verified against actual source code.

The `2>/dev/null` issue in `register-james-watchdog.bat` line 11 is a warning but does not block the phase goal — the task was successfully registered (human-verified checkpoint), and the redirect only affects a silent-delete step where failure is harmless.

The HKLM Run key failure at deployment time is documented and accepted — Task Scheduler via SYSTEM account provides equivalent and primary persistence for the 2-minute polling cycle.

---

_Verified: 2026-03-22T22:30:00+05:30_
_Verifier: Claude (gsd-verifier)_
