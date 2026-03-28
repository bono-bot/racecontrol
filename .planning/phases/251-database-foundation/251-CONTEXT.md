# Phase 251: Database Foundation - Context

**Gathered:** 2026-03-29
**Status:** Ready for planning
**Mode:** Auto-generated (infrastructure phase, discuss skipped)

<domain>
## Phase Boundary

The SQLite database layer is stable under concurrent writes, timer state survives server restarts, and orphaned sessions are automatically detected. This is the foundational infrastructure phase that all other v27.0 phases depend on.

Requirements: RESIL-01 (WAL mode), RESIL-02 (staggered writes), FSM-09 (timer persistence), FSM-10 (orphan detection on startup), RESIL-03 (orphan detection background job)

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase. Use ROADMAP phase goal, success criteria, and codebase conventions to guide decisions.

Key implementation notes:
- SQLite WAL mode: enable via PRAGMA at DB connection init in `crates/racecontrol/src/db/mod.rs`
- busy_timeout=5000ms: set via PRAGMA or sqlx connection options
- Billing timer persistence: write elapsed_seconds + status to billing_sessions table every 60s
- Stagger by pod index: Pod N writes at second (N * 7) % 60 within each minute
- Orphaned session detection: background tokio task every 5 minutes, check last_heartbeat_at vs now
- On server startup: scan billing_sessions WHERE status='active' AND last_heartbeat_at < now - 5min

</decisions>

<code_context>
## Existing Code Insights

### Key Files
- `crates/racecontrol/src/db/mod.rs` — DB initialization, migrations, pool creation
- `crates/racecontrol/src/billing.rs` — BillingTimer, tick logic, session management
- `crates/racecontrol/src/api/routes.rs` — Billing API endpoints
- `crates/racecontrol/src/main.rs` — Server startup, background task spawning

### Established Patterns
- SQLite via sqlx with connection pool
- Background tasks via tokio::spawn
- Billing timers are in-memory HashMap<pod_id, BillingTimer>
- billing_sessions table has driving_seconds, status, started_at, ended_at columns

</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase. Refer to ROADMAP phase description and success criteria.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
