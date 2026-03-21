---
phase: 90-customer-progression
plan: "01"
subsystem: psychology + catalog + api/routes
tags: [driving-passport, badges, customer-api, lap-tracker, progression]
dependency_graph:
  requires: [phase-89-psychology-foundation]
  provides: [customer-passport-api, customer-badges-api, passport-data-population]
  affects: [racecontrol-pwa, racingpoint-admin]
tech_stack:
  added: []
  patterns: [lazy-backfill-on-first-access, ON-CONFLICT-upsert, tiered-collections]
key_files:
  created: []
  modified:
    - crates/racecontrol/src/psychology.rs
    - crates/racecontrol/src/catalog.rs
    - crates/racecontrol/src/lap_tracker.rs
    - crates/racecontrol/src/api/routes.rs
decisions:
  - "Lazy backfill triggered when passport_count == 0 on first /customer/passport call — safe with INSERT OR IGNORE for concurrent requests"
  - "Tier boundaries: Starter=0..6, Explorer=6..15, Legend=15..end (by sort_order = index in FEATURED_TRACKS/FEATURED_CARS)"
  - "parse_badge_progress reads criteria_json keys type/value (not metric/threshold) — confirmed from DB schema"
metrics:
  duration_minutes: 7
  completed_date: "2026-03-21"
  tasks_completed: 2
  files_modified: 4
requirements: [PROG-01, PROG-02, PROG-03, PROG-04, PROG-05]
---

# Phase 90 Plan 01: Passport Data Population + API Endpoints Summary

**One-liner:** Driving passport ON CONFLICT upsert on every lap + lazy backfill + tiered /customer/passport and /customer/badges endpoints

## What Was Built

### Task 1: Passport data functions + catalog accessors + persist_lap wiring

Added to `psychology.rs`:
- `pub async fn update_driving_passport()` — upserts driving_passport via `ON CONFLICT(driver_id, track, car) DO UPDATE` incrementing lap_count and updating best_lap_ms if faster. Called from persist_lap on every valid lap.
- `pub async fn backfill_driving_passport()` — `INSERT OR IGNORE INTO driving_passport ... SELECT ... FROM laps WHERE driver_id = ? AND valid = 1 AND lap_time_ms > 0 GROUP BY driver_id, track, car`. Lazy, safe for concurrent callers.

Added to `catalog.rs`:
- `pub fn get_featured_tracks_for_passport() -> Vec<Value>` — returns 36 featured tracks with sort_order=index for tier grouping
- `pub fn get_featured_cars_for_passport() -> Vec<Value>` — returns 41 featured cars with sort_order=index
- Made `id_to_display_name()` public for use in routes.rs "other" sections

Wired in `lap_tracker.rs`:
- Added `use crate::psychology;`
- Inserted `psychology::update_driving_passport(state, &lap.driver_id, &lap.track, &lap.car, lap.lap_time_ms as i64).await;` after driver aggregate stats update, before Phase 14 auto_enter_event block

### Task 2: Customer passport and badges API endpoints

Added to `api/routes.rs`:
- Route: `GET /customer/passport` → `customer_passport`
- Route: `GET /customer/badges` → `customer_badges`

`customer_passport`:
- JWT auth via `extract_driver_id()`
- Lazy backfill: counts driving_passport rows, calls `psychology::backfill_driving_passport()` if zero
- Builds driven track/car sets from passport entries
- Applies tier boundaries (Starter: 0-5, Explorer: 6-14, Legend: 15+) using sort_order from catalog accessors
- Returns nested tiers (starter/explorer/legend) plus "other" for non-featured tracks/cars
- Summary: unique_tracks, unique_cars, total_laps, streak_weeks

`customer_badges`:
- JWT auth via `extract_driver_id()`
- Queries `achievements` table (column: `badge_icon` not `icon`, is_active=1)
- Earned lookup from `driver_achievements` table (column: `achievement_id` not `badge_id`)
- Progress computed via `parse_badge_progress()` reading criteria_json keys `"type"` and `"value"`
- Returns `earned` array (with earned_at) and `available` array (with progress/target)

Helper `parse_badge_progress()`:
- Maps type strings: total_laps, unique_tracks, unique_cars, pb_count, session_count, first_lap, streak_weeks → driver metrics
- Returns (current.min(threshold), threshold) for bounded progress display

### Pre-existing Bug Fixed (Rule 1)

`psychology.rs` test helper `make_state_with_db()` was calling `AppState::new(config, db)` with 2 args, but `AppState::new` was updated in a previous phase to require 3 args (added `FieldCipher`). Fixed by using `crate::crypto::encryption::test_field_cipher()`.

## Verification Results

- `cargo check -p racecontrol-crate`: PASSES (zero errors, 6 pre-existing unused-import warnings)
- `cargo test -p racecontrol-crate --lib`: 374 PASS, 1 pre-existing FAIL (server_ops::test_exec_echo — returns HTTP 500 in cloud env, unrelated to this plan)
- All 5 requirement IDs have backend support:
  - PROG-01: /customer/passport endpoint exists
  - PROG-02: Response includes starter/explorer/legend tiers for both tracks and cars
  - PROG-03: /customer/badges endpoint exists
  - PROG-04: backfill_driving_passport() called lazily from customer_passport when passport_count == 0
  - PROG-05: /customer/badges returns earned (with earned_at) + available (with progress/target)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed pre-existing AppState::new arity mismatch in psychology.rs tests**
- **Found during:** Task 2 (running cargo test)
- **Issue:** `make_state_with_db()` called `AppState::new(config, db)` but the function signature requires 3 arguments including `FieldCipher` (added in a prior phase). Compilation error in test binary.
- **Fix:** Added `let cipher = crate::crypto::encryption::test_field_cipher();` and passed it as third arg.
- **Files modified:** `crates/racecontrol/src/psychology.rs`
- **Commit:** 104a22c (included in Task 2 commit)

### Out-of-scope Pre-existing Failure

- `server_ops::tests::test_exec_echo` fails because the /exec endpoint returns 500 in the VPS cloud environment (exec service not configured). This pre-existed this plan and is unrelated. Logged but not fixed per scope boundary rules.

## Commits

| Task | Commit | Description |
|------|--------|-------------|
| Task 1 | 4486468 | feat(90-01): passport data functions + catalog accessors + persist_lap wiring |
| Task 2 | 104a22c | feat(90-01): customer passport and badges API endpoints |

## Self-Check: PASSED

All key files exist. Both commits verified in git log.
- psychology.rs: FOUND
- catalog.rs: FOUND
- lap_tracker.rs: FOUND
- routes.rs: FOUND
- Commit 4486468: FOUND
- Commit 104a22c: FOUND
