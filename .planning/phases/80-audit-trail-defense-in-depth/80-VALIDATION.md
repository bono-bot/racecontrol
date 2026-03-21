---
phase: 80
slug: audit-trail-defense-in-depth
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-21
---

# Phase 80 — Validation Strategy

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust) |
| **Quick run command** | `cargo test -p racecontrol -- --test-threads=1 audit` |
| **Full suite command** | `cargo test -p racecontrol && cargo test -p rc-common` |
| **Estimated runtime** | ~45 seconds |

## Sampling Rate

- **After every task commit:** `cargo build --release --bin racecontrol 2>&1 | tail -5`
- **After every plan wave:** Full suite
- **Max feedback latency:** 45 seconds

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| WhatsApp alert on admin login | ADMIN-05 | Requires WhatsApp delivery | Login to dashboard, check Uday's WhatsApp |
| PIN rotation alert | ADMIN-06 | Requires 30-day wait or time mock | Set pin_changed_at to 31 days ago, restart server |

**Approval:** pending
