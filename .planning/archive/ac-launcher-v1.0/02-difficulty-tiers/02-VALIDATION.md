---
phase: 2
slug: difficulty-tiers
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-13
---

# Phase 2 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | Cargo.toml workspace |
| **Quick run command** | `cargo test -p rc-agent --lib ac_launcher` |
| **Full suite command** | `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p rc-core` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p rc-agent --lib ac_launcher`
- **After every plan wave:** Run `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p rc-core`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 02-01-01 | 01 | 1 | DIFF-01 | unit | `cargo test -p rc-agent -- test_tier_boundaries test_tier_names -x` | ❌ W0 | ⬜ pending |
| 02-01-02 | 01 | 1 | DIFF-02 | unit | `cargo test -p rc-agent -- test_race_ini_uses_session_ai_level -x` | ❌ W0 | ⬜ pending |
| 02-01-03 | 01 | 1 | DIFF-03/04 | unit | `cargo test -p rc-agent -- test_write_race_ini_practice_with_aids -x` | ✅ | ⬜ pending |
| 02-01-04 | 01 | 1 | DIFF-05 | unit | `cargo test -p rc-agent -- test_tier_for_level_custom test_backward_compat -x` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/rc-agent/src/ac_launcher.rs` — add #[cfg(test)] tests for DifficultyTier boundaries, midpoints, names
- [ ] `test_tier_boundaries` — tests all 5 tier boundary values (min, max, midpoint)
- [ ] `test_tier_for_level_custom` — values below 70 and above 100 return None
- [ ] `test_race_ini_uses_session_ai_level` — session ai_level overrides per-car default in INI
- [ ] `test_backward_compat_no_ai_level_field` — old JSON without ai_level defaults to 87
- [ ] `test_effective_ai_cars_inherits_session_ai_level` — all AI car slots get session ai_level
- [ ] `test_trackday_default_ai_inherits_session_ai_level` — trackday AI uses session ai_level not hardcoded 85

*Wave 0 creates test stubs before implementation begins.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| AI drives at expected difficulty on pod | DIFF-02 | Requires AC runtime | Deploy to Pod 8, race at Rookie (75) and Alien (98), verify AI speed difference |
| Slider in PWA/kiosk updates tier label | DIFF-01/05 | Requires PWA | Load PWA, drag slider, verify tier name updates in real-time |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
