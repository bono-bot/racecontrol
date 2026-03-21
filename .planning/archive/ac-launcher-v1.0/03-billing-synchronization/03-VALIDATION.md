---
phase: 3
slug: billing-synchronization
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-13
---

# Phase 3 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | Cargo.toml workspace |
| **Quick run command** | `cargo test -p rc-common && cargo test -p rc-core -- billing && cargo test -p rc-agent -- overlay` |
| **Full suite command** | `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p rc-core` |
| **Estimated runtime** | ~20 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p rc-common && cargo test -p rc-core -- billing && cargo test -p rc-agent -- overlay`
- **After every plan wave:** Run `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p rc-core`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 20 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 03-01-01 | 01 | 1 | BILL-01 | unit | `cargo test -p rc-core -- billing::tests::billing_starts_on_live_only -x` | ❌ W0 | ⬜ pending |
| 03-01-02 | 01 | 1 | BILL-01 | unit | `cargo test -p rc-core -- billing::tests::launch_timeout_triggers_retry -x` | ❌ W0 | ⬜ pending |
| 03-01-03 | 01 | 1 | BILL-01 | unit | `cargo test -p rc-core -- billing::tests::double_failure_cancels_no_charge -x` | ❌ W0 | ⬜ pending |
| 03-01-04 | 01 | 1 | BILL-02 | unit | `cargo test -p rc-core -- billing::tests::no_billing_during_waiting -x` | ❌ W0 | ⬜ pending |
| 03-01-05 | 01 | 1 | ALL | unit | `cargo test -p rc-core -- billing::tests::cost_calculation -x` | ❌ W0 | ⬜ pending |
| 03-01-06 | 01 | 1 | ALL | unit | `cargo test -p rc-core -- billing::tests::retroactive_tier_crossing -x` | ❌ W0 | ⬜ pending |
| 03-01-07 | 01 | 1 | ALL | unit | `cargo test -p rc-core -- billing::tests::timer_counts_up -x` | ❌ W0 | ⬜ pending |
| 03-01-08 | 01 | 1 | ALL | unit | `cargo test -p rc-core -- billing::tests::pause_freezes_elapsed -x` | ❌ W0 | ⬜ pending |
| 03-01-09 | 01 | 1 | ALL | unit | `cargo test -p rc-core -- billing::tests::pause_timeout_auto_end -x` | ❌ W0 | ⬜ pending |
| 03-01-10 | 01 | 1 | ALL | unit | `cargo test -p rc-core -- billing::tests::hard_max_cap_auto_end -x` | ❌ W0 | ⬜ pending |
| 03-01-11 | 01 | 1 | ALL | unit | `cargo test -p rc-common -- protocol::tests::game_status_update_roundtrip -x` | ❌ W0 | ⬜ pending |
| 03-02-01 | 02 | 1 | BILL-06 | unit | `cargo test -p rc-agent -- overlay::tests::taxi_meter_display -x` | ❌ W0 | ⬜ pending |
| 03-02-02 | 02 | 1 | BILL-06 | unit | `cargo test -p rc-agent -- overlay::tests::paused_badge_display -x` | ❌ W0 | ⬜ pending |
| 03-02-03 | 02 | 1 | BILL-06 | unit | `cargo test -p rc-agent -- overlay::tests::waiting_for_game_display -x` | ❌ W0 | ⬜ pending |
| 03-02-04 | 02 | 1 | ALL | unit | `cargo test -p rc-agent -- assetto_corsa::tests::ac_status_read -x` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/rc-core/src/billing.rs` — add #[cfg(test)] tests for STATUS-triggered billing, cost calculation, timer count-up, pause, timeout
- [ ] `crates/rc-common/src/protocol.rs` — add GameStatusUpdate message roundtrip test
- [ ] `crates/rc-agent/src/overlay.rs` — add tests for taxi meter display, PAUSED badge, WAITING FOR GAME state
- [ ] `crates/rc-agent/src/sims/assetto_corsa.rs` — add cfg(not(windows)) stub test for ac_status read

*Wave 0 creates test stubs before implementation begins.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Billing starts when car hits track in AC | BILL-01 | Requires AC runtime + shared memory | Deploy to Pod 8, launch AC, verify billing only starts when STATUS=LIVE in shared memory |
| Overlay shows taxi meter with live cost | BILL-06 | Requires AC runtime + overlay rendering | Deploy to Pod 8, drive, verify elapsed time + running cost display updates in real-time |
| Pause badge appears on ESC | BILL-06 | Requires AC runtime | Press ESC during AC session, verify PAUSED badge appears and timer freezes |
| Rate upgrade prompt at ~25 min | BILL-06 | Requires sustained AC session | Drive for 25+ min, verify upgrade prompt appears |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 20s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
