# Phase 177: Server-Side Registry + Config Foundation - Research

**Researched:** 2026-03-24
**Domain:** Rust/Axum SQLite REST API — feature flag registry + config push queue + audit log
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Flag Storage & API Design**
- Single `feature_flags` table (name TEXT PK, enabled BOOLEAN, default_value BOOLEAN, overrides JSON, version INTEGER, updated_at TEXT) — matches existing `kiosk_settings` pattern
- REST endpoints: `GET/POST /api/v1/flags` + `PUT /api/v1/flags/:name` — staff-tier auth via `require_staff_jwt` middleware
- Per-pod overrides stored as JSON column: `{"pod_3": true, "pod_8": false}` — simple for 8-pod fleet
- Monotonic integer `version` per flag, incremented on every mutation — pods track last-seen version

**Config Push & Delivery**
- SQLite `config_push_queue` table (id INTEGER PK, pod_id TEXT, payload JSON, seq_num INTEGER, status TEXT, created_at TEXT, acked_at TEXT) — survives server restart
- Delivery via `CoreToAgentMessage::ConfigPush` over existing per-pod mpsc channels — no new transport
- Reconnect sync: pod sends last-seen `seq_num` on reconnect → server replays all queued pushes with seq > that value
- Schema-based validation in `validate_config_push()` fn — whitelist of known fields with type/range checks (billing_rate > 0, game_limit 1-10, etc.)

**Audit Log & Cross-Project Sync**
- `config_audit_log` table (id INTEGER PK, action TEXT, entity_type TEXT, entity_name TEXT, old_value TEXT, new_value TEXT, pushed_by TEXT, pods_acked JSON, created_at TEXT) — append-only
- `pushed_by` = staff JWT `sub` claim (email/name) extracted from auth middleware
- OpenAPI: add to existing `docs/openapi.yaml` under new `Feature Flags` and `Config Push` tags — 6 new endpoints, 4 new schemas
- TypeScript: new `packages/shared-types/src/config.ts` exporting `FeatureFlag`, `ConfigPush`, `ConfigAuditEntry` — re-exported from index.ts

### Claude's Discretion
- Internal module organization (separate `flags.rs` vs inline in existing modules)
- Error response format details beyond the required 400 + field-level errors
- Exact contract test fixture structure

### Deferred Ideas (OUT OF SCOPE)
None — discussion stayed within phase scope.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| FF-01 | Server maintains a central named boolean feature flag registry backed by SQLite with fleet-wide defaults | `feature_flags` table pattern, sqlx async pool, AppState flag cache |
| FF-02 | Operator can set per-pod flag overrides (e.g., enable AC EVO telemetry on Pod 8 only) | JSON column overrides pattern, existing `broadcast_settings` per-pod fan-out |
| FF-03 | Flag changes are delivered to pods over the existing WebSocket connection as typed messages | `CoreToAgentMessage::FlagSync(FlagSyncPayload)` already in protocol.rs; `agent_senders` mpsc map in AppState |
| CP-01 | Server pushes config changes over WebSocket as typed ConfigPush — never through fleet exec endpoint | `CoreToAgentMessage::ConfigPush(ConfigPushPayload)` exists; standing rule in CONTEXT.md + STATE.md |
| CP-02 | Server maintains a pending config queue per pod — offline pods receive queued updates on reconnect | `config_push_queue` SQLite table; `FlagCacheSyncPayload.cached_version` pattern for reconnect sync |
| CP-04 | Config push includes schema version — rc-agent ignores unknown fields from newer schema versions | `ConfigPushPayload.schema_version: u32` already defined in types.rs |
| CP-05 | All config changes recorded in append-only audit log table | `config_audit_log` table; `pushed_by` from `StaffClaims.sub` |
| CP-06 | Server validates config changes against schema before accepting — invalid values return 400 with field-level errors | `validate_config_push()` fn pattern; Axum Json extractor + custom error response |
| SYNC-01 | Feature flag and config push APIs documented in OpenAPI 3.0 spec with shared TypeScript types | Extend `docs/openapi.yaml`; new `packages/shared-types/src/config.ts` |
</phase_requirements>

---

## Summary

Phase 177 builds the server side of the v22.0 feature management system: three new SQLite tables, six REST endpoints, WebSocket delivery of flag/config messages to pods, and OpenAPI + TypeScript type exports. All infrastructure (protocol variants, payload types, auth middleware, mpsc delivery channels) was laid in Phase 176 — Phase 177 wires those pieces together with persistent storage, validation, and audit trails.

The codebase is mature and highly patterned. Every new construct has an established analog: `feature_flags` mirrors `kiosk_settings`, config push delivery mirrors `broadcast_settings()`, staff endpoint registration mirrors the existing `staff_routes()` block, and TypeScript types follow the same `interface + re-export` structure as `fleet.ts`. Contract tests follow the `assert*()` + fixture JSON pattern in `src/contract-tests/`.

**Primary recommendation:** Implement as a dedicated `flags.rs` module (handler functions + DB helpers) and a `config_push.rs` module, registered in `routes.rs` staff tier. Use the `broadcast_settings()` pattern for fan-out. The `validate_config_push()` function is the only truly novel logic — all other pieces are pattern applications.

---

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| sqlx | already in Cargo.toml | Async SQLite queries, type-checked at compile time | Used everywhere in racecontrol; WAL pool in `db/mod.rs` |
| axum | already in Cargo.toml | HTTP routing, extractors (`State`, `Path`, `Json`) | Entire server is Axum |
| serde / serde_json | already in Cargo.toml | JSON serialization for overrides column, response bodies | All types derived |
| tokio::sync::mpsc | std | Per-pod command channel delivery | `agent_senders` map already uses this |
| jsonwebtoken | already in Cargo.toml | JWT decode in middleware | `StaffClaims.sub` extraction pattern exists |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| chrono | already in Cargo.toml | `datetime('now')` timestamps, `updated_at` TEXT fields | Consistent timestamp formatting |
| tracing | already in Cargo.toml | Structured logging for audit events | Already used fleet-wide |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| JSON overrides column | Separate `flag_overrides` join table | Join table is cleaner for large fleets; JSON column is simpler and sufficient for 8 pods |
| Custom error type for 400 | axum `(StatusCode, Json)` tuple | Custom type adds indirection; tuple is idiomatic and readable |

**No new dependencies needed.** All required libraries are already in `Cargo.toml`.

---

## Architecture Patterns

### Recommended Module Organization

```
crates/racecontrol/src/
├── flags.rs            # Feature flag handlers + DB helpers (new)
├── config_push.rs      # Config push handlers + validation (new)
├── api/
│   └── routes.rs       # Add flag + config endpoints to staff_routes()
├── db/
│   └── mod.rs          # Add 3 new tables to migrate()
├── state.rs            # Add feature_flags HashMap cache to AppState
└── ws/
    └── mod.rs          # Handle FlagCacheSync on reconnect, handle ConfigAck

packages/shared-types/src/
├── config.ts           # New: FeatureFlag, ConfigPush, ConfigAuditEntry
└── index.ts            # Re-export from config.ts

packages/contract-tests/src/
├── flags.contract.test.ts    # New
├── config.contract.test.ts   # New
└── fixtures/
    ├── flags.json             # New
    └── config-push.json       # New

docs/
└── openapi.yaml        # Add Feature Flags + Config Push tags/paths/schemas
```

### Pattern 1: SQLite Table Addition (migrate() pattern)
**What:** Add 3 new `CREATE TABLE IF NOT EXISTS` blocks to `db/mod.rs` `migrate()` function, followed by `ALTER TABLE ADD COLUMN IF NOT EXISTS` guards for any columns added to existing tables.
**When to use:** All new persistent state.

```rust
// Source: crates/racecontrol/src/db/mod.rs — existing pattern
sqlx::query(
    "CREATE TABLE IF NOT EXISTS feature_flags (
        name TEXT PRIMARY KEY,
        enabled BOOLEAN NOT NULL DEFAULT 0,
        default_value BOOLEAN NOT NULL DEFAULT 0,
        overrides TEXT NOT NULL DEFAULT '{}',
        version INTEGER NOT NULL DEFAULT 1,
        updated_at TEXT DEFAULT (datetime('now'))
    )",
)
.execute(pool)
.await?;

sqlx::query(
    "CREATE TABLE IF NOT EXISTS config_push_queue (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        pod_id TEXT NOT NULL,
        payload TEXT NOT NULL,
        seq_num INTEGER NOT NULL,
        status TEXT NOT NULL DEFAULT 'pending',
        created_at TEXT DEFAULT (datetime('now')),
        acked_at TEXT
    )",
)
.execute(pool)
.await?;

sqlx::query(
    "CREATE TABLE IF NOT EXISTS config_audit_log (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        action TEXT NOT NULL,
        entity_type TEXT NOT NULL,
        entity_name TEXT NOT NULL,
        old_value TEXT,
        new_value TEXT,
        pushed_by TEXT NOT NULL,
        pods_acked TEXT NOT NULL DEFAULT '[]',
        created_at TEXT DEFAULT (datetime('now'))
    )",
)
.execute(pool)
.await?;
```

### Pattern 2: Staff Route Registration
**What:** Add new handler functions to the `staff_routes()` Router block in `api/routes.rs`. The block already has `require_staff_jwt` + `require_non_pod_source` middleware applied at the bottom.
**When to use:** Any endpoint that requires staff authentication and must be blocked from pod source IPs.

```rust
// Source: crates/racecontrol/src/api/routes.rs — lines 197-401
// Add inside staff_routes() Router::new() chain:
.route("/flags", get(flags::list_flags).post(flags::create_flag))
.route("/flags/:name", put(flags::update_flag))
.route("/config/push", post(config_push::push_config))
.route("/config/push/queue", get(config_push::get_queue))
.route("/config/audit", get(config_push::get_audit_log))
```

### Pattern 3: Axum Handler with StaffClaims Extraction
**What:** Extract `pushed_by` from the JWT sub claim already placed in request extensions by `require_staff_jwt` middleware.
**When to use:** Any mutation endpoint that needs an audit identity.

```rust
// Source: crates/racecontrol/src/auth/middleware.rs — StaffClaims pattern
use axum::extract::Extension;
use crate::auth::middleware::StaffClaims;

pub async fn update_flag(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<StaffClaims>,
    Path(name): Path<String>,
    Json(body): Json<UpdateFlagRequest>,
) -> Result<Json<FeatureFlagRow>, (StatusCode, Json<serde_json::Value>)> {
    let pushed_by = &claims.sub;  // email or staff ID
    // ... DB update, audit log insert, broadcast FlagSync
}
```

### Pattern 4: Per-Pod Fan-Out (broadcast_settings analog)
**What:** Iterate `agent_senders` RwLock map, send typed message to each connected pod. Disconnected pods (not in map) will receive updates via reconnect sync.
**When to use:** Flag changes that must propagate to all connected pods immediately (FF-03, FF-07).

```rust
// Source: crates/racecontrol/src/state.rs lines 255-300 — broadcast_settings pattern
pub async fn broadcast_flag_sync(state: &AppState, flags: HashMap<String, bool>, version: u64) {
    let agent_senders = state.agent_senders.read().await;
    let payload = FlagSyncPayload { flags, version };
    for (pod_id, sender) in agent_senders.iter() {
        if let Err(e) = sender.send(CoreToAgentMessage::FlagSync(payload.clone())).await {
            tracing::warn!("Failed to send FlagSync to {}: {}", pod_id, e);
        }
    }
}
```

### Pattern 5: Reconnect Sync via FlagCacheSync
**What:** When a pod reconnects and sends `AgentMessage::FlagCacheSync(payload)`, the WS handler reads `payload.cached_version`, compares to current server flag version, and sends a full `FlagSync` if the pod is stale. For config push, replays all `config_push_queue` entries with `seq_num > payload.cached_version` and `pod_id = this_pod`.
**When to use:** WS connect handler for pods (ws/mod.rs).

```rust
// Source: rc-common/src/protocol.rs — FlagCacheSyncPayload
// Handle in ws/mod.rs AgentMessage::FlagCacheSync branch:
AgentMessage::FlagCacheSync(payload) => {
    if payload.cached_version < current_flag_version {
        // Send full flag state
        let _ = cmd_tx.send(CoreToAgentMessage::FlagSync(full_flag_payload)).await;
    }
    // Replay pending config pushes for this pod
    replay_pending_config_pushes(&state, pod_id, payload.cached_version, &cmd_tx).await;
}
```

### Pattern 6: validate_config_push() — Field Whitelist Validation
**What:** Whitelist-based validation. Check that all submitted fields are known, then validate each known field's type and range. Return 400 with a `{ "errors": { "field": "reason" } }` body on failure.
**When to use:** `POST /api/v1/config/push` handler before queuing.

```rust
// New code — no direct analog, but error response pattern matches existing 400s
fn validate_config_push(fields: &HashMap<String, serde_json::Value>)
    -> Result<(), HashMap<String, String>>
{
    let mut errors = HashMap::new();
    for (key, value) in fields {
        match key.as_str() {
            "billing_rate" => {
                if value.as_f64().map(|v| v <= 0.0).unwrap_or(true) {
                    errors.insert(key.clone(), "must be > 0".into());
                }
            }
            "game_limit" => {
                if value.as_u64().map(|v| v < 1 || v > 10).unwrap_or(true) {
                    errors.insert(key.clone(), "must be 1-10".into());
                }
            }
            "debug_verbosity" => {
                let valid = ["off", "error", "warn", "info", "debug", "trace"];
                if !value.as_str().map(|s| valid.contains(&s)).unwrap_or(false) {
                    errors.insert(key.clone(), format!("must be one of {:?}", valid));
                }
            }
            _ => {
                errors.insert(key.clone(), "unknown config field".into());
            }
        }
    }
    if errors.is_empty() { Ok(()) } else { Err(errors) }
}
```

### Pattern 7: AppState Cache for Flags
**What:** Add `feature_flags: RwLock<HashMap<String, FeatureFlagRow>>` to `AppState` for in-memory read access without DB round-trip on every FlagSync. Populated at startup from DB and updated on every mutation.
**When to use:** Reading current flag state for FlagSync payloads.

```rust
// Source: crates/racecontrol/src/state.rs — AppState struct pattern
// Add to AppState:
pub feature_flags: RwLock<HashMap<String, FeatureFlagRow>>,
pub config_push_seq: std::sync::atomic::AtomicU64,  // monotonic seq counter
```

### Pattern 8: TypeScript Type Export
**What:** Define interfaces in `packages/shared-types/src/config.ts`, then re-export from `index.ts`. Matches exact pattern of `fleet.ts`.

```typescript
// Source: packages/shared-types/src/fleet.ts — pattern
/** packages/shared-types/src/config.ts */
export interface FeatureFlag {
  name: string;
  enabled: boolean;
  default_value: boolean;
  overrides: Record<string, boolean>;
  version: number;
  updated_at: string;
}

export interface ConfigPush {
  id: number;
  pod_id: string;
  payload: Record<string, unknown>;
  seq_num: number;
  status: 'pending' | 'delivered' | 'acked';
  created_at: string;
  acked_at?: string;
}

export interface ConfigAuditEntry {
  id: number;
  action: string;
  entity_type: string;
  entity_name: string;
  old_value?: string;
  new_value?: string;
  pushed_by: string;
  pods_acked: string[];
  created_at: string;
}

// packages/shared-types/src/index.ts — add to existing exports:
export type { FeatureFlag, ConfigPush, ConfigAuditEntry } from './config';
```

### Anti-Patterns to Avoid
- **Routing config push through `/api/v1/fleet/exec`:** Standing rule — always use typed WebSocket `ConfigPush` message. Any exec-based config delivery must be rejected at design time.
- **Skipping the `agent_senders` map:** Never send directly to pods via HTTP or SSH from handler code. All pod messages go through the mpsc channel in `agent_senders`.
- **Blocking billing sessions during config pushes:** Config push must check for active billing state and preserve session data. Do not clear or reset billing state on config delivery. The `billing.active_timers` map is the check source.
- **Unguarded `ALTER TABLE`:** SQLite has no `ADD COLUMN IF NOT EXISTS` in older versions. Use `PRAGMA table_info()` or accept the error from duplicate column add — but `CREATE TABLE IF NOT EXISTS` is always safe for new tables.
- **No audit entry for mutations:** Every flag create/update and every config push must write to `config_audit_log` before returning 200. Partial writes (mutation succeeded, audit failed) must log a warning — never silently drop audit entries.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| JWT claims extraction | Manual header parse | `Extension<StaffClaims>` from existing `require_staff_jwt` | Middleware already injects claims into extensions |
| Per-pod message fan-out | Custom notification system | `agent_senders: RwLock<HashMap<String, mpsc::Sender<CoreToAgentMessage>>>` | Already in AppState; used by `broadcast_settings()` |
| Config queue sequence numbers | UUID or timestamp-based IDs | `AUTOINCREMENT` INTEGER PK + monotonic `AtomicU64` in AppState | Integers are cheap, sortable, and directly comparable for replay |
| Reconnect delta sync | Full-state push on every connect | `FlagCacheSyncPayload.cached_version` + server-side compare | Protocol type already exists in rc-common |
| TypeScript type validation | Zod schemas or runtime validators | Vitest `assert*()` type guard pattern (see `pods.contract.test.ts`) | Matches existing contract test infrastructure |
| Config schema registry | External JSON Schema or protobuf | `validate_config_push()` whitelist fn in Rust | 8-pod LAN fleet with ~5 known config fields; schema registry adds complexity for no benefit |

**Key insight:** All the heavy infrastructure (WS channels, auth middleware, DB pool, mpsc fan-out) already exists. Phase 177 is almost entirely "wire existing pieces together" — the only genuinely new logic is `validate_config_push()` and the three DB tables.

---

## Common Pitfalls

### Pitfall 1: Missing `require_non_pod_source` Middleware
**What goes wrong:** Flag and config endpoints added to a router without the pod-source guard — pods can mutate flags directly.
**Why it happens:** Developer adds routes to the wrong router tier (e.g., `kiosk_routes` instead of `staff_routes`).
**How to avoid:** Always add flag/config mutation endpoints inside `staff_routes()` — that function applies both `require_staff_jwt` AND `require_non_pod_source` at the bottom via `.layer()`.
**Warning signs:** `cargo test` passes but a pod IP can call `PUT /flags/:name`.

### Pitfall 2: Forgetting AppState Cache Population on Startup
**What goes wrong:** Server starts, `feature_flags` HashMap in AppState is empty, first `GET /flags` returns empty list even though rows exist in DB.
**Why it happens:** Cache is initialized to `HashMap::new()` but no startup query populates it from the DB.
**How to avoid:** In `main.rs` or `AppState::new()`, run `SELECT * FROM feature_flags` and populate the cache before starting the HTTP server. Use the same pattern as `billing.active_timers` pre-population.
**Warning signs:** GET /flags returns `[]` after server restart while DB has rows.

### Pitfall 3: seq_num Gap on Server Restart
**What goes wrong:** `config_push_seq` AtomicU64 resets to 0 on restart; new pushes get seq_nums that overlap with unacked entries still in the queue.
**Why it happens:** AtomicU64 in AppState is volatile (not persisted).
**How to avoid:** On startup, query `SELECT MAX(seq_num) FROM config_push_queue` and initialize the AtomicU64 from that value + 1.
**Warning signs:** A pod reconnects and replays config entries it already acked (duplicate config application).

### Pitfall 4: Billing Session Disruption on Config Push
**What goes wrong:** Config push delivers a `billing_rate` change while a session is active; the agent hot-reloads the rate mid-session, charging the customer at the wrong rate.
**Why it happens:** Config push has no session awareness.
**How to avoid:** For billing-sensitive config fields (`billing_rate`), the server should check `billing.active_timers` for each target pod before delivery. Queue the push but only send when the pod has no active session. Document this in the handler comment.
**Warning signs:** Customer charged at new rate for time already allocated at old rate.

### Pitfall 5: JSON Column Deserialization Errors
**What goes wrong:** `overrides` column stores `{}` as TEXT; reading it back with `serde_json::from_str` panics or returns error for unexpected NULL.
**Why it happens:** SQLite stores NULL for rows inserted without setting the column default.
**How to avoid:** Define column default as `NOT NULL DEFAULT '{}'`. On read, use `serde_json::from_str(&override_str).unwrap_or_default()` with a `HashMap` default.
**Warning signs:** `GET /flags` returns 500 for any flag that has never had overrides set.

### Pitfall 6: Stale OpenAPI Spec
**What goes wrong:** New endpoints are live but not documented in `docs/openapi.yaml`; admin dashboard and contract tests reference stale spec.
**Why it happens:** OpenAPI update is treated as optional documentation.
**How to avoid:** Per cascade update standing rule, OpenAPI spec is a required deliverable for SYNC-01 — it must be updated in the same plan wave as the Rust endpoints.
**Warning signs:** Contract tests import types not present in the spec; admin dashboard autogenerated client calls wrong path.

---

## Code Examples

### Verified Pattern: sqlx fetch_all + serde_json column

```rust
// Source: crates/racecontrol/src/db/mod.rs + existing handler patterns
#[derive(sqlx::FromRow, serde::Serialize, serde::Deserialize)]
struct FeatureFlagRow {
    name: String,
    enabled: bool,
    default_value: bool,
    overrides: String,   // JSON text — parse separately
    version: i64,
    updated_at: Option<String>,
}

// Fetch all flags
let rows = sqlx::query_as::<_, FeatureFlagRow>(
    "SELECT name, enabled, default_value, overrides, version, updated_at FROM feature_flags"
)
.fetch_all(&state.db)
.await
.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;
```

### Verified Pattern: Axum 400 with field-level errors

```rust
// Source: pattern from existing handlers (e.g., billing.rs create_session validation)
if let Err(field_errors) = validate_config_push(&body.fields) {
    return Err((
        StatusCode::BAD_REQUEST,
        Json(json!({ "errors": field_errors })),
    ));
}
```

### Verified Pattern: Insert audit log entry

```rust
// Source: state.rs + db/mod.rs pattern
sqlx::query(
    "INSERT INTO config_audit_log (action, entity_type, entity_name, old_value, new_value, pushed_by, pods_acked)
     VALUES (?, ?, ?, ?, ?, ?, ?)"
)
.bind("update")
.bind("feature_flag")
.bind(&flag_name)
.bind(old_value_json)
.bind(new_value_json)
.bind(&claims.sub)
.bind("[]")
.execute(&state.db)
.await?;
```

### Verified Pattern: Contract test fixture structure

```typescript
// Source: packages/contract-tests/src/pods.contract.test.ts — exact pattern to follow
import { describe, test, expect } from 'vitest';
import type { FeatureFlag } from '@racingpoint/types';
import flagsFixture from './fixtures/flags.json';

function assertFeatureFlag(data: unknown): asserts data is FeatureFlag {
  const d = data as Record<string, unknown>;
  expect(typeof d.name, 'name must be string').toBe('string');
  expect(typeof d.enabled, 'enabled must be boolean').toBe('boolean');
  expect(typeof d.version, 'version must be number').toBe('number');
  expect(d.overrides !== null && typeof d.overrides === 'object', 'overrides must be object').toBe(true);
}

describe('GET /api/v1/flags — FeatureFlag contract', () => {
  test('fixture is a non-empty array', () => {
    expect(Array.isArray(flagsFixture)).toBe(true);
  });
  test('each flag matches FeatureFlag contract', () => {
    flagsFixture.forEach((flag, i) => {
      try { assertFeatureFlag(flag); }
      catch (e) { throw new Error(`Flag at index ${i} failed: ${String(e)}`); }
    });
  });
});
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Config via `kiosk_settings` key-value TEXT | Typed `feature_flags` table + JSON overrides column | Phase 177 | Named flags with per-pod granularity, versioned, auditable |
| Config changes via fleet exec endpoint | Typed `ConfigPush` WS message | Phase 177 decision | No exec channel dependency, queued for offline pods, acked |
| No mutation audit trail | `config_audit_log` append-only table | Phase 177 | `pushed_by`, `old_value`, `new_value`, `pods_acked` per event |
| Full flag state on every reconnect | `FlagCacheSyncPayload.cached_version` delta sync | Phase 176 protocol | Reduces reconnect traffic; only sends full state when stale |

**Deprecated/outdated:**
- Using `kiosk_settings` table for feature flags: Phase 176/177 introduces dedicated `feature_flags` table — do not add new flag-like entries to `kiosk_settings`.
- Config push via `/api/v1/fleet/exec` or `ws_exec_pod`: Prohibited by standing rule. Use `CoreToAgentMessage::ConfigPush` only.

---

## Open Questions

1. **billing_rate config push timing**
   - What we know: Config push must not disrupt active billing sessions (CONTEXT.md specifics)
   - What's unclear: Should the handler queue the push immediately (mark as `pending`) and only send to currently-active pods after session ends? Or should it send immediately and the agent holds the new rate until next session?
   - Recommendation: Queue all pushes immediately (CP-02 compliance). For `billing_rate` specifically, the server checks `billing.active_timers` and withholds delivery via WS until session ends. The queued entry has `status='pending'` until then.

2. **`pods_acked` update mechanism for audit log**
   - What we know: `config_audit_log.pods_acked` should record which pods acknowledged each config push
   - What's unclear: This requires correlating `ConfigAck.sequence` back to the audit log entry after the fact — WS handler must update the audit entry when ack arrives
   - Recommendation: WS handler on `AgentMessage::ConfigAck` looks up `config_push_queue` by seq_num, then updates `config_audit_log.pods_acked` JSON array. Keep this in a helper in `config_push.rs`.

3. **Per-flag vs global version for FlagSync**
   - What we know: Each flag has its own `version` field; `FlagCacheSyncPayload.cached_version` is a single u64
   - What's unclear: Is the reconnect sync based on a global max version across all flags, or per-flag?
   - Recommendation: Use a global monotonic counter (`config_push_seq` AtomicU64 or a `flag_global_version` AtomicU64) that increments on any flag mutation. `FlagCacheSync.cached_version` compares against this global counter. If stale, send full flag state. Simpler than per-flag tracking for 8-pod fleet.

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Vitest (packages/contract-tests/package.json) |
| Config file | packages/contract-tests/vitest.config.ts |
| Quick run command | `cd packages/contract-tests && npm test` |
| Full suite command | `cargo test -p rc-common && cargo test -p racecontrol && cd packages/contract-tests && npm test` |
| Rust test command | `cargo test -p racecontrol -- flags config_push` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| FF-01 | GET /flags returns flag array matching FeatureFlag type | contract | `cd packages/contract-tests && npm test -- flags` | ❌ Wave 0 |
| FF-02 | PUT /flags/:name with overrides field returns updated flag | contract | `cd packages/contract-tests && npm test -- flags` | ❌ Wave 0 |
| FF-03 | FlagSync WS message serializes/deserializes correctly | unit | `cargo test -p rc-common -- flag_sync` | ✅ (Phase 176 types.rs tests) |
| CP-01 | ConfigPush WS message used (not exec) | unit | `cargo test -p rc-common -- config_push` | ✅ (Phase 176 types.rs tests) |
| CP-02 | config_push_queue persists entries, replays on reconnect | unit | `cargo test -p racecontrol -- config_push_queue` | ❌ Wave 0 |
| CP-04 | ConfigPushPayload.schema_version preserved | unit | `cargo test -p rc-common -- config_push_payload` | ✅ (Phase 176) |
| CP-05 | config_audit_log entry written on every mutation | unit | `cargo test -p racecontrol -- audit_log` | ❌ Wave 0 |
| CP-06 | validate_config_push() rejects negative billing_rate | unit | `cargo test -p racecontrol -- validate_config_push` | ❌ Wave 0 |
| SYNC-01 | FeatureFlag TypeScript type matches API response | contract | `cd packages/contract-tests && npm test -- flags` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p racecontrol -- flags config_push`
- **Per wave merge:** `cargo test -p rc-common && cargo test -p racecontrol && cd packages/contract-tests && npm test`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `packages/contract-tests/src/flags.contract.test.ts` — covers FF-01, FF-02, SYNC-01
- [ ] `packages/contract-tests/src/fixtures/flags.json` — sample FeatureFlag array fixture
- [ ] `packages/contract-tests/src/config.contract.test.ts` — covers CP-06 (schema validation response shape)
- [ ] `packages/contract-tests/src/fixtures/config-push.json` — sample ConfigPush fixture
- [ ] `packages/shared-types/src/config.ts` — FeatureFlag, ConfigPush, ConfigAuditEntry types
- [ ] Rust unit tests for `validate_config_push()` — CP-06 coverage (inline `#[cfg(test)]` in flags.rs or config_push.rs)
- [ ] Rust unit tests for audit log insertion — CP-05 coverage

---

## Sources

### Primary (HIGH confidence)
- Direct code inspection: `crates/racecontrol/src/db/mod.rs` — migrate() table pattern, kiosk_settings analog
- Direct code inspection: `crates/racecontrol/src/api/routes.rs` — staff_routes() pattern, middleware layers
- Direct code inspection: `crates/racecontrol/src/state.rs` — AppState fields, broadcast_settings(), agent_senders
- Direct code inspection: `crates/rc-common/src/protocol.rs` — FlagSync, ConfigPush, ConfigAck, FlagCacheSync variants
- Direct code inspection: `crates/rc-common/src/types.rs` lines 885-948 — FlagSyncPayload, ConfigPushPayload, ConfigAckPayload, FlagCacheSyncPayload
- Direct code inspection: `crates/racecontrol/src/auth/middleware.rs` — StaffClaims.sub pattern
- Direct code inspection: `packages/shared-types/src/fleet.ts` — TypeScript interface export pattern
- Direct code inspection: `packages/contract-tests/src/pods.contract.test.ts` — assert* contract test pattern

### Secondary (MEDIUM confidence)
- `docs/openapi.yaml` header — confirmed OpenAPI 3.0.3, existing tags structure, staffJWT securityScheme
- `.planning/REQUIREMENTS.md` — FF-01 through CP-06, SYNC-01 scope
- `.planning/phases/177-server-side-registry-config-foundation/177-CONTEXT.md` — locked decisions, code_context, integration points

### Tertiary (LOW confidence)
- None — all claims verified from direct codebase inspection.

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all libraries confirmed present in existing Cargo.toml via usage in codebase
- Architecture: HIGH — all patterns verified from direct code inspection of existing modules
- Pitfalls: HIGH — derived from observed code patterns and standing rules in CLAUDE.md
- Validation: HIGH — contract test pattern verified from existing test files

**Research date:** 2026-03-24 IST
**Valid until:** 2026-04-24 (stable Rust/Axum/sqlx patterns; flag/config design is locked)
