---
phase: 319-reliability-dashboard
plan: 01
subsystem: ui
tags: [dashboard, reliability, game-matrix, combo-reliability, axum, nextjs, sqlite, tailwind]

requires:
  - phase: 298-config-management
    provides: combo_reliability table with success_rate, car, track, pod_id columns
  - phase: 318-launch-intelligence
    provides: pod_game_inventory table from agent game scanning

provides:
  - GET /api/v1/fleet/game-matrix — installed games per pod from pod_game_inventory
  - GET /api/v1/admin/combo-list — sortable combo reliability rows with flagged field
  - Three-section reliability dashboard page (Fleet Matrix + Combo Reliability + Launch Matrix)

affects: [320-reliability-dashboard-02, any phase reading combo_reliability or pod_game_inventory]

tech-stack:
  added: []
  patterns:
    - "Whitelist sort_by values before interpolating into SQL ORDER BY to prevent injection"
    - "game_matrix_handler in routes.rs (not metrics.rs) because it queries pod_game_inventory, not metrics tables"
    - "Never hold lock across await — Arc<SqlitePool> cloned per query"
    - "setInterval in useEffect with cleanup return () => clearInterval(id) for 30s polling"

key-files:
  created: []
  modified:
    - crates/racecontrol/src/api/metrics.rs — ComboListParams, ComboListRow, combo_list_handler, TDD tests
    - crates/racecontrol/src/api/routes.rs — game_matrix_handler, /fleet/game-matrix + /admin/combo-list routes, TDD tests
    - web/src/lib/api/metrics.ts — GameMatrixPodEntry, GameMatrixGame, GameMatrixResponse, ComboListRow, getGameMatrix(), getComboList()
    - web/src/app/games/reliability/page.tsx — rewritten with three sections (451 lines)
    - web/src/components/DashboardLayout.tsx — parentMap /games/reliability → /games

key-decisions:
  - "game_matrix_handler lives in routes.rs (not metrics.rs) because it queries pod_game_inventory, a fleet table not a metrics table"
  - "Pod ID display uses last 4 chars as label with TODO comment for future pod registry mapping"
  - "sort_by whitelist prevents SQL injection when interpolating column name into ORDER BY"
  - "Pre-existing /presets duplicate route is not introduced by this plan — verified via uniq -d check"

patterns-established:
  - "Sortable API endpoints: whitelist column names, interpolate into SQL, accept asc/desc via query param"
  - "Three-section reliability page pattern: each section has its own state + 30s polling interval"

requirements-completed: [DASH-01, DASH-02]

duration: 10min
completed: 2026-04-03
---

# Phase 319 Plan 01: Reliability Dashboard Summary

**Fleet game matrix (pod x game grid) and combo reliability table added to staff dashboard, with sortable backend endpoints backed by pod_game_inventory and combo_reliability tables**

## Performance

- **Duration:** ~10 min
- **Started:** 2026-04-03T08:45:07Z
- **Completed:** 2026-04-03T08:55:00Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments

- Backend: `combo_list_handler` queries `combo_reliability`, supports sort_by/order/game params, flags rows with success_rate < 0.70
- Backend: `game_matrix_handler` queries `pod_game_inventory`, returns `{ games: [{ game_id, display_name, sim_type, pods: { pod_id: { installed, launchable, scanned_at } } }] }`
- Both endpoints registered in staff-gated routes block (require staff JWT)
- Frontend: reliability page rewritten with 3 sections — Fleet Game Matrix + Combo Reliability + Launch Matrix (existing preserved)
- 4 TDD tests pass: test_combo_list_flagged, test_combo_list_empty_db, test_game_matrix_empty, test_game_matrix_installed_pods
- Both new sections auto-refresh every 30s via setInterval with cleanup

## Task Commits

1. **Task 1: Backend endpoints** - `574fbce7` (feat)
2. **Task 2: Frontend page** - `d0bd2ede` (feat)
3. **Parallel agent metrics.ts additions** - `97b01887` (chore — kept 319-02 timeline types)

## Files Created/Modified

- `crates/racecontrol/src/api/metrics.rs` — ComboListParams, ComboListRow structs + combo_list_handler + 2 TDD tests
- `crates/racecontrol/src/api/routes.rs` — game_matrix_handler + /fleet/game-matrix + /admin/combo-list routes + 2 TDD tests
- `web/src/lib/api/metrics.ts` — GameMatrixPodEntry/Game/Response, ComboListRow, getGameMatrix(), getComboList()
- `web/src/app/games/reliability/page.tsx` — 451-line rewrite with three sections, all rp-* Tailwind classes
- `web/src/components/DashboardLayout.tsx` — parentMap entry for /games/reliability

## Decisions Made

- `game_matrix_handler` placed in routes.rs (not metrics.rs) because it queries `pod_game_inventory` (a fleet/inventory table), not metrics/analytics tables — consistent with module responsibility
- Pod ID display uses last 4 chars as label with `// TODO: map to pod numbers when pod registry is available` — avoids UUID sprawl in the UI without hardcoding pod numbers
- `sort_by` column name is whitelisted (`success_rate` | `total_launches` | `avg_time_to_track_ms`) before interpolation into SQL — prevents SQL injection via ORDER BY column
- Pre-existing `/presets` duplicate route confirmed not introduced by this plan (uniq -d check)

## Deviations from Plan

None — plan executed exactly as written.

## Issues Encountered

- Cargo package name is `racecontrol-crate` not `racecontrol` — used correct name in test command
- Pre-existing TypeScript error in `pods/page.tsx` (unrelated BillingStartModalProps issue) — confirmed not introduced by this plan, no action taken
- Parallel 319-02 agent added timeline types to metrics.ts — preserved and committed

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

- Plan 319-02 (Timeline Viewer) can proceed — the reliability page foundation is in place
- Both endpoints are staff-JWT protected and available immediately after server rebuild
- Frontend sections will show empty states until agents report inventory and combo data

---
*Phase: 319-reliability-dashboard*
*Completed: 2026-04-03*
