---
phase: 12
slug: data-foundation
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-14
---

# Phase 12 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in test framework + tokio-test |
| **Config file** | Cargo.toml (workspace) |
| **Quick run command** | `export PATH="$PATH:/c/Users/bono/.cargo/bin" && cargo test -p racecontrol-crate -- db 2>&1 \| tail -20` |
| **Full suite command** | `export PATH="$PATH:/c/Users/bono/.cargo/bin" && cargo test -p rc-common && cargo test -p rc-agent-crate && cargo test -p racecontrol-crate` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p racecontrol-crate -- db 2>&1 | tail -20`
- **After every plan wave:** Run full suite (rc-common + rc-agent + racecontrol)
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 12-01-01 | 01 | 0 | DATA-01 | unit | `cargo test -p racecontrol-crate -- test_leaderboard_index_exists` | ❌ W0 | ⬜ pending |
| 12-01-02 | 01 | 0 | DATA-02 | unit | `cargo test -p racecontrol-crate -- test_telemetry_index_exists` | ❌ W0 | ⬜ pending |
| 12-01-03 | 01 | 0 | DATA-03 | unit | `cargo test -p racecontrol-crate -- test_wal_tuning` | ❌ W0 | ⬜ pending |
| 12-01-04 | 01 | 0 | DATA-04 | unit | `cargo test -p racecontrol-crate -- test_cloud_driver_id_column` | ❌ W0 | ⬜ pending |
| 12-01-05 | 01 | 0 | DATA-05 | unit | `cargo test -p racecontrol-crate -- test_competitive_tables_exist` | ❌ W0 | ⬜ pending |
| 12-01-06 | 01 | 0 | DATA-06 | unit | `cargo test -p racecontrol-crate -- test_lap_car_class_populated` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/racecontrol/tests/integration.rs` — add `test_leaderboard_index_exists`: EXPLAIN QUERY PLAN leaderboard query, assert "idx_laps_leaderboard"
- [ ] `crates/racecontrol/tests/integration.rs` — add `test_telemetry_index_exists`: EXPLAIN telemetry query, assert "idx_telemetry_lap_offset"
- [ ] `crates/racecontrol/tests/integration.rs` — add `test_wal_tuning`: PRAGMA wal_autocheckpoint, assert 400
- [ ] `crates/racecontrol/tests/integration.rs` — add `test_cloud_driver_id_column`: INSERT into drivers with cloud_driver_id, assert success
- [ ] `crates/racecontrol/tests/integration.rs` — add `test_competitive_tables_exist`: INSERT into all six tables, assert no error
- [ ] `crates/racecontrol/tests/integration.rs` — add `test_lap_car_class_populated`: persist_lap with seeded billing_session, assert car_class set
- [ ] `run_test_migrations()` update — include car_class in laps table + all six competitive tables

---

## Manual-Only Verifications

*All phase behaviors have automated verification.*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
