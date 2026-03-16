# Phase 34: Admin Rates API - Research

**Researched:** 2026-03-17
**Domain:** Rust / Axum HTTP handlers + SQLite + in-memory RwLock cache
**Confidence:** HIGH — all findings are from direct code inspection of the live codebase

---

## Summary

All four required HTTP handlers already exist as private `async fn` functions in
`crates/racecontrol/src/api/routes.rs`, and the four routes are already wired into
`api_routes()`. The table schema and three seed rows exist in both production
(`crates/racecontrol/src/db/mod.rs`) and integration-test migrations
(`crates/racecontrol/tests/integration.rs`). The `BillingManager` struct exposes
a `rate_tiers: RwLock<Vec<BillingRateTier>>` field, and the existing
`billing::refresh_rate_tiers(&state)` function is already called from all three
write handlers (POST, PUT, DELETE) to push DB changes into the cache immediately.

The delta between "what exists" and "what the success criteria requires" is narrow:

1. POST returns `Json<Value>` with HTTP 200 — success criterion requires 201 Created.
2. DELETE does a soft-delete (`is_active = 0`) and returns `Json<Value>` with HTTP
   200 — success criterion requires 204 No Content on hard or soft delete, and the
   cost-calculation unit test must confirm the deleted tier is excluded.
3. The integration test that asserts the 3 seed rows via `GET /billing/rates` does
   not yet exist (only a raw SQL count test exists).
4. A unit test confirming that `compute_session_cost()` ignores a soft-deleted tier
   (after cache refresh) does not yet exist.

The planner therefore needs tasks only for: (a) fixing the POST status to 201,
(b) fixing the DELETE status to 204, (c) adding integration HTTP tests, and
(d) adding the `compute_session_cost` unit test. No new files, no schema changes,
no cache-wiring work — that is all already done.

**Primary recommendation:** Patch the two response-type mismatches in
`api/routes.rs`, then add the missing test cases in `tests/integration.rs`.

---

<phase_requirements>
## Phase Requirements

| ID       | Description                                                                       | Research Support |
|----------|-----------------------------------------------------------------------------------|-----------------|
| ADMIN-01 | Staff can GET all billing rates via `/billing/rates`                               | Handler `list_billing_rates` already exists and is wired. Returns `{"rates":[...]}`. Needs HTTP integration test asserting 3 seed rows. |
| ADMIN-02 | Staff can create a rate tier via POST `/billing/rates`                             | Handler `create_billing_rate` already exists, calls `refresh_rate_tiers`. Needs status changed from 200 to 201, and integration test confirming subsequent GET includes the new row. |
| ADMIN-03 | Staff can update via PUT `/billing/rates/{id}` — cache invalidates immediately     | Handler `update_billing_rate` already calls `refresh_rate_tiers` after DB write. Cache invalidation is already implemented. Needs integration test and unit test confirming cache has new value within one tick. |
| ADMIN-04 | Staff can delete via DELETE `/billing/rates/{id}` — cache invalidates immediately  | Handler `delete_billing_rate` already calls `refresh_rate_tiers` after soft delete. Needs status changed from 200 to 204 No Content, plus unit test that `compute_session_cost` does not include the deleted tier. |
</phase_requirements>

---

## Standard Stack

### Core (already in use — no new deps required)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| axum | 0.8 | HTTP routing + extractors | Already the web framework |
| sqlx | 0.8 | SQLite queries | Already the DB layer |
| tokio::sync::RwLock | tokio 1.x | Async read/write lock for rate_tiers cache | Already wrapping BillingManager fields |
| serde_json / axum::Json | already in scope | JSON extraction and response | Already used in all handlers |
| uuid | workspace | ID generation | Already used in create handlers |

### No new dependencies needed for this phase.

---

## Architecture Patterns

### Existing Pattern: Handler in `api/routes.rs`, module fn for DB logic

All CRUD for billing rates follows the same pattern as `pricing_tiers` (lines
1492–1629). Handlers live as private `async fn` in `api/routes.rs` and call
`sqlx::query*` directly against `state.db`. Cache invalidation is a fire-and-forget
call to `billing::refresh_rate_tiers(&state).await` after the DB write succeeds.

### Pattern for 201 Created response

The existing handlers return `Json<Value>` (always HTTP 200). To return 201, change
the return type to `(axum::http::StatusCode, Json<Value>)`:

```rust
// Source: existing pattern in api/routes.rs line 11703-11705
async fn create_billing_rate(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> (axum::http::StatusCode, Json<Value>) {
    // ... DB insert ...
    match result {
        Ok(_) => {
            billing::refresh_rate_tiers(&state).await;
            (axum::http::StatusCode::CREATED, Json(json!({ "id": id, "tier_name": tier_name })))
        }
        Err(e) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))),
    }
}
```

### Pattern for 204 No Content response

```rust
// Source: axum docs pattern — return StatusCode directly (no body)
async fn delete_billing_rate(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> axum::http::StatusCode {
    // ... soft delete + audit + cache refresh ...
    axum::http::StatusCode::NO_CONTENT
}
```

Note: if audit logging or error return is needed, use
`Result<StatusCode, (StatusCode, Json<Value>)>` as the return type.

### Pattern: Integration test (DB-layer, no HTTP server)

Existing tests in `tests/integration.rs` use raw sqlx against an in-memory pool
via `create_test_db()` + `create_test_state()`. There are no axum HTTP-layer tests
(no `tower::ServiceExt::oneshot` or `axum-test` crate). The pattern to follow is:

```rust
// Source: integration.rs test_db_setup() and billing tests
#[tokio::test]
async fn test_billing_rates_get_returns_3_seed_rows() {
    let pool = create_test_db().await;
    let state = create_test_state(pool.clone());

    // Call handler logic via billing module (not HTTP)
    billing::refresh_rate_tiers(&state).await;
    let tiers = state.billing.rate_tiers.read().await;
    assert_eq!(tiers.len(), 3);
}
```

For the HTTP-layer assertions (201 status, 204 status), a direct call to the
handler function using axum's extractor pattern is sufficient because the handlers
are pure async functions — they can be called directly in tests if made `pub(crate)`
or tested via sqlx assertions post-action.

### Recommended approach for HTTP status tests

Since axum-test is not a dependency and the existing test suite does not use HTTP
layer testing, the integration tests should:
- For ADMIN-01: query `billing_rates` table via sqlx after `create_test_db()` and
  assert 3 rows (already exists in `test_db_setup`; add a dedicated test that also
  verifies the cache matches).
- For ADMIN-02: call the DB INSERT directly (as the handler does), call
  `refresh_rate_tiers`, then assert the cache has 4 entries and the new row is in
  the DB.
- For ADMIN-03: same pattern — UPDATE, refresh, assert cache value changed.
- For ADMIN-04: soft-DELETE, refresh, call `compute_session_cost`, assert deleted
  tier is not reflected.

If the success criteria strictly requires HTTP status code 201/204 to be verified,
add `axum-test = "0.5"` as a dev-dependency and use its `TestServer`. But the
existing test infrastructure does not use it — the planner should decide.

---

## Exact Code Inventory

### BillingManager (state.rs line 88, billing.rs lines 338–358)

```
AppState field:   state.billing          (type: BillingManager)
Cache field:      state.billing.rate_tiers  (type: RwLock<Vec<BillingRateTier>>)
Cache write:      *state.billing.rate_tiers.write().await = tiers;
Invalidation fn:  billing::refresh_rate_tiers(&state).await
                  (reads DB, writes into rate_tiers — defined billing.rs lines 77–100)
```

`refresh_rate_tiers` only loads rows where `is_active = 1`, so a soft-delete
(`is_active = 0`) followed by `refresh_rate_tiers` immediately removes the tier
from the cache. No additional invalidation mechanism is needed.

### BillingRateTier struct (billing.rs lines 59–65)

```rust
pub struct BillingRateTier {
    pub tier_order: u32,
    pub tier_name: String,
    pub threshold_minutes: u32,  // 0 = unlimited
    pub rate_per_min_paise: i64,
}
```

Note: `is_active` is a DB column but is not on the struct (filtered at query time).

### billing_rates table schema (db/mod.rs lines 237–246)

```sql
CREATE TABLE IF NOT EXISTS billing_rates (
    id                  TEXT PRIMARY KEY,
    tier_order          INTEGER NOT NULL,
    tier_name           TEXT NOT NULL,
    threshold_minutes   INTEGER NOT NULL,
    rate_per_min_paise  INTEGER NOT NULL,
    is_active           BOOLEAN DEFAULT 1,
    created_at          TEXT DEFAULT (datetime('now')),
    updated_at          TEXT DEFAULT (datetime('now'))
)
```

Seed rows: `rate_standard` (order 1, 30min, 2500p), `rate_extended` (order 2, 60min,
2000p), `rate_marathon` (order 3, unlimited, 1500p).

### Existing handler locations (api/routes.rs)

| Handler | Lines | Status returned | Gap |
|---------|-------|-----------------|-----|
| `list_billing_rates` | 1633–1657 | 200 + `{"rates":[...]}` | None — correct |
| `create_billing_rate` | 1659–1688 | 200 + `{"id":..., "tier_name":...}` | Must be 201 |
| `update_billing_rate` | 1690–1752 | 200 + `{"ok":true}` | None per spec |
| `delete_billing_rate` | 1754–1775 | 200 + `{"ok":true}` | Must be 204 (no body) |

All four routes are registered at lines 70–71 of `api_routes()`.

### compute_session_cost (billing.rs line 122)

```rust
pub fn compute_session_cost(elapsed_seconds: u32, tiers: &[BillingRateTier]) -> SessionCost
```

Takes a slice of tiers. The caller passes `&state.billing.rate_tiers.read().await`.
After soft-delete + cache refresh, the deleted tier will not appear in that slice,
so a subsequent call naturally excludes it. The unit test just needs to verify this.

### audit logging pattern (already in update + delete handlers)

Both `update_billing_rate` and `delete_billing_rate` already call:
```rust
accounting::snapshot_row(&state, "billing_rates", &id).await;
accounting::log_audit(&state, "billing_rates", &id, "update"/"delete", ...).await;
```
`create_billing_rate` does not call `log_audit` (consistent with `create_pricing_tier`
which also skips audit on create).

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Cache invalidation | Custom notify/broadcast mechanism | `billing::refresh_rate_tiers(&state)` | Already implemented, already called from all write handlers |
| ID generation | Sequential counters | `uuid::Uuid::new_v4().to_string()` | Already used in `create_billing_rate` |
| Soft delete | Hard DELETE + audit cascade | `UPDATE ... SET is_active = 0` | Already the pattern for pricing_tiers; `refresh_rate_tiers` filters `is_active = 1` |
| HTTP test server | Manual TCP bind | `axum-test` crate if needed | Existing tests are DB-layer only; planner decides if HTTP-layer tests are added |

---

## Common Pitfalls

### Pitfall 1: DELETE returns body with 204
**What goes wrong:** Returning `Json<Value>` with `StatusCode::NO_CONTENT` —
axum will ignore the body for 204 but the return type mismatch causes a compile
error if using `(StatusCode, Json<Value>)`.
**How to avoid:** Return `axum::http::StatusCode` directly (no body) for 204.
Or use `impl IntoResponse` and return `StatusCode::NO_CONTENT.into_response()`.

### Pitfall 2: Stale cache after handler returns
**What goes wrong:** Calling `refresh_rate_tiers` after the DB write but not
awaiting it — rate_tiers cache stays stale until the 60-second background refresh.
**How to avoid:** Always `.await` the call. All three existing write handlers
already do this correctly.

### Pitfall 3: refresh_rate_tiers silently no-ops on empty result
**What goes wrong:** If `billing_rates` has no active rows (all soft-deleted),
`refresh_rate_tiers` leaves the old tiers in cache (it only writes if `!rows.is_empty()`).
**Warning signs:** Billing continues to charge at old rates after all tiers are deleted.
**How to avoid:** Not a concern for this phase — at least the 3 seed rows are
always present and the success criteria doesn't require deleting all tiers.

### Pitfall 4: Test migration missing billing_rates table
**What goes wrong:** Tests that rely on billing_rates CRUD fail because
`run_test_migrations` in `integration.rs` must include the billing_rates table.
**Current status:** Already present at lines 637–657 of `integration.rs` (added in
Phase 33). No migration gap.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[tokio::test]` + sqlx in-memory SQLite |
| Config file | No separate config — `[dev-dependencies]` in `Cargo.toml` |
| Quick run command | `cargo test -p racecontrol-crate billing_rates` |
| Full suite command | `cargo test -p racecontrol-crate && cargo test -p rc-common && cargo test -p rc-agent-crate` |

### Phase Requirements to Test Map

| Req ID   | Behavior | Test Type | Automated Command | File Exists? |
|----------|----------|-----------|-------------------|-------------|
| ADMIN-01 | GET /billing/rates returns 3 seed rows as JSON | integration (DB layer) | `cargo test -p racecontrol-crate test_billing_rates_get_returns_seed_rows` | ❌ Wave 0 |
| ADMIN-02 | POST inserts new tier, subsequent GET includes it, returns 201 | integration (DB layer) + status check | `cargo test -p racecontrol-crate test_billing_rates_create_inserts_and_cache_updates` | ❌ Wave 0 |
| ADMIN-03 | PUT updates rate, cache reflects new value within 1 billing tick | integration (DB layer) + cache assert | `cargo test -p racecontrol-crate test_billing_rates_update_invalidates_cache` | ❌ Wave 0 |
| ADMIN-04 | DELETE removes tier, compute_session_cost excludes it | unit + integration | `cargo test -p racecontrol-crate test_billing_rates_delete_excludes_from_cost` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p racecontrol-crate billing_rates`
- **Per wave merge:** `cargo test -p racecontrol-crate`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `tests/integration.rs` — add `test_billing_rates_get_returns_seed_rows` block
- [ ] `tests/integration.rs` — add `test_billing_rates_create_inserts_and_cache_updates` block
- [ ] `tests/integration.rs` — add `test_billing_rates_update_invalidates_cache` block
- [ ] `tests/integration.rs` — add `test_billing_rates_delete_excludes_from_cost` block
- No new test infrastructure needed — `create_test_db()` and `create_test_state()` are already available

---

## Implementation Recommendation

### New file vs extend existing
Do NOT create a new file. All changes go into two existing files:

1. `crates/racecontrol/src/api/routes.rs` — patch `create_billing_rate` (return 201)
   and `delete_billing_rate` (return 204).
2. `crates/racecontrol/tests/integration.rs` — add four `#[tokio::test]` functions
   in a new `// ─── Phase 34: Admin Rates API ───` section at the bottom.

### Specific line targets
- `create_billing_rate`: line 1659 — change return type from `Json<Value>` to
  `(axum::http::StatusCode, Json<Value>)`, wrap success arm in
  `(StatusCode::CREATED, Json(...))`.
- `delete_billing_rate`: line 1754 — change return type to
  `axum::http::StatusCode` (drop JSON body), return `StatusCode::NO_CONTENT` on
  success. Keep audit logging. For error case use
  `impl IntoResponse` or log + return 204 anyway (soft deletes rarely fail on
  SQLite).

---

## Existing Gap Summary

| Gap | File | Action |
|-----|------|--------|
| POST returns 200 not 201 | api/routes.rs:1681 | Change return type + wrap in StatusCode::CREATED |
| DELETE returns 200 not 204 | api/routes.rs:1765 | Change return type to StatusCode, return NO_CONTENT |
| No HTTP integration tests for ADMIN-01..04 | tests/integration.rs | Add 4 test functions |
| No unit test for compute_session_cost excluding soft-deleted tier | tests/integration.rs | Add 1 test |

---

## Sources

### Primary (HIGH confidence)
- Direct read of `crates/racecontrol/src/api/routes.rs` lines 1630–1775
- Direct read of `crates/racecontrol/src/billing.rs` lines 1–358
- Direct read of `crates/racecontrol/src/state.rs` (full file)
- Direct read of `crates/racecontrol/src/db/mod.rs` lines 225–260
- Direct read of `crates/racecontrol/tests/integration.rs` lines 1–762
- Direct read of `crates/racecontrol/src/main.rs` (full file)
- Direct read of `crates/racecontrol/Cargo.toml`

### Secondary (N/A)
No external sources were needed — all findings are from direct code inspection.

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — directly observed in Cargo.toml and source files
- Architecture patterns: HIGH — directly observed in routes.rs and billing.rs
- Existing implementation status: HIGH — handlers confirmed present at specific line numbers
- Pitfalls: HIGH — derived from reading actual code paths
- Test infrastructure: HIGH — integration.rs inspected directly

**Research date:** 2026-03-17
**Valid until:** Until api/routes.rs or billing.rs are significantly refactored
