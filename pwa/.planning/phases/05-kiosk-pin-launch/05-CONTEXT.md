# Phase 5: Kiosk PIN Launch - Context

**Gathered:** 2026-03-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Customer arrives at venue, enters their 6-char PIN at the kiosk screen, local server validates the PIN against synced reservations, assigns first available pod, and auto-launches the game. PIN is one-time use, marked redeemed immediately. Rate limiting prevents brute force. Customer sees assigned pod number and loading status.

This is the venue-side completion of the remote booking flow started in Phase 4.

</domain>

<decisions>
## Implementation Decisions

### PIN Entry UI
- New PIN entry screen on the kiosk — accessible from the kiosk home/booking page
- Large numpad-style input for 6-character alphanumeric PIN
- Clear visual feedback: each character fills a box as entered
- Submit button validates against local server
- Success: shows assigned pod number + "Head to Pod X" with game loading indicator
- Failure: "Invalid PIN" with attempt counter, lockout message after 10 failures

### Backend PIN Validation
- New endpoint: POST /kiosk/redeem-pin (or similar) on local server
- Validates PIN exists in local reservations table (synced from cloud via Phase 3)
- Checks reservation status is "confirmed" (not expired/cancelled/already redeemed)
- Assigns first available pod (using existing pod assignment logic)
- Marks reservation as "redeemed" immediately (one-time use)
- Triggers game launch on assigned pod (using existing game launch flow)
- Returns pod number and game loading status to kiosk

### Rate Limiting
- Track PIN attempts per kiosk (by kiosk IP or session)
- Max 5 attempts per minute
- Lockout after 10 consecutive failures — 5 minute cooldown
- Show remaining attempts and lockout timer on UI

### Pod Assignment
- Use existing pod assignment logic from pod_reservation.rs
- First available pod (no specific pod promised in remote booking)
- If no pods available: "All pods busy — please wait" with retry option
- Once assigned, game launches automatically via rc-agent

### Claude's Discretion
- Exact kiosk PIN entry component styling (should match existing kiosk design language)
- Rate limiting storage mechanism (in-memory vs SQLite)
- Whether to show a QR code scanner as alternative to PIN entry
- Game loading animation/progress indicator design
- How to handle the gap between "redeemed" and "game actually launched"

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Kiosk Codebase
- `kiosk/.planning/codebase/ARCHITECTURE.md` — Kiosk app architecture, routes, data flow
- `kiosk/.planning/codebase/STRUCTURE.md` — Directory layout, component locations
- `kiosk/src/app/book/page.tsx` — Existing booking flow on kiosk (pattern for new PIN entry)
- `kiosk/src/components/PodKioskView.tsx` — Pod display component
- `kiosk/src/components/SetupWizard.tsx` — Existing wizard pattern

### Backend
- `crates/racecontrol/src/reservation.rs` — Reservation CRUD + PIN validation (Phase 4)
- `crates/racecontrol/src/pod_reservation.rs` — Existing pod assignment logic
- `crates/racecontrol/src/api/routes.rs` — Existing kiosk endpoints pattern
- `crates/racecontrol/src/db/mod.rs` — Reservations table schema

### Project Guidelines
- `racecontrol/CLAUDE.md` — Deploy rules, naming conventions, dev rules

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `PodKioskView` component — shows pod status, can be reused for "Head to Pod X" screen
- `SetupWizard` — step-by-step flow component, could be adapted for PIN → pod assignment → game launch
- `StaffLoginScreen` — has a PIN-style input pattern that could be reference
- Existing pod assignment in `pod_reservation.rs` — `assign_pod()` or similar function
- Game launch via rc-agent WebSocket — existing pattern in kiosk

### Established Patterns
- Kiosk uses WebSocket for real-time pod state
- Staff login uses PIN-style input
- Booking flow is a multi-step wizard
- All kiosk API calls go to local server (192.168.31.23:8080)

### Integration Points
- New kiosk route/page for PIN entry
- New backend endpoint for PIN redemption + pod assignment
- Existing game launch flow triggered after pod assignment
- Rate limiting middleware or in-handler tracking

</code_context>

<specifics>
## Specific Ideas

- PIN entry should feel seamless — type PIN, press enter, pod assigned, walk to pod
- The "Head to Pod X" screen should be unmissable — large pod number, clear direction
- Consider auto-focusing the PIN input field when the page loads
- Lockout should show a countdown timer, not just "try again later"

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 05-kiosk-pin-launch*
*Context gathered: 2026-03-21*
