---
phase: 292-model-reputation-persistence
plan: 02
subsystem: api
tags: [sqlx, sqlite, model-reputation, websocket, protocol, rest-api, server-sync]

# Dependency graph
requires:
  - phase: 292-01
    provides: ModelReputationStore, load_into_memory, run_reputation_sweep with rep_store
  - phase: 290-model-evaluation-store
    provides: ModelEvalSync WS pattern + fleet_kb sqlx patterns to replicate

provides:
  - ReputationPayload struct in rc-common/protocol.rs (wire format)
  - AgentMessage::ModelReputationSync variant for WS transport
  - fleet_kb::migrate_reputation_store() + upsert_reputation() + query_reputation()
  - GET /api/v1/models/reputation endpoint in staff_routes
  - WS handler for ModelReputationSync in ws/mod.rs
  - rc-agent sweep pushes ModelReputationSync after each daily sweep

affects: [294-weekly-report, admin-dashboard, cloud-platform]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Mirror eval sync pattern: rc-agent push → WS → server upsert → REST query"
    - "blocking_send() for sync→async boundary inside tokio::task::block_in_place"
    - "ON CONFLICT DO UPDATE for idempotent server-side reputation upsert"

key-files:
  created: []
  modified:
    - crates/rc-common/src/protocol.rs
    - crates/racecontrol/src/fleet_kb.rs
    - crates/racecontrol/src/db/mod.rs
    - crates/racecontrol/src/api/routes.rs
    - crates/racecontrol/src/ws/mod.rs
    - crates/rc-agent/src/model_reputation.rs
    - crates/rc-agent/src/main.rs

key-decisions:
  - "Use blocking_send() not .await in run_reputation_sweep() — function is sync (rusqlite), called via block_in_place; can't use .await there"
  - "pod_id in ModelReputationSync is 'local' since reputation sweep is node-global, not per-pod; server upserts use model_id as PK so multi-pod would naturally merge"
  - "WS send is best-effort — failure logs warn but does NOT prevent local persistence (MREP-01..03 still complete)"
  - "ReputationPayload is the wire format; ReputationRow (rc-agent) is the DB format — separate structs to avoid cross-crate dependency"

patterns-established:
  - "Agent push pattern: rc-agent collects local data → pushes to server via AgentMessage WS variant → server upserts → REST query API"

requirements-completed: [MREP-04]

# Metrics
duration: 20min
completed: 2026-04-01
---

# Phase 292 Plan 02: Model Reputation Sync API Summary

**End-to-end model reputation pipeline: rc-agent pushes ReputationPayload to server via WS after each sweep; GET /api/v1/models/reputation exposes per-model accuracy/status/cost sorted by accuracy DESC**

## Performance

- **Duration:** 20 min
- **Started:** 2026-04-01T14:53:00Z
- **Completed:** 2026-04-01T15:13:00Z
- **Tasks:** 2 (combined in 1 commit)
- **Files modified:** 7

## Accomplishments
- `ReputationPayload` struct and `AgentMessage::ModelReputationSync` added to rc-common/protocol.rs — all consumers share the wire format
- Server-side `model_reputation` table with `migrate_reputation_store()`, `upsert_reputation()`, `query_reputation()` in fleet_kb.rs following exact sqlx async patterns from ModelEvalSync
- `GET /api/v1/models/reputation?status=<filter>` in staff_routes — returns `{models: [...], count: N}` sorted by accuracy DESC
- WS handler processes `ModelReputationSync` — upserts each row idempotently via ON CONFLICT DO UPDATE
- `run_reputation_sweep()` sends `ModelReputationSync` after sweep via `ws_msg_tx.blocking_send()` — best-effort, failure doesn't abort sweep
- All three crates (rc-common, racecontrol, rc-agent) compile cleanly (pre-existing Windows-only errors unchanged)

## Task Commits

1. **Task 1+2: Protocol + server DB/API + WS handler + rc-agent push** - `1ae085d7` (feat)

## Files Created/Modified
- `crates/rc-common/src/protocol.rs` - Added ReputationPayload struct + ModelReputationSync enum variant
- `crates/racecontrol/src/fleet_kb.rs` - Added migrate_reputation_store(), upsert_reputation(), query_reputation()
- `crates/racecontrol/src/db/mod.rs` - Wired migrate_reputation_store() call after migrate_eval_store()
- `crates/racecontrol/src/api/routes.rs` - Added /models/reputation route + list_model_reputation handler
- `crates/racecontrol/src/ws/mod.rs` - Added ModelReputationSync match arm — upserts each row
- `crates/rc-agent/src/model_reputation.rs` - Added ws_msg_tx param, builds rep_rows vec, sends ModelReputationSync via blocking_send after sweep
- `crates/rc-agent/src/main.rs` - Updated sweep task to pass rep_ws_tx = ws_exec_result_tx.clone()

## Decisions Made
- Tasks 1 and 2 combined into single commit — tightly coupled changes across 7 files that would be non-compilable in any intermediate state
- `blocking_send()` used instead of `.await` because run_reputation_sweep is a sync fn called inside `tokio::task::block_in_place`; using async send would require making the function async or adding a separate spawn
- WS send failure is logged as WARN but sweep completes normally — local persistence (MREP-01..03) takes priority over server sync

## Deviations from Plan

None — plan executed exactly as written. All 6 files from the plan's files_modified list were updated.

## Issues Encountered
None — all compilation succeeded on first attempt.

## Known Stubs
None — all production code paths complete. The `cost_per_correct_usd` field is now computed from per_model_cost tracking in the sweep (total_cost / correct_count for each model); defaults to 0.0 for models with no correct diagnoses in the 7-day window.

## Next Phase Readiness
- Phase 293 (Retrain Export) can use model_evaluations data independently
- Phase 294 (Weekly Report v2) can now query GET /api/v1/models/reputation for per-model trends
- Server model_reputation table available for Admin dashboard integration

---
*Phase: 292-model-reputation-persistence*
*Completed: 2026-04-01*
