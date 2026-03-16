---
phase: 8
slug: pod-lock-screen-hardening
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-14
---

# Phase 8 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in `#[cfg(test)]` modules |
| **Config file** | None — colocated with source modules |
| **Quick run command** | `cargo test -p rc-agent-crate && cargo test -p rc-common` |
| **Full suite command** | `cargo test -p rc-agent-crate && cargo test -p rc-common && cargo test -p racecontrol-crate` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p rc-agent-crate && cargo test -p rc-common`
- **After every plan wave:** Run `cargo test -p rc-agent-crate && cargo test -p rc-common && cargo test -p racecontrol-crate`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 08-xx-01 | TBD | 1 | LOCK-01 | unit | `cargo test -p rc-agent-crate lock_screen::tests::wait_for_self_ready_succeeds_when_port_open` | ❌ W0 | ⬜ pending |
| 08-xx-02 | TBD | 1 | LOCK-01 | unit | `cargo test -p rc-agent-crate lock_screen::tests::wait_for_self_ready_timeout` | ❌ W0 | ⬜ pending |
| 08-xx-03 | TBD | 1 | LOCK-02 | unit | `cargo test -p rc-agent-crate lock_screen::tests::startup_connecting_renders_branded_html` | ❌ W0 | ⬜ pending |
| 08-xx-04 | TBD | 1 | LOCK-02 | unit | `cargo test -p rc-agent-crate lock_screen::tests::startup_connecting_has_reload_script` | ❌ W0 | ⬜ pending |
| 08-xx-05 | TBD | 1 | LOCK-02 | unit | `cargo test -p rc-agent-crate lock_screen::tests::startup_connecting_is_idle_or_blanked` | ❌ W0 | ⬜ pending |
| 08-xx-06 | TBD | 1 | LOCK-03 | unit | `cargo test -p rc-agent-crate lock_screen::tests::health_degraded_for_startup_connecting` | ❌ W0 | ⬜ pending |
| 08-xx-07 | TBD | 1 | LOCK-03 | manual | `schtasks /query /TN RCAgentWatchdog` on Pod 8 | N/A | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `lock_screen::tests::wait_for_self_ready_succeeds_when_port_open` — stub for LOCK-01
- [ ] `lock_screen::tests::wait_for_self_ready_timeout` — stub for LOCK-01
- [ ] `lock_screen::tests::startup_connecting_renders_branded_html` — stub for LOCK-02
- [ ] `lock_screen::tests::startup_connecting_has_reload_script` — stub for LOCK-02
- [ ] `lock_screen::tests::startup_connecting_is_idle_or_blanked` — stub for LOCK-02
- [ ] `lock_screen::tests::health_degraded_for_startup_connecting` — stub for LOCK-03

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Pod reboot: Edge shows branded page within 10s | LOCK-01, LOCK-02 | Requires physical pod reboot | Reboot Pod 8, observe screen, time from desktop to branded page |
| rc-agent restart: auto-recovery within 30s | LOCK-03 | Requires process kill on live pod | Kill rc-agent via pod-agent, observe screen recovery |
| Watchdog fires after crash | LOCK-03 | Requires scheduled task on pod | Kill rc-agent, wait 60s, confirm auto-restart via tasklist |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
