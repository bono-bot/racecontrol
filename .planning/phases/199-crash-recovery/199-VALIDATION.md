---
phase: 199
slug: crash-recovery
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-26
---

# Phase 199 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust `cargo test` (built-in) |
| **Config file** | `crates/racecontrol/Cargo.toml`, `crates/rc-agent/Cargo.toml` |
| **Quick run command** | `cargo test --package racecontrol -- game_launcher && cargo test --package rc-agent -- event_loop` |
| **Full suite command** | `cargo test --package racecontrol && cargo test --package rc-agent` |
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

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Full crash recovery cycle | RECOVER-02 | Requires live pod + game crash | Kill AC mid-session, verify relaunch <60s |
| Staff WhatsApp alert | RECOVER-06 | Requires Evolution API | Exhaust retries, verify WhatsApp received |
| Safe mode cooldown | RECOVER-07 | Requires live pod | Crash game during safe mode window |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] No watch-mode flags
- [ ] Feedback latency < 45s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
