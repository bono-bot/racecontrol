---
phase: 6
slug: mid-session-controls
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-14
---

# Phase 6 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | Cargo.toml workspace |
| **Quick run command** | `cargo test -p rc-agent && cargo test -p rc-common` |
| **Full suite command** | `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p rc-core` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p rc-agent && cargo test -p rc-common`
- **After every plan wave:** Run full suite
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 06-01-01 | 01 | 1 | DIFF-06 | unit | `cargo test -p rc-agent toggle_transmission` | ❌ W0 | ⬜ pending |
| 06-01-02 | 01 | 1 | DIFF-07 | unit | `cargo test -p rc-agent toggle_abs` | ❌ W0 | ⬜ pending |
| 06-01-03 | 01 | 1 | DIFF-08 | unit | `cargo test -p rc-agent toggle_tc` | ❌ W0 | ⬜ pending |
| 06-01-04 | 01 | 1 | DIFF-09 | unit | `cargo test -p rc-agent stability_excluded` | ❌ W0 | ⬜ pending |
| 06-01-05 | 01 | 1 | DIFF-10 | unit | `cargo test -p rc-agent set_gain` | ❌ W0 | ⬜ pending |
| 06-01-06 | 01 | 1 | N/A | unit | `cargo test -p rc-common mid_session` | ❌ W0 | ⬜ pending |
| 06-01-07 | 01 | 1 | N/A | unit | `cargo test -p rc-agent toast` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Test stubs for SendInput helpers in rc-agent (buffer format, not actual keypress)
- [ ] Test stubs for `set_gain()` HID buffer format in ffb_controller.rs
- [ ] Serialization tests for new protocol variants (SetAssist/SetFfbGain/AssistState)
- [ ] Test stubs for toast overlay data management (set, expire, replace)
- [ ] Test stubs for assist state reading from shared memory (offset correctness)

*Existing test infrastructure covers framework needs. Only test stubs needed.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| SendInput keypresses reach AC | DIFF-06, DIFF-07, DIFF-08 | Requires running AC process on pod | Deploy to Pod 8, launch AC, send Ctrl+G via rc-agent, verify transmission toggles in-game |
| FFB gain HID command changes force | DIFF-10 | Requires physical wheelbase | Deploy to Pod 8, set FFB to 50% via PWA, verify reduced force on wheel |
| Stability control NOT shown in PWA | DIFF-09 | Requires PWA visual inspection | Open PWA session controls, verify no stability control toggle present |
| Overlay toast visible on pod screen | N/A | Requires running overlay on pod | Trigger assist change, verify toast appears top-center and disappears after 3s |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
