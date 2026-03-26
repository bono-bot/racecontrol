---
phase: 198
slug: on-track-billing
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-26
---

# Phase 198 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust `cargo test` (built-in) |
| **Config file** | `crates/racecontrol/Cargo.toml`, `crates/rc-agent/Cargo.toml`, `crates/rc-common/Cargo.toml` |
| **Quick run command** | `cargo test --package racecontrol -- billing && cargo test --package rc-agent -- event_loop` |
| **Full suite command** | `cargo test --package racecontrol && cargo test --package rc-agent && cargo test --package rc-common` |
| **Estimated runtime** | ~45 seconds |

---

## Sampling Rate

- **After every task commit:** Run quick run command
- **After every plan wave:** Run full suite command
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 45 seconds

---

## Wave 0 Requirements

- Existing test infrastructure covers all phase requirements
- Tests written alongside implementation (TDD pattern)

*Existing infrastructure covers all phase requirements.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| AC PlayableSignal detection | BILL-01 | Requires live AC + UDP telemetry | Launch AC, verify billing starts when car on track |
| F1 25 PlayableSignal | BILL-02 | Requires live F1 25 + UDP | Launch F1, verify billing starts at race start |
| Dashboard "Loading..." state | BILL-05 | Requires kiosk UI | Launch game, check kiosk shows Loading |
| Multiplayer billing | BILL-10 | Requires multiple pods | Start multiplayer, verify all pods billed |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 45s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
