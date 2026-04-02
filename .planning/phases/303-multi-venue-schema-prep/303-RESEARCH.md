# Phase 303: Multi-Venue Schema Prep - Research

**Researched:** 2026-04-01
**Domain:** SQLite schema migration, Rust/sqlx, multi-tenant data modeling
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- venue_id default: 'racingpoint-hyd-001'
- ALTERs must be idempotent (let _ = pattern for existing DBs)
- Must NOT break existing billing, sessions, game launch flows
- routes.rs is 16K lines — venue_id additions must be systematic, not ad-hoc
- Design doc: docs/MULTI-VENUE-ARCHITECTURE.md

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase.

### Deferred Ideas (OUT OF SCOPE)
None.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| VENUE-01 | All major tables have venue_id column (default: 'racingpoint-hyd-001') | 46 major tables identified; 2 already have venue_id; 44 need ALTER migrations |
| VENUE-02 | Migration is backward compatible — existing data gets default venue_id, no functional change | ALTERs with DEFAULT clause backfill existing rows automatically in SQLite |
| VENUE-03 | All INSERT/UPDATE queries include venue_id (prepared for multi-venue) | 157 INSERT/UPDATE sites across routes.rs + billing.rs + wallet.rs + lap_tracker.rs + reservation.rs |
| VENUE-04 | Design doc created: MULTI-VENUE-ARCHITECTURE.md with trigger conditions for venue 2 | docs/ directory exists; no pre-existing MULTI-VENUE-ARCHITECTURE.md |
</phase_requirements>

## Summary

Phase 303 adds `venue_id TEXT NOT NULL DEFAULT 'racingpoint-hyd-001'` to all major operational tables, making the schema multi-venue-ready without changing any application behavior. The change is purely additive: existing rows get the default silently via SQLite's ALTER TABLE, and all INSERT/UPDATE statements pass the constant value explicitly so the field is never NULL.

The schema has 3,925 lines in `db/mod.rs` across 60+ tables. Of those, two tables already have `venue_id`: `model_evaluations` (Phase 301, full column in CREATE TABLE) and `metrics_rollups` (Phase 301, ALTER migration). All other major tables need ALTER migrations. The constant `'racingpoint-hyd-001'` lives in `config.rs` `VenueConfig` (no `venue_id` field exists there yet — needs adding) or can be a module-level constant in `db/mod.rs`. The systematic approach is to add `pub fn venue_id(config: &Config) -> &str` or just use the constant directly in a helper.

INSERT/UPDATE scope: 157 statements in `routes.rs` (21,880 lines), 60 in `billing.rs`, 17 in `lap_tracker.rs`, 15 in `reservation.rs`, 5 in `wallet.rs` — total approximately 254 affected statements. However, only statements touching **major tables** (not lookup/config/internal tables) need the venue_id parameter.

**Primary recommendation:** Add `venue_id: String` to `VenueConfig` in `config.rs` (with default `'racingpoint-hyd-001'`), then thread it through all INSERT/UPDATE statements via `state.config.venue.venue_id`. Add all ALTERs to `db/mod.rs` in a single migration block.

---

## Standard Stack

### Core (already in use)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| sqlx | 0.7.x | Async SQLite queries, compile-time checking | Already used project-wide |
| serde | 1.x | Config deserialization | Already used project-wide |

**No new dependencies required.** This is pure migration + code change.

---

## Architecture Patterns

### Pattern 1: Idempotent ALTER Migration (existing project pattern)
**What:** `let _ = sqlx::query("ALTER TABLE X ADD COLUMN venue_id TEXT NOT NULL DEFAULT 'racingpoint-hyd-001'").execute(pool).await;`
**When to use:** Adding columns to tables that already exist in production. SQLite returns `SqliteError { code: 1, message: "duplicate column name: venue_id" }` which `let _` silently ignores.
**Example:**
```rust
// Source: existing pattern in db/mod.rs (lines 553-614, 1304-1313)
let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN venue_id TEXT NOT NULL DEFAULT 'racingpoint-hyd-001'")
    .execute(pool)
    .await;
```

**CRITICAL NOTE:** `ALTER TABLE ADD COLUMN` with `NOT NULL DEFAULT '...'` works in SQLite 3.37+ (2021). All production SQLite versions in this codebase are >= 3.37. The DEFAULT value is stored in the schema, not per-row — no UPDATE backfill needed. Existing rows read the default value transparently.

### Pattern 2: venue_id in Config (new addition)
**What:** Add `venue_id` to `VenueConfig` in `config.rs` so all code can access it via `state.config.venue.venue_id`.
**When to use:** When INSERT/UPDATE code has access to `state` (all route handlers and billing functions do).
**Example:**
```rust
// In config.rs:
pub struct VenueConfig {
    pub name: String,
    pub location: String,
    pub timezone: String,
    pub venue_gstin: String,
    #[serde(default = "default_venue_id")]
    pub venue_id: String,  // NEW
}

fn default_venue_id() -> String {
    "racingpoint-hyd-001".to_string()
}
```

No `racecontrol.toml` change required — serde default means it works without TOML updates.

### Pattern 3: INSERT with venue_id
**What:** Add `venue_id` column + `?` bind to existing INSERT statements.
**When to use:** Every INSERT into a major table.
**Example:**
```rust
// Before:
"INSERT INTO billing_sessions (id, driver_id, pod_id, pricing_tier_id, allocated_seconds, status)
 VALUES (?, ?, ?, ?, ?, ?)"
.bind(&session_id).bind(&driver_id)...

// After:
"INSERT INTO billing_sessions (id, driver_id, pod_id, pricing_tier_id, allocated_seconds, status, venue_id)
 VALUES (?, ?, ?, ?, ?, ?, ?)"
.bind(&session_id).bind(&driver_id)....bind(&state.config.venue.venue_id)
```

### Pattern 4: UPDATE with venue_id (existing rows)
**What:** Most UPDATEs should NOT add `venue_id = ?` unless they're changing the record's venue ownership (which never happens in single-venue mode). UPDATEs on existing rows already have the default venue_id from INSERT. Setting venue_id on UPDATE is unnecessary.
**When to use:** Only add `venue_id` on INSERT, not UPDATE.

### Anti-Patterns to Avoid
- **Rebuilding tables for venue_id:** Not needed; ALTER TABLE ADD COLUMN with DEFAULT is sufficient. Only use table rebuild (v2 migration pattern) if venue_id needs to be in a PRIMARY KEY or UNIQUE constraint — not the case here.
- **Hardcoding 'racingpoint-hyd-001' as string literals in routes.rs:** Use `state.config.venue.venue_id` instead. One constant to change when venue 2 is deployed.
- **Adding venue_id to every UPDATE:** Only INSERTs need it. UPDATE statements on existing rows already have venue_id from insert time. Exception: if you're doing INSERT OR REPLACE patterns (upserts), add venue_id.

---

## Table Classification

### Major Tables — Need venue_id (VENUE-01 scope)

These tables hold operational/financial/gameplay data that must be separable per venue:

| Table | Category | Priority |
|-------|----------|----------|
| `billing_sessions` | Financial/core | CRITICAL |
| `wallet_transactions` | Financial | CRITICAL |
| `wallets` | Financial | CRITICAL |
| `refunds` | Financial | CRITICAL |
| `invoices` | Financial | CRITICAL |
| `billing_events` | Financial audit | HIGH |
| `billing_audit_log` | Financial audit | HIGH |
| `auth_tokens` | Auth/billing | HIGH |
| `drivers` | Core entity | HIGH |
| `laps` | Gameplay | HIGH |
| `sessions` | Gameplay | HIGH |
| `pods` | Infrastructure | HIGH |
| `reservations` | Booking | HIGH |
| `debit_intents` | Financial | HIGH |
| `cafe_orders` | Financial | HIGH |
| `kiosk_experiences` | Config/content | MEDIUM |
| `events` | Competitive | MEDIUM |
| `event_entries` | Competitive | MEDIUM |
| `hotlap_events` | Competitive | MEDIUM |
| `hotlap_event_entries` | Competitive | MEDIUM |
| `championships` | Competitive | MEDIUM |
| `championship_standings` | Competitive | MEDIUM |
| `tournaments` | Competitive | MEDIUM |
| `tournament_registrations` | Competitive | MEDIUM |
| `driver_ratings` | Analytics | MEDIUM |
| `personal_bests` | Leaderboard | MEDIUM |
| `track_records` | Leaderboard | MEDIUM |
| `bookings` | Scheduling | MEDIUM |
| `group_sessions` | Multiplayer | MEDIUM |
| `coupon_redemptions` | Financial | MEDIUM |
| `pod_activity_log` | Operational log | MEDIUM |
| `game_launch_events` | Operational log | MEDIUM |
| `launch_events` | Operational log | MEDIUM |
| `recovery_events` | Operational log | MEDIUM |
| `billing_accuracy_events` | Analytics | MEDIUM |
| `dispute_requests` | Financial | MEDIUM |
| `session_feedback` | Analytics | MEDIUM |
| `memberships` | Financial | MEDIUM |
| `pod_reservations` | Operational | MEDIUM |
| `game_launch_requests` | Operational | MEDIUM |
| `system_events` | Operational log | MEDIUM |
| `split_sessions` | Financial | MEDIUM |
| `virtual_queue` | Operational | LOW |
| `review_nudges` | Marketing | LOW |

Already have venue_id (SKIP):
- `model_evaluations` — has venue_id from Phase 301 CREATE TABLE
- `metrics_rollups` — has venue_id from Phase 301 ALTER

### Tables That Should NOT Get venue_id (infrastructure/config/audit tables)

| Table | Reason |
|-------|--------|
| `settings` | Global config key-value, not venue-scoped |
| `kiosk_settings` | Venue-specific but managed by single venue config |
| `pricing_tiers` | Shared rate definitions — venue uses billing_rates |
| `billing_rates` | Global pricing config |
| `pricing_rules` | Global pricing rules |
| `staff_members` | Venue-scoped by deployment, not by column |
| `pods` | Venue-scoped by deployment |
| `ac_presets` | Game config, not venue data |
| `audit_log` | Global system audit, cross-venue by design |
| `config_audit_log` | System config audit |
| `config_push_queue` | Internal queue mechanism |
| `pod_configs` | Per-pod config, not venue data |
| `feature_flags` | Global feature management |
| `sync_log` | Replication mechanism, not venue data |
| `sync_state` | Replication state |
| `fleet_solutions` | Already has venue_id from fleet_kb.rs migrate() |
| `debug_playbooks` | Global debug reference |
| `debug_incidents` | Global operational debug |
| `debug_resolutions` | Global operational debug |
| `terminal_commands` | Internal automation |
| `system_settings` | Single-row system config |
| `data_retention_config` | Single-row config |
| `accounts` | Chart of accounts — global |
| `invoice_sequence` | Single-row counter — global |
| `admin_lockout` | Security state |
| `kiosk_allowlist` | Fleet-wide process allowlist |
| `achievements` | Badge definitions — global |
| `staff_badges` | Staff badge definitions |
| `membership_tiers` | Tier definitions |
| `packages` | Package definitions |
| `coupons` | Coupon definitions (redemptions get venue_id) |
| `nudge_templates` | Global template library |
| `campaign_templates` | Global template library |
| `hiring_sjts` | Global HR content |
| `job_preview` | Global HR content |
| `bonus_tiers` | Global bonus config |
| `time_trials` | Competition definitions |
| `app_health_log` | Internal monitoring |
| `deploy_logs` | Internal deploy tracking |
| `ai_suggestions` | Per-pod AI, not venue-scoped |
| `ai_training_pairs` | Global AI training |
| `ai_messages` | AI-to-AI comms |
| `action_queue` | Internal queue mechanism |
| `scheduler_events` | Internal scheduler |
| `pod_uptime_samples` | Fleet monitoring |
| `alert_incidents` | Fleet monitoring |
| `policy_rules` | Global policy config |
| `policy_eval_log` | Internal policy log |
| `game_presets` | Global game config |
| `metrics_samples` | TSDB — raw samples, venue-agnostic |
| `cafe_categories` | Menu category definitions |
| `cafe_items` | Menu item definitions |
| `cafe_promos` | Promo definitions |
| `notification_outbox` | Delivery queue mechanism |
| `pod_crash_events` | Fleet monitoring |
| `customer_sessions` | JWT sessions, security-scoped |

---

## INSERT/UPDATE Scope Analysis

Total affected files:
- `routes.rs` — 157 INSERT/UPDATE statements (21,880 lines)
- `billing.rs` — 60 INSERT/UPDATE statements
- `lap_tracker.rs` — 17 INSERT/UPDATE statements  
- `reservation.rs` — 15 INSERT/UPDATE statements
- `wallet.rs` — 5 INSERT/UPDATE statements

Of the 254 total statements, approximately **80-100** are into major tables needing venue_id. The rest are into config/lookup/internal tables.

**The systematic approach:** grep for `INSERT INTO <table>` for each major table, update each occurrence. For files outside routes.rs (billing.rs, wallet.rs, etc.), venue_id must be passed in via function parameter or `state.config.venue.venue_id`. Most functions already take `state: &Arc<AppState>`.

**Functions in billing.rs that need venue_id threaded through:**
- `defer_billing_start()` — inserts into `billing_sessions`, `billing_events`
- `defer_billing_with_precommitted_session()` — inserts into `billing_sessions`
- `handle_game_status_update()` — inserts into `billing_events`
- Direct SQL at lines 1026, 1863, 2772 — all insert `billing_sessions`
- These all take `state: &Arc<AppState>` — access via `state.config.venue.venue_id`

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Backfilling existing rows | Custom UPDATE loop | `ALTER TABLE ADD COLUMN ... DEFAULT 'racingpoint-hyd-001'` | SQLite applies DEFAULT to all new reads automatically; no per-row UPDATE needed |
| Checking if column exists | Checking sqlite_master | `let _ =` pattern (ignore duplicate column error) | Simpler and already the project convention (see lines 553-614 in db/mod.rs) |
| Venue ID lookup at query time | DB query to get venue config | `state.config.venue.venue_id` | config is in-memory, no DB round-trip needed |

**Key insight:** SQLite's `ALTER TABLE ADD COLUMN` with a `DEFAULT` clause stores the default in the schema metadata. All existing rows return the default value without any storage overhead or UPDATE migration. This is SQLite-specific behavior (since 3.37 it even allows NOT NULL with DEFAULT via this mechanism).

---

## Common Pitfalls

### Pitfall 1: ALTER TABLE with NOT NULL but no DEFAULT
**What goes wrong:** `ALTER TABLE X ADD COLUMN venue_id TEXT NOT NULL` fails on SQLite if the table has existing rows, because SQLite cannot guarantee the NOT NULL constraint is satisfied for existing rows without a DEFAULT.
**Why it happens:** SQLite documentation requires either: (a) a literal DEFAULT value, (b) nullable column, or (c) empty table.
**How to avoid:** Always use `ALTER TABLE X ADD COLUMN venue_id TEXT NOT NULL DEFAULT 'racingpoint-hyd-001'`
**Warning signs:** Migration step fails with "NOT NULL constraint failed"

### Pitfall 2: Missing venue_id on upsert patterns
**What goes wrong:** `INSERT OR REPLACE` / `INSERT OR IGNORE` / `ON CONFLICT DO UPDATE` patterns in Phase 301 cloud sync routes. If the base INSERT doesn't include venue_id, the upsert inherits NULL or loses the value on conflict resolution.
**Why it happens:** Upserts require explicit column lists to include venue_id.
**How to avoid:** Check all INSERT OR REPLACE and ON CONFLICT patterns in routes.rs (lines ~10736-10903) — these already have venue_id for fleet_solutions/model_evaluations/metrics_rollups.
**Warning signs:** venue_id NULL after an upsert operation

### Pitfall 3: Compile-time query checking (sqlx)
**What goes wrong:** sqlx compile-time checking via `query!` macro would fail if the column doesn't exist in the offline query cache. However, this project uses `sqlx::query()` (runtime, not compile-time), so this is not an issue.
**Why it happens:** `sqlx::query!` vs `sqlx::query()` distinction.
**How to avoid:** No action needed — project uses runtime queries throughout.
**Warning signs:** Build fails with "unknown column" during sqlx offline preparation

### Pitfall 4: Test helper CREATE TABLE statements
**What goes wrong:** Tests in routes.rs create minimal tables inline (e.g., `CREATE TABLE billing_events (id TEXT PRIMARY KEY, ...)`). These inline CREATE statements don't include venue_id, so test code that inserts with venue_id will fail.
**Why it happens:** Test fixtures create minimal schemas, not the full db/mod.rs schema.
**How to avoid:** Use `crate::db::init_pool(":memory:")` in new tests so the full migration runs, OR keep venue_id optional (NULL) in test-only INSERT statements.
**Warning signs:** Test compile/runtime failures about "table has N columns but M were supplied"

### Pitfall 5: Personal_bests and track_records have non-standard schema
**What goes wrong:** These tables were rebuilt in Phase 88 (migrate_leaderboard_sim_type) via the v2-rename pattern. The ALTER migration for venue_id must target the final table name (personal_bests, track_records — not the v2 variants).
**Why it happens:** Table rebuild left the final name correct, but the migration check uses pragma_table_info.
**How to avoid:** Standard `let _ = ALTER TABLE personal_bests ADD COLUMN venue_id ...` will work correctly.
**Warning signs:** None — this is just a note that Phase 88 doesn't change the approach.

---

## Code Examples

### Migration Block (add to end of migrate() in db/mod.rs)
```rust
// Source: project pattern, db/mod.rs lines 1302-1313

// Phase 303: venue_id column on all major operational tables
// DEFAULT 'racingpoint-hyd-001' — backward compatible, existing rows get the default
// NOT NULL enforced by the DEFAULT value; no UPDATE backfill required (SQLite 3.37+)
for table in &[
    "billing_sessions", "billing_events", "billing_audit_log",
    "wallet_transactions", "wallets", "refunds", "invoices",
    "auth_tokens", "drivers", "laps", "sessions",
    "reservations", "debit_intents", "cafe_orders",
    "kiosk_experiences", "events", "event_entries",
    "hotlap_events", "hotlap_event_entries", "championships",
    "championship_standings", "championship_rounds",
    "tournaments", "tournament_registrations", "tournament_matches",
    "driver_ratings", "personal_bests", "track_records",
    "bookings", "group_sessions", "group_session_members",
    "coupon_redemptions", "pod_activity_log",
    "game_launch_events", "launch_events", "recovery_events",
    "billing_accuracy_events", "dispute_requests",
    "session_feedback", "memberships", "pod_reservations",
    "game_launch_requests", "system_events", "split_sessions",
    "virtual_queue", "review_nudges", "multiplayer_results",
] {
    let _ = sqlx::query(&format!(
        "ALTER TABLE {} ADD COLUMN venue_id TEXT NOT NULL DEFAULT 'racingpoint-hyd-001'",
        table
    ))
    .execute(pool)
    .await;
}
```

### config.rs addition
```rust
// Source: existing VenueConfig pattern (config.rs line 114)
pub struct VenueConfig {
    pub name: String,
    #[serde(default = "default_location")]
    pub location: String,
    #[serde(default = "default_timezone")]
    pub timezone: String,
    #[serde(default = "default_venue_gstin")]
    pub venue_gstin: String,
    // NEW: venue identifier for multi-venue schema (VENUE-01)
    #[serde(default = "default_venue_id")]
    pub venue_id: String,
}

fn default_venue_id() -> String {
    "racingpoint-hyd-001".to_string()
}
```

### INSERT pattern with venue_id
```rust
// Source: existing billing.rs line 2772 (extended)
sqlx::query(
    "INSERT INTO billing_sessions (id, driver_id, pod_id, pricing_tier_id,
     allocated_seconds, status, custom_price_paise, started_at, staff_id,
     split_count, split_duration_minutes, venue_id)
     VALUES (?, ?, ?, ?, ?, 'active', ?, ?, ?, ?, ?, ?)",
)
.bind(&session_id)
// ... existing binds ...
.bind(&state.config.venue.venue_id)  // always last
.execute(&state.db)
.await
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| venue_id absent from schema | Add venue_id to all major tables | Phase 303 | Enables per-venue filtering without structural changes |
| Single-venue implicit assumption | Multi-venue explicit via venue_id column | Phase 303 | Second venue can be added by changing config value |

**Existing venue_id implementations to reference:**
- `fleet_kb.rs` lines 28-34: `venue_id TEXT` in `fleet_solutions` CREATE TABLE
- `db/mod.rs` line 1296: `venue_id TEXT` in `model_evaluations` CREATE TABLE
- `db/mod.rs` line 1307: ALTER migration for `metrics_rollups`
- `mesh_cloud_sync.rs` lines 15, 38-40: venue_id validation pattern

---

## Open Questions

1. **Should pods table get venue_id?**
   - What we know: `pods` table stores pod hardware info (IP, number, name). Pods belong to a physical venue by deployment.
   - What's unclear: In theory, a venue 2 server would have its own `pods` table. But for cross-venue admin queries in the future, venue_id on pods might be useful.
   - Recommendation: Include `pods` in the venue_id migration for completeness. It's a major operational table.

2. **What is the trigger doc content for MULTI-VENUE-ARCHITECTURE.md?**
   - What we know: VENUE-04 requires "trigger conditions for venue 2" documented.
   - What's unclear: Business triggers not specified in CONTEXT.md.
   - Recommendation: Document the technical trigger (config change + new DB file) and practical triggers (>8 pods needed, second physical location confirmed, revenue justification). These are straightforward to draft from context.

3. **Should `ai_suggestions` and `ai_training_pairs` get venue_id?**
   - What we know: These are per-pod AI tables. Pod is venue-scoped by deployment.
   - What's unclear: If AI knowledge base becomes shared across venues, venue_id would be needed to filter.
   - Recommendation: Skip them (pod_id already provides venue scoping via the fleet). Include them only if the cloud mesh sync would need to distinguish venue origins — but currently mesh sync uses `fleet_solutions` (which already has venue_id), not `ai_suggestions`.

---

## Environment Availability

Step 2.6: SKIPPED (no external dependencies — purely code + schema changes, no new tools required)

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in + tokio::test |
| Config file | none (inline per test module) |
| Quick run command | `cargo test -p racecontrol -- venue` |
| Full suite command | `cargo test -p racecontrol && cargo test -p rc-common && cargo test -p rc-agent` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| VENUE-01 | All major tables have venue_id column after migration | unit (in-memory DB) | `cargo test -p racecontrol -- test_venue_id_migration` | ❌ Wave 0 |
| VENUE-02 | Existing data gets default venue_id; existing queries unaffected | unit (in-memory DB) | `cargo test -p racecontrol -- test_venue_id_backward_compat` | ❌ Wave 0 |
| VENUE-03 | All INSERT paths include venue_id in column list | unit + compile check | `cargo test -p racecontrol -- test_billing_session_has_venue_id` | ❌ Wave 0 |
| VENUE-04 | MULTI-VENUE-ARCHITECTURE.md exists and is non-empty | smoke (file exists check) | `ls docs/MULTI-VENUE-ARCHITECTURE.md` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p racecontrol -- venue`
- **Per wave merge:** `cargo test -p racecontrol && cargo build --release --bin racecontrol`
- **Phase gate:** Full suite green + binary builds before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/racecontrol/src/api/routes.rs` — add `#[tokio::test] async fn test_venue_id_migration()` to existing test module that: (1) runs full `db::init_pool(":memory:")`, (2) checks `pragma_table_info` for venue_id on billing_sessions, laps, drivers
- [ ] `crates/racecontrol/src/api/routes.rs` — add `test_billing_session_has_venue_id()` that inserts a billing session via `start_billing_session` or direct SQL and asserts venue_id = 'racingpoint-hyd-001'
- [ ] Framework already present — no install needed

---

## Sources

### Primary (HIGH confidence)
- `db/mod.rs` (3,925 lines, fully read) — complete schema with all CREATE TABLE and ALTER migrations
- `api/routes.rs` (21,880 lines, grep analysis) — INSERT/UPDATE patterns and existing venue_id usage
- `billing.rs`, `wallet.rs`, `lap_tracker.rs`, `reservation.rs` — secondary INSERT files
- `config.rs` — VenueConfig struct (no venue_id field yet)
- `state.rs` — AppState has `pub config: Config` (access path confirmed)

### Secondary (MEDIUM confidence)
- `fleet_kb.rs` lines 28-34: existing venue_id column pattern in fleet_solutions
- `mesh_cloud_sync.rs`: existing venue_id validation pattern for cross-venue data

### Tertiary (LOW confidence)
- None

---

## Metadata

**Confidence breakdown:**
- Table classification: HIGH — full schema read, all 60+ tables inspected
- INSERT/UPDATE scope: HIGH — grep count verified (157 in routes.rs, 60 in billing.rs)
- Migration pattern: HIGH — existing project uses identical let _ = ALTER pattern
- Config access pattern: HIGH — state.config.venue.name already used in routes.rs line 776

**Research date:** 2026-04-01
**Valid until:** 2026-05-01 (schema is stable; no external dependencies)

## Project Constraints (from CLAUDE.md)

| Constraint | Source | Impact on Phase |
|-----------|--------|-----------------|
| No `.unwrap()` in production Rust | CLAUDE.md | All new sqlx queries use `?` or `let _` |
| Idempotent SQL migrations | CLAUDE.md | All ALTERs use `let _` ignore pattern |
| `cargo test` must pass before deploy | CLAUDE.md | Wave 0 tests required |
| Cascade updates: changing a column adds it to ALL tables | CLAUDE.md | Cannot add venue_id to only billing_sessions; must do all major tables in one phase |
| DB migrations must cover ALL consumers | CLAUDE.md | ALTER migrations in db/mod.rs; CREATE TABLE IF NOT EXISTS won't update existing tables |
| `touch build.rs` before release builds after commits | CLAUDE.md | Required in any deploy instructions |
