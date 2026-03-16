---
phase: 2
slug: hud-infrastructure
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-11
---

# Phase 2 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | `crates/rc-agent/Cargo.toml` |
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
| 02-01-01 | 01 | 1 | INFRA-01 | unit | `cargo test -p rc-agent-crate -- overlay::tests::test_format_timer` | ❌ W0 | ⬜ pending |
| 02-01-02 | 01 | 1 | INFRA-01 | unit | `cargo test -p rc-agent-crate -- overlay::tests::test_format_lap` | ❌ W0 | ⬜ pending |
| 02-01-02 | 01 | 1 | INFRA-01 | unit | `cargo test -p rc-agent-crate -- overlay::tests::test_format_sector` | ❌ W0 | ⬜ pending |
| 02-01-03 | 01 | 1 | INFRA-01 | unit | `cargo test -p rc-agent-crate -- overlay::tests::test_sector_color` | ❌ W0 | ⬜ pending |
| 02-01-04 | 01 | 2 | INFRA-01 | unit | `cargo test -p rc-agent-crate` | N/A | ⬜ pending |
| 02-01-05 | 01 | 2 | INFRA-02 | unit | `cargo test -p rc-agent-crate -- overlay::tests::test_compute_layout` | ❌ W0 | ⬜ pending |
| 02-01-06 | 01 | 3 | INFRA-02 | unit | `cargo test -p rc-agent-crate` | N/A | ⬜ pending |
| 02-01-06 | 01 | 3 | INFRA-02 | manual | Deploy to Pod 8, visual regression check | N/A | ⬜ pending |
| 02-01-07 | 01 | 4 | INFRA-01 | unit | `cargo test -p rc-agent-crate` | N/A | ⬜ pending |
| 02-01-07 | 01 | 4 | INFRA-01 | manual | Deploy to Pod 8, 30-min GDI handle count | N/A | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `overlay.rs #[cfg(test)] mod tests` — characterization tests for format_timer, format_lap_time, format_sector, sector_color
- [ ] `overlay.rs tests::test_compute_layout` — layout math verification

*These tests must be written BEFORE the refactor and verified to pass AFTER (Test First, Refactor Second).*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| GDI handle count stays constant over 30-min session | INFRA-01 | Requires real Win32 desktop session with GDI subsystem | 1. Start rc-agent on Pod 8. 2. Activate overlay. 3. Note GDI handle count in Task Manager. 4. Wait 30 min. 5. Check GDI count has not grown. |
| Existing HUD renders identically after refactor | INFRA-02 | Visual comparison requires real display | 1. Screenshot HUD before refactor. 2. Deploy refactored code to Pod 8. 3. Screenshot and compare layout, fonts, colors. |
| New component can be added with trait + register only | INFRA-02 | Structural/API verification | 1. Create a dummy test component. 2. Register it. 3. Verify it appears without modifying paint_hud(). |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
