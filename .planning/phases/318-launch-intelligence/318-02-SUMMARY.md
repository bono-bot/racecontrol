---
phase: 318-launch-intelligence
plan: "02"
subsystem: launch-tracing
tags: [launch-intelligence, sqlite, websocket, rest-api, tdd]
dependency_graph:
  requires: [318-01, 315-01]
  provides: [launch_timeline_spans table, LaunchTimelineReport WS flow, GET /api/v1/launch-timeline/:launch_id]
  affects: [phase-319-reliability-dashboard]
tech_stack:
  added: [uuid crate usage in rc-agent for launch_id generation]
  patterns: [tokio::spawn fire-and-forget persist, INSERT OR REPLACE for idempotent timeline upsert, fetch_optional + empty-events fallback for unknown launch_id]
key_files:
  created: []
  modified:
    - crates/racecontrol/src/db/mod.rs
    - crates/racecontrol/src/game_launcher.rs
    - crates/racecontrol/src/ws/mod.rs
    - crates/racecontrol/src/api/routes.rs
    - crates/rc-agent/src/event_loop.rs
    - crates/rc-agent/src/ws_handler.rs
    - crates/racecontrol/src/billing.rs
decisions:
  - "launch_id generated in rc-agent at LaunchGame receipt (not propagated from server) — keeps agent self-contained and avoids protocol change"
  - "launch_start + current_launch_id placed in ConnectionState (not AppState) — resets per WS connection which matches launch lifecycle"
  - "try_send (non-blocking) used for LaunchTimelineReport from BillingStarted/LaunchTimedOut handlers — fire-and-forget to avoid blocking WS message loop"
  - "INSERT OR REPLACE INTO launch_timeline_spans — handles duplicate launch_ids gracefully without error"
  - "fetch_optional + empty events fallback for unknown launch_id — GET endpoint never returns 404, safe for dashboard polling"
metrics:
  duration_minutes: 85
  tasks_completed: 2
  files_modified: 7
  tests_added: 4
  completed_date: "2026-04-03T08:28:00Z"
requirements: [LAUNCH-05]
---

# Phase 318 Plan 02: Launch Timeline Tracing Summary

Step-level launch timeline tracing with millisecond-resolution checkpoint spans persisted to SQLite and queryable via REST API.

## What Was Built

- `launch_timeline_spans` SQLite table (server migration in db/mod.rs)
- `GameTracker.launch_id` field (UUID v4, added to server-side tracker struct)
- `ConnectionState.launch_start` + `current_launch_id` fields (rc-agent, track per connection)
- `build_launch_timeline()` helper function (rc-agent/ws_handler.rs)
- `LaunchTimelineReport` send from BillingStarted (success) and LaunchTimedOut (timeout) handlers
- Server WS handler for `AgentMessage::LaunchTimelineReport` persisting via tokio::spawn
- `GET /api/v1/launch-timeline/:launch_id` endpoint returning full span row or empty events for unknown IDs

## Commits

| Hash | Task | Description |
|------|------|-------------|
| `a9578f74` | Task 1 | add launch_timeline_spans migration + GameTracker launch_id field |
| `6de5625a` | Task 2 | agent sends LaunchTimelineReport + server persists + GET endpoint |

## Tests Added (4 total, all passing)

| Test | Location | Behavior |
|------|----------|----------|
| `test_launch_timeline_spans_table_exists` | db/mod.rs | Table exists after migrate() |
| `test_launch_timeline_spans_round_trip` | db/mod.rs | INSERT/SELECT events_json round-trip |
| `test_launch_timeline_success_has_required_event_kinds` | rc-agent/ws_handler.rs | agent_received + playable_signal present for success |
| `test_launch_timeline_timeout_has_timeout_event` | rc-agent/ws_handler.rs | timeout event present in timeout outcome |
| `test_get_launch_timeline_unknown_returns_empty_events` | routes.rs | unknown launch_id returns 200 with empty events |
| `test_get_launch_timeline_persisted_row_round_trip` | routes.rs | INSERT then GET returns correct row |

(6 tests total)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed pre-existing ws/mod.rs compile error: store_startup_report missing 9th argument**
- **Found during:** Task 1 (cargo check --no-run to validate all construction sites)
- **Issue:** `store_startup_report()` call in ws/mod.rs was missing `windows_session_id: Option<u32>` 9th argument, blocking compilation of all racecontrol tests
- **Fix attempts:** First tried `windows_session_id.copied()` (failed — `&Option<u32>` has no `.copied()`), then `*windows_session_id` (works — dereferences the reference)
- **Files modified:** `crates/racecontrol/src/ws/mod.rs`
- **Commit:** `a9578f74`

**2. [Rule 1 - Bug] Fixed 14 test GameTracker construction sites missing launch_id field**
- **Found during:** Task 1 post-implementation `cargo test --no-run` (cargo check only compiles non-test code)
- **Issue:** Adding `launch_id` field to GameTracker struct requires updating all construction sites; 14 test sites in game_launcher.rs + 1 in billing.rs were missing it
- **Fix:** Python regex script added `launch_id: "test-launch-001".to_string(),` after each `billing_session_id: None,` pattern in game_launcher.rs; billing.rs fixed manually
- **Files modified:** `crates/racecontrol/src/game_launcher.rs`, `crates/racecontrol/src/billing.rs`
- **Commit:** `a9578f74`

**3. [Rule 3 - Blocking] Moved launch tracking fields from AppState to ConnectionState**
- **Found during:** Task 2 planning — AppState persists across reconnects, launch data must reset per connection
- **Fix:** Added `launch_start: Option<std::time::Instant>` and `current_launch_id: Option<String>` to `ConnectionState` struct in `event_loop.rs` instead of AppState
- **Commit:** `6de5625a`

**4. [Rule 3 - Blocking] Routes.rs test used private migrate() — replaced with inline table creation**
- **Found during:** Task 2 test compilation — `crate::db::migrate` is private, cannot call from routes.rs tests
- **Fix:** Created `setup_timeline_pool()` helper that creates the table inline without calling `migrate()`
- **Files modified:** `crates/racecontrol/src/api/routes.rs`
- **Commit:** `6de5625a`

## Verification Results

All success criteria from PLAN.md confirmed:

1. `grep "launch_timeline_spans" crates/racecontrol/src/db/mod.rs` — returns CREATE TABLE statement: PASS
2. `grep "LaunchTimelineReport" crates/racecontrol/src/ws/mod.rs` — returns handler match arm: PASS
3. `grep -c 'launch-timeline' crates/racecontrol/src/api/routes.rs` — returns exactly 1 route registration: PASS (count=1)
4. `cargo test -p racecontrol-crate test_launch_timeline_spans_table_exists` — PASS
5. `cargo check -p racecontrol-crate && cargo check -p rc-agent-crate` — both PASS (37 pre-existing warnings, 0 errors)
6. No `.unwrap()` in new production code — verified by grepping additions in all 4 commits

## Known Stubs

None. All event data is real (Instant::elapsed for timing, UUID v4 for launch_id, actual BillingStarted/LaunchTimedOut outcome signals). No placeholder values flow to UI rendering.

## Self-Check: PASSED
