---
phase: 60
slug: pre-launch-profile-loading
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-24
---

# Phase 60 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (rc-agent-crate) |
| **Config file** | crates/rc-agent/Cargo.toml |
| **Quick run command** | `cargo test -p rc-agent-crate -- --test-threads=1 pre_load` |
| **Full suite command** | `cargo test -p rc-agent-crate -- --test-threads=1` |
| **Estimated runtime** | ~20 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p rc-agent-crate -- --test-threads=1 pre_load`
- **After every plan wave:** Run `cargo test -p rc-agent-crate -- --test-threads=1`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 20 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 60-01-01 | 01 | 1 | PROF-03 | unit | `cargo test -p rc-agent-crate -- test_pre_load_recognized_game` | ❌ W0 | ⬜ pending |
| 60-01-01 | 01 | 1 | PROF-03 | unit | `cargo test -p rc-agent-crate -- test_pre_load_f125` | ❌ W0 | ⬜ pending |
| 60-01-01 | 01 | 1 | PROF-03 | unit | `cargo test -p rc-agent-crate -- test_pre_load_ac_evo` | ❌ W0 | ⬜ pending |
| 60-01-01 | 01 | 1 | PROF-05 | unit | `cargo test -p rc-agent-crate -- test_pre_load_unrecognized_no_global_write` | ❌ W0 | ⬜ pending |
| 60-01-01 | 01 | 1 | PROF-05 | unit | `cargo test -p rc-agent-crate -- test_sim_type_to_game_key_unrecognized` | ❌ W0 | ⬜ pending |
| 60-01-01 | 01 | 1 | PROF-05 | unit | `cargo test -p rc-agent-crate -- test_sim_type_to_game_key_recognized` | ❌ W0 | ⬜ pending |
| 60-01-01 | 01 | 1 | PROF-03+05 | unit | `cargo test -p rc-agent-crate -- test_unrecognized_fallback_no_device` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Unit tests for `pre_load_game_preset()` — stubs for PROF-03
- [ ] Unit tests for `sim_type_to_game_key()` — stubs for PROF-05
- [ ] Unit tests for `apply_unrecognized_game_fallback()` — stubs for PROF-05

*All tests are new — created by Task 1 (TDD plan). Existing cargo test infrastructure covers framework requirements.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Preset loads before game starts on real pod | PROF-03 | Requires ConspitLink + game on hardware | Launch AC on Pod 8, observe CL preset loaded before game window appears |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 20s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
