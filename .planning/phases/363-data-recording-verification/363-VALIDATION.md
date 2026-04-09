---
phase: 363
slug: data-recording-verification
status: draft
nyquist_compliant: true
wave_0_complete: true
created: 2026-04-09
revised: 2026-04-09
---

# Phase 363 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Source: `363-RESEARCH.md` §"Validation Architecture" (lines 555-596).
>
> **Revision 1 (2026-04-09):** Updated test module paths. Grace window tests live in a
> dedicated `mod billing_grace` submodule inside billing.rs (so `billing_grace::` is the
> cargo test filter prefix). F-05 regression tests live in the main `mod tests` block as
> `billing::tests::`. Nyquist compliance confirmed — every requirement row has a real
> `<automated>` command bound to a task in the PLAN files. Wave 0 is folded into Wave 1
> tasks (each task creates its own test files TDD-style before implementation).

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust `#[cfg(test)]` inline modules + `#[tokio::test]` for async |
| **Config file** | None — inline modules in each .rs file |
| **Quick run command** | `cargo test -p racecontrol -- billing 2>&1 \| tail -20` |
| **Full suite command** | `cargo test -p racecontrol && cargo test -p rc-common && cargo test -p rc-agent` |
| **Estimated runtime** | ~90 seconds (quick: ~15s, full: ~90s) |

### Test Module Layout

| File | Module path | Tests |
|------|-------------|-------|
| `crates/racecontrol/src/session_audit.rs` | `session_audit::tests` | All GLD-C-01/C-02 tests (pure + integration) |
| `crates/racecontrol/src/billing.rs` | `billing::tests` | F-05 regression + coverage histogram + lap_rejections INSERT tests |
| `crates/racecontrol/src/billing.rs` | `billing_grace` (NEW submodule inside billing.rs) | All 3 grace window tests — matches `billing_grace::` cargo filter prefix |
| `crates/racecontrol/src/cloud_sync.rs` | `cloud_sync::tests` | Phase 363 payload column presence |
| `crates/racecontrol/src/api/routes.rs` | `telemetry_fallback` submodule | GLD-C-03 server endpoint tests |
| `crates/rc-agent/src/csv_lap_fallback.rs` | `csv_fallback` submodule | GLD-C-03 rc-agent push tests |
| `crates/racecontrol/src/db/mod.rs` | `phase363_migration_tests` | Migration + schema tests |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p racecontrol -- billing 2>&1 | tail -20`
- **After every plan wave:** Run `cargo test -p racecontrol && cargo test -p rc-common && cargo test -p rc-agent`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 90 seconds

---

## Per-Task Verification Map

> Tasks are emitted by `gsd-planner` into PLAN.md files. This map is the requirement→test contract; each test row is bound to a specific task in the plans.

| Req ID | Behavior | Test Type | Automated Command | Plan/Task | Status |
|--------|----------|-----------|-------------------|-----------|--------|
| GLD-C-01 | Lap heuristic: 30min trackday → expect 10 laps; 0 actual → UNDER_RECORDED | unit | `cargo test -p racecontrol -- session_audit::tests::test_lap_heuristic` | 363-01 / Task 2 | ⬜ pending |
| GLD-C-01 | Lap audit: 9 laps in 30min (>10% gap) → UNDER_RECORDED | unit | `cargo test -p racecontrol -- session_audit::tests::test_lap_audit_under_recorded` | 363-01 / Task 2 | ⬜ pending |
| GLD-C-01 | Lap audit: fast driver, 12 laps in 30min → OK (directional, no over-flag) | unit | `cargo test -p racecontrol -- session_audit::tests::test_lap_audit_ok_over_expected` | 363-01 / Task 2 | ⬜ pending |
| GLD-C-01 | Crash path: session ended before audit → UNVERIFIED preserved | integration | `cargo test -p racecontrol -- session_audit::tests::test_crash_unverified` | 363-01 / Task 2 | ⬜ pending |
| GLD-C-02 | Coverage: 1800s session, 1200s covered → 66.7% → suspect=true | unit | `cargo test -p racecontrol -- session_audit::tests::test_telemetry_coverage_suspect` | 363-01 / Task 2 | ⬜ pending |
| GLD-C-02 | Coverage: 1800s session, 1500s covered → 83% → suspect=false | unit | `cargo test -p racecontrol -- session_audit::tests::test_telemetry_coverage_ok` | 363-01 / Task 2 | ⬜ pending |
| GLD-C-02 | suspect_reasons JSON array emitted when multiple flags fire | unit | `cargo test -p racecontrol -- session_audit::tests::test_suspect_reasons_multi` | 363-01 / Task 2 | ⬜ pending |
| GLD-C-03 | CSV fallback: file has content → POST fired on SessionEnded | integration (mock server) | `cargo test -p rc-agent -- csv_fallback::test_push_on_session_end` | 363-02 / Task 2 | ⬜ pending |
| GLD-C-03 | CSV fallback: file empty → no POST | unit | `cargo test -p rc-agent -- csv_fallback::test_no_push_when_empty` | 363-02 / Task 2 | ⬜ pending |
| GLD-C-03 | CSV fallback: clear_csv_laps only after confirmed 200 | integration (mock server) | `cargo test -p rc-agent -- csv_fallback::test_no_clear_on_failure` | 363-02 / Task 2 | ⬜ pending |
| GLD-C-03 | Server endpoint: POST /api/v1/sessions/{id}/telemetry-fallback requires service key | integration | `cargo test -p racecontrol -- telemetry_fallback::test_telemetry_fallback_requires_service_key` | 363-02 / Task 1 | ⬜ pending |
| GLD-C-03 | Server endpoint: writes csv_fallback_received_at on 200 | integration (SQLite) | `cargo test -p racecontrol -- telemetry_fallback::test_telemetry_fallback_receipt_timestamp` | 363-02 / Task 1 | ⬜ pending |
| GLD-C-04 | Grace window: lap reject arrives within 5s → grace_window_caught=true, lap removed before finalize | integration | `cargo test -p racecontrol -- billing_grace::test_grace_window_catches_reject` | 363-03 / Task 2 | ⬜ pending |
| GLD-C-04 | Grace window: no lap reject in 5s → finalize proceeds with original count | integration | `cargo test -p racecontrol -- billing_grace::test_grace_window_expires_normally` | 363-03 / Task 2 | ⬜ pending |
| GLD-C-04 | Grace window: server restart mid-window → hydrate_active_timers_from_db rebuilds timer with grace fields | integration (SQLite) | `cargo test -p racecontrol -- billing_grace::test_grace_window_restart_safe` | 363-03 / Task 2 | ⬜ pending |
| F-05 regression | end_billing_session early-end: CAS UPDATE SQL does not include wallet_debit_paise in SET clause (invariant test via create_test_db) | integration (SQLite) | `cargo test -p racecontrol -- billing::tests::test_end_billing_session_early_end_refund_amount` | 363-03 / Task 1 | ⬜ pending |
| F-05 regression | Rs.700 30min session ends at 15min → compute_refund(1800, 900, 70000) == 35000 | unit (pure) | `cargo test -p racecontrol -- billing::tests::test_f05_refund_uses_original_debit` | 363-03 / Task 1 | ⬜ pending |
| GLD-C-01..04 | Cloud sync payload: all 8 new billing_sessions columns present in upsert | integration | `cargo test -p racecontrol -- cloud_sync::test_billing_session_push_columns_phase363` | 363-01 / Task 3 | ⬜ pending |
| GLD-C-01..04 | Feature flag: phase363_session_audit=false bypasses all new audit paths | integration | `cargo test -p racecontrol -- session_audit::tests::test_feature_flag_kill_switch` | 363-01 / Task 2 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

### Notes on F-05 regression scope (revision 1)

Code audit during revision confirmed that `end_billing_session()` at `billing.rs:3972` is private,
takes `&Arc<AppState>`, and is NOT unit-testable without extensive mocking. The two F-05 regression
tests therefore exercise the two layers of the bug independently:

1. **Formula layer** (`test_f05_refund_uses_original_debit`) — pure-function call to `compute_refund(1800, 900, 70000)`,
   asserts the return value is `35000` (Rs.350). This locks the formula contract; if it ever regresses
   to Rs.187.50 the test fails.
2. **SQL invariant layer** (`test_end_billing_session_early_end_refund_amount`) — replays the exact CAS
   UPDATE shape from `billing.rs:4059` against an in-memory DB (via `create_test_db()`) and asserts
   that `wallet_debit_paise` is NOT modified. This locks the invariant that the CAS UPDATE SET clause
   does not include `wallet_debit_paise`. If a future refactor adds it back, the test fails.

Together these reproduce the exact class of regression that caused F-05. The full end-to-end trace
(customer → topup → session → early end → wallet balance) is covered by the Manual-Only E2E row below.

---

## Wave 0 Requirements

Wave 0 is folded into Wave 1 / Wave 2 tasks — each TDD task in the plans creates its own
test file scaffolding before implementation. No separate Wave 0 phase is needed.

- [x] `crates/racecontrol/src/session_audit.rs` — new module hosting lap heuristic + coverage + audit orchestration + `#[cfg(test)]` block for GLD-C-01/C-02 (created by 363-01 Task 2)
- [x] `crates/racecontrol/src/billing.rs` — extend existing `#[cfg(test)]` block with GLD-C-04 + F-05 regression tests; add new `mod billing_grace` submodule for grace window tests (created by 363-03 Tasks 1, 2, 3)
- [x] `crates/racecontrol/src/cloud_sync.rs` — `#[cfg(test)]` block asserting all 8 new column names appear in the billing_sessions push payload (created by 363-01 Task 3)
- [x] `crates/rc-agent/src/csv_lap_fallback.rs` — `#[cfg(test)]` block for push_on_session_end / no_push_when_empty / no_clear_on_failure (use mock HTTP server) (created by 363-02 Task 2)
- [x] `crates/racecontrol/src/api/routes.rs` — integration test module for new POST /api/v1/sessions/{id}/telemetry-fallback endpoint (created by 363-02 Task 1)
- [x] Reuse `create_test_db()` from `billing.rs:7562` for SQLite in-memory integration tests where billing schema is needed

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| End-to-end refund trace (customer → topup → 30min → end at 15min → verify ₹350 refund) | F-05 + GLD-C-04 | CLAUDE.md §"Financial flow E2E" mandates live-value tracing before shipping billing changes. `end_billing_session()` requires full AppState and is not unit-testable. | (1) Create test customer with ₹1000 balance; (2) book 30min/₹700 session on Pod 8; (3) end session at 15min mark; (4) verify customer balance = ₹1000 − ₹350 = ₹650; (5) verify no Rs.162.50 ghost loss |
| Cloud parity: Bono VPS DB has all 8 new columns after deploy | GLD-C-01..04 | Post-deploy schema drift can only be observed on the remote DB | After cloud `git_pull + cargo build + pm2 restart`: `ssh bono-vps "sqlite3 /root/racingpoint/racecontrol.db '.schema billing_sessions'"` and grep for lap_count_flag, telemetry_coverage_pct, suspect, suspect_reasons, csv_fallback_received_at, lap_reject_grace_until, lap_count_expected, lap_count_actual |
| Feature flag kill switch: set phase363_session_audit=false → new columns stay NULL for new sessions | Kill switch | Requires a real session on a real pod with the flag toggled live | Toggle flag via admin, run a session on Pod 8, verify billing_sessions row for that session has lap_count_flag=UNVERIFIED and telemetry_coverage_pct=NULL |
| Restart-safety live test: verify hydrate_active_timers_from_db rebuilds an active session after racecontrol restart | GLD-C-04 D-10 | Hydration is a new code path (no prior hydration existed). Unit test covers the function; this verifies real-world boot behavior. | Start a session on Pod 8, confirm billing_sessions row has status='active'. `schtasks /End /TN StartRCDirect` + restart racecontrol. On restart, check `GET /api/v1/billing/active` — the session should still be in active_timers. Confirm via logs: `rg 'hydrated active_timers from DB at startup' racecontrol.log`. |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify commands (no MISSING stubs)
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 folded into Wave 1/2 TDD tasks (no separate Wave 0 needed)
- [x] No watch-mode flags in test commands
- [x] Feedback latency < 90s
- [x] `nyquist_compliant: true` set in frontmatter (revision 1, 2026-04-09)
- [x] Test module paths reconciled with PLAN.md files (grace window → `billing_grace::` submodule)

**Approval:** pending execution
