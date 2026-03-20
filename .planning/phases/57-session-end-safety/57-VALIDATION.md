---
phase: 57
slug: session-end-safety
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-20
---

# Phase 57 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (existing) + manual hardware verification |
| **Config file** | `crates/rc-agent/Cargo.toml` |
| **Quick run command** | `cargo test -p rc-agent -- ffb` |
| **Full suite command** | `cargo test -p rc-agent` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p rc-agent -- ffb`
- **After every plan wave:** Run `cargo test -p rc-agent`
- **Before `/gsd:verify-work`:** Full suite must be green + manual hardware test on canary pod
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 57-01-01 | 01 | 1 | SAFE-02 | unit | `cargo test -p rc-agent -- ffb_controller::tests` | ❌ W0 | ⬜ pending |
| 57-01-02 | 01 | 1 | SAFE-04 | unit | `cargo test -p rc-agent -- ffb_controller::tests` | ❌ W0 | ⬜ pending |
| 57-01-03 | 01 | 1 | SAFE-05 | unit | `cargo test -p rc-agent -- ffb_controller::tests` | ❌ W0 | ⬜ pending |
| 57-02-01 | 02 | 2 | SAFE-06 | unit+manual | `cargo test -p rc-agent -- conspit` | ❌ W0 | ⬜ pending |
| 57-02-02 | 02 | 2 | SAFE-01, SAFE-03 | manual | N/A (hardware) | N/A | ⬜ pending |
| 57-02-03 | 02 | 2 | SAFE-07 | unit | `cargo test -p rc-agent -- conspit` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Unit tests for new HID commands (fxm.reset, idlespring, power) in ffb_controller.rs
- [ ] Unit tests for ConspitLink WM_CLOSE graceful shutdown
- [ ] Unit tests for safe_session_end() orchestration logic

*Existing cargo test infrastructure covers compilation and existing tests.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Wheel physically centers within 2s | SAFE-01 | Requires real wheelbase hardware | Launch game on canary pod, end session, observe wheel position, measure time |
| No snap-back during centering | SAFE-03 | Requires physical observation | Place hands near wheel during session end, confirm gentle gradual force |
| ConspitLink restarts with config intact | SAFE-07 | Requires process observation | End session, verify ConspitLink restarts, check Global.json/Settings.json parse ok |
| Works on all 4 games | SAFE-01 | Per-game testing | Test AC, F1 25, ACC/ACE, AC Rally — end session on each |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
