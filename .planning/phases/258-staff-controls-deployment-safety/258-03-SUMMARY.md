---
phase: 258-staff-controls-deployment-safety
plan: 03
subsystem: billing
tags: [rust, axum, sqlite, websocket, deployment-safety, billing-recovery]

# Dependency graph
requires:
  - phase: 252-financial-atomicity-core
    provides: compute_refund(), end_billing_session_public(), CAS guard
  - phase: 257-billing-edge-cases
    provides: end_billing_session_public() with EndedEarly status and refund logic
provides:
  - DEPLOY-02: Agent graceful shutdown with billing session persistence
  - DEPLOY-04: Post-restart interrupted session recovery via sentinel files
  - DEPLOY-05: WebSocket command_id deduplication to prevent stale replay
affects:
  - fleet deploys, rc-agent restarts, WS reconnection reliability

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "CoreMessage wrapper: optional command_id field on all server→agent WS messages (backward-compatible)"
    - "Sentinel file protocol: INTERRUPTED_SESSION_{id}.json on shutdown failure, consumed on startup"
    - "Deduplication via HashMap<String, Instant> with 5-min TTL and periodic prune"
    - "Service key header validation (Bearer) for pod→server HTTP endpoints without JWT"

key-files:
  created: []
  modified:
    - crates/rc-agent/src/main.rs
    - crates/rc-agent/src/ws_handler.rs
    - crates/rc-agent/src/event_loop.rs
    - crates/rc-common/src/protocol.rs
    - crates/racecontrol/src/billing.rs
    - crates/racecontrol/src/api/routes.rs
    - crates/racecontrol/src/ws/mod.rs
    - crates/racecontrol/src/db/mod.rs

key-decisions:
  - "CoreMessage wrapper uses #[serde(flatten)] so old agents (no command_id) still parse via bare CoreToAgentMessage fallback"
  - "Sentinel files written as JSON to C:\\RacingPoint\\INTERRUPTED_SESSION_{session_id}.json — consumed on next startup after WS connect"
  - "agent_shutdown endpoint in public_routes (no JWT), gated by sentry_service_key Bearer header; allows agent to call without staff auth"
  - "recover_interrupted_sessions() only runs once per process lifetime (flag in reconnect loop) after first WS connect"
  - "shutdown_at stored as TEXT (SQLite datetime) — ALTER TABLE migration; only set when NULL to be idempotent"
  - "Dedup cleanup tracked by dedup_cleanup_ticks counter (per message, not per heartbeat tick) — pruning fires after 60 WS messages"

patterns-established:
  - "Idempotency pattern: agent-shutdown and interrupted-sessions endpoints both use CAS guard in end_billing_session"
  - "Feature-gated HTTP: reqwest calls in shutdown handler wrapped in #[cfg(feature = http-client)] — safe for non-http builds"

requirements-completed: [DEPLOY-02, DEPLOY-04, DEPLOY-05]

# Metrics
duration: 38min
completed: 2026-03-29
---

# Phase 258 Plan 03: Staff Controls & Deployment Safety (Agent-Side) Summary

**Graceful agent shutdown with billing session persistence, post-restart interrupted session recovery, and WS command_id deduplication preventing stale replay on reconnect**

## Performance

- **Duration:** 38 min
- **Started:** 2026-03-29T08:20:00Z
- **Completed:** 2026-03-29T08:58:44Z
- **Tasks:** 2
- **Files modified:** 8

## Accomplishments
- DEPLOY-02: Agent reads active billing session on SIGTERM, POSTs to server for partial refund; writes sentinel file if server unreachable
- DEPLOY-04: On startup after WS connect, agent scans for INTERRUPTED_SESSION_*.json sentinels, replays agent-shutdown call for each
- DEPLOY-05: `CoreMessage` wrapper added to protocol.rs; server wraps every outbound WS message with UUID command_id; agent deduplicates within 5-min TTL

## Task Commits

Each task was committed atomically:

1. **Task 1: Graceful shutdown and post-restart session recovery** - `74b11b47` (feat)
2. **Task 2: WebSocket command_id deduplication** - `c9fa9b2a` (feat)

## Files Created/Modified
- `crates/rc-agent/src/main.rs` - write_interrupted_session_sentinel(), recover_interrupted_sessions(), shutdown handler with DEPLOY-02 HTTP call, post-startup recovery task
- `crates/rc-agent/src/ws_handler.rs` - CoreMessage parse + command_id dedup logic
- `crates/rc-agent/src/event_loop.rs` - ConnectionState gains seen_command_ids + dedup_cleanup_ticks
- `crates/rc-common/src/protocol.rs` - CoreMessage struct with command_id + CoreMessage::wrap() helper
- `crates/racecontrol/src/billing.rs` - handle_agent_shutdown(), handle_interrupted_sessions_check()
- `crates/racecontrol/src/api/routes.rs` - POST /billing/{id}/agent-shutdown, GET /billing/pod/{pod_id}/interrupted in public_routes
- `crates/racecontrol/src/ws/mod.rs` - CoreMessage::wrap() applied at single serialization point in send loop
- `crates/racecontrol/src/db/mod.rs` - ALTER TABLE billing_sessions ADD COLUMN shutdown_at TEXT migration

## Decisions Made
- CoreMessage wrapper uses `#[serde(flatten)]` so old agents parse it as bare CoreToAgentMessage via fallback path — backward-compatible rolling deploy
- agent-shutdown endpoint placed in public_routes (no JWT); Bearer token validation against sentry_service_key is sufficient since agents are LAN-local
- Sentinel files provide offline resilience: if server is down during shutdown, the sentinel ensures refund is processed on next restart even days later
- recover_interrupted_sessions() is #[cfg(feature = "http-client")] gated, matching existing pattern for reqwest usage in rc-agent
- Dedup counter dedup_cleanup_ticks counts WS messages (not time) for simplicity — prune fires after 60 messages as a proxy for ~60s

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] glob crate unavailable in rc-agent**
- **Found during:** Task 1 (post-restart recovery implementation)
- **Issue:** Plan suggested using glob::glob() for INTERRUPTED_SESSION_*.json scanning, but `glob` is not in rc-agent's Cargo.toml
- **Fix:** Used std::fs::read_dir() + manual prefix/suffix check — no new dependency needed
- **Files modified:** crates/rc-agent/src/main.rs
- **Verification:** cargo check passes; directory scanning logic is equivalent
- **Committed in:** 74b11b47

**2. [Rule 3 - Blocking] axum::extract::TypedHeader removed in axum 0.8**
- **Found during:** Task 1 (agent_shutdown_handler implementation)
- **Issue:** Plan's example used TypedHeader for Bearer auth extraction, but axum 0.8 removed TypedHeader (requires axum-extra)
- **Fix:** Used axum::http::HeaderMap + manual .get("authorization") + .strip_prefix("Bearer ")
- **Files modified:** crates/racecontrol/src/api/routes.rs
- **Verification:** cargo check passes; auth extraction equivalent
- **Committed in:** 74b11b47

---

**Total deviations:** 2 auto-fixed (2 blocking)
**Impact on plan:** Both fixes required for compilation. No scope creep.

## Issues Encountered
- billing.rs and routes.rs showed as clean vs HEAD after my Edit calls — investigation revealed 258-01 parallel plan already committed those files with the same content in commit 3257b077. The functions were already present from the parallel executor, confirming no conflicts.

## Next Phase Readiness
- DEPLOY-02/04/05 complete — agent now survives rolling deploys without billing session loss
- WS command deduplication prevents double-start scenarios on reconnect
- Ready for Phase 258 ship gate: all STAFF-01–05 and DEPLOY-01–05 requirements addressed across plans 01–03

## Self-Check: PASSED
- SUMMARY.md: FOUND at .planning/phases/258-staff-controls-deployment-safety/258-03-SUMMARY.md
- Commit 74b11b47: FOUND (Task 1 — DEPLOY-02/04)
- Commit c9fa9b2a: FOUND (Task 2 — DEPLOY-05)

---
*Phase: 258-staff-controls-deployment-safety*
*Completed: 2026-03-29*
