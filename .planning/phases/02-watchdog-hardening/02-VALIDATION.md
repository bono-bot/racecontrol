---
phase: 2
slug: watchdog-hardening
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-13
---

# Phase 2 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (built-in Rust test runner) |
| **Config file** | none — `cargo test` discovers tests automatically |
| **Quick run command** | `export PATH="$PATH:/c/Users/bono/.cargo/bin" && cargo test -p rc-common && cargo test -p racecontrol-crate` |
| **Full suite command** | `export PATH="$PATH:/c/Users/bono/.cargo/bin" && cargo test -p rc-common && cargo test -p racecontrol-crate && cargo test -p rc-agent-crate` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p rc-common && cargo test -p racecontrol-crate`
- **After every plan wave:** Run `cargo test -p rc-common && cargo test -p racecontrol-crate && cargo test -p rc-agent-crate`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 02-xx-01 | 01 | 0 | WD-01 | unit | `cargo test -p rc-common watchdog` | ✅ (11 tests) | ⬜ pending |
| 02-xx-02 | 01 | 0 | WD-01 | unit | `cargo test -p racecontrol-crate pod_monitor` | ❌ W0 | ⬜ pending |
| 02-xx-03 | 01 | 0 | WD-03 | unit | `cargo test -p racecontrol-crate verify_restart` | ❌ W0 | ⬜ pending |
| 02-xx-04 | 01 | 0 | WD-03 | unit | `cargo test -p racecontrol-crate verify_restart_failure` | ❌ W0 | ⬜ pending |
| 02-xx-05 | 01 | 0 | WD-03 | unit | `cargo test -p racecontrol-crate verify_restart_partial` | ❌ W0 | ⬜ pending |
| 02-xx-06 | 01 | 0 | WD-04 | unit | `cargo test -p racecontrol-crate backoff_reset_on_recovery` | ❌ W0 | ⬜ pending |
| 02-xx-07 | 01 | 0 | ALERT-01 | unit | `cargo test -p racecontrol-crate alert_on_verify_fail` | ❌ W0 | ⬜ pending |
| 02-xx-08 | 01 | 0 | ALERT-01 | unit | `cargo test -p racecontrol-crate alert_on_exhaustion` | ❌ W0 | ⬜ pending |
| 02-xx-09 | 01 | 0 | ALERT-02 | unit | `cargo test -p racecontrol-crate email_alerts` | ✅ (8 tests) | ⬜ pending |
| 02-xx-10 | 01 | 0 | WD-01, WD-04 | unit | `cargo test -p racecontrol-crate watchdog_state_transitions` | ❌ W0 | ⬜ pending |
| 02-xx-11 | 01 | 0 | WD-03 | unit | `cargo test -p racecontrol-crate healer_skips_restarting` | ❌ W0 | ⬜ pending |
| 02-xx-12 | 01 | 0 | WD-01 | unit | `cargo test -p racecontrol-crate needs_restart_flag` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/racecontrol/src/pod_monitor.rs` — Add `#[cfg(test)] mod tests {}` block with helper to create mock AppState and test backoff/WatchdogState transitions
- [ ] `crates/racecontrol/src/pod_healer.rs` — Add `#[cfg(test)] mod tests {}` block with helper to verify healer skips Restarting pods
- [ ] Shared test helper: `fn make_test_app_state() -> Arc<AppState>` using `Config::default_test()` and in-memory SQLite — currently only in integration.rs, needs to be available to unit tests in each module

*Note: Integration test infra in `tests/integration.rs` already has `create_test_db()` and `run_test_migrations()`. The `make_test_app_state()` helper should use the same pattern.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Pod 8 live restart + recovery cycle | WD-01, WD-03 | Requires real pod hardware | Deploy to Pod 8, kill rc-agent, observe kiosk status + recovery + email |
| Kiosk dashboard shows watchdog states | WD-01 | Requires browser visual check | Open kiosk, crash Pod 8, verify Restarting/Verifying/RecoveryFailed labels |
| Email arrives at usingh@racingpoint.in | ALERT-01 | Requires real Gmail OAuth | Trigger verification failure on Pod 8, check inbox |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
