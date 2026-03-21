---
phase: 4
slug: safety-enforcement
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-14
---

# Phase 4 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | Cargo.toml workspace |
| **Quick run command** | `cargo test -p rc-agent -- --test-threads=1` |
| **Full suite command** | `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p rc-core` |
| **Estimated runtime** | ~20 seconds |

---

## Sampling Rate

- **After every task commit:** `cargo test -p rc-agent && cargo test -p rc-common`
- **After every plan wave:** `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p rc-core`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 20 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | Status |
|---------|------|------|-------------|-----------|-------------------|--------|
| 04-01-01 | 01 | 1 | BILL-03 | unit | `cargo test -p rc-agent -- test_write_race_ini_grip_always_100` | ⬜ pending |
| 04-01-02 | 01 | 1 | BILL-04 | unit | `cargo test -p rc-agent -- test_write_race_ini_damage_always_zero` | ⬜ pending |
| 04-01-03 | 01 | 1 | BILL-04 | unit | `cargo test -p rc-agent -- test_write_assists_ini_damage_always_zero` | ⬜ pending |
| 04-01-04 | 01 | 1 | BILL-03 | unit | `cargo test -p rc-core -- test_server_cfg_grip_always_100` | ⬜ pending |
| 04-01-05 | 01 | 1 | BILL-04 | unit | `cargo test -p rc-core -- test_server_cfg_damage_always_zero` | ⬜ pending |
| 04-01-06 | 01 | 1 | BILL-03+04 | unit | `cargo test -p rc-agent -- test_verify_safety_settings` | ⬜ pending |
| 04-02-01 | 02 | 1 | BILL-05 | unit | `cargo test -p rc-common -- test_ffb_zeroed_roundtrip` | ⬜ pending |
| 04-02-02 | 02 | 1 | BILL-05 | unit | `cargo test -p rc-agent -- test_vendor_cmd_buffer` | ⬜ existing |
| 04-02-03 | 02 | 1 | BILL-05 | manual | Deploy to Pod 8, end session, verify FFB zeros before kill | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `ac_launcher::tests::test_write_race_ini_damage_always_zero` — DAMAGE=0 regardless of params
- [ ] `ac_launcher::tests::test_write_race_ini_grip_always_100` — SESSION_START=100 in DYNAMIC_TRACK
- [ ] `ac_launcher::tests::test_verify_safety_settings_passes` — accepts correct INI
- [ ] `ac_launcher::tests::test_verify_safety_settings_rejects_damage` — rejects DAMAGE != 0
- [ ] `ac_launcher::tests::test_verify_safety_settings_rejects_grip` — rejects SESSION_START != 100
- [ ] `ac_launcher::tests::test_write_assists_ini_damage_always_zero` — assists.ini DAMAGE=0
- [ ] `protocol::tests::test_ffb_zeroed_roundtrip` — FfbZeroed message serde roundtrip
- [ ] `ac_server::tests::test_server_cfg_damage_always_zero` — server DAMAGE_MULTIPLIER=0
- [ ] `ac_server::tests::test_server_cfg_grip_always_100` — server SESSION_START=100

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| FFB zeros before game kill on session end | BILL-05 | Requires physical wheelbase | Deploy to Pod 8, start AC session, end via kiosk, verify wheel goes limp before AC window closes |
| FFB zeros on game crash | BILL-05 | Requires physical wheelbase + crash | Deploy to Pod 8, kill acs.exe manually, verify wheel goes limp |
| Grip feels correct in-game | BILL-03 | Subjective driving feel | Drive on Pod 8, verify tyre grip feels normal |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 20s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
