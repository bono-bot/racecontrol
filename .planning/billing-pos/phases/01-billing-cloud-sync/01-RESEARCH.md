# Phase 1: Billing Cloud Sync - Research

**Researched:** 2026-03-14
**Domain:** Cloud data synchronization for billing tables (Rust/Axum, SQLite, HTTP/relay sync)
**Confidence:** HIGH

## Summary

The existing `cloud_sync.rs` infrastructure already does 90% of what Phase 1 needs. `billing_sessions` and `wallet_transactions` are **already collected and pushed** in `collect_push_payload()`. The cloud-side `sync_push` handler in `routes.rs` **already receives and upserts both tables**. The only table genuinely missing from the push payload is `billing_events`.

The `SYNC_TABLES` constant (`"drivers,wallets,pricing_tiers,pricing_rules,kiosk_experiences,kiosk_settings"`) controls the HTTP fallback **pull** path only -- it governs what the venue requests from cloud. Since billing tables are venue-authoritative and should never be pulled from cloud, `SYNC_TABLES` does NOT need to change. The push path is independent and already includes billing data.

**Primary recommendation:** Add `billing_events` to `collect_push_payload()`, add the corresponding `billing_events` upsert handler to the `sync_push` route, verify the read-only constraint is enforced (no cloud-to-venue pull for billing tables), and confirm retry-on-reconnect behavior with the existing relay/HTTP fallback mechanism.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| SYNC-01 | Completed billing_sessions pushed to cloud within 60s | Already implemented in `collect_push_payload()` lines 277-304. Uses `WHERE created_at > ? OR ended_at > ?` with `_push` timestamp. Relay mode pushes every 2s, HTTP fallback every 30s. Already meets 60s SLA. |
| SYNC-02 | wallet_transactions pushed to cloud within 60s | Already implemented in `collect_push_payload()` lines 384-403. Uses `WHERE created_at >= ?` with LIMIT 500. Same 2s/30s cycle as above. Already meets 60s SLA. |
| SYNC-03 | billing_events pushed to cloud within 60s | **NOT implemented.** `billing_events` is not collected in `collect_push_payload()` and has no upsert handler in `sync_push`. Must be added. |
| SYNC-04 | Cloud copy is read-only -- no cloud-to-venue push for billing tables | Already enforced. `SYNC_TABLES` (the pull list) does not include billing_sessions, wallet_transactions, or billing_events. `sync_once_http()` only pulls tables in that list. Relay mode only pushes outbound. However, there is no explicit guard preventing future additions -- should be documented. |
| SYNC-05 | Sync survives network outage -- queues and retries on reconnect | Partially met by existing design. The `_push` timestamp watermark means any data created during an outage will be picked up on the next successful cycle. However, there is no explicit outage queue -- it relies on timestamp-based catch-up. The LIMIT 500 per table per cycle could cause backlog during prolonged outages (>500 rows). |
</phase_requirements>

## Standard Stack

### Core (already in use -- do NOT add new dependencies)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| axum | 0.7.x | HTTP server + routes | Already powers racecontrol API |
| sqlx | 0.7.x | SQLite async queries | Already used for all DB ops |
| reqwest | 0.12.x | HTTP client for sync push | Already used in cloud_sync.rs |
| serde_json | 1.x | JSON payload construction | Already used throughout |
| tokio | 1.x | Async runtime | Already used |
| chrono | 0.4.x | UTC timestamps | Already used for sync timestamps |
| uuid | 1.x | UUID generation for billing_events | Already used |

### No new dependencies needed
This phase is purely additive to existing code. No new crates, no new infrastructure.

## Architecture Patterns

### Existing Sync Architecture (DO NOT CHANGE)
```
Venue racecontrol (SQLite)
    |
    |-- collect_push_payload() -- gathers changed rows since _push timestamp
    |
    |-- [Relay available?]
    |       YES -> POST localhost:876X/relay/sync (2s cycle)
    |       NO  -> POST cloud:8080/sync/push (30s cycle)
    |
    |-- Cloud racecontrol receives via sync_push() handler
    |       - Upserts billing_sessions (ON CONFLICT UPDATE status, driving_seconds, ended_at)
    |       - INSERT OR IGNORE wallet_transactions (immutable, UUID idempotent)
    |       - Upserts pods, wallets, drivers, laps, track_records, personal_bests
    |
    |-- update_push_state() records current UTC as _push watermark
```

### Anti-loop Protection (DO NOT MODIFY)
The `_push` timestamp in `sync_state` table prevents re-pushing received data:
1. After successful push, `update_push_state()` records current time.
2. Next `collect_push_payload()` queries `WHERE created_at > last_push`.
3. Received data (from `/sync/push` handler) does NOT update `_push` -- only outbound pushes do.
4. So received data's timestamps fall before `_push` and are never re-collected.

### Pattern: Adding a New Table to Push
Follow the exact pattern used for `wallet_transactions` (lines 384-403 of cloud_sync.rs):
```rust
// In collect_push_payload():
let billing_events = sqlx::query_as::<_, (String,)>(
    "SELECT json_object(
        'id', id, 'billing_session_id', billing_session_id,
        'event_type', event_type, 'driving_seconds_at_event', driving_seconds_at_event,
        'metadata', metadata, 'created_at', created_at
    ) FROM billing_events WHERE created_at >= ? ORDER BY created_at ASC LIMIT 500",
)
.bind(&last_push)
.fetch_all(&state.db)
.await?;

if !billing_events.is_empty() {
    let items: Vec<serde_json::Value> = billing_events.iter()
        .filter_map(|r| serde_json::from_str(&r.0).ok())
        .collect();
    tracing::info!("Cloud sync push: {} billing events", items.len());
    payload["billing_events"] = serde_json::json!(items);
    has_data = true;
}
```

### Pattern: Adding a New Table to sync_push Receiver
Follow the `wallet_transactions` pattern (lines 6460-6525 of routes.rs):
```rust
// In sync_push():
if let Some(events) = body.get("billing_events").and_then(|v| v.as_array()) {
    for ev in events {
        let id = ev.get("id").and_then(|v| v.as_str()).unwrap_or_default();
        if id.is_empty() { continue; }
        let r = sqlx::query(
            "INSERT OR IGNORE INTO billing_events
                (id, billing_session_id, event_type, driving_seconds_at_event, metadata, created_at)
             VALUES (?1,?2,?3,?4,?5,?6)",
        )
        .bind(id)
        .bind(ev.get("billing_session_id").and_then(|v| v.as_str()))
        .bind(ev.get("event_type").and_then(|v| v.as_str()).unwrap_or("unknown"))
        .bind(ev.get("driving_seconds_at_event").and_then(|v| v.as_i64()).unwrap_or(0))
        .bind(ev.get("metadata").and_then(|v| v.as_str()))
        .bind(ev.get("created_at").and_then(|v| v.as_str()))
        .execute(&state.db)
        .await;
        if r.is_ok() { total += 1; }
    }
    tracing::info!("Sync push: {} billing events", events.len());
}
```

### Anti-Patterns to Avoid
- **Do NOT add billing tables to SYNC_TABLES.** That constant controls the pull path (cloud -> venue). Billing data must only flow venue -> cloud.
- **Do NOT add a separate sync trigger on session completion.** The existing 2s/30s push cycle already provides < 60s latency. Adding event-driven sync would duplicate complexity.
- **Do NOT use the `sync_log` table.** It exists in the schema but is not used by the current sync mechanism. The `_push` timestamp watermark approach is simpler and already proven.
- **Do NOT modify `update_push_state()`.** It is shared between relay and HTTP paths. Changing its behavior could break anti-loop protection.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Outage queue | Custom WAL-based queue | Existing `_push` timestamp watermark | Timestamp catch-up already handles outage recovery -- rows created during downtime have timestamps > last successful `_push` |
| Retry logic | Custom exponential backoff | Existing relay/HTTP fallback loop | The `spawn()` loop already retries every 2s (relay) or 30s (HTTP) indefinitely |
| Deduplication | Custom dedup logic | `INSERT OR IGNORE` with UUID PK | billing_events and wallet_transactions use UUID primary keys -- INSERT OR IGNORE is idempotent |
| Read-only enforcement | Custom middleware | Omission from SYNC_TABLES + no upsert_billing_* functions in HTTP pull path | Simply never adding billing tables to the pull path is sufficient |

**Key insight:** The existing cloud_sync.rs infrastructure is production-proven. This phase is a ~50-line code change, not an architecture change.

## Common Pitfalls

### Pitfall 1: LIMIT 500 Backlog During Long Outages
**What goes wrong:** If the venue is offline for hours and hundreds of sessions complete, the 500-row LIMIT per cycle means it takes multiple cycles to catch up.
**Why it happens:** `collect_push_payload()` uses `LIMIT 500` on each table query.
**How to avoid:** This is acceptable for normal operations (venue rarely has >500 billing events between cycles). For Phase 1, document this as a known limitation. If needed later, increase the limit or add a catch-up mode.
**Warning signs:** Sync health endpoint shows increasing lag between `_push` timestamp and current time.

### Pitfall 2: Timestamp Format Mismatch
**What goes wrong:** SQLite uses `datetime('now')` format ("2026-03-14 12:00:00") but ISO timestamps use 'T' separator and timezone suffix ("2026-03-14T12:00:00+00:00"). String comparison breaks.
**Why it happens:** `normalize_timestamp()` exists to handle this, but only applies to the `_push` watermark. The `WHERE created_at >= ?` comparison in push queries uses the normalized timestamp against SQLite-formatted `created_at` values.
**How to avoid:** The existing code already handles this correctly via `normalize_timestamp()` on `last_push`. New billing_events query should use the same `last_push` variable that is already normalized.
**Warning signs:** Rows never appear in push payload despite existing in the DB.

### Pitfall 3: Health Endpoint Column Name Mismatch
**What goes wrong:** The ops health endpoint at line 9512 of routes.rs queries `billing_events` with `SELECT id, session_id, event_type...` but the actual table column is `billing_session_id`, not `session_id`. This is an existing bug.
**Why it happens:** Likely a typo from when the schema was first written.
**How to avoid:** When adding billing_events to sync, use the correct column name `billing_session_id`. Do NOT fix the health endpoint bug in this phase -- it is unrelated scope.
**Warning signs:** The ops health endpoint may already be erroring silently on this query.

### Pitfall 4: billing_events Uses `>=` vs `>` for Timestamp Comparison
**What goes wrong:** Using `>` might miss rows created in the same second as the last push. Using `>=` might re-push the same rows.
**Why it happens:** SQLite timestamp precision is seconds, not milliseconds.
**How to avoid:** Use `>=` (like wallet_transactions does at line 391) combined with `INSERT OR IGNORE` on the receiver. Re-pushing is harmless; missing rows is not.
**Warning signs:** Intermittent missing billing events on cloud.

### Pitfall 5: No Index on billing_events.created_at
**What goes wrong:** The `WHERE created_at >= ?` query does a full table scan as the table grows.
**Why it happens:** The existing indexes are on `billing_session_id` (line 439) but not on `created_at`.
**How to avoid:** Add `CREATE INDEX IF NOT EXISTS idx_billing_events_created ON billing_events(created_at)` in the migration function. This is a one-line addition.
**Warning signs:** Sync push latency increases over weeks as billing_events table grows.

## Code Examples

### Current billing_sessions Push (already working -- cloud_sync.rs lines 277-304)
```rust
// Source: crates/racecontrol/src/cloud_sync.rs lines 277-304
let sessions = sqlx::query_as::<_, (String,)>(
    "SELECT json_object(
        'id', id, 'driver_id', driver_id, 'pod_id', pod_id,
        'pricing_tier_id', pricing_tier_id, 'allocated_seconds', allocated_seconds,
        'driving_seconds', driving_seconds, 'status', status,
        'custom_price_paise', custom_price_paise, 'notes', notes,
        'started_at', started_at, 'ended_at', ended_at, 'created_at', created_at,
        'experience_id', experience_id, 'car', car, 'track', track, 'sim_type', sim_type,
        'split_count', split_count, 'split_duration_minutes', split_duration_minutes,
        'wallet_debit_paise', wallet_debit_paise,
        'discount_paise', discount_paise, 'coupon_id', coupon_id,
        'original_price_paise', original_price_paise, 'discount_reason', discount_reason
    ) FROM billing_sessions WHERE created_at > ? OR ended_at > ?
    ORDER BY created_at ASC LIMIT 500",
)
.bind(&last_push)
.bind(&last_push)
.fetch_all(&state.db)
.await?;
```

Note: billing_sessions uses `created_at > ? OR ended_at > ?` -- this catches both new sessions AND sessions that completed since last push. This is already correct for SYNC-01.

### Current wallet_transactions Push (already working -- cloud_sync.rs lines 384-403)
```rust
// Source: crates/racecontrol/src/cloud_sync.rs lines 384-403
let wallet_txns = sqlx::query_as::<_, (String,)>(
    "SELECT json_object(
        'id', id, 'driver_id', driver_id, 'amount_paise', amount_paise,
        'balance_after_paise', balance_after_paise, 'txn_type', txn_type,
        'reference_id', reference_id, 'notes', notes, 'staff_id', staff_id,
        'created_at', created_at
    ) FROM wallet_transactions WHERE created_at >= ? ORDER BY created_at ASC LIMIT 500",
)
.bind(&last_push)
.fetch_all(&state.db)
.await?;
```

Note: Uses `>=` (not `>`) -- intentional to avoid missing same-second rows. Idempotent via INSERT OR IGNORE on the receiver.

### billing_events Table Schema (db/mod.rs lines 252-261)
```sql
CREATE TABLE IF NOT EXISTS billing_events (
    id TEXT PRIMARY KEY,
    billing_session_id TEXT NOT NULL REFERENCES billing_sessions(id),
    event_type TEXT NOT NULL,
    driving_seconds_at_event INTEGER NOT NULL DEFAULT 0,
    metadata TEXT,
    created_at TEXT DEFAULT (datetime('now'))
)
```

Event types observed in billing.rs: `created`, `started`, `time_expired`, `ended_early`, `cancelled`, `ended`, `paused_disconnect`, `pause_timeout_ended`, `resumed_disconnect`, `extended`, plus status transition events (paused_manual, resumed, etc.).

### billing_sessions New Columns (added via ALTER TABLE migrations)
```sql
-- These columns exist but may not appear in the original CREATE TABLE:
experience_id TEXT
car TEXT
track TEXT
sim_type TEXT
reservation_id TEXT
wallet_debit_paise INTEGER
wallet_txn_id TEXT
staff_id TEXT
split_count INTEGER DEFAULT 1
split_duration_minutes INTEGER
discount_paise INTEGER DEFAULT 0
coupon_id TEXT
original_price_paise INTEGER
discount_reason TEXT
pause_count INTEGER DEFAULT 0
total_paused_seconds INTEGER DEFAULT 0
last_paused_at TEXT
refund_paise INTEGER DEFAULT 0
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| HTTP-only sync (30s) | Relay + HTTP fallback (2s / 30s) | Comms-link launch | 15x faster sync when relay available |
| No billing sync | billing_sessions + wallet_transactions push | Already exists | SYNC-01 and SYNC-02 already met |
| sync_log CDC table | _push timestamp watermark | Current design | Simpler, no CDC overhead |

**Existing but unused:**
- `sync_log` table: CDC-style change log exists in schema (db/mod.rs lines 402-415) with `synced` flag and indexes, but is never written to or read from in the codebase. The timestamp watermark approach replaced it. Do NOT attempt to use sync_log.

## Open Questions

1. **billing_sessions pause/refund columns in push payload**
   - What we know: The `collect_push_payload()` query for billing_sessions (line 277) includes `wallet_debit_paise`, `discount_paise`, `coupon_id`, `original_price_paise`, `discount_reason` but does NOT include `pause_count`, `total_paused_seconds`, `last_paused_at`, or `refund_paise`.
   - What's unclear: Are these columns needed on cloud for Phase 2/3 dashboard display?
   - Recommendation: Add `pause_count`, `total_paused_seconds`, and `refund_paise` to the push payload now. They exist in the DB and will be needed for analytics (Phase 3). Cost is 3 extra JSON fields per row -- negligible.

2. **Driver ID mismatch for billing_sessions on cloud**
   - What we know: Wallet sync resolves ID mismatches via phone/email lookup. Billing sessions push uses venue driver_id directly. If cloud has a different UUID for the same customer, the billing_session row will reference a non-existent driver_id on cloud.
   - What's unclear: Is this a problem for Phase 2/3 dashboards?
   - Recommendation: For Phase 1, push as-is (venue driver_id). Cloud dashboard (Phase 2) should join on driver_id that the venue pushes, since the venue also pushes driver records with those IDs. The sync already handles driver push (lines 307-332), so the driver should exist on cloud by the time the billing session arrives.

3. **billing_events column name discrepancy in health endpoint**
   - What we know: routes.rs line 9513 queries `session_id` but the column is `billing_session_id`. This is an existing bug unrelated to sync.
   - Recommendation: Flag for fix but out of Phase 1 scope.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in test (#[cfg(test)]) with cargo test |
| Config file | Cargo.toml per crate |
| Quick run command | `cargo test -p racecontrol-crate -- cloud_sync` |
| Full suite command | `cargo test -p rc-common && cargo test -p racecontrol-crate` |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| SYNC-01 | billing_sessions rows appear in push payload when status is completed/ended_early | unit | `cargo test -p racecontrol-crate -- cloud_sync::tests::test_billing_sessions_in_payload -x` | No -- Wave 0 |
| SYNC-02 | wallet_transactions rows appear in push payload after creation | unit | `cargo test -p racecontrol-crate -- cloud_sync::tests::test_wallet_txns_in_payload -x` | No -- Wave 0 |
| SYNC-03 | billing_events rows appear in push payload after session end | unit | `cargo test -p racecontrol-crate -- cloud_sync::tests::test_billing_events_in_payload -x` | No -- Wave 0 |
| SYNC-04 | billing tables not in SYNC_TABLES (pull path) | unit | `cargo test -p racecontrol-crate -- cloud_sync::tests::test_billing_tables_not_in_pull -x` | No -- Wave 0 |
| SYNC-05 | Rows created during simulated outage appear in next push cycle | unit | `cargo test -p racecontrol-crate -- cloud_sync::tests::test_outage_catchup -x` | No -- Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p racecontrol-crate -- cloud_sync`
- **Per wave merge:** `cargo test -p rc-common && cargo test -p racecontrol-crate`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/racecontrol/src/cloud_sync.rs` -- add `#[cfg(test)] mod tests` section with unit tests for payload collection
- [ ] Test helper: in-memory SQLite pool with billing schema for isolated testing
- [ ] Note: `collect_push_payload` requires `Arc<AppState>` -- tests need mock state with SQLite pool

## Sources

### Primary (HIGH confidence)
- `crates/racecontrol/src/cloud_sync.rs` -- read in full (1004 lines). Contains all sync logic: relay, HTTP fallback, push payload collection, anti-loop protection.
- `crates/racecontrol/src/db/mod.rs` -- read billing table schemas (billing_sessions, billing_events, wallet_transactions, wallets, sync_state).
- `crates/racecontrol/src/api/routes.rs` -- read sync_push handler (lines 6120-6529). Contains cloud-side upsert logic for all pushed tables.
- `crates/racecontrol/src/billing.rs` -- read session lifecycle: end_billing_session(), post_session_hooks(), billing tick loop. Confirmed billing_events written at every lifecycle event.

### Secondary (MEDIUM confidence)
- `crates/racecontrol/src/config.rs` -- CloudConfig struct with sync_interval_secs (default 30), comms_link_url, terminal_secret, api_url.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all libraries already in use, no new dependencies
- Architecture: HIGH -- existing sync architecture is well-documented in code comments and already handles billing_sessions and wallet_transactions
- Pitfalls: HIGH -- identified from direct code reading, especially timestamp handling and LIMIT 500 behavior
- Scope of work: HIGH -- this is a ~50-line change, not an architecture change

**Research date:** 2026-03-14
**Valid until:** 2026-04-14 (stable codebase, no expected upstream changes)
