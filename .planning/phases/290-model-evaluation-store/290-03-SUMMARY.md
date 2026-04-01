---
phase: 290-model-evaluation-store
plan: "03"
subsystem: model-lifecycle
tags: [eval-store, protocol, server-api, ws-handler, sqlite, EVAL-03]
dependency_graph:
  requires: [290-01, 290-02]
  provides: [EVAL-03, model-eval-query-api, server-eval-db]
  affects: [292-model-reputation, 294-report-v2]
tech_stack:
  added: [EvalRecordPayload, AgentMessage::ModelEvalSync, model_evaluations table]
  patterns: [INSERT OR IGNORE idempotency, sqlx QueryBuilder dynamic filter, best-effort WS push]
key_files:
  created: []
  modified:
    - crates/rc-common/src/protocol.rs
    - crates/racecontrol/src/fleet_kb.rs
    - crates/racecontrol/src/db/mod.rs
    - crates/racecontrol/src/api/routes.rs
    - crates/racecontrol/src/ws/mod.rs
    - crates/rc-agent/src/tier_engine.rs
key_decisions:
  - EvalRecordPayload lives in rc-common (not rc-agent) so server and agent share the type without cross-crate import
  - INSERT OR IGNORE ensures idempotent push — same record_id pushed twice does not duplicate
  - EVAL-03 WS push is best-effort — local EVAL-01 write to mesh_kb.db is authoritative
metrics:
  duration_minutes: 25
  completed_date: "2026-04-01"
  tasks_completed: 2
  files_modified: 6
---

# Phase 290 Plan 03: Evaluation Query API Summary

Server-side evaluation data bridge: `AgentMessage::ModelEvalSync` pushes evaluation records from rc-agent to racecontrol server via WebSocket; server stores in `model_evaluations` SQLite table; `GET /api/v1/models/evaluations` queries records filtered by model/date range.

## Objective

EVAL-03: make evaluation data (stranded in per-pod `mesh_kb.db` files) queryable via a central server API. After this plan, operators can query `GET /api/v1/models/evaluations?model=tier1/deterministic&from=2026-01-01` to see which models are performing well or failing across all pods. This data feeds Phase 292 (model reputation persistence) and Phase 294 (enhanced weekly reports).

## Tasks Completed

| Task | Description | Commit | Files |
|------|-------------|--------|-------|
| 1 | Add EvalRecordPayload + ModelEvalSync to rc-common; add fleet_kb DB/insert/query; wire db migration, route, and WS handler | `95660775` | protocol.rs, fleet_kb.rs, db/mod.rs, routes.rs, ws/mod.rs |
| 2 | Send ModelEvalSync from rc-agent after each EVAL-01 write | `15c9ce49` | tier_engine.rs |

## What Was Built

### rc-common/src/protocol.rs
- New `EvalRecordPayload` struct: `id, model_id, pod_id, trigger_type, prediction, actual_outcome, correct, cost_usd, created_at`
- New `AgentMessage::ModelEvalSync { pod_id, records: Vec<EvalRecordPayload> }` variant

### crates/racecontrol/src/fleet_kb.rs
- `migrate_eval_store(pool)`: creates `model_evaluations` table with indexes on `model_id` and `created_at`
- `insert_eval_record(pool, rec)`: `INSERT OR IGNORE` — idempotent, safe to call twice with same record id
- `query_eval_records(pool, model_id, from, to, limit)`: dynamic WHERE with sqlx::QueryBuilder

### crates/racecontrol/src/db/mod.rs
- Added `crate::fleet_kb::migrate_eval_store(pool).await?;` after the existing fleet_kb::migrate() call

### crates/racecontrol/src/api/routes.rs
- New `EvalQueryParams` struct with `model`, `from`, `to`, `limit` (default 1000)
- New `list_model_evaluations` handler returning `{ records: [...], count: N }`
- Route registered in `staff_routes()`: `.route("/models/evaluations", get(list_model_evaluations))`

### crates/racecontrol/src/ws/mod.rs
- New match arm for `AgentMessage::ModelEvalSync`: iterates records, calls `fleet_kb::insert_eval_record()` for each, logs WARN on failure

### crates/rc-agent/src/tier_engine.rs
- After the EVAL-01 `store.insert(&record)` block, builds `EvalRecordPayload` from record fields
- Sends `AgentMessage::ModelEvalSync` via `ws_msg_tx.send(sync_msg).await`
- Best-effort: failure logged as WARN, does not roll back local write

## Data Flow (end-to-end)

```
rc-agent: tier_engine runs diagnosis
    → store.insert() writes to mesh_kb.db (EVAL-01, local authoritative)
    → ws_msg_tx.send(ModelEvalSync) → WS channel
        → server ws/mod.rs: ModelEvalSync match arm
            → fleet_kb::insert_eval_record() writes to racecontrol.db
                → GET /api/v1/models/evaluations returns records
```

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed pre-existing borrow errors in ExperienceScoreReport handler**
- **Found during:** Task 1 (racecontrol cargo check)
- **Issue:** `AgentMessage::ExperienceScoreReport` match arm used borrowed `pod_id`, `total_score`, `status` as owned values in `fleet.entry(pod_id)`, `Some(total_score)`, `Some(status)` — 3 compiler errors
- **Fix:** Added `.clone()` on `pod_id` and `status`, and `*` deref on `total_score`
- **Files modified:** `crates/racecontrol/src/ws/mod.rs`
- **Commit:** `95660775`

## Verification Results

```
cargo check -p rc-common               → 0 errors
cargo check -p racecontrol-crate       → 0 errors (5 warnings pre-existing)
grep ModelEvalSync crates/rc-common/src/protocol.rs     → line 529
grep migrate_eval_store crates/racecontrol/src/db/mod.rs → line 3143
grep models/evaluations crates/racecontrol/src/api/routes.rs → lines 537, 20363
grep ModelEvalSync crates/racecontrol/src/ws/mod.rs     → line 1565
grep EVAL-03 crates/rc-agent/src/tier_engine.rs         → lines 737, 758
```

Note: `cargo build --release --bin rc-agent` fails on Linux due to pre-existing Windows-only API usage (`std::os::windows::process::CommandExt`) in `self_monitor.rs` and `process_guard.rs`. These are unrelated to EVAL-03 changes. The rc-agent binary is built and deployed on Windows (pod machines).

## Known Stubs

None — all new functions are fully wired end-to-end.

## Self-Check: PASSED

Files exist:
- crates/rc-common/src/protocol.rs: FOUND
- crates/racecontrol/src/fleet_kb.rs: FOUND
- crates/racecontrol/src/db/mod.rs: FOUND
- crates/racecontrol/src/api/routes.rs: FOUND
- crates/racecontrol/src/ws/mod.rs: FOUND
- crates/rc-agent/src/tier_engine.rs: FOUND

Commits exist:
- 95660775: FOUND
- 15c9ce49: FOUND
