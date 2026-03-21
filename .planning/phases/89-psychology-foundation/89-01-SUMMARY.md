---
phase: 89-psychology-foundation
plan: 89-01
subsystem: psychology
tags: [database, schema, rust, types, serialization]
dependency_graph:
  requires: []
  provides: [psychology-tables, psychology-types, badge-criteria-eval]
  affects: [racecontrol-db, racecontrol-crate]
tech_stack:
  added: [serde_json badge criteria parsing]
  patterns: [CREATE TABLE IF NOT EXISTS, sqlx::query execute, serde rename_all]
key_files:
  created:
    - /root/racecontrol/crates/racecontrol/src/psychology.rs
  modified:
    - /root/racecontrol/crates/racecontrol/src/db/mod.rs
    - /root/racecontrol/crates/racecontrol/src/lib.rs
decisions:
  - "7 psychology tables added before final tracing::info migration line — CREATE TABLE IF NOT EXISTS pattern consistent with entire file"
  - "psychology.rs inserted alphabetically between pod_reservation and remote_terminal in lib.rs"
  - "Operator enum uses serde rename attribute for symbol strings (>=, >, ==, <=, <) rather than string literals"
  - "Async function stubs take Arc<AppState> by ref — Plan 02 fills logic without changing signatures"
  - "WHATSAPP_DAILY_BUDGET = 2 is a named constant (not magic number) — FOUND-01 requirement"
metrics:
  duration_minutes: 75
  completed_date: "2026-03-21"
  tasks_completed: 2
  tasks_total: 2
  files_changed: 3
  tests_added: 13
---

# Phase 89 Plan 01: Psychology Foundation — DB Schema + Module Skeleton Summary

**One-liner:** 7 psychology tables (achievements, streaks, nudge_queue, passport, staff) + typed Rust module with JSON badge criteria evaluation and 13 passing unit tests.

## Tasks Completed

| Task | Name | Commit | Key Files |
|------|------|--------|-----------|
| 1 | Add 7 psychology tables to db/mod.rs migration | 9d95a18 | crates/racecontrol/src/db/mod.rs |
| 2 | Create psychology.rs module skeleton with types and JSON criteria evaluation | a620a52 | crates/racecontrol/src/psychology.rs, src/lib.rs |

## What Was Built

### Task 1: Database Schema (db/mod.rs)

Added 7 new `CREATE TABLE IF NOT EXISTS` statements under the comment `// --- Psychology Foundation (Phase 1) ---` before the final migration log line:

1. **achievements** — badge definitions with `criteria_json TEXT NOT NULL`, category CHECK constraint (milestone/skill/dedication/social/special), reward credits
2. **driver_achievements** — junction table with `UNIQUE(driver_id, achievement_id)` preventing duplicate badge awards
3. **streaks** — per-driver visit tracking with `current_streak INTEGER NOT NULL DEFAULT 0`, `longest_streak`, `grace_expires_date`
4. **driving_passport** — track/car exploration with `UNIQUE(driver_id, track, car)`, `best_lap_ms`, `lap_count`
5. **nudge_queue** — priority notification queue with `CHECK(channel IN ('whatsapp', 'discord', 'pwa'))` and `CHECK(status IN ('pending', 'sent', 'failed', 'expired', 'throttled'))`
6. **staff_badges** — staff skill badge definitions with `criteria_json TEXT NOT NULL`
7. **staff_challenges** — team challenges with `CHECK(status IN ('active', 'completed', 'expired'))`

Plus 8 indexes: `idx_driver_achievements_driver`, `idx_driver_achievements_achievement`, `idx_streaks_driver`, `idx_driving_passport_driver`, `idx_driving_passport_track`, `idx_nudge_queue_status` (composite: status+priority+scheduled_at), `idx_nudge_queue_driver`, `idx_staff_challenges_status`.

### Task 2: psychology.rs Module (psychology.rs + lib.rs)

Created the complete module foundation:

- **NotificationChannel** enum: `Whatsapp`, `Discord`, `Pwa` — `as_str()` + `from_str()` converters, serde `rename_all = "lowercase"`
- **NudgeStatus** enum: `Pending`, `Sent`, `Failed`, `Expired`, `Throttled` — `as_str()` converter
- **MetricType** enum: 7 variants (`TotalLaps`, `UniqueTracks`, `UniqueCars`, `SessionCount`, `PbCount`, `StreakWeeks`, `FirstLap`) — serde `rename_all = "snake_case"` for JSON compatibility
- **Operator** enum: 5 variants with serde rename to symbol strings (`>=`, `>`, `==`, `<=`, `<`)
- **BadgeCriteria** struct: `metric_type` (serde rename "type"), `operator`, `value: i64`
- **parse_criteria_json()**: parses `achievements.criteria_json` column → `Option<BadgeCriteria>`
- **evaluate_criteria()**: matches `Operator` variant → boolean comparison
- **Constants**: `WHATSAPP_DAILY_BUDGET=2`, `DISPATCHER_INTERVAL_SECS=30`, `DISPATCHER_BATCH_SIZE=10`, `NUDGE_TTL_DAYS=7`, `STREAK_GRACE_DAYS=7`
- **Async function stubs**: `evaluate_badges`, `update_streak`, `queue_notification`, `is_whatsapp_budget_exceeded`, `spawn_dispatcher` — all compile, Plan 02 implements them
- **13 unit tests** in `#[cfg(test)] mod tests` — all pass

Module registered as `pub mod psychology;` alphabetically between `pod_reservation` and `remote_terminal` in lib.rs.

## Verification Results

```
cargo check -p racecontrol-crate → Finished (1 pre-existing warning, 0 errors)
cargo test -p racecontrol-crate --lib psychology::tests → 13 passed; 0 failed
```

## Deviations from Plan

None - plan executed exactly as written.

## Self-Check: PASSED

- `/root/racecontrol/crates/racecontrol/src/psychology.rs` — FOUND
- `/root/racecontrol/crates/racecontrol/src/db/mod.rs` — FOUND (7 psychology tables)
- `/root/racecontrol/crates/racecontrol/src/lib.rs` — FOUND (pub mod psychology)
- Commit `9d95a18` — FOUND
- Commit `a620a52` — FOUND
- All 13 psychology tests — PASS
