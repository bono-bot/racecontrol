# Architecture Patterns

**Domain:** Cafe inventory, ordering & marketing for an existing gaming center platform
**Researched:** 2026-03-22
**Confidence:** HIGH (built entirely on verified existing codebase patterns)

## Existing Architecture Summary

The racecontrol platform follows a clear pattern:

- **Backend:** Rust/Axum server (`racecontrol` crate, port 8080) with SQLite (sqlx, WAL mode)
- **Admin Dashboard:** Next.js web app (`web/`, port 3200) -- staff/admin facing
- **Customer PWA:** Next.js PWA (`pwa/`, port 3300 via kiosk) -- customer facing
- **POS:** Kiosk mode on pod PCs, authenticates via staff JWT
- **Wallet:** SQLite `wallets` table, balance in paise, atomic debit/credit with `wallet_transactions` audit trail and double-entry `accounting` journal
- **Alerts:** WhatsApp via Evolution API (`whatsapp_alerter.rs`), Email via Gmail OAuth (`email_alerts.rs`)
- **API tiers:** Public (no auth) > Customer (JWT in-handler) > Kiosk (staff JWT) > Staff/Admin (staff JWT + pod source block) > Service (in-handler auth)

## Recommended Architecture

Cafe operations integrate as a **new domain within the existing monolith** -- not a separate service. This follows the established pattern: billing, wallet, reservations, fleet health are all modules within the same `racecontrol` binary sharing `AppState` and the SQLite pool.

### Component Boundaries

| Component | Responsibility | Communicates With |
|-----------|---------------|-------------------|
| `cafe.rs` (Rust) | Menu CRUD, item availability, category management, PDF/spreadsheet import parsing | `state.db` (SQLite), `accounting.rs` |
| `cafe_inventory.rs` (Rust) | Stock tracking, auto-decrement on sale, restock entry, low-stock threshold checks | `state.db`, `cafe_alerts.rs` |
| `cafe_orders.rs` (Rust) | Order creation, validation (stock check + wallet balance), receipt generation, order status | `wallet.rs` (debit), `cafe_inventory.rs` (decrement), `accounting.rs` (journal) |
| `cafe_promos.rs` (Rust) | Combo deals, happy hour rules, gaming+cafe bundles, time-based activation/deactivation | `state.db`, price calculation at order time |
| `cafe_alerts.rs` (Rust) | Low-stock alert firing via WhatsApp + email + dashboard event broadcast | `whatsapp_alerter.rs`, `email_alerts.rs`, `DashboardEvent` broadcast |
| `cafe_marketing.rs` (Rust) | Content generation orchestration -- calls AI for image/text, assembles promo materials | External AI API (or local Ollama), `cafe.rs` (current menu data), `cafe_promos.rs` (active promos) |
| `web/src/app/cafe/` (Next.js) | Admin pages: menu management, inventory view, promo builder, marketing content | Rust API via fetch |
| `pwa/src/app/cafe/` (Next.js) | Customer pages: menu browsing, ordering flow, cart, order history | Rust API via fetch |
| POS integration | Staff-assisted ordering on existing kiosk flow | Kiosk API routes (staff JWT) |

### Data Flow

```
ORDERING FLOW (Customer via PWA):

  PWA /cafe/menu  -->  GET /api/v1/customer/cafe/menu
                       (returns items with availability, active promos applied)
       |
  PWA /cafe/order -->  POST /api/v1/customer/cafe/order
                       |
                       +-- cafe_orders.rs validates:
                       |     1. All items in stock (cafe_inventory.rs)
                       |     2. Wallet balance sufficient (wallet.rs::get_balance)
                       |     3. Promo rules valid (cafe_promos.rs)
                       |
                       +-- On success (atomic):
                       |     1. wallet::debit() -- deducts from RP wallet
                       |     2. cafe_inventory::decrement_stock() -- reduces counts
                       |     3. INSERT into cafe_orders + cafe_order_items
                       |     4. accounting::post_cafe_sale() -- journal entry
                       |     5. Generate receipt number
                       |     6. Broadcast DashboardEvent::CafeOrder for real-time POS/admin
                       |
                       +-- Return: order confirmation + receipt number


ORDERING FLOW (Staff via POS):

  Kiosk/POS  -->  POST /api/v1/kiosk/cafe/order
                  (same flow, but staff JWT + driver_id in body)


INVENTORY ALERT FLOW:

  cafe_inventory::decrement_stock()
       |
       +-- After decrement, check: quantity <= threshold?
       |     YES --> cafe_alerts::fire_low_stock()
       |              +-- WhatsApp to Uday (whatsapp_alerter pattern)
       |              +-- Email to admin (email_alerts pattern)
       |              +-- DashboardEvent::CafeLowStock broadcast
       |     NO  --> done


RESTOCK FLOW (Admin):

  Admin /cafe/inventory  -->  POST /api/v1/staff/cafe/restock
                              (item_id, quantity_added, staff_id)
                              +-- cafe_inventory::restock()
                              +-- audit_log entry
                              +-- Clear low-stock alert if above threshold


PROMO APPLICATION FLOW:

  At order time:
  1. Fetch active promos WHERE now() BETWEEN start_time AND end_time
  2. Check if ordered items match any combo deal
  3. Check if happy hour rule applies (time-of-day)
  4. Check if gaming+cafe bundle applies (active billing session for driver?)
  5. Apply best discount (never stack -- pick most favorable)
  6. Return adjusted prices to caller


MARKETING CONTENT FLOW:

  Admin /cafe/marketing  -->  POST /api/v1/staff/cafe/generate-content
                              |
                              +-- cafe_marketing.rs:
                                    1. Fetch current menu + active promos
                                    2. Call AI (Ollama or cloud) for copy/layout
                                    3. Generate image via template + data
                                    4. Return content for review
                                    5. On approval: POST to WhatsApp broadcast
                                       or save as poster image
```

## Database Schema (New Tables)

All tables go in the existing SQLite database via migration in `db/mod.rs`, following the `CREATE TABLE IF NOT EXISTS` pattern.

```sql
-- Menu items
CREATE TABLE IF NOT EXISTS cafe_items (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    category TEXT NOT NULL,           -- 'beverages', 'snacks', 'meals', etc.
    selling_price_paise INTEGER NOT NULL,
    cost_price_paise INTEGER,         -- for margin tracking
    image_url TEXT,
    is_available BOOLEAN DEFAULT 1,   -- admin toggle
    is_countable BOOLEAN DEFAULT 1,   -- FALSE for made-to-order items (coffee, etc.)
    sort_order INTEGER DEFAULT 0,
    created_at TEXT DEFAULT (datetime('now')),
    updated_at TEXT
);

-- Inventory (only for countable items)
CREATE TABLE IF NOT EXISTS cafe_inventory (
    item_id TEXT PRIMARY KEY REFERENCES cafe_items(id),
    quantity INTEGER NOT NULL DEFAULT 0,
    low_stock_threshold INTEGER DEFAULT 5,
    last_restocked_at TEXT,
    updated_at TEXT DEFAULT (datetime('now'))
);

-- Orders
CREATE TABLE IF NOT EXISTS cafe_orders (
    id TEXT PRIMARY KEY,
    receipt_number TEXT NOT NULL UNIQUE,  -- human-readable: RP-CAFE-0001
    driver_id TEXT NOT NULL REFERENCES drivers(id),
    total_paise INTEGER NOT NULL,
    discount_paise INTEGER DEFAULT 0,
    promo_id TEXT,                        -- which promo applied, if any
    wallet_txn_id TEXT,                   -- links to wallet_transactions.id
    status TEXT DEFAULT 'placed',         -- placed, preparing, ready, delivered, cancelled
    ordered_by TEXT DEFAULT 'customer',   -- 'customer' (PWA) or 'staff' (POS)
    staff_id TEXT,                        -- if staff-assisted
    notes TEXT,                           -- special requests
    created_at TEXT DEFAULT (datetime('now'))
);

-- Order line items
CREATE TABLE IF NOT EXISTS cafe_order_items (
    id TEXT PRIMARY KEY,
    order_id TEXT NOT NULL REFERENCES cafe_orders(id),
    item_id TEXT NOT NULL REFERENCES cafe_items(id),
    quantity INTEGER NOT NULL DEFAULT 1,
    unit_price_paise INTEGER NOT NULL,    -- price at time of order
    subtotal_paise INTEGER NOT NULL,
    created_at TEXT DEFAULT (datetime('now'))
);

-- Stock movements (audit trail for inventory changes)
CREATE TABLE IF NOT EXISTS cafe_stock_movements (
    id TEXT PRIMARY KEY,
    item_id TEXT NOT NULL REFERENCES cafe_items(id),
    movement_type TEXT NOT NULL,          -- 'sale', 'restock', 'adjustment', 'waste'
    quantity_change INTEGER NOT NULL,     -- negative for sales/waste, positive for restock
    quantity_after INTEGER NOT NULL,
    reference_id TEXT,                    -- order_id for sales, NULL for manual
    staff_id TEXT,
    notes TEXT,
    created_at TEXT DEFAULT (datetime('now'))
);

-- Promotions
CREATE TABLE IF NOT EXISTS cafe_promos (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    promo_type TEXT NOT NULL,             -- 'combo', 'happy_hour', 'gaming_bundle'
    config_json TEXT NOT NULL,            -- flexible: items in combo, discount %, time rules
    discount_type TEXT NOT NULL,          -- 'fixed_price', 'percent_off', 'amount_off'
    discount_value INTEGER NOT NULL,      -- paise for fixed/amount, basis points for percent
    start_time TEXT,                      -- NULL = always active
    end_time TEXT,                        -- NULL = no end
    is_active BOOLEAN DEFAULT 1,
    created_at TEXT DEFAULT (datetime('now')),
    updated_at TEXT
);
```

## API Route Structure

Following the existing tiered routing pattern in `routes.rs`:

```rust
// Customer routes (JWT in-handler) -- add to customer_routes()
.route("/customer/cafe/menu", get(cafe_menu))
.route("/customer/cafe/menu/{category}", get(cafe_menu_by_category))
.route("/customer/cafe/order", post(cafe_place_order))
.route("/customer/cafe/orders", get(cafe_order_history))
.route("/customer/cafe/orders/{id}", get(cafe_order_detail))

// Kiosk routes (staff JWT, pod-accessible) -- add to kiosk_routes()
.route("/kiosk/cafe/menu", get(kiosk_cafe_menu))
.route("/kiosk/cafe/order", post(kiosk_cafe_order))

// Staff/Admin routes -- add to staff_routes()
.route("/staff/cafe/items", get(list_cafe_items).post(create_cafe_item))
.route("/staff/cafe/items/{id}", get(get_cafe_item).put(update_cafe_item).delete(delete_cafe_item))
.route("/staff/cafe/items/import", post(import_cafe_items))  -- PDF/spreadsheet
.route("/staff/cafe/items/{id}/availability", put(toggle_item_availability))
.route("/staff/cafe/inventory", get(inventory_dashboard))
.route("/staff/cafe/inventory/{item_id}/restock", post(restock_item))
.route("/staff/cafe/inventory/{item_id}/adjust", post(adjust_stock))
.route("/staff/cafe/orders", get(list_cafe_orders))
.route("/staff/cafe/orders/{id}/status", put(update_order_status))
.route("/staff/cafe/promos", get(list_promos).post(create_promo))
.route("/staff/cafe/promos/{id}", get(get_promo).put(update_promo).delete(delete_promo))
.route("/staff/cafe/marketing/generate", post(generate_marketing_content))
.route("/staff/cafe/marketing/broadcast", post(broadcast_promo))
.route("/staff/cafe/reports/sales", get(cafe_sales_report))
```

## Patterns to Follow

### Pattern 1: Atomic Wallet Debit (existing pattern from billing)
**What:** All cafe orders must use the same `wallet::debit()` function that gaming sessions use. This ensures atomic balance check + deduction, transaction logging, and double-entry accounting.
**Why:** Reusing the wallet module means cafe and gaming balances are unified without any new payment code. The existing `wallet_transactions` table already supports arbitrary `txn_type` values -- just add `cafe_order` as a new type.

### Pattern 2: DashboardEvent Broadcast (existing pattern from pod_monitor)
**What:** New cafe events (`CafeOrder`, `CafeLowStock`, `CafeRestock`) broadcast through the existing `DashboardEvent` channel so the web dashboard gets real-time updates.
**Why:** The admin dashboard already subscribes to the `dashboard_event_tx` broadcast channel. Adding cafe events means the admin sees order flow and stock alerts in real-time without building a separate notification system.

### Pattern 3: Audit via accounting.rs (existing pattern)
**What:** Every cafe sale posts a journal entry via the accounting module. Every menu/inventory change posts to `audit_log`.
**Why:** The accounting module already handles double-entry for gaming sessions. Cafe revenue should appear in the same P&L view.

### Pattern 4: is_countable Flag for Mixed Inventory
**What:** Not all cafe items need stock tracking. Coffee can be made-to-order (unlimited until manually toggled off), while packaged items (Diet Coke, water bottles) are countable with auto-decrement.
**Why:** The project spec calls for stock tracking on "countable items (buns, water bottles, Diet Coke)." Made-to-order items do not need quantity tracking -- only an available/unavailable toggle.

## Anti-Patterns to Avoid

### Anti-Pattern 1: Separate Database
**What:** Creating a separate SQLite file for cafe data.
**Why bad:** Breaks foreign key references to `drivers`, prevents atomic transactions across wallet + cafe tables, doubles connection pool overhead.
**Instead:** Add cafe tables to the existing `racecontrol.db` via the existing migration function in `db/mod.rs`.

### Anti-Pattern 2: Microservice for Cafe
**What:** Running cafe as a separate HTTP service.
**Why bad:** The platform is a monolith by design. All 40+ Rust modules share `AppState` and the same SQLite pool. A separate service would need IPC for wallet deductions, duplicate auth, and complicate deployment to the server (.23).
**Instead:** Add `cafe*.rs` modules to the `racecontrol` crate, same as every other feature.

### Anti-Pattern 3: Cart Stored Server-Side
**What:** Persisting shopping cart state in the database.
**Why bad:** Cafe ordering is low-volume, in-venue only. Cart state adds abandoned-cart cleanup complexity for no benefit.
**Instead:** Cart is client-side only (PWA state / POS state). Server only sees the final order POST.

### Anti-Pattern 4: Complex Promo Stacking
**What:** Allowing multiple promotions to stack on a single order.
**Why bad:** Creates edge cases (combo + happy hour + gaming bundle = negative price?), hard to audit.
**Instead:** Apply the single best discount. `config_json` on the promo is flexible enough for any rule shape; the order logic picks the most favorable one.

### Anti-Pattern 5: Receipt as Separate Service
**What:** Building a receipt microservice or using a third-party receipt API.
**Why bad:** The POS already has thermal printer access. Receipts are just formatted text.
**Instead:** Generate receipt text server-side (or client-side on POS), send to the existing printer infrastructure.

## Component Dependency Graph and Build Order

```
Phase 1: Data Foundation (no dependencies on other new components)
  cafe_items table + cafe.rs CRUD + admin UI for menu management
  cafe_items import (PDF/spreadsheet parsing)
  |
Phase 2: Inventory (depends on Phase 1 items)
  cafe_inventory table + cafe_inventory.rs
  cafe_stock_movements table (audit)
  Admin inventory UI (stock view, restock, adjust)
  cafe_alerts.rs (low-stock via WhatsApp/email/dashboard)
  |
Phase 3: Ordering (depends on Phase 1 items + Phase 2 inventory)
  cafe_orders + cafe_order_items tables
  cafe_orders.rs (validates stock, debits wallet, decrements inventory)
  PWA ordering flow (menu browse, cart, place order)
  POS/kiosk ordering flow (staff-assisted)
  Receipt generation
  |
Phase 4: Promos (depends on Phase 1 items, used by Phase 3 ordering)
  cafe_promos table + cafe_promos.rs
  Admin promo builder UI
  Promo display in PWA/POS menu
  Promo application in order flow (retroactive integration into Phase 3)
  |
Phase 5: Marketing (depends on Phase 1 items + Phase 4 promos)
  cafe_marketing.rs (content generation)
  Admin marketing UI (generate, preview, broadcast)
  WhatsApp broadcast integration
  Digital poster generation
```

**Build order rationale:**
- Phase 1 (Menu) is zero-dependency -- everything else needs items to exist first
- Phase 2 (Inventory) only needs items. Alert infrastructure reuses existing WhatsApp/email modules
- Phase 3 (Ordering) is the core value delivery -- requires both menu and inventory
- Phase 4 (Promos) can ship after basic ordering works. Retroactively integrates into the order price calculation
- Phase 5 (Marketing) is the most independent -- only needs menu/promo data as input. Can run in parallel with Phase 4 if needed

## Scalability Considerations

| Concern | Current Scale (1 cafe) | If 5x Volume |
|---------|----------------------|--------------|
| Order throughput | SQLite WAL handles 100+ writes/sec -- cafe will do ~50 orders/day | Still fine. SQLite limit is ~1000 writes/sec |
| Stock decrement races | Atomic UPDATE SET quantity = quantity - 1 WHERE quantity >= 1 | Same pattern works. SQLite serializes writes |
| Receipt numbering | Sequential counter in settings table (RP-CAFE-0001) | Atomic increment, no contention at this scale |
| Menu data | In-memory cache refreshed on mutation. ~50-100 items | HashMap in AppState, same pattern as billing_rates cache |
| WhatsApp alerts | Rate-limited per item (same cooldown pattern as pod alerts) | Add per-item cooldown to prevent alert storms |

## Integration Points with Existing Systems

| Existing System | Integration | Effort |
|----------------|-------------|--------|
| Wallet (`wallet.rs`) | Call `wallet::debit()` with txn_type "cafe_order" | LOW -- function already accepts arbitrary txn_type |
| Accounting (`accounting.rs`) | Add `post_cafe_sale()` -- same pattern as `post_session_charge()` | LOW -- copy+adapt existing pattern |
| WhatsApp (`whatsapp_alerter.rs`) | Add low-stock alert message type | LOW -- reuse `send_whatsapp_message()` |
| Email (`email_alerts.rs`) | Add low-stock email template | LOW -- reuse existing `EmailAlerter` |
| Dashboard events | Add `CafeOrder`, `CafeLowStock` variants to `DashboardEvent` enum | LOW -- enum extension in `rc-common` |
| Admin dashboard (`web/`) | Add `/cafe` route group with menu, inventory, orders, promos, marketing pages | MEDIUM -- new pages but existing layout/auth patterns |
| PWA (`pwa/`) | Add `/cafe` route group with menu browsing and ordering | MEDIUM -- new pages but existing layout/auth patterns |
| POS/Kiosk | Add cafe tab to existing kiosk flow, reuse staff JWT auth | MEDIUM -- extends existing kiosk UI |
| Cloud sync | Cafe menu/promos sync to cloud (cloud authoritative for menu, local for orders) | LOW-MEDIUM -- extend existing `cloud_sync.rs` |

## Sources

- Verified against existing codebase: `crates/racecontrol/src/wallet.rs`, `billing.rs`, `accounting.rs`, `whatsapp_alerter.rs`, `email_alerts.rs`, `api/routes.rs`, `db/mod.rs`, `state.rs`
- Existing API tier pattern from `routes.rs` lines 34-49 (5-tier auth model)
- SQLite WAL + atomic UPDATE pattern from `wallet.rs` line 159 (TOCTOU prevention)
- DashboardEvent broadcast pattern from `state.rs` and `pod_monitor.rs`
