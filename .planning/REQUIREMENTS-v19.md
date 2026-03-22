# Requirements: v19.0 Cafe Inventory, Ordering & Marketing

**Defined:** 2026-03-22
**Core Value:** Customers can browse the cafe menu, place orders, and pay from their existing RP wallet -- with staff always knowing what's in stock and promos driving revenue.

## v1 Requirements

Requirements for initial release. Each maps to roadmap phases.

### Menu & Item Management

- [x] **MENU-01**: Admin can upload cafe items from PDF or spreadsheet (name, price, category, cost price, description) with preview-and-confirm flow
- [ ] **MENU-02**: Admin can manually add cafe items with name, description, category, selling price, and cost price
- [ ] **MENU-03**: Admin can edit existing cafe item details (name, description, category, prices)
- [ ] **MENU-04**: Admin can delete cafe items
- [ ] **MENU-05**: Admin can toggle item availability (available/unavailable -- hides from POS/PWA)
- [x] **MENU-06**: Admin can upload item images that display in PWA and POS
- [ ] **MENU-07**: Cafe items display in POS grouped by category with correct pricing
- [ ] **MENU-08**: Cafe items display in PWA grouped by category with images, descriptions, and pricing

### Inventory

- [ ] **INV-01**: Admin can set stock quantities for countable items (bottles, buns, packaged snacks)
- [ ] **INV-02**: Items are categorized as countable (stock-tracked) or uncountable (made-to-order, availability toggle only)
- [x] **INV-03**: Stock auto-decrements when a countable item is sold via POS or PWA order
- [ ] **INV-04**: Admin can manually adjust stock quantities (restock entry when supplies arrive)
- [ ] **INV-05**: Admin can set low-stock threshold per countable item
- [ ] **INV-06**: Low-stock alert fires via WhatsApp when threshold is reached (once per breach, with cooldown)
- [ ] **INV-07**: Low-stock alert displays as warning banner in admin dashboard
- [ ] **INV-08**: Low-stock alert fires via email when threshold is reached
- [ ] **INV-09**: Admin can view inventory dashboard showing all items, current stock, and threshold status

### Ordering & Billing

- [ ] **ORD-01**: Customer can browse cafe menu in PWA, add items to cart, and place order
- [ ] **ORD-02**: Staff can enter cafe orders via POS on behalf of a customer
- [x] **ORD-03**: Order amount deducts from the existing RP customer wallet
- [x] **ORD-04**: Each order generates a unique receipt number and transaction ID
- [x] **ORD-05**: Order prints a thermal receipt for cafe staff to prepare
- [x] **ORD-06**: Order receipt is sent to customer via WhatsApp
- [x] **ORD-07**: Items with zero stock cannot be ordered (out-of-stock blocking)
- [x] **ORD-08**: Order deduction and stock decrement are atomic (no race conditions on concurrent orders)
- [x] **ORD-09**: Customer can view their cafe order history in PWA

### Promotions & Deals

- [ ] **PROMO-01**: Admin can create combo deals (bundle items at discounted price, e.g., Burger + Coke = ₹149)
- [ ] **PROMO-02**: Admin can create happy hour time-based discounts (e.g., 20% off 3-6 PM IST)
- [ ] **PROMO-03**: Admin can create gaming+cafe combo bundles (game session + cafe item at bundle price)
- [ ] **PROMO-04**: Stacking rules defined -- admin can configure which promos can/cannot combine
- [ ] **PROMO-05**: Active promos display on POS and PWA during applicable time windows
- [ ] **PROMO-06**: Promo discounts are applied automatically at checkout when conditions are met

### Marketing

- [ ] **MKT-01**: Auto-generate promo graphics (menu images, daily specials) for Instagram stories/posts
- [ ] **MKT-02**: WhatsApp broadcast of promo messages to customer list (using separate number from operational bot)

## v2 Requirements

Deferred to future release. Tracked but not in current roadmap.

### Marketing (Deferred)

- **MKT-03**: Generate digital posters for in-store display screens
- **MKT-04**: Auto-post generated content directly to Instagram
- **MKT-05**: Marketing content scheduling (auto-publish at specific times)

### Advanced Features (Deferred)

- **ADV-01**: Loyalty points / rewards program for cafe purchases
- **ADV-02**: Purchase order workflow (PO -> receive -> auto-update stock)
- **ADV-03**: Supplier management and auto-reorder when stock is low
- **ADV-04**: Sales analytics dashboard (revenue, margins, popular items, peak hours)
- **ADV-05**: Customer favorite items and quick-reorder

## Out of Scope

| Feature | Reason |
|---------|--------|
| Kitchen display / order queue system | Receipt-based fulfillment sufficient for current cafe scale |
| Separate cafe wallet | Using existing RP wallet -- one balance for gaming + cafe |
| Online delivery / external platforms | In-store only for v1 |
| Ingredient-level inventory tracking | Overkill for bottled drinks and simple prepared foods -- track at sellable-item level |
| Separate cafe app | PWA and POS already exist -- extend them |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| MENU-01 | Phase 150 | Complete |
| MENU-02 | Phase 149 | Pending |
| MENU-03 | Phase 149 | Pending |
| MENU-04 | Phase 149 | Pending |
| MENU-05 | Phase 149 | Pending |
| MENU-06 | Phase 150 | Complete |
| MENU-07 | Phase 151 | Pending |
| MENU-08 | Phase 151 | Pending |
| INV-01 | Phase 152 | Pending |
| INV-02 | Phase 152 | Pending |
| INV-03 | Phase 154 | Complete |
| INV-04 | Phase 152 | Pending |
| INV-05 | Phase 152 | Pending |
| INV-06 | Phase 153 | Pending |
| INV-07 | Phase 153 | Pending |
| INV-08 | Phase 153 | Pending |
| INV-09 | Phase 152 | Pending |
| ORD-01 | Phase 154 | Pending |
| ORD-02 | Phase 154 | Pending |
| ORD-03 | Phase 154 | Complete |
| ORD-04 | Phase 154 | Complete |
| ORD-05 | Phase 155 | Complete |
| ORD-06 | Phase 155 | Complete |
| ORD-07 | Phase 154 | Complete |
| ORD-08 | Phase 154 | Complete |
| ORD-09 | Phase 155 | Complete |
| PROMO-01 | Phase 156 | Pending |
| PROMO-02 | Phase 156 | Pending |
| PROMO-03 | Phase 156 | Pending |
| PROMO-04 | Phase 156 | Pending |
| PROMO-05 | Phase 157 | Pending |
| PROMO-06 | Phase 157 | Pending |
| MKT-01 | Phase 158 | Pending |
| MKT-02 | Phase 158 | Pending |

**Coverage:**
- v1 requirements: 34 total
- Mapped to phases: 34
- Unmapped: 0

---
*Requirements defined: 2026-03-22*
*Last updated: 2026-03-22 after roadmap creation*
