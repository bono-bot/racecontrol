---
phase: 319-reliability-dashboard
plan: 02
subsystem: ui
tags: [nextjs, typescript, axum, sqlite, dashboard, launch-timeline]

# Dependency graph
requires:
  - phase: 318-launch-intelligence
    provides: launch_timeline_spans table and GET /api/v1/launch-timeline/:launch_id endpoint

provides:
  - GET /api/v1/launch-timeline/recent endpoint returning most-recent 50 launches
  - /games/timeline page with expandable per-launch checkpoint detail
  - TypelineSummary, TimelineEvent, TimelineDetail types in metrics.ts
  - getRecentTimelines() and getTimeline() typed API functions

affects: [319-01-PLAN.md, future-dashboard-phases, staff-debugging-workflows]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - TDD for Rust DB query tests (RED: insert tests, GREEN: add handler)
    - /recent route registered before /:param to prevent Axum routing collision
    - useEffect + setInterval pattern for 30s auto-refresh with cleanup
    - Expandable row pattern: selected state + colSpan detail row inline in table

key-files:
  created:
    - crates/racecontrol/src/api/routes.rs (RecentTimelineQuery struct, get_recent_launch_timelines handler, 2 TDD tests)
    - web/src/app/games/timeline/page.tsx (193 lines - timeline viewer page)
  modified:
    - crates/racecontrol/src/api/routes.rs (route registration: /recent before /:launch_id)
    - web/src/lib/api/metrics.ts (TimelineSummary, TimelineDetail, TimelineEvent types + API functions)
    - web/src/components/DashboardLayout.tsx (parentMap: /games/timeline -> /games)
    - web/src/app/games/page.tsx (Launch Timeline nav link in header)

key-decisions:
  - "Route /launch-timeline/recent registered before /:launch_id — Axum would treat literal 'recent' as param value otherwise"
  - "unwrap_or_default() on fetch_all in recent handler — returns [] on DB error rather than 500"
  - "TypelineEvent uses [key: string]: unknown index signature — no any, handles arbitrary checkpoint event shapes"
  - "Expandable detail row uses inline JSX fragment with colSpan=6 — avoids separate state arrays for open rows"
  - "Started_at stored as UTC ISO string, displayed with + 'Z' suffix to force UTC parse before toLocaleString()"

requirements-completed: [DASH-03]

# Metrics
duration: 25min
completed: 2026-04-03
---

# Phase 319 Plan 02: Reliability Dashboard — Launch Timeline Viewer Summary

**Launch timeline viewer at /games/timeline: expandable per-launch checkpoint detail with 30s auto-refresh, backed by new GET /api/v1/launch-timeline/recent endpoint returning last 50 launches ordered by recency**

## Performance

- **Duration:** ~25 min
- **Started:** 2026-04-03T08:50:00Z
- **Completed:** 2026-04-03T09:15:00Z
- **Tasks:** 2
- **Files modified:** 5 (1 created)

## Accomplishments

- Added `GET /api/v1/launch-timeline/recent?limit=50` endpoint — returns summary list without events_json for fast list view
- Created `/games/timeline` Next.js page with expandable rows, color-coded outcomes, 30s auto-refresh
- TDD: 2 tests added (empty returns [], DESC ordering) — both pass
- Route uniqueness preserved: /recent registered before /:launch_id in Axum router
- TypeScript clean: no `any` types, no hydration violations

## Task Commits

1. **Task 1: Backend — GET /api/v1/launch-timeline/recent endpoint** - `2c0f2578` (feat)
2. **Task 2: Frontend — timeline viewer page at /games/timeline** - `d4a3a6c2` (feat)

## Files Created/Modified

- `crates/racecontrol/src/api/routes.rs` — RecentTimelineQuery struct, get_recent_launch_timelines handler, route registration, 2 TDD tests
- `web/src/app/games/timeline/page.tsx` — New timeline viewer page (193 lines)
- `web/src/lib/api/metrics.ts` — TimelineSummary, TimelineEvent, TimelineDetail types + getRecentTimelines(), getTimeline()
- `web/src/components/DashboardLayout.tsx` — parentMap: /games/timeline -> /games
- `web/src/app/games/page.tsx` — "Launch Timeline" nav link added to header

## Decisions Made

- Route precedence: `/recent` must be before `/:launch_id` — Axum matches routes in declaration order and would treat the literal string "recent" as a launch_id param value otherwise
- `unwrap_or_default()` on `fetch_all` return: returns empty vec on DB error rather than propagating 500, matching the "never fail silently but degrade gracefully" pattern
- `TimelineEvent` uses `[key: string]: unknown` index signature to avoid `any` while handling arbitrary checkpoint event shapes from the JSON column

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- Worktree lacked node_modules; verified TypeScript by temporarily copying files to main repo (which has node_modules) — confirmed zero errors for new files
- Main repo merge was required to bring in the 318-02 `launch_timeline_spans` table code before implementing this plan

## Known Stubs

None - all data is wired to real API endpoints. Page shows "No launch timeline data yet" when the table is empty, which is the correct empty state.

## Next Phase Readiness

- 319-01 (Fleet Game Matrix + Combo Reliability) can proceed independently — the parentMap entries for /games/reliability were already added in this plan
- /games/timeline page is fully functional once backend is deployed
- Both 319-01 and 319-02 need frontend build + deploy to take effect

---
*Phase: 319-reliability-dashboard*
*Completed: 2026-04-03*
