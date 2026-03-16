---
phase: 13
slug: leaderboard-core
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-15
---

# Phase 13 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust `#[tokio::test]` (in-memory SQLite) in `crates/racecontrol/tests/integration.rs` |
| **Config file** | Cargo.toml (auto-discovered `[[test]]`) |
| **Quick run command** | `export PATH="$PATH:/c/Users/bono/.cargo/bin" && cargo test -p racecontrol-crate test_leaderboard 2>&1 \| tail -20` |
| **Full suite command** | `export PATH="$PATH:/c/Users/bono/.cargo/bin" && cargo test -p rc-common && cargo test -p rc-agent-crate && cargo test -p racecontrol-crate` |
| **Estimated runtime** | ~20 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p racecontrol-crate 2>&1 | tail -5`
- **After every plan wave:** Run full suite (rc-common + rc-agent + racecontrol)
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 20 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 13-W0-01 | W0 | 0 | LB-01, LB-04 | unit | `cargo test -p racecontrol-crate test_leaderboard_sim_type_filter` | ❌ W0 | ⬜ pending |
| 13-W0-02 | W0 | 0 | LB-05 | unit | `cargo test -p racecontrol-crate test_lap_suspect` | ❌ W0 | ⬜ pending |
| 13-W0-03 | W0 | 0 | LB-06 | unit | `cargo test -p racecontrol-crate test_leaderboard_invalid_toggle` | ❌ W0 | ⬜ pending |
| 13-W0-04 | W0 | 0 | LB-02 | unit | `cargo test -p racecontrol-crate test_circuit_records` | ❌ W0 | ⬜ pending |
| 13-W0-05 | W0 | 0 | LB-03 | unit | `cargo test -p racecontrol-crate test_vehicle_records` | ❌ W0 | ⬜ pending |
| 13-W0-06 | W0 | 0 | DRV-01 | unit | `cargo test -p racecontrol-crate test_driver_search` | ❌ W0 | ⬜ pending |
| 13-W0-07 | W0 | 0 | DRV-02 | unit | `cargo test -p racecontrol-crate test_public_driver_no_pii` | ❌ W0 | ⬜ pending |
| 13-W0-08 | W0 | 0 | NTF-02 | unit | `cargo test -p racecontrol-crate test_notification_data_before_upsert` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/racecontrol/tests/integration.rs` — add `test_leaderboard_sim_type_filter` covering LB-01, LB-04
- [ ] `crates/racecontrol/tests/integration.rs` — add `test_lap_suspect_sector_sum` and `test_lap_suspect_sanity` covering LB-05
- [ ] `crates/racecontrol/tests/integration.rs` — add `test_leaderboard_invalid_toggle` covering LB-06
- [ ] `crates/racecontrol/tests/integration.rs` — add `test_circuit_records` covering LB-02
- [ ] `crates/racecontrol/tests/integration.rs` — add `test_vehicle_records` covering LB-03
- [ ] `crates/racecontrol/tests/integration.rs` — add `test_driver_search` covering DRV-01
- [ ] `crates/racecontrol/tests/integration.rs` — add `test_public_driver_no_pii` covering DRV-02
- [ ] `crates/racecontrol/tests/integration.rs` — add `test_notification_data_before_upsert` covering NTF-02
- [ ] `crates/racecontrol/src/db/mod.rs` — `suspect` column in `run_test_migrations()` mirroring production

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Public endpoints accessible without auth | DRV-04, PUB-01 | Requires running HTTP server | `curl http://localhost:8080/public/leaderboard/monza?sim_type=ac` — expect 200 |
| Mobile layout at 375px | PUB-02 | Visual/responsive check | Open PWA in Chrome DevTools mobile emulation at 375px, check table readability |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 20s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
