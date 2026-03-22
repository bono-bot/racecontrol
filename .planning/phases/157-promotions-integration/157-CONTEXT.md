# Phase 157: Promotions Integration - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Wire promo engine (Phase 156) into the ordering flow (Phase 154). Active promos display in POS/PWA, discounts auto-apply at checkout, applied promo recorded with order. Server-side price calculation only.

</domain>

<decisions>
## Implementation Decisions

### Promo Display
- Public API endpoint returns currently active promos (time-window checked server-side in IST)
- POS and PWA show active promos as banners/badges on applicable items
- Happy hour shows countdown or "Active until X:XX PM"

### Auto-Apply at Checkout
- Server-side promo evaluation in place_cafe_order — calculate best applicable promo(s) before wallet debit
- Stacking rules enforced: promos in same stacking_group are exclusive (pick best discount)
- Applied promo ID and discount amount stored with the order record
- Client sends cart items; server calculates final price (no client-side discount calculation)

### Promo Evaluation Logic
- Combo: if all required items present in cart with sufficient qty, apply bundle price
- Happy hour: if current IST time within window AND items match target, apply discount
- Gaming bundle: if active billing session exists for customer AND cafe items match, apply bundle price

### Claude's Discretion
- Exact promo evaluation order (combos first vs happy hour first)
- How to display multiple applicable promos to customer
- Discount amount display format in cart

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/racecontrol/src/cafe_promos.rs` — promo CRUD, CafePromo struct with config JSON
- `crates/racecontrol/src/cafe.rs` — place_cafe_order_inner (insert promo evaluation before wallet debit)
- `pwa/src/app/cafe/page.tsx` — PWA cart + checkout flow
- `kiosk/src/components/CafeMenuPanel.tsx` — POS order builder

### Integration Points
- `cafe.rs` — add evaluate_promos() function, modify place_cafe_order to apply discounts
- `cafe_promos.rs` — add list_active_promos() public endpoint
- PWA + POS — fetch active promos, show badges, display applied discount at checkout

</code_context>

<specifics>
## Specific Ideas

No specific requirements.

</specifics>

<deferred>
## Deferred Ideas

None.

</deferred>
