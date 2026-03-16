---
phase: 33
slug: db-schema-billing-engine
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-17
---

# Phase 33 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in test + sqlx integration tests |
| **Config file** | Cargo.toml (workspace) |
| **Quick run command** | `export PATH="$PATH:/c/Users/bono/.cargo/bin" && cargo test -p rc-common` |
| **Full suite command** | `export PATH="$PATH:/c/Users/bono/.cargo/bin" && cargo test -p rc-common && cargo test -p racecontrol-crate` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p rc-common`
- **After every plan wave:** Run full suite (rc-common + racecontrol-crate)
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** ~30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 33-01-01 | 01 | 1 | PROTOC-01/02 | unit | `cargo test -p rc-common` | ✅ existing | ⬜ pending |
| 33-01-02 | 01 | 1 | RATE-01/02/03 | integration | `cargo test -p racecontrol-crate` | ✅ existing | ⬜ pending |
| 33-01-03 | 01 | 1 | BILLC-02/03/04 | unit | `cargo test -p racecontrol-crate billing` | ✅ existing | ⬜ pending |
| 33-01-04 | 01 | 1 | BILLC-05 | unit | `cargo test -p racecontrol-crate billing` | ✅ existing | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Seed capitalization fix verified (db/mod.rs `'Standard'` not `'standard'`)
- [ ] Alias round-trip test added for `minutes_to_value_tier` → `minutes_to_next_tier`
- [ ] `test_db_setup` asserts billing_rates seed count == 3

*All three gaps are small fixes to existing test/seed code — no new test infrastructure needed.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| GET /billing/rates returns 3 seed rows after startup | RATE-02 | Requires live server | Start racecontrol, hit `curl localhost:8080/billing/rates`, assert 3 rows |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
