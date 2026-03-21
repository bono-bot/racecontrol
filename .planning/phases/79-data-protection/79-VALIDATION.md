---
phase: 79
slug: data-protection
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-21
---

# Phase 79 — Validation Strategy

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust) |
| **Quick run command** | `cargo test -p racecontrol -- --test-threads=1 crypto` |
| **Full suite command** | `cargo test -p racecontrol && cargo test -p rc-agent && cargo test -p rc-common` |
| **Estimated runtime** | ~45 seconds |

## Sampling Rate

- **After every task commit:** `cargo build --release --bin racecontrol 2>&1 | tail -5`
- **After every plan wave:** Full suite
- **Max feedback latency:** 45 seconds

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| SQLite shows encrypted values | DATA-01 | Requires running server | `sqlite3 racecontrol.db "SELECT phone_enc FROM drivers LIMIT 1"` — should be base64 blob |
| OTP login still works | DATA-02 | Requires phone + OTP flow | Start session via PWA, verify OTP arrives and login succeeds |
| Logs clean of PII | DATA-03 | Requires log inspection | `grep -rn "@\|+91" logs/` — should return 0 matches |

## Validation Sign-Off

- [ ] All tasks have automated verify
- [ ] Feedback latency < 45s

**Approval:** pending
