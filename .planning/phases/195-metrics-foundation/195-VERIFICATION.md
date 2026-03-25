---
phase: 195-metrics-foundation
verified: 2026-03-26T12:45:00Z
status: passed
score: 10/10 must-haves verified
re_verification: false
---

# Phase 195: Metrics Foundation Verification Report

**Phase Goal:** Every game launch, billing event, and crash recovery is recorded in dual storage (SQLite for queries, JSONL for immutable audit) with queryable APIs -- the data backbone that powers dynamic timeouts, intelligence, and debugging
**Verified:** 2026-03-26T12:45:00Z (IST: 2026-03-26T18:15:00)
**Status:** PASSED
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|---------|
| 1 | Every game launch attempt is recorded in SQLite `launch_events` table | VERIFIED | `metrics::record_launch_event()` called at 5 call sites in game_launcher.rs: lines 202, 284, 355, 459, 712 |
| 2 | Every game launch attempt is also written to `launch-events.jsonl` | VERIFIED | `append_launch_jsonl()` always called after SQLite insert in `record_launch_event()` — line 99 of metrics.rs |
| 3 | If SQLite insert fails, event still appears in JSONL with `db_fallback=true` | VERIFIED | `jsonl_event.db_fallback = Some(true)` set on DB error path (metrics.rs lines 93-95), then JSONL written |
| 4 | `log_game_event()` DB errors are logged via `tracing::error`, not swallowed | VERIFIED | `if let Err(e) = result { tracing::error!("game_launch_event insert failed...")` at game_launcher.rs line 748; JSONL fallback at line 766 |
| 5 | `billing_accuracy_events` and `recovery_events` tables exist with all required columns | VERIFIED | db/mod.rs lines 354 and 383 — both CREATE TABLE IF NOT EXISTS with all required fields including delta_ms, failure_mode, recovery_action_tried, recovery_outcome |
| 6 | Billing start in billing.rs records a billing_accuracy_event with delta_ms | VERIFIED | `crate::metrics::record_billing_accuracy_event()` called at billing.rs lines 571 (multiplayer) and 618 (single-player); delta_ms from `entry.waiting_since.elapsed().as_millis()` |
| 7 | Race Engineer crash relaunch records recovery_events | VERIFIED | `metrics::record_recovery_event()` at game_launcher.rs lines 589 (attempt) and 634 (exhausted) with `failure_mode="game_crash"`, `auto_relaunch_attempt_N` and `auto_relaunch_exhausted` |
| 8 | GET /api/v1/metrics/launch-stats returns all required fields with pod/game/car/track filters | VERIFIED | api/metrics.rs: `LaunchStatsResponse` contains success_rate, avg_time_to_track_ms, p95_time_to_track_ms, total_launches, common_failure_modes, last_30d_trend; dynamic WHERE clause with sqlx .bind() |
| 9 | GET /api/v1/metrics/billing-accuracy returns all required fields | VERIFIED | api/metrics.rs: `BillingAccuracyResponse` contains avg_delta_ms, max_delta_ms, sessions_with_zero_delta, sessions_where_billing_never_started, false_playable_signals |
| 10 | Both API endpoints are registered and reachable without authentication | VERIFIED | api/routes.rs lines 120-121 inside `public_routes()` function — both routes confirmed inside public_routes() closure (lines 79-123) |

**Score:** 10/10 truths verified

---

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/racecontrol/src/metrics.rs` | Metrics module with LaunchEvent, LaunchOutcome, ErrorTaxonomy, record_launch_event(), append_launch_jsonl(), hash_launch_args() | VERIFIED | 264 lines; all 6 types/functions present; dual-write pattern implemented; BillingAccuracyEvent, RecoveryEvent also added in Plan 02 |
| `crates/racecontrol/src/db/mod.rs` | launch_events, billing_accuracy_events, recovery_events table migrations | VERIFIED | All 3 tables at lines 319, 354, 383 with all required columns; 10 indexes total (idx_launch_events_combo, idx_billing_accuracy_session, idx_recovery_events_pod etc.) |
| `crates/racecontrol/src/lib.rs` | `pub mod metrics;` declaration | VERIFIED | Line 35: `pub mod metrics;` |
| `crates/racecontrol/src/game_launcher.rs` | Updated with `use crate::metrics`, 5 record_launch_event() calls, fixed log_game_event(), 2 record_recovery_event() calls | VERIFIED | Line 9: `use crate::metrics`; 5 record_launch_event calls; log_game_event uses `if let Err(e)` not `let _ =`; 2 recovery event calls |
| `crates/racecontrol/src/billing.rs` | 2 record_billing_accuracy_event() calls (single-player + multiplayer) | VERIFIED | Lines 571, 618 — both billing start paths covered |
| `crates/racecontrol/src/api/metrics.rs` | Axum handlers for launch-stats and billing-accuracy | VERIFIED | 241 lines; launch_stats_handler and billing_accuracy_handler both present and substantive |
| `crates/racecontrol/src/api/mod.rs` | `pub mod metrics;` declaration | VERIFIED | Line 1: `pub mod metrics;` |
| `crates/racecontrol/src/api/routes.rs` | Route registration for /metrics/* endpoints in public_routes | VERIFIED | Lines 120-121 inside public_routes() |

---

### Key Link Verification

| From | To | Via | Status | Details |
|------|-----|-----|--------|---------|
| game_launcher.rs | metrics.rs | `metrics::record_launch_event()` | WIRED | 5 call sites (launch, relaunch, stop, timeout, crash state); `use crate::metrics` at line 9 |
| metrics.rs | SQLite launch_events + launch-events.jsonl | `sqlx INSERT` + `file append` | WIRED | INSERT INTO launch_events at metrics.rs line 72; append_launch_jsonl() always called after |
| billing.rs | metrics.rs | `crate::metrics::record_billing_accuracy_event()` | WIRED | Called at both single-player (line 618) and multiplayer (line 571) billing start paths |
| game_launcher.rs | metrics.rs | `metrics::record_recovery_event()` / `crate::metrics::record_recovery_event()` | WIRED | Called at relaunch attempt (line 589) and exhausted (line 634) |
| metrics.rs | SQLite billing_accuracy_events | `sqlx INSERT` | WIRED | INSERT INTO billing_accuracy_events in record_billing_accuracy_event() |
| metrics.rs | SQLite recovery_events | `sqlx INSERT` | WIRED | INSERT INTO recovery_events in record_recovery_event() |
| api/metrics.rs | SQLite launch_events | `sqlx SELECT queries` | WIRED | SELECT queries from launch_events for total/success, p95, failure modes, trend |
| api/metrics.rs | SQLite billing_accuracy_events | `sqlx SELECT queries` | WIRED | SELECT queries from billing_accuracy_events for avg/max/zero delta, never_started, false_signals |
| api/routes.rs | api/metrics.rs | route registration | WIRED | `/metrics/launch-stats` → `metrics::launch_stats_handler`, `/metrics/billing-accuracy` → `metrics::billing_accuracy_handler` in public_routes() |

---

### Requirements Coverage

The METRICS-01 through METRICS-07 requirement IDs are defined as success criteria within Phase 195's ROADMAP.md entry (not in a separate REQUIREMENTS.md file for this milestone). All 7 are covered:

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|---------|
| METRICS-01 | 195-01 | launch_events table with all required columns | SATISFIED | db/mod.rs line 319; all columns present: pod_id, sim_type, car, track, session_type, timestamp, outcome, error_taxonomy, duration_to_playable_ms, error_details, launch_args_hash, attempt_number |
| METRICS-02 | 195-01 | JSONL fallback with db_fallback=true on DB failure | SATISFIED | metrics.rs lines 93-99: error sets db_fallback, JSONL always written; `record_launch_event_jsonl_only()` for log_game_event fallback path |
| METRICS-03 | 195-02 | billing_accuracy_events with timing columns | SATISFIED | db/mod.rs line 354; billing.rs wired at both billing start paths with delta_ms from waiting_since.elapsed() |
| METRICS-04 | 195-02 | recovery_events with failure_mode, action, outcome, duration | SATISFIED | db/mod.rs line 383; game_launcher.rs wired at 2 recovery points with game_crash failure_mode and RecoveryOutcome enum |
| METRICS-05 | 195-03 | GET /api/v1/metrics/launch-stats with pod/game/car/track filters | SATISFIED | api/metrics.rs launch_stats_handler; LaunchStatsParams with all 4 filters; response contains all required fields; route registered in public_routes |
| METRICS-06 | 195-03 | GET /api/v1/metrics/billing-accuracy with all required fields | SATISFIED | api/metrics.rs billing_accuracy_handler; BillingAccuracyResponse contains all 5 required fields; 30-day rolling window |
| METRICS-07 | 195-01 | DB errors logged via tracing::error, not swallowed | SATISFIED | game_launcher.rs: `let _ = sqlx::query` replaced with `if let Err(e) = result { tracing::error!(...)` at line 748; all 3 record_*() functions in metrics.rs follow same pattern |

No orphaned requirements found. REQUIREMENTS.md and REQUIREMENTS-v25.md contain requirements for different milestones (v23.1 audit protocol and v25.0 debug-first-time-right respectively) — the METRICS-XX IDs are self-contained to Phase 195 in ROADMAP.md.

---

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| api/metrics.rs | 107, 130 | `.unwrap_or_default()` on fetch_all results | Info | Acceptable — fetch_all failure returns empty Vec, not a panic; empty result correctly yields 0 values |
| api/metrics.rs | 172, 240 | `serde_json::to_value(&response).unwrap_or_default()` | Info | Acceptable — serialization of known-shape struct to serde_json::Value will never fail; unwrap_or_default is safe |
| api/metrics.rs | 219, 230 | `.unwrap_or(0)` on query_scalar results | Info | Acceptable — returns 0 count on DB error, which is a safe degraded response |

No blockers. All `unwrap_or*` usages are on well-bounded safe paths (empty Vec fallback, known-shape struct serialization, count defaults to 0). No `.unwrap()` calls without fallback. No `let _ =` error swallowing. No TODO/FIXME/placeholder patterns.

---

### Compilation Status

`cargo check -p racecontrol-crate` passes with 3 pre-existing warnings (unrelated to Phase 195 changes):
- Unused variable `opt_session_id` (pre-existing)
- Dead code `SESSION_WAIT_TIMEOUT` (pre-existing)
- One other pre-existing warning

Zero errors. Zero new warnings introduced by Phase 195.

---

### Commits Verified

All 6 commits documented in summaries exist in git history:

| Commit | Plan | Task | Description |
|--------|------|------|-------------|
| `176c2f4e` | 195-01 | Task 1 | Create metrics module with launch_events table and JSONL writer |
| `3135e7dc` | 195-01 | Task 2 | Wire record_launch_event into game_launcher and fix error swallowing |
| `503ef7c0` | 195-02 | Task 1 | Add billing_accuracy_events and recovery_events tables + recording functions |
| `2ec92cb5` | 195-02 | Task 2 | Wire billing accuracy and recovery event recording at call sites |
| `d941ff68` | 195-03 | Task 1 | Create metrics API handlers for launch-stats and billing-accuracy |
| `6d17f271` | 195-03 | Task 2 | Register metrics routes in public_routes |

---

### Human Verification Required

#### 1. JSONL Fallback Under Real DB Failure

**Test:** Rename `racecontrol.db` on the server while racecontrol is running, trigger a game launch (or relaunch), then check `C:\RacingPoint\data\launch-events.jsonl` for a line with `"db_fallback":true`.
**Expected:** JSONL file grows by one line containing `"db_fallback":true` and all event fields.
**Why human:** Cannot simulate a real DB failure programmatically without server access. Code path is correct but end-to-end behavior requires live execution.

#### 2. Billing Delta Measurement Accuracy

**Test:** Start a billing session via the kiosk, let the game reach AcStatus::Live, then query `SELECT delta_ms FROM billing_accuracy_events ORDER BY created_at DESC LIMIT 1`. Compare to actual observed launch-to-live time.
**Expected:** delta_ms within ~500ms of the visually observed launch-to-playable gap.
**Why human:** The `waiting_since.elapsed()` timing is code-correct but the measurement accuracy (whether `defer_billing_start` is called at the right moment vs the actual launch command) requires live observation to validate.

#### 3. API Response With Real Data

**Test:** After a few game launches, `curl 'http://192.168.31.23:8080/api/v1/metrics/launch-stats?game=assetto_corsa'`.
**Expected:** JSON response with non-zero total_launches, computed success_rate, and at least one failure mode if any crashes occurred.
**Why human:** The endpoint returns correct structure even with empty tables (0s). Real data validation confirms the event recording and querying pipeline works end-to-end.

---

### Phase Goal Assessment

The phase goal "Every game launch, billing event, and crash recovery is recorded in dual storage (SQLite for queries, JSONL for immutable audit) with queryable APIs" is achieved:

1. **Dual storage established:** Every game launch writes to `launch_events` SQLite table AND `launch-events.jsonl`. DB failure triggers JSONL-only path with `db_fallback=true`.
2. **All event types covered:** Game launch (5 call sites), billing accuracy (2 call sites), crash recovery (2 call sites).
3. **Queryable APIs live:** Both `/api/v1/metrics/launch-stats` (with 4 filters + 6 computed fields) and `/api/v1/metrics/billing-accuracy` (5 computed fields) registered and accessible without auth.
4. **Error swallowing fixed:** `log_game_event()` now logs DB errors via `tracing::error` and falls back to JSONL — zero silent data loss.
5. **Foundation for downstream phases:** `launch_events` table feeds Phase 196+ dynamic timeouts; `recovery_events` feeds Phase 199 history-informed recovery; APIs feed Phase 200 intelligence and Phase 201 admin dashboard.

---

_Verified: 2026-03-26T12:45:00Z (IST: 2026-03-26T18:15:00)_
_Verifier: Claude (gsd-verifier)_
