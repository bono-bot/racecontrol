---
phase: 156-promotions-engine
plan: "02"
subsystem: web-frontend
tags: [cafe, promos, typescript, nextjs, crud-ui]
one_liner: "Promos tab on /cafe admin page with typed API functions and full CRUD UI for three promo types"
dependency_graph:
  requires:
    - 156-01  # cafe_promos DB table + Rust API endpoints
  provides:
    - "CafePromo TypeScript types (CafePromo, PromoType, ComboConfig, HappyHourConfig, GamingBundleConfig)"
    - "listCafePromos/createCafePromo/updateCafePromo/deleteCafePromo/toggleCafePromo API functions"
    - "/cafe page Promos tab with full CRUD and PromoPanel component"
  affects:
    - web/src/lib/api.ts
    - web/src/app/cafe/page.tsx
tech_stack:
  added: []
  patterns:
    - "Lazy tab loading — useEffect with activeTab guard, loads promos only when tab is active"
    - "PromoPanel outside CafePage — defined as separate named function after CafePage closing brace"
    - "Typed config union (ComboConfig | HappyHourConfig | GamingBundleConfig) — no any"
    - "useState initializers parse promo.config for pre-population on edit"
key_files:
  created: []
  modified:
    - web/src/lib/api.ts
    - web/src/app/cafe/page.tsx
decisions:
  - "Promos loaded lazily (only when Promos tab is active) rather than on page mount — avoids unnecessary API call before Promos is viewed"
  - "PromoPanel defined outside CafePage to keep CafePage focused — receives all needed props"
  - "start_time/end_time on CafePromo are top-level fields (not inside config) per Plan 01 schema — applied to all promo types, not just happy_hour in UI"
  - "deleteCafePromo uses fetchApi directly (not api.xxx) because api object doesn't expose standalone delete for promos"
metrics:
  duration_seconds: 574
  completed_date: "2026-03-22"
  tasks_completed: 3
  tasks_total: 3
  files_modified: 2
requirements:
  - PROMO-01
  - PROMO-02
  - PROMO-03
  - PROMO-04
---

# Phase 156 Plan 02: Promos Tab UI Summary

Promos tab on /cafe admin page with typed API functions and full CRUD UI for three promo types. Gives Uday a way to create and manage combo deals, happy hour discounts, and gaming+cafe bundles from the same admin page where he manages items and inventory.

## Tasks Completed

| # | Task | Commit | Key Files |
|---|------|--------|-----------|
| 1 | TypeScript types and API call functions in api.ts | 1826731d | web/src/lib/api.ts |
| 2 | Promos tab UI in /cafe/page.tsx | e8d3f04a | web/src/app/cafe/page.tsx |
| 3 | Visual verify Promos tab (checkpoint:human-verify) | — | auto-approved |

## What Was Built

### Task 1: TypeScript Types (api.ts)

Added after the existing LowStockItem type:

- `PromoType` union: `"combo" | "happy_hour" | "gaming_bundle"`
- `ComboConfig`, `HappyHourConfig`, `GamingBundleConfig` — explicit typed config interfaces, no `any`
- `PromoConfig` union type
- `CafePromo` interface matching Plan 01 DB schema
- `CreateCafePromoRequest` / `UpdateCafePromoRequest` interfaces
- Five standalone API functions: `listCafePromos`, `createCafePromo`, `updateCafePromo`, `deleteCafePromo`, `toggleCafePromo`

### Task 2: Promos Tab UI (page.tsx)

- Extended `ActiveTab` type to `"items" | "inventory" | "promos"`
- Added six promo state variables inside `CafePage`
- Added lazy-loading `useEffect` — only fires when `activeTab === "promos"`
- Added "Promos" tab button after Inventory in tab bar
- Added promos tab content block:
  - List table: name, type, time window, stacking group, status badge, activate/edit/delete actions
  - Empty state message
  - Loading state
- Added `PromoPanel` component outside `CafePage`:
  - Fixed right slide-in panel (480px wide, full-height)
  - Name input, type selector (disabled on edit), stacking group input
  - Conditional fields per type:
    - **combo**: checkbox item list with per-item qty inputs + bundle price
    - **happy_hour**: discount mode radio (percent/flat), applies_to selector, target multi-select, start/end time inputs labeled IST
    - **gaming_bundle**: session duration input, item checkboxes, bundle price
  - Pre-populated from `JSON.parse(promo.config)` when editing
  - Save/Cancel buttons; Save disabled while saving

## Verification

- `npx tsc --noEmit` — zero errors after both tasks
- `cargo build --release --bin racecontrol` — passes (1 pre-existing unused import warning, not new)
- No `any` in added TypeScript code (grep confirmed)
- Task 3 checkpoint (human-verify) — auto-approved per execution instructions

## Deviations from Plan

None — plan executed exactly as written.

## Self-Check

- [x] `web/src/lib/api.ts` modified with promo types
- [x] `web/src/app/cafe/page.tsx` modified with Promos tab
- [x] Commit 1826731d exists (api.ts types)
- [x] Commit e8d3f04a exists (page.tsx UI)
- [x] TypeScript passes with zero new errors
- [x] Cargo build passes

## Self-Check: PASSED
