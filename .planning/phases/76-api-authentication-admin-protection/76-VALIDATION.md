---
phase: 76
slug: api-authentication-admin-protection
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-20
---

# Phase 76 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust) + curl verification scripts |
| **Config file** | Cargo.toml (existing) |
| **Quick run command** | `cargo test -p racecontrol -- --test-threads=1 auth` |
| **Full suite command** | `cargo test -p racecontrol && cargo test -p rc-agent && cargo test -p rc-common` |
| **Estimated runtime** | ~45 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p racecontrol -- --test-threads=1`
- **After every plan wave:** Run `cargo test -p racecontrol && cargo test -p rc-agent`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 45 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 76-01-T1 | 01 | 1 | AUTH-01,02,03 | unit+integration | `cargo test -p racecontrol auth_middleware` | ❌ W0 | ⬜ pending |
| 76-02-T1 | 02 | 1 | ADMIN-01,02,03 | unit | `cargo test -p racecontrol admin_auth` | ❌ W0 | ⬜ pending |
| 76-03-T1 | 03 | 2 | AUTH-04 | unit | `cargo test -p racecontrol rate_limit` | ❌ W0 | ⬜ pending |
| 76-04-T1 | 04 | 2 | AUTH-05,SESS-01,02,03 | unit+integration | `cargo test -p racecontrol billing` | ❌ W0 | ⬜ pending |
| 76-05-T1 | 05 | 3 | AUTH-06 | unit | `cargo test -p rc-agent hmac` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Auth middleware unit tests (JWT extraction, tier rejection)
- [ ] Admin PIN hashing and verification tests
- [ ] Rate limiting threshold tests
- [ ] Billing session atomicity tests
- [ ] HMAC signature verification tests

*Tests created as part of TDD tasks within each plan.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Dashboard PIN gate UX | ADMIN-01 | Browser interaction | Load dashboard URL, verify PIN prompt appears before content |
| Bot wallet check | AUTH-05 | Requires Discord/WhatsApp message | Send bot command with zero balance, verify rejection |
| Pod 8 canary HMAC | AUTH-06 | Requires live pod | curl Pod 8 :8090/health without HMAC header, verify 401 |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 45s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
