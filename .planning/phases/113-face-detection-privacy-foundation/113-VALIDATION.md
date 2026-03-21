---
phase: 113
slug: face-detection-privacy-foundation
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-21
---

# Phase 113 — Validation Strategy

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

- **After every task commit:** Run `cargo test -p rc-sentry-ai`
- **After every plan wave:** Run `cargo test -p rc-sentry-ai && cargo test -p rc-common`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 20 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 113-01-01 | 01 | 1 | FACE-01 | unit | `cargo test -p rc-sentry-ai` | ❌ W0 | ⬜ pending |
| 113-02-01 | 02 | 1 | FACE-01 | integration | `cargo build -p rc-sentry-ai` | ❌ W0 | ⬜ pending |
| 113-03-01 | 03 | 2 | PRIV-01 | unit | `cargo test -p rc-sentry-ai` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] SCRFD-10GF ONNX model downloaded to `C:\RacingPoint\models\`
- [ ] ort crate with CUDA EP compiles on James

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Face detection on live camera feed | FACE-01 | Requires physical person in front of camera | Stand in front of entrance camera, check detection logs |
| Detection under 10ms per frame | FACE-01 | GPU timing varies | Run with RUST_LOG=debug, check timing logs |
| Consent signage requirements | PRIV-01 | Physical signage verification | Verify signage document covers DPDP requirements |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 20s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
