---
phase: 5
slug: blanking-screen-protocol
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-13
---

# Phase 5 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in + `#[cfg(test)]` modules |
| **Config file** | None — colocated with source modules |
| **Quick run command** | `cargo test -p rc-agent && cargo test -p rc-common` |
| **Full suite command** | `cargo test -p rc-agent && cargo test -p rc-common && cargo test -p rc-core` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p rc-agent && cargo test -p rc-common`
- **After every plan wave:** Run `cargo test -p rc-agent && cargo test -p rc-common && cargo test -p rc-core`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 5-01-01 | 01 | 1 | SCREEN-01 | unit | `cargo test -p rc-agent lock_screen` | ❌ W0 | ⬜ pending |
| 5-01-02 | 01 | 1 | SCREEN-01 | unit | `cargo test -p rc-agent lock_screen` | ❌ W0 | ⬜ pending |
| 5-02-01 | 02 | 1 | SCREEN-02 | unit | `cargo test -p rc-agent` | ❌ W0 | ⬜ pending |
| 5-02-02 | 02 | 1 | SCREEN-03 | unit | `cargo test -p rc-agent lock_screen` | ✅ partial | ⬜ pending |
| 5-03-01 | 03 | 1 | AUTH-01 | unit | `cargo test -p rc-core auth` | ❌ W0 | ⬜ pending |
| 5-03-02 | 03 | 1 | AUTH-01 | unit | `cargo test -p rc-core auth` | ❌ W0 | ⬜ pending |
| 5-03-03 | 03 | 1 | PERF-02 | unit timing | `cargo test -p rc-core auth -- --nocapture` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/rc-agent/src/lock_screen.rs` — tests for LaunchSplash state HTML rendering (no system text), is_blanked() for LaunchSplash
- [ ] `crates/rc-agent/src/lock_screen.rs` — test for transition: show_session_summary() before close_browser()
- [ ] `crates/rc-core/src/auth/mod.rs` — `#[cfg(test)] mod tests` for validate_pin_inner(): wrong PIN, employee PIN, expired token
- [ ] `crates/rc-agent/src/ac_launcher.rs` — test for extended dialog process list in enforce_safe_state()

*Existing infrastructure covers framework; Wave 0 adds test stubs for phase requirements.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| No desktop flash on session end | SCREEN-01 | Requires visual confirmation on pod display | Deploy to Pod 8, end a session, visually confirm lock screen covers before game closes |
| Anti-cheat compatibility | SCREEN-02 | Requires real game client + anti-cheat service | Launch iRacing, F1 25, LMU in sequence with rc-agent running |
| Taskbar hidden after registry change | SCREEN-01 | Requires reboot + visual check | Apply registry key on Pod 8, reboot, confirm taskbar invisible |
| Keyboard shortcuts blocked | SCREEN-01 | Requires physical interaction | Press Win, Alt+Tab, Ctrl+Esc on Pod 8 after GP/registry changes applied |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
