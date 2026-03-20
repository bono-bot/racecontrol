---
phase: 56
slug: whatsapp-alerting-weekly-report
status: draft
nyquist_compliant: true
wave_0_complete: false
created: 2026-03-20
---

# Phase 56 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust unit tests) |
| **Config file** | Cargo.toml (workspace) |
| **Quick run command** | `cargo test -p racecontrol whatsapp_alerter` |
| **Full suite command** | `cargo test -p racecontrol && cargo test -p weekly-report` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p racecontrol whatsapp_alerter`
- **After every plan wave:** Run `cargo test -p racecontrol && cargo test -p weekly-report`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 56-01-01 | 01 | 1 | MON-06 | unit | `cargo test -p racecontrol whatsapp_alerter` | W0 | pending |
| 56-01-02 | 01 | 1 | MON-06 | unit | `cargo test -p racecontrol whatsapp_alerter` | W0 | pending |
| 56-02-01 | 02 | 2 | MON-07 | unit | `cargo test -p weekly-report` | W0 | pending |
| 56-02-02 | 02 | 2 | MON-07 | integration | `cargo build --release --bin weekly-report` | W0 | pending |

*Status: pending / green / red / flaky*

---

## Wave 0 Requirements

- [ ] `crates/racecontrol/src/whatsapp_alerter.rs` — test module with rate_limit, ist_format, message_format tests
- [ ] `crates/weekly-report/src/main.rs` — test module with week_query, html_format tests

*Existing cargo test infrastructure covers all phase requirements.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| WhatsApp delivery to Uday's phone | MON-06 | Requires live Evolution API + real phone number | Stop racecontrol, verify WhatsApp arrives within 60s |
| Weekly email arrives Monday 08:00 IST | MON-07 | Requires Task Scheduler + live email delivery | Manually trigger schtasks, verify email in usingh@racingpoint.in inbox |

---

## Validation Sign-Off

- [x] All tasks have automated verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 15s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
