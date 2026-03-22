# Phase 10: Operational Hardening - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning
**Source:** Smart Discuss (infrastructure phase)

<domain>
## Phase Boundary

Verify and harden production edge cases: extended outages (cloud bookings queue and resolve), brute force protection (rate limiting on auth), and sync conflict handling. Much of this is already implemented — this phase validates and fills gaps.

**Key discovery:** Rate limiting already exists (tower_governor 5/min on auth routes). Cloud sync already handles reservations bidirectionally with 2s relay + 30s HTTP fallback. Reservations use pending_debit status that resolves when local processes debit_intents. The outage queue behavior is effectively built into the sync architecture.

</domain>

<decisions>
## Implementation Decisions

### What Already Works
- Rate limiting: tower_governor 5 req/min per IP on auth_rate_limited_routes (login, OTP verify, PIN redeem)
- Cloud sync: 2s relay interval + 30s HTTP fallback — reservations sync bidirectionally
- Debit intents: pending → completed flow handles offline-created bookings
- Relay hysteresis: consecutive failure thresholds prevent flapping

### What May Need Verification/Hardening
- SYNC-05: Verify that during an extended outage, cloud bookings with pending_debit status survive and resolve when connectivity returns — may need an E2E test scenario
- API-05: Verify rate limiting covers all auth-adjacent endpoints (login, OTP verify, PIN entry) — may already be complete
- Pending booking timeout: 24h TTL cleanup in scheduler already handles expired reservations

### Claude's Discretion
- Whether to add new code or just write verification tests
- Any additional hardening beyond the two requirements

</decisions>

<code_context>
## Existing Code Insights

### Rate Limiting (API-05 — likely already done)
- `crates/racecontrol/src/api/routes.rs` — `auth_rate_limited_routes()` with tower_governor 5/min
- Covers: `/customer/login`, `/customer/verify-otp`, `/kiosk/redeem-pin`

### Sync Queue (SYNC-05 — likely already done)
- `crates/racecontrol/src/cloud_sync.rs` — bidirectional sync with relay + HTTP fallback
- Reservations and debit_intents are in SYNC_TABLES
- `process_debit_intents()` handles pending → completed flow
- Relay hysteresis prevents flapping during outages

### Integration Points
- cloud_sync.rs → reservation.rs (debit intent processing)
- routes.rs → auth rate limiting layer

</code_context>

<specifics>
## Specific Ideas

- This may be a validation-only phase — verify existing implementations satisfy SYNC-05 and API-05
- If gaps found, add targeted fixes

</specifics>

<deferred>
## Deferred Ideas

None

</deferred>

---

*Phase: 10-operational-hardening*
*Context gathered: 2026-03-22 via Smart Discuss (infrastructure phase)*
