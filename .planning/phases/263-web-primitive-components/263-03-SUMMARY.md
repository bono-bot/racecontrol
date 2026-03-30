---
phase: 263-web-primitive-components
plan: 03
subsystem: ui
tags: [react, tanstack-table, websocket, animation, leaderboard, data-table]

requires:
  - phase: 263-web-primitive-components
    plan: 01
    provides: Skeleton, SkeletonRow, EmptyState loading primitives

provides:
  - LiveDataTable generic TanStack Table wrapper with sticky header, sortable columns, row selection
  - LeaderboardTable F1-style leaderboard with WS reconnect, AnimatePresence row reordering, PB/SB highlights

affects: [264 dashboard pages, kiosk leaderboard display (separate implementation)]

tech-stack:
  added: ["@tanstack/react-table ^8.21.3 (web only)"]
  patterns: [useRef-ws-reconnect, isMountedRef-cleanup-guard, AnimatePresence-layout-reorder]

key-files:
  created:
    - web/src/components/LiveDataTable.tsx
    - web/src/components/LeaderboardTable.tsx
  modified:
    - web/package.json

key-decisions:
  - "TanStack Table installed in web/ only, NOT kiosk — kiosk uses AnimatePresence list, not sortable data grid"
  - "LeaderboardTable uses native WebSocket with useRef+isMountedRef cleanup pattern, not socket.io-client"
  - "WS reconnect delay hardcoded to 1000ms minimum to prevent reconnect storms"
  - "Session best (green) takes precedence over personal best (purple) for row accent styling"
  - "REST fallback fetch on mount provides immediate data before first WS push"
  - "React Compiler confirmed NOT enabled in web/next.config.ts (TanStack Table incompatibility)"

patterns-established:
  - "WS reconnect: useRef(WebSocket) + useRef(isMounted) + useRef(reconnectTimeout), cleanup clears timer then closes WS"
  - "AnimatePresence mode=popLayout with motion.tr layout prop for smooth row reordering"
  - "F1 rank colors: P1=rp-red, P2=neutral-200, P3=rp-yellow, rest=neutral-400"
  - "Lap time formatting: ms -> M:SS.mmm via formatLapTime() helper"

metrics:
  duration: ~15min
  completed: "2026-03-30"
  tasks: 2
  files: 3
---

# Phase 263 Plan 03: Data Display Components Summary

LiveDataTable (TanStack Table) and LeaderboardTable (WS + AnimatePresence) for web dashboard data display.

## What Was Built

### Task 1: LiveDataTable -- TanStack Table wrapper (web-only)

**LiveDataTable<T>** -- generic table component wrapping @tanstack/react-table v8:
- Sticky header (`sticky top-0 z-10`) stays visible during scroll
- Sortable columns with toggle: asc (triangle up) -> desc (triangle down) -> none (diamond)
- Row selection with `bg-rp-red/10` highlight, calls `onRowSelect(row.original)`
- Loading state renders 5 SkeletonRows
- Empty state renders EmptyState component
- Accepts generic `ColumnDef<T>[]` and `data: T[]` props
- Optional `getRowId` for stable row identity

@tanstack/react-table ^8.21.3 added to web/package.json ONLY. Kiosk does not have this dependency.

### Task 2: LeaderboardTable -- F1 style + WS reconnect + AnimatePresence

**LeaderboardTable** -- the highest-risk component in Phase 263:

**F1 Styling:**
- 5 columns: Rank (#), Driver, Best Lap, Gap, Laps
- P1 rank = rp-red, P2 = neutral-200, P3 = rp-yellow
- Personal best rows: `border-l-2 border-l-rp-purple bg-rp-purple/5`
- Session best rows: `border-l-2 border-l-rp-green bg-rp-green/5` (takes precedence)
- Lap times formatted as M:SS.mmm (e.g., 83456ms -> "1:23.456")
- Gap formatted as "+X.XXX" (P1 shows em dash)

**WS Reconnect (critical pattern):**
- `wsRef` holds WebSocket instance
- `isMountedRef` guards all state updates and reconnect attempts
- `reconnectTimeoutRef` holds pending setTimeout for reconnect
- `onclose` triggers reconnect after exactly 1000ms delay
- `onerror` calls `ws.close()` to trigger onclose -> reconnect
- Cleanup: sets `isMountedRef=false`, clears timeout, closes WS
- 3 total `ws.close()` calls: onopen guard (line 136), onerror (line 168), cleanup (line 183)

**AnimatePresence row reordering:**
- `<AnimatePresence mode="popLayout">` wraps tbody rows
- `<motion.tr layout>` enables smooth position transitions
- `initial={{ opacity: 0, x: -8 }}` / `exit={{ opacity: 0, x: 8 }}`

**REST fallback:**
- On mount, fetches from `/api/v1/leaderboards` for immediate data
- WS updates replace REST data as they arrive

## Verification Results

| Check | Result |
|-------|--------|
| TypeScript `tsc --noEmit` | 0 errors |
| @tanstack/react-table in web/package.json | Present (^8.21.3) |
| tanstack in kiosk/package.json | N/A (file does not exist) |
| reactCompiler in web/next.config.ts | Not present |
| ws.close() in LeaderboardTable | 3 hits (lines 136, 168, 183) |
| isMountedRef guards | 11 references throughout WS lifecycle |
| setTimeout(connect, 1000) | Present (line 164) |
| AnimatePresence in LeaderboardTable | Present with mode="popLayout" |
| motion.tr layout prop | Present (line 267) |
| sticky top-0 in LiveDataTable | Present (2 locations: loading + data) |

## Deviations from Plan

None -- plan executed exactly as written.

## Known Stubs

None -- both components are fully functional with real data flow (REST + WS).

## Self-Check: PENDING

Awaiting git commit to record hashes.
