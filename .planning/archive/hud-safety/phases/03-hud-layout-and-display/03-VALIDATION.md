---
phase: 3
slug: hud-layout-and-display
status: draft
nyquist_compliant: true
wave_0_complete: true
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
| **Quick run command** | `cargo test -p rc-agent-crate` |
| **Full suite command** | `cargo test -p rc-common && cargo test -p rc-agent-crate && cargo test -p racecontrol-crate` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p rc-agent-crate`
- **After every plan wave:** Run `cargo test -p rc-common && cargo test -p rc-agent-crate && cargo test -p racecontrol-crate`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 03-01-T1 | 01 | 1 | HUD-01, HUD-02, HUD-09 | unit | `cargo test -p rc-agent-crate -- overlay::tests` | ✅ (test_compute_layout exists, test_rpm_color_zones created inline) | ⬜ pending |
| 03-01-T2 | 01 | 1 | HUD-08 | build+unit | `cargo test -p rc-agent-crate` | ✅ | ⬜ pending |
| 03-02-T1 | 02 | 2 | HUD-03, HUD-04, HUD-05, HUD-07 | build+unit | `cargo test -p rc-agent-crate` | ✅ | ⬜ pending |
| 03-02-T2 | 02 | 2 | HUD-06 | full suite | `cargo test -p rc-common && cargo test -p rc-agent-crate && cargo test -p racecontrol-crate` | ✅ | ⬜ pending |
| 03-02-T3 | 02 | 2 | All HUD | checkpoint | Pod 8 visual verification | N/A | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

*No separate Wave 0 needed — test_rpm_color_zones is created inline by Plan 03-01 Task 1 (TDD). test_compute_layout already exists and is updated by the same task. All other verification uses existing cargo test infrastructure.*

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
