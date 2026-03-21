---
phase: 114
slug: face-recognition-quality-gates
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-21
---

# Phase 114 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust) |
| **Config file** | Cargo.toml workspace |
| **Quick run command** | `cargo test -p rc-sentry-ai` |
| **Full suite command** | `cargo test -p rc-sentry-ai && cargo test -p rc-common` |
| **Estimated runtime** | ~20 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo check -p rc-sentry-ai`
- **After every plan wave:** Run `cargo check -p rc-sentry-ai`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 20 seconds

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| ArcFace recognizes enrolled person | FACE-02 | Requires enrolled face + live camera | Enroll test face, walk past entrance camera |
| Quality gates reject blurry captures | FACE-03 | Requires physical testing | Wave hand quickly past camera, check rejection logs |
| CLAHE handles entrance backlight | FACE-04 | Lighting conditions vary | Test recognition at different times of day |
| Face tracker deduplicates within 60s | FACE-02 | Requires walking past camera twice | Walk past, wait 30s, walk past again — should not re-recognize |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 20s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
