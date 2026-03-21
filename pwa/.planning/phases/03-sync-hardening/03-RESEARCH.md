# Phase 3: Sync Hardening - Research

**Researched:** 2026-03-21
**Domain:** Cloud-local SQLite sync layer (Rust/Axum, bidirectional, HMAC-signed)
**Confidence:** HIGH

## Summary

Phase 3 hardens the existing `cloud_sync.rs` bidirectional sync system for production correctness. The sync layer already works (dual-mode relay/HTTP, HMAC signing, timestamp-based delta detection, authority rules), but it needs three critical additions before remote booking goes live: (1) a reservations table synced cloud-to-local, (2) a debit intent pattern so wallet debits for bookings don't cause financial inconsistencies, and (3) origin tags to definitively prevent sync loops beyond the current timestamp-based approach.

The existing codebase is well-structured for these additions. `SYNC_TABLES` is a comma-separated const that controls which tables are pulled from cloud. The `collect_push_payload()` function collects venue-to-cloud data. The `sync_push` route handler in `routes.rs` processes cloud-to-venue pushes. Adding a new table follows the same pattern as existing tables (drivers, wallets, pricing_tiers, etc.) -- add to the const, write an upsert function, add to the push/pull collectors.

**Primary recommendation:** Implement changes in the Rust `cloud_sync.rs` and `routes.rs` files on both cloud and local racecontrol instances, following the existing patterns exactly. No new libraries or architectural changes needed -- this is all internal Rust code.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| SYNC-01 | Reservations table added to cloud_sync (cloud-authoritative) | New `reservations` table + migration, add to SYNC_TABLES, write upsert_reservation(), add to sync_changes and sync_push handlers |
| SYNC-02 | Wallet uses debit intent pattern -- cloud sends debit request, local processes and syncs balance back | New `debit_intents` table, cloud creates intent on booking, local processes intent and updates wallet, syncs result back |
| SYNC-03 | Origin tags added to sync payloads to prevent sync loops | Add `"origin": "cloud"/"local"` field to push payloads, receiving side skips rows matching own origin |
| SYNC-04 | Cloud shows "booking pending confirmation" when sync lag exceeds 60 seconds | Cloud-side sync_health endpoint already exists; PWA reads lag from `/sync/status`, shows pending UI when lag > 60s |
| SYNC-06 | All admin-managed tables sync correctly cloud-to-local | Already working for pricing_tiers, pricing_rules, kiosk_experiences, kiosk_settings, billing_rates. Verify and test. |
| SYNC-07 | Sync health endpoint exposed at api.racingpoint.cloud/sync/status | Endpoint already exists as `sync_health()` in routes.rs. Needs enhancement: add last_push timestamp, computed lag_seconds, per-table staleness |
</phase_requirements>

## Standard Stack

### Core (no new dependencies)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| sqlx | 0.8.x | SQLite async queries | Already used throughout; all sync queries use sqlx |
| serde_json | 1.x | JSON payload construction/parsing | Already used for all sync payloads |
| chrono | 0.4.x | Timestamp handling | Already used for sync timestamps |
| uuid | 1.x | ID generation for new records | Already used for driver/session IDs |

### Supporting (already in workspace)

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| hmac + sha2 | 0.12.x + 0.10.x | HMAC-SHA256 sync signing | Already integrated (AUTH-07) |
| reqwest | 0.12.x | HTTP client for sync | Already used by cloud_sync.rs |
| tracing | 0.1.x | Structured logging | Already used throughout |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Custom origin tags | CRDT libraries | Overkill -- system has clear write-authority per table, not multi-writer |
| Debit intent table | Direct wallet mutation | UNSAFE -- race conditions between cloud top-up and local debit |
| Timestamp-based deltas | WAL-based CDC | More complex, existing approach works with origin tags added |

**Installation:** No new packages needed. All dependencies already in Cargo.toml.

## Architecture Patterns

### Recommended Changes Structure
```
crates/racecontrol/src/
  cloud_sync.rs        # Add reservations + debit_intents to push/pull
  db/mod.rs            # Add reservations + debit_intents table migrations
  api/routes.rs        # Add reservation upsert in sync_push, enhance sync_health
```

### Pattern 1: Adding a New Sync Table (Reservations)
**What:** Follow the exact same pattern used for `auth_tokens` (cloud-authoritative, synced cloud-to-local).
**When to use:** For any new cloud-authoritative data that needs to reach the venue.

Steps:
1. Add migration in `db/mod.rs` (CREATE TABLE IF NOT EXISTS)
2. Add table name to `SYNC_TABLES` const in `cloud_sync.rs`
3. Add to `sync_changes` query in `routes.rs` (the GET /sync/changes handler)
4. Write `upsert_reservation()` function in `cloud_sync.rs`
5. Add to `sync_once_http()` pull handler to call upsert
6. Add to `sync_push` route handler to accept incoming pushes
7. Add to `collect_push_payload()` to push local changes back (status updates)

**Example (from existing auth_tokens pattern):**
```rust
// In SYNC_TABLES const:
const SYNC_TABLES: &str = "drivers,wallets,...,auth_tokens,reservations";

// In sync_changes (routes.rs), add a new match arm:
"reservations" => {
    let rows = sqlx::query_as::<_, (String,)>(
        "SELECT json_object(
            'id', id, 'driver_id', driver_id, 'experience_id', experience_id,
            'pin', pin, 'status', status, 'created_at', created_at,
            'expires_at', expires_at, 'redeemed_at', redeemed_at,
            'pod_number', pod_number, 'updated_at', updated_at
        ) FROM reservations
        WHERE updated_at > ? OR (updated_at IS NULL AND created_at > ?)
        ORDER BY COALESCE(updated_at, created_at) ASC
        LIMIT ?",
    )
    .bind(&since).bind(&since).bind(limit)
    .fetch_all(&state.db).await;
    // ... same pattern as drivers
}
```

### Pattern 2: Debit Intent (Financial Safety)
**What:** Cloud creates a `debit_intent` record instead of directly mutating wallet balance. Local server processes the intent, debits the wallet, and syncs the result back.
**When to use:** Any time the cloud needs to charge a customer's wallet.

```
Cloud side:
  1. Customer books -> create reservation (status: pending_debit)
  2. Create debit_intent: { id, driver_id, amount_paise, reservation_id, status: pending }
  3. Sync debit_intent to local via cloud_sync push

Local side:
  1. Receive debit_intent via sync pull
  2. Check wallet balance >= amount_paise
  3. If sufficient: debit wallet, create wallet_transaction, set intent status: completed
  4. If insufficient: set intent status: failed, reason: insufficient_balance
  5. Update reservation status based on intent result
  6. Sync intent status + wallet balance back to cloud

Cloud side (on next sync):
  1. Receive updated intent status
  2. If completed: show "booking confirmed"
  3. If failed: show "insufficient balance", offer top-up
```

### Pattern 3: Origin Tags (Anti-Loop)
**What:** Add `"origin": "cloud"` or `"origin": "local"` to every sync payload. Receiving side skips rows that originated from itself.
**When to use:** Every sync push/pull cycle.

```rust
// In collect_push_payload():
let mut payload = serde_json::json!({
    "schema_version": SCHEMA_VERSION,
    "origin": "local"  // or "cloud" on cloud instance
});

// In sync_push handler (routes.rs):
let origin = body.get("origin").and_then(|v| v.as_str()).unwrap_or("unknown");
// Skip processing if origin matches our own identity
if origin == "local" && is_local_instance {
    // This data came from us -- skip to prevent loop
    return Json(json!({ "ok": true, "upserted": 0, "skipped": "same_origin" }));
}
```

**Important nuance:** The current system already has anti-loop protection via `_push` timestamp tracking (see cloud_sync.rs lines 242-253). Origin tags add a second layer of defense. Both mechanisms should coexist -- origin tags prevent the loop, timestamps prevent re-pushing stale data.

### Pattern 4: Config-Driven Origin Identity
**What:** Add `origin_id` field to `CloudConfig` so each instance knows its identity.
**When to use:** At startup, configured in racecontrol.toml.

```toml
[cloud]
enabled = true
origin_id = "local"  # or "cloud" on the VPS instance
```

### Anti-Patterns to Avoid
- **Direct wallet mutation from cloud:** NEVER update wallet balance directly on cloud for booking debits. Always use debit intent pattern via sync.
- **Sync loop "fixes" by adding delays:** Adding sleep/cooldown periods masks the root cause. Use origin tags instead.
- **Skipping updated_at checks:** The existing MAX(updated_at) CRDT merge is correct. Don't bypass it for "simplicity".
- **Making reservations local-authoritative:** Reservations are created on cloud (remote booking). Cloud must be authoritative. Local only updates status (pending -> redeemed).

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Sync loop prevention | Custom loop detector with counters/timers | Origin tags + existing timestamp-based delta | Simple, deterministic, no edge cases |
| Wallet debit from cloud | Direct wallet UPDATE on cloud DB | Debit intent pattern (intent table + local processing) | Race conditions between top-up and debit would cause financial errors |
| Sync health monitoring | Custom heartbeat system | Enhance existing sync_health() endpoint | Already has relay status, sync_state table, just needs lag calculation |
| Reservation ID generation | Custom PIN generator | `uuid::Uuid::new_v4()` for ID, separate 6-char PIN generation | Standard pattern already used for all IDs in the system |

**Key insight:** The sync layer is already 90% correct. The remaining 10% is adding the reservations table (following existing patterns), making wallet debits safe (debit intents), and adding origin tags as a second anti-loop defense layer.

## Common Pitfalls

### Pitfall 1: Wallet Double-Charge on Booking
**What goes wrong:** Cloud debits wallet for booking. Local debits wallet for session start. Both sync their version -- customer loses money.
**Why it happens:** Wallet has two writers (cloud for top-up, local for billing) and last-write-wins causes data loss.
**How to avoid:** Debit intent pattern. Cloud never directly modifies wallet balance. Cloud creates a debit_intent, local processes it, local is the single writer for wallet debits.
**Warning signs:** Wallet balance_paise on cloud != wallet balance_paise on local after a booking+session cycle.

### Pitfall 2: Sync Loop After Adding Reservations
**What goes wrong:** Cloud pushes reservation to local. Local's updated_at changes on upsert. Local pushes reservation back to cloud. Cloud upserts (updating its updated_at). Infinite loop.
**Why it happens:** Both sides use `WHERE updated_at > last_push` to detect changes. Upserting received data updates the local updated_at.
**How to avoid:** Origin tags filter out data that originated from the receiving side. Additionally, the existing `_push` timestamp mechanism already prevents this in most cases (see cloud_sync.rs anti-loop comment block).
**Warning signs:** Sync logs showing the same reservation ID being pushed repeatedly every cycle.

### Pitfall 3: Reservation Synced But Intent Not Yet Processed
**What goes wrong:** Reservation syncs to local before the debit_intent. Customer arrives, tries to redeem PIN, but wallet hasn't been debited yet. System either blocks redemption (bad UX) or allows unpaid session (financial loss).
**How to avoid:** Reservation status should be `pending_debit` until the debit_intent is processed. PIN redemption requires reservation.status == `confirmed`. Local processes debit_intent -> updates reservation status to `confirmed` -> then PIN is redeemable.
**Warning signs:** Reservation exists on local but status is still `pending_debit` and no matching debit_intent exists.

### Pitfall 4: Clock Drift Between Cloud and Local
**What goes wrong:** Cloud and local servers have different system clocks. Timestamp comparisons in sync break (stale data wins, or new data gets skipped).
**Why it happens:** VPS runs on Hetzner (UTC), local server runs on Windows (IST). SQLite `datetime('now')` uses system clock.
**How to avoid:** All timestamps stored as UTC in SQLite (already the case). Use `chrono::Utc::now()` not `Local::now()` for sync timestamps. The 5-minute HMAC replay window already tolerates moderate drift.
**Warning signs:** sync_state.last_synced_at is in the future relative to actual time.

### Pitfall 5: Stale sync_health Endpoint
**What goes wrong:** The sync_health endpoint reports "ok" but sync hasn't actually run in minutes. Dashboard shows green but data is stale.
**Why it happens:** sync_health only reads from sync_state table. If the sync loop is stuck/crashed, the table isn't updated but the endpoint doesn't know.
**How to avoid:** Add `last_push_at` and `last_pull_at` timestamps to sync_health response. Compute `lag_seconds` as `now - max(last_push_at, last_pull_at)`. Return `"status": "degraded"` when lag > 60s.
**Warning signs:** sync_state.updated_at is more than 60 seconds old but sync_health still returns "ok".

## Code Examples

### Reservations Table Migration (db/mod.rs)
```rust
// Source: follows existing pattern from auth_tokens migration in db/mod.rs
sqlx::query(
    "CREATE TABLE IF NOT EXISTS reservations (
        id TEXT PRIMARY KEY,
        driver_id TEXT NOT NULL REFERENCES drivers(id),
        experience_id TEXT NOT NULL,
        pin TEXT NOT NULL,
        status TEXT NOT NULL DEFAULT 'pending_debit'
            CHECK(status IN ('pending_debit','confirmed','redeemed','expired','cancelled','failed')),
        pod_number INTEGER,
        debit_intent_id TEXT,
        created_at TEXT DEFAULT (datetime('now')),
        expires_at TEXT NOT NULL,
        redeemed_at TEXT,
        cancelled_at TEXT,
        updated_at TEXT DEFAULT (datetime('now'))
    )",
)
.execute(pool)
.await?;

sqlx::query("CREATE INDEX IF NOT EXISTS idx_reservations_pin ON reservations(pin, status)")
    .execute(pool).await?;
sqlx::query("CREATE INDEX IF NOT EXISTS idx_reservations_driver ON reservations(driver_id, status)")
    .execute(pool).await?;
sqlx::query("CREATE INDEX IF NOT EXISTS idx_reservations_expires ON reservations(expires_at, status)")
    .execute(pool).await?;
```

### Debit Intent Table Migration (db/mod.rs)
```rust
sqlx::query(
    "CREATE TABLE IF NOT EXISTS debit_intents (
        id TEXT PRIMARY KEY,
        driver_id TEXT NOT NULL REFERENCES drivers(id),
        amount_paise INTEGER NOT NULL,
        reservation_id TEXT NOT NULL,
        status TEXT NOT NULL DEFAULT 'pending'
            CHECK(status IN ('pending','processing','completed','failed','cancelled')),
        failure_reason TEXT,
        wallet_txn_id TEXT,
        origin TEXT NOT NULL DEFAULT 'cloud',
        created_at TEXT DEFAULT (datetime('now')),
        processed_at TEXT,
        updated_at TEXT DEFAULT (datetime('now'))
    )",
)
.execute(pool)
.await?;

sqlx::query("CREATE INDEX IF NOT EXISTS idx_debit_intents_status ON debit_intents(status)")
    .execute(pool).await?;
sqlx::query("CREATE INDEX IF NOT EXISTS idx_debit_intents_reservation ON debit_intents(reservation_id)")
    .execute(pool).await?;
```

### Origin Tag in Push Payload (cloud_sync.rs)
```rust
// Source: modification to existing collect_push_payload()
async fn collect_push_payload(state: &Arc<AppState>) -> anyhow::Result<(Value, bool)> {
    let last_push = normalize_timestamp(&get_last_push_time(state).await);
    let origin = state.config.cloud.origin_id
        .as_deref()
        .unwrap_or("local");
    let mut payload = serde_json::json!({
        "schema_version": SCHEMA_VERSION,
        "origin": origin,
    });
    // ... rest of existing code
}
```

### Origin Filter in sync_push Handler (routes.rs)
```rust
// Source: modification to existing sync_push handler
async fn sync_push(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    body_bytes: axum::body::Bytes,
) -> Json<Value> {
    // ... existing auth checks ...

    let body: Value = match serde_json::from_slice(&body_bytes) {
        Ok(v) => v,
        Err(e) => return Json(json!({ "error": format!("Invalid JSON: {}", e) })),
    };

    // Origin tag check: skip if data originated from us
    let incoming_origin = body.get("origin").and_then(|v| v.as_str()).unwrap_or("unknown");
    let my_origin = state.config.cloud.origin_id.as_deref().unwrap_or("local");
    if incoming_origin == my_origin {
        tracing::warn!(target: "sync", "Rejecting sync_push from same origin: {}", my_origin);
        return Json(json!({ "ok": true, "upserted": 0, "reason": "same_origin" }));
    }

    // ... existing upsert logic ...
}
```

### Enhanced sync_health Endpoint (routes.rs)
```rust
// Source: enhancement to existing sync_health function
async fn sync_health(State(state): State<Arc<AppState>>) -> Json<Value> {
    // ... existing code ...

    // Compute lag: time since last successful sync activity
    let last_activity = sqlx::query_as::<_, (String,)>(
        "SELECT MAX(updated_at) FROM sync_state",
    )
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let lag_seconds = match last_activity {
        Some((ts,)) => {
            chrono::DateTime::parse_from_rfc3339(&ts)
                .or_else(|_| chrono::NaiveDateTime::parse_from_str(&ts, "%Y-%m-%d %H:%M:%S")
                    .map(|n| n.and_utc().fixed_offset()))
                .map(|dt| (chrono::Utc::now() - dt.with_timezone(&chrono::Utc)).num_seconds())
                .unwrap_or(-1)
        }
        None => -1,
    };

    let health_status = if lag_seconds < 0 {
        "unknown"
    } else if lag_seconds <= 60 {
        "healthy"
    } else if lag_seconds <= 300 {
        "degraded"
    } else {
        "critical"
    };

    Json(json!({
        "status": health_status,
        "lag_seconds": lag_seconds,
        // ... existing fields ...
    }))
}
```

### Debit Intent Processing (local side, new function)
```rust
/// Process pending debit intents received from cloud.
/// Called during sync pull cycle on the local server.
async fn process_debit_intents(state: &Arc<AppState>) -> anyhow::Result<u64> {
    let pending = sqlx::query_as::<_, (String, String, i64, String)>(
        "SELECT id, driver_id, amount_paise, reservation_id
         FROM debit_intents WHERE status = 'pending' ORDER BY created_at ASC",
    )
    .fetch_all(&state.db)
    .await?;

    let mut processed = 0u64;
    for (intent_id, driver_id, amount, reservation_id) in &pending {
        // Check wallet balance
        let balance = sqlx::query_as::<_, (i64,)>(
            "SELECT balance_paise FROM wallets WHERE driver_id = ?",
        )
        .bind(driver_id)
        .fetch_optional(&state.db)
        .await?;

        match balance {
            Some((bal,)) if bal >= *amount => {
                // Debit wallet
                let new_balance = bal - amount;
                let txn_id = uuid::Uuid::new_v4().to_string();

                sqlx::query(
                    "UPDATE wallets SET balance_paise = ?, total_debited_paise = total_debited_paise + ?,
                     updated_at = datetime('now') WHERE driver_id = ?",
                )
                .bind(new_balance).bind(amount).bind(driver_id)
                .execute(&state.db).await?;

                // Record wallet transaction
                sqlx::query(
                    "INSERT INTO wallet_transactions (id, driver_id, amount_paise, balance_after_paise,
                     txn_type, reference_id, notes, created_at)
                     VALUES (?, ?, ?, ?, 'debit_session', ?, 'Remote booking debit', datetime('now'))",
                )
                .bind(&txn_id).bind(driver_id).bind(-amount).bind(new_balance).bind(reservation_id)
                .execute(&state.db).await?;

                // Mark intent as completed
                sqlx::query(
                    "UPDATE debit_intents SET status = 'completed', wallet_txn_id = ?,
                     processed_at = datetime('now'), updated_at = datetime('now') WHERE id = ?",
                )
                .bind(&txn_id).bind(intent_id)
                .execute(&state.db).await?;

                // Update reservation status to confirmed
                sqlx::query(
                    "UPDATE reservations SET status = 'confirmed', updated_at = datetime('now')
                     WHERE id = ?",
                )
                .bind(reservation_id)
                .execute(&state.db).await?;

                processed += 1;
            }
            _ => {
                // Insufficient balance
                sqlx::query(
                    "UPDATE debit_intents SET status = 'failed', failure_reason = 'insufficient_balance',
                     processed_at = datetime('now'), updated_at = datetime('now') WHERE id = ?",
                )
                .bind(intent_id)
                .execute(&state.db).await?;

                sqlx::query(
                    "UPDATE reservations SET status = 'failed', updated_at = datetime('now')
                     WHERE id = ?",
                )
                .bind(reservation_id)
                .execute(&state.db).await?;
            }
        }
    }

    Ok(processed)
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Timestamp-only anti-loop | Timestamp + origin tags | Phase 3 | Eliminates edge-case loops under clock drift |
| Direct wallet mutation | Debit intent pattern | Phase 3 | Prevents double-charge on remote booking |
| Sync health = table count | Sync health + lag + status | Phase 3 | Dashboard can show "pending confirmation" when sync is slow |
| cloud_sync only pulls existing tables | Reservations + debit_intents added | Phase 3 | Enables remote booking flow in Phase 4 |

**Deprecated/outdated:**
- `pod_reservations` table (existing in db/mod.rs) is for local pod-driver binding during active sessions, NOT for remote booking reservations. The new `reservations` table is separate.

## Open Questions

1. **Config origin_id: how to set on both sides?**
   - What we know: racecontrol.toml on local server has `[cloud]` section. VPS also runs racecontrol with its own toml.
   - What's unclear: Does the VPS racecontrol.toml already exist and get managed separately?
   - Recommendation: Add `origin_id = "local"` to local toml, `origin_id = "cloud"` to VPS toml. Both sides must deploy this config change.

2. **SYNC-04: Frontend or backend responsibility for "pending confirmation"?**
   - What we know: sync_health endpoint exists, can return lag_seconds. PWA is a Next.js app.
   - What's unclear: Should the PWA poll sync/status directly, or should the booking API return a status field?
   - Recommendation: Both. Booking API returns reservation.status which includes `pending_debit`. PWA also checks sync lag for a general "sync delayed" banner. This is primarily a Phase 4 PWA concern but the backend endpoint must be ready in Phase 3.

3. **Schema version bump needed?**
   - What we know: `SCHEMA_VERSION` is currently 2. Adding new tables to sync payload changes the schema.
   - What's unclear: Does the cloud side reject pushes with unknown schema versions?
   - Recommendation: Bump to 3 when adding reservations + debit_intents. Both sides must be deployed together (or cloud accepts schema >= 2).

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (built-in Rust test framework) |
| Config file | Cargo.toml workspace test config |
| Quick run command | `cargo test -p racecontrol --lib cloud_sync` |
| Full suite command | `cargo test -p rc-common && cargo test -p racecontrol` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| SYNC-01 | Reservation upserted on sync pull | unit | `cargo test -p racecontrol -- upsert_reservation` | No - Wave 0 |
| SYNC-02 | Debit intent processed correctly | unit | `cargo test -p racecontrol -- process_debit_intent` | No - Wave 0 |
| SYNC-02 | Insufficient balance fails intent | unit | `cargo test -p racecontrol -- debit_intent_insufficient` | No - Wave 0 |
| SYNC-03 | Same-origin payload rejected | unit | `cargo test -p racecontrol -- origin_tag_reject` | No - Wave 0 |
| SYNC-04 | Sync health returns lag_seconds | unit | `cargo test -p racecontrol -- sync_health_lag` | No - Wave 0 |
| SYNC-06 | Admin tables sync correctly | integration | `cargo test -p racecontrol -- sync_admin_tables` | No - Wave 0 |
| SYNC-07 | Sync health endpoint returns expected fields | unit | `cargo test -p racecontrol -- sync_health_endpoint` | No - Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p racecontrol --lib`
- **Per wave merge:** `cargo test -p rc-common && cargo test -p racecontrol`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] Test helper: in-memory SQLite pool factory for sync unit tests
- [ ] `tests/sync_hardening.rs` -- covers SYNC-01 through SYNC-07
- [ ] Mock HTTP responses for cloud API calls during testing

## Sources

### Primary (HIGH confidence)
- `cloud_sync.rs` (lines 1-1095) -- complete existing sync implementation, anti-loop mechanism, HMAC signing, relay/HTTP dual-mode
- `db/mod.rs` (lines 1-870+) -- all table schemas, migrations, existing wallets/sync_state/auth_tokens tables
- `api/routes.rs` (lines 7300-8116) -- sync_changes, sync_push, sync_health endpoints
- `config.rs` (lines 83-109) -- CloudConfig structure with all sync-related fields
- `state.rs` -- AppState with relay_available AtomicBool, db pool, http_client
- `billing.rs` -- Wallet debit patterns, wallet_transactions table usage

### Secondary (MEDIUM confidence)
- `.planning/research/ARCHITECTURE.md` -- sync authority model (cloud vs local authoritative tables)
- `.planning/research/PITFALLS.md` -- wallet sync financial risk (#2), anti-loop fragility (#3)

### Tertiary (LOW confidence)
- None -- all findings based on direct codebase analysis

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- no new dependencies, all changes are in existing Rust files
- Architecture: HIGH -- follows exact patterns already established in cloud_sync.rs
- Pitfalls: HIGH -- identified from direct code analysis of existing sync mechanisms
- Validation: MEDIUM -- test infrastructure needs Wave 0 setup (in-memory SQLite pool)

**Research date:** 2026-03-21
**Valid until:** 2026-04-21 (stable -- core sync architecture won't change)
