---
phase: 75
slug: security-audit-foundations
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-20
---

# Phase 75 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust) + shell verification scripts |
| **Config file** | Cargo.toml (existing) |
| **Quick run command** | `cargo test -p racecontrol -- --test-threads=1 config` |
| **Full suite command** | `cargo test -p racecontrol && cargo test -p rc-agent` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p racecontrol -- --test-threads=1 config`
- **After every plan wave:** Run `cargo test -p racecontrol && cargo test -p rc-agent`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 75-01-01 | 01 | 1 | AUDIT-01 | shell | `grep -c "fn " crates/racecontrol/src/api/routes.rs` | N/A | ⬜ pending |
| 75-01-02 | 01 | 1 | AUDIT-02 | shell | `grep -rn "phone\|email\|name" crates/racecontrol/src/` | N/A | ⬜ pending |
| 75-01-03 | 01 | 1 | AUDIT-05 | shell | `grep -n "cors\|CorsLayer" crates/racecontrol/src/` | N/A | ⬜ pending |
| 75-02-01 | 02 | 2 | AUDIT-03 | unit | `cargo test -p racecontrol config` | ❌ W0 | ⬜ pending |
| 75-02-02 | 02 | 2 | AUDIT-04 | unit | `cargo test -p racecontrol jwt_key` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Config test for env var override of JWT secret
- [ ] Config test for auto-generated random JWT key

*Existing cargo test infrastructure covers compilation verification.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Route classification doc is complete | AUDIT-01 | Document review | Verify .planning/phases/75-security-audit-foundations/SECURITY-AUDIT.md lists all routes |
| PII audit doc is complete | AUDIT-02 | Document review | Verify SECURITY-AUDIT.md lists all PII locations |
| CORS/HTTPS state documented | AUDIT-05 | Document review | Verify SECURITY-AUDIT.md documents current state per service |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
