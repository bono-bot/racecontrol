# Phase 94 Plan 02 Summary: Frontend Pricing UI

**Status:** Complete
**Commit:** c12fecec

## What was built

1. **Kiosk PricingDisplay** (`kiosk/src/components/PricingDisplay.tsx`) — 3-tier anchoring card layout. Middle tier highlighted with Racing Red border, `scale-105`, "Most Popular" badge. Strikethrough original price when dynamic pricing differs. Trial tier shown separately as "Try for Free" if unused. 30s polling.

2. **Kiosk ScarcityBanner** (`kiosk/src/components/ScarcityBanner.tsx`) — Real-time pod availability from `/fleet/health`. Color gradient: green (5-8), yellow (2-4), red (0-1). Zero availability shows "All pods in use — next slot likely in ~30min". 10s polling.

3. **SetupWizard integration** — Replaced flat tier list in `select_plan` step with `<ScarcityBanner />` + `<PricingDisplay />`. `handleSelectTier` receives `dynamic_price_paise` mapped to `price_paise`.

4. **Web /book page** (`web/src/app/book/page.tsx`) — Standalone customer-facing page (no auth, no DashboardLayout). 3-tier anchoring pricing, scarcity banner, social proof bar ("X drivers this week", "Y sessions today"). Shows Rupee symbol. Zero counts show "Be the first!" messages.

5. **fetchPublic helper** (`web/src/lib/api.ts`) — Public endpoint fetcher without auth headers or 401 redirect.

## Verification
- Kiosk: `npx next build` passes, `/book` route exists
- Web: `npx next build` passes, `/book` route exists
- No `fetchApi` or `DashboardLayout` imports in /book page
