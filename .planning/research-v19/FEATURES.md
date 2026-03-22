# Feature Landscape: Cafe Inventory, Ordering & Marketing

**Domain:** Gaming cafe POS + inventory + promotions + marketing content
**Researched:** 2026-03-22
**Overall confidence:** HIGH (well-established domain, clear industry patterns)

## Table Stakes

Features users and staff expect. Missing = product feels incomplete or broken.

### Ordering & Billing

| Feature | Why Expected | Complexity | Notes |
|---------|-------------|------------|-------|
| Menu browsing with categories | Customers cannot order what they cannot find. Every cafe POS has categorized menus. | Low | Categories: Beverages, Snacks, Meals, Combos. Reuse existing item structure. |
| PWA self-service ordering | Customers already have the RP PWA and wallet. Ordering from their phone while gaming is the core value prop. | Medium | Must show real-time availability. Must not require leaving the gaming session. |
| Staff POS ordering | Staff must take orders for walk-ups and customers who prefer human interaction. | Low | Extend existing POS — add cafe item selection to existing flow. |
| Wallet deduction on order | Unified billing is the entire premise. Gaming + cafe on one balance. | Low | Already built. Wire cafe orders into existing wallet deduction endpoint. |
| Order confirmation with receipt number | Staff need to know what to prepare. Customer needs proof of purchase. | Low | Sequential receipt numbers. Thermal print to existing POS printer. |
| Out-of-stock blocking | Allowing orders for unavailable items destroys trust and creates staff headaches. | Low | Check stock >= quantity at order time. Race condition guard needed (optimistic lock or check-and-decrement atomically). |
| Item availability toggle | Admin needs to instantly hide seasonal items, broken equipment items (e.g., waffle maker down), or sold-out items without deleting them. | Low | Boolean flag on item. Filters in POS + PWA queries. |

### Inventory Tracking

| Feature | Why Expected | Complexity | Notes |
|---------|-------------|------------|-------|
| Stock quantity per item | Must know what's available. This is the minimum viable inventory. | Low | Integer count for countable items (bottles, buns, packaged goods). |
| Auto-decrement on sale | Manual stock tracking after every sale is unsustainable. Every modern POS does this. | Low | Decrement in same transaction as wallet deduction. Atomic. |
| Manual restock entry | Supplies arrive, staff enters new quantities. No workflow needed at this scale. | Low | Admin form: select item, enter quantity added, optional note. Log the adjustment. |
| Low-stock threshold + alerts | Running out mid-service with no warning is an operational failure. Three-channel alerts (WhatsApp + dashboard + email) match existing infra. | Medium | Per-item configurable threshold. Alert fires once per threshold breach (not repeatedly). Cooldown or "acknowledged" flag to prevent alert spam. |
| Stock adjustment log | Need audit trail for discrepancies, theft detection, and accountability. Every restock and manual adjustment recorded with who/when/why. | Low | Append-only log. Timestamps, user, quantity change, reason. |

### Menu & Item Management

| Feature | Why Expected | Complexity | Notes |
|---------|-------------|------------|-------|
| CRUD for cafe items | Admin must add, edit, delete items. Basic data management. | Low | Fields: name, description, category, selling price, cost price, image (optional), available flag. |
| Bulk import from PDF/spreadsheet | Initial menu has ~30-80 items. Manual entry is painful. One-time import + occasional updates. | Medium | Parse CSV/Excel reliably. PDF parsing is harder — convert to CSV first or use structured extraction. Validate before insert. |
| Cost price tracking | Gross margin visibility per item is essential for a cafe business. Without cost price, you cannot know if you are profitable. | Low | Stored alongside selling price. Not shown to customers. Used in admin reports. |
| Category management | Items must be organized. Flat list is unusable past 15 items. | Low | Predefined categories with ability to add custom ones. |

### Admin Visibility

| Feature | Why Expected | Complexity | Notes |
|---------|-------------|------------|-------|
| Cafe sales dashboard | Admin needs to see today's sales, popular items, and revenue at a glance. | Medium | Aggregate orders by day/item/category. Show totals, item counts, gross margin. |
| Order history | Must look up past orders for disputes, refunds, and pattern analysis. | Low | Searchable/filterable list. By date, customer, item. |

## Differentiators

Features that set Racing Point apart from a generic cafe. Not expected, but create real value in the gaming cafe context.

### Gaming + Cafe Integration (unique to this context)

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Gaming+cafe combo bundles | "1 hour sim racing + burger + drink = Rs 999" drives higher spend per visit and is unique to gaming cafes. Cross-sell is the #1 revenue lever for gaming cafes (15-25% higher average transaction). | Medium | Bundle = virtual item containing game session + cafe items at bundle price. Apply at POS or PWA. Must validate both gaming slot availability AND cafe item stock. |
| Order-from-seat via PWA | Customer orders food from their gaming pod without leaving the session. This is the killer feature — no interruption, no queue, food arrives at the pod. Industry data shows 20-38% higher order values when customers can browse unhurried. | Low | PWA already exists. Add menu tab. Associate order with pod/seat number. Staff sees "Pod 3 wants a Coke." |
| Pod number on order ticket | Staff knows exactly where to deliver. No shouting names, no customer walking away from their game. | Low | Order includes pod_number from session. Printed on receipt. |

### Promotions

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Combo deals (item bundles) | "Burger + Coke = Rs 149 instead of Rs 180." Standard upsell. Increases average order value. | Medium | Define combo: list of items + combo price. Validate all items in stock. Display as single orderable unit in POS/PWA. |
| Happy hour time-based discounts | "20% off all drinks 3-6 PM" fills slow hours. Common in cafes, powerful when combined with gaming off-peak rates. | Medium | Rule: discount percentage + applicable categories/items + time window (start/end, days of week). Auto-apply in POS/PWA during valid window. |
| Promo visibility in POS/PWA | Active promos displayed prominently so staff and customers know about them. No promo is useful if nobody sees it. | Low | Banner/badge on applicable items. "DEAL" tag. Countdown for time-limited offers. |

### Marketing Content Generation

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Auto-generated promo graphics | Instagram stories/posts with current menu items, prices, and promo details. Saves hours of manual design work weekly. | High | Template-based image generation. Use existing menu data + brand colors (Racing Red #E10600). Canvas/image library (Sharp, node-canvas, or Puppeteer screenshot of HTML template). |
| WhatsApp broadcast of promos | Push promos to customer list via existing WhatsApp bot infrastructure. Direct channel, high open rates. | Medium | Template message with promo details. Broadcast to opted-in customer list. Use existing comms-link WhatsApp bot (staff number: 7075778180). |
| Digital poster for in-store screens | Menu boards and promo displays on the spectator screen or any connected display. Always current, no manual updates. | Medium | HTML page served at a URL, auto-refreshes. Shows current menu, prices, active promos. Full-screen mode for display screens. Reuses same templates as social media graphics. |

## Anti-Features

Features to explicitly NOT build. Each has a clear reason.

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| Kitchen Display System (KDS) / order queue | Racing Point is a small cafe (not a restaurant kitchen). Receipt-based prep is sufficient. KDS adds complexity for no gain at this scale (~30-80 items, low volume). | Print receipt for staff. Order list visible in admin dashboard. |
| Separate cafe wallet / payment system | Defeats the unified billing purpose. Two balances confuse customers and complicate accounting. | Use existing RP wallet. One balance for everything. |
| Purchase order / supplier workflow | At current scale, supplies are bought at local market. PO workflow is enterprise overhead for a single-location cafe. | Manual restock entry with quantity + note. |
| Loyalty points / rewards program | Scope creep. Valuable but independent of core cafe operations. Defer to v20+. | Note in roadmap for future milestone. |
| Online delivery / external platform integration | Racing Point is in-store only. Delivery logistics, packaging, and platform fees are a different business. | In-store ordering only (PWA + POS). |
| Supplier management / auto-reorder | Requires supplier database, lead times, order minimums. Overkill for manual purchasing at a single location. | Low-stock alerts tell admin what to buy. They buy it manually. |
| Ingredient-level tracking / recipe costing | Full restaurants track flour, oil, salt per dish. A gaming cafe serving pre-made items (bottled drinks, packaged snacks, simple prepared foods) does not need this granularity. | Track stock at the sellable-item level, not ingredient level. |
| Multi-location support | One location. Multi-location adds tenant isolation, location-specific pricing, cross-location inventory. Unnecessary complexity. | Single-location data model. |
| Table/seat reservation for cafe | Gaming pods already have session booking. Cafe seating is informal. No need for restaurant-style table management. | Orders associated with pod number (from gaming session), not table number. |
| Calorie/nutrition tracking | Regulatory requirement for large chains (200+ locations in some jurisdictions). Not required for a single-location gaming cafe in India. | Optional description field can include nutrition info if desired. |

## Feature Dependencies

```
Menu Item CRUD ──────────────> All other features (everything depends on items existing)
     |
     v
Bulk Import ──> Initial menu population (one-time, but needed before anything works)
     |
     v
Stock Tracking ──────────────> Out-of-stock blocking
     |                              |
     v                              v
Low-stock Alerts              Order Placement (POS + PWA)
     |                              |
     v                              v
WhatsApp/Email alerts         Wallet Deduction ──> Receipt Generation
                                    |
                                    v
                              Order History ──> Sales Dashboard
                                    |
                              Combo Deals ──> Gaming+Cafe Bundles
                                    |
                              Happy Hour Rules
                                    |
                                    v
                              Promo Visibility (POS/PWA)
                                    |
                                    v
                              Marketing Content Generation (needs menu + promo data)
                                    |
                                    ├──> Auto-generated graphics
                                    ├──> WhatsApp broadcast
                                    └──> Digital poster/menu board
```

**Critical path:** Item CRUD -> Stock Tracking -> Ordering -> Promos -> Marketing

Marketing content generation depends on everything else existing first (menu data, prices, active promos).

## MVP Recommendation

**Phase 1 — Core Cafe Operations (table stakes):**
1. Menu item CRUD + bulk import
2. Stock tracking with auto-decrement
3. POS ordering (staff-assisted) with wallet deduction + receipt
4. PWA ordering (self-service) with pod number association
5. Item availability toggle + out-of-stock blocking
6. Low-stock alerts (WhatsApp + dashboard + email)

**Phase 2 — Promotions & Deals:**
1. Combo deals (item bundles at bundle price)
2. Happy hour time-based discounts
3. Gaming+cafe combo bundles
4. Promo display in POS and PWA

**Phase 3 — Marketing & Content:**
1. Auto-generated promo graphics (template-based)
2. WhatsApp promo broadcasts
3. Digital menu board / in-store display

**Defer to v20+:** Loyalty program, delivery, supplier management, ingredient tracking.

**Rationale:** Core operations must work before promos make sense (you need items and orders before you can discount them). Marketing content requires both menu data and promo data to be meaningful. Each phase is independently shippable and valuable.

## Sources

- [44 Best Restaurant POS Features Every Operator Needs in 2026](https://getquantic.com/restaurant-pos-system-features/)
- [Top 14 Features to Look for in a Cafe POS System in 2026](https://getquantic.com/cafe-pos-system-features/)
- [Restaurant Inventory Management Best Practices](https://bepbackoffice.com/blog/restaurant-inventory-management-best-practices/)
- [Restaurant Food Combo Offers - Strategies to Boost Sales](https://www.restauranttimes.com/blogs/menu-design/restaurant-food-combo-offers/)
- [7 KPIs for Gaming Cafe: Breakeven](https://financialmodelslab.com/blogs/kpi-metrics/gaming-cafe)
- [10 AI Tools for Restaurant Marketing 2026](https://www.imagine.art/blogs/ai-tools-for-restaurant)
- [Self-Service Kiosk Benefits](https://kiosk.com/benefits-of-self-service-kiosks/)
- [Preventing Stockouts and Overstock: Smart Inventory Planning](https://supy.io/blog/preventing-stockouts-and-overstock-smart-inventory-planning-for-restaurants)
- [Promotion Engine | Voucherify](https://www.voucherify.io/promotion-engine)
- [Restaurant POS Systems: Top Trends for 2026](https://www.mentorpos.com/restaurant-pos-systems-top-trends-for-2026/)
