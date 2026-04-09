# Phase 346: Cafe Menu Proxy Rewrite (SSOT) — Context

**Milestone:** v47.0 Admin Dashboard Venue-Ready Hardening
**Wave:** 1
**Status:** Scaffolding committed (flag-gated); cutover deferred to maintenance window
**Depends on:** Phase 345 (withAdminDbError helper)
**Blocks:** Phase 350 (contract test will gate the cutover)

## Why this phase exists

The 2026-04-09 Admin audit found that admin cafe menu editor writes to
`admin.db.menu_items` but POS `/billing` and kiosk read
`racecontrol.db.cafe_items`. These are two completely disconnected tables.
Staff edit menu items in admin, and nothing reaches POS or kiosk. Customers
see the old menu forever.

Fix: rewrite admin cafe routes to proxy to racecontrol `/api/v1/cafe/items`,
which is the canonical store read by POS + kiosk.

## CRITICAL SCHEMA DIFF (DO NOT SKIP)

| admin.db.menu_items | racecontrol.db.cafe_items | Risk |
|---|---|---|
| `id` INTEGER AUTOINCREMENT | `id` TEXT (UUID) | UI currently uses numeric ids — string ids may break edit flows |
| `category` TEXT (free-form string) | `category_id` TEXT FK to cafe_categories.id | admin must resolve/create category_id before write |
| `name` TEXT | `name` TEXT | — |
| **`price` INTEGER (rupees — e.g. 199 = ₹199)** | **`selling_price_paise` i64 (paise — e.g. 19900 = ₹199)** | **P0 — naive proxy divides prices by 100 on POS** |
| `veg` INTEGER 0/1 | (no equivalent) | veg flag lost unless added to rc schema |
| `available` INTEGER 0/1 | `is_available` bool | — |
| `created_at` TEXT | `created_at` TEXT | — |
| (none) | `description`, `cost_price_paise`, `image_path`, `is_countable`, `stock_quantity`, `low_stock_threshold`, `updated_at` | new fields to expose in admin UI |

**THE RUPEES→PAISE CONVERSION IS THE SILENT KILLER.** PITFALLS-v47.md #5 calls
this out as P0: "Customer orders a ₹50 drink, charged ₹5000 because a paise/rupee
field was misinterpreted." A Playwright contract test (Phase 350) must verify
price round-trip through admin → POS UI before cutover.

## Scope split

### Phase 346-01 (THIS COMMIT — scaffolding, flag-gated, default OFF)

- Add `CAFE_PROXY_ENABLED` env flag (default `false`)
- Dual-path implementation in `racingpoint-admin/src/app/api/cafe/menu/route.ts`:
  - `CAFE_PROXY_ENABLED=true` → proxy to rc `/cafe/items` + `/cafe/categories`, flatten to admin shape
  - `CAFE_PROXY_ENABLED=false` → legacy admin.db.menu_items reads/writes (current behavior)
- Read-only proxy path implemented with correct paise→rupees conversion
- Write path returns 503 in proxy mode (explicit: use legacy mode for writes until cutover)
- Adopt `withAdminDbError` helper in legacy paths

### Phase 346-02 (maintenance window — deferred to separate session)

- Implement proxy WRITE path (POST/PUT/DELETE)
- Category resolution: lookup-or-create for category string → category_id
- Rupees→paise conversion on write
- Pre-cutover snapshot: `sqlite3 admin.db ".backup admin-pre-cafe-migration.db"`
- Playwright contract test: admin edit → POS billing within 10s
- Manual smoke test on POS + kiosk
- Flip `CAFE_PROXY_ENABLED=true` on venue + cloud
- Verify no UI regressions for 24h
- Drop migration: `DROP TABLE admin.db.menu_items; DROP TABLE admin.db.inventory;`
- Schema guard: startup check refuses to boot if dropped tables reappear

### Phase 346-03 (cleanup — can ship with 346-02)

- Drop `admin.db.employees` (dead code — `/api/hr/employees` already proxies to rc `/staff`) — Phase 343 D4
- Remove hardcoded `terminal_pin` + `terminal_secret` from `deploy-staging/racecontrol.toml` + `racecontrol-server.toml` (Phase 343 D6)
- Identity source consolidation — `auth/mod.rs:1815-1820` single read path (Phase 343 C8)

## Requirements covered (partial — full cutover is Phase 346-02)

- ADMIN-14: Admin `/api/cafe/menu` proxies to rc — SCAFFOLDING only (read path done, write deferred)
- ADMIN-15: Admin `/api/cafe/inventory` proxies — deferred to 346-02
- ADMIN-16: Drop dead tables — deferred to 346-02
- ADMIN-17: Schema guard — deferred to 346-02
- ADMIN-20: Schema diff documented — DONE (this file)

## Files changed (this commit)

| File | Change |
|---|---|
| `racingpoint-admin/src/app/api/cafe/menu/route.ts` | Dual-path (flag-gated), proxy reader, paise↔rupees conversion helper, explicit write-not-implemented stub |
| `.planning/phases/346-cafe-menu-proxy/346-CONTEXT.md` | This file |

## Success criteria

1. `CAFE_PROXY_ENABLED=false` (default) → legacy admin.db behavior UNCHANGED. Zero regression for existing deploys.
2. `CAFE_PROXY_ENABLED=true` → GET returns items flattened from rc `/cafe/items`, with `source: "racecontrol"` field. Prices in rupees (NOT paise) matching UI expectations.
3. `CAFE_PROXY_ENABLED=true` → POST/PUT returns structured 503 `{error_code: "CAFE_PROXY_WRITE_PENDING"}` — never silently writes wrong data.
4. Playwright contract test (Phase 350) confirms round-trip price integrity.

## NOT tested (handoff list)

- Live proxy mode on venue or cloud (flag off by default — safe)
- Inventory proxy path (`src/app/api/cafe/inventory/route.ts` unchanged)
- Write-path cutover (explicitly deferred to 346-02)
- Drop migration (deferred)
- Schema guard (deferred)
- Field coverage beyond name/category/price/available (image, stock, cost — need UI work)

## Pre-cutover checklist (for Phase 346-02)

- [ ] Phase 350 Playwright contract test passes with `CAFE_PROXY_ENABLED=true`
- [ ] `sqlite3 admin.db ".backup admin-pre-cafe-migration-<date>.db"` on venue
- [ ] Same backup on cloud
- [ ] Backup copied to Bono VPS `/root/backups/migrations/`
- [ ] Maintenance window scheduled with Uday
- [ ] Venue closed to customers
- [ ] Kiosk + POS browsers refreshed after cutover
- [ ] Test order end-to-end: create menu item in admin → order on kiosk → confirm price matches
- [ ] If any test fails: flip `CAFE_PROXY_ENABLED=false` and restore backup
