---
phase: 367-staff-tools
plan: 01
subsystem: api, ui
tags: [rust, axum, sqlite, sqlx, nextjs, recharts, billing, telemetry, suspect-sessions]

# Dependency graph
requires:
  - phase: 363-data-recording-verification
    provides: "billing_sessions.suspect column, suspect_reasons, telemetry_coverage_pct, lap_count_actual, lap_count_expected, lap_count_flag"
  - phase: 364-session-quality-monitor
    provides: "3-sigma outlier detection and lap consistency data"
provides:
  - "GET /api/v1/admin/suspect-sessions — paginated list of flagged sessions (manager+ role)"
  - "GET /api/v1/admin/sessions/{id}/telemetry-heatmap — per-lap sample count array (manager+ role)"
  - "Admin portal page /sessions/suspect — list + recharts BarChart heatmap drill-down"
affects: [367-admin-suspect-laps, admin-dashboard-rebuild, cloud-parity]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Suspect session query: LEFT JOIN drivers + date filter via DATE(ended_at) with empty-string passthrough"
    - "Telemetry heatmap: per-lap COUNT(*) from telemetry_samples using telemetry_db pool (Phase 251 pattern)"
    - "Admin heatmap: recharts BarChart + Cell per lap with lapCellColor threshold (0/50/150 samples)"

key-files:
  created:
    - racingpoint-admin/src/app/(dashboard)/sessions/suspect/page.tsx
  modified:
    - crates/racecontrol/src/api/routes.rs

key-decisions:
  - "Routes placed in manager-role sub-router (RBAC SEC-04) — financial/audit data requires manager+"
  - "Telemetry heatmap uses telemetry_db pool with fallback to main db (Phase 251 pattern)"
  - "Color thresholds: 0=grey, <50=red, <50-150=amber, >=150=green — matches Phase 363 coverage definitions"
  - "Tooltip formatter typed as number | undefined to satisfy recharts strict TypeScript"

patterns-established:
  - "Suspect data query: bind empty string twice for optional date filter — avoids dynamic SQL"
  - "Per-lap heatmap: loop over laps, COUNT(*) per lap from telemetry pool — O(N) queries, N=lap count"

requirements-completed: [GLD-G-01]

# Metrics
duration: 25min
completed: 2026-04-11
---

# Phase 367 Plan 01: Suspect Sessions List + Telemetry Heatmap Summary

**Manager-role API routes + admin page for suspect billing session review with per-lap recharts heatmap drill-down (GLD-G-01)**

## Performance

- **Duration:** 25 min
- **Started:** 2026-04-11T08:30:00Z
- **Completed:** 2026-04-11T09:00:00Z
- **Tasks:** 3
- **Files modified:** 2 (routes.rs, new suspect/page.tsx)

## Accomplishments
- Added `GET /admin/suspect-sessions` (paginated, date-filtered, manager+ RBAC)
- Added `GET /admin/sessions/{id}/telemetry-heatmap` (per-lap sample counts, uses Phase 251 telemetry pool)
- Created admin portal page at `/sessions/suspect` with expandable BarChart heatmap per session

## Task Commits

1. **Tasks 01+02: Rust backend routes + handlers** - `8c2e7047` (feat, racecontrol repo — parallel agent)
2. **Task 03: Admin portal page** - `c87f630` (feat, racingpoint-admin repo)

## Files Created/Modified
- `crates/racecontrol/src/api/routes.rs` — Two new routes in manager sub-router + handler functions `list_suspect_sessions_handler` + `session_telemetry_heatmap_handler`
- `racingpoint-admin/src/app/(dashboard)/sessions/suspect/page.tsx` — New page: suspect session list with click-to-expand recharts heatmap (176 lines)

## Decisions Made
- Routes placed in manager-role sub-router (not staff) — suspect data is audit/financial context requiring manager+
- Telemetry heatmap queries use `telemetry_db.as_ref().unwrap_or(&state.db)` following Phase 251 pattern for separate pool
- TypeScript Tooltip formatter typed `number | undefined` — recharts strict type requires this for safety

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed TypeScript Tooltip formatter type error**
- **Found during:** Task 03 (TypeScript compilation check)
- **Issue:** `formatter={(v: number) => ...}` rejected by recharts types — `value` param is `number | undefined`
- **Fix:** Changed to `(v: number | undefined) => [`${v ?? 0} samples`, 'Coverage']`
- **Files modified:** `racingpoint-admin/src/app/(dashboard)/sessions/suspect/page.tsx`
- **Verification:** `npx tsc --noEmit` returns 0 errors
- **Committed in:** `c87f630`

---

**Total deviations:** 1 auto-fixed (Rule 1 — TypeScript type bug)
**Impact on plan:** Minor fix, no scope change. Page compiles cleanly.

## Issues Encountered
- Parallel agent had already committed routes.rs changes in `8c2e7047` before this agent could commit them. Duplicate handler definitions were written by this agent and cleaned up by another agent in `b36e8a7b`. Net result: routes.rs is correct with no duplicates.
- Admin page was tracked in racecontrol repo git history (`8c2e7047`) but the file was never written to the racingpoint-admin repo on disk. This agent created the file in the correct location and committed it to the racingpoint-admin repo.

## Known Stubs
None — page fetches live data from `/api/rc/admin/suspect-sessions`. Empty state shows informative message when no suspect sessions exist (Phase 363 prerequisite noted in UI).

## Next Phase Readiness
- `/sessions/suspect` admin page is deployable once Phase 363 is deployed to server .23
- Phase 363 migration must run first (billing_sessions.suspect column prerequisite)
- Admin dashboard (:3201) rebuild required on both server .23 and cloud (Bono VPS) per deploy: section

---
*Phase: 367-staff-tools*
*Completed: 2026-04-11*
