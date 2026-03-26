---
phase: 196
slug: game-launcher-structural-rework
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-26
---

# Phase 196 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust `cargo test` (built-in) |
| **Config file** | `crates/racecontrol/Cargo.toml` |
| **Quick run command** | `cargo test --package racecontrol -- game_launcher` |
| **Full suite command** | `cargo test --package racecontrol` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --package racecontrol -- game_launcher`
- **After every plan wave:** Run `cargo test --package racecontrol`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 196-01-01 | 01 | 1 | LAUNCH-01 | unit | `cargo test -- test_trait_dispatch` | ❌ W0 | ⬜ pending |
| 196-01-02 | 01 | 1 | LAUNCH-02 | unit | `cargo test -- test_billing_gate_deferred` | ❌ W0 | ⬜ pending |
| 196-01-03 | 01 | 1 | LAUNCH-03 | unit | `cargo test -- test_billing_gate_paused` | ❌ W0 | ⬜ pending |
| 196-01-04 | 01 | 1 | LAUNCH-04 | unit | `cargo test -- test_billing_gate_toctou` | ❌ W0 | ⬜ pending |
| 196-02-01 | 02 | 1 | STATE-01 | unit | `cargo test -- test_double_launch_stopping` | ❌ W0 | ⬜ pending |
| 196-02-02 | 02 | 1 | STATE-02 | unit | `cargo test -- test_stopping_timeout` | ❌ W0 | ⬜ pending |
| 196-02-03 | 02 | 1 | STATE-03 | unit | `cargo test -- test_disconnected_agent` | ❌ W0 | ⬜ pending |
| 196-02-04 | 02 | 1 | STATE-04 | unit | `cargo test -- test_feature_flag_block` | ❌ W0 | ⬜ pending |
| 196-03-01 | 03 | 2 | LAUNCH-05 | unit | `cargo test -- test_invalid_json_launch` | ❌ W0 | ⬜ pending |
| 196-03-02 | 03 | 2 | LAUNCH-06 | unit | `cargo test -- test_broadcast_failure_logged` | ❌ W0 | ⬜ pending |
| 196-03-03 | 03 | 2 | STATE-05 | unit | `cargo test -- test_externally_tracked` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- Existing test infrastructure covers all phase requirements (cargo test already configured)
- Tests will be added alongside implementation in each plan

*Existing infrastructure covers all phase requirements.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Stopping 30s timeout | STATE-02 | Requires real-time wait | Set game to Stopping, wait 30s, verify Error state |
| Dashboard broadcast | LAUNCH-06 | Requires WebSocket client | Connect to dashboard WS, trigger broadcast failure |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
