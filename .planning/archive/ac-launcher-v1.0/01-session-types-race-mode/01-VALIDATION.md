---
phase: 1
slug: session-types-race-mode
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
| **Framework** | cargo test (Rust built-in) |
| **Config file** | Cargo.toml workspace |
| **Quick run command** | `cargo test -p rc-agent -- --test-threads=1` |
| **Full suite command** | `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p rc-core` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p rc-agent -- --test-threads=1`
- **After every plan wave:** Run `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p rc-core`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 01-01-01 | 01 | 1 | SESS-01 | unit | `cargo test -p rc-agent write_race_ini_practice -x` | ❌ W0 | ⬜ pending |
| 01-01-02 | 01 | 1 | SESS-02 | unit | `cargo test -p rc-agent write_race_ini_race_ai -x` | ❌ W0 | ⬜ pending |
| 01-01-03 | 01 | 1 | SESS-03 | unit | `cargo test -p rc-agent write_race_ini_hotlap -x` | ❌ W0 | ⬜ pending |
| 01-01-04 | 01 | 1 | SESS-04 | unit | `cargo test -p rc-agent write_race_ini_trackday -x` | ❌ W0 | ⬜ pending |
| 01-01-05 | 01 | 1 | SESS-05 | unit | `cargo test -p rc-agent write_race_ini_weekend -x` | ❌ W0 | ⬜ pending |
| 01-01-06 | 01 | 1 | SESS-08 | unit | `cargo test -p rc-agent write_race_ini_no_fallback -x` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/rc-agent/src/ac_launcher.rs` — add #[cfg(test)] mod with tests for each session type INI generation
- [ ] Test helper: function that parses generated INI string and returns a HashMap of sections for assertion
- [ ] `rand` dependency in rc-agent/Cargo.toml for AI name shuffling

*Wave 0 creates test stubs before implementation begins.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Game actually launches with correct session type | SESS-08 | Requires AC installed on pod | Deploy to Pod 8, launch each session type, verify in-game |
| AI opponents visible and driving in race | SESS-02 | Requires AC runtime | Launch Race vs AI on Pod 8, verify AI cars appear on grid |
| Race Weekend transitions between sessions | SESS-05 | Requires AC runtime | Launch Race Weekend on Pod 8, verify P→Q→R sequence |
| Track Day mixed traffic behavior | SESS-04 | Requires AC runtime | Launch Track Day on Pod 8, verify mixed class AI present |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
