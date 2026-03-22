# Phase 149: Menu Data Model & CRUD - Research

**Researched:** 2026-03-22
**Domain:** SQLite schema (sqlx), Rust/Axum CRUD module, Next.js admin UI with side-panel form
**Confidence:** HIGH — all findings verified directly against existing codebase source

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Data Model & Schema**
- Store cafe items in the existing SQLite database — add `cafe_items` and `cafe_categories` tables via `db/mod.rs` migrate() pattern (`CREATE TABLE IF NOT EXISTS`)
- Categories managed via separate `cafe_categories` table with id, name, sort_order — admin can add/reorder without code changes
- Prices stored as integer paise (i64) — ₹149.50 = 14950. Matches existing `wallet_debit_paise` pattern in billing
- Item IDs use UUID v4 (String) — matches existing `drivers.id` pattern

**Admin UI Approach**
- New route `/cafe` in existing web dashboard at :3200 — follows pattern of `/cameras`, `/drivers`, `/billing`
- Side panel form for add/edit — click "Add Item" → slide-in panel, keeps item table visible
- Category dropdown with inline "Add Category" — select existing or type new to auto-create
- Table display with sortable columns (name, category, price, stock, status) — density-efficient for 50+ items

**API Design**
- Route prefix: `/api/v1/cafe/` — items, categories as sub-resources
- Admin-only auth for CRUD endpoints (JWT with admin role). Customer-facing menu read is public
- Paginated JSON responses: `{ items: [...], total: N, page: N }`

### Claude's Discretion
- Exact table column names and index strategy
- Form validation rules and error message wording
- Table column widths and sort defaults

### Deferred Ideas (OUT OF SCOPE)
None — discussion stayed within phase scope.
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| MENU-02 | Admin can manually add cafe items with name, description, category, selling price, and cost price | Covered by cafe.rs create_item handler + side-panel form in /cafe page |
| MENU-03 | Admin can edit existing cafe item details (name, description, category, prices) | Covered by cafe.rs update_item handler + PUT /api/v1/cafe/items/:id |
| MENU-04 | Admin can delete cafe items | Covered by cafe.rs delete_item handler + DELETE /api/v1/cafe/items/:id |
| MENU-05 | Admin can toggle item availability (available/unavailable — hides from POS/PWA) | Covered by is_available BOOLEAN column + PUT toggle endpoint |
</phase_requirements>

---

## Summary

Phase 149 creates the cafe menu data layer on top of the existing `racecontrol` Rust/Axum backend. The codebase uses a well-established pattern: SQLite tables added to `db/mod.rs`'s `migrate()` function, a flat Rust module (`cafe.rs`) with sqlx queries and Axum handlers, route registration in `api/routes.rs`, module declaration in `lib.rs`, and a Next.js page under `web/src/app/cafe/`. All of these patterns are in active use across drivers, billing, wallet, and cameras pages.

The key design points locked by CONTEXT.md are: paise integer prices (matching `pricing_tiers.price_paise`), UUID v4 string IDs (matching `drivers.id`), a separate `cafe_categories` table, admin JWT-protected CRUD under `/api/v1/cafe/`, and a slide-in side panel UI on the `/cafe` dashboard page. None of these require new libraries — they are direct extensions of the existing stack.

The only discretion area is column naming, index selection, and UI validation wording. Recommendations are provided below based on conventions observed in the existing schema.

**Primary recommendation:** Model `cafe.rs` after the billing module pattern — `#[derive(Serialize, Deserialize, sqlx::FromRow)]` structs, `sqlx::query_as!` macros, `Arc<AppState>` state injection, `anyhow::Result` returns. The UI should follow the `billing/pricing/page.tsx` pattern (inline edit state, API calls, DashboardLayout wrapper).

---

## Standard Stack

### Core (all already in Cargo.toml — no new dependencies)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| sqlx | existing | SQLite queries, `FromRow` derive, query_as! macro | Already used for all DB access in the codebase |
| uuid | existing | UUID v4 generation for item IDs | Used for `drivers.id`, `billing_sessions.id`, etc. |
| serde / serde_json | existing | JSON serialization of API types | Used on every API struct in the codebase |
| axum | existing | HTTP routing, State/Path/Json extractors | Entire API is Axum — same handler pattern throughout |
| anyhow | existing | `Result<T>` error propagation | Used in wallet.rs, billing.rs, all fallible ops |
| tracing | existing | `info!`/`warn!` structured logging | Used in every module |

### Frontend (already in web package — no new dependencies)

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| React (useState, useEffect) | existing | Local form state, data fetching | All dashboard pages |
| next/navigation | existing | Route grouping under /cafe | Every page uses DashboardLayout |
| Tailwind CSS | existing | RP brand classes (rp-red, rp-card, rp-border) | All UI components |

**Installation:** No new packages required. Phase 149 uses only what is already declared.

---

## Architecture Patterns

### Recommended Project Structure

```
crates/racecontrol/src/
├── cafe.rs              # New: CafeItem / CafeCategory types + all CRUD handlers
├── api/routes.rs        # Modified: register cafe routes in staff_routes() + public menu route
├── lib.rs               # Modified: add `pub mod cafe;`
└── db/mod.rs            # Modified: append cafe_categories + cafe_items CREATE TABLE IF NOT EXISTS

web/src/
├── app/cafe/
│   └── page.tsx         # New: admin table + slide-in side panel
├── lib/api.ts           # Modified: add cafe API methods
└── components/Sidebar.tsx # Modified: add /cafe nav entry
```

### Pattern 1: SQLite Table Registration (migrate() append)

**What:** Every table in the codebase is created by appending a `sqlx::query("CREATE TABLE IF NOT EXISTS ...").execute(pool).await?` call inside `db/mod.rs`'s private `migrate()` function. This is purely additive — existing tables are never altered.
**When to use:** Any new persistent entity.

```rust
// Source: crates/racecontrol/src/db/mod.rs (verified)
// Pattern: every table appended in migrate() with IF NOT EXISTS
sqlx::query(
    "CREATE TABLE IF NOT EXISTS cafe_categories (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL UNIQUE,
        sort_order INTEGER DEFAULT 0,
        created_at TEXT DEFAULT (datetime('now'))
    )",
)
.execute(pool)
.await?;

sqlx::query(
    "CREATE TABLE IF NOT EXISTS cafe_items (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        description TEXT,
        category_id TEXT NOT NULL REFERENCES cafe_categories(id),
        selling_price_paise INTEGER NOT NULL,
        cost_price_paise INTEGER NOT NULL,
        is_available BOOLEAN DEFAULT 1,
        created_at TEXT DEFAULT (datetime('now')),
        updated_at TEXT
    )",
)
.execute(pool)
.await?;

sqlx::query("CREATE INDEX IF NOT EXISTS idx_cafe_items_category ON cafe_items(category_id)")
    .execute(pool)
    .await?;
sqlx::query("CREATE INDEX IF NOT EXISTS idx_cafe_items_available ON cafe_items(is_available)")
    .execute(pool)
    .await?;
```

### Pattern 2: Rust Handler Module (flat module)

**What:** New modules live as a single flat `.rs` file in `crates/racecontrol/src/`. Types derive `Serialize`, `Deserialize`, and `sqlx::FromRow`. All functions accept `Arc<AppState>` (via Axum `State` extractor) and return `Result<Json<T>, StatusCode>`.
**When to use:** Any new API feature.

```rust
// Source: crates/racecontrol/src/wallet.rs and billing.rs (verified pattern)
use std::sync::Arc;
use axum::{extract::{Path, State}, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use crate::state::AppState;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct CafeItem {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub category_id: String,
    pub selling_price_paise: i64,
    pub cost_price_paise: i64,
    pub is_available: bool,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateCafeItemRequest {
    pub name: String,
    pub description: Option<String>,
    pub category_id: String,
    pub selling_price_paise: i64,
    pub cost_price_paise: i64,
}

pub async fn list_cafe_items(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let items = sqlx::query_as::<_, CafeItem>(
        "SELECT id, name, description, category_id, selling_price_paise, cost_price_paise,
                is_available, created_at, updated_at
         FROM cafe_items ORDER BY name ASC"
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| { tracing::warn!("list_cafe_items DB error: {}", e); StatusCode::INTERNAL_SERVER_ERROR })?;

    let total = items.len();
    Ok(Json(serde_json::json!({ "items": items, "total": total })))
}
```

### Pattern 3: Route Registration (staff_routes)

**What:** New routes are appended to the appropriate tier function in `api/routes.rs`. CRUD endpoints go in `staff_routes()` which applies `require_staff_jwt` + `require_non_pod_source`. The customer-facing read endpoint for available items goes in `public_routes()`.
**When to use:** All cafe CRUD handlers.

```rust
// Source: crates/racecontrol/src/api/routes.rs — staff_routes() function (verified)
// Append inside fn staff_routes():
.route("/cafe/items", get(cafe::list_cafe_items).post(cafe::create_cafe_item))
.route("/cafe/items/:id", put(cafe::update_cafe_item).delete(cafe::delete_cafe_item))
.route("/cafe/items/:id/toggle", post(cafe::toggle_cafe_item_availability))
.route("/cafe/categories", get(cafe::list_cafe_categories).post(cafe::create_cafe_category))

// In public_routes() — customer-facing read (no auth):
.route("/cafe/menu", get(cafe::public_menu))
```

### Pattern 4: Next.js Admin Page with Side Panel

**What:** The `/cafe/page.tsx` file uses `DashboardLayout` wrapper, `useState` for items, a side panel controlled by a boolean `showPanel` state, and an edit state holding the item being edited (null = add mode, non-null = edit mode). Follows `billing/pricing/page.tsx` structure.
**When to use:** Any admin CRUD page.

```tsx
// Source: web/src/app/billing/pricing/page.tsx (verified pattern, adapted)
"use client";
import { useEffect, useState } from "react";
import DashboardLayout from "@/components/DashboardLayout";
import { api } from "@/lib/api";

export default function CafePage() {
  const [items, setItems] = useState<CafeItem[]>([]);
  const [categories, setCategories] = useState<CafeCategory[]>([]);
  const [showPanel, setShowPanel] = useState(false);
  const [editItem, setEditItem] = useState<CafeItem | null>(null); // null = add mode

  // fetch, handleCreate, handleUpdate, handleDelete, handleToggle...

  return (
    <DashboardLayout>
      <div className="flex gap-6">
        {/* Item table — always visible */}
        <div className="flex-1">...</div>
        {/* Side panel — slides in on add/edit */}
        {showPanel && (
          <div className="w-80 bg-rp-card border border-rp-border rounded-lg p-4">
            ...form...
          </div>
        )}
      </div>
    </DashboardLayout>
  );
}
```

### Pattern 5: lib.rs Module Declaration

**What:** Every module in `crates/racecontrol/src/` must be declared with `pub mod` in `lib.rs`.

```rust
// Source: crates/racecontrol/src/lib.rs line 40 area (verified)
// Append to lib.rs:
pub mod cafe;
```

### Anti-Patterns to Avoid

- **Separate migration file:** Do not create a separate `migrations/` directory or use `sqlx::migrate!` macro. The codebase uses the inline `migrate()` function in `db/mod.rs`. Using a different mechanism would break the single-file migration pattern.
- **unwrap() in handlers:** Per CLAUDE.md standing rules, no `.unwrap()` in production Rust. Use `?` operator or `.map_err(|e| StatusCode::INTERNAL_SERVER_ERROR)`.
- **any in TypeScript:** All API types must be typed explicitly. Define `CafeItem` and `CafeCategory` interfaces in `web/src/lib/api.ts`.
- **sessionStorage in useState initializer:** Per CLAUDE.md, never read `sessionStorage`/`localStorage` in `useState` — use `useEffect` + hydrated flag. (Relevant if session persistence is added later.)
- **Separate category_name TEXT column on cafe_items:** The decision is to use a FK to `cafe_categories`. Do not add a redundant `category_name` TEXT column — join or include category name in API response from a JOIN query.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| UUID generation | Custom ID generator | `uuid::Uuid::new_v4().to_string()` | Already used for drivers.id, billing_sessions.id |
| Paise formatting | Custom currency formatter | `(paise as f64 / 100.0)` or frontend `paise / 100` | Same pattern used in billing/pricing page |
| JWT validation | Custom auth middleware | `require_staff_jwt` extractor from `auth::middleware` | Already implemented, handles 401 correctly |
| DB connection pool | Manual pool management | `state.db` from `Arc<AppState>` | Pool is initialized in `db::init_pool` at server start |
| Paginated list response | Custom pagination | `{ items: [...], total: N }` JSON shape | Matches CONTEXT.md spec, simple enough at current scale |

**Key insight:** Every piece of infrastructure this phase needs already exists in the codebase. The work is purely domain logic — schema + handlers + UI.

---

## Common Pitfalls

### Pitfall 1: BOOLEAN columns return as i64 in sqlx
**What goes wrong:** SQLite stores BOOLEAN as INTEGER (0/1). `sqlx::FromRow` maps them to `i64` unless the Rust field type is explicitly `bool`. If the struct has `pub is_available: i64` instead of `pub is_available: bool`, the JSON serialization sends `0`/`1` instead of `true`/`false`, and the frontend toggle logic breaks.
**Why it happens:** SQLite has no native boolean type. sqlx infers the Rust type from the struct field declaration.
**How to avoid:** Declare the field as `pub is_available: bool` in the `CafeItem` struct. Confirmed: `billing_sessions.is_trial` uses `bool` in existing types.
**Warning signs:** Frontend receives `0` or `1` for a boolean field instead of `true`/`false`.

### Pitfall 2: Foreign key constraint on category_id
**What goes wrong:** `PRAGMA foreign_keys=ON` is set in `migrate()`. Inserting a `cafe_item` with a non-existent `category_id` returns a DB error that propagates as HTTP 500 instead of a user-friendly 400.
**Why it happens:** FK enforcement is enabled globally. The create handler must verify the category exists before inserting the item, or return a 400 with a clear error.
**How to avoid:** In `create_cafe_item`, first check `SELECT id FROM cafe_categories WHERE id = ?` — if not found, return `StatusCode::BAD_REQUEST` with a JSON error body.
**Warning signs:** 500 errors on item creation when category_id is invalid.

### Pitfall 3: Category name uniqueness constraint
**What goes wrong:** `cafe_categories.name` has a `UNIQUE` constraint. If the "Add Category" inline flow tries to create a duplicate, sqlx returns `SqliteError: UNIQUE constraint failed`. The handler must distinguish this from generic DB errors and return a 409 Conflict or silently return the existing category.
**Why it happens:** UNIQUE constraint enforcement is automatic at DB level.
**How to avoid:** Use `INSERT OR IGNORE INTO cafe_categories` and then `SELECT id FROM cafe_categories WHERE name = ?` — ensures idempotent category creation. This is the correct pattern for the "type new to auto-create" flow.
**Warning signs:** 500 on duplicate category name entry.

### Pitfall 4: Missing `updated_at` on PATCH/PUT
**What goes wrong:** `cafe_items.updated_at` is nullable (matches `drivers.updated_at` pattern). If the UPDATE handler does not set `updated_at = datetime('now')`, the column stays NULL forever, making change tracking useless.
**Why it happens:** SQLite has no automatic `ON UPDATE` trigger by default.
**How to avoid:** Always include `updated_at = datetime('now')` in UPDATE queries: `UPDATE cafe_items SET name = ?, ..., updated_at = datetime('now') WHERE id = ?`

### Pitfall 5: Public menu endpoint must filter is_available
**What goes wrong:** If `GET /api/v1/cafe/menu` returns all items (including unavailable ones), the customer-facing PWA and POS will show items that are toggled off.
**Why it happens:** Easy to forget the WHERE clause when copy-pasting from the admin list handler.
**How to avoid:** The public menu endpoint MUST have `WHERE is_available = 1`. The admin list endpoint returns all items (for management). Two separate queries for two separate routes.
**Warning signs:** Unavailable items appear on PWA/POS after toggling off in admin.

### Pitfall 6: Next.js hydration on admin auth token
**What goes wrong:** If `api.ts`'s `getToken()` reads `sessionStorage` synchronously during SSR, the `/cafe` page hydration mismatches. This is a known codebase rule.
**Why it happens:** `sessionStorage` is not available server-side.
**How to avoid:** The existing `fetchApi` function in `web/src/lib/api.ts` already calls `getToken()` which (verified) reads `sessionStorage` inside an event handler, not during render. Adding new API methods to `api.ts` following the same pattern is safe.

---

## Code Examples

Verified patterns from existing codebase source files:

### UUID v4 generation (from wallet.rs)
```rust
// Source: crates/racecontrol/src/wallet.rs (uses uuid::Uuid::new_v4())
let id = uuid::Uuid::new_v4().to_string();
```

### sqlx query_as pattern (from wallet.rs)
```rust
// Source: crates/racecontrol/src/wallet.rs (verified pattern)
let row = sqlx::query_as::<_, (i64,)>(
    "SELECT balance_paise FROM wallets WHERE driver_id = ?",
)
.bind(driver_id)
.fetch_optional(&state.db)
.await
.map_err(|e| format!("DB error: {}", e))?;
```

### Paise price display in frontend (from billing/pricing/page.tsx)
```tsx
// Source: web/src/app/billing/pricing/page.tsx line 8 (verified)
const formatCredits = (paise: number) => `${Math.floor(paise / 100)} cr`;
// For cafe, show rupees: const formatRupees = (paise: number) => `₹${(paise / 100).toFixed(2)}`
```

### fetchApi call pattern (from web/src/lib/api.ts)
```typescript
// Source: web/src/lib/api.ts lines 30-57 (verified)
// Add to the api object:
listCafeItems: () => fetchApi<{ items: CafeItem[]; total: number }>("/cafe/items"),
createCafeItem: (data: CreateCafeItemRequest) =>
  fetchApi<{ id: string }>("/cafe/items", { method: "POST", body: JSON.stringify(data) }),
updateCafeItem: (id: string, data: Partial<CreateCafeItemRequest>) =>
  fetchApi<CafeItem>(`/cafe/items/${id}`, { method: "PUT", body: JSON.stringify(data) }),
deleteCafeItem: (id: string) =>
  fetchApi<{ ok: boolean }>(`/cafe/items/${id}`, { method: "DELETE" }),
toggleCafeItem: (id: string) =>
  fetchApi<{ is_available: boolean }>(`/cafe/items/${id}/toggle`, { method: "POST" }),
listCafeCategories: () => fetchApi<{ categories: CafeCategory[] }>("/cafe/categories"),
createCafeCategory: (name: string) =>
  fetchApi<{ id: string; name: string }>("/cafe/categories", {
    method: "POST",
    body: JSON.stringify({ name }),
  }),
```

### Sidebar nav entry pattern (from web/src/components/Sidebar.tsx)
```tsx
// Source: web/src/components/Sidebar.tsx lines 6-25 (verified)
// Append to nav array:
{ href: "/cafe", label: "Cafe Menu", icon: "&#9749;" },
```

---

## Recommended Column Names (Claude's Discretion)

Based on naming conventions observed in the existing schema:

**cafe_categories table:**
| Column | Type | Notes |
|--------|------|-------|
| `id` | TEXT PRIMARY KEY | UUID v4 |
| `name` | TEXT NOT NULL UNIQUE | Display name |
| `sort_order` | INTEGER DEFAULT 0 | Admin-controlled ordering |
| `created_at` | TEXT DEFAULT datetime('now') | ISO timestamp |

**cafe_items table:**
| Column | Type | Notes |
|--------|------|-------|
| `id` | TEXT PRIMARY KEY | UUID v4 |
| `name` | TEXT NOT NULL | Item display name |
| `description` | TEXT | Nullable, matches avatar_url pattern on drivers |
| `category_id` | TEXT NOT NULL REFERENCES cafe_categories(id) | FK, NOT NULL |
| `selling_price_paise` | INTEGER NOT NULL | ₹ × 100, matches price_paise in pricing_tiers |
| `cost_price_paise` | INTEGER NOT NULL | For margin tracking |
| `is_available` | BOOLEAN DEFAULT 1 | Toggle; 0 = hidden from POS/PWA |
| `created_at` | TEXT DEFAULT datetime('now') | ISO timestamp |
| `updated_at` | TEXT | NULL until first edit, matches drivers.updated_at |

**Indexes:** `idx_cafe_items_category` on `category_id` (for grouped menu queries in Phase 151), `idx_cafe_items_available` on `is_available` (for public menu filter).

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Manual SQL string building | sqlx parameterized queries with `?` binding | Baseline | Prevents SQL injection |
| Separate migration files | Inline `migrate()` in db/mod.rs | Baseline | Single migration point, no migration runner needed |
| `unwrap()` in handlers | `?` / `map_err` / anyhow::Result | CLAUDE.md rule | Prevents service panics on DB errors |

**Deprecated/outdated:**
- `orange #FF4400`: CLAUDE.md explicitly says this is DEPRECATED. Use `#E10600` (rp-red) for any UI accent colors.

---

## Open Questions

1. **Category seeding**
   - What we know: `cafe_categories` is user-managed, not code-managed. No default categories are locked.
   - What's unclear: Should `migrate()` seed any starter categories (e.g., "Beverages", "Snacks") or leave the table empty?
   - Recommendation: Seed 3 default categories via `INSERT OR IGNORE` in `migrate()` — same pattern as `pricing_tiers`. Planner should decide names; Claude's discretion applies here.

2. **Category delete behavior**
   - What we know: `cafe_items.category_id` is a NOT NULL FK to `cafe_categories.id`. Deleting a category with items would violate FK.
   - What's unclear: Phase 149 scope covers item CRUD, not category delete. Should a DELETE /cafe/categories/:id endpoint be included?
   - Recommendation: Include the endpoint but return 409 if any items reference the category. Prevents orphaned items.

3. **Paginated vs full-list response for admin**
   - What we know: CONTEXT.md specifies `{ items: [...], total: N, page: N }` pagination. For Phase 149 (foundation), item count is likely < 100.
   - What's unclear: Is actual server-side pagination (LIMIT/OFFSET) required in Phase 149, or just the response shape?
   - Recommendation: Implement the response shape with `total` but no LIMIT/OFFSET in Phase 149. Add pagination parameters in Phase 151 when POS display is built. This matches the pattern in `/drivers` which returns all records.

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in test + sqlx in-memory SQLite |
| Config file | none (cargo test runs automatically) |
| Quick run command | `cargo test -p racecontrol cafe` |
| Full suite command | `cargo test -p racecontrol && cargo test -p rc-common` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| MENU-02 | Create item with all fields persists to DB | unit | `cargo test -p racecontrol cafe::tests::test_create_item` | ❌ Wave 0 |
| MENU-03 | Update item fields reflected in DB read | unit | `cargo test -p racecontrol cafe::tests::test_update_item` | ❌ Wave 0 |
| MENU-04 | Deleted item not found in list query | unit | `cargo test -p racecontrol cafe::tests::test_delete_item` | ❌ Wave 0 |
| MENU-05 | Toggle sets is_available=0; excluded from public_menu query | unit | `cargo test -p racecontrol cafe::tests::test_toggle_availability` | ❌ Wave 0 |
| MENU-02 | POST /api/v1/cafe/items returns 201 with id | integration | `cargo test -p racecontrol integration` (extend integration.rs) | ❌ Wave 0 |
| MENU-05 | GET /api/v1/cafe/menu excludes unavailable items | integration | `cargo test -p racecontrol integration` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p racecontrol cafe`
- **Per wave merge:** `cargo test -p racecontrol && cargo test -p rc-common`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `#[cfg(test)] mod tests` block inside `crates/racecontrol/src/cafe.rs` — covers MENU-02, MENU-03, MENU-04, MENU-05
- [ ] Extend `crates/racecontrol/tests/integration.rs` with `run_test_migrations` cafe tables — covers HTTP-level MENU-02, MENU-05
- [ ] Framework install: none needed — Rust test framework is built-in; sqlx in-memory pattern already used in `integration.rs`

*(Existing `integration.rs` uses `create_test_db()` + in-memory SQLite. Cafe tests must replicate the same pattern and add cafe table SQL to `run_test_migrations`.)*

---

## Sources

### Primary (HIGH confidence)
- `crates/racecontrol/src/db/mod.rs` — Full migrate() schema, all table patterns, PRAGMA settings, WAL mode
- `crates/racecontrol/src/wallet.rs` — Paise pattern, sqlx query_as binding, Arc<AppState> usage
- `crates/racecontrol/src/api/routes.rs` — Route registration tiers, staff_routes/public_routes structure
- `crates/racecontrol/src/auth/middleware.rs` — StaffClaims struct, require_staff_jwt verification
- `crates/racecontrol/src/lib.rs` — pub mod declaration pattern
- `crates/racecontrol/src/state.rs` — AppState fields (db: SqlitePool, config: Config)
- `crates/racecontrol/tests/integration.rs` — In-memory test DB pattern, run_test_migrations structure
- `web/src/lib/api.ts` — fetchApi pattern, api object methods, auth token handling
- `web/src/app/billing/pricing/page.tsx` — Inline CRUD admin page pattern, paise formatting
- `web/src/app/drivers/page.tsx` — DashboardLayout wrapper, useEffect fetch, loading state
- `web/src/components/Sidebar.tsx` — Nav array pattern for new route entry
- `.planning/phases/149-menu-data-model-crud/149-CONTEXT.md` — All locked decisions

### Secondary (MEDIUM confidence)
- CLAUDE.md — Project rules: no unwrap, no any, rp-red color, paise convention
- `.planning/REQUIREMENTS-v19.md` — MENU-02 through MENU-05 requirement wording

### Tertiary (LOW confidence)
- None

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all libraries verified in Cargo.toml and active use
- Architecture: HIGH — patterns verified line-by-line in existing source files
- Schema design: HIGH — column conventions verified against db/mod.rs existing tables
- Pitfalls: HIGH — most come from observable behavior in existing code (FK enforcement, BOOLEAN/i64, etc.)
- UI patterns: HIGH — pricing/page.tsx and drivers/page.tsx verified directly

**Research date:** 2026-03-22
**Valid until:** 2026-04-22 (stable stack, 30-day window)
