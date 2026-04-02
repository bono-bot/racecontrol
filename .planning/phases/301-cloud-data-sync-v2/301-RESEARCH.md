# Phase 301: Cloud Data Sync v2 - Research

**Researched:** 2026-04-01
**Domain:** Rust cloud_sync extension + SQLite upsert conflict resolution + Next.js status panel
**Confidence:** HIGH

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Extend existing `cloud_sync.rs` (additive, not rewrite)
- Cloud sync uses existing HTTP-based sync mechanism to Bono VPS racecontrol at :8080
- Server-authoritative for: fleet_solutions, model_evaluations, metrics_rollups
- Cloud-authoritative for: cross-venue solutions (future venue_id rows)
- Conflict resolution: last-write-wins by updated_at, venue_id tiebreaker on equal timestamps
- Admin panel in `web/` Next.js app (the admin dashboard — `racingpoint-admin` is an alias)

### Claude's Discretion
All other implementation choices — pure infrastructure phase. Use ROADMAP phase goal, success criteria, and codebase conventions.

### Deferred Ideas (OUT OF SCOPE)
None — discuss phase was skipped.
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| SYNC-01 | fleet_solutions rows written at venue appear in Bono VPS within next sync cycle | Extend `collect_push_payload()` + `sync_push` handler with fleet_solutions table queries |
| SYNC-02 | model_evaluations rows written at venue appear in Bono VPS within next sync cycle | model_evaluations table does NOT exist yet — needs CREATE TABLE migration first, then push/receive wiring |
| SYNC-03 | metrics_rollups rows written at venue appear in Bono VPS within next sync cycle | metrics_rollups table exists (Phase 285). Extend push/receive with INTEGER PRIMARY KEY auto-dedup strategy |
| SYNC-04 | Cross-venue: rows with future venue_id on Bono VPS flow back to venue on next sync | Extend `sync_changes` handler + `sync_once_http` pull path to include these three tables |
| SYNC-05 | Conflict resolution: later updated_at wins; equal timestamps broken by lexicographically smaller venue_id | All three upsert handlers need the same LWW+venue_id SQL pattern |
| SYNC-06 | Admin dashboard sync panel: last sync time, tables synced, conflict count | New `/web/src/app/settings/` sync section OR new `/sync` page; backend needs conflict counter in sync_state |
</phase_requirements>

---

## Summary

Phase 301 extends the existing bidirectional cloud sync to cover three intelligence tables: `fleet_solutions`, `model_evaluations`, and `metrics_rollups`. The existing `cloud_sync.rs` already handles ~12 tables via two paths — relay push (2s interval, comms-link) and HTTP fallback (30s, direct). The extension follows the identical pattern already proven in the codebase.

The key complication is that `model_evaluations` does not exist in the database yet. It was planned for v35.0 (Phase 290) which has not shipped. This phase must create the table as part of its migration, then wire it into the sync pipeline. The table schema can be derived from the v35.0 roadmap spec: `id, model_name, pod_id, problem_key, prediction, actual, correct, cost, created_at, updated_at, venue_id`.

Conflict resolution for all three tables uses SQLite's `ON CONFLICT DO UPDATE SET ... WHERE excluded.updated_at > table.updated_at`. The venue_id tiebreaker requires an additional CASE expression for the equal-timestamp edge case.

The admin dashboard sync panel extends the existing `/sync/health` endpoint (which already returns `sync_state` rows) by adding a `conflict_count` field. The frontend panel can be added to the existing `web/src/app/settings/page.tsx` or as a new `/sync` page in the web admin.

**Primary recommendation:** Follow the exact `collect_push_payload` + `sync_push` + `sync_changes` pattern already established for the 12 existing tables. Two plans: (1) DB migration + Rust sync extension, (2) admin dashboard panel.

---

## Standard Stack

### Core (all pre-existing in Cargo.toml — no new dependencies)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| sqlx | 0.8.x | SQLite queries + upsert | Project standard — all DB access |
| serde_json | 1.x | JSON payload building | Existing collect_push_payload pattern |
| tokio | 1.x | Async runtime | Existing spawn() background task |
| chrono | 0.4.x | Timestamp normalization | Existing `normalize_timestamp()` helper |
| axum | 0.7.x | HTTP handlers | Existing sync_push/sync_changes handlers |

### No New Dependencies
Zero new Cargo dependencies are needed. All required functionality exists.

**Installation:** No installation step needed.

---

## Architecture Patterns

### Existing Sync Architecture (HIGH confidence — read from source)

```
cloud_sync.rs spawn()
  ├── Relay path (2s): push_via_relay() → collect_push_payload() → POST /relay/sync
  └── HTTP fallback (30s): sync_once_http()
        ├── Pull: GET /sync/changes → upsert each table
        └── Push: push_to_cloud() → collect_push_payload() → POST /sync/push

routes.rs
  ├── GET /sync/changes → sync_changes() — returns JSON per table
  ├── POST /sync/push → sync_push() — receives and upserts
  └── GET /sync/health → sync_health() — returns sync_state rows
```

### Pattern: collect_push_payload() Extension

The push payload is assembled by `collect_push_payload()` in `cloud_sync.rs`. Each table follows this exact pattern:

```rust
// Source: cloud_sync.rs lines 554-574 (laps example)
let rows = sqlx::query_as::<_, (String,)>(
    "SELECT json_object(
        'id', id, 'col1', col1, ..., 'updated_at', updated_at
    ) FROM table_name WHERE updated_at > ? ORDER BY updated_at ASC LIMIT 500",
)
.bind(&last_push)
.fetch_all(&state.db)
.await?;

if !rows.is_empty() {
    let items: Vec<serde_json::Value> = rows.iter()
        .filter_map(|r| serde_json::from_str(&r.0).ok())
        .collect();
    tracing::info!("Cloud sync push: {} table_name", items.len());
    payload["table_name"] = serde_json::json!(items);
    has_data = true;
}
```

### Pattern: Last-Write-Wins Upsert with venue_id Tiebreaker

The conflict resolution rule (later `updated_at` wins; on tie, lexicographically smaller `venue_id` wins) is NOT yet in the codebase — this is new for Phase 301. The SQL pattern to implement it:

```sql
-- For fleet_solutions (text primary key)
INSERT INTO fleet_solutions (id, problem_key, ..., updated_at, venue_id)
VALUES (?1, ?2, ..., ?N, ?M)
ON CONFLICT(id) DO UPDATE SET
    root_cause = excluded.root_cause,
    fix_action = excluded.fix_action,
    status = excluded.status,
    success_count = excluded.success_count,
    confidence = excluded.confidence,
    updated_at = excluded.updated_at,
    venue_id = excluded.venue_id
WHERE
    excluded.updated_at > fleet_solutions.updated_at
    OR (excluded.updated_at = fleet_solutions.updated_at
        AND excluded.venue_id < fleet_solutions.venue_id)
```

**The WHERE clause on ON CONFLICT DO UPDATE is standard SQLite.** SQLite supports partial updates via `ON CONFLICT DO UPDATE ... WHERE condition`. This is the correct, idiomatic approach.

### Pattern: sync_changes Handler Extension

For the pull direction (Bono VPS → venue), the `sync_changes` handler in `routes.rs` already has a `match *table` dispatch. Add three new arms:

```rust
"fleet_solutions" => {
    let rows = sqlx::query_as::<_, (String,)>(
        "SELECT json_object(...) FROM fleet_solutions
         WHERE updated_at > ? ORDER BY updated_at ASC LIMIT ?",
    )
    .bind(&since)
    .bind(limit)
    .fetch_all(&state.db)
    .await;
    ...
}
```

### Pattern: sync_push Handler Extension

The inbound push handler already dispatches by JSON key. Add blocks for the three new tables after the existing dispatch.

### Pattern: Conflict Counter

The `sync_state` table has: `table_name TEXT PRIMARY KEY, last_synced_at TEXT, last_sync_count INTEGER, updated_at TEXT`. Add a `conflict_count INTEGER DEFAULT 0` column via ALTER TABLE migration, incremented when an incoming row's `updated_at` is older than the stored row (a write was skipped due to LWW).

The `/sync/health` endpoint already queries `sync_state` and returns it in the response. After the migration, `conflict_count` flows out automatically.

### Pattern: Admin Dashboard Sync Panel

The settings page (`web/src/app/settings/page.tsx`) already fetches `api.health()` and `api.backupStatus()`. Add a `api.syncHealth()` call to the same `useEffect`. The panel renders the data from `/api/v1/sync/health` (already registered in routes.rs at line 614).

Existing API client file: `web/src/lib/api.ts`. Add a new method following the existing pattern.

### Recommended Project Structure (changes only)

```
crates/racecontrol/src/
├── cloud_sync.rs          # Extend: collect_push_payload, sync_push receive path
├── db/mod.rs              # Add: model_evaluations CREATE TABLE + ALTER TABLE migrations
└── api/routes.rs          # Extend: sync_changes arms, sync_push blocks, sync_health response

web/src/
├── lib/api.ts             # Add: syncHealth() method type
└── app/settings/page.tsx  # Add: SyncStatusPanel component
```

### Anti-Patterns to Avoid
- **Don't add `venue_id` to the `sync_state` table tracking rows** — `sync_state` uses `table_name` as its key and tracks the last push/pull watermark per table, not per venue.
- **Don't use `REPLACE INTO`** — it deletes the row and re-inserts, losing any columns not in the payload. Use `INSERT ... ON CONFLICT DO UPDATE SET`.
- **Don't use `ON CONFLICT DO NOTHING`** for mutable tables — that pattern (used for laps) is correct for immutable append-only data. Solutions and evaluations are mutable (status, confidence change).
- **Don't hold a lock across `.await` during push payload collection** — the existing code already shows the correct pattern: snapshot the payload synchronously then drop before async ops.
- **Don't forget `normalize_timestamp()`** — incoming ISO timestamps with 'T' separator fail SQLite string comparison. The existing `normalize_timestamp()` helper must be applied to any `updated_at` values used in WHERE clauses.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| HTTP push to Bono VPS | Custom HTTP client | Existing `push_to_cloud()` in cloud_sync.rs | Already has HMAC signing, backoff, circuit breaker |
| Conflict detection | Custom comparison logic | `ON CONFLICT DO UPDATE WHERE` SQLite clause | Atomic, race-free, no Rust-level compare needed |
| Sync watermark tracking | Custom timestamp store | Existing `sync_state` table + `update_push_state()` | Already handles `_push` sentinel row |
| Admin status fetching | New API endpoint | Existing `GET /sync/health` in routes.rs | Already returns sync_state rows |
| JSON serialization | Manual string building | `json_object()` in SQLite query | Same pattern for all 12 existing tables |

**Key insight:** Every infrastructure piece already exists. This phase is purely additive — new table migrations + 3 new switch arms + 3 new push blocks + 1 admin panel.

---

## Table Schema Research

### fleet_solutions (EXISTS — fleet_kb.rs line 15)
```
id TEXT PRIMARY KEY
problem_key TEXT NOT NULL
problem_hash TEXT NOT NULL
symptoms TEXT NOT NULL
environment TEXT NOT NULL
root_cause TEXT NOT NULL
fix_action TEXT NOT NULL
fix_type TEXT NOT NULL
status TEXT NOT NULL DEFAULT 'candidate'
success_count INTEGER DEFAULT 1
fail_count INTEGER DEFAULT 0
confidence REAL DEFAULT 1.0
cost_to_diagnose REAL DEFAULT 0
models_used TEXT
diagnosis_tier TEXT NOT NULL DEFAULT 'deterministic'
source_node TEXT NOT NULL
venue_id TEXT
created_at TEXT NOT NULL
updated_at TEXT NOT NULL
version INTEGER DEFAULT 1
ttl_days INTEGER DEFAULT 90
tags TEXT
```
Has `venue_id` and `updated_at` already. No migration needed for the table itself. Upsert key: `id`.

### model_evaluations (DOES NOT EXIST — planned for v35.0/Phase 290)

Must be created in this phase. Schema derived from v35.0 roadmap description ("every AI diagnosis writes: prediction, actual, correct, cost; weekly rollup: accuracy, cost-per-correct"):

```
id TEXT PRIMARY KEY
model_name TEXT NOT NULL
pod_id TEXT
problem_key TEXT
prediction TEXT
actual TEXT
correct INTEGER NOT NULL DEFAULT 0
cost_usd REAL DEFAULT 0
diagnosis_tier TEXT
created_at TEXT NOT NULL DEFAULT (datetime('now'))
updated_at TEXT NOT NULL DEFAULT (datetime('now'))
venue_id TEXT
```

This is a CREATE TABLE IF NOT EXISTS migration — safe to add here. The v35.0 plan will add additional columns (rollup fields) via ALTER TABLE when it ships.

### metrics_rollups (EXISTS — db/mod.rs line 3655)
```
id INTEGER PRIMARY KEY AUTOINCREMENT
resolution TEXT NOT NULL CHECK(resolution IN ('hourly', 'daily'))
metric_name TEXT NOT NULL
pod_id TEXT
min_value REAL NOT NULL
max_value REAL NOT NULL
avg_value REAL NOT NULL
sample_count INTEGER NOT NULL
period_start TEXT NOT NULL
UNIQUE(resolution, metric_name, pod_id, period_start)
```

No `updated_at` column — conflict resolution must be based on the `UNIQUE` constraint. Since rollup values for a given (resolution, metric_name, pod_id, period_start) are computed, the correct strategy is `ON CONFLICT DO UPDATE SET avg_value = ...` using a "max sample_count wins" heuristic (more samples = more data = more authoritative). No `venue_id` column either — must add via migration.

**Migration for metrics_rollups:**
```sql
ALTER TABLE metrics_rollups ADD COLUMN updated_at TEXT DEFAULT (datetime('now'));
ALTER TABLE metrics_rollups ADD COLUMN venue_id TEXT;
```

**Upsert key:** The `UNIQUE(resolution, metric_name, pod_id, period_start)` constraint is the natural conflict key. Use `ON CONFLICT(resolution, metric_name, pod_id, period_start) DO UPDATE`.

---

## Common Pitfalls

### Pitfall 1: INTEGER AUTOINCREMENT Key for metrics_rollups
**What goes wrong:** `metrics_rollups.id` is `INTEGER PRIMARY KEY AUTOINCREMENT`. On Bono VPS, the same rollup will have a different integer id than on the venue server. Upserting by `id` would fail or create duplicates.
**Why it happens:** The table was designed for local-only use. AUTOINCREMENT IDs are per-database.
**How to avoid:** Use `ON CONFLICT(resolution, metric_name, pod_id, period_start) DO UPDATE` — conflict on the UNIQUE constraint, NOT on `id`. Don't include `id` in the INSERT column list when syncing (let the target DB assign its own AUTOINCREMENT id).
**Warning signs:** Duplicate `(resolution, metric_name, pod_id, period_start)` rows on the VPS after a push.

### Pitfall 2: Missing `updated_at` column on metrics_rollups
**What goes wrong:** LWW conflict resolution requires `updated_at`. The table doesn't have this column yet.
**Why it happens:** The table was designed before the cross-venue sync requirement existed.
**How to avoid:** Include the `ALTER TABLE metrics_rollups ADD COLUMN updated_at` migration at the top of the phase migration. ALWAYS check the column exists before writing the upsert SQL.
**Warning signs:** Compile error or runtime panic when binding `updated_at` in the query.

### Pitfall 3: normalize_timestamp() Omission
**What goes wrong:** ISO 8601 timestamps with 'T' separator fail SQLite string comparison. A row updated at "2026-04-01T12:00:00Z" appears to be OLDER than a row at "2026-04-01 00:00:00" because 'T' (0x54) > ' ' (0x20).
**Why it happens:** Rust's `chrono::Utc::now().to_rfc3339()` produces ISO format; SQLite `datetime('now')` produces space format.
**How to avoid:** Call `normalize_timestamp()` on any `updated_at` value before binding it to a SQLite query. The existing helper is at `cloud_sync.rs:101`.
**Warning signs:** Rows synced from venue never overwrite rows on VPS even when clearly newer.

### Pitfall 4: Schema Version Not Bumped
**What goes wrong:** `SCHEMA_VERSION` constant in `cloud_sync.rs` is currently `3`. Adding new tables changes the sync payload shape. The VPS side reads schema_version to detect incompatible pushes.
**Why it happens:** Easy to forget.
**How to avoid:** Bump `SCHEMA_VERSION` to `4` in the same commit that adds the new table push blocks.
**Warning signs:** Cloud accepts pushes silently but ignores the new table keys (VPS running old code).

### Pitfall 5: fleet_solutions Columns That Need Escaping
**What goes wrong:** `fleet_solutions.symptoms`, `environment`, `root_cause`, `fix_action` can contain arbitrary strings with quotes, newlines, JSON. SQLite `json_object()` handles this correctly, but hand-rolled string interpolation does not.
**Why it happens:** Attempting to optimize by not using the `json_object()` pattern.
**How to avoid:** Always use the `json_object()` in the SELECT query. Never build JSON strings in Rust from raw SQL data.

### Pitfall 6: model_evaluations Missing from sync_changes Dispatch
**What goes wrong:** Bono VPS pushes `model_evaluations` in the payload but the venue's `sync_changes` GET handler doesn't have a match arm for it — returns empty array.
**Why it happens:** Adding to push without adding to pull.
**How to avoid:** For each new table: add to `collect_push_payload`, `sync_push` receive block, AND `sync_changes` dispatch arm. All three must be updated atomically (same commit).

---

## Code Examples

### 1. fleet_solutions Push (collect_push_payload extension)
```rust
// Pattern from cloud_sync.rs — extend collect_push_payload()
let solutions = sqlx::query_as::<_, (String,)>(
    "SELECT json_object(
        'id', id, 'problem_key', problem_key, 'problem_hash', problem_hash,
        'symptoms', symptoms, 'environment', environment, 'root_cause', root_cause,
        'fix_action', fix_action, 'fix_type', fix_type, 'status', status,
        'success_count', success_count, 'fail_count', fail_count,
        'confidence', confidence, 'cost_to_diagnose', cost_to_diagnose,
        'models_used', models_used, 'diagnosis_tier', diagnosis_tier,
        'source_node', source_node, 'venue_id', venue_id,
        'created_at', created_at, 'updated_at', updated_at,
        'version', version, 'ttl_days', ttl_days, 'tags', tags
    ) FROM fleet_solutions WHERE updated_at > ? ORDER BY updated_at ASC LIMIT 500",
)
.bind(&last_push)
.fetch_all(&state.db)
.await?;

if !solutions.is_empty() {
    let items: Vec<serde_json::Value> = solutions.iter()
        .filter_map(|r| serde_json::from_str(&r.0).ok())
        .collect();
    tracing::info!("Cloud sync push: {} fleet_solutions", items.len());
    payload["fleet_solutions"] = serde_json::json!(items);
    has_data = true;
}
```

### 2. LWW Upsert with venue_id Tiebreaker (sync_push receive)
```rust
// In sync_push handler, new block for fleet_solutions
if let Some(solutions) = body.get("fleet_solutions").and_then(|v| v.as_array()) {
    for sol in solutions {
        let id = sol.get("id").and_then(|v| v.as_str()).unwrap_or_default();
        if id.is_empty() { continue; }
        let incoming_ts = sol.get("updated_at").and_then(|v| v.as_str()).unwrap_or("");
        let incoming_ts_norm = normalize_timestamp(incoming_ts);
        let r = sqlx::query(
            "INSERT INTO fleet_solutions
                (id, problem_key, problem_hash, symptoms, environment, root_cause,
                 fix_action, fix_type, status, success_count, fail_count, confidence,
                 cost_to_diagnose, models_used, diagnosis_tier, source_node, venue_id,
                 created_at, updated_at, version, ttl_days, tags)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22)
             ON CONFLICT(id) DO UPDATE SET
                status = excluded.status,
                success_count = excluded.success_count,
                fail_count = excluded.fail_count,
                confidence = excluded.confidence,
                root_cause = excluded.root_cause,
                fix_action = excluded.fix_action,
                models_used = excluded.models_used,
                updated_at = excluded.updated_at,
                version = excluded.version,
                venue_id = excluded.venue_id
             WHERE excluded.updated_at > fleet_solutions.updated_at
                OR (excluded.updated_at = fleet_solutions.updated_at
                    AND excluded.venue_id < fleet_solutions.venue_id)",
        )
        // ...bind all columns...
        .execute(&state.db)
        .await;
        if r.is_ok() { total += 1; }
    }
}
```

### 3. metrics_rollups Upsert (UNIQUE conflict key, not id)
```rust
// Key difference: ON CONFLICT uses the UNIQUE columns, not (id)
"INSERT INTO metrics_rollups
    (resolution, metric_name, pod_id, min_value, max_value, avg_value,
     sample_count, period_start, updated_at, venue_id)
 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)
 ON CONFLICT(resolution, metric_name, pod_id, period_start) DO UPDATE SET
    avg_value = CASE WHEN excluded.sample_count > metrics_rollups.sample_count
                THEN excluded.avg_value ELSE metrics_rollups.avg_value END,
    min_value = MIN(excluded.min_value, metrics_rollups.min_value),
    max_value = MAX(excluded.max_value, metrics_rollups.max_value),
    sample_count = MAX(excluded.sample_count, metrics_rollups.sample_count),
    updated_at = excluded.updated_at"
// Note: id is NOT in the INSERT list — target DB assigns its own AUTOINCREMENT id
```

### 4. Conflict Tracking in sync_state
```sql
-- Migration: add conflict_count column to sync_state
ALTER TABLE sync_state ADD COLUMN conflict_count INTEGER DEFAULT 0;
```

```rust
// In sync_push upsert, detect a skipped write and count it
let rows_affected = r.map(|r| r.rows_affected()).unwrap_or(0);
if rows_affected == 0 {
    // Row existed and was NOT updated (LWW said local was newer)
    sqlx::query(
        "UPDATE sync_state SET conflict_count = conflict_count + 1
         WHERE table_name = 'fleet_solutions'",
    )
    .execute(&state.db)
    .await
    .ok();
}
```

### 5. Admin Panel (web/src/app/settings/page.tsx extension)
```typescript
// Add to api.ts
syncHealth: async (): Promise<SyncHealth> => {
  return fetchApi<SyncHealth>("/api/v1/sync/health");
},

// Type definition
interface SyncTableState {
  table: string;
  last_synced_at: string;
  last_sync_count: number;
  staleness_seconds: number;
  conflict_count?: number;
}
interface SyncHealth {
  status: string;
  lag_seconds: number;
  sync_mode: string;
  relay_available: boolean;
  sync_state: SyncTableState[];
}
```

---

## State of the Art

| Old Approach | Current Approach | Applies Here |
|--------------|-----------------|--------------|
| `ON CONFLICT DO NOTHING` | `ON CONFLICT DO UPDATE WHERE` | LWW upsert for mutable tables |
| Fixed SYNC_TABLES constant only | SYNC_TABLES + per-table push blocks | New tables need both constant update AND push block |
| No conflict tracking | sync_state.conflict_count | New column to count skipped overwrites |

**Deprecated/outdated:**
- Using `INSERT OR REPLACE` — deletes row first, loses columns not in payload, breaks `AUTOINCREMENT` sequences. Use `INSERT ... ON CONFLICT DO UPDATE`.

---

## Open Questions

1. **Are model_evaluations rows written anywhere today?**
   - What we know: The table does not exist. fleet_kb.rs writes fleet_solutions but no evaluations.
   - What's unclear: Phase 301 syncing an empty table is a no-op but valid. The question is whether to CREATE TABLE with minimal schema now and let v35.0 add columns via ALTER, or wait.
   - Recommendation: Create the table now with a minimal schema that covers the sync columns. Mark clearly in migration comment that v35.0 will add columns. This avoids a migration dependency on v35.0 before sync can work.

2. **Does Bono VPS need a matching migration?**
   - What we know: Bono VPS runs `racecontrol` binary. When the new binary (with migrations) deploys to VPS, `db::init()` will run and create the new tables/columns. This is the existing pattern.
   - What's unclear: Whether the Bono VPS deployment is a separate task or whether auto-deploy handles it.
   - Recommendation: Plan should include a "deploy to Bono VPS" step explicitly, as the receiving side must have the table before a push arrives.

3. **Should the admin panel be on /settings or a new /sync page?**
   - What we know: settings/page.tsx already shows Server Status + Backup Status panels. A sync panel fits the pattern.
   - Recommendation: Add to settings page as a third panel. A dedicated /sync page is overkill for this data volume.

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Bono VPS racecontrol | Receiving sync pushes | Assumed running | Build `0267ce15` | Redeploy first |
| comms-link relay | Relay sync path (2s) | ✓ (running on James .27:8766) | v18.0 | HTTP fallback (30s) |
| SQLite WAL | migrations | ✓ (built-in) | — | — |
| Node v22.22.0 | Next.js admin build | ✓ | v22.22.0 | — |
| cargo 1.93.1 | Rust build | ✓ | 1.93.1 | — |

**Bono VPS deployment is a required step.** The VPS must run the new binary with migrations before inbound sync pushes will succeed. Include as a plan step.

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (Rust) + manual integration check |
| Config file | Cargo.toml workspace |
| Quick run command | `cargo test -p racecontrol -- cloud_sync` |
| Full suite command | `cargo test -p racecontrol && cargo test -p rc-agent && cargo test -p rc-common` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| SYNC-01 | fleet_solutions rows appear on VPS after push | Integration (manual) | Push 1 row, query VPS | No — Wave 0 |
| SYNC-02 | model_evaluations rows appear on VPS after push | Integration (manual) | Push 1 row, query VPS | No — Wave 0 |
| SYNC-03 | metrics_rollups rows appear on VPS after push | Integration (manual) | Push 1 row, query VPS | No — Wave 0 |
| SYNC-04 | Cross-venue rows flow from VPS to venue | Integration (manual) | Insert row on VPS, wait cycle, query venue | No — Wave 0 |
| SYNC-05 | LWW conflict resolution | Unit test | `cargo test -p racecontrol -- sync_conflict` | No — Wave 0 |
| SYNC-06 | Admin panel shows sync data | Visual / manual | Open settings page, verify panel renders | No — Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p racecontrol 2>&1 | tail -20` (compilation + unit tests)
- **Per wave merge:** Full suite `cargo test -p racecontrol && cargo test -p rc-agent && cargo test -p rc-common`
- **Phase gate:** Full suite green + visual verification of admin panel + one live sync cycle observed

### Wave 0 Gaps
- [ ] Unit test for LWW conflict resolution SQL (SYNC-05) — can be tested with a mock SQLite in-process
- [ ] Integration test scaffold: push 1 fleet_solution to VPS, query back, assert it arrived

*(No existing test infrastructure targets sync directly — the conflict resolution WHERE clause logic is new and untested)*

---

## Project Constraints (from CLAUDE.md)

The following directives from `racecontrol/CLAUDE.md` constrain this phase:

- **No `.unwrap()` in production Rust** — use `?`, `.ok()`, or match
- **Idempotent SQL migrations** — `CREATE TABLE IF NOT EXISTS`, `ALTER TABLE ADD COLUMN IF NOT EXISTS` (SQLite doesn't support IF NOT EXISTS on ALTER — wrap in `ignore` pattern: `let _ = sqlx::query("ALTER TABLE...").execute(pool).await;`)
- **DB migrations must cover ALL consumers** — if `conflict_count` is added to `sync_state`, the existing handler reading sync_state must handle the new column
- **Cascade updates (RECURSIVE)** — adding new table to sync means: (1) `collect_push_payload`, (2) `sync_push` receive, (3) `sync_changes` dispatch, (4) `SYNC_TABLES` constant update, (5) `update_sync_state()` coverage. All five must be updated.
- **SCHEMA_VERSION must be bumped** — currently 3, must become 4 when new table keys are added to push payload
- **Auto-push + notify** — after committing, push + INBOX.md + comms-link WS message
- **LOGBOOK.md** — after every commit, append entry
- **Route uniqueness** — no new routes needed here; extending existing handlers only
- **Deploy to Bono VPS** — any server binary change must also deploy to Bono VPS (cloud racecontrol)

---

## Sources

### Primary (HIGH confidence — read directly from source)
- `crates/racecontrol/src/cloud_sync.rs` — full sync mechanism, push payload pattern, relay/HTTP paths, SCHEMA_VERSION, SYNC_TABLES, normalize_timestamp()
- `crates/racecontrol/src/api/routes.rs` — sync_push handler, sync_changes handler, sync_health handler
- `crates/racecontrol/src/fleet_kb.rs` — fleet_solutions schema, venue_id column confirmed
- `crates/racecontrol/src/db/mod.rs` — metrics_rollups schema (line 3655), sync_state schema (line 1271)
- `.planning/backlog/infrastructure-roadmap-v34-v37.md` — v35.0 model_evaluations spec (Phase 290)
- `.planning/phases/300-sqlite-backup-pipeline/` — Phase 300 shipped (SUMMARY files present)

### Secondary (MEDIUM confidence)
- `web/src/app/settings/page.tsx` — existing settings panel pattern for admin panel extension
- `web/src/lib/api.ts` — API client pattern for new syncHealth() method

### Tertiary (LOW confidence — architectural inference)
- LWW conflict resolution WHERE clause: standard SQLite pattern, not specifically tested in this codebase yet

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all pre-existing, no new dependencies
- Architecture: HIGH — existing patterns are well-established and directly readable
- Table schemas: HIGH for fleet_solutions/metrics_rollups (read from source); MEDIUM for model_evaluations (inferred from roadmap description)
- Pitfalls: HIGH for AUTOINCREMENT pitfall and normalize_timestamp (both from reading existing code); MEDIUM for conflict counting (new pattern)

**Research date:** 2026-04-01
**Valid until:** 2026-05-01 (stable infrastructure, slow-moving)
