# Phase 152: Inventory Tracking - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Stock management for cafe items: countable vs uncountable categorization, stock quantities, manual restock, low-stock thresholds, and an inventory dashboard in the admin panel. Extends cafe_items schema and admin /cafe page from Phase 149.

</domain>

<decisions>
## Implementation Decisions

### Data Model
- Add inventory columns to existing cafe_items table: is_countable BOOLEAN, stock_quantity INTEGER, low_stock_threshold INTEGER
- Countable items (bottles, buns, packaged snacks) have stock tracked; uncountable items (chai, coffee from pot) use availability toggle only
- is_countable set per item during creation/editing in admin

### Admin UI
- Extend existing /cafe admin page with inventory columns in the item table (Stock, Threshold, Type)
- "Restock" action button per item — opens small input to add quantity
- Inventory dashboard view: toggle between "Items" and "Inventory" tabs on the same /cafe page
- Inventory tab shows: all items with current stock, threshold status (green/yellow/red), countable badge

### Restock Flow
- Manual entry only — staff enters restock quantity via admin panel
- Restock adds to current quantity (not replaces)
- Log restock events for audit trail (optional, Claude's discretion)

### Claude's Discretion
- Exact threshold color coding logic (green/yellow/red)
- Whether to add a separate restock history table or just increment
- Sort order for inventory view (low-stock first vs alphabetical)

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/racecontrol/src/cafe.rs` — existing CRUD handlers + cafe_items table
- `crates/racecontrol/src/db/mod.rs` — migration pattern (ALTER TABLE ADD COLUMN)
- `web/src/app/cafe/page.tsx` — existing admin page to extend with inventory columns

### Integration Points
- `db/mod.rs` — add is_countable, stock_quantity, low_stock_threshold columns
- `cafe.rs` — update CafeItem struct, add restock handler, update create/edit to include inventory fields
- `page.tsx` — add inventory columns + restock button + threshold badges

</code_context>

<specifics>
## Specific Ideas

No specific requirements beyond the stated scope.

</specifics>

<deferred>
## Deferred Ideas

None.

</deferred>
