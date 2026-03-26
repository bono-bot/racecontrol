---
phase: 200-self-improving-intelligence
plan: 02
subsystem: api/metrics
tags: [intel, alternatives-api, launch-matrix, combo-reliability, sqlite, tdd, axum]
dependency_graph:
  requires: [combo_reliability_table, query_combo_reliability, update_combo_reliability]
  provides: [alternatives_handler, launch_matrix_handler, GET_games_alternatives, GET_admin_launch-matrix]
  affects: [api/routes.rs, api/metrics.rs]
tech_stack:
  added: [AlternativesParams, AlternativeCombo, LaunchMatrixParams, LaunchMatrixRow, query_alternatives, query_launch_matrix]
  patterns: [pod-specific-then-fleet-fallback, 30-day-rolling-window, similarity-ordering, flagged-threshold-0.70]
key_files:
  created: []
  modified:
    - crates/racecontrol/src/api/metrics.rs
    - crates/racecontrol/src/api/routes.rs
decisions:
  - "query_alternatives takes &SqlitePool directly (not AppState) — testable without State, consistent with Plan 01 query_ pattern"
  - "Pod fallback uses pod-specific first (up to 3), fills remaining from fleet with different pod_id — avoids duplicates"
  - "Similarity ORDER BY uses (CASE WHEN car = ? OR track = ? THEN 1 ELSE 0 END) — SQLite-compatible, no subquery needed"
  - "launch_matrix runs two queries per pod (aggregate + failure modes) rather than one complex pivot — readable and SQLite-safe"
metrics:
  duration_minutes: 22
  completed_date: "2026-03-26T10:29:00Z"
  tasks_completed: 1
  files_modified: 2
---

# Phase 200 Plan 02: Alternatives + Launch Matrix API Summary

GET /api/v1/games/alternatives returning top-3 reliable combos with pod fallback and similarity preference, and GET /api/v1/admin/launch-matrix returning per-pod reliability grid with flagged=true for < 70% success rate.

## What Was Built

### Task 1: Alternatives + launch matrix handlers and route registration (TDD)

**RED phase:** 6 failing tests written first covering all behavioral contracts:
- `test_alternatives_top3`: max 3 results, all > 90% success_rate
- `test_alternatives_similarity`: at least 1 result shares car or track
- `test_alternatives_excludes_self`: failing combo excluded even if high success_rate
- `test_alternatives_pod_fallback`: pod < 3 results falls back to fleet-wide data
- `test_launch_matrix_flagged`: pod-5 (60%) flagged=true, pod-1 (90%) and pod-8 (80%) flagged=false
- `test_launch_matrix_failure_modes`: LaunchTimeout (count=3) as first failure mode

**GREEN phase:**

**`query_alternatives(db, params)` in api/metrics.rs:**
- Pod-specific query first when `pod` param provided: `WHERE sim_type = ? AND pod_id = ? AND success_rate > 0.90 AND total_launches >= 5` with NULL-safe self-exclusion via `COALESCE(car,'') = COALESCE(?,'')`
- If pod-specific results < 3, fetches from fleet (other pods) to fill remaining slots up to 3
- Both queries ORDER BY similarity CASE expression first, then `success_rate DESC`
- No-pod path uses fleet-wide query directly (LIMIT 3)

**`alternatives_handler()` in api/metrics.rs:**
- `GET /api/v1/games/alternatives?game=assetto_corsa&car=ks_ferrari&track=spa&pod=pod-5`
- Returns `Vec<AlternativeCombo>`: car, track, success_rate, avg_time_ms, total_launches

**`query_launch_matrix(db, sim_type)` in api/metrics.rs:**
- Single GROUP BY pod_id query for aggregates (total, successes, avg_ms) with 30-day window
- Per-pod sub-query for top 3 failure modes: `WHERE outcome != '"Success"' AND error_taxonomy IS NOT NULL GROUP BY error_taxonomy ORDER BY cnt DESC LIMIT 3`
- Sets `flagged = success_rate < 0.70`

**`launch_matrix_handler()` in api/metrics.rs:**
- `GET /api/v1/admin/launch-matrix?game=assetto_corsa`
- Returns `Vec<LaunchMatrixRow>`: pod_id, total_launches, success_rate, avg_time_ms, top_3_failure_modes, flagged

**Route registration in api/routes.rs (admin_routes):**
```rust
.route("/games/alternatives", get(metrics::alternatives_handler))
.route("/admin/launch-matrix", get(metrics::launch_matrix_handler))
```

## Tests

- 6 new tests all passing
- 4 alternatives tests: top3, similarity, excludes_self, pod_fallback
- 2 launch matrix tests: flagged, failure_modes
- 556 total suite tests pass (lib crate), 66 integration tests pass
- Pre-existing 3 failures (config::env_var, config::fallback, crypto::load_keys) confirmed unrelated — environment-dependent, fail intermittently before any of our changes
- `cargo build --release --bin racecontrol` compiles cleanly

## Deviations from Plan

None — plan executed exactly as written. Query patterns from plan's SQL specification worked without modification. `query_alternatives` and `query_launch_matrix` as testable pure functions (taking `&SqlitePool`) followed naturally from Plan 01's `query_combo_reliability` pattern.

## Self-Check: PASSED

- FOUND: `crates/racecontrol/src/api/metrics.rs` — `pub async fn alternatives_handler` on line 399
- FOUND: `crates/racecontrol/src/api/metrics.rs` — `pub async fn launch_matrix_handler` on line 491
- FOUND: `crates/racecontrol/src/api/routes.rs` — `/games/alternatives` route registration on line 124
- FOUND: `crates/racecontrol/src/api/routes.rs` — `/admin/launch-matrix` route registration on line 125
- FOUND: commit `8d99bf70` — feat(200-02): alternatives + launch matrix API endpoints (INTEL-03, INTEL-04)
- 4 alternatives tests pass, 2 launch_matrix tests pass, 5 combo_reliability tests pass (Plan 01)
- `cargo build --release --bin racecontrol` — Finished `release` profile [optimized]
