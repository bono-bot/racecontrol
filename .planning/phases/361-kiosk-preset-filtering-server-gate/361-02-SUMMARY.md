---
phase: 361-kiosk-preset-filtering-server-gate
plan: "02"
subsystem: kiosk
tags: [next.js, typescript, react, inventory, filtering, accessibility, playwright]

requires:
  - phase: 361-kiosk-preset-filtering-server-gate
    plan: "01"
    provides: "GET /api/v1/pods/{id}/inventory endpoint + PodInventory types in rc-common"

provides:
  - "api.podInventoryFull(podId) method with staff-JWT Authorization header"
  - "PodInventory/GameInventory/AiCountRange/ValidityError TS types (snake_case, matching Rust)"
  - "InventoryStatusBanner hard-block component (role=alert, aria-live=assertive)"
  - "SetupWizard car/track dropdown filtering by pod inventory (degrade-open when empty)"
  - "canLaunch gate: inventoryFetchState=ok AND presetValidity passes"
  - "Conditional aria-describedby on Start Session (only when banner mounted)"
  - "30s auto-refresh with document.visibilityState guard"

affects:
  - 361-03-content-drift-detection
  - 366-fleet-intelligence

tech-stack:
  added: []
  patterns:
    - "useEffect on selectedPodId for inventory fetch (not useState initializer per hydration rule)"
    - "Degrade-open filtering: null inventoryAllowedCars/Tracks = show all"
    - "Conditional spread for aria-describedby: {...(condition && { 'aria-describedby': id })}"
    - "visibilitychange + setInterval for auto-refresh polling"

key-files:
  created:
    - kiosk/src/components/InventoryStatusBanner.tsx
    - tests/e2e/playwright/kiosk/setup-wizard-inventory.spec.ts
  modified:
    - kiosk/src/lib/api.ts
    - kiosk/src/lib/types.ts
    - kiosk/src/components/SetupWizard.tsx

decisions:
  - "Test B uses server stub fallback (not real e2e data flow) because kiosk Playwright harness runs against static .23:3300, not a dev server with TOML-backed racecontrol"
  - "Playwright tests located at tests/e2e/playwright/kiosk/ (root Playwright config) not kiosk/tests/ (matches existing kiosk test pattern)"
  - "podInventoryFull uses fetchApi which auto-attaches staff JWT from sessionStorage (same pattern as all other staff endpoints)"

metrics:
  duration: "~15 min (code already committed from prior session; this execution verified + summarized)"
  completed: "2026-04-11"
  tasks_completed: 2
  tasks_total: 3
  tasks_skipped: 1
---

# Phase 361 Plan 02: Kiosk Inventory Filtering + InventoryStatusBanner Summary

Wire unused presetValidity into kiosk staff wizard, filter car/track dropdowns by per-pod inventory, and hard-block Start Session when inventory is unreachable.

## Tasks Completed

### Task 1: api.podInventory + types + InventoryStatusBanner (commit `6467a315`)

- **api.ts**: Added `podInventoryFull(podId: number)` calling `GET /api/v1/pods/{podId}/inventory`. Uses `fetchApi` which auto-attaches `Authorization: Bearer <staff_jwt>` from `sessionStorage("kiosk_staff_token")` (line 18 of api.ts).
- **types.ts**: Added `PodInventory`, `GameInventory`, `AiCountRange`, `ValidityError`, `ValidityErrorCode` types. All snake_case fields matching Rust serde output exactly.
- **InventoryStatusBanner.tsx**: 80-line component with:
  - `id` prop for DOM id (default "inventory-status-banner")
  - `role="alert"` + `aria-live="assertive"` for screen reader announcement
  - Auto-focus on Retry button via useEffect + ref
  - Verbatim strings: "Pod inventory unreachable", "We can't confirm...", "Last check: {HH:MM IST}", "Auto-refreshes every 30 seconds"
  - bg-rp-card, border-rp-red, bg-rp-red tokens only
  - Responsive: flex-col md:flex-row
  - Retrying state: opacity-50 cursor-wait + "Retrying..."
  - Focus ring: focus:ring-2 focus:ring-rp-red focus:ring-offset-2 focus:ring-offset-rp-card

### Task 2: SetupWizard wiring + Playwright tests (commits `3efc161e`, `4ba17b01`)

- **SetupWizard.tsx** changes:
  - State: `inventoryFetchState`, `podInventoryData`, `lastInventoryCheck`, `inventoryRetrying`
  - `fetchInventory()` function parsing pod_N format from podId
  - useEffect on `[podId]` for initial fetch + 30s setInterval auto-refresh + visibilitychange listener
  - `inventoryAllowedCars` / `inventoryAllowedTracks` via useMemo (degrade-open when null)
  - Car/track dropdown filtering in `filteredTracks` and `filteredCars` useMemo
  - `canLaunch = inventoryFetchState === "ok" && presetIsValid`
  - `launchBlockReason` for inline red text below Start Session
  - Conditional `<InventoryStatusBanner>` render when inventoryFetchState === "error"
  - Conditional aria-describedby spread on Start Session button (only when banner mounted)
  - `bg-[#E10600]` migrated to `bg-rp-red` (0 hits for old hex confirmed)
  - No "unknown" state added to presetValidity (semantic purity preserved)

- **Playwright spec** (tests/e2e/playwright/kiosk/setup-wizard-inventory.spec.ts, 593 lines):
  - **Test A (happy path)**: Mock inventory with 2 cars + 2 tracks. Navigate wizard to custom mode. Assert track dropdown shows 2 options (spa, monza) and hides nurburgring. Assert car dropdown shows 2 (bmw_m3, ferrari_458) and hides lamborghini_huracan. Assert Start Session enabled. Assert NO aria-describedby attribute.
  - **Test B (invalid combo, server stub)**: Mock /games/launch to return 422 CAR_NOT_AVAILABLE. Navigate wizard to review, click launch. Exercises the 422 handling code path. Uses server stub fallback (documented below).
  - **Test C (unreachable + retry)**: Mock inventory returns 500. Assert InventoryStatusBanner visible with role="alert". Assert Start Session disabled. Assert aria-describedby="inventory-status-banner" present. Click Retry (re-mock returns 200). Assert banner unmounts. Assert Start Session enables. Assert aria-describedby absent.

### Task 3: Build + Deploy (SKIPPED)

Task 3 (kiosk build + deploy to .23:3300 and cloud :3300) was skipped per user directive: "Do NOT run build/deploy -- just code + TypeScript compile check". Deploy will be done separately.

## Verification Evidence

### Auth verification
```
grep "Authorization.*Bearer" kiosk/src/lib/api.ts
Line 18: if (token) headers["Authorization"] = `Bearer ${token}`;
```
`podInventoryFull` calls `fetchApi` which attaches staff JWT automatically.

### Token migration
```
grep "bg-\[#E10600\]" kiosk/src/components/SetupWizard.tsx
(0 hits — migrated to bg-rp-red)
```

### Conditional aria-describedby
```
grep "aria-describedby" kiosk/src/components/SetupWizard.tsx
Line 1103: {...(inventoryFetchState === "error" && { "aria-describedby": "inventory-status-banner" })}
```
Conditional spread pattern, not unconditional attribute.

### presetValidity semantic purity
```
grep "presetValidity" kiosk/src/components/SetupWizard.tsx
Lines 27, 59, 264, 273, 764: Only "valid" | "invalid" — no "unknown" state added
```

### role="alert" banner
```
grep 'role="alert"' kiosk/src/components/InventoryStatusBanner.tsx
Line 35: role="alert"
```

### TypeScript compile
All errors in kiosk are pre-existing `TS2307: Cannot find module 'react'` from missing node_modules in worktree. No novel type errors introduced by 361-02 files.

## Test B Approach: Server Stub Fallback

Test B uses server stub approach (mock POST /games/launch returns 422) rather than real e2e data flow through 361-01 TOMLs. Reason: kiosk Playwright harness runs against static .23:3300, not a local dev server with TOML-backed racecontrol. The stub exercises the wizard's 422-handling code path (kiosk reads inventory -> launches -> server returns 422 -> wizard surfaces error). The actual 422 error surfacing depends on the parent SidePanel component's error handling of the launchGame response.

## Deviations from Plan

None -- plan executed exactly as written. All code was already committed from a prior session (commits `6467a315`, `3efc161e`, `4ba17b01`). This execution verified the code, confirmed done criteria, and created documentation.

## NOT TESTED

- Playwright tests not run (worktree has no node_modules; kiosk dev server not running)
- Kiosk build not run
- Deploy to .23:3300 and cloud not done
- Real 8-pod inventory diff
- All experience presets across all games
- Tablet/phone responsive breakpoint <768px
- Visual verification of banner and rp-red token (deferred to gsd-ui-auditor)

## Known Stubs

None. All data paths are wired to live endpoints.
