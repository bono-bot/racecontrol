---
phase: 54
slug: structured-logging-error-rate-alerting
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-20
---

# Phase 54 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust unit tests) |
| **Config file** | Cargo.toml workspace |
| **Quick run command** | `cargo test -p racecontrol-crate -p rc-agent-crate 2>&1 \| tail -5` |
| **Full suite command** | `cargo test --workspace` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** `cargo check --workspace` (fast compilation check)
- **After every plan wave:** `cargo test --workspace`
- **Before `/gsd:verify-work`:** Full test + deploy to server, verify JSON log output
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 54-01-01 | 01 | 1 | MON-01 | build+grep | `cargo check -p racecontrol-crate && grep "json" Cargo.toml` | ✅ | ⬜ pending |
| 54-02-01 | 02 | 1 | MON-02 | build+grep | `cargo check -p rc-agent-crate && grep "json" crates/rc-agent/src/main.rs` | ✅ | ⬜ pending |
| 54-03-01 | 03 | 2 | MON-03 | build+test | `cargo test -p racecontrol-crate` | ✅ | ⬜ pending |

---

## Wave 0 Requirements

*Existing test infrastructure covers all phase requirements. cargo test already runs for both crates.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| JSON log file appears after restart | MON-01/02 | Requires restarting services on server/pod | Restart racecontrol, check `logs/racecontrol-YYYY-MM-DD.jsonl` has JSON entries |
| Error rate email fires | MON-03 | Requires triggering 5 errors in 1 minute on live server | Generate errors, verify email arrives within 2 minutes |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
