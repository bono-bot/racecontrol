# Phase 12: Data Foundation - Research

**Researched:** 2026-03-14
**Domain:** SQLite schema migration, index tuning, WAL configuration, Rust/sqlx migration patterns
**Confidence:** HIGH — based on direct codebase inspection of all relevant source files

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| DATA-01 | Database has composite covering indexes on laps table for leaderboard queries (track, car, valid, lap_time_ms) | Confirmed missing — only `idx_laps_track_car ON laps(track, car)` exists; the covering index with `valid` and `lap_time_ms` is absent |
| DATA-02 | Database has index on telemetry_samples (lap_id, offset_ms) for telemetry visualization | Confirmed missing — only `idx_telemetry_lap ON telemetry_samples(lap_id)` exists; `offset_ms` is not in the index, so ORDER BY triggers a sort pass |
| DATA-03 | SQLite WAL checkpoint is tuned (wal_autocheckpoint=400, connection max_lifetime=300s) to prevent read latency growth | Confirmed missing — `init_pool()` has no `wal_autocheckpoint` pragma and no `max_lifetime` on the pool |
| DATA-04 | Venue drivers table has cloud_driver_id column that resolves UUID mismatch before lap sync | Confirmed missing — drivers table has no `cloud_driver_id` column anywhere in the migration |
| DATA-05 | Database schema includes hotlap_events, hotlap_event_entries, championships, championship_rounds, championship_standings, and driver_ratings tables | Confirmed missing — these six tables do not exist; only scaffold `events` and `event_entries` exist |
| DATA-06 | Laps table has car_class column populated from car-to-class mapping on lap completion | Confirmed missing — laps table has no `car_class` column; `kiosk_experiences.car_class` exists and is the lookup source |
</phase_requirements>

---

## Summary

Phase 12 is pure database infrastructure — no customer-visible UI, no API surface changes, no frontend work. Every subsequent phase (leaderboards, events, championships, telemetry) builds directly on the tables and indexes created here. Getting it wrong in Phase 12 causes retrofitting pain through five interconnected modules later.

The current schema at the end of `db/mod.rs` (lines 1-1790, confirmed via direct read) has four confirmed deficiencies that will cause production failures as data accumulates. The `telemetry_samples` table has a single-column index `idx_telemetry_lap` that covers the `WHERE lap_id = ?` filter but does not include `offset_ms`, meaning every `ORDER BY offset_ms` query still performs a sort pass — the full covering index `(lap_id, offset_ms)` is absent. The `laps` table has `idx_laps_track_car ON laps(track, car)` but lacks the composite covering index `(track, car, valid, lap_time_ms)` that leaderboard GROUP BY + ORDER BY requires without a temp table sort. WAL mode is enabled via `PRAGMA journal_mode=WAL` in `migrate()` but `wal_autocheckpoint` is not set (defaults to 1000 pages) and the sqlx pool has no `max_lifetime`, which together cause WAL file growth and read latency degradation over hours of continuous operation. The `drivers` table has no `cloud_driver_id` column, meaning the known UUID mismatch documented in MEMORY.md is still unresolved and will corrupt competitive leaderboard data the moment lap sync is extended.

Six new tables must be added as idempotent `CREATE TABLE IF NOT EXISTS` statements appended to the `migrate()` function. The `laps` table needs a `car_class` column via `ALTER TABLE ... ADD COLUMN` (the established pattern in this codebase). The `lap_tracker.rs` `persist_lap()` function must be extended to populate `car_class` by looking up the active billing session's `experience_id` in `kiosk_experiences`. All of these changes run at startup on both venue and cloud instances — the schema must be identical on both sides for the sync to work.

**Primary recommendation:** Add all indexes, pragmas, new tables, and the cloud_driver_id column as one contiguous block at the end of `migrate()` in `db/mod.rs`, then extend `persist_lap()` in `lap_tracker.rs` for `car_class` population. Test with `cargo test -p rc-core` using the existing in-memory SQLite test pattern. Deploy to venue and verify with `EXPLAIN QUERY PLAN` before declaring done.

---

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| sqlx | 0.8 (already in Cargo.toml) | SQLite async queries, pool options | Already used for all DB work; no new crate needed |
| SQLite | bundled with sqlx | Storage engine (WAL mode) | Already in production at venue |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| sqlx::sqlite::SqlitePoolOptions | 0.8 | Pool configuration including max_lifetime | Used in `db::init_pool()` — extend it there |
| PRAGMA wal_autocheckpoint | SQLite built-in | Controls WAL checkpoint frequency | Set once in `migrate()` alongside existing WAL pragma |
| PRAGMA busy_timeout | SQLite built-in | How long readers wait on locked DB | Set in `migrate()` — already needed for cloud SQLite under concurrent PWA reads |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `ALTER TABLE laps ADD COLUMN car_class` | Rebuild laps table with car_class | ALTER is the established pattern in this codebase (40+ existing ALTER statements); rebuild is unnecessary for a nullable column |
| `let _ = ALTER TABLE` pattern | sqlx migrations with versioned files | The codebase uses idempotent startup migrations, not versioned files. Do not introduce a new migration system in Phase 12. |
| Append to `migrate()` | Separate migration module | Consistent with all 40+ existing tables — stay in `migrate()` |

**Installation:** No new dependencies. All changes are SQL statements and Rust using existing imports.

---

## Architecture Patterns

### Recommended Project Structure

Phase 12 touches exactly three files:

```
crates/rc-core/src/
├── db/mod.rs          # All SQL changes: pragmas + indexes + new tables + ALTER TABLE
└── lap_tracker.rs     # car_class population on lap persist
crates/rc-core/tests/
└── integration.rs     # Test migrations must be updated to include new columns/tables
```

No new files. No new modules. No new crates.

### Pattern 1: Idempotent Migration Append

**What:** Every schema addition uses `CREATE TABLE IF NOT EXISTS` and `let _ = sqlx::query("ALTER TABLE ... ADD COLUMN")`. The `let _ =` discards the error when the column already exists (SQLite returns `table already has column` — not a fatal error). This is the established pattern for all 40+ tables and all ~20 ALTER TABLE statements in `db/mod.rs`.

**When to use:** For ALL schema additions in Phase 12. Never use a different migration strategy.

**Example (from existing code at line 346):**
```rust
// Source: crates/rc-core/src/db/mod.rs line 346
let _ = sqlx::query("ALTER TABLE drivers ADD COLUMN has_used_trial BOOLEAN DEFAULT 0")
    .execute(pool)
    .await;
```

**Pattern for new tables (from existing code at line 27):**
```rust
// Source: crates/rc-core/src/db/mod.rs line 27
sqlx::query(
    "CREATE TABLE IF NOT EXISTS <name> (
        id TEXT PRIMARY KEY,
        ...
    )",
)
.execute(pool)
.await?;
```

Note: `CREATE TABLE IF NOT EXISTS` uses `?` propagation (hard error if CREATE itself fails unexpectedly). `ALTER TABLE ADD COLUMN` uses `let _ =` (silently ignore "already exists" error). Know which to use when.

### Pattern 2: WAL Pragma Block

**What:** PRAGMAs are set immediately after `PRAGMA journal_mode=WAL` in `migrate()`. They run once at every startup on both venue and cloud instances.

**Where to add (after line 25 in db/mod.rs):**
```rust
// Source: confirmed missing from codebase; pattern from SQLite official docs + phiresky.github.io
sqlx::query("PRAGMA journal_mode=WAL").execute(pool).await?;
sqlx::query("PRAGMA foreign_keys=ON").execute(pool).await?;
sqlx::query("PRAGMA wal_autocheckpoint=400").execute(pool).await?;  // ADD: was missing
sqlx::query("PRAGMA busy_timeout=5000").execute(pool).await?;       // ADD: was missing
```

**Pool max_lifetime — where to add (in `init_pool()` at line 11):**
```rust
// Source: sqlx 0.8 SqlitePoolOptions API; confirmed missing from current init_pool()
let pool = SqlitePoolOptions::new()
    .max_connections(5)
    .max_lifetime(std::time::Duration::from_secs(300))  // ADD: recycle every 5 min
    .connect(&url)
    .await?;
```

### Pattern 3: car_class Lookup in lap_tracker.rs

**What:** When `persist_lap()` inserts a lap, it must look up the `car_class` from `kiosk_experiences` via the active billing session's `experience_id`.

**Source of truth:** `billing_sessions.experience_id` -> `kiosk_experiences.car_class`

**How to look it up:** The billing session for the pod is already resolved via `resolve_driver_for_pod()` which returns `(driver_id, session_id)`. Extend `persist_lap()` to query `kiosk_experiences.car_class` before the INSERT.

**Lookup query:**
```rust
// Source: derived from billing_sessions.experience_id -> kiosk_experiences.car_class join
// billing_sessions.experience_id added at db/mod.rs line 655
let car_class = sqlx::query_as::<_, (Option<String>,)>(
    "SELECT ke.car_class
     FROM billing_sessions bs
     JOIN kiosk_experiences ke ON ke.id = bs.experience_id
     WHERE bs.driver_id = ? AND bs.status = 'active'
     LIMIT 1",
)
.bind(&lap.driver_id)
.fetch_optional(&state.db)
.await
.ok()
.flatten()
.and_then(|(c,)| c);
```

**Then add `car_class` to the INSERT:**
```rust
// Source: extend existing INSERT in lap_tracker.rs line 32
"INSERT INTO laps (id, session_id, driver_id, pod_id, sim_type, track, car,
                   lap_number, lap_time_ms, sector1_ms, sector2_ms, sector3_ms,
                   valid, car_class, created_at)
 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, datetime('now'))"
```

**Historical laps:** All laps before Phase 12 will have `NULL car_class`. This is correct and expected — NULL laps will not auto-qualify for hotlap events (Phase 14 behavior). Do NOT backfill historical laps. NULL is the explicit sentinel for "pre-v3.0 lap."

### Anti-Patterns to Avoid

- **Adding competitive tables to SYNC_TABLES constant:** `cloud_sync.rs` line 18 shows `const SYNC_TABLES: &str = "drivers,wallets,..."`. Never add `hotlap_events`, `championships`, etc. to this constant. It controls bidirectional config sync. Competitive tables go into `collect_push_payload()` only (Phase 14 concern, but the new tables must not be added to SYNC_TABLES even accidentally in Phase 12).
- **Adding `?` propagation to ALTER TABLE:** The `?` operator will abort the entire migration if an ALTER TABLE statement fails. Use `let _ =` for all column additions. This is the consistent pattern across all 20+ existing ALTER statements.
- **Running EXPLAIN QUERY PLAN without the index present:** Always verify indexes with EXPLAIN after the migration runs on the actual production DB file, not just a fresh in-memory DB. The index may already exist from a partial earlier run.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Migration versioning | Custom migration version table | Idempotent `CREATE TABLE IF NOT EXISTS` + `let _ = ALTER TABLE` | The codebase has 40+ tables with this pattern; introducing versioned migrations in Phase 12 would require migrating existing production DB — high risk for zero benefit |
| WAL tuning | Custom checkpoint logic | `PRAGMA wal_autocheckpoint=400` | SQLite handles checkpointing automatically with the correct pragma value |
| Index analysis | Custom query planner | `EXPLAIN QUERY PLAN` (SQLite built-in) | Directly shows whether index is used; no code needed |

**Key insight:** Phase 12 is SQL, not Rust logic. The only Rust change is `init_pool()` pool configuration and `persist_lap()` car_class lookup. Everything else is SQL statements appended to `migrate()`.

---

## Common Pitfalls

### Pitfall 1: Replacing the Single-Column Telemetry Index

**What goes wrong:** The existing `idx_telemetry_lap ON telemetry_samples(lap_id)` is present (db/mod.rs line 454). A naive approach would `DROP INDEX` and recreate it. This risks locking the table on a production DB with millions of rows.

**Why it happens:** The new covering index `(lap_id, offset_ms)` supersedes the old single-column index. The temptation is to drop the old one.

**How to avoid:** Do NOT drop `idx_telemetry_lap`. Instead add the new `idx_telemetry_lap_offset ON telemetry_samples(lap_id, offset_ms)` alongside it using `CREATE INDEX IF NOT EXISTS`. SQLite's query planner will prefer the more specific covering index. The old index becomes harmless dead weight — drop it only in a future cleanup phase when you know the covering index is present everywhere.

**Warning signs:** Migration that contains `DROP INDEX idx_telemetry_lap` — remove it.

### Pitfall 2: Integration Test Schema Drift

**What goes wrong:** `crates/rc-core/tests/integration.rs` contains `run_test_migrations()` which manually replicates the production schema for in-memory SQLite tests. When Phase 12 adds `car_class` to `laps`, the test migration will be out of sync — tests that insert into `laps` without `car_class` will fail FK errors or produce stale results.

**Why it happens:** `run_test_migrations()` is a manual copy of `db/mod.rs` schema, not a shared function. It drifts when new columns are added.

**How to avoid:** After adding `car_class` to `laps` in `db/mod.rs`, also add `car_class TEXT` to the `CREATE TABLE laps` statement inside `run_test_migrations()` in `tests/integration.rs`. Same for the six new tables if any integration tests exercise them.

**Warning signs:** `cargo test -p rc-core` fails with "table laps has no column named car_class" on `persist_lap` tests.

### Pitfall 3: cloud_driver_id column added but not used

**What goes wrong:** DATA-04 requires the `cloud_driver_id` column to exist and to be consulted before syncing a lap. Adding the column satisfies the success criterion literally but leaves the sync logic broken — `cloud_sync.rs` still pushes laps using `driver_id` without checking `cloud_driver_id`.

**Why it happens:** The success criterion says "the cloud_driver_id column resolves before any lap is written to competitive tables." Phase 12 only creates the schema plumbing. The actual resolution logic (blocking unresolved laps from sync) belongs in Phase 14 when lap sync is extended to competitive tables. The column must exist first.

**How to avoid:** Phase 12 adds the column. Phase 12 does NOT modify `cloud_sync.rs`. The planner must clearly document that the column addition satisfies DATA-04 at the schema level, with the enforcement logic deferred to Phase 14's sync extension task. This is correct phasing — do not add sync enforcement in Phase 12.

### Pitfall 4: WAL autocheckpoint runs on both venue and cloud

**What goes wrong:** The `migrate()` function runs on startup for both the venue instance and the cloud instance (same binary, `cloud.mode=true` flag). The WAL pragma settings are appropriate for both.

**Why it happens:** Developers worry about changing cloud behavior when adding pragmas.

**How to avoid:** `PRAGMA wal_autocheckpoint=400` and `PRAGMA busy_timeout=5000` are correct and necessary on both instances. The cloud instance is also SQLite. No conditional logic is needed.

### Pitfall 5: championship_rounds table does not exist

**What goes wrong:** DATA-05 requires "hotlap_events, hotlap_event_entries, championships, championship_rounds, championship_standings, driver_ratings." ARCHITECTURE.md from prior research specifies `championship_standings` as the materialized table, not `championship_rounds`. The REQUIREMENTS.md wording includes "championship_rounds" as a required table name.

**Why it happens:** REQUIREMENTS.md and ARCHITECTURE.md use slightly different table names. REQUIREMENTS.md was written at a higher level; ARCHITECTURE.md was written with the implementation in mind.

**How to avoid:** Phase 12 must create all six tables named in REQUIREMENTS.md DATA-05 verbatim: `hotlap_events`, `hotlap_event_entries`, `championships`, `championship_rounds`, `championship_standings`, `driver_ratings`. The `championship_rounds` table maps events (rounds) to championships — it is the foreign key join table between `championships` and `hotlap_events`. ARCHITECTURE.md's `championship_standings` is a separate materialized standings table. Both must exist.

**championship_rounds schema** (derived from requirements and architecture):
```sql
CREATE TABLE IF NOT EXISTS championship_rounds (
    championship_id TEXT NOT NULL REFERENCES championships(id),
    event_id TEXT NOT NULL REFERENCES hotlap_events(id),
    round_number INTEGER NOT NULL,
    PRIMARY KEY (championship_id, event_id)
)
```

---

## Code Examples

Verified patterns from direct codebase inspection:

### Covering index — leaderboard query (DATA-01)
```sql
-- Source: missing from db/mod.rs; required pattern from SQLite optoverview.html
-- This index makes GROUP BY driver_id ORDER BY MIN(lap_time_ms) on (track, car, valid) a pure index scan
CREATE INDEX IF NOT EXISTS idx_laps_leaderboard
ON laps(track, car, valid, lap_time_ms);

-- Driver profile index (all laps for one driver, sorted by date)
CREATE INDEX IF NOT EXISTS idx_laps_driver_created
ON laps(driver_id, created_at);

-- Session index (laps within one session — needed for group event queries later)
-- idx_laps_session already exists at line 445 — verify, do not duplicate
```

### Covering index — telemetry visualization (DATA-02)
```sql
-- Source: required by DATA-02; replaces (supersedes) idx_telemetry_lap
-- EXPLAIN QUERY PLAN on "SELECT * FROM telemetry_samples WHERE lap_id=? ORDER BY offset_ms"
-- should show "USING INDEX idx_telemetry_lap_offset" not "SCAN TABLE"
CREATE INDEX IF NOT EXISTS idx_telemetry_lap_offset
ON telemetry_samples(lap_id, offset_ms);
-- Do NOT drop idx_telemetry_lap — add alongside it
```

### WAL tuning (DATA-03)
```rust
// Source: db/mod.rs migrate() function, after line 25; sqlx 0.8 SqlitePoolOptions API
// In migrate():
sqlx::query("PRAGMA wal_autocheckpoint=400").execute(pool).await?;
sqlx::query("PRAGMA busy_timeout=5000").execute(pool).await?;

// In init_pool() (line 11), add max_lifetime:
let pool = SqlitePoolOptions::new()
    .max_connections(5)
    .max_lifetime(std::time::Duration::from_secs(300))
    .connect(&url)
    .await?;
```

### cloud_driver_id column (DATA-04)
```rust
// Source: established ALTER TABLE pattern from db/mod.rs line 346+
// Append to end of migrate():
let _ = sqlx::query("ALTER TABLE drivers ADD COLUMN cloud_driver_id TEXT")
    .execute(pool)
    .await;
let _ = sqlx::query(
    "CREATE UNIQUE INDEX IF NOT EXISTS idx_drivers_cloud_id ON drivers(cloud_driver_id)"
)
.execute(pool)
.await;
```

### Six new competitive tables (DATA-05)
```sql
-- Source: ARCHITECTURE.md table schemas, adapted to include championship_rounds per REQUIREMENTS.md

-- 1. hotlap_events
CREATE TABLE IF NOT EXISTS hotlap_events (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    track TEXT NOT NULL,
    car TEXT NOT NULL,
    car_class TEXT NOT NULL,
    sim_type TEXT NOT NULL DEFAULT 'assetto_corsa',
    status TEXT NOT NULL DEFAULT 'upcoming'
        CHECK(status IN ('upcoming', 'active', 'scoring', 'completed', 'cancelled')),
    starts_at TEXT,
    ends_at TEXT,
    rule_107_percent INTEGER DEFAULT 1,
    reference_time_ms INTEGER,
    max_valid_laps INTEGER,
    championship_id TEXT REFERENCES championships(id),
    created_by TEXT,
    created_at TEXT DEFAULT (datetime('now')),
    updated_at TEXT DEFAULT (datetime('now'))
);

-- 2. hotlap_event_entries
CREATE TABLE IF NOT EXISTS hotlap_event_entries (
    id TEXT PRIMARY KEY,
    event_id TEXT NOT NULL REFERENCES hotlap_events(id),
    driver_id TEXT NOT NULL REFERENCES drivers(id),
    lap_id TEXT REFERENCES laps(id),
    lap_time_ms INTEGER,
    sector1_ms INTEGER,
    sector2_ms INTEGER,
    sector3_ms INTEGER,
    position INTEGER,
    points INTEGER DEFAULT 0,
    badge TEXT,
    gap_to_leader_ms INTEGER,
    within_107_percent INTEGER DEFAULT 1,
    result_status TEXT DEFAULT 'pending'
        CHECK(result_status IN ('pending', 'finished', 'dns', 'dnf')),
    entered_at TEXT DEFAULT (datetime('now')),
    UNIQUE(event_id, driver_id)
);

-- 3. championships
CREATE TABLE IF NOT EXISTS championships (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    season TEXT,
    car_class TEXT NOT NULL,
    sim_type TEXT NOT NULL DEFAULT 'assetto_corsa',
    status TEXT NOT NULL DEFAULT 'upcoming'
        CHECK(status IN ('upcoming', 'active', 'completed')),
    scoring_system TEXT NOT NULL DEFAULT 'f1_2010',
    total_rounds INTEGER DEFAULT 0,
    completed_rounds INTEGER DEFAULT 0,
    created_at TEXT DEFAULT (datetime('now')),
    updated_at TEXT DEFAULT (datetime('now'))
);

-- 4. championship_rounds (maps events to championships as rounds)
CREATE TABLE IF NOT EXISTS championship_rounds (
    championship_id TEXT NOT NULL REFERENCES championships(id),
    event_id TEXT NOT NULL REFERENCES hotlap_events(id),
    round_number INTEGER NOT NULL,
    PRIMARY KEY (championship_id, event_id)
);

-- 5. championship_standings (materialized — recomputed after each round scores)
CREATE TABLE IF NOT EXISTS championship_standings (
    championship_id TEXT NOT NULL REFERENCES championships(id),
    driver_id TEXT NOT NULL REFERENCES drivers(id),
    position INTEGER,
    total_points INTEGER DEFAULT 0,
    rounds_entered INTEGER DEFAULT 0,
    best_result INTEGER,
    wins INTEGER DEFAULT 0,
    podiums INTEGER DEFAULT 0,
    updated_at TEXT DEFAULT (datetime('now')),
    PRIMARY KEY (championship_id, driver_id)
);

-- 6. driver_ratings
CREATE TABLE IF NOT EXISTS driver_ratings (
    driver_id TEXT PRIMARY KEY REFERENCES drivers(id),
    rating_class TEXT NOT NULL DEFAULT 'Rookie',
    class_points INTEGER NOT NULL DEFAULT 0,
    total_events INTEGER DEFAULT 0,
    total_podiums INTEGER DEFAULT 0,
    total_wins INTEGER DEFAULT 0,
    updated_at TEXT DEFAULT (datetime('now'))
);
```

### Indexes for new tables
```sql
-- Source: ARCHITECTURE.md index recommendations
CREATE INDEX IF NOT EXISTS idx_hotlap_events_status ON hotlap_events(status, track);
CREATE INDEX IF NOT EXISTS idx_hotlap_events_updated ON hotlap_events(updated_at);
CREATE INDEX IF NOT EXISTS idx_hotlap_entries_event ON hotlap_event_entries(event_id, position);
CREATE INDEX IF NOT EXISTS idx_hotlap_entries_driver ON hotlap_event_entries(driver_id);
CREATE INDEX IF NOT EXISTS idx_championships_updated ON championships(updated_at);
CREATE INDEX IF NOT EXISTS idx_champ_rounds_champ ON championship_rounds(championship_id, round_number);
CREATE INDEX IF NOT EXISTS idx_champ_standings_champ ON championship_standings(championship_id, position);
CREATE INDEX IF NOT EXISTS idx_driver_ratings_class ON driver_ratings(rating_class, class_points);
```

### car_class column on laps (DATA-06)
```rust
// Source: established pattern from db/mod.rs ALTER TABLE blocks
// Append to migrate():
let _ = sqlx::query("ALTER TABLE laps ADD COLUMN car_class TEXT")
    .execute(pool)
    .await;
// Support leaderboard by class:
let _ = sqlx::query(
    "CREATE INDEX IF NOT EXISTS idx_laps_car_class ON laps(track, car_class)"
)
.execute(pool)
.await;
```

### Verifying DATA-01 and DATA-02 with EXPLAIN QUERY PLAN
```sql
-- Run these via sqlite3 CLI against the production racecontrol.db to verify
-- DATA-01 verification:
EXPLAIN QUERY PLAN
SELECT driver_id, MIN(lap_time_ms) as best
FROM laps
WHERE track = 'spa' AND car = 'ks_ferrari_sf15t' AND valid = 1
GROUP BY driver_id
ORDER BY best ASC
LIMIT 20;
-- Expected: "SEARCH laps USING INDEX idx_laps_leaderboard" — NOT "SCAN TABLE"

-- DATA-02 verification:
EXPLAIN QUERY PLAN
SELECT offset_ms, speed, throttle, brake, steering
FROM telemetry_samples
WHERE lap_id = 'test-lap-id'
ORDER BY offset_ms ASC
LIMIT 2000;
-- Expected: "SEARCH telemetry_samples USING INDEX idx_telemetry_lap_offset" — NOT "SCAN TABLE"
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `idx_telemetry_lap(lap_id)` only | `idx_telemetry_lap_offset(lap_id, offset_ms)` covering | Phase 12 | ORDER BY offset_ms becomes index-only, no sort pass |
| `idx_laps_track_car(track, car)` only | `idx_laps_leaderboard(track, car, valid, lap_time_ms)` covering | Phase 12 | GROUP BY + ORDER BY leaderboard query is index scan, no temp table |
| No wal_autocheckpoint | `PRAGMA wal_autocheckpoint=400` | Phase 12 | WAL file stays bounded; reads don't degrade over uptime |
| No pool max_lifetime | `max_lifetime(300s)` | Phase 12 | Pool connections recycle, checkpoint can complete |
| No cloud_driver_id | `drivers.cloud_driver_id` column | Phase 12 | Foundation for ID-safe lap sync in Phase 14 |

**Not changed in Phase 12 (deferred):**
- `cloud_sync.rs`: No new tables pushed to cloud yet (Phase 14)
- `collect_push_payload()`: Not extended yet (Phase 14)
- `hotlap_events.rs`, `championships.rs`, `driver_rating.rs`: New modules created in Phase 14

---

## Open Questions

1. **Does `hotlap_events.car` reference the car name or the kiosk_experiences row?**
   - What we know: `kiosk_experiences` stores `car TEXT` (e.g., `ks_ferrari_sf15t`) and `car_class TEXT` (e.g., `A`). Event matching in Phase 14 will use `car_class` not `car` for auto-entry.
   - What's unclear: The `hotlap_events.car` column — is it a display label or the exact car string from `laps.car`? For Phase 12, both are fine (it's just stored). The matching logic in Phase 14 will clarify.
   - Recommendation: Store `car` as a free-text display field on `hotlap_events`. The auto-entry logic in Phase 14 matches on `car_class`, not `car`. Mark `car` as nullable on `hotlap_events` if the event allows any car of the class.

2. **Should `result_status` be added to existing `event_entries` or only to `hotlap_event_entries`?**
   - What we know: `PITFALLS.md` identifies lack of result_status on event entries as a championship scoring edge case. The existing `event_entries` scaffold table (db/mod.rs line 148) has no `result_status` column.
   - What's unclear: Whether to also patch the existing `events`/`event_entries` scaffold or leave it as-is since the new `hotlap_event_entries` table replaces its function.
   - Recommendation: The `hotlap_event_entries` new table (Phase 12) has `result_status` built in from day one. The old `event_entries` scaffold can remain unchanged — it is unused by any active feature.

---

## Validation Architecture

nyquist_validation is enabled (config.json `workflow.nyquist_validation: true`).

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in test framework + tokio-test (already in Cargo.toml) |
| Config file | Cargo.toml (workspace) — no separate config file |
| Quick run command | `export PATH="$PATH:/c/Users/bono/.cargo/bin" && cargo test -p rc-core 2>&1 \| tail -20` |
| Full suite command | `export PATH="$PATH:/c/Users/bono/.cargo/bin" && cargo test -p rc-common && cargo test -p rc-agent && cargo test -p rc-core 2>&1 \| tail -30` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| DATA-01 | Leaderboard index exists and EXPLAIN shows index usage | unit | `cargo test -p rc-core -- test_leaderboard_index_exists -x` | ❌ Wave 0 |
| DATA-02 | Telemetry index exists and EXPLAIN shows covering index | unit | `cargo test -p rc-core -- test_telemetry_index_exists -x` | ❌ Wave 0 |
| DATA-03 | WAL autocheckpoint=400 and pool max_lifetime set | unit | `cargo test -p rc-core -- test_wal_tuning -x` | ❌ Wave 0 |
| DATA-04 | drivers table has cloud_driver_id column | unit | `cargo test -p rc-core -- test_cloud_driver_id_column -x` | ❌ Wave 0 |
| DATA-05 | All six competitive tables accept valid inserts | unit | `cargo test -p rc-core -- test_competitive_tables_exist -x` | ❌ Wave 0 |
| DATA-06 | laps.car_class populated on lap persist | unit | `cargo test -p rc-core -- test_lap_car_class_populated -x` | ❌ Wave 0 |

All DATA-01 through DATA-06 tests must be written in Wave 0 before implementation.

### Sampling Rate

- **Per task commit:** `cargo test -p rc-core -- db 2>&1 | tail -20`
- **Per wave merge:** `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p rc-core`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps

The following test stubs must be written before implementation begins:

- [ ] `crates/rc-core/tests/integration.rs` — add `test_leaderboard_index_exists`: run migration on in-memory DB, then `EXPLAIN QUERY PLAN` the leaderboard query and assert the output contains "idx_laps_leaderboard"
- [ ] `crates/rc-core/tests/integration.rs` — add `test_telemetry_index_exists`: run migration, EXPLAIN the telemetry query, assert output contains "idx_telemetry_lap_offset"
- [ ] `crates/rc-core/tests/integration.rs` — add `test_wal_tuning`: run migration, query `PRAGMA wal_autocheckpoint`, assert value is 400
- [ ] `crates/rc-core/tests/integration.rs` — add `test_cloud_driver_id_column`: run migration, INSERT into drivers, assert SELECT of `cloud_driver_id` does not fail
- [ ] `crates/rc-core/tests/integration.rs` — add `test_competitive_tables_exist`: run migration, attempt INSERT into all six tables with minimal valid data, assert no error
- [ ] `crates/rc-core/tests/integration.rs` — add `test_lap_car_class_populated`: call `persist_lap()` with a seeded billing_session and kiosk_experience, assert laps.car_class matches experience.car_class
- [ ] `run_test_migrations()` in `tests/integration.rs` must be updated to include `car_class TEXT` in the laps table and all six new competitive tables, to prevent schema drift failures on existing tests

Framework install: Not needed — tokio and sqlx are already in Cargo.toml.

---

## Sources

### Primary (HIGH confidence — direct codebase inspection)

- `crates/rc-core/src/db/mod.rs` (lines 1–1791, read in full) — confirmed: `idx_telemetry_lap` exists (single-column, no offset_ms); `idx_laps_track_car` exists (no valid/lap_time_ms); no wal_autocheckpoint pragma; no cloud_driver_id on drivers; no hotlap_events, hotlap_event_entries, championships, championship_rounds, championship_standings, driver_ratings tables; no car_class on laps
- `crates/rc-core/src/lap_tracker.rs` (lines 1–151, read in full) — confirmed: `persist_lap()` has no car_class lookup or population; track_records update is a bare UPDATE not wrapped in BEGIN IMMEDIATE; no hotlap event hook exists
- `crates/rc-core/src/cloud_sync.rs` (lines 1–461, partially read) — confirmed: `SYNC_TABLES` constant does not include competitive tables; `collect_push_payload()` pushes laps without cloud_driver_id check; no competitive table queries in payload
- `crates/rc-core/tests/integration.rs` (lines 1–80+, partially read) — confirmed: `run_test_migrations()` is a manual copy of the schema; it will need updating for new columns/tables
- `.planning/config.json` — confirmed: `nyquist_validation: true`

### Secondary (HIGH confidence — prior research documents)

- `.planning/research/SUMMARY.md` — Phase 1 (Data Foundation) rationale and deliverables list; confirmed four production risks
- `.planning/research/ARCHITECTURE.md` — table schemas for all six new tables; index list; anti-patterns; integration points
- `.planning/research/PITFALLS.md` — Pitfall 1 (telemetry index), Pitfall 3 (laps index), Pitfall 6 (WAL), Pitfall 7 (ID mismatch) all verified against actual code

### Tertiary (MEDIUM confidence — official documentation, cited in prior research)

- SQLite WAL documentation (sqlite.org/wal.html) — `wal_autocheckpoint` default is 1000 pages; behavior with persistent readers confirmed
- SQLite query optimizer (sqlite.org/optoverview.html) — covering index behavior for GROUP BY + ORDER BY confirmed
- sqlx 0.8 `SqlitePoolOptions::max_lifetime` — API confirmed in prior research; pattern consistent with Rust docs

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — no new dependencies, all changes are SQL + minimal Rust
- Architecture: HIGH — based on full read of all three source files; exact line numbers confirmed
- Pitfalls: HIGH — all four confirmed-missing items verified by direct code inspection; no speculation

**Research date:** 2026-03-14
**Valid until:** 2026-04-14 (schema is stable; SQLite and sqlx APIs do not change rapidly)
