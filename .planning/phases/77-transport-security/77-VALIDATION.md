---
phase: 77
slug: transport-security
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-20
---

# Phase 77 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust) + curl verification |
| **Config file** | Cargo.toml (existing) |
| **Quick run command** | `cargo test -p racecontrol -- --test-threads=1 tls` |
| **Full suite command** | `cargo test -p racecontrol && cargo test -p rc-agent` |
| **Estimated runtime** | ~45 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo build --release --bin racecontrol 2>&1 | tail -5`
- **After every plan wave:** Run `cargo test -p racecontrol`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 45 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | Status |
|---------|------|------|-------------|-----------|-------------------|--------|
| 77-01-T1 | 01 | 1 | TLS-02, TLS-04 | unit | `cargo test -p racecontrol tls` | ⬜ pending |
| 77-02-T1 | 02 | 1 | KIOSK-06 | unit | `cargo test -p racecontrol helmet` | ⬜ pending |
| 77-03-T1 | 03 | 2 | TLS-01, TLS-03 | manual | curl + browser check | ⬜ pending |

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| HTTPS loads in browser | TLS-01 | Browser trust check | Open https://192.168.31.23:8443 on kiosk, verify lock icon |
| Let's Encrypt on cloud | TLS-03 | External service | Check https://app.racingpoint.cloud cert validity |
| Security headers present | KIOSK-06 | Response inspection | `curl -I https://192.168.31.23:8443` check headers |

---

## Validation Sign-Off

- [ ] All tasks have automated verify or Wave 0 dependencies
- [ ] Feedback latency < 45s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
