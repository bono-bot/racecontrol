# Phase 154: Ordering Core - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Core cafe ordering: PWA self-service + POS staff ordering, wallet deduction, atomic stock decrement, out-of-stock blocking, receipt/transaction ID generation. This is the core value delivery phase.

</domain>

<decisions>
## Implementation Decisions

### Order Flow & Transactions
- SQLite transaction for atomicity: BEGIN → check stock → decrement stock → wallet debit → insert order → COMMIT
- Receipt number format: RP-YYYYMMDD-NNNN (sequential per day, e.g., RP-20260322-0001)
- Simple order status: pending → confirmed. No cancellation in v1. Once wallet debited, order is confirmed
- Each order gets a UUID as transaction_id plus the human-readable receipt number

### Cart & Checkout UX
- PWA cart: React state only (lost on refresh). Simple for v1
- POS: Quick-add flow — staff taps items to add to order on /cafe page, sees running total, confirms with customer ID
- Wallet balance displayed at checkout — warn if insufficient before attempting order

### Stock Integration
- Atomic stock decrement within the order transaction
- Out-of-stock items (stock_quantity = 0 for countable items) blocked from ordering
- Call check_low_stock_alerts() from Phase 153 after each stock decrement
- Uncountable items skip stock checks entirely

### Data Model
- New cafe_orders table: id, receipt_number, driver_id, items (JSON), total_paise, wallet_txn_id, status, created_at
- Wallet debit uses txn_type = "cafe_order", reference_id = order_id

### Claude's Discretion
- Exact cart UI layout and animations
- Order confirmation screen design
- Error handling for edge cases (wallet debit succeeds but stock decrement fails — unlikely with single SQLite txn)

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/racecontrol/src/wallet.rs` — `debit()` with atomic `WHERE balance_paise >= ?`, returns `(new_balance, txn_id)`
- `crates/racecontrol/src/cafe.rs` — item CRUD, stock fields, restock handler
- `crates/racecontrol/src/cafe_alerts.rs` — `check_low_stock_alerts()` to call after stock decrement
- `crates/racecontrol/src/accounting.rs` — double-entry journal auto-posted by wallet debit

### Integration Points
- `cafe.rs` — add place_order handler, cafe_orders table, receipt number generator
- `routes.rs` — add POST /cafe/orders endpoint (authenticated for both staff + customer)
- `pwa/src/app/cafe/page.tsx` — add cart state + checkout flow
- `kiosk/src/components/CafeMenuPanel.tsx` — add "Add to Order" + checkout for POS

</code_context>

<specifics>
## Specific Ideas

No specific requirements beyond stated scope.

</specifics>

<deferred>
## Deferred Ideas

None.

</deferred>
