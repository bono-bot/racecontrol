---
phase: 364
slug: session-quality-monitor
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-09
---

# Phase 364 -- Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in `#[test]` + `#[tokio::test]` |
| **Config file** | `Cargo.toml` (workspace) |
| **Quick run command** | `cargo test -p racecontrol 2>&1 \| tail -5` |
| **Full suite command** | `cargo test --workspace 2>&1 \| tail -10` |
| **Estimated runtime** | ~45s (racecontrol only), ~90s (workspace) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p racecontrol -- --test-threads=4 2>&1 | tail -5`
- **After every plan wave:** Run `cargo test --workspace 2>&1 | tail -10`
- **Before verify-work:** Full suite must be green (zero failures)
- **Max feedback latency:** ~45 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | Status |
|---------|------|------|-------------|-----------|-------------------|--------|
| 364-01-01 | 01 | 1 | GLD-D-01 | unit | `cargo test -p rc-common -- telemetry_quality_gap` | pending |
| 364-01-02 | 01 | 1 | GLD-D-03 | unit | `cargo test -p rc-common -- session_stalled` | pending |
| 364-01-03 | 01 | 2 | GLD-D-01 | unit | `cargo test -p racecontrol -- handle_telemetry_quality_gap` | pending |
| 364-01-04 | 01 | 2 | GLD-D-03 | unit | `cargo test -p racecontrol -- handle_session_stalled` | pending |
| 364-01-05 | 01 | 2 | GLD-D-01 | unit | `cargo test -p rc-agent -- quality_gap_fired` | pending |
| 364-01-06 | 01 | 2 | GLD-D-03 | unit | `cargo test -p rc-agent -- stall_warn_fired` | pending |
| 364-02-01 | 02 | 1 | GLD-D-02 | unit | `cargo test -p racecontrol -- lap_consistency` | pending |
| 364-02-02 | 02 | 1 | GLD-D-02 | unit | `cargo test -p racecontrol -- check_outlier` | pending |
| 364-02-03 | 02 | 2 | GLD-D-02 | unit | `cargo test -p racecontrol -- consistency_appends_suspect_reason` | pending |
| 364-03-01 | 03 | 1 | GLD-D-04 | rg-verify | `rg 'let _ = ws_sender' crates/racecontrol/src/ws/` | pending |
| 364-03-02 | 03 | 1 | GLD-D-05 | unit | `cargo test -p racecontrol -- ws_try_send_overflows` | pending |
| 364-03-03 | 03 | 2 | GLD-D-05 | integration | `cargo test -p racecontrol -- prometheus_exposes_overflow` | pending |

*Status: pending / green / red / flaky*

---

## Wave 0 Requirements

Phase 364 uses the existing test infrastructure. No new test framework installation needed.

New test files to create (stub stubs in Wave 0 of each plan):
- `crates/racecontrol/src/lap_consistency.rs` -- module with inline `#[cfg(test)]` tests
- `crates/rc-common/src/protocol.rs` -- new roundtrip tests for `TelemetryQualityGap` + `SessionStalled`
- `crates/rc-agent/src/failure_monitor.rs` -- new unit tests for QUALITY-01 and STALL-01

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| `suspect_reasons` updated in live session | GLD-D-01 | Requires real UDP telemetry gap | Start billing, block UDP for 1s, check `billing_sessions.suspect_reasons` via SQLite query |
| `ws_try_send_overflows_total` in Prometheus output | GLD-D-05 | Requires running server | `curl http://localhost:3200/api/v1/metrics/prometheus \| grep ws_try_send` |

---

## Validation Sign-Off

- [ ] All tasks have automated verify or manual instructions
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING module stubs
- [ ] No watch-mode flags
- [ ] Feedback latency < 45s
- [ ] `nyquist_compliant: true` set in frontmatter after executor completes Wave 0

**Approval:** pending
