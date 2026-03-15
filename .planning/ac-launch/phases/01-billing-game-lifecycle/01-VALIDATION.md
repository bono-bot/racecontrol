---
phase: 01
slug: billing-game-lifecycle
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-15
---

# Phase 01 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust `cargo test` with `#[cfg(test)]` modules |
| **Config file** | none (inline in source files) |
| **Quick run command** | `cargo test -p rc-core game_launcher::tests && cargo test -p rc-agent` |
| **Full suite command** | `cargo test -p rc-common && cargo test -p rc-core && cargo test -p rc-agent` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p rc-core game_launcher::tests && cargo test -p rc-agent`
- **After every plan wave:** Run `cargo test -p rc-common && cargo test -p rc-core && cargo test -p rc-agent`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 01-01-01 | 01 | 1 | LIFE-02 | unit | `cargo test -p rc-core game_launcher::tests::test_launch_rejected_no_billing` | ❌ W0 | ⬜ pending |
| 01-01-02 | 01 | 1 | LIFE-02 | unit | `cargo test -p rc-core game_launcher::tests::test_launch_allowed_with_billing` | ❌ W0 | ⬜ pending |
| 01-01-03 | 01 | 1 | LIFE-04 | unit | `cargo test -p rc-core game_launcher::tests::test_double_launch_blocked_running` | ❌ W0 | ⬜ pending |
| 01-01-04 | 01 | 1 | LIFE-04 | unit | `cargo test -p rc-core game_launcher::tests::test_double_launch_blocked_launching` | ❌ W0 | ⬜ pending |
| 01-02-01 | 02 | 1 | LIFE-03 | build+test | `cargo build -p rc-agent 2>&1 && cargo test -p rc-agent` | ✅ | ⬜ pending |
| 01-02-02 | 02 | 1 | LIFE-01,02,03,04 | manual | Pod 8 end-to-end test | manual-only | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/rc-core/src/game_launcher.rs` — add `#[cfg(test)] mod tests` with billing gate + double-launch tests (4 test cases)
- [ ] No new test files needed — all tests inline in existing source files per project convention

*Existing infrastructure: 47 tests across 3 crates, all inline `#[cfg(test)]` modules.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Game process killed within 10s of billing end | LIFE-01 | Requires running pod with real acs.exe process | Deploy to Pod 8, start billing + AC, expire billing, verify acs.exe killed via tasklist |
| Lock screen shows summary then blank after 15s | LIFE-03 | Requires live lock screen + blank_timer event loop | Deploy to Pod 8, end billing, observe 15s transition |
| Full regression: billing start/stop/pause | ALL | Integration flow across rc-core + rc-agent | Start billing, pause, resume, stop — verify all work |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
