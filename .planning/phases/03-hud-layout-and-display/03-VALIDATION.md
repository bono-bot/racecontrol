---
phase: 3
slug: hud-layout-and-display
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-12
---

# Phase 3 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | Cargo.toml (workspace) |
| **Quick run command** | `cargo test -p rc-agent` |
| **Full suite command** | `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p rc-core` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p rc-agent`
- **After every plan wave:** Run `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p rc-core`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 03-01-01 | 01 | 1 | HUD-01 | unit | `cargo test -p rc-agent -- gear_indicator` | ❌ W0 | ⬜ pending |
| 03-01-02 | 01 | 1 | HUD-02 | unit | `cargo test -p rc-agent -- speed_display` | ❌ W0 | ⬜ pending |
| 03-01-03 | 01 | 1 | HUD-03 | unit | `cargo test -p rc-agent -- rpm_bar` | ❌ W0 | ⬜ pending |
| 03-02-01 | 02 | 1 | HUD-04 | unit | `cargo test -p rc-agent -- lap_time` | ❌ W0 | ⬜ pending |
| 03-02-02 | 02 | 1 | HUD-05 | unit | `cargo test -p rc-agent -- session_timer` | ❌ W0 | ⬜ pending |
| 03-02-03 | 02 | 1 | HUD-06 | unit | `cargo test -p rc-agent -- sector_time` | ❌ W0 | ⬜ pending |
| 03-02-04 | 02 | 1 | HUD-07 | unit | `cargo test -p rc-agent -- lap_counter` | ❌ W0 | ⬜ pending |
| 03-02-05 | 02 | 1 | HUD-08 | unit | `cargo test -p rc-agent -- invalid_indicator` | ❌ W0 | ⬜ pending |
| 03-02-06 | 02 | 1 | HUD-09 | unit | `cargo test -p rc-agent -- monospace_font` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Add layout geometry tests for gear/speed/RPM positioning (HUD-01, HUD-02, HUD-03)
- [ ] Add timing display tests for lap times, sectors, session timer (HUD-04, HUD-05, HUD-06)
- [ ] Add lap counter + invalid indicator tests (HUD-07, HUD-08)
- [ ] Add font/monospace assertion tests (HUD-09)

*Existing test infrastructure (cargo test) covers framework needs — Wave 0 adds stubs only.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Gear readable from 1.5m | HUD-01 | Physical distance check | Sit in driver position on Pod 8, verify gear digit is readable |
| RPM bar color zones visible | HUD-03 | Visual color perception | Drive on Pod 8, verify green→yellow→amber→red progression |
| No layout jitter on digit change | HUD-09 | Visual animation artifact | Drive on Pod 8, watch speed/lap digits — no horizontal shifting |
| Elements don't overlap AC UI | All | Overlay positioning | Drive on Pod 8, verify no HUD elements obscure game UI |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
