---
phase: 84
slug: iracing-telemetry
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-21
---

# Phase 84 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (standard) |
| **Config file** | none — workspace Cargo.toml |
| **Quick run command** | `cargo test -p rc-agent-crate sims::iracing` |
| **Full suite command** | `cargo test -p rc-agent-crate && cargo test -p rc-common` |
| **Estimated runtime** | ~20 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p rc-agent-crate sims::iracing`
- **After every plan wave:** Run `cargo test -p rc-agent-crate && cargo test -p rc-common`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 20 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 84-01-01 | 01 | 1 | TEL-IR-01 | unit | `cargo test -p rc-agent-crate sims::iracing::tests::test_connect_no_shm` | W0 | pending |
| 84-01-01 | 01 | 1 | TEL-IR-02 | unit | `cargo test -p rc-agent-crate sims::iracing::tests::test_session_transition_resets_lap` | W0 | pending |
| 84-01-01 | 01 | 1 | TEL-IR-03 | unit | `cargo test -p rc-agent-crate sims::iracing::tests::test_lap_completed_event` | W0 | pending |
| 84-01-01 | 01 | 1 | TEL-IR-03 | unit | `cargo test -p rc-agent-crate sims::iracing::tests::test_first_packet_safety` | W0 | pending |
| 84-01-01 | 01 | 1 | TEL-IR-04 | unit | `cargo test -p rc-agent-crate sims::iracing::tests::test_preflight_missing_ini` | W0 | pending |
| 84-01-01 | 01 | 1 | TEL-IR-04 | unit | `cargo test -p rc-agent-crate sims::iracing::tests::test_preflight_ini_enabled` | W0 | pending |

*Status: pending*

---

## Wave 0 Requirements

- [ ] All 6 tests created as part of Plan 01 TDD task
- [ ] Tests use synthetic buffers (no iRacing process needed)
- [ ] Pre-flight tests use tempfile for app.ini

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Shared memory read with real iRacing | TEL-IR-01 | Requires iRacing running on pod | Launch iRacing, verify adapter connects and reads telemetry |
| Lap completion during live race | TEL-IR-03 | Requires actual race session | Complete a lap, verify LapCompleted emitted |

---

## Validation Sign-Off

- [ ] All tasks have automated verify or Wave 0 dependencies
- [ ] Sampling continuity maintained
- [ ] Wave 0 covers all MISSING references
- [ ] Feedback latency < 20s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
