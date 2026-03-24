# Phase 183: Recovery Events API - Context

**Gathered:** 2026-03-25
**Status:** Ready for planning

<domain>
## Phase Boundary

Server-side recovery events endpoint in racecontrol that all recovery authorities (rc-sentry, pod_healer, self_monitor) report to and query. Ring-buffered in-memory storage with POST (report) and GET (query) routes.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase.

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `rc-common/src/recovery.rs` — `RecoveryAuthority` enum (RcSentry, PodHealer, JamesMonitor), `ProcessOwnership` registry, `OwnershipConflict`, JSONL recovery log constants
- `state.rs` — `AppState` struct with existing `WatchdogState` enum (Healthy, Restarting, Verifying, RecoveryFailed)
- `api/routes.rs` — central route registration (both `public_routes` and auth-gated routes)
- `whatsapp_alerter.rs` — existing WhatsApp alert integration for Tier 4 escalation
- `fleet_health.rs` — `FleetHealthStore`, `ViolationStore` patterns for in-memory stores

### Established Patterns
- API routes defined in `api/routes.rs` with `Router::new()` chaining
- `AppState` is Axum state, shared via `Arc<AppState>` across handlers
- In-memory stores use `Mutex<VecDeque>` or `RwLock<HashMap>` on AppState fields
- Public (no-auth) routes added to `public_routes`, staff routes require JWT middleware

### Integration Points
- New recovery events ring buffer added as field on `AppState`
- New route handlers in a new `recovery.rs` module under `crates/racecontrol/src/`
- GET endpoint must be public (rc-sentry queries without auth, like kiosk-allowlist pattern)
- POST endpoint should also be public (rc-sentry reports from pods without staff JWT)

</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
