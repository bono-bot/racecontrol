---
phase: 1
slug: state-wiring-config-hardening
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-13
---

# Phase 1 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in test runner (`cargo test`) |
| **Config file** | `.cargo/config.toml` (workspace-level, for CRT static linking) |
| **Quick run command** | `export PATH="$PATH:/c/Users/bono/.cargo/bin" && cargo test -p rc-common && cargo test -p rc-agent-crate && cargo test -p racecontrol-crate` |
| **Full suite command** | Same as quick + `cd /c/Users/bono/racingpoint/pod-agent && cargo test` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `export PATH="$PATH:/c/Users/bono/.cargo/bin" && cargo test -p rc-common && cargo test -p rc-agent-crate && cargo test -p racecontrol-crate`
- **After every plan wave:** Run full suite + deploy to Pod 8 + verify pod reports correct pod_number
- **Before `/gsd:verify-work`:** Full suite must be green + Pod 8 smoke verify
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 01-01-01 | 01 | 0 | WD-02 | unit | `cargo test -p racecontrol-crate -- state::tests` | ❌ W0 | ⬜ pending |
| 01-01-02 | 01 | 0 | WD-02 | unit | `cargo test -p racecontrol-crate -- pod_monitor::tests` | ❌ W0 | ⬜ pending |
| 01-02-01 | 02 | 0 | DEPLOY-01 | unit | `cargo test -p rc-agent-crate -- validate_config` | ❌ W0 | ⬜ pending |
| 01-02-02 | 02 | 0 | DEPLOY-01 | unit | `cargo test -p rc-agent-crate -- load_config_no_file` | ❌ W0 | ⬜ pending |
| 01-03-01 | 03 | 0 | DEPLOY-03 | unit | `cd /c/Users/bono/racingpoint/pod-agent && cargo test` | ❌ W0 | ⬜ pending |
| 01-04-01 | 04 | 1 | DEPLOY-04 | manual | Deploy to Pod 8, verify no stale config | N/A | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/racecontrol/src/state.rs` — add `#[cfg(test)] mod tests` block for AppState::new() backoff pre-population (WD-02)
- [ ] `crates/rc-agent/src/main.rs` — add `#[cfg(test)] mod tests` block with validate_config unit tests (DEPLOY-01)
- [ ] `pod-agent/src/main.rs` (separate workspace at `/c/Users/bono/racingpoint/pod-agent/`) — add unit tests for exec_command response codes (DEPLOY-03)

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Remote deploy overwrites stale config | DEPLOY-04 | Requires physical pod + deploy-staging HTTP server | Deploy to Pod 8 via pod-agent, verify no stale racecontrol.toml remains |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
