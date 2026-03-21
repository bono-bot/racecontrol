---
phase: 3
slug: sync-hardening
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-21
---

# Phase 3 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (built-in Rust test framework) |
| **Config file** | Cargo.toml workspace test config |
| **Quick run command** | `cargo test -p racecontrol --lib cloud_sync` |
| **Full suite command** | `cargo test -p rc-common && cargo test -p racecontrol` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p racecontrol --lib cloud_sync`
- **After every plan wave:** Run `cargo test -p rc-common && cargo test -p racecontrol`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Req ID | Behavior | Test Type | Automated Command | File Exists | Status |
|--------|----------|-----------|-------------------|-------------|--------|
| SYNC-01 | Reservation upserted on sync pull | unit | `cargo test -p racecontrol -- upsert_reservation` | No | Wave 0 |
| SYNC-02 | Debit intent processed correctly | unit | `cargo test -p racecontrol -- process_debit_intent` | No | Wave 0 |
| SYNC-02 | Insufficient balance fails intent | unit | `cargo test -p racecontrol -- debit_intent_insufficient` | No | Wave 0 |
| SYNC-03 | Same-origin payload rejected | unit | `cargo test -p racecontrol -- origin_tag_reject` | No | Wave 0 |
| SYNC-04 | Sync health returns lag_seconds | unit | `cargo test -p racecontrol -- sync_health_lag` | No | Wave 0 |
| SYNC-06 | Admin tables sync correctly | integration | `cargo test -p racecontrol -- sync_admin_tables` | No | Wave 0 |
| SYNC-07 | Sync health endpoint returns expected fields | unit | `cargo test -p racecontrol -- sync_health_endpoint` | No | Wave 0 |

---

## Wave 0 Gaps

- [ ] Test helper: in-memory SQLite pool factory for sync unit tests
- [ ] `tests/sync_hardening.rs` — covers SYNC-01 through SYNC-07
- [ ] Mock HTTP responses for cloud API calls during testing

---

## Coverage Targets

| Requirement | Tests Required | Tests Exist | Gap |
|-------------|---------------|-------------|-----|
| SYNC-01 | 1 | 0 | 1 |
| SYNC-02 | 2 | 0 | 2 |
| SYNC-03 | 1 | 0 | 1 |
| SYNC-04 | 1 | 0 | 1 |
| SYNC-06 | 1 | 0 | 1 |
| SYNC-07 | 1 | 0 | 1 |
| **Total** | **7** | **0** | **7** |

---

*Phase: 03-sync-hardening*
*Validation strategy created: 2026-03-21*
