# v19.0 Cafe Inventory, Ordering & Marketing

## What This Is

A complete cafe operations system for Racing Point eSports and Cafe — menu management, customer ordering (self-service via PWA + staff-assisted via POS), inventory tracking with low-stock alerts, promotional deals, and automated marketing content generation. Built on top of the existing racecontrol platform, sharing the existing customer wallet for unified gaming + cafe billing.

## Core Value

Customers can browse the cafe menu, place orders, and pay from their existing RP wallet — with staff always knowing what's in stock and promos driving revenue.

## Requirements

### Validated

(None yet — ship to validate)

### Active

**Menu & Item Management (Admin)**
- [ ] Admin can upload cafe items from PDF/spreadsheet (name, price, category, cost price, description)
- [ ] Admin can manually add/edit/delete cafe items with name, description, category, selling price, and cost price
- [ ] Cafe items display in POS and PWA with correct pricing and categories
- [ ] Admin can mark items as available/unavailable (toggles visibility in POS/PWA)

**Inventory Tracking**
- [ ] Admin can set stock quantities for countable items (buns, water bottles, Diet Coke, etc.)
- [ ] Stock auto-decrements when items are sold via POS or PWA orders
- [ ] Admin can manually adjust stock (restock entry when supplies arrive)
- [ ] Admin can set low-stock thresholds per item
- [ ] Low-stock alerts fire via WhatsApp, admin dashboard banner, and email when threshold is hit

**Ordering & Billing**
- [ ] Customers can browse cafe menu and place orders via PWA (self-service)
- [ ] Staff can enter cafe orders via POS on behalf of customers
- [ ] Orders deduct from the existing RP customer wallet
- [ ] Each order generates a receipt number and transaction ID
- [ ] Orders print a receipt for cafe staff to prepare
- [ ] Items that are out-of-stock cannot be ordered

**Promotions & Deals**
- [ ] Admin can create combo deals (e.g., Burger + Coke = ₹149 instead of ₹180)
- [ ] Admin can create happy hour time-based discounts (e.g., 20% off 3-6 PM)
- [ ] Admin can create gaming+cafe combo bundles (game session + cafe item at bundle price)
- [ ] Active promos display on POS and PWA during applicable times

**Marketing & Content**
- [ ] Auto-generate menu images / promo graphics for Instagram stories/posts
- [ ] WhatsApp broadcast of promo messages to customer list
- [ ] Generate digital posters for in-store display screens
- [ ] Marketing content reflects current menu, prices, and active promos

### Out of Scope

- Kitchen display / order queue system — receipt-based fulfillment is sufficient for current scale
- Separate cafe wallet — using existing RP wallet
- Purchase order workflow — manual restock entry is sufficient
- Loyalty points / rewards program — defer to v20+
- Online delivery / external ordering platforms — in-store only
- Supplier management / auto-reorder — manual for now

## Context

**Existing infrastructure:**
- POS system already exists with partial item structure — needs extension for cafe-specific fields (category, cost price, description, stock tracking)
- PWA exists for customer-facing features — needs cafe menu browsing and ordering flow
- Admin dashboard exists — needs cafe management pages
- Customer wallet system already built — shared balance for gaming + cafe
- WhatsApp bot operational (staff: 7075778180, customer: 9059833001/9054548180)
- Email system available (Gmail OAuth working)

**Data import:**
- Initial cafe menu will be provided as PDF or spreadsheet
- Contains: item name, selling price, category, cost price, description
- No images in import — photos added manually later if needed

## Constraints

- **Stack**: Must integrate with existing racecontrol web dashboard (Next.js :3200), POS, PWA, and admin
- **Wallet**: Must use existing wallet deduction flow — no separate payment system
- **Receipts**: Must support thermal receipt printing (existing POS printer infrastructure)
- **Alerts**: WhatsApp alerts go through existing comms-link bot infrastructure
- **Data**: Item/stock data stored in existing database (server-side JSON or DB)

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Shared wallet for gaming + cafe | Simpler for customers, one balance to manage | — Pending |
| Receipt printing (no kitchen display) | Current cafe scale doesn't need order queue system | — Pending |
| Manual restock only | No supplier integration needed at current scale | — Pending |
| Three-channel alerts (WhatsApp + dashboard + email) | Ensure staff never misses low-stock warnings | — Pending |

---
*Last updated: 2026-03-22 after initialization*
