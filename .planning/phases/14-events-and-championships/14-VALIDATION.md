---
phase: 14
slug: events-and-championships
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-15
---

# Phase 14 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust `#[tokio::test]` (in-memory SQLite) in `crates/rc-core/tests/integration.rs` |
| **Config file** | Cargo.toml (auto-discovered `[[test]]`) |
| **Quick run command** | `export PATH="$PATH:/c/Users/bono/.cargo/bin" && cargo test -p rc-core test_event 2>&1 \| tail -20` |
| **Full suite command** | `export PATH="$PATH:/c/Users/bono/.cargo/bin" && cargo test -p rc-common && cargo test -p rc-agent && cargo test -p rc-core` |
| **Estimated runtime** | ~25 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p rc-core 2>&1 | tail -5`
- **After every plan wave:** Run full suite (rc-common + rc-agent + rc-core)
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 25 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 14-W0-01 | W0 | 0 | EVT-02 | unit | `cargo test -p rc-core test_auto_event_entry` | ❌ W0 | ⬜ pending |
| 14-W0-02 | W0 | 0 | EVT-02 | unit | `cargo test -p rc-core test_auto_entry_no_match` | ❌ W0 | ⬜ pending |
| 14-W0-03 | W0 | 0 | EVT-02 | unit | `cargo test -p rc-core test_auto_entry_date_range` | ❌ W0 | ⬜ pending |
| 14-W0-04 | W0 | 0 | EVT-02 | unit | `cargo test -p rc-core test_auto_entry_faster_lap` | ❌ W0 | ⬜ pending |
| 14-W0-05 | W0 | 0 | EVT-02 | unit | `cargo test -p rc-core test_auto_entry_no_replace_slower` | ❌ W0 | ⬜ pending |
| 14-W0-06 | W0 | 0 | EVT-05 | unit | `cargo test -p rc-core test_107_percent_rule` | ❌ W0 | ⬜ pending |
| 14-W0-07 | W0 | 0 | EVT-05 | unit | `cargo test -p rc-core test_107_boundary` | ❌ W0 | ⬜ pending |
| 14-W0-08 | W0 | 0 | EVT-06 | unit | `cargo test -p rc-core test_badge_gold` | ❌ W0 | ⬜ pending |
| 14-W0-09 | W0 | 0 | EVT-06 | unit | `cargo test -p rc-core test_badge_silver` | ❌ W0 | ⬜ pending |
| 14-W0-10 | W0 | 0 | EVT-06 | unit | `cargo test -p rc-core test_badge_bronze` | ❌ W0 | ⬜ pending |
| 14-W0-11 | W0 | 0 | EVT-06 | unit | `cargo test -p rc-core test_badge_no_reference` | ❌ W0 | ⬜ pending |
| 14-W0-12 | W0 | 0 | GRP-01 | unit | `cargo test -p rc-core test_f1_points_scoring` | ❌ W0 | ⬜ pending |
| 14-W0-13 | W0 | 0 | GRP-01 | unit | `cargo test -p rc-core test_dns_dnf_zero_points` | ❌ W0 | ⬜ pending |
| 14-W0-14 | W0 | 0 | GRP-04 | unit | `cargo test -p rc-core test_gap_to_leader` | ❌ W0 | ⬜ pending |
| 14-W0-15 | W0 | 0 | CHP-02 | unit | `cargo test -p rc-core test_championship_standings_sum` | ❌ W0 | ⬜ pending |
| 14-W0-16 | W0 | 0 | CHP-04 | unit | `cargo test -p rc-core test_championship_tiebreaker_wins` | ❌ W0 | ⬜ pending |
| 14-W0-17 | W0 | 0 | CHP-04 | unit | `cargo test -p rc-core test_championship_tiebreaker_p2` | ❌ W0 | ⬜ pending |
| 14-W0-18 | W0 | 0 | SYNC-01 | unit | `cargo test -p rc-core test_sync_competitive_tables` | ❌ W0 | ⬜ pending |
| 14-W0-19 | W0 | 0 | SYNC-02 | unit | `cargo test -p rc-core test_sync_targeted_telemetry` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/rc-core/tests/integration.rs` — add `test_auto_event_entry` (EVT-02 happy path: matching lap auto-enters event)
- [ ] `crates/rc-core/tests/integration.rs` — add `test_auto_entry_no_match` (wrong car_class, no entry)
- [ ] `crates/rc-core/tests/integration.rs` — add `test_auto_entry_date_range` (expired event, no entry)
- [ ] `crates/rc-core/tests/integration.rs` — add `test_auto_entry_faster_lap` (replace on faster)
- [ ] `crates/rc-core/tests/integration.rs` — add `test_auto_entry_no_replace_slower` (keep existing best)
- [ ] `crates/rc-core/tests/integration.rs` — add `test_107_percent_rule` + `test_107_boundary`
- [ ] `crates/rc-core/tests/integration.rs` — add `test_badge_gold`, `test_badge_silver`, `test_badge_bronze`, `test_badge_no_reference`
- [ ] `crates/rc-core/tests/integration.rs` — add `test_f1_points_scoring` + `test_dns_dnf_zero_points`
- [ ] `crates/rc-core/tests/integration.rs` — add `test_gap_to_leader`
- [ ] `crates/rc-core/tests/integration.rs` — add `test_championship_standings_sum`
- [ ] `crates/rc-core/tests/integration.rs` — add `test_championship_tiebreaker_wins` + `test_championship_tiebreaker_p2`
- [ ] `crates/rc-core/tests/integration.rs` — add `test_sync_competitive_tables` + `test_sync_targeted_telemetry`
- [ ] `crates/rc-core/src/db/mod.rs` — verify `run_test_migrations()` includes `group_sessions.hotlap_event_id` ALTER TABLE
- [ ] Confirm whether `multiplayer_results` table exists in production DB; add migration if absent

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Public event pages accessible without auth | EVT-07, PUB-01 | Requires running HTTP server | `curl http://localhost:8080/public/events` — expect 200 |
| Cloud sync reflects within 60s | SYNC-01 | Requires two running servers | Create event locally, wait 60s, verify on app.racingpoint.cloud |
| Mobile layout for events/championships | PUB-02 | Visual/responsive check | Chrome DevTools mobile 375px, check table readability |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 25s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
