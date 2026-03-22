# Phase 149: Menu Data Model & CRUD - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Cafe item data model (SQLite schema), Rust CRUD module, REST API endpoints, and admin dashboard UI for managing cafe menu items. This phase creates the foundation that all subsequent cafe phases depend on — items must exist before import, display, inventory, or ordering can work.

</domain>

<decisions>
## Implementation Decisions

### Data Model & Schema
- Store cafe items in the existing SQLite database — add `cafe_items` and `cafe_categories` tables via `db/mod.rs` migrate() pattern (`CREATE TABLE IF NOT EXISTS`)
- Categories managed via separate `cafe_categories` table with id, name, sort_order — admin can add/reorder without code changes
- Prices stored as integer paise (i64) — ₹149.50 = 14950. Matches existing `wallet_debit_paise` pattern in billing
- Item IDs use UUID v4 (String) — matches existing `drivers.id` pattern

### Admin UI Approach
- New route `/cafe` in existing web dashboard at :3200 — follows pattern of `/cameras`, `/drivers`, `/billing`
- Side panel form for add/edit — click "Add Item" → slide-in panel, keeps item table visible
- Category dropdown with inline "Add Category" — select existing or type new to auto-create
- Table display with sortable columns (name, category, price, stock, status) — density-efficient for 50+ items

### API Design
- Route prefix: `/api/v1/cafe/` — items, categories as sub-resources
- Admin-only auth for CRUD endpoints (JWT with admin role). Customer-facing menu read is public
- Paginated JSON responses: `{ items: [...], total: N, page: N }`

### Claude's Discretion
- Exact table column names and index strategy
- Form validation rules and error message wording
- Table column widths and sort defaults

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/racecontrol/src/db/mod.rs` — SQLite pool init, WAL mode, migrate() pattern with `CREATE TABLE IF NOT EXISTS`
- `crates/racecontrol/src/wallet.rs` — wallet module with `get_balance`, `debit`, `refund` (paise-based)
- `crates/racecontrol/src/api/routes.rs` — Axum route registration pattern
- `crates/racecontrol/src/auth.rs` — JWT auth middleware with role-based access
- `web/src/app/` — Next.js route folders (cameras/, drivers/, billing/) as UI pattern reference

### Established Patterns
- Flat module organization: new `cafe.rs` in `crates/racecontrol/src/`
- `Arc<AppState>` shared state with `SqlitePool` accessible via `state.pool`
- `anyhow::Result<T>` for all fallible operations
- `tracing::info!` / `tracing::warn!` for logging
- `serde::Serialize`/`Deserialize` on all API types

### Integration Points
- `db/mod.rs` migrate() — append new CREATE TABLE statements
- `api/routes.rs` — register new cafe routes under `/api/v1/cafe/`
- `lib.rs` — add `pub mod cafe;`
- `web/src/app/cafe/` — new Next.js route folder for admin UI

</code_context>

<specifics>
## Specific Ideas

No specific requirements — follow established codebase conventions and patterns.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
