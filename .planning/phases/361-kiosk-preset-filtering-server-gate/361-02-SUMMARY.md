---
phase: 361-kiosk-preset-filtering-server-gate
plan: 02
subsystem: ui
tags: [kiosk, inventory, setup-wizard, playwright, next.js, react]

requires:
  - phase: 361-01
    provides: GET /api/v1/pods/{id}/inventory endpoint with PresetValidity, available_cars, available_tracks

provides:
  - api.podInventoryFull() TypeScript method + PodInventory types
  - InventoryStatusBanner component (hard-block, retry, aria-describedby)
  - SetupWizard wired with inventory fetch, dropdown filtering, canLaunch gate
  - Pod ID parse fix: "pod_N" format stripped before parseInt (live-kiosk bug fix)
  - 3 Playwright E2E tests: A (inventory ok), B (422 invalid combo), C (error->retry->ok)

affects:
  - 361-03 (UI audit depends on these components)
  - Any future kiosk wizard work

tech-stack:
  added: []
  patterns:
    - "fetchInventory with isRetry flag to track retry spinner state"
    - "useEffect with visibility API for 30s auto-refresh pausing on tab hide"
    - "aria-describedby conditional on banner mount state"
    - "Playwright wildcard route mock: page.route('**/api/v1/pods/*/inventory', ...)"

key-files:
  created:
    - kiosk/src/components/InventoryStatusBanner.tsx
    - tests/e2e/playwright/kiosk/setup-wizard-inventory.spec.ts
  modified:
    - kiosk/src/lib/api.ts
    - kiosk/src/lib/types.ts
    - kiosk/src/components/SetupWizard.tsx

key-decisions:
  - "Pod ID from WS is 'pod_N' format (per rc-common normalize_pod_id). Strip prefix before parseInt — parseInt('pod_1', 10) = NaN was causing fetchInventory to always early-exit, showing the error banner on every real kiosk load."
  - "InventoryStatusBanner uses role=alert id=inventory-status-banner for Playwright targeting"
  - "canLaunch computed from: inventoryFetchState===ok AND presetValidity===valid (or no validity rule)"
  - "aria-describedby on Start button only when banner is mounted (not always)"

patterns-established:
  - "Pod ID normalization: always strip pod[_-]? prefix before parseInt in any kiosk component receiving podId from WS"

requirements-completed:
  - GLD-A-01
  - GLD-A-02

duration: 90min (across two session continuations)
completed: 2026-04-11
---

# Phase 361 Plan 02: Kiosk Inventory Wiring + Deploy Summary

**Kiosk staff wizard wired to server inventory endpoint: car/track dropdowns filtered per pod, error banner with retry, pod ID parse bug fixed (was blocking ALL inventory fetches on live kiosk), 3 Playwright tests pass, deployed to server .23 and cloud.**

## Performance

- **Duration:** ~90min (across two session continuations)
- **Started:** 2026-04-11 ~05:00 IST
- **Completed:** 2026-04-11 ~06:55 IST
- **Tasks:** 3/3 completed
- **Files modified:** 5

## Accomplishments

- Wired `SetupWizard.tsx` to fetch `GET /api/v1/pods/{id}/inventory` with staff JWT, filter car/track dropdowns to pod-installed content, gate Start Session on presetValidity
- Fixed live-kiosk bug: `parseInt("pod_1", 10)` returns NaN — fetchInventory always early-exited, showing error banner on EVERY real kiosk load. Fixed by stripping `pod[_-]?` prefix before parse
- `InventoryStatusBanner` renders with retry button and `aria-describedby` wiring on Start Session button
- 3 Playwright E2E tests covering inventory-ok, 422 invalid combo, and error->retry->success flows (3/3 pass, 31.6s)
- Deployed to server (.23:3300) BUILD_ID `0ncViMD8v0EJ4rBBxzjFo` and cloud (staff.racingpoint.cloud:3300) BUILD_ID `ouovunpOjraG8n88w5uXt`

## Task Commits

1. **Task 1: PodInventory types + api.podInventoryFull + InventoryStatusBanner** - `6467a315` (feat)
2. **Task 2: Wire SetupWizard, Playwright spec** - `3efc161e` (feat)
3. **Task 3: Fix pod_N parse + deploy** - `4ba17b01` (fix)

## Files Created/Modified

- `kiosk/src/lib/types.ts` — Added PodInventory, PresetValidity, InventoryFetchState types
- `kiosk/src/lib/api.ts` — Added podInventoryFull(podId: number) method
- `kiosk/src/components/InventoryStatusBanner.tsx` — New: hard-block banner, retry button, aria
- `kiosk/src/components/SetupWizard.tsx` — fetchInventory wiring, dropdown filtering, canLaunch gate, pod_N fix
- `tests/e2e/playwright/kiosk/setup-wizard-inventory.spec.ts` — New: 3 E2E inventory tests

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed pod_N format parse causing NaN in fetchInventory**
- **Found during:** Task 3 (Test C debugging)
- **Issue:** `parseInt(podId, 10)` where podId is `"pod_1"` from WebSocket returns NaN. The NaN guard fires immediately, setting inventoryFetchState to "error" before any HTTP request is made. This caused the error banner to appear on EVERY real kiosk load (not just tests).
- **Root cause path:** page.waitForResponse timeout (no HTTP request made after retry click) → only pre-HTTP early exit = NaN guard → podId comes from WS pod_list event → WS maps `p.id` → rc-common `normalize_pod_id()` always returns `"pod_N"` format → parseInt("pod_1") = NaN confirmed
- **Fix:** `const numericPart = podId.replace(/^pod[_-]?/i, ""); const podIdNum = parseInt(numericPart, 10);`
- **Files modified:** `kiosk/src/components/SetupWizard.tsx`
- **Commit:** `4ba17b01`

**Secondary impact:** This bug was also causing the InventoryStatusBanner to appear on the live production kiosk even when all pods were healthy — customers would see "Pod inventory unreachable" on every wizard open. Fix eliminates this spurious error state.

## Verification Results

- **Playwright:** 3/3 passed (31.6s) — `pw-test-all.txt` captured
  - Test A: inventory ok, dropdowns filtered, no aria-describedby
  - Test B: 422 CAR_NOT_AVAILABLE, wizard surfaces inline reason
  - Test C: error banner shown, retry re-enables with 200 OK
- **Server (.23:3300):** HTTP 200, BUILD_ID `0ncViMD8v0EJ4rBBxzjFo` in HTML, fix confirmed in static chunks (`replace(/^pod[_-]?/i,"")`)
- **Cloud (staff.racingpoint.cloud:3300):** HTTP 200, BUILD_ID `ouovunpOjraG8n88w5uXt` (Linux build), fix confirmed in SSR chunk `src_app_staff_page_tsx_ca854755._.js`

## Known Stubs

None — all data flows are wired to real endpoints.

## Self-Check: PASSED

- `6467a315` exists: CONFIRMED (git log)
- `3efc161e` exists: CONFIRMED (git log)
- `4ba17b01` exists: CONFIRMED (git log)
- `InventoryStatusBanner.tsx` exists: CONFIRMED
- `setup-wizard-inventory.spec.ts` exists: CONFIRMED
- Server kiosk BUILD_ID matches standalone build: CONFIRMED (`0ncViMD8v0EJ4rBBxzjFo`)
- Cloud kiosk has fix in chunks: CONFIRMED (`replace(/^pod[_-]?/i,"")` in SSR chunk)
