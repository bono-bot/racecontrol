# Phase 156: Promotions Engine - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Promo data model, admin CRUD for three promo types (combo deals, happy hour, gaming+cafe bundles), stacking rules configuration. This phase builds the engine — Phase 157 integrates it into checkout.

</domain>

<decisions>
## Implementation Decisions

### Promo Types
- Combo deal: bundle specific items at discounted price (e.g., Burger + Coke = 149 instead of 180)
- Happy hour: time-based % or flat discount on items/categories (e.g., 20% off beverages 3-6 PM IST)
- Gaming+cafe bundle: game session + cafe item at bundle price (links to billing session)

### Data Model
- New cafe_promos table: id, name, promo_type (combo/happy_hour/gaming_bundle), config (JSON), is_active, start_time, end_time, created_at
- Config JSON varies by type — combo: {items: [{id, qty}], bundle_price_paise}, happy_hour: {discount_percent or discount_paise, applies_to: category|item|all, target_ids}, gaming: {session_duration_mins, cafe_item_ids, bundle_price_paise}
- Stacking rules: separate cafe_promo_stacking table or stacking_group field — promos in same group are exclusive

### Admin UI
- New "Promos" tab on /cafe admin page
- Create/edit promo form with type selector that changes fields dynamically
- Active/inactive toggle per promo
- Time window picker for happy hour (start/end time in IST)

### Claude's Discretion
- Exact stacking rule implementation (group-based vs pairwise exclusion)
- Promo validation rules (e.g., combo must have 2+ items)
- Admin form layout for each promo type

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/racecontrol/src/cafe.rs` — cafe CRUD patterns, cafe_items table
- `crates/racecontrol/src/db/mod.rs` — migration pattern
- `web/src/app/cafe/page.tsx` — admin page with tabs (Items, Inventory)

### Integration Points
- `db/mod.rs` — add cafe_promos table
- `cafe.rs` or new `cafe_promos.rs` — promo CRUD handlers
- `routes.rs` — register promo admin endpoints
- `page.tsx` — add Promos tab with CRUD UI

</code_context>

<specifics>
## Specific Ideas

No specific requirements.

</specifics>

<deferred>
## Deferred Ideas

None.

</deferred>
