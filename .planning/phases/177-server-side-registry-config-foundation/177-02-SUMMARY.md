---
phase: 177-server-side-registry-config-foundation
plan: 02
subsystem: api
tags: [rust, axum, sqlx, sqlite, websocket, config-push, feature-flags]

# Dependency graph
requires:
  - phase: 177-01
    provides: "AppState with feature_flags RwLock, config_push_seq AtomicU64, agent_senders; ConfigPushPayload/ConfigAckPayload/FlagCacheSyncPayload types in rc-common; config_push_queue and config_audit_log DB tables"

provides:
  - "config_push.rs: validate_config_push, push_config, get_queue, get_audit_log, replay_pending_config_pushes"
  - "POST /api/v1/config/push: validates fields, queues per-pod, delivers via ConfigPush WS, writes audit log with seq_num"
  - "GET /api/v1/config/push/queue and GET /api/v1/config/audit endpoints"
  - "FlagCacheSync WS handler: sends full flag state if stale + replays all unacked config pushes on reconnect"
  - "ConfigAck WS handler: marks queue entry acked + updates correct audit log entry by seq_num"
  - "ALTER TABLE config_audit_log ADD COLUMN seq_num migration"

affects: [178-agent-feature-flag-client, 179-ota-pipeline, 180-admin-ui]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Config push validation uses whitelist approach with per-field error messages (not early-return)"
    - "Offline delivery uses status-based queue filter (status != 'acked'), not sequence number comparison"
    - "ConfigAck audit lookup uses WHERE seq_num=? (deterministic), never ORDER BY id DESC LIMIT 1"
    - "Single seq_num per push batch — all pods in batch share same sequence number"

key-files:
  created:
    - crates/racecontrol/src/config_push.rs
  modified:
    - crates/racecontrol/src/api/routes.rs
    - crates/racecontrol/src/db/mod.rs
    - crates/racecontrol/src/lib.rs
    - crates/racecontrol/src/ws/mod.rs

key-decisions:
  - "replay_pending_config_pushes has no last_seq parameter — uses status != 'acked' filter because FlagCacheSync.cached_version is a FLAG version counter, not a config push sequence"
  - "ConfigAck audit lookup matches by seq_num column (added via ALTER TABLE) not by ORDER BY id DESC LIMIT 1 — deterministic under concurrent pushes"
  - "Single seq_num assigned per push batch — all target pods share same sequence for a given push event"

patterns-established:
  - "Status-based queue replay: always filter by status != 'acked', never by sequence number comparison"
  - "Deterministic audit lookup: seq_num column as foreign key equivalent for audit-to-queue correlation"

requirements-completed: [CP-01, CP-02, CP-04, CP-06]

# Metrics
duration: 25min
completed: 2026-03-24
---

# Phase 177 Plan 02: Config Push Validation, Queuing, WS Delivery, and Ack Summary

**Config push REST+WS pipeline: whitelist validation with 400 field-level errors, per-pod SQLite queue with monotonic seq_num, WebSocket ConfigPush delivery, offline replay on reconnect (status-based filter), and deterministic ConfigAck audit lookup by seq_num**

## Performance

- **Duration:** 25 min
- **Started:** 2026-03-24T09:00:00+05:30
- **Completed:** 2026-03-24T09:25:00+05:30
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- Created `config_push.rs` with full REST handler set: `push_config`, `get_queue`, `get_audit_log`, `validate_config_push`, `replay_pending_config_pushes`
- Whitelist validation rejects unknown fields and invalid values (billing_rate, game_limit, debug_verbosity, process_guard_whitelist) with 400 + field-level error map
- Per-pod queue entries written with shared seq_num; online pods receive ConfigPush WS message immediately; offline pods hold `status='pending'` for reconnect replay
- Added `FlagCacheSync` and `ConfigAck` WS message handlers in `ws/mod.rs` before catch-all
- `FlagCacheSync` handler sends full flag state to stale pods then replays all unacked config pushes — replay uses `status != 'acked'` filter only, no `last_seq` parameter
- `ConfigAck` handler marks queue `status='acked'` and updates audit log `pods_acked` field by matching `seq_num` column (deterministic lookup, not recency-based)
- `ALTER TABLE config_audit_log ADD COLUMN seq_num INTEGER` migration added in `db/mod.rs`

## Task Commits

Each task was committed atomically:

1. **Task 1: config_push.rs module with validation, queuing, and delivery** - `3ef83189` (feat)
2. **Task 2: WS handlers for FlagCacheSync and ConfigAck** - `bb1ead5c` (feat)

**Plan metadata:** (docs commit follows)

## Files Created/Modified
- `crates/racecontrol/src/config_push.rs` - New module: validate_config_push, push_config, get_queue, get_audit_log, replay_pending_config_pushes
- `crates/racecontrol/src/ws/mod.rs` - FlagCacheSync and ConfigAck handlers added before catch-all
- `crates/racecontrol/src/api/routes.rs` - config/push, config/push/queue, config/audit routes in staff_routes; import added
- `crates/racecontrol/src/db/mod.rs` - ALTER TABLE migration for config_audit_log.seq_num
- `crates/racecontrol/src/lib.rs` - pub mod config_push added

## Decisions Made
- `replay_pending_config_pushes` signature has no `last_seq` parameter. The plan was explicit: `FlagCacheSync.cached_version` is a FLAG version counter. Using it as a config push sequence filter would silently skip pushes with low sequence numbers. Status-based filter (`status != 'acked'`) is the correct approach.
- ConfigAck audit update uses `WHERE entity_type = 'config' AND seq_num = ?` instead of recency-based lookup, because concurrent pushes could create multiple audit entries and ORDER BY id DESC LIMIT 1 would update the wrong one.
- Single `seq_num` assigned per push batch via `config_push_seq.fetch_add(1, SeqCst)` — all target pods in one push call share the same sequence number.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Config push REST + WS pipeline complete
- rc-agent (Phase 178) can now receive ConfigPush messages and send ConfigAck
- Admin UI (Phase 180) can use POST /api/v1/config/push, GET /api/v1/config/push/queue, GET /api/v1/config/audit
- No blockers

---
*Phase: 177-server-side-registry-config-foundation*
*Completed: 2026-03-24*
