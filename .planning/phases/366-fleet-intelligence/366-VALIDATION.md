---
phase: 366
slug: fleet-intelligence
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-10
---

# Phase 366 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in `cargo test` + `#[tokio::test]` |
| **Config file** | `crates/racecontrol/Cargo.toml` |
| **Quick run command** | `cargo test -p racecontrol --lib fleet 2>&1 \| tail -10` |
| **Full suite command** | `cargo test -p racecontrol 2>&1 \| tail -10` |
| **Estimated runtime** | ~45 seconds (existing suite is 891 tests) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p racecontrol --lib fleet 2>&1 | tail -10`
- **After every plan wave:** Run `cargo test -p racecontrol 2>&1 | tail -10`
- **Before `/gsd:verify-work`:** Full suite must be green (891+ tests passing)
- **Max feedback latency:** 45 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 366-01-01 | 01 | 1 | GLD-F-01 | unit | `cargo test -p racecontrol fleet_intelligence 2>&1 \| tail -5` | ❌ W0 | pending |
| 366-01-02 | 01 | 1 | GLD-F-01 | unit | `cargo test -p racecontrol fleet_intelligence::test_insufficient_data 2>&1 \| tail -5` | ❌ W0 | pending |
| 366-02-01 | 02 | 1 | GLD-F-02 | unit | `cargo test -p racecontrol fleet_intelligence::test_time_of_day 2>&1 \| tail -5` | ❌ W0 | pending |
| 366-03-01 | 03 | 2 | GLD-F-03 | unit | `cargo test -p racecontrol content_drift 2>&1 \| tail -5` | ❌ W0 | pending |
| 366-04-01 | 04 | 2 | GLD-F-04 | unit | `cargo test -p racecontrol concurrent_session 2>&1 \| tail -5` | ❌ W0 | pending |
| 366-04-02 | 04 | 2 | GLD-F-04 | unit | `cargo test -p racecontrol concurrent_session::test_game_launch_409 2>&1 \| tail -5` | ❌ W0 | pending |

*Status: pending · green · red · flaky*

---

## Wave 0 Requirements

- [ ] `crates/racecontrol/src/fleet_intelligence.rs` — new module with unit tests (GLD-F-01, GLD-F-02)
- [ ] `crates/racecontrol/src/content_drift.rs` — new module with unit tests (GLD-F-03)
- [ ] Tests use existing `#[tokio::test]` + in-memory SQLite (`sqlx::SqlitePool::connect(":memory:")`) pattern from Phase 363 tests

*Existing test infrastructure (`cargo test`) covers all phase requirements — no new framework install needed.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| GET /fleet/intelligence returns 200 with pod scores | GLD-F-01 | Requires live server + DB with sessions | `curl -H "Authorization: Bearer <staff_jwt>" http://localhost:3000/api/v1/fleet/intelligence` |
| Parallel curl smoke test: 2 concurrent billing starts return 1x200 + 1x409 | GLD-F-04 | Race timing, requires live server | `curl -X POST ... & curl -X POST ...` — verify one 409 |
| ContentDriftDetected fires when car removed | GLD-F-03 | Requires pod with rc-agent + TOML mutation | Edit TOML to remove a car, wait 60s for poll |

---

## Validation Sign-Off

- [ ] All tasks have automated unit tests or manual verification instructions
- [ ] Wave 0 gaps documented above
- [ ] Full test suite passes before /gsd:verify-work
- [ ] `nyquist_compliant: true` set in frontmatter after Wave 0 complete

**Approval:** pending
