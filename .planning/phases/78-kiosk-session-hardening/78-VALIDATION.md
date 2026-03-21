---
phase: 78
slug: kiosk-session-hardening
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-21
---

# Phase 78 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust) + PowerShell registry verification |
| **Config file** | Cargo.toml (existing) |
| **Quick run command** | `cargo test -p rc-agent -- --test-threads=1 kiosk` |
| **Full suite command** | `cargo test -p racecontrol && cargo test -p rc-agent` |
| **Estimated runtime** | ~45 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo build --release --bin rc-agent 2>&1 | tail -5`
- **After every plan wave:** Run `cargo test -p rc-agent && cargo test -p racecontrol`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 45 seconds

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| F12/DevTools blocked on pod | KIOSK-01 | Requires physical pod | Press F12 on pod kiosk, verify nothing opens |
| USB storage rejected | KIOSK-03 | Requires USB device + pod | Plug USB stick into pod, verify not mounted |
| Sticky Keys disabled | KIOSK-04 | Requires physical pod | Press Shift 5 times on pod, verify no popup |
| Session end locks kiosk | SESS-04 | Requires active billing session | End session, verify kiosk locks within 10s |

---

## Validation Sign-Off

- [ ] All tasks have automated verify or are manual-only
- [ ] Feedback latency < 45s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
