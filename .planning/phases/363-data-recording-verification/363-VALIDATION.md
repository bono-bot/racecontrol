---
phase: 363
slug: data-recording-verification
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-09
---

# Phase 363 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Source: `363-RESEARCH.md` §"Validation Architecture" (lines 555-596).

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust `#[cfg(test)]` inline modules + `#[tokio::test]` for async |
| **Config file** | None — inline modules in each .rs file |
| **Quick run command** | `cargo test -p racecontrol -- billing 2>&1 \| tail -20` |
| **Full suite command** | `cargo test -p racecontrol && cargo test -p rc-common && cargo test -p rc-agent` |
| **Estimated runtime** | ~90 seconds (quick: ~15s, full: ~90s) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p racecontrol -- billing 2>&1 | tail -20`
- **After every plan wave:** Run `cargo test -p racecontrol && cargo test -p rc-common && cargo test -p rc-agent`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 90 seconds

---

## Per-Task Verification Map

> Tasks will be emitted by `gsd-planner` into PLAN.md files. This map is the requirement→test contract; the planner must bind each test row to a specific task ID during planning.

| Req ID | Behavior | Test Type | Automated Command | File Exists | Status |
|--------|----------|-----------|-------------------|-------------|--------|
| GLD-C-01 | Lap heuristic: 30min trackday → expect 10 laps; 0 actual → UNDER_RECORDED | unit | `cargo test -p racecontrol -- session_audit::test_lap_heuristic` | ❌ W0 | ⬜ pending |
| GLD-C-01 | Lap audit: 9 laps in 30min (>10% gap) → UNDER_RECORDED | unit | `cargo test -p racecontrol -- session_audit::test_lap_audit_under_recorded` | ❌ W0 | ⬜ pending |
| GLD-C-01 | Lap audit: fast driver, 12 laps in 30min → OK (directional, no over-flag) | unit | `cargo test -p racecontrol -- session_audit::test_lap_audit_ok_over_expected` | ❌ W0 | ⬜ pending |
| GLD-C-01 | Crash path: session ended before audit → UNVERIFIED preserved | integration | `cargo test -p racecontrol -- session_audit::test_crash_unverified` | ❌ W0 | ⬜ pending |
| GLD-C-02 | Coverage: 1800s session, 1200s covered → 66.7% → suspect=true | unit | `cargo test -p racecontrol -- session_audit::test_telemetry_coverage_suspect` | ❌ W0 | ⬜ pending |
| GLD-C-02 | Coverage: 1800s session, 1500s covered → 83% → suspect=false | unit | `cargo test -p racecontrol -- session_audit::test_telemetry_coverage_ok` | ❌ W0 | ⬜ pending |
| GLD-C-02 | suspect_reasons JSON array emitted when multiple flags fire | unit | `cargo test -p racecontrol -- session_audit::test_suspect_reasons_multi` | ❌ W0 | ⬜ pending |
| GLD-C-03 | CSV fallback: file has content → POST fired on SessionEnded | integration (mock server) | `cargo test -p rc-agent -- csv_fallback::test_push_on_session_end` | ❌ W0 | ⬜ pending |
| GLD-C-03 | CSV fallback: file empty → no POST | unit | `cargo test -p rc-agent -- csv_fallback::test_no_push_when_empty` | ❌ W0 | ⬜ pending |
| GLD-C-03 | CSV fallback: clear_csv_laps only after confirmed 200 | integration (mock server) | `cargo test -p rc-agent -- csv_fallback::test_no_clear_on_failure` | ❌ W0 | ⬜ pending |
| GLD-C-03 | Server endpoint: POST /api/v1/sessions/{id}/telemetry-fallback requires service key | integration | `cargo test -p racecontrol -- telemetry_fallback::test_requires_service_key` | ❌ W0 | ⬜ pending |
| GLD-C-03 | Server endpoint: writes csv_fallback_received_at on 200 | integration (SQLite) | `cargo test -p racecontrol -- telemetry_fallback::test_receipt_timestamp` | ❌ W0 | ⬜ pending |
| GLD-C-04 | Grace window: lap reject arrives within 5s → grace_window_caught=true, lap removed before finalize | integration (SQLite) | `cargo test -p racecontrol -- billing_grace::test_grace_window_catches_reject` | ❌ W0 | ⬜ pending |
| GLD-C-04 | Grace window: no lap reject in 5s → finalize proceeds with original count | integration (SQLite) | `cargo test -p racecontrol -- billing_grace::test_grace_window_expires_normally` | ❌ W0 | ⬜ pending |
| GLD-C-04 | Grace window: server restart mid-window → next tick resumes from DB (lap_reject_grace_until) | integration (SQLite) | `cargo test -p racecontrol -- billing_grace::test_grace_window_restart_safe` | ❌ W0 | ⬜ pending |
| F-05 regression | end_billing_session early-end: refund uses ORIGINAL debit, not overwritten value | integration (SQLite) | `cargo test -p racecontrol -- billing::test_end_billing_session_early_end_refund_amount` | ❌ W0 | ⬜ pending |
| F-05 regression | Rs.700 30min session ends at 15min → refund Rs.350 (not Rs.187.50) | integration (SQLite) | `cargo test -p racecontrol -- billing::test_f05_refund_uses_original_debit` | ❌ W0 | ⬜ pending |
| GLD-C-01..04 | Cloud sync payload: all 8 new billing_sessions columns present in upsert | integration | `cargo test -p racecontrol -- cloud_sync::test_billing_session_push_columns_phase363` | ❌ W0 | ⬜ pending |
| GLD-C-01..04 | Feature flag: phase363_session_audit=false bypasses all new audit paths | integration | `cargo test -p racecontrol -- session_audit::test_feature_flag_kill_switch` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/racecontrol/src/session_audit.rs` — new module hosting lap heuristic + coverage + audit orchestration + `#[cfg(test)]` block for GLD-C-01/C-02
- [ ] `crates/racecontrol/src/billing.rs` — extend existing `#[cfg(test)]` block (~line 4420-4504 uses `db::test_pool()`) with GLD-C-04 + F-05 regression tests
- [ ] `crates/racecontrol/src/cloud_sync.rs` — `#[cfg(test)]` block asserting all 8 new column names appear in the billing_sessions push payload
- [ ] `crates/rc-agent/src/csv_lap_fallback.rs` — `#[cfg(test)]` block for push_on_session_end / no_push_when_empty / no_clear_on_failure (use mock HTTP server — confirm crate available, else use `hyper::server` directly)
- [ ] `crates/racecontrol/src/api/routes.rs` — integration test module for new POST /api/v1/sessions/{id}/telemetry-fallback endpoint (service key gate + SQLite receipt)
- [ ] Reuse `db::test_pool()` from `db/mod.rs` for all SQLite in-memory integration tests (pattern already established at billing.rs:4420-4504)

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| End-to-end refund trace (customer → topup → 30min → end at 15min → verify ₹350 refund) | F-05 + GLD-C-04 | CLAUDE.md §"Financial flow E2E" mandates live-value tracing before shipping billing changes | (1) Create test customer with ₹1000 balance; (2) book 30min/₹700 session on Pod 8; (3) end session at 15min mark; (4) verify customer balance = ₹1000 − ₹350 = ₹650; (5) verify no Rs.162.50 ghost loss |
| Cloud parity: Bono VPS DB has all 8 new columns after deploy | GLD-C-01..04 | Post-deploy schema drift can only be observed on the remote DB | After cloud `git_pull + cargo build + pm2 restart`: `ssh bono-vps "sqlite3 /root/racingpoint/racecontrol.db '.schema billing_sessions'"` and grep for lap_count_flag, telemetry_coverage_pct, suspect, suspect_reasons, csv_fallback_received_at, lap_reject_grace_until, lap_count_expected, lap_count_actual |
| Feature flag kill switch: set phase363_session_audit=false → new columns stay NULL for new sessions | Kill switch | Requires a real session on a real pod with the flag toggled live | Toggle flag via admin, run a session on Pod 8, verify billing_sessions row for that session has lap_count_flag=UNVERIFIED and telemetry_coverage_pct=NULL |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references (6 items above)
- [ ] No watch-mode flags in test commands
- [ ] Feedback latency < 90s
- [ ] `nyquist_compliant: true` set in frontmatter (gsd-planner responsibility after task binding)

**Approval:** pending
