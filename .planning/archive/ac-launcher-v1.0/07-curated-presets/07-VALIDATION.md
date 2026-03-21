---
phase: 7
slug: curated-presets
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-14
---

# Phase 7 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) + Next.js TypeScript compile |
| **Config file** | Cargo.toml workspace |
| **Quick run command** | `cargo test -p rc-core catalog` |
| **Full suite command** | `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p rc-core` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p rc-core catalog`
- **After every plan wave:** Run full suite
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 07-01-01 | 01 | 1 | CONT-08 | unit | `cargo test -p rc-core preset` | ❌ W0 | ⬜ pending |
| 07-01-02 | 01 | 1 | CONT-09 | unit | `cargo test -p rc-core preset_filtering` | ❌ W0 | ⬜ pending |
| 07-01-03 | 01 | 1 | CONT-08 | compile | `cd pwa && npx tsc --noEmit` | ✅ | ⬜ pending |
| 07-01-04 | 01 | 1 | CONT-08 | compile | `cd kiosk && npx tsc --noEmit` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Test stubs for preset data structure (PresetEntry fields, category, featured flag)
- [ ] Test stubs for preset filtering (manifest-based, AI line check for race/trackday presets)
- [ ] Test stubs for preset inclusion in catalog JSON response

*Existing test infrastructure covers framework needs. Only test stubs needed.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Presets visible in PWA booking screen | CONT-08 | Requires browser + running PWA | Open PWA, navigate to booking, verify preset cards visible with thumbnails |
| Preset tap pre-fills configurator | CONT-08 | Requires browser interaction | Tap a preset, verify car/track/session/difficulty pre-filled in wizard |
| Presets visible in kiosk | CONT-08 | Requires kiosk UI running | Open kiosk, verify preset section visible in GameConfigurator |
| Pod-specific preset filtering | CONT-08 | Requires pod with limited content | Connect pod with partial manifest, verify only matching presets shown |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
