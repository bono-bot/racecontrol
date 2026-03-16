---
phase: 20
slug: deploy-resilience
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-15
---

# Phase 20 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | Cargo.toml workspace |
| **Quick run command** | `cargo test -p rc-common && cargo test -p racecontrol-crate` |
| **Full suite command** | `cargo test -p rc-common && cargo test -p rc-agent-crate && cargo test -p racecontrol-crate && cargo test -p rc-watchdog` |
| **Estimated runtime** | ~25 seconds |

---

## Sampling Rate

- **After every task commit:** Run quick run command
- **After every plan wave:** Run full suite command
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 25 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 20-01-01 | 01 | 1 | DEP-01 | unit | `cargo test -p racecontrol-crate deploy::tests` | ✅ | ⬜ pending |
| 20-01-02 | 01 | 1 | DEP-02 | unit | `cargo test -p racecontrol-crate deploy::tests` | ✅ | ⬜ pending |
| 20-01-03 | 01 | 1 | DEP-02 | unit | `cargo test -p rc-common types::tests` | ✅ | ⬜ pending |
| 20-02-01 | 02 | 2 | DEP-03 | unit | `cargo test -p rc-agent-crate self_heal::tests` | ✅ | ⬜ pending |
| 20-02-02 | 02 | 2 | DEP-04 | unit | `cargo test -p racecontrol-crate deploy::tests` | ✅ | ⬜ pending |
| 20-02-03 | 02 | 2 | DEP-04 | integration | manual | N/A | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

Existing infrastructure covers all phase requirements. No new crates or test frameworks needed.

- [x] `crates/racecontrol/src/deploy.rs` — existing test file with 13+ tests
- [x] `crates/rc-common/src/types.rs` — existing serde tests
- [x] `crates/rc-agent/src/self_heal.rs` — existing test file with 8 tests

*All test infrastructure already exists.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| rc-agent-prev.exe exists after deploy | DEP-01 | Requires live pod deploy | Deploy to Pod 8, then run `dir C:\RacingPoint\rc-agent-prev.exe` via pod-agent |
| Bad binary triggers auto-rollback | DEP-02 | Requires deploying known-bad binary | Deploy a 6MB dummy .exe to Pod 8, verify rc-agent-prev.exe restored within 60s |
| Defender doesn't quarantine staging binary | DEP-03 | Requires Defender active on pod | Deploy to Pod 8, verify `dir C:\RacingPoint\rc-agent-new.exe` during download is not quarantined |
| Fleet deploy summary logged | DEP-04 | Requires multi-pod deploy | Trigger rolling deploy, check racecontrol logs for summary with per-pod status |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 25s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
