# Phase 341: POS + Kiosk Display - Context

**Gathered:** 2026-04-07
**Status:** Ready for planning

<domain>
## Phase Boundary

Replace ₹ symbol with "credits" on all customer-facing wallet balance displays. This applies ONLY to wallet/session credit displays, NOT to cafe pricing (which remains in rupees). Verify kiosk and PWA already show credits correctly.

**Depends on:** Phase 339 (API now returns `balance_credits` field name)
**Scope:** web/ app (drivers page, billing page) and kiosk/ app (verify only)

</domain>

<decisions>
## Implementation Decisions

### Drivers Page (web/src/app/drivers/page.tsx)
- **D-01:** Line 82: Change `{"\u20B9"}{Math.floor((driver.wallet_balance_paise ?? 0) / 100)}` to show "X credits" instead of "₹X"
- **D-02:** Since Phase 339 renamed API field to `balance_credits`, also update field reference from `wallet_balance_paise` to `balance_credits` if the API response uses the new name. Verify which field name the drivers list API returns.
- **D-03:** Display format: `{value} credits` (no currency symbol, "credits" suffix)

### POS Billing Page (web/src/app/billing/)
- **D-04:** Check if billing page shows wallet balance with ₹ symbol — if so, change to credits
- **D-05:** Session pricing (₹700/30min etc.) stays in rupees — these are real-money prices, not credits

### Kiosk (kiosk/src/)
- **D-06:** Kiosk CafeMenuPanel.tsx uses `Rs.` for cafe menu prices — this is CORRECT (cafe items are priced in rupees, not credits). No change needed.
- **D-07:** Verify kiosk session pricing pages show credits if they display wallet balance. If they only show session prices (₹700/hr), that's correct — no change.

### PWA Wallet
- **D-08:** Verify PWA wallet display already shows "credits" (likely already correct from Phase 339 serde renames). If not, fix.

### Out of Scope
- **D-09:** Cafe pricing (web/cafe, kiosk cafe) stays in rupees — these are real-money transactions
- **D-10:** Booking page ₹ symbols stay — bookings are real-money pricing
- **D-11:** Analytics/EBITDA page ₹ stays — financial reporting uses rupees
- **D-12:** Cafe menu graphic generator (`generate-graphic/route.tsx`) stays in ₹ — cafe pricing

### Claude's Discretion
- Whether to create a shared `formatCredits()` utility or inline the change
- How to handle the transition if API field name hasn't changed on the drivers list endpoint (drivers list may return different fields than wallet info)

</decisions>

<specifics>
## Specific Ideas

- The key change is drivers/page.tsx line 82: one line replacement from ₹ to credits format
- POS billing and kiosk may already be correct — SC-3 and SC-4 say "verify already correct"
- This is a minimal phase — mostly verification with one confirmed code change

</specifics>

<canonical_refs>
## Canonical References

### Files to modify
- `web/src/app/drivers/page.tsx` line 82 — ₹ symbol to credits (confirmed via grep)

### Files to verify (may already be correct)
- `web/src/app/billing/` — POS billing page wallet display
- `kiosk/src/` — session pricing display
- Kiosk wallet/PWA pages (if they exist)

### Phase 339 API contract
- `.planning/phases/339-api-endpoints/339-CONTEXT.md` — field names

</canonical_refs>

<code_context>
## Existing Code Insights

### Confirmed ₹ Usage (grep results)
- `web/src/app/drivers/page.tsx:82` — `{"\u20B9"}{Math.floor((driver.wallet_balance_paise ?? 0) / 100)}` — MUST CHANGE
- `web/src/app/book/page.tsx:150,159` — booking prices — OUT OF SCOPE (real money)
- `web/src/app/analytics/ebitda/page.tsx:22` — EBITDA — OUT OF SCOPE (financial reporting)
- `web/src/app/cafe/page.tsx` — cafe pricing — OUT OF SCOPE (real money)
- `web/src/app/page.tsx:30` — revenue dashboard — OUT OF SCOPE (financial reporting)
- `kiosk/src/components/CafeMenuPanel.tsx:8-9` — cafe prices — OUT OF SCOPE (real money)

### No ₹ found in
- `kiosk/src/` (except CafeMenuPanel which is correct)
- Session pricing pages (these use API values, likely already in credits format)

</code_context>

<deferred>
## Deferred Ideas

None — this is a focused find-and-replace phase.

</deferred>

---

*Phase: 341-pos-kiosk-display*
*Context gathered: 2026-04-07*
*[auto] Grep-based analysis of ₹ symbol usage across web/ and kiosk/ codebases.*
