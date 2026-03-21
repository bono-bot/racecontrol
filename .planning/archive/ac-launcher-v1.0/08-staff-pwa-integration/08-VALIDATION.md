---
phase: 8
slug: staff-pwa-integration
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-14
---

# Phase 8 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust) + Next.js TypeScript compile |
| **Config file** | Cargo.toml workspace |
| **Quick run command** | `cargo test -p rc-core -- catalog` |
| **Full suite command** | `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p rc-core` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p rc-core -- catalog && cd kiosk && npx tsc --noEmit && cd ../pwa && npx tsc --noEmit`
- **After every plan wave:** Run full suite
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 08-01-01 | 01 | 1 | SESS-06, CONT-03 | build | `cd kiosk && npx tsc --noEmit` | ✅ | ⬜ pending |
| 08-01-02 | 01 | 1 | SESS-06, CONT-03 | build | `cd kiosk && npx tsc --noEmit` | ✅ | ⬜ pending |
| 08-01-03 | 01 | 1 | SESS-06 | build | `cd pwa && npx tsc --noEmit` | ✅ | ⬜ pending |
| 08-01-04 | 01 | 1 | SESS-06, CONT-03 | unit | `cargo test -p rc-core -- catalog` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Test stub for validate_launch_combo with race_weekend session type
- [ ] Test stub for build_custom_launch_args with session_type parameter
- [ ] TypeScript compilation checks for both kiosk and pwa after type changes

*Existing test infrastructure covers framework needs. Only test stubs needed.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Kiosk GameConfigurator shows all 5 session types | SESS-06 | Requires browser + kiosk running | Open kiosk, click pod, verify 5 session types in wizard |
| Kiosk SetupWizard shows all 5 session types | SESS-06 | Requires browser + kiosk running | Open kiosk booking flow, verify session type step has 5 options |
| PWA shows session type picker (replaces Mode) | CONT-03 | Requires browser + running PWA | Open PWA /book, verify session types shown instead of Mode |
| AI session types hidden for no-AI tracks | SESS-06 | Requires pod with limited content | Select track without AI, verify Race vs AI / Track Day hidden |
| Staff launch and customer launch use same validation | CONT-03 | Requires both paths running | Launch from kiosk and PWA with same config, verify both validated |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
