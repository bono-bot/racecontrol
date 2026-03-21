---
phase: 82
slug: billing-and-session-lifecycle
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-21
---

# Phase 82 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in `#[test]` + `#[tokio::test]` |
| **Config file** | None — inline `#[cfg(test)]` modules |
| **Quick run command** | `cargo test -p rc-agent -p racecontrol billing -- --nocapture` |
| **Full suite command** | `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p racecontrol` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p racecontrol billing -- --nocapture`
- **After every plan wave:** Run `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p racecontrol`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 82-01-01 | 01 | 1 | BILL-01, BILL-02 | unit | `cargo test -p racecontrol billing::tests::bill01` | ❌ W0 | ⬜ pending |
| 82-01-02 | 01 | 1 | BILL-02 | unit | `cargo test -p rc-agent playable_signal` | ❌ W0 | ⬜ pending |
| 82-01-03 | 01 | 1 | BILL-03 | unit | `cargo test -p racecontrol billing::tests::per_game_rate` | ❌ W0 | ⬜ pending |
| 82-02-01 | 02 | 1 | BILL-04 | unit | `cargo test -p racecontrol billing::tests::bill04_grace` | ❌ W0 | ⬜ pending |
| 82-02-02 | 02 | 1 | BILL-05 | unit | `cargo test -p racecontrol billing::tests::bill05_lifecycle` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Characterization tests for `compute_session_cost()` with `sim_type` parameter -- covers BILL-03
- [ ] Unit tests for 30s grace period timer -- covers BILL-04
- [ ] Unit test: `GameState::Loading` serialization roundtrip -- covers BILL-05
- [ ] Test: `PlayableSignal::ProcessFallback` after 90s elapsed -- covers BILL-02

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Loading state visible in kiosk | BILL-05 | Requires live kiosk with game launching | Launch game from kiosk, verify pod card shows "Loading..." |
| Admin UI per-game rates | BILL-03 | Requires admin dashboard access | Open admin, verify game column in rates table |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
