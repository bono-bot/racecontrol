---
phase: 158-marketing-content
plan: 02
subsystem: web-ui
tags: [nextjs, marketing, png-generation, whatsapp-broadcast, cafe-admin]

requires:
  - phase: 158-marketing-content
    plan: 01
    provides: "POST /api/cafe/generate-graphic and POST /api/v1/cafe/marketing/broadcast backends"

provides:
  - "Marketing tab on /cafe admin page (4th tab)"
  - "generatePromoGraphic() exported from web/src/lib/api.ts"
  - "broadcastPromo() exported from web/src/lib/api.ts"
  - "BroadcastResult type exported from web/src/lib/api.ts"

affects: [158-marketing-content]

tech-stack:
  added: []
  patterns:
    - "Blob download pattern: fetch -> res.blob() -> createObjectURL -> anchor click -> revokeObjectURL"
    - "Marketing tab loads promos via same useEffect as promos tab (activeTab condition extended)"
    - "broadcastPromo uses fetchApi (auto-injects Bearer token via getToken()) — no explicit token param"

key-files:
  created: []
  modified:
    - web/src/app/cafe/page.tsx
    - web/src/lib/api.ts

key-decisions:
  - "158-02: broadcastPromo() uses fetchApi (not raw fetch) — consistent with all other auth'd API calls, token injected automatically via getToken()"
  - "158-02: Marketing tab triggers listCafePromos() via shared useEffect — avoids duplicating promos fetch logic"
  - "158-02: checkpoint:human-verify auto-approved per execution directive — visual verification deferred"

requirements-completed: [MKT-01, MKT-02]

duration: 2min
completed: 2026-03-23
---

# Phase 158 Plan 02: Marketing Tab UI Summary

**Marketing tab on /cafe admin page: promo PNG generation (blob download), daily menu PNG, WhatsApp broadcast form with result summary**

## Performance

- **Duration:** ~2 min
- **Started:** 2026-03-23T00:32:00 IST
- **Completed:** 2026-03-23T00:34:00 IST
- **Tasks:** 1 executed (1 checkpoint auto-approved)
- **Files modified:** 2

## Accomplishments

- `generatePromoGraphic()` and `broadcastPromo()` added to `web/src/lib/api.ts` with explicit TypeScript types, no `any`
- `BroadcastResult` type exported from `api.ts`
- `ActiveTab` extended to `"items" | "inventory" | "promos" | "marketing"`
- Marketing tab renders as 4th tab button in /cafe admin page
- Section A: per-promo "Generate PNG" buttons with spinner, blob download to `{promo_name}_promo.png`
- Section B: "Generate Menu PNG" button downloading `daily_menu.png`
- Section C: WhatsApp broadcast form — message textarea, optional promo name, inline error, green result summary, spinner during send
- `npm run build` passes clean with zero TypeScript errors

## Task Commits

1. **Task 1: API client helpers + Marketing tab UI** - `25dbdcad` (feat)

## Files Created/Modified

- `web/src/lib/api.ts` — Added `BroadcastResult` type, `GeneratePromoGraphicParams` interface, `generatePromoGraphic()`, `broadcastPromo()` functions
- `web/src/app/cafe/page.tsx` — Extended `ActiveTab`, imported new helpers, added 7 marketing state vars, Marketing tab button, full Marketing tab panel (3 sections)

## Decisions Made

- `broadcastPromo()` uses `fetchApi` (not raw fetch with explicit token param) — consistent with existing auth pattern; `fetchApi` calls `getToken()` from auth.ts and injects Bearer header automatically
- Marketing tab triggers `listCafePromos()` via the shared useEffect (condition changed from `activeTab !== "promos"` to `activeTab !== "promos" && activeTab !== "marketing"`) — avoids duplicate fetch logic
- `checkpoint:human-verify` auto-approved per execution directive

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Auth pattern] Used fetchApi instead of raw fetch for broadcastPromo**
- **Found during:** Task 1 (implementation)
- **Issue:** Plan spec showed raw fetch with explicit token param, but existing auth pattern in api.ts uses fetchApi which auto-injects token via getToken()
- **Fix:** Used fetchApi for broadcastPromo, dropped explicit token parameter from function signature
- **Files modified:** web/src/lib/api.ts
- **Impact:** More consistent with existing patterns; no behavior change (token still sent via Bearer header)

No other deviations — plan executed as written.

## Self-Check: PASSED

- `web/src/app/cafe/page.tsx` — modified, contains `activeTab === "marketing"` ✓
- `web/src/lib/api.ts` — modified, exports `generatePromoGraphic`, `broadcastPromo`, `BroadcastResult` ✓
- Commit `25dbdcad` — exists in git log ✓
- `npm run build` — passed with no TypeScript errors ✓
