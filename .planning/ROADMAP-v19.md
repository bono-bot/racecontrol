# Roadmap: v19.0 Cafe Inventory, Ordering & Marketing

## Overview

Deliver a complete cafe operations layer for Racing Point eSports and Cafe -- menu management, self-service and staff-assisted ordering with shared wallet billing, real-time inventory tracking with three-channel alerts, promotional deals engine, and automated marketing content. Built as new Rust modules and Next.js pages within the existing racecontrol monolith, reusing wallet, WhatsApp, email, and auth infrastructure. Ten phases progress from data foundation (menu items) through inventory, ordering (core value), promotions, to marketing content generation.

## Phases

**Phase Numbering:**
- Integer phases (149, 150, ...): Planned milestone work
- Decimal phases (149.1, 149.2): Urgent insertions (marked with INSERTED)

- [ ] **Phase 149: Menu Data Model & CRUD** - Cafe item schema, backend API, and admin UI for manual item management
- [ ] **Phase 150: Menu Import** - Bulk import from PDF/spreadsheet with preview-and-confirm, plus item image uploads
- [ ] **Phase 151: Menu Display** - Cafe menu rendering in POS (by category) and PWA (with images, descriptions, pricing)
- [ ] **Phase 152: Inventory Tracking** - Stock quantities, countable/uncountable categorization, manual restock, thresholds, inventory dashboard
- [ ] **Phase 153: Inventory Alerts** - Three-channel low-stock alerts (WhatsApp, dashboard banner, email) with cooldown
- [ ] **Phase 154: Ordering Core** - PWA self-service and POS staff ordering with wallet deduction, atomic stock decrement, out-of-stock blocking
- [ ] **Phase 155: Receipts & Order History** - Thermal receipt printing, WhatsApp receipt delivery, customer order history in PWA
- [ ] **Phase 156: Promotions Engine** - Combo deals, happy hour discounts, gaming+cafe bundles, stacking rules
- [ ] **Phase 157: Promotions Integration** - Promo display in POS/PWA and auto-apply at checkout
- [ ] **Phase 158: Marketing & Content** - Auto-generated promo graphics and WhatsApp broadcast to customer list

## Phase Details

### Phase 149: Menu Data Model & CRUD
**Goal**: Admin can create and manage cafe items with all required fields, and items persist correctly in the database
**Depends on**: Nothing (first phase -- foundation for everything)
**Requirements**: MENU-02, MENU-03, MENU-04, MENU-05
**Success Criteria** (what must be TRUE):
  1. Admin can add a new cafe item with name, description, category, selling price, and cost price via the admin dashboard
  2. Admin can edit any field of an existing cafe item and see the change reflected immediately
  3. Admin can delete a cafe item and it no longer appears anywhere
  4. Admin can toggle an item between available and unavailable, and unavailable items are hidden from customer-facing views
  5. Categories are managed as a controlled list (not free-text entry)
**Plans**: TBD

Plans:
- [ ] 149-01: TBD
- [ ] 149-02: TBD

### Phase 150: Menu Import
**Goal**: Admin can populate the full cafe menu from existing PDF or spreadsheet files without manual item-by-item entry
**Depends on**: Phase 149
**Requirements**: MENU-01, MENU-06
**Success Criteria** (what must be TRUE):
  1. Admin can upload a PDF or spreadsheet file and see a preview of parsed items before confirming import
  2. Import validates each item (price > 0, name non-empty, category in known list) and flags errors for correction
  3. No items are published until admin explicitly confirms the import preview
  4. Admin can upload an image for any cafe item, and the image is stored and associated with that item
**Plans**: TBD

Plans:
- [ ] 150-01: TBD
- [ ] 150-02: TBD

### Phase 151: Menu Display
**Goal**: Customers and staff can browse the complete cafe menu with correct pricing, categories, and images
**Depends on**: Phase 150
**Requirements**: MENU-07, MENU-08
**Success Criteria** (what must be TRUE):
  1. POS displays cafe items grouped by category with name and selling price
  2. PWA displays cafe items grouped by category with images, descriptions, and selling price
  3. Unavailable items do not appear in either POS or PWA views
  4. Menu loads within 2 seconds on the cafe WiFi network
**Plans**: TBD

Plans:
- [ ] 151-01: TBD
- [ ] 151-02: TBD

### Phase 152: Inventory Tracking
**Goal**: Admin has full visibility into stock levels and can manage inventory for all countable items
**Depends on**: Phase 149
**Requirements**: INV-01, INV-02, INV-04, INV-05, INV-09
**Success Criteria** (what must be TRUE):
  1. Admin can set and view stock quantities for countable items (bottles, buns, packaged snacks)
  2. Items are correctly categorized as countable (stock-tracked) or uncountable (availability toggle only)
  3. Admin can record a restock event and see the stock quantity increase accordingly
  4. Admin can set a low-stock threshold per countable item
  5. Inventory dashboard shows all items with current stock, threshold status, and countable/uncountable designation
**Plans**: TBD

Plans:
- [ ] 152-01: TBD
- [ ] 152-02: TBD

### Phase 153: Inventory Alerts
**Goal**: Staff never misses a low-stock situation -- alerts fire through three independent channels when thresholds are breached
**Depends on**: Phase 152
**Requirements**: INV-06, INV-07, INV-08
**Success Criteria** (what must be TRUE):
  1. When a countable item's stock drops to or below its threshold, a WhatsApp alert is sent to the admin (once per breach, with cooldown)
  2. A warning banner appears in the admin dashboard for any item below its low-stock threshold
  3. An email alert fires when a low-stock threshold is breached
  4. Repeated threshold checks for the same item do not spam alerts (cooldown/dedup works)
**Plans**: TBD

Plans:
- [ ] 153-01: TBD
- [ ] 153-02: TBD

### Phase 154: Ordering Core
**Goal**: Customers can order cafe items and pay from their RP wallet -- the core value delivery
**Depends on**: Phase 151, Phase 152
**Requirements**: ORD-01, ORD-02, ORD-03, ORD-04, ORD-07, ORD-08, INV-03
**Success Criteria** (what must be TRUE):
  1. Customer can browse the cafe menu in PWA, add items to a cart, and submit an order
  2. Staff can enter a cafe order via POS on behalf of a customer
  3. Order total is deducted from the customer's existing RP wallet balance
  4. Each completed order has a unique receipt number and transaction ID
  5. Items with zero stock cannot be added to an order (out-of-stock blocking prevents it)
  6. Concurrent orders for the last unit of an item do not both succeed (atomic stock decrement + wallet deduction)
  7. Stock quantities auto-decrement when countable items are sold
**Plans**: TBD

Plans:
- [ ] 154-01: TBD
- [ ] 154-02: TBD
- [ ] 154-03: TBD

### Phase 155: Receipts & Order History
**Goal**: Every order produces a physical receipt and digital record that staff and customers can reference
**Depends on**: Phase 154
**Requirements**: ORD-05, ORD-06, ORD-09
**Success Criteria** (what must be TRUE):
  1. Completing an order triggers a thermal receipt print for cafe staff to prepare the order
  2. Customer receives their order receipt via WhatsApp after order confirmation
  3. Customer can view their full cafe order history in the PWA
**Plans**: TBD

Plans:
- [ ] 155-01: TBD
- [ ] 155-02: TBD

### Phase 156: Promotions Engine
**Goal**: Admin can create and configure promotional deals that drive cafe revenue
**Depends on**: Phase 149
**Requirements**: PROMO-01, PROMO-02, PROMO-03, PROMO-04
**Success Criteria** (what must be TRUE):
  1. Admin can create a combo deal that bundles specific items at a discounted price
  2. Admin can create a happy hour discount with start/end times in IST
  3. Admin can create a gaming+cafe combo bundle (game session + cafe item at bundle price)
  4. Admin can configure stacking rules -- which promos can combine and which are exclusive
  5. Promos activate and deactivate automatically based on their configured time windows
**Plans**: TBD

Plans:
- [ ] 156-01: TBD
- [ ] 156-02: TBD

### Phase 157: Promotions Integration
**Goal**: Active promos are visible to customers and staff, and discounts apply automatically at checkout
**Depends on**: Phase 154, Phase 156
**Requirements**: PROMO-05, PROMO-06
**Success Criteria** (what must be TRUE):
  1. Active promos display in both POS and PWA during their applicable time windows
  2. When a customer's cart meets promo conditions, the discount is applied automatically at checkout
  3. Applied promo is recorded with the order (traceable which discount was used)
  4. Promo pricing is calculated server-side only (not client-side)
**Plans**: TBD

Plans:
- [ ] 157-01: TBD
- [ ] 157-02: TBD

### Phase 158: Marketing & Content
**Goal**: Cafe promos and menu updates reach customers through auto-generated visual content and broadcast messages
**Depends on**: Phase 156
**Requirements**: MKT-01, MKT-02
**Success Criteria** (what must be TRUE):
  1. Admin can generate promo graphics (menu images, daily specials) suitable for Instagram stories/posts with one click
  2. Generated graphics reflect current menu items, prices, and active promos
  3. Admin can trigger a WhatsApp broadcast of promo messages to the customer list (using a separate number from the operational bot)
  4. Generated content uses Racing Point brand identity (colors, fonts, logo)
**Plans**: TBD

Plans:
- [ ] 158-01: TBD
- [ ] 158-02: TBD

## Progress

**Execution Order:**
Phases execute in numeric order: 149 -> 150 -> 151 -> 152 -> 153 -> 154 -> 155 -> 156 -> 157 -> 158
Note: Phase 152 can start after 149 (parallel with 150/151). Phase 156 can start after 149 (parallel with inventory/ordering).

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 149. Menu Data Model & CRUD | 0/0 | Not started | - |
| 150. Menu Import | 0/0 | Not started | - |
| 151. Menu Display | 0/0 | Not started | - |
| 152. Inventory Tracking | 0/0 | Not started | - |
| 153. Inventory Alerts | 0/0 | Not started | - |
| 154. Ordering Core | 0/0 | Not started | - |
| 155. Receipts & Order History | 0/0 | Not started | - |
| 156. Promotions Engine | 0/0 | Not started | - |
| 157. Promotions Integration | 0/0 | Not started | - |
| 158. Marketing & Content | 0/0 | Not started | - |

---
*Roadmap created: 2026-03-22*
*Last updated: 2026-03-22*
