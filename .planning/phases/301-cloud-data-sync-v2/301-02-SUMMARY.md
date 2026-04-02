---
phase: 301-cloud-data-sync-v2
plan: 02
subsystem: frontend
tags: [nextjs, sync, dashboard, typescript, admin-ui]

# Dependency graph
requires:
  - phase: 301-cloud-data-sync-v2
    plan: 01
    provides: conflict_count in sync_health endpoint
provides:
  - SyncStatusPanel component on admin settings page
  - syncHealth() API client method
  - SyncHealth + SyncTableState TypeScript interfaces
affects:
  - web/src/app/settings/page.tsx (adds SyncStatusPanel)
  - web/src/lib/api.ts (adds syncHealth method + types)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "SyncStatusPanel: inline component in page file, receives data as prop"
    - "formatStaleness(): seconds→human (Xs/Xm/Xh) shared helper in same file"
    - "Conflict count: text-rp-red when > 0, text-neutral-400 when 0"
    - "Status badge: emerald=healthy, amber=degraded, rp-red=critical, neutral=unknown"

key-files:
  created: []
  modified:
    - web/src/lib/api.ts
    - web/src/app/settings/page.tsx

key-decisions:
  - "SyncStatusPanel declared as top-level function before SettingsPage — avoids nesting component defs inside render"
  - "SyncTableState imported as named type in settings page — explicit over implicit"
  - "Loading state shows text placeholder in same card — consistent with existing backup/health loading pattern"

# Metrics
duration: 10min
completed: 2026-04-01
---

# Phase 301 Plan 02: Cloud Sync Status Panel Summary

**Admin settings page Cloud Sync Status panel showing sync_mode, relay availability, lag, and per-table last_synced_at/records/staleness/conflict_count from GET /api/v1/sync/health**

## Performance

- **Duration:** ~10 min
- **Started:** 2026-04-01T15:45:00Z
- **Completed:** 2026-04-01T15:49:31Z
- **Tasks:** 1
- **Files modified:** 2

## Accomplishments

- `SyncTableState` and `SyncHealth` TypeScript interfaces added to `web/src/lib/api.ts` (SYNC-06)
- `syncHealth()` API client method added — calls `GET /api/v1/sync/health` via `fetchApi`
- `SyncStatusPanel` component added to `web/src/app/settings/page.tsx`:
  - Status badge (green=healthy, amber=degraded, red=critical, gray=unknown)
  - Summary row: sync mode, relay available (green/red), overall lag
  - Per-table table: table name, last_synced_at, records synced, staleness (Xs/Xm/Xh), conflicts (red if > 0)
  - Loading placeholder when `syncHealth` is null
- `useEffect` in SettingsPage fetches `syncHealth` alongside existing health/backup calls
- TypeScript compiles with zero errors

## Task Commits

1. **Task 1: Add syncHealth API client method and SyncStatusPanel to settings page** - `c1976a92` (feat)

## Files Created/Modified

- `web/src/lib/api.ts` — SyncTableState interface, SyncHealth interface, syncHealth() method
- `web/src/app/settings/page.tsx` — SyncHealth import, syncHealth state, useEffect fetch, SyncStatusPanel component + render

## Decisions Made

- SyncStatusPanel declared as top-level function before the default export — avoids the React pattern warning about defining components inside render functions
- SyncTableState imported explicitly in settings page for type clarity on the `.map()` callback
- formatStaleness() helper function placed at module level for reuse

## Deviations from Plan

None - plan executed exactly as written.

## Known Stubs

None — SyncStatusPanel reads live data from `/api/v1/sync/health`. The endpoint was wired in Plan 301-01 (conflict_count exposed, sync_state array populated). Empty sync_state displays "No tables synced yet." — valid state before first sync cycle runs.

## Self-Check: PASSED

- [x] `web/src/lib/api.ts` — `grep "syncHealth"` shows method, `grep "SyncHealth"` shows interface
- [x] `web/src/app/settings/page.tsx` — `grep "SyncStatusPanel"`, `grep "syncHealth"`, `grep "conflict_count"` all return matches
- [x] `grep "sync/health" web/src/lib/api.ts` — shows endpoint path
- [x] `npx tsc --noEmit` — passes with no output (zero errors)
- [x] Commit `c1976a92` exists in `git log`
