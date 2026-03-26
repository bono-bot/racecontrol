---
phase: 197
slug: launch-resilience-ac-hardening
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-26
---

# Phase 197 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust `cargo test` (built-in) |
| **Config file** | `crates/racecontrol/Cargo.toml`, `crates/rc-agent/Cargo.toml` |
| **Quick run command** | `cargo test --package racecontrol -- game_launcher && cargo test --package rc-agent -- game_manager` |
| **Full suite command** | `cargo test --package racecontrol && cargo test --package rc-agent` |
| **Estimated runtime** | ~45 seconds |

---

## Sampling Rate

- **After every task commit:** Run quick run command
- **After every plan wave:** Run full suite command
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 45 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 197-01-01 | 01 | 1 | LAUNCH-08,09 | unit | `cargo test -- test_dynamic_timeout` | ❌ W0 | ⬜ pending |
| 197-01-02 | 01 | 1 | LAUNCH-11,12,13 | unit | `cargo test -- test_error_taxonomy` | ❌ W0 | ⬜ pending |
| 197-01-03 | 01 | 1 | LAUNCH-14,15,16,17,18 | unit | `cargo test -- test_race_engineer` | ❌ W0 | ⬜ pending |
| 197-02-01 | 02 | 2 | LAUNCH-10 | unit | `cargo test -- test_pre_launch_check` | ❌ W0 | ⬜ pending |
| 197-02-02 | 02 | 2 | AC-01,02,03,04 | unit | `cargo test -- test_ac_polling` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- Existing test infrastructure covers all phase requirements (cargo test already configured)
- Tests will be added alongside implementation in each plan (TDD pattern)

*Existing infrastructure covers all phase requirements.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| WhatsApp staff alert | LAUNCH-14 | Requires Evolution API | Trigger 2 failed retries, check WhatsApp |
| AC window detection | AC-01 | Requires live AC process | Launch AC on pod, verify window polling |
| Pre-launch disk check | LAUNCH-10 | Requires live pod | Create MAINTENANCE_MODE, attempt launch |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 45s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
