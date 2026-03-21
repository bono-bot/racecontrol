---
phase: 81
slug: game-launch-core
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-21
---

# Phase 81 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (nextest configured at `.config/nextest.toml`) |
| **Config file** | `.config/nextest.toml` |
| **Quick run command** | `cargo test -p rc-agent -- crash_recovery 2>&1` |
| **Full suite command** | `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p racecontrol` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p rc-agent 2>&1 | tail -5`
- **After every plan wave:** Run `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p racecontrol`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 81-01-01 | 01 | 1 | LAUNCH-03 | unit | `cargo test -p rc-agent -- test_game_exe_config` | ✅ existing | ⬜ pending |
| 81-01-02 | 01 | 1 | LAUNCH-03 | unit | `cargo test -p rc-agent -- test_installed_games` | ✅ existing | ⬜ pending |
| 81-01-03 | 01 | 1 | LAUNCH-04 | unit | `cargo test -p rc-agent -- crash01` | ✅ existing | ⬜ pending |
| 81-01-04 | 01 | 1 | LAUNCH-04 | unit | `cargo test -p rc-agent -- crash02` | ✅ existing | ⬜ pending |
| 81-02-01 | 02 | 1 | LAUNCH-05 | unit | `cargo test -p rc-agent -- non_ac_crash_recovery` | ❌ W0 | ⬜ pending |
| 81-02-02 | 02 | 1 | LAUNCH-06 | unit | `cargo test -p racecontrol -- game_state_update` | ✅ existing | ⬜ pending |
| 81-03-01 | 03 | 2 | LAUNCH-01 | manual | deploy to pod8, launch F1 25 from kiosk | ❌ manual | ⬜ pending |
| 81-03-02 | 03 | 2 | LAUNCH-02 | manual | PWA on phone -> staff kiosk confirm | ❌ manual | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/rc-agent/src/main.rs` -- unit test for non-AC crash recovery path (`non_ac_crash_recovery`) -- covers LAUNCH-05
- [ ] Existing test infrastructure covers LAUNCH-03, LAUNCH-04, LAUNCH-06

*Existing infrastructure covers most phase requirements. One new test needed for LAUNCH-05.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Staff launches non-AC game from kiosk | LAUNCH-01 | Requires real pod + Steam + game install | Deploy to Pod 8, open kiosk, select F1 25, verify game launches |
| Customer requests game from PWA | LAUNCH-02 | Requires real PWA + phone + kiosk | Open PWA on phone, tap game, verify staff sees request in kiosk |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
