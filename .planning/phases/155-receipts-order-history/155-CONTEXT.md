# Phase 155: Receipts & Order History - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Receipt generation (thermal print + WhatsApp delivery) and customer order history in PWA. Extends the ordering flow from Phase 154.

</domain>

<decisions>
## Implementation Decisions

### Thermal Receipt
- Generate receipt text server-side after order confirmation
- Send to POS printer via existing printing infrastructure (or node-thermal-printer if available)
- Receipt includes: receipt number, date/time, item list with prices, total, customer name
- Auto-print on staff POS orders; customer PWA orders trigger print at cafe station

### WhatsApp Receipt
- Send receipt summary to customer's phone via comms-link WhatsApp bot
- Format: text message with receipt number, items, total, balance remaining
- Use existing comms-link send-message.js or direct Evolution API pattern from cafe_alerts

### Order History
- New /orders page in PWA showing customer's past cafe orders
- List view: receipt number, date, total, item count
- Tap to expand: full item list with prices
- API endpoint: GET /customer/cafe/orders (JWT authenticated, returns only that customer's orders)

### Claude's Discretion
- Receipt text formatting and line widths
- WhatsApp message template
- Order history pagination (or load all for v1)
- Print error handling (queue retry vs fail-silent)

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/racecontrol/src/cafe.rs` — cafe_orders table with receipt_number, items JSON
- `crates/racecontrol/src/cafe_alerts.rs` — WhatsApp send pattern via Evolution API
- `crates/racecontrol/src/whatsapp_alerter.rs` — WhatsApp HTTP client pattern
- POS PC at 192.168.31.20 — thermal printer connected

### Integration Points
- `cafe.rs` — add order history endpoint, receipt generation function
- `routes.rs` — add GET /customer/cafe/orders
- `pwa/src/app/` — add orders page or orders tab

</code_context>

<specifics>
## Specific Ideas

No specific requirements.

</specifics>

<deferred>
## Deferred Ideas

None.

</deferred>
