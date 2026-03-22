# Project Research Summary

**Project:** v19.0 Cafe Inventory, Ordering & Marketing
**Domain:** Gaming cafe POS + inventory + promotions + marketing content, integrated with existing RP wallet and platform
**Researched:** 2026-03-22
**Confidence:** HIGH

## Executive Summary

Racing Point is adding cafe operations (ordering, inventory, promotions, marketing) on top of an existing Rust/Axum + Next.js + SQLite platform that already handles gaming sessions, wallet billing, WhatsApp alerts, and email notifications. The research consensus is clear: build this as a new domain within the existing monolith — not a separate service. All precedent in the codebase (billing, wallet, reservations, fleet health) follows the same pattern of Rust modules sharing `AppState` and the same SQLite pool, and cafe must follow this pattern to get atomic wallet deductions and foreign key integrity with the `drivers` table for free.

The feature set is well-understood from industry research. The critical path is: Menu items first (everything else depends on items existing), then inventory tracking, then ordering (the core value delivery), then promotions, then marketing content. The killer differentiator for a gaming cafe is order-from-seat via the existing PWA — customers ordering food without leaving their sim racing session, with delivery to their pod number. Industry data puts this at 20-38% higher order values versus queue-based ordering. The marketing content generation (auto-generated promo graphics, WhatsApp broadcast, digital menu board) requires all other layers to be in place first, so it is correctly positioned as the final phase.

The highest-risk areas are the shared wallet integration (double-deduction races in production money flows), stock decrement atomicity (classic TOCTOU race when PWA and POS submit simultaneous orders), and the WhatsApp marketing broadcast (risk of getting the operational bot number banned). All three have clear, proven mitigations documented in this research. None require architectural invention — they use patterns already in the codebase or well-established SQLite atomic update patterns.

---

## Key Findings

### Recommended Stack

All new code lives in the existing racecontrol repository. No new services, no new databases. Six new Node.js libraries are needed in `web/` for import, image generation, and scheduling. The PWA needs no new dependencies — it is a consumer of the Rust API.

**Core technologies:**

- **pdf-parse@^2.4.5** — PDF menu import — pure TypeScript, zero native deps, sufficient for text-based menu extraction with admin review step
- **exceljs@^4.4.0** — Spreadsheet (.xlsx/.csv) menu import — MIT-licensed (SheetJS community edition has commercial restrictions), supports read + write for future export
- **satori@^0.19.2 + @resvg/resvg-js@^2.6.2** — Marketing image generation — JSX-to-SVG-to-PNG pipeline, no headless browser required, same NAPI binary pattern already proven with `sharp`
- **croner@^10.0.1** — Promo scheduling (happy hour activation/deactivation) — TypeScript-native, IST timezone support, overlap protection; `node-cron` lacks all three
- **node-thermal-printer@^4.6.0** — Receipt printing — actively maintained, multi-printer support; `escpos` is abandoned (6-year-old alpha)
- **nanoid** — Order/receipt IDs — already a transitive dependency, no install needed

**Critical note on `node-thermal-printer`:** Confidence is MEDIUM because hardware integration requires live testing with the specific POS printer model and connection type (USB vs. network). Build an early prototype receipt in Phase 3 before full order flow, and maintain a `window.print()` fallback. See STACK.md for full risk matrix.

### Expected Features

**Must have (table stakes):**
- Menu browsing with categories (Beverages, Snacks, Meals, Combos)
- PWA self-service ordering with real-time availability
- Staff POS ordering (extend existing kiosk flow)
- Wallet deduction on order (unified billing — one balance for gaming + cafe)
- Order confirmation with receipt number + thermal print
- Out-of-stock blocking (atomic, cannot allow orders when stock=0)
- Item availability toggle (instant hide/show, no delete required)
- Stock quantity tracking with auto-decrement on sale
- Manual restock entry with audit log
- Low-stock threshold alerts (WhatsApp + dashboard + email, batched digest)
- CRUD for cafe items with cost price tracking
- Bulk import from PDF/spreadsheet (with preview-and-confirm, never auto-publish)
- Category management as a managed list (not free-text)
- Cafe sales dashboard + order history

**Should have (competitive differentiators):**
- Order-from-seat via PWA with pod number association (killer feature — no session interruption)
- Gaming + cafe combo bundles ("1 hour + burger + drink = Rs 999")
- Combo deals (item bundles at bundle price)
- Happy hour time-based discounts (auto-apply during valid window)
- Promo visibility in POS/PWA (banners, DEAL badges, countdowns)
- Auto-generated promo graphics (template-based, brand-compliant, preview before publish)
- WhatsApp promo broadcast (separate number from operational bot — see Pitfall 8)
- Digital menu board / in-store display (HTML page, auto-refresh, full-screen mode)

**Defer to v20+:**
- Loyalty/rewards program
- Online delivery or external platform integration
- Supplier management / auto-reorder
- Ingredient-level recipe costing
- Kitchen Display System (KDS)
- Multi-location support

### Architecture Approach

Cafe operations are added as new Rust modules (`cafe.rs`, `cafe_inventory.rs`, `cafe_orders.rs`, `cafe_promos.rs`, `cafe_alerts.rs`, `cafe_marketing.rs`) within the existing `racecontrol` crate, sharing `AppState` and the SQLite pool. New tables (`cafe_items`, `cafe_inventory`, `cafe_orders`, `cafe_order_items`, `cafe_stock_movements`, `cafe_promos`) are added via the existing `CREATE TABLE IF NOT EXISTS` migration pattern. Admin UI lives in `web/src/app/cafe/`, customer UI in `pwa/src/app/cafe/`. PDF/spreadsheet import and marketing image generation live in Next.js API routes (`web/src/app/api/cafe/`). The cart is client-side only — the server only receives the final order POST.

**Major components:**
1. **`cafe.rs` + `cafe_inventory.rs`** — Menu CRUD, item availability, stock tracking, restock, audit log
2. **`cafe_orders.rs`** — Order validation (stock check + wallet balance + promo rules), atomic debit + decrement + journal + receipt generation, DashboardEvent broadcast
3. **`cafe_promos.rs`** — Combo deals, happy hour rules, gaming+cafe bundles, best-discount selection (no stacking)
4. **`cafe_alerts.rs`** — Low-stock digests via existing WhatsApp/email/dashboard infrastructure
5. **`cafe_marketing.rs`** — Content generation orchestration (satori templates + menu/promo data), WhatsApp broadcast, digital poster
6. **`web/src/app/cafe/`** — Admin: menu management, inventory view, promo builder, marketing content
7. **`pwa/src/app/cafe/`** — Customer: menu browsing, cart, ordering flow with pod number, order history

**Patterns to reuse (no new architecture needed):**
- `wallet::debit()` with `txn_type = "cafe_order"` — already accepts arbitrary transaction types
- `DashboardEvent` enum extension — add `CafeOrder`, `CafeLowStock` variants
- `accounting::post_cafe_sale()` — copy pattern from `post_session_charge()`
- `whatsapp_alerter.rs` + `email_alerts.rs` — reuse `send_whatsapp_message()` and `EmailAlerter`
- 5-tier auth model from `routes.rs` — customer JWT / kiosk staff JWT / staff routes map exactly to cafe needs

### Critical Pitfalls

1. **Stock decrement race condition (double-sell)** — Use atomic `UPDATE cafe_inventory SET quantity = quantity - 1 WHERE item_id = ? AND quantity > 0`, check rows-affected. Never SELECT then UPDATE. Address in Phase 3 (ordering), but design the schema to support it from Phase 2.

2. **Shared wallet double-deduction or partial failure** — Route all cafe wallet debits through the existing `wallet::debit()` function. Wrap order creation + wallet deduction in a SQLite transaction with rollback on failure. Use optimistic concurrency: `UPDATE wallets SET balance = balance - ? WHERE id = ? AND balance >= ?`. Address in Phase 3.

3. **PDF/spreadsheet import creates garbage data** — Never auto-publish imported items. Mandatory preview-and-confirm flow before any item goes live. Validate: price > 0, name non-empty, category in known list, no cost > selling price. Address in Phase 1.

4. **Promo stacking destroys margins** — Define stacking rules before the first promo type is implemented: one promotion per order, "most specific wins" (named combo beats blanket discount). Calculate prices server-side only. Log applied promo ID with each order line. Address in Phase 4.

5. **WhatsApp broadcast gets the bot banned** — Never use the operational bot number (7075778180) for bulk marketing. Use a separate number or skip WhatsApp for marketing entirely. Rate-limit all outbound messages. Opt-in/opt-out tracking required. Address before Phase 5 begins.

---

## Implications for Roadmap

Based on the feature dependency graph from FEATURES.md and the component build order from ARCHITECTURE.md, 5 phases are recommended:

### Phase 1: Menu & Item Foundation
**Rationale:** Every other feature depends on items existing. This is the only zero-dependency phase. Includes bulk import because manual entry of 30-80 items before anything works is not viable.
**Delivers:** Full menu CRUD, categories, cost price tracking, bulk import with preview+confirm, availability toggle
**Features from FEATURES.md:** Menu item CRUD, bulk import, category management, cost price tracking
**Avoids:** PDF garbage data (Pitfall 3), category naming drift (Pitfall 12), countable/uncountable model confusion (Pitfall 7)
**Stack:** `pdf-parse`, `exceljs` (both in `web/src/app/api/cafe/import/`)
**Needs research-phase:** No — CRUD + import is well-documented. Preview+confirm UX is the only design decision.

### Phase 2: Inventory Tracking & Alerts
**Rationale:** Stock data must exist before ordering can happen (out-of-stock blocking requires accurate counts). Alert infrastructure uses existing WhatsApp/email modules — low integration risk.
**Delivers:** Per-item stock counts, auto-decrement hooks (ready for Phase 3), manual restock, stock movement audit log, low-stock digest alerts (batched, severity-tiered)
**Features from FEATURES.md:** Stock quantity, auto-decrement (wired in Phase 3), manual restock, low-stock alerts with three-channel delivery
**Avoids:** Alert fatigue (Pitfall 6) — digest/batch design must be built here, not retrofitted
**Stack:** Existing `whatsapp_alerter.rs`, `email_alerts.rs`, `DashboardEvent`
**Needs research-phase:** No — established inventory alert patterns.

### Phase 3: Ordering & Billing
**Rationale:** Core value delivery. Requires Phase 1 (items) and Phase 2 (stock data). This is the phase with the highest-risk integrations (wallet + atomic stock decrement).
**Delivers:** PWA self-service ordering (with pod number), POS staff-assisted ordering, wallet deduction, receipt generation, thermal printing, order history, real-time dashboard events
**Features from FEATURES.md:** PWA ordering, POS ordering, wallet deduction, receipt number, out-of-stock blocking, order history, admin sales dashboard
**Differentiators:** Order-from-seat via PWA is implemented here — pod number on every order
**Avoids:** Stock decrement race (Pitfall 1), shared wallet double-deduction (Pitfall 2), receipt printing failure (Pitfall 5), wallet UX surprise (Pitfall 9)
**Stack:** `node-thermal-printer` (test early with actual printer), `nanoid` for receipt numbers
**Needs research-phase:** Yes — receipt printing path needs validation against the actual POS printer model before Phase 3 planning is finalized.

### Phase 4: Promotions & Deals
**Rationale:** Promos require items to exist (Phase 1) and integrate into the order price calculation (Phase 3). Stacking rules must be designed before the first promo type is built — retroactive stacking logic is painful.
**Delivers:** Combo deals, happy hour time-based discounts, gaming+cafe bundles, promo display in PWA/POS, `croner`-based scheduler for timed activation
**Features from FEATURES.md:** Combo deals, happy hour, gaming+cafe bundles, promo visibility
**Avoids:** Promo stacking (Pitfall 4) — "most specific wins" rule designed before any promo type; IST timezone bugs (Pitfall 11) — all times stored as naive IST HH:MM with explicit label
**Stack:** `croner` for scheduling, `cafe_promos.rs` config_json flexible schema
**Needs research-phase:** No — established promo engine patterns. Stacking decision is a product decision, not a research gap.

### Phase 5: Marketing & Content
**Rationale:** Depends on complete menu data (Phase 1) and active promos (Phase 4). Marketing content without real menu items and real promos is a shell. This phase is the most self-contained and has no downstream dependencies.
**Delivers:** Auto-generated promo graphics (template-based, brand-compliant), WhatsApp broadcast (separate number decision required), digital menu board / in-store display
**Features from FEATURES.md:** Auto-generated promo graphics, WhatsApp broadcast, digital poster
**Avoids:** WhatsApp bot ban (Pitfall 8) — operational bot number is never used for broadcasts; generated image quality (Pitfall 10) — fixed templates with brand guidelines, preview before publish
**Stack:** `satori` + `@resvg/resvg-js` for PNG generation in Next.js API routes, `sharp` (already in PWA) for post-processing
**Needs research-phase:** Yes — the WhatsApp broadcast channel decision (separate official API number vs. email-only for marketing) must be resolved before implementation. This is a business/infrastructure decision with technical implications.

### Phase Ordering Rationale

- **Items before everything** because every query, every validation, every promo references `cafe_items.id`. No shortcuts here.
- **Inventory before ordering** because out-of-stock blocking requires accurate stock counts. The atomic decrement pattern in ordering presupposes an existing `cafe_inventory` row.
- **Ordering before promos** because the promo engine integrates into the order price calculation — you need a working order flow to integrate into.
- **Promos before marketing** because marketing content generation fetches `active promos` as its primary input. A marketing phase with no promos data produces generic content.
- **Ordering is the highest-risk phase** (wallet + concurrency + hardware printing). It deliberately follows two lower-risk phases to give the team time to validate the printer and wallet patterns before they become blockers.

### Research Flags

**Phases needing deeper research during planning:**
- **Phase 3 (Ordering):** Validate receipt printing path against actual POS printer model (USB vs. network, exact ESC/POS command support). Do this before Phase 3 planning — it determines whether `node-thermal-printer` config is a 30-minute task or a 3-day investigation.
- **Phase 5 (Marketing):** Resolve the WhatsApp broadcast channel decision. Options: (a) register a separate official WhatsApp Business API number, (b) use email only for marketing, (c) skip broadcast entirely and use digital poster + in-store display only. Each has different infrastructure cost and timeline implications.

**Phases with standard patterns (skip research-phase):**
- **Phase 1 (Menu):** CRUD + file import is well-documented. pdf-parse and exceljs have clear APIs. The only non-standard piece (preview+confirm UX) is a design choice, not a research gap.
- **Phase 2 (Inventory):** Stock tracking and alert batching are well-established patterns. Existing `whatsapp_alerter.rs` and `email_alerts.rs` reduce integration research to zero.
- **Phase 4 (Promos):** Promo engine patterns are well-documented. The stacking decision is a product call. `croner` API is simple and straightforward.

---

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | 5 of 6 new libraries are HIGH confidence with clear rationale. `node-thermal-printer` is MEDIUM — hardware-dependent, needs live testing. Version pinning strategy is conservative and documented. |
| Features | HIGH | Well-established domain (POS/cafe systems). Industry research from multiple sources. Anti-features explicitly justified. Feature dependency graph is clear and consistent. |
| Architecture | HIGH | Built entirely from verified existing codebase patterns (`wallet.rs`, `accounting.rs`, `whatsapp_alerter.rs`, `routes.rs`). No speculative design — every component maps to an existing precedent in the monolith. |
| Pitfalls | HIGH | 4 critical pitfalls are well-documented with specific prevention code patterns. Most have direct precedents in existing codebase (SQLite atomic update already in `wallet.rs` line 159). WhatsApp ban risk is documented from 2025 official policy updates. |

**Overall confidence:** HIGH

### Gaps to Address

- **Printer model:** The specific POS thermal printer make/model and connection type (USB vs. TCP/IP) must be confirmed before Phase 3 planning. This determines `node-thermal-printer` configuration and affects fallback strategy.
- **WhatsApp marketing channel:** The decision on whether to use a separate official WhatsApp Business API number for marketing broadcasts, or to exclude WhatsApp from marketing entirely, must be made before Phase 5. This is a business + infrastructure decision with cost implications (official API requires monthly spend). Flag for Uday to decide.
- **Satori font files:** Montserrat (body) and Enthocentric (headers) `.ttf`/`.woff` files must be available as ArrayBuffer at server startup. Confirm file locations in the existing repo before Phase 5 planning.
- **is_countable categorization:** The list of which menu items are countable (bottles, packaged snacks) vs. uncountable (chai, fresh coffee) must be decided by Uday/staff before Phase 1 data entry. This affects stock tracking scope.

---

## Sources

### Primary (HIGH confidence — official docs / existing codebase)
- Existing codebase: `crates/racecontrol/src/wallet.rs`, `billing.rs`, `accounting.rs`, `whatsapp_alerter.rs`, `email_alerts.rs`, `api/routes.rs`, `db/mod.rs`, `state.rs`
- [pdf-parse npm](https://www.npmjs.com/package/pdf-parse) — v2.4.5, pure TypeScript rewrite
- [exceljs npm](https://www.npmjs.com/package/exceljs) — v4.4.0, MIT license confirmed
- [satori GitHub](https://github.com/vercel/satori) — v0.19.2, Vercel-maintained
- [@resvg/resvg-js npm](https://www.npmjs.com/package/@resvg/resvg-js) — v2.6.2, Windows x64 prebuilt confirmed
- [croner npm](https://www.npmjs.com/package/croner) — v10.0.1, TypeScript-native, IST timezone support confirmed
- [node-thermal-printer npm](https://www.npmjs.com/package/node-thermal-printer) — v4.6.0, active maintenance

### Secondary (MEDIUM confidence — industry research, multiple sources agree)
- [44 Best Restaurant POS Features 2026](https://getquantic.com/restaurant-pos-system-features/)
- [Restaurant Inventory Management Best Practices](https://bepbackoffice.com/blog/restaurant-inventory-management-best-practices/)
- [Beyond if (stock > 0): Handling Race Conditions](https://dev.to/aaditya_efec6eedf3319cec3/beyond-if-stock-0-handling-race-conditions-in-high-traffic-e-commerce-systems-481)
- [Promotion Stacking: Definition, Logic & Margin Control](https://www.voucherify.io/glossary/promotion-stacking)
- [PkgPulse: PDF parsing comparison](https://www.pkgpulse.com/blog/unpdf-vs-pdf-parse-vs-pdfjs-dist-pdf-parsing-extraction-nodejs-2026)
- [PkgPulse: Scheduling comparison](https://www.pkgpulse.com/blog/node-cron-vs-node-schedule-vs-croner-task-scheduling-nodejs-2026)
- [npm-compare: Excel libraries](https://npm-compare.com/excel4node,exceljs,xlsx,xlsx-populate)

### Tertiary (MEDIUM confidence — single source, validate during implementation)
- [WhatsApp Business 2025 Update: New Restrictions](https://blog.zepic.com/article/whatsapp-business-april-2025-update-new-restrictions-whatsapp-broadcast-limit-explained) — broadcast limits (policy may change; verify current tier limits before Phase 5)
- [Satori + resvg image generation guide](https://anasrin.dev/blog/generate-image-from-html-using-satori-and-resvg/) — confirm font loading pattern during Phase 5 planning
- [7 KPIs for Gaming Cafe](https://financialmodelslab.com/blogs/kpi-metrics/gaming-cafe) — 15-25% higher transaction from bundles (single source, treat as directional)

---
*Research completed: 2026-03-22*
*Ready for roadmap: yes*
