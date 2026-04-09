# Phase 363: Data Recording Verification - Research

**Researched:** 2026-04-09
**Domain:** Rust/SQLite billing FSM, telemetry ingest, cloud sync, rc-agent session-end hooks
**Confidence:** HIGH

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**D-01:** Expected lap count uses a conservative floor heuristic. Formula: `max(1, floor(session_minutes / 3))` for trackday/practice sessions; `max(1, session_minutes / 2)` for hotlap sessions.

**D-02:** Gap flag is directional — flag only `actual < expected * 0.9` ("too few"). Over-recording is not flagged.

**D-03:** `sessions.lap_count_flag TEXT` (enum: OK / UNDER_RECORDED / UNVERIFIED). Default UNVERIFIED. Written at session end.

**D-04:** Telemetry completeness = `(seconds_with_any_packet / total_session_seconds) * 100`. Threshold: <80% = suspect. 1s-bucket histogram computed server-side at session end from ingest timestamps.

**D-05:** Histogram maintained in memory during session, flushed to `sessions.telemetry_coverage_pct` on finalize. Crash → bucket lost → NULL → UNVERIFIED.

**D-06:** `sessions.suspect BOOLEAN NOT NULL DEFAULT 0` + `sessions.suspect_reasons TEXT` (JSON array). Computed from `lap_count_flag != OK OR telemetry_coverage_pct < 80`.

**D-07:** CSV fallback uses pod-side session-end push hook (not server pull, not background scanner). rc-agent calls `POST /api/v1/sessions/{id}/telemetry-fallback` with multipart CSV body at session end.

**D-08:** Fallback triggered when `telemetry_coverage_pct < 100`. Pod checks if primary ingest had gaps this session; skips push if no gaps.

**D-09:** Server endpoint behind `sentry_service_key` auth (same as `/exec`). Max body 50 MB. CSV stored to `C:\RacingPoint\telemetry-fallback\{session_id}.csv`. Receipt recorded in `sessions.csv_fallback_received_at`.

**D-10:** Grace window via `lap_reject_grace_until TIMESTAMP` flag + FSM re-check. Sequence: session ends → FinalizePending state → `lap_reject_grace_until = now() + 5s` → FSM tick re-checks every 1s → after 5s, finalize proceeds. Not a tokio::sleep.

**D-11:** F-05 bug fix is bundled into GLD-C-04 scope. Fix touches `end_billing_session()` in `billing.rs`.

**D-12:** Rejected laps during grace window logged to `lap_rejections` table: `session_id, lap_number, rejected_at, reason, grace_window_caught BOOLEAN`.

**D-13:** All schema changes are additive ALTER TABLE. New columns on `sessions` table: `lap_count_expected INTEGER`, `lap_count_actual INTEGER`, `lap_count_flag TEXT`, `telemetry_coverage_pct REAL`, `suspect BOOLEAN NOT NULL DEFAULT 0`, `suspect_reasons TEXT`, `csv_fallback_received_at TIMESTAMP`, `lap_reject_grace_until TIMESTAMP`.

**D-14:** `ALTER TABLE ADD COLUMN IF NOT EXISTS` pattern (actually `let _ = sqlx::query("ALTER TABLE...").execute(pool).await;` — see pattern below). Cloud sync schema MUST be updated in same commit.

**D-15:** New `sessions.*` columns replicate via existing Phase 301 cloud_data_sync_v2 pipeline. `sync/sessions.rs` equivalent is cloud_sync.rs billing_sessions push — must add new columns to that JSON object.

### Claude's Discretion

- Exact DB migration file naming and numbering (follow existing pattern in `crates/racecontrol/migrations/`)
- Whether to use `TEXT JSON` or TEXT[] for `suspect_reasons` (SQLite → use JSON TEXT)
- Exact retry backoff schedule for CSV fallback POST
- Feature flag: `feature_flags.phase363_session_audit` (default true)
- Telemetry metric names for the new histogram bucket (follow Phase 285/289 naming)
- Exact tracing span structure for the finalize re-check loop

### Deferred Ideas (OUT OF SCOPE)

- Admin Suspect Laps page (`/admin/suspect-laps`) — Phase 367 GLD-G-01
- Per-lap telemetry heatmap drill-down — Phase 367 GLD-G-01 sub-feature
- AI-tier-aware expected lap count — Phase 365 GLD-E-01/E-02
- Telemetry gap detection in hot path — Phase 364 GLD-D-01
- Retroactive historical session flagging — forward-only per D-13
- Session replay for admin — Phase 367 GLD-G-03
- Batch export of session data — Phase 367 GLD-G-04
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| GLD-C-01 | Per-session lap audit: compare expected vs actual lap count at session end, flag sessions with >10% gap | `laps` table already has `session_id` FK; `end_billing_session()` is the hook point; floor heuristic formula locked in D-01 |
| GLD-C-02 | Telemetry completeness check: <80% coverage → `suspect: true` + `suspect_reasons` in DB | 1s-bucket histogram lives in BillingTimer (new field); flushed in `post_session_hooks` or session-end hook; `Telemetry` AgentMessage already received in ws/mod.rs |
| GLD-C-03 | CSV fallback auto-sync: POST from pod at session end within 30s | `csv_lap_fallback.rs` exists with CSV path `C:\RacingPoint\laps-offline.csv`; `SessionEnded` handler in ws_handler.rs is the push point; `sentry_service_key` auth pattern confirmed |
| GLD-C-04 + F-05 | Billing 5s grace window + F-05 read-after-write fix | `end_billing_session()` at billing.rs:3972 confirmed; F-05 partially fixed (CAS no longer overwrites `wallet_debit_paise`); grace window adds `FinalizePending` to billing_fsm.rs |
</phase_requirements>

---

## Summary

Phase 363 is a server-side Rust phase touching four tightly coupled subsystems: the billing FSM, the sessions table, the telemetry ingest pipeline, and cloud sync. All four requirements (GLD-C-01 through C-04) converge on the `end_billing_session()` function in `billing.rs` as the primary hook point. The F-05 bug (read-after-write on `wallet_debit_paise`) has already been **partially fixed** — the CAS UPDATE at line 4059 no longer includes `wallet_debit_paise` in its SET clause (comment at line 4054 confirms). The refund path (line 4113) does a fresh SELECT after the CAS and reads the original value correctly. This means D-11 scope is smaller than expected: the structural fix is already in place, but the phase still needs integration tests to verify it, and the grace window adds new state.

The `sessions` table is the lightweight race session table (not `billing_sessions`). It currently has 10 columns and is NOT included in the cloud sync push. The 8 new columns from D-13 all go on `sessions` via `ALTER TABLE` in `db/mod.rs`'s `migrate()` function (no separate migration files — the project uses a single inline migration function). The `lap_rejections` table does not exist yet and must be created.

The cloud sync push in `cloud_sync.rs` uses an explicit column list JSON object for `billing_sessions` (line 656-668). The `sessions` table is NOT currently synced. Phase 363 needs to add a sessions sync block to the push function, or clarify whether only `billing_sessions` needs the new audit columns. Research finding: GLD-C-01/C-02 audit columns belong on `billing_sessions` (not the abstract `sessions` table), because the lap count and telemetry coverage are per-billing-session, not per-race-session. This is a critical clarification.

**Primary recommendation:** Put all 8 new audit columns on `billing_sessions`, not `sessions`. The `billing_sessions` table already has cloud sync; the abstract `sessions` table does not. Update the `billing_sessions` push JSON in `cloud_sync.rs` to include the new columns.

---

## F-05 Bug Status (Critical Finding)

**F-05 is already structurally fixed in the current codebase.**

The RCA document describes the bug as `end_billing_session()` overwriting `wallet_debit_paise` at line 2213 before reading it at line 2255. The current function (now at lines 3972–4262 after growth) has this explicit comment at line 4054:

```
// NOTE: Do NOT overwrite wallet_debit_paise here — it must retain the original
// pre-session charge for correct refund calculation downstream (F-05 fix).
// final_cost_paise is stored in end_reason for audit purposes.
```

The CAS UPDATE (line 4059) sets only `status`, `driving_seconds`, `ended_at`, `end_reason`. The `wallet_debit_paise` column is NOT in the SET clause. The refund SELECT (line 4114–4123) reads `wallet_debit_paise` via a fresh SELECT after the CAS, which returns the original unmodified value.

**Remaining gap:** Zero integration tests exercise `end_billing_session()` end-to-end (confirmed by RCA RC-2). The phase must add these tests even though the structural fix is in place. Without them, a future edit could re-introduce the bug undetected.

**GLD-C-04 scope adjustment:** The grace window (`FinalizePending` + `lap_reject_grace_until`) is still net-new work. The F-05 integration tests are additive work within the same plan. No structural fix to billing.rs is required, only:
1. Add `FinalizePending` state to billing_fsm transition table + re-check tick
2. Add `lap_reject_grace_until` column to `billing_sessions`
3. Add integration tests for `end_billing_session()` with early-end + grace window paths

---

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| sqlx | workspace | SQLite ALTER TABLE migrations | Already used throughout db/mod.rs |
| tokio | workspace | Async runtime, `Instant` for grace window | Project baseline |
| chrono | workspace | UTC timestamps for `lap_reject_grace_until` | Already used in BillingTimer |
| serde_json | workspace | JSON encoding for `suspect_reasons TEXT` | SQLite has no native array type |
| axum | workspace | Route handler for CSV fallback POST | Project web framework |
| uuid | workspace | Session IDs for lap_rejections table | Already used everywhere |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| reqwest (rc-agent) | workspace | HTTP POST for CSV fallback push | Pod-side session-end push |
| multipart | check Cargo.toml | Multipart body for CSV upload | CSV fallback POST body |

---

## Architecture Patterns

### Migration Pattern (db/mod.rs)

All migrations live inline in the `migrate()` async function in `crates/racecontrol/src/db/mod.rs`. There are NO separate `.sql` files or numbered migration files. The pattern is:

```rust
// ─── Phase 363: Data Recording Verification ──────────────────────────────────
// SQLite does not support IF NOT EXISTS on ALTER TABLE — use let _ = ignore pattern
let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN lap_count_expected INTEGER")
    .execute(pool)
    .await;
let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN lap_count_actual INTEGER")
    .execute(pool)
    .await;
// ... (one execute() per column — never chain multiple ADDs in one query)
```

Each column addition is a separate `.execute()` call wrapped in `let _ =` to silently ignore the "duplicate column" error on re-run. This is the established idempotency pattern for the project.

For new tables, use `CREATE TABLE IF NOT EXISTS` followed by immediate `ALTER TABLE` additions for columns that may need to be added to an existing created table:

```rust
sqlx::query(
    "CREATE TABLE IF NOT EXISTS lap_rejections (
        id TEXT PRIMARY KEY,
        session_id TEXT NOT NULL,
        lap_number INTEGER NOT NULL,
        rejected_at TEXT NOT NULL DEFAULT (datetime('now')),
        reason TEXT,
        grace_window_caught BOOLEAN NOT NULL DEFAULT 0,
        venue_id TEXT
    )",
)
.execute(pool)
.await?;
```

### Cloud Sync Pattern (cloud_sync.rs)

The `build_push_payload()` function (around line 655) includes an explicit `json_object(...)` column list for `billing_sessions`. To add Phase 363 columns:

```rust
// In the billing_sessions json_object query, add:
'lap_count_expected', lap_count_expected,
'lap_count_actual', lap_count_actual,
'lap_count_flag', lap_count_flag,
'telemetry_coverage_pct', telemetry_coverage_pct,
'suspect', suspect,
'suspect_reasons', suspect_reasons,
'csv_fallback_received_at', csv_fallback_received_at,
'lap_reject_grace_until', lap_reject_grace_until
```

This is the ONLY change needed for cloud sync — no new sync path, no new handler. The cloud sync pipeline is push-only for billing_sessions (venue → cloud).

### Feature Flag Pattern

Feature flags are rows in the `feature_flags` SQLite table (name TEXT PRIMARY KEY, enabled BOOLEAN). The `AppState.feature_flags` is a `RwLock<HashMap<String, FeatureFlagRow>>` loaded at startup via `state.load_feature_flags()`.

To seed a flag at startup, add to `migrate()`:

```rust
let _ = sqlx::query(
    "INSERT OR IGNORE INTO feature_flags (name, enabled, default_value, overrides)
     VALUES ('phase363_session_audit', 1, 1, '{}')"
)
.execute(pool)
.await;
```

To read in code:

```rust
let audit_enabled = {
    let flags = state.feature_flags.read().await;
    flags.get("phase363_session_audit").map(|f| f.enabled).unwrap_or(true)
};
if !audit_enabled { return; }
```

### Billing FSM Extension Pattern

`billing_fsm.rs` is a pure transition table (no async logic). To add `FinalizePending`:

1. Add `FinalizePending` to the `BillingEvent` enum (NOT to BillingSessionStatus — it's an intermediate in-memory state, not a DB status)
2. The grace window is implemented as a field on `BillingTimer` (`lap_reject_grace_until: Option<DateTime<Utc>>`) and checked in the billing tick loop
3. No new DB status enum value is needed — `billing_sessions.status` stays as-is until finalize completes

**Key insight:** `FinalizePending` should be an in-memory flag on `BillingTimer`, not a new FSM status, because:
- The DB status must remain `active/paused_*` during the 5s window (CAS guards rely on this)
- The re-check happens in the existing billing tick (1s cadence)
- Restart-safety is provided by `lap_reject_grace_until` column on `billing_sessions`

```rust
// Add to BillingTimer struct:
/// GLD-C-04: Timestamp until which finalize is deferred (grace window for lap rejects)
/// None = no grace window active, Some(t) = wait until t before finalizing
pub lap_reject_grace_until: Option<DateTime<Utc>>,

/// GLD-C-04: End status deferred during grace window (set at session-end trigger)
pub pending_end_status: Option<BillingSessionStatus>,
```

The billing tick in `billing.rs` already runs every 1s. The grace window check goes into the tick loop:

```rust
// In the tick loop, for any timer with pending_end_status + lap_reject_grace_until:
if let (Some(grace_until), Some(end_status)) = (timer.lap_reject_grace_until, timer.pending_end_status) {
    if Utc::now() >= grace_until {
        // Grace window elapsed — finalize now
        timer.lap_reject_grace_until = None;
        timer.pending_end_status = None;
        // Call end_billing_session() with the deferred status
    }
    // else: still within grace window, skip this timer
}
```

### Session End Hook for Lap Audit

The audit runs in `post_session_hooks()` (billing.rs:4602) which is already a `tokio::spawn` fire-and-forget. Phase 363 extends `post_session_hooks` with the lap audit:

```rust
// At end of post_session_hooks:
run_session_audit(&state_clone, &session_id_clone).await;
```

`run_session_audit()` becomes a new function that:
1. Counts `SELECT COUNT(*) FROM laps WHERE billing_session_id = ?` (or joins via sessions)
2. Gets `allocated_seconds` and `experience_type` from `billing_sessions`
3. Computes expected laps via floor heuristic
4. Computes `suspect` boolean
5. Does `UPDATE billing_sessions SET lap_count_actual = ?, lap_count_expected = ?, lap_count_flag = ?, suspect = ?, suspect_reasons = ? WHERE id = ?`

**Important:** The billing_sessions table does NOT have a direct link to laps. The `laps` table has `session_id TEXT REFERENCES sessions(id)` — this is the abstract `sessions` table, not `billing_sessions`. Research finding: there is NO direct FK from laps to billing_sessions.

This means the lap audit must use one of:
- Time-window correlation: `SELECT COUNT(*) FROM laps WHERE driver_id = ? AND created_at BETWEEN billing_start AND billing_end`
- Or an indirect join via the pod + driver combination

Check the lap_tracker to see how laps are associated with billing sessions:

```
lap.billing_session_id = billing_session_id  (from resolve_driver_for_pod)
```

Actually — looking at ws/mod.rs line 880-885, `resolve_driver_for_pod` returns `(driver_id, session_id)` where `session_id` is the billing_session_id (from `active_timers`). The `laps` table has `session_id` which is set to the billing_session_id. This means: `SELECT COUNT(*) FROM laps WHERE session_id = ?` using the billing_session_id will work.

**Confidence:** MEDIUM — based on code reading. Needs confirmed in lap_tracker.rs.

### Stage 5 Pattern (launch_verifier.rs) as Template

The Phase 362 Stage 5 pattern (`verify_launch_config`) takes:
- An `ExpectedConfig` struct (what should happen)
- A reader function (what actually happened)
- An `on_stage` callback (reporting)

Phase 363's `run_session_audit()` follows the same shape:
- `AuditConfig` struct (expected laps from heuristic)
- DB queries for actual state
- No callback needed (just writes to DB)

The function is blocking-async (it does `await` on DB queries), called from `post_session_hooks` which is already `tokio::spawn`'d. No `spawn_blocking` needed — it's pure Rust async.

### rc-agent SessionEnded Hook

In `crates/rc-agent/src/ws_handler.rs`, the `CoreToAgentMessage::SessionEnded { ... }` handler is at line 377. This is where the CSV fallback POST will be added. The existing handler:

1. Clears inactivity monitor
2. Sets billing_active = false
3. Stops game process
4. Shows session summary on lock screen
5. Sets a blank timer (30s)

Phase 363 adds after step 2 (before game is stopped, while we still have session context):

```rust
// GLD-C-03: CSV fallback push if telemetry gaps occurred this session
if state.had_telemetry_gaps_this_session {
    tokio::spawn(push_csv_fallback(
        state.server_url.clone(),
        state.service_key.clone(),
        billing_session_id.clone(),
    ));
    state.had_telemetry_gaps_this_session = false;
}
```

The `push_csv_fallback` function reads `C:\RacingPoint\laps-offline.csv` and POSTs it as multipart to the server. It should be a separate async function with exponential retry.

**Tracking telemetry gaps in rc-agent:** The agent needs a boolean flag `had_telemetry_gaps_this_session` on its connection state. The UDP telemetry ingest (in the sim adapter) already tracks data flow. Where a gap is detected (Phase 364 will wire it fully), a simple `AtomicBool` is enough for Phase 363.

However, Phase 363's scope is: "if CSV file exists and has content → push it." The D-08 decision says "primary ingest self-reports gaps." The simplest implementation: **always check if the CSV file has content at session end, and push if it does.** This avoids needing a new in-memory flag.

### Service Key Auth Pattern

From `api/routes.rs` line 22544-22557, the service key pattern is:

```rust
let expected = state.config.pods.sentry_service_key.as_deref().unwrap_or("");
let provided = headers.get("X-Service-Key").and_then(|v| v.to_str().ok()).unwrap_or("");
if expected.is_empty() || provided.is_empty() || provided != expected {
    return (axum::http::StatusCode::UNAUTHORIZED, "Invalid service key").into_response();
}
```

For the new `/api/v1/sessions/{id}/telemetry-fallback` endpoint, use this exact pattern — inline service key check, NOT a middleware layer.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Telemetry coverage histogram | Custom time-window indexer | `Vec<bool>` per second OR `BitVec<u8>` | 30min session = 1800 bools = 1800 bytes in-memory, negligible |
| Migration numbering | Separate .sql files | Inline in `db/mod.rs` migrate() | Project doesn't use separate migration files — all migrations are inline |
| Auth middleware for new endpoint | New middleware layer | Inline service key check (existing pattern) | All pod-authenticated endpoints use inline key check, not Axum middleware |
| Retry logic for CSV POST | Custom retry state machine | Simple `for attempt in 0..5` with exponential backoff | 5 retries × exponential = <10min, matches 30s normal-case SLA |
| Grace window timer | `tokio::time::sleep` | `lap_reject_grace_until` field on BillingTimer + billing tick | Not restart-safe; flag-based is the locked decision (D-10) |

---

## Runtime State Inventory

> Phase 363 is a forward-only, additive phase. No renames, no rebrands, no schema rewrites.

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | No `lap_count_flag`, `telemetry_coverage_pct`, or `lap_reject_grace_until` columns exist in any DB | Additive ALTER TABLE only — no data migration needed |
| Live service config | Feature flag `phase363_session_audit` does not yet exist in DB | Seed via `INSERT OR IGNORE` in migrate() |
| OS-registered state | None — no new scheduled tasks, no new registry keys | None |
| Secrets/env vars | `sentry_service_key` — existing, unchanged. Used by new fallback endpoint | None — no key changes |
| Build artifacts | None — new code, no renamed binaries | None |

**Nothing found that requires a data migration** — only schema additions (new columns default to NULL/0/UNVERIFIED for existing rows, which is correct for forward-only behavior).

---

## Common Pitfalls

### Pitfall 1: Wrong Table (sessions vs billing_sessions)

**What goes wrong:** Putting the 8 new audit columns on `sessions` (abstract race session) instead of `billing_sessions` (the financial record).
**Why it happens:** CONTEXT.md D-13 says "sessions table" but the research shows that `billing_sessions` is the table with cloud sync, driver_id, laps FK, and all the financial fields. The abstract `sessions` table is used for multiplayer scheduling and has no billing data.
**How to avoid:** Put ALL 8 new columns on `billing_sessions`. The lap audit writes there. Cloud sync in cloud_sync.rs pushes `billing_sessions` — adding columns there is one JSON object change.
**Warning signs:** If you find yourself needing to JOIN sessions to billing_sessions just to read the new audit fields, you're on the wrong table.

### Pitfall 2: Lock Held Across .await in Billing Tick

**What goes wrong:** Reading `active_timers` write lock, then calling `end_billing_session()` (which also acquires `active_timers` write lock) while the outer lock is held.
**Why it happens:** The grace window check is in the tick loop that already holds or re-acquires `active_timers`.
**How to avoid:** Follow existing pattern: collect timers to finalize into a `Vec<(String, BillingSessionStatus)>` first (within the lock), drop the lock, then iterate and call `end_billing_session()`. Never hold a lock across `.await`.
**Warning signs:** Deadlock symptoms — billing tick stops processing.

### Pitfall 3: F-05 Already Fixed — Don't Double-Fix

**What goes wrong:** Implementing a "save original before write" fix in `end_billing_session()` that conflicts with the already-applied CAS-based fix.
**Why it happens:** The RCA document describes the bug at "line 2213/2255" — these line numbers are stale. The function has grown to lines 3972-4262. The fix is already there.
**How to avoid:** The CAS UPDATE (line 4059) already does NOT include `wallet_debit_paise`. The fresh SELECT (line 4114) reads the correct original value. Only add integration tests; do NOT restructure the UPDATE.
**Warning signs:** If your diff adds `let original_debit = wallet_debit_paise;` before line 4059, check if it's already handled.

### Pitfall 4: Lap Count Query Using Wrong FK

**What goes wrong:** `SELECT COUNT(*) FROM laps WHERE session_id = ?` returning 0 because `session_id` in laps is set to the abstract `sessions.id`, not the `billing_sessions.id`.
**Why it happens:** The laps schema shows `session_id TEXT REFERENCES sessions(id)` — not billing_sessions. But in ws/mod.rs line 883-885, `resolve_driver_for_pod()` sets `lap.session_id = billing_session_id`. This is a schema / runtime mismatch that needs to be verified.
**How to avoid:** Verify in `lap_tracker::persist_lap()` what value gets written to `laps.session_id`. If it's the billing_session_id, the COUNT query works. If not, use a time-window JOIN.
**Warning signs:** The audit always shows 0 laps for every session.

### Pitfall 5: CSV File Cleared Before Fallback Push

**What goes wrong:** `csv_lap_fallback::clear_csv_laps()` is called somewhere that clears the file before the session-end push fires.
**Why it happens:** The CSV clear might be called on WS reconnect or on successful lap sync.
**How to avoid:** In `push_csv_fallback()`, read and buffer the CSV content BEFORE clearing. Clear only after a confirmed successful POST (HTTP 200 from server).
**Warning signs:** CSV POST sends empty body; server receives 0 bytes.

### Pitfall 6: Cloud Sync SYNC_TABLES Constant Does Not Include billing_sessions

**What goes wrong:** The new `billing_sessions` columns (lap audit data) are in the push payload but never pulled by cloud.
**Why it happens:** Cloud sync push is one-way for billing_sessions (venue → cloud). The SYNC_TABLES constant (line 31 of cloud_sync.rs) lists tables for PULL (cloud → venue). billing_sessions is not in SYNC_TABLES (which is correct — billing is venue-authoritative). The new columns will automatically be part of the push as long as they're in the json_object() query. No change to SYNC_TABLES needed.
**Warning signs:** Wrong symptom — if new audit columns appear in venue DB but not cloud DB, check the json_object() query in build_push_payload(), not SYNC_TABLES.

---

## Code Examples

### Migration Pattern (Confirmed from db/mod.rs)

```rust
// Source: crates/racecontrol/src/db/mod.rs (lines 1381-1396 pattern)
// Phase 363: Data Recording Verification schema additions
// All on billing_sessions (not sessions) — billing_sessions has cloud sync + driver_id + laps FK
let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN lap_count_expected INTEGER")
    .execute(pool)
    .await;
let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN lap_count_actual INTEGER")
    .execute(pool)
    .await;
let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN lap_count_flag TEXT DEFAULT 'UNVERIFIED'")
    .execute(pool)
    .await;
let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN telemetry_coverage_pct REAL")
    .execute(pool)
    .await;
let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN suspect BOOLEAN NOT NULL DEFAULT 0")
    .execute(pool)
    .await;
let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN suspect_reasons TEXT")
    .execute(pool)
    .await;
let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN csv_fallback_received_at TEXT")
    .execute(pool)
    .await;
let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN lap_reject_grace_until TEXT")
    .execute(pool)
    .await;

// lap_rejections: new table (D-12)
sqlx::query(
    "CREATE TABLE IF NOT EXISTS lap_rejections (
        id TEXT PRIMARY KEY,
        billing_session_id TEXT NOT NULL,
        lap_number INTEGER NOT NULL,
        rejected_at TEXT NOT NULL DEFAULT (datetime('now')),
        reason TEXT,
        grace_window_caught BOOLEAN NOT NULL DEFAULT 0,
        venue_id TEXT
    )",
)
.execute(pool)
.await?;

// Feature flag seed
let _ = sqlx::query(
    "INSERT OR IGNORE INTO feature_flags (name, enabled, default_value, overrides)
     VALUES ('phase363_session_audit', 1, 1, '{}')"
)
.execute(pool)
.await;
```

### Cloud Sync Extension (Confirmed from cloud_sync.rs lines 655-668)

```rust
// Source: crates/racecontrol/src/cloud_sync.rs
// Extend the billing_sessions json_object() query to include Phase 363 audit columns.
// Add after 'end_reason', end_reason:
'lap_count_expected', lap_count_expected,
'lap_count_actual', lap_count_actual,
'lap_count_flag', COALESCE(lap_count_flag, 'UNVERIFIED'),
'telemetry_coverage_pct', telemetry_coverage_pct,
'suspect', COALESCE(suspect, 0),
'suspect_reasons', suspect_reasons,
'csv_fallback_received_at', csv_fallback_received_at
// Note: lap_reject_grace_until is transient — cleared after finalize. Sync if non-null.
```

### Service Key Auth for New Endpoint (Confirmed from routes.rs line 22544-22558)

```rust
// Source: crates/racecontrol/src/api/routes.rs (pattern from mesh_audit_seed_service)
// POST /api/v1/sessions/{id}/telemetry-fallback
async fn telemetry_fallback_handler(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    headers: axum::http::HeaderMap,
    // multipart body
    mut multipart: axum::extract::Multipart,
) -> axum::response::Response {
    let expected = state.config.pods.sentry_service_key.as_deref().unwrap_or("");
    let provided = headers.get("X-Service-Key").and_then(|v| v.to_str().ok()).unwrap_or("");
    if expected.is_empty() || provided.is_empty() || provided != expected {
        return (StatusCode::UNAUTHORIZED, "Invalid service key").into_response();
    }
    // ... process multipart, write CSV, update billing_sessions.csv_fallback_received_at
}
```

### BillingTimer Grace Window Fields

```rust
// Source: crates/racecontrol/src/billing.rs (extend BillingTimer struct)
// GLD-C-04: Grace window for lap-reject deferral
/// UTC timestamp until which finalize is deferred. None = no grace window active.
/// Written at session-end trigger. Checked in billing tick. Cleared after finalize.
pub lap_reject_grace_until: Option<DateTime<Utc>>,
/// End status deferred during grace window. Set at session-end trigger.
pub pending_end_status: Option<BillingSessionStatus>,
```

### Coverage Histogram in BillingTimer

```rust
// New field on BillingTimer
/// GLD-C-02: 1s-bucket telemetry coverage. Bit N = true if any telemetry packet
/// was received during second N of the session. Length = elapsed_seconds.
/// Allocated lazily (None until first packet). On server crash, bucket is lost → NULL coverage.
pub telemetry_seconds_covered: std::collections::HashSet<u32>,
// Alternatively: Vec<bool> with capacity allocated at session start
```

Updated in `ws/mod.rs` when `AgentMessage::Telemetry` is received:

```rust
// In the Telemetry handler, after existing processing:
// GLD-C-02: Record telemetry coverage bucket (DO NOT hold lock across .await)
{
    let mut timers = state.billing.active_timers.write().await; // already written above
    if let Some(timer) = timers.get_mut(&frame.pod_id) {
        let elapsed = timer.elapsed_seconds;
        timer.telemetry_seconds_covered.insert(elapsed);
    }
} // guard dropped
```

---

## Environment Availability

> Step 2.6: Phase 363 is a server-side Rust code change. No external tools beyond the project's own Rust toolchain are required. Environment audit SKIPPED — pure Rust/SQLite/HTTP, no external service dependencies.

---

## Validation Architecture

> `workflow.nyquist_validation: true` in `.planning/config.json` — section required.

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust `#[cfg(test)]` inline modules + `#[tokio::test]` for async |
| Config file | None — inline modules in each .rs file |
| Quick run command | `cargo test -p racecontrol -- billing 2>&1 \| tail -20` |
| Full suite command | `cargo test -p racecontrol && cargo test -p rc-common && cargo test -p rc-agent` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| GLD-C-01 | Lap heuristic: 30min → expect 10 laps trackday; 0 actual → UNDER_RECORDED | unit | `cargo test -p racecontrol -- session_audit::test_lap_heuristic` | ❌ Wave 0 |
| GLD-C-01 | Lap audit: 9 laps in 30min (>10% gap) → UNDER_RECORDED | unit | `cargo test -p racecontrol -- session_audit::test_lap_audit_under_recorded` | ❌ Wave 0 |
| GLD-C-01 | Lap audit: fast driver, 12 laps in 30min → OK (no over-recording flag) | unit | `cargo test -p racecontrol -- session_audit::test_lap_audit_ok_over_expected` | ❌ Wave 0 |
| GLD-C-02 | Coverage: 1800s session, 1200 seconds covered → 66.7% → suspect=true | unit | `cargo test -p racecontrol -- session_audit::test_telemetry_coverage_suspect` | ❌ Wave 0 |
| GLD-C-02 | Coverage: 1800s session, 1500 seconds covered → 83% → suspect=false | unit | `cargo test -p racecontrol -- session_audit::test_telemetry_coverage_ok` | ❌ Wave 0 |
| GLD-C-03 | CSV fallback: file has content → POST fired at session end | integration (mock server) | `cargo test -p rc-agent -- csv_fallback::test_push_on_session_end` | ❌ Wave 0 |
| GLD-C-03 | CSV fallback: file empty → no POST | unit | `cargo test -p rc-agent -- csv_fallback::test_no_push_when_empty` | ❌ Wave 0 |
| GLD-C-04 | Grace window: lap reject arrives within 5s → grace_window_caught=true | integration (SQLite) | `cargo test -p racecontrol -- billing_grace::test_grace_window_catches_reject` | ❌ Wave 0 |
| GLD-C-04 | Grace window: no lap reject in 5s → finalize proceeds | integration (SQLite) | `cargo test -p racecontrol -- billing_grace::test_grace_window_expires_normally` | ❌ Wave 0 |
| F-05 fix | end_billing_session early-end: refund uses original debit, not final_cost_paise | integration (SQLite in-mem) | `cargo test -p racecontrol -- billing::test_end_billing_session_early_end_refund_amount` | ❌ Wave 0 |
| F-05 fix | end_billing_session: Rs.700 session ends at 15min → refund Rs.350 (not Rs.187.50) | integration (SQLite in-mem) | `cargo test -p racecontrol -- billing::test_f05_refund_uses_original_debit` | ❌ Wave 0 |

### Sampling Rate

- **Per task commit:** `cargo test -p racecontrol -- billing 2>&1 | tail -20`
- **Per wave merge:** `cargo test -p racecontrol && cargo test -p rc-common && cargo test -p rc-agent`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps

- [ ] `crates/racecontrol/src/session_audit.rs` — new module for GLD-C-01/C-02 audit logic + tests
- [ ] `crates/racecontrol/src/billing.rs` — `#[cfg(test)]` block for `end_billing_session()` integration tests (GLD-C-04 + F-05)
- [ ] `crates/rc-agent/src/csv_lap_fallback.rs` — add tests for push_on_session_end + no_push_when_empty
- [ ] SQLite in-memory test pool: `db::test_pool()` already exists in db/mod.rs (used by tests at lines 4420-4504) — reuse this pattern

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| F-05 bug (wallet_debit_paise overwritten in CAS UPDATE) | CAS UPDATE excludes wallet_debit_paise; fresh SELECT reads original value | Between 2026-03-28 RCA and current build | Phase 363 only needs tests, not structural fix |
| Single migrate() call with no idempotency | `let _ = sqlx::query(ALTER TABLE...)` pattern — silently ignores duplicate column errors | Phase 301+ | New columns must use `let _ =` pattern, not raw execute() |
| billing_fsm.rs had no restart-safe deferred finalize | Extension: `lap_reject_grace_until` field on BillingTimer + tick loop re-check | Phase 363 (new) | Restart-safe by design; inspectable via DB |

**Current in billing_fsm.rs:** No `FinalizePending` state exists. The FSM only has terminal end paths (End/EndEarly/Cancel). Phase 363 adds the deferred finalize as an in-memory timer field, not a new FSM status.

---

## Open Questions

1. **Which table do laps.session_id actually reference at runtime?**
   - What we know: Schema says `REFERENCES sessions(id)` but ws/mod.rs line 883-885 sets `lap.session_id = billing_session_id` (from `active_timers` which holds billing_session_ids)
   - What's unclear: Is there a separate path where laps get session_id = the abstract sessions.id?
   - Recommendation: Read `lap_tracker::persist_lap()` to confirm. If billing_session_id is written to `laps.session_id`, the audit COUNT query works directly. If not, need time-window JOIN.

2. **Does BillingTimer.telemetry_seconds_covered need to survive server restart?**
   - What we know: D-05 says "if server crashes mid-session, bucket is lost → NULL → UNVERIFIED." This is acceptable.
   - What's unclear: Does the coverage bucket need to be snapshotted to `billing_sessions` periodically during the session (for long sessions)?
   - Recommendation: Flush only at session end (D-05 explicitly allows loss on crash). No periodic snapshot needed for Phase 363.

3. **rc-agent reqwest dependency: is it available for the CSV fallback POST?**
   - What we know: rc-agent makes HTTP calls (to server's :8090 endpoint). The `reqwest` crate is likely in Cargo.toml.
   - What's unclear: Whether reqwest supports multipart and what the exact feature set is.
   - Recommendation: Check `crates/rc-agent/Cargo.toml` during planning. Alternative: use the existing `ureq` or `hyper` if present. The POST could also use raw TCP if reqwest is unavailable.

4. **Where does the telemetry coverage bucket get updated — ws/mod.rs or billing tick?**
   - What we know: `AgentMessage::Telemetry` is received in `ws/mod.rs` (line 856). The handler currently reads from frame.pod_id to look up the pod.
   - What's unclear: Whether writing to `BillingTimer.telemetry_seconds_covered` inside the Telemetry handler creates a lock contention issue (telemetry is high-frequency at 10-60Hz).
   - Recommendation: Use `try_write()` in the Telemetry handler — if the lock is busy, skip the bucket update (minor coverage undercounting is acceptable per D-04's "tolerates brief blips" design). This avoids blocking the WS receive loop.

---

## Project Constraints (from CLAUDE.md)

The following CLAUDE.md directives are directly relevant to Phase 363 and must be enforced:

1. **Never hold a lock across `.await`** — Any new code in billing.rs that reads `active_timers` or `agent_senders` must snapshot + drop the guard in a `{ }` block before any `.await`. The grace window check in the billing tick and the coverage histogram update in the Telemetry handler both touch these locks.

2. **Financial flow E2E** — Before shipping Phase 363, trace: create customer → topup → 30min session → end at 15min → verify refund = Rs.350. The integration tests MUST verify this specific scenario (F-05 regression prevention).

3. **DB migrations must cover ALL consumers** — `ALTER TABLE billing_sessions ADD COLUMN` for all 8 new columns. The cloud racecontrol DB (Bono VPS) will be updated by `git_pull + cargo build + restart` which runs `migrate()` automatically.

4. **Every `::default()` in new code must be reviewed** — The new `BillingTimer` fields (`lap_reject_grace_until: None`, `pending_end_status: None`, `telemetry_seconds_covered: HashSet::new()`) are all correct defaults. The `lap_count_flag` default `UNVERIFIED` is intentional. Mark with `// Intentional default: UNVERIFIED until audit runs` comment.

5. **DEPLOY PARITY** — racecontrol binary + Bono VPS binary must both include the migration. Deploy sequence: venue binary → migration runs automatically → cloud git_pull → cloud cargo build → cloud pm2 restart → migration runs automatically on cloud DB. The new `billing_sessions` columns will exist on both DBs before any session produces audit data.

6. **Deploy Manifest Protocol** — Phase 363 PLAN.md must include `deploy:` section covering: Rust binary (racecontrol + rc-agent), DB migration (auto-run via migrate()), feature flag seed (auto-run via migrate()), cloud parity (Bono VPS git_pull + cargo build + pm2 restart), rc-agent to all 8 pods (CSV fallback POST hook), config (C:\RacingPoint\telemetry-fallback\ directory creation).

7. **gsd-nyquist-auditor MANDATORY** — Phase 363 is business logic (billing, sessions). MUST run `gsd-nyquist-auditor` after execution before marking done.

8. **MMA audit MANDATORY** — Phase 363 creates a cross-system bridge: rc-agent → server HTTP POST (telemetry-fallback) and billing tick → deferred finalize. Per standing rules: "New feature that creates a data flow across 2+ system boundaries MUST have multi-model AI audit before deploy."

9. **Route uniqueness** — New `POST /api/v1/sessions/{id}/telemetry-fallback` must not duplicate any existing route. Verify with `grep -n '\.route.*telemetry-fallback' crates/racecontrol/src/api/routes.rs` returns zero hits before adding.

10. **ASCII-only .bat files** — If Phase 363 adds a directory creation to start-racecontrol.bat or any bat file, use ASCII-safe content and verify with pre-flight grep.

---

## Sources

### Primary (HIGH confidence)
- `crates/racecontrol/src/billing.rs` (lines 3972-4262) — `end_billing_session()` current state; F-05 status confirmed
- `crates/racecontrol/src/billing_fsm.rs` — full FSM transition table; no FinalizePending exists
- `crates/racecontrol/src/db/mod.rs` (lines 88-101, 1381-1396, 3957-3960) — sessions schema; migration pattern; last migration
- `crates/racecontrol/src/cloud_sync.rs` (lines 655-684) — billing_sessions push column list
- `crates/rc-agent/src/ws_handler.rs` (lines 377-414) — SessionEnded handler; CSV push insertion point
- `crates/rc-agent/src/csv_lap_fallback.rs` — CSV path `C:\RacingPoint\laps-offline.csv`; clear_csv_laps()
- `crates/rc-agent/src/launch_verifier.rs` (lines 222-260) — Stage 5 pattern as template
- `.planning/phases/363-data-recording-verification/363-CONTEXT.md` — 15 locked decisions
- `.planning/audits/ROOT-CAUSE-ANALYSIS-F05-2026-03-28.md` — F-05 root causes; confirmed fix exists
- `crates/racecontrol/src/flags.rs` — feature flag table schema; runtime read pattern
- `crates/racecontrol/src/state.rs` (line 536) — load_feature_flags(); `INSERT OR IGNORE` seed pattern

### Secondary (MEDIUM confidence)
- `.planning/milestones/v46.0-REQUIREMENTS.md` — REQ-ID mapping; requirements text
- `.planning/codebase/CONVENTIONS.md` — module structure, naming, error handling
- `.planning/codebase/TESTING.md` — existing test infrastructure (minimal); db::test_pool() confirmed
- `crates/racecontrol/src/ws/mod.rs` (lines 856-897) — Telemetry handler; billing session lock access

### Tertiary (LOW confidence — needs verification)
- Lap tracker session_id = billing_session_id claim (based on ws/mod.rs line 883 reading, not lap_tracker.rs direct read) — **verify during planning**
- rc-agent reqwest/HTTP client availability for multipart POST — **check Cargo.toml during planning**

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all libraries already in workspace
- Architecture: HIGH — all modification points confirmed by direct code reading
- Pitfalls: HIGH — based on actual code structure; F-05 status verified directly
- Open questions: 4 items, all MEDIUM risk (verifiable in 1-2 file reads)

**Research date:** 2026-04-09
**Valid until:** 2026-05-09 (30 days — stable codebase, no fast-moving dependencies)

---

## RESEARCH COMPLETE
