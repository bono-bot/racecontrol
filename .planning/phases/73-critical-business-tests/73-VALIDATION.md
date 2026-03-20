---
phase: 73
slug: critical-business-tests
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-20
---

# Phase 73 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (built-in) + mockall 0.13 (dev-dependency) |
| **Config file** | Cargo.toml (workspace) |
| **Quick run command** | `cargo test -p rc-agent-crate billing_guard failure_monitor ffb` |
| **Full suite command** | `cargo test -p rc-agent-crate` |
| **Estimated runtime** | ~20 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p rc-agent-crate billing_guard failure_monitor ffb`
- **After every plan wave:** Run `cargo test -p rc-agent-crate`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 20 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 73-01-01 | 01 | 1 | TEST-03 | unit | `cargo test -p rc-agent-crate ffb` | ❌ W0 | ⬜ pending |
| 73-02-01 | 02 | 1 | TEST-01 | unit | `cargo test -p rc-agent-crate billing_guard` | ❌ W0 | ⬜ pending |
| 73-02-02 | 02 | 1 | TEST-02 | unit | `cargo test -p rc-agent-crate failure_monitor` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `mockall = "0.13"` added to rc-agent `[dev-dependencies]`
- [ ] `FfbBackend` trait + `MockFfbBackend` in ffb_controller.rs
- [ ] billing_guard test module with tokio::time::pause() + advance() pattern
- [ ] failure_monitor test module with mock game state

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| None | — | — | — |

*All phase behaviors have automated verification.*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 20s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
