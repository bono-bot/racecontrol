---
phase: 5
slug: watchdog-hardening
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-12
---

# Phase 5 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust, built-in) |
| **Config file** | Cargo.toml workspace |
| **Quick run command** | `cargo test -p rc-common -- watchdog && cargo test -p rc-core -- pod_monitor email_alerts` |
| **Full suite command** | `cargo test --workspace` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p rc-common && cargo test -p rc-core`
- **After every plan wave:** Run `cargo test --workspace`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 05-01-01 | 01 | 1 | WD-01 | unit | `cargo test -p rc-common -- watchdog::tests` | ❌ W0 | ⬜ pending |
| 05-01-02 | 01 | 1 | WD-02 | unit | `cargo test -p rc-core -- pod_monitor::tests` | ❌ W0 | ⬜ pending |
| 05-01-03 | 01 | 1 | WD-05 | unit | `cargo test -p rc-core -- pod_monitor::tests` | ❌ W0 | ⬜ pending |
| 05-01-04 | 01 | 1 | WD-06 | unit | `cargo test -p rc-core -- config::tests` | ❌ W0 | ⬜ pending |
| 05-02-01 | 02 | 1 | WD-03 | unit | `cargo test -p rc-core -- email_alerts::tests` | ❌ W0 | ⬜ pending |
| 05-02-02 | 02 | 1 | WD-04 | unit | `cargo test -p rc-core -- email_alerts::tests` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/rc-common/src/watchdog.rs` — EscalatingBackoff struct + tests (covers WD-01)
- [ ] `crates/rc-core/src/email_alerts.rs` — EmailAlerter struct + tests (covers WD-03, WD-04)
- [ ] Test module in `pod_monitor.rs` — verification logic tests (covers WD-02, WD-05)
- [ ] Test module in `config.rs` — WatchdogConfig with new email fields (covers WD-06)

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Email arrives at Uday's inbox | WD-03 | Requires real Gmail OAuth2 + network | 1. Force crash on Pod 8 2. Wait for escalation 3. Check usingh@racingpoint.in |
| Session 0 GUI detection | WD-02 | Requires SYSTEM-context restart | 1. Stop rc-agent on Pod 8 2. Restart via pod-agent 3. Verify lock screen status |
| WebSocket reconnect after restart | WD-02 | Requires real pod + server | 1. Kill rc-agent on Pod 8 2. Let pod_monitor restart 3. Verify WS reconnect in rc-core logs |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
