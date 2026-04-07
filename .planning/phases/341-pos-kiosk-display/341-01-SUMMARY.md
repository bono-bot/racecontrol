---
plan: 341-01
phase: 341-pos-kiosk-display
status: complete
started: 2026-04-07
completed: 2026-04-07
---

# Plan 341-01: POS + Kiosk Display Credits

## Status: Complete (deploy deferred)

## What Was Done

### Task 1: Fix drivers page (committed 836860bd)
- Changed `web/src/app/drivers/page.tsx` line 82 from `₹X` format to `X credits`
- Replaced `{"\u20B9"}{Math.floor((driver.wallet_balance_paise ?? 0) / 100)}` with credits display

### Task 2: Verification (grep-based)
- POS billing directory: ZERO rupee symbols for wallet balances
- Kiosk: already uses "credits" in all 4 key locations (PricingDisplay.tsx, PodKioskView.tsx, LiveSessionPanel.tsx, staff/page.tsx)
- Cafe pricing correctly uses rupee formatting (real money, not credits) — untouched
- Visual verification deferred until deploy

## Key Files
- `web/src/app/drivers/page.tsx` — wallet display changed from ₹ to credits

## Deviations
None.

## Deploy Status
Code committed. Web app rebuild + deploy to server .23 and cloud deferred.
