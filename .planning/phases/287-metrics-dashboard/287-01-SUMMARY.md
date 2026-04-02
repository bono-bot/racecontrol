---
phase: 287-metrics-dashboard
plan: 01
subsystem: ui
tags: [next.js, recharts, swr, metrics, dashboard, sparkline]

requires:
  - phase: 286-metrics-query-api
    provides: "REST API contracts (stubs used until Phase 286 ships)"
provides:
  - "/metrics dashboard page with sparkline charts, pod selector, time range, auto-refresh"
  - "Stub API client matching Phase 286 contracts"
  - "Sidebar navigation link under Fleet section"
affects: [288-prometheus-export, 289-metric-alert-thresholds]

tech-stack:
  added: []
  patterns: ["useSWR with refreshInterval for auto-refresh", "stub API client with TODO markers for real API swap"]

key-files:
  created:
    - racingpoint-admin/src/lib/api/metrics.ts
    - racingpoint-admin/src/app/(dashboard)/metrics/page.tsx
  modified:
    - racingpoint-admin/src/components/AdminLayout.tsx

key-decisions:
  - "Deterministic sine-wave stubs (not Math.random) to prevent SWR revalidation flicker"
  - "Stub API client isolated in metrics.ts for clean Phase 286 swap"

patterns-established:
  - "Stub API pattern: TODO-marked functions returning deterministic fake data, matching future API contracts"

requirements-completed: [DASH-01, DASH-02, DASH-03, DASH-04, DASH-05]

duration: 8min
completed: 2026-04-01
---

# Phase 287 Plan 01: Metrics Dashboard Summary

**Next.js /metrics page with 7 sparkline charts (recharts), pod selector, time range picker (1h-30d), headline snapshot cards, and 30s SWR auto-refresh -- all using stub API client ready for Phase 286 swap**

## Performance

- **Duration:** ~8 min
- **Started:** 2026-04-01
- **Completed:** 2026-04-01
- **Tasks:** 2 (1 auto + 1 visual checkpoint)
- **Files modified:** 3

## Accomplishments

- Created stub API client with typed interfaces matching Phase 286 contracts (zero `any` usage)
- Built /metrics page with 7 sparkline AreaCharts, headline snapshot cards, pod selector (All + 1-8), and time range buttons
- All charts auto-refresh every 30s via SWR refreshInterval
- Added Metrics sidebar link under Fleet section in AdminLayout
- Visual verification approved by user

## Task Commits

Each task was committed atomically:

1. **Task 1: Stub API client + types + metrics page with all controls** - `723d002` (feat) -- in racingpoint-admin repo
2. **Task 2: Visual verification of metrics dashboard** - checkpoint approved, no code changes

## Files Created/Modified

- `racingpoint-admin/src/lib/api/metrics.ts` - Stub API client with fetchMetricsQuery, fetchMetricNames, fetchMetricsSnapshot, timeRangeToMs
- `racingpoint-admin/src/app/(dashboard)/metrics/page.tsx` - Metrics dashboard page (267 lines) with charts, controls, auto-refresh
- `racingpoint-admin/src/components/AdminLayout.tsx` - Added Metrics sidebar link

## Decisions Made

- Used deterministic sine-wave generation (not Math.random) for stub data to prevent flicker on SWR revalidation
- Isolated stub API in metrics.ts with clear TODO markers for Phase 286 replacement

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Known Stubs

| File | Description | Resolution |
|------|-------------|------------|
| `racingpoint-admin/src/lib/api/metrics.ts` | All 3 API functions return hardcoded/synthetic data | Phase 286 ships real API, then swap stub calls to rcFetch |

These stubs are intentional -- Phase 286 (Query API) is not yet built. The TODO markers in each function indicate the swap point.

## Next Phase Readiness

- Dashboard is fully functional with stub data
- When Phase 286 ships, replace stub functions in metrics.ts with real rcFetch calls (3 functions, clearly marked with TODO)
- No blockers for Phase 288 (Prometheus) or Phase 289 (Alerts)

## Self-Check: PASSED

- FOUND: commit 723d002 in racingpoint-admin
- FOUND: 287-01-SUMMARY.md

---
*Phase: 287-metrics-dashboard*
*Completed: 2026-04-01*
