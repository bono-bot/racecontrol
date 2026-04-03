---
phase: 320-kiosk-game-filtering
plan: 01
subsystem: ui
tags: [next.js, react, axum, sqlx, sqlite, kiosk, game-inventory, combo-validation]

# Dependency graph
requires:
  - phase: 319-reliability-dashboard
    provides: combo_validation_flags table and preset_validity data populated per-pod
  - phase: 318-launch-intelligence
    provides: pod_game_inventory table populated from GameInventoryUpdate WS messages

provides:
  - GET /api/v1/fleet/pod-inventory/{pod_id} — per-pod installed sim_types + preset validity
  - kiosk GamePickerPanel driven by server inventory (not stale WS state)
  - SetupWizard "Unavailable" badge on AC experiences with broken combos

affects:
  - 320-kiosk-game-filtering (Task 3 visual verification deferred)
  - Any future kiosk phase touching game selection or AC experience flow

# Tech tracking
tech-stack:
  added: []
  patterns:
    - public route pattern for kiosk-fetched endpoints (no JWT required)
    - debug_sim_type_to_snake helper for converting Rust Debug format to API snake_case
    - 30s polling useEffect with cleanup and cancelledFlag for non-blocking refresh

key-files:
  created: []
  modified:
    - crates/racecontrol/src/api/routes.rs
    - kiosk/src/lib/types.ts
    - kiosk/src/lib/api.ts
    - kiosk/src/app/staff/page.tsx
    - kiosk/src/components/SetupWizard.tsx

key-decisions:
  - "Route registered in public_routes (no JWT) — kiosk fetches without auth per standing rules"
  - "Unknown pod_id returns 200 empty (not 404) for backward compatibility with kiosk boot order"
  - "sim_type stored as Rust Debug format (AssettoCorsa); converted to snake_case at API boundary"
  - "Inventory fetch falls back to pod.installed_games from WS state if server not yet responded"
  - "presetValidity passed as prop (not from sessionStorage) — satisfies Next.js hydration rule"

patterns-established:
  - "Phase 320 pod inventory: podInventory state + 30s interval poll + cancelledFlag cleanup"
  - "Combo badge: isUnavailable = exp.ac_preset_id ? presetValidity[id] === 'invalid' : false"

requirements-completed: [INV-03, COMBO-05]

# Metrics
duration: 14min
completed: 2026-04-03
---

# Phase 320 Plan 01: Kiosk Game Filtering Summary

**Per-pod game filtering via GET /api/v1/fleet/pod-inventory/{pod_id} with Unavailable badge on AC experiences that have broken combo_validation_flags for the current pod**

## Performance

- **Duration:** ~14 min
- **Started:** 2026-04-03T14:38 IST
- **Completed:** 2026-04-03T14:53 IST
- **Tasks:** 2 of 3 complete (Task 3 deferred — visual verification)
- **Files modified:** 5

## Accomplishments

- New Axum handler `pod_inventory_handler` reads `pod_game_inventory` + `combo_validation_flags` and returns `{ installed_sim_types, preset_validity }` per pod, registered as a public route
- Kiosk `GamePickerPanel` now shows only games present in server inventory (with WS state fallback), refreshed every 30 seconds
- `SetupWizard` `select_experience` step shows `Unavailable` badge (Racing Red `#E10600`) on AC experiences whose `ac_preset_id` maps to an invalid combo on this pod; disabled experiences cannot be clicked

## Task Commits

1. **Task 1: Server endpoint (TDD)** - `be0e6716` (feat) — 4 unit tests (RED→GREEN), handler, route
2. **Task 2: Kiosk wiring** - `6e6a0ede` (feat) — types, api client, staff page, SetupWizard badge

## Files Created/Modified

- `crates/racecontrol/src/api/routes.rs` — `pod_inventory_handler`, `debug_sim_type_to_snake`, route in public_routes, `pod_inventory_tests` module (4 tests)
- `kiosk/src/lib/types.ts` — `PodInventoryResponse` interface
- `kiosk/src/lib/api.ts` — `api.podInventory(podId)` method
- `kiosk/src/app/staff/page.tsx` — `podInventory` state, 30s polling useEffect, GamePickerPanel wired, SetupWizard `presetValidity` prop
- `kiosk/src/components/SetupWizard.tsx` — `presetValidity` prop, Unavailable badge in `select_experience` step

## Decisions Made

- Route in `public_routes` (not `staff_routes`) — kiosk must fetch without JWT. Follows existing pattern for `presets` and `pod-availability` endpoints.
- `Unknown pod_id → 200 empty` rather than 404 — kiosk may fetch before pods register; a 404 would break the fallback flow.
- `sim_type` stored in DB as Rust Debug format `"AssettoCorsa"`. Conversion to snake_case `"assetto_corsa"` done in `debug_sim_type_to_snake` at the API boundary — keeps DB schema unchanged.
- Inventory fetched with `cancelled` flag + interval cleanup in useEffect to prevent stale state updates after pod deselect (SC-4 no-flicker requirement).

## Deviations from Plan

None — plan executed exactly as written.

## Issues Encountered

The kiosk initially failed to build due to a `splitOptions` reference left by a pre-existing agent working in the same worktree (the variable state was removed but JSX usage remained). This resolved itself — the file state at build time no longer contained the reference. No action required.

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

- Task 3 (visual verification) is deferred — requires deploy to server + kiosk rebuild + manual browser check at `http://192.168.31.23:3300/staff`
- Verification steps from plan: select a connected pod → game picker should show only installed games; AC experiences with broken combos should show red Unavailable badge
- Server endpoint directly testable: `curl http://192.168.31.23:8080/api/v1/fleet/pod-inventory/pod-1`

---
*Phase: 320-kiosk-game-filtering*
*Completed: 2026-04-03*
