# Phase 4: Remote Booking + PIN Generation - Context

**Gathered:** 2026-03-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Customer books an experience from the PWA (cloud), receives a 6-char alphanumeric PIN via WhatsApp, and can view/cancel/modify the reservation. Backend creates pod-agnostic reservations using the `reservations` table (created in Phase 3), generates PINs, sends WhatsApp messages, handles expiry cleanup, and wallet debit via debit_intents. PWA gets new booking flow pages.

Key constraint: One customer, one account, one active reservation at a time.

</domain>

<decisions>
## Implementation Decisions

### Reservation Flow
- Customer selects experience + car/track + duration tier on PWA booking page
- Cloud API creates reservation in `reservations` table (cloud-authoritative, synced in Phase 3)
- Pod is NOT assigned at booking time — reservation is pod-agnostic
- PIN: 6-character alphanumeric (A-Z0-9, excluding ambiguous chars O/0/I/1/L), generated server-side
- Reservation status state machine: pending → redeemed | expired | cancelled
- One active reservation per customer enforced at API level

### Wallet Integration
- Booking debits wallet via debit_intent pattern (Phase 3 infrastructure)
- Cloud creates debit_intent, local processes it, balance syncs back
- Cancellation creates a credit/refund debit_intent (negative amount or separate refund mechanism)
- Expired reservation cleanup refunds wallet automatically

### WhatsApp PIN Delivery
- Use existing WhatsApp integration (OTP sending pattern) to deliver PIN after booking
- Message template: "Your Racing Point PIN: {PIN}. Valid for 24 hours. Show this at the kiosk when you arrive."
- If WhatsApp delivery fails, PIN is still displayed in PWA (WhatsApp is convenience, not critical path)

### Expiry & Cleanup
- Default TTL: 24 hours from booking time
- Background scheduler task runs every 5 minutes to mark expired reservations
- Expired reservations with wallet debit get automatic refund via debit_intent

### PWA Booking UI
- New `/book` flow for remote booking (extending existing booking page)
- Booking confirmation page shows PIN prominently
- `/reservations` page to view/cancel/modify active reservation
- Modification: can change experience/duration but not extend beyond original TTL

### Claude's Discretion
- Exact PIN generation algorithm (cryptographically secure random)
- Background scheduler implementation (tokio::spawn interval vs cron-like)
- PWA booking UI component layout and styling
- Error handling UX for failed bookings
- WhatsApp message template exact wording

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Sync Infrastructure (Phase 3)
- `crates/racecontrol/src/db/mod.rs` — reservations + debit_intents table schemas (created in Phase 3)
- `crates/racecontrol/src/cloud_sync.rs` — sync integration, debit intent processing, origin tags
- `crates/racecontrol/src/api/routes.rs` — existing sync endpoints, sync_health

### Existing Booking System
- `crates/racecontrol/src/api/routes.rs` — existing `/customer/book` endpoint pattern
- `pwa/src/lib/api.ts` — existing booking API client methods
- `pwa/src/app/book/page.tsx` — existing booking UI

### WhatsApp Integration
- `crates/racecontrol/src/api/routes.rs` — existing OTP/WhatsApp sending pattern

### Project Guidelines
- `racecontrol/CLAUDE.md` — deploy rules, naming conventions, dev rules

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `api.bookSession()` / `api.bookCustom()` — existing booking API methods in PWA
- `/customer/book` endpoint — existing booking flow in routes.rs
- WhatsApp OTP sender — existing pattern for sending WhatsApp messages
- `reservations` table — created in Phase 3 with PIN, status, expires_at fields
- `debit_intents` table — created in Phase 3 for wallet operations

### Established Patterns
- Booking: POST /customer/book with experience_id, tier, game selection
- Auth: JWT Bearer token on all customer endpoints
- State management: useState + useEffect in PWA pages
- API client: fetchApi wrapper in src/lib/api.ts

### Integration Points
- New reservation API endpoints in routes.rs
- New PWA pages: booking confirmation, reservation management
- Background scheduler for expiry cleanup (new module or addition to existing scheduler.rs)
- WhatsApp message delivery for PIN

</code_context>

<specifics>
## Specific Ideas

- PIN should be visually prominent on confirmation screen (large font, copy-to-clipboard button)
- Booking flow should feel smooth — select game → select car/track → select duration → confirm → PIN shown
- WhatsApp message should include venue address for navigation convenience

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 04-remote-booking-pin-generation*
*Context gathered: 2026-03-21*
