# Phase 177: Server-Side Registry + Config Foundation - Context

**Gathered:** 2026-03-24
**Status:** Ready for planning

<domain>
## Phase Boundary

Build the server-side feature flag registry, config push channel, and REST endpoints. Operators can create/toggle boolean flags with per-pod overrides, push config changes that queue for offline pods, and all mutations are audit-logged. OpenAPI spec and shared TypeScript types updated.

</domain>

<decisions>
## Implementation Decisions

### Flag Storage & API Design
- Single `feature_flags` table (name TEXT PK, enabled BOOLEAN, default_value BOOLEAN, overrides JSON, version INTEGER, updated_at TEXT) — matches existing `kiosk_settings` pattern
- REST endpoints: `GET/POST /api/v1/flags` + `PUT /api/v1/flags/:name` — staff-tier auth via `require_staff_jwt` middleware
- Per-pod overrides stored as JSON column: `{"pod_3": true, "pod_8": false}` — simple for 8-pod fleet
- Monotonic integer `version` per flag, incremented on every mutation — pods track last-seen version

### Config Push & Delivery
- SQLite `config_push_queue` table (id INTEGER PK, pod_id TEXT, payload JSON, seq_num INTEGER, status TEXT, created_at TEXT, acked_at TEXT) — survives server restart
- Delivery via `CoreToAgentMessage::ConfigPush` over existing per-pod mpsc channels — no new transport
- Reconnect sync: pod sends last-seen `seq_num` on reconnect → server replays all queued pushes with seq > that value
- Schema-based validation in `validate_config_push()` fn — whitelist of known fields with type/range checks (billing_rate > 0, game_limit 1-10, etc.)

### Audit Log & Cross-Project Sync
- `config_audit_log` table (id INTEGER PK, action TEXT, entity_type TEXT, entity_name TEXT, old_value TEXT, new_value TEXT, pushed_by TEXT, pods_acked JSON, created_at TEXT) — append-only
- `pushed_by` = staff JWT `sub` claim (email/name) extracted from auth middleware
- OpenAPI: add to existing `docs/openapi.yaml` under new `Feature Flags` and `Config Push` tags — 6 new endpoints, 4 new schemas
- TypeScript: new `packages/shared-types/src/config.ts` exporting `FeatureFlag`, `ConfigPush`, `ConfigAuditEntry` — re-exported from index.ts

### Claude's Discretion
- Internal module organization (separate `flags.rs` vs inline in existing modules)
- Error response format details beyond the required 400 + field-level errors
- Exact contract test fixture structure

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `state.agent_senders: RwLock<HashMap<String, mpsc::Sender<CoreToAgentMessage>>>` — per-pod WS channels, ready for FlagSync/ConfigPush delivery
- `db/mod.rs` migration pattern — `CREATE TABLE IF NOT EXISTS` inline in `migrate()` fn
- `routes.rs` staff-tier router with `require_staff_jwt` middleware
- `CoreToAgentMessage::FlagSync`, `::ConfigPush` variants already in protocol.rs (Phase 176)
- `FlagSyncPayload`, `ConfigPushPayload` structs already in types.rs (Phase 176)
- Existing `broadcast_settings()` in state.rs as pattern for pushing to all connected pods

### Established Patterns
- SQLite via sqlx async pool (max 5 connections, WAL mode)
- Axum extractors: `State(state)`, `Path(id)`, `Json(body)` → `Json<T>` response
- Staff auth: `require_staff_jwt` middleware extracts claims
- Dashboard events: `state.dashboard_tx.send(DashboardEvent::...)` for real-time UI updates

### Integration Points
- `crates/racecontrol/src/db/mod.rs` — add 3 new tables in migrate()
- `crates/racecontrol/src/api/routes.rs` — add flag + config endpoints to staff tier
- `crates/racecontrol/src/state.rs` — add feature_flags HashMap + config_push_queue to AppState
- `crates/racecontrol/src/ws/mod.rs` — send FlagSync on pod connect, handle ConfigAck
- `docs/openapi.yaml` — add schemas + endpoints
- `packages/shared-types/src/config.ts` — new file
- `packages/shared-types/src/index.ts` — re-export config types
- `packages/contract-tests/` — new fixture + test for flag/config endpoints

</code_context>

<specifics>
## Specific Ideas

- Standing rule: Config push must NEVER route through fleet exec endpoint — WebSocket typed ConfigPush only (from STATE.md decisions)
- Config push must handle pod offline gracefully (queue + retry when pod reconnects)
- Billing session state must be preserved during config pushes — never lose active session data

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
