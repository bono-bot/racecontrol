---
phase: 158-marketing-content
plan: 01
subsystem: api
tags: [satori, resvg-wasm, png-generation, whatsapp, evolution-api, nextjs, rust, axum]

requires:
  - phase: 156-cafe-promos
    provides: "CafePromo struct and drivers table with phone column"
  - phase: 157-promotions-integration
    provides: "cafe promos evaluation, active promo patterns"

provides:
  - "POST /api/cafe/generate-graphic — Next.js route generating PNG from promo/menu/new_item templates"
  - "POST /api/v1/cafe/marketing/broadcast — Rust/Axum handler broadcasting WhatsApp to all driver phones"

affects: [158-marketing-content, plan-02]

tech-stack:
  added: [satori@0.10.14, "@resvg/resvg-wasm@2.6.2"]
  patterns:
    - "WASM init via fs.readFileSync to bypass Turbopack module resolution"
    - "LazyLock<Mutex<HashMap>> for in-memory per-driver broadcast cooldown"
    - "Evolution API broadcast follows same pattern as whatsapp_alerter.rs send_whatsapp"

key-files:
  created:
    - web/src/app/api/cafe/generate-graphic/route.tsx
    - crates/racecontrol/src/cafe_marketing.rs
  modified:
    - web/package.json
    - crates/racecontrol/src/lib.rs
    - crates/racecontrol/src/api/routes.rs

key-decisions:
  - "158-01: Use @resvg/resvg-wasm (not resvg-js) — WASM variant avoids native Node addon compilation issues on Windows"
  - "158-01: WASM init via fs.readFileSync(node_modules path) instead of dynamic import — Turbopack cannot resolve .wasm imports as ES modules"
  - "158-01: Rename route to .tsx — Next.js App Router API routes need .tsx extension to parse JSX syntax"
  - "158-01: Remove cooldown entry on Evolution API failure so retries can re-attempt failed drivers"
  - "158-01: Broadcast route placed under require_staff_jwt middleware layer (same as /cafe/promos)"

patterns-established:
  - "PNG generation pattern: satori JSX -> SVG string -> resvg-wasm -> PNG Buffer"
  - "Broadcast rate-limit pattern: LazyLock static map, remove on failure for retry eligibility"

requirements-completed: [MKT-01, MKT-02]

duration: 25min
completed: 2026-03-23
---

# Phase 158 Plan 01: Marketing Content Backend Summary

**satori/resvg-wasm PNG generation API (Next.js) + Evolution API WhatsApp broadcast with 24h per-driver cooldown (Rust/Axum)**

## Performance

- **Duration:** ~25 min
- **Started:** 2026-03-23T00:06:33 IST
- **Completed:** 2026-03-23T00:31:00 IST
- **Tasks:** 2
- **Files modified:** 5 (2 created, 3 modified)

## Accomplishments

- POST /api/cafe/generate-graphic returns a PNG binary for promo, daily_menu, or new_item templates using satori + resvg-wasm, brand colors #E10600/#1A1A1A, Montserrat font
- POST /api/v1/cafe/marketing/broadcast sends WhatsApp to all drivers with phones via Evolution API, with 24h per-driver cooldown and per-failure retry eligibility
- npm run build and cargo build --release both pass cleanly

## Task Commits

1. **Task 1: Next.js satori graphic generation API route** - `d720761d` (feat)
2. **Task 2: Rust broadcast endpoint in cafe_marketing.rs** - `5420fcbb` (feat)

## Files Created/Modified

- `web/src/app/api/cafe/generate-graphic/route.tsx` - Next.js POST handler: accepts template type, builds JSX via satori, converts SVG to PNG via resvg-wasm, returns binary PNG response
- `crates/racecontrol/src/cafe_marketing.rs` - Rust broadcast handler: queries drivers table, applies 24h cooldown map, sends via Evolution API, returns JSON count summary
- `web/package.json` - Added satori and @resvg/resvg-wasm dependencies
- `crates/racecontrol/src/lib.rs` - Registered pub mod cafe_marketing
- `crates/racecontrol/src/api/routes.rs` - Added use crate::cafe_marketing + POST /cafe/marketing/broadcast route under staff JWT layer

## Decisions Made

- Used @resvg/resvg-wasm over resvg-js — native addon has compilation issues on Windows
- WASM binary loaded via `fs.readFileSync` from node_modules — Turbopack cannot resolve `.wasm` files as ES module imports
- Route file uses `.tsx` extension — Next.js App Router requires `.tsx` for JSX syntax in API routes
- On Evolution API error (non-2xx or network failure), cooldown entry is removed so the driver can be retried in a subsequent broadcast call

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Renamed route.ts to route.tsx for JSX support**
- **Found during:** Task 1 (build verification)
- **Issue:** Next.js/Turbopack parse error — JSX syntax is not valid in `.ts` files, only `.tsx`
- **Fix:** Renamed `route.ts` → `route.tsx`
- **Files modified:** web/src/app/api/cafe/generate-graphic/route.tsx
- **Verification:** npm run build passed after rename
- **Committed in:** d720761d (Task 1 commit)

**2. [Rule 3 - Blocking] Fixed @resvg/resvg-wasm WASM init approach**
- **Found during:** Task 1 (build verification)
- **Issue:** Turbopack emits "Module not found: Can't resolve 'wbg'" when importing `.wasm` as ES module
- **Fix:** Changed WASM init to use `fs.readFileSync` on the node_modules path, bypassing Turbopack's module resolution
- **Files modified:** web/src/app/api/cafe/generate-graphic/route.tsx
- **Verification:** npm run build passed
- **Committed in:** d720761d (Task 1 commit)

**3. [Rule 3 - Blocking] Fixed satori type import and Buffer return type**
- **Found during:** Task 1 (TypeScript check)
- **Issue 1:** `satori.SatoriOptions` namespace reference invalid — satori exports `SatoriOptions` as named export not namespace; Issue 2:** `Uint8Array` not assignable to `BodyInit` for `new Response()`
- **Fix:** Imported `Font` type from satori; changed `svgToPng` return to `Buffer`, passed `.buffer as ArrayBuffer` to Response
- **Files modified:** web/src/app/api/cafe/generate-graphic/route.tsx
- **Verification:** TypeScript check passed in npm run build
- **Committed in:** d720761d (Task 1 commit)

---

**Total deviations:** 3 auto-fixed (all Rule 3 - Blocking during Task 1 build iteration)
**Impact on plan:** All fixes necessary for TypeScript correctness and Turbopack compatibility. No scope creep.

## Issues Encountered

None beyond the deviations documented above.

## User Setup Required

None — route uses existing Evolution API config from racecontrol.toml and INTERNAL_API_SECRET env var for item fetching (gracefully falls back to empty list if unset).

## Next Phase Readiness

- Both endpoints are curl-testable immediately
- Plan 02 can consume POST /api/cafe/generate-graphic for the admin marketing UI
- Plan 02 can wire the broadcast button to POST /api/v1/cafe/marketing/broadcast
- Font loading fetches from Google Fonts CDN at runtime — production should consider caching these ArrayBuffers or bundling them

---
*Phase: 158-marketing-content*
*Completed: 2026-03-23*
