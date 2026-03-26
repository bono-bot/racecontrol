---
phase: 200-self-improving-intelligence
verified: 2026-03-26T05:30:00Z
status: passed
score: 8/8 must-haves verified
re_verification: false
---

# Phase 200: Self-Improving Intelligence Verification Report

**Phase Goal:** The system uses accumulated launch data to warn about unreliable combos, suggest alternatives, and display reliability insights to staff — every launch makes the system smarter without manual threshold tuning

**Verified:** 2026-03-26T05:30:00Z (IST 11:00)
**Status:** PASSED
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | combo_reliability table is populated after every launch event | VERIFIED | `update_combo_reliability()` called at end of `record_launch_event()` (metrics.rs:103), after both SQLite insert and JSONL write |
| 2 | Launch response includes warning when combo reliability < 70% with >= 5 launches | VERIFIED | `reliability_warning` injected in routes.rs HTTP launch handler (lines 3640-3658); `.filter(r.success_rate < 0.70)` applied; query returns None for < 5 launches |
| 3 | No warning for combos with >= 70% success or < 5 launches | VERIFIED | `query_combo_reliability()` returns `None` when `total_launches < 5` (metrics.rs:486); `.filter()` suppresses >= 70% warnings |
| 4 | Low-reliability combos (< 50%) get 3 auto-relaunch attempts instead of 2 | VERIFIED | `max_relaunch_cap = 3` when `success_rate < 0.50 && total_launches >= 5` (game_launcher.rs:267); tracker uses `tracker.max_auto_relaunch` (lines 749, 854); no hardcoded `>= 2` comparisons remain |
| 5 | GET /api/v1/games/alternatives returns top 3 high-reliability combos with similarity preference | VERIFIED | `alternatives_handler` (api/metrics.rs:399), `query_alternatives` (line 266); ORDER BY similarity CASE expression first, then success_rate DESC; pod-specific then fleet fallback |
| 6 | GET /api/v1/admin/launch-matrix returns per-pod reliability grid with flagged boolean | VERIFIED | `launch_matrix_handler` (api/metrics.rs:491), `query_launch_matrix` (line 426); `flagged: success_rate < 0.70` |
| 7 | Alternatives prefer same-car or same-track combos over random | VERIFIED | SQL ORDER BY `(CASE WHEN car = ? OR track = ? THEN 1 ELSE 0 END) DESC` in both pod-specific and fleet fallback queries |
| 8 | Launch matrix flags pods with < 70% success rate | VERIFIED | `flagged: success_rate < 0.70` set per pod row in `query_launch_matrix()` |

**Score:** 8/8 truths verified

---

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/racecontrol/src/db/mod.rs` | combo_reliability CREATE TABLE migration | VERIFIED | Table created at line 416; UNIQUE INDEX on `COALESCE(car,''), COALESCE(track,'')` at line 432 (SQLite workaround — no COALESCE in PRIMARY KEY) |
| `crates/racecontrol/src/metrics.rs` | `query_combo_reliability()`, `update_combo_reliability()` | VERIFIED | `update_combo_reliability` at line 294; `query_combo_reliability` at line 459; `ComboReliability` struct present |
| `crates/racecontrol/src/game_launcher.rs` | `max_auto_relaunch` field in GameTracker, reliability-based retry cap | VERIFIED | Field at line 38; set to `max_relaunch_cap` in tracker construction (line 299); used in relaunch checks (lines 749, 854) |
| `crates/racecontrol/src/api/routes.rs` | Warning injection in HTTP launch handler | VERIFIED | `reliability_warning` block at lines 3640-3658; injected into response at lines 3668-3672 |
| `crates/racecontrol/src/api/metrics.rs` | `alternatives_handler`, `launch_matrix_handler` | VERIFIED | `alternatives_handler` at line 399; `launch_matrix_handler` at line 491; both with testable `query_*` helper functions |

---

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `crates/racecontrol/src/metrics.rs` | combo_reliability table | `update_combo_reliability()` called from `record_launch_event()` | WIRED | Called at metrics.rs:103 after both DB insert and JSONL write |
| `crates/racecontrol/src/api/routes.rs` | `crates/racecontrol/src/metrics.rs` | `crate::metrics::query_combo_reliability()` called before `handle_dashboard_command()` | WIRED | Full crate path used at routes.rs:3647 to disambiguate from `super::metrics` (api::metrics) |
| `crates/racecontrol/src/game_launcher.rs` | `crates/racecontrol/src/metrics.rs` | `query_combo_reliability()` to set `max_auto_relaunch` | WIRED | Called at game_launcher.rs:259-265; result drives `max_relaunch_cap` at line 266-269 |
| `crates/racecontrol/src/api/metrics.rs` | combo_reliability table | SQL queries in `query_alternatives()` and `query_launch_matrix()` | WIRED | `query_alternatives` queries `combo_reliability` (line 276+); `query_launch_matrix` queries `launch_events` directly (per-pod aggregate) |
| `crates/racecontrol/src/api/routes.rs` | `crates/racecontrol/src/api/metrics.rs` | Route registration for new handlers | WIRED | `/games/alternatives` → `metrics::alternatives_handler` at routes.rs:124; `/admin/launch-matrix` → `metrics::launch_matrix_handler` at routes.rs:125 |

---

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| INTEL-01 | 200-01 | Combo reliability score computed from launch_events per (pod_id, sim_type, car, track) over 30-day rolling window — minimum 5 launches before scoring activates | SATISFIED | `combo_reliability` table in db/mod.rs:416; `update_combo_reliability()` with 30-day window; `query_combo_reliability()` returns None for <5 launches; 5 unit tests pass |
| INTEL-02 | 200-01 | Warning injected into POST /api/v1/games/launch response when combo reliability < 70% (with minimum 5 launches) | SATISFIED | `reliability_warning` block in routes.rs:3640-3658; warning injected at lines 3669-3671; suppressed when >=70% or <5 launches |
| INTEL-03 | 200-02 | GET /api/v1/games/alternatives returns top 3 combos with same sim, higher reliability (>90%), sorted by success_rate DESC; preference for same-car or same-track | SATISFIED | `alternatives_handler` + `query_alternatives` in api/metrics.rs; route registered at routes.rs:124; 4 tests pass (top3, similarity, excludes_self, pod_fallback) |
| INTEL-04 | 200-02 | GET /api/v1/admin/launch-matrix returns per-pod grid with pod_id, total_launches, success_rate, avg_time_ms, top_3_failure_modes, flagged boolean (<70%) | SATISFIED | `launch_matrix_handler` + `query_launch_matrix` in api/metrics.rs; route registered at routes.rs:125; 2 tests pass (flagged, failure_modes) |
| INTEL-05 | 200-01 | Auto-tuning: if combo reliability < 50%, increase max auto_relaunch_count from 2 to 3 | SATISFIED | `max_auto_relaunch` field in GameTracker; set to 3 when `success_rate < 0.50 && total_launches >= 5`; all 17 GameTracker construction sites updated with default 2; hardcoded `>= 2` comparisons eliminated |

All 5 requirements: SATISFIED. No orphaned requirements found.

---

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `crates/racecontrol/src/metrics.rs` | 583 | `.unwrap()` on `partial_cmp` in `sort_by` | Info | Pre-existing code in `query_dynamic_timeout()` (Phase 197), not Phase 200 code. `partial_cmp` on f64 only returns None for NaN values; values come from i64 cast to f64 so NaN is impossible. No impact. |
| `crates/racecontrol/src/api/metrics.rs` | 496 | `serde_json::to_value().unwrap_or_default()` | Info | Standard Axum handler pattern — `to_value` on `Vec<LaunchMatrixRow>` (derived Serialize) will never fail in practice. `unwrap_or_default()` returns `Value::Null` as fallback, not a panic. Acceptable. |

No blockers. No stubs. No placeholders.

---

### Human Verification Required

#### 1. End-to-end warning flow with live launch data

**Test:** Perform 5+ launches of a game+car+track combo where some fail (crash or timeout). Then attempt another launch of the same combo.
**Expected:** POST /api/v1/games/launch response includes `"warning": "This combination has a X% success rate on this pod (Y/Z launches)"` when success rate is below 70%.
**Why human:** Requires live pods, actual game launches, and real failure events in the launch_events table. Cannot simulate with static code analysis.

#### 2. Alternatives UI surface

**Test:** Hit GET /api/v1/games/alternatives?game=assetto_corsa&car=ks_ferrari&track=spa&pod=pod-5 after seeding with a few launch events.
**Expected:** Returns JSON array of up to 3 alternative combos, all with success_rate > 0.90, sorted by similarity then rate.
**Why human:** Endpoint exists and tests pass against in-memory DB, but real-world behavior with a seeded live DB needs verification. Also confirms the route is publicly accessible (auth scope check).

#### 3. Admin launch matrix display

**Test:** Hit GET /api/v1/admin/launch-matrix?game=assetto_corsa when pods have launch history.
**Expected:** Per-pod rows with flagged=true for any pod below 70% success rate.
**Why human:** Same reason — requires real launch_events data to have meaningful output.

---

### Test Results (Automated)

| Test Suite | Count | Status |
|-----------|-------|--------|
| combo_reliability tests (Plan 01) | 5 | All pass |
| alternatives tests (Plan 02) | 4 | All pass |
| launch_matrix tests (Plan 02) | 2 | All pass |
| **Phase 200 total** | **11** | **All pass** |
| Full suite regression | 556 lib + 66 integration | Pass (3 pre-existing env-dependent failures confirmed unrelated) |

---

### Gaps Summary

No gaps. All 8 must-haves verified, all 5 requirements satisfied, all 11 phase tests pass, no stub anti-patterns found. The phase goal is achieved: every launch event updates the combo_reliability table, staff see warnings for unreliable combos in the launch API response, alternatives are queryable, and the admin launch matrix shows fleet-wide reliability with flagged pods — all without any manual threshold tuning.

---

_Verified: 2026-03-26T05:30:00Z_
_Verifier: Claude (gsd-verifier)_
