---
phase: 264-web-dashboard-pages
plan: 02
subsystem: web-dashboard
tags: [sessions, billing, LiveDataTable, Toast, Skeleton, EmptyState]
dependency_graph:
  requires: [Phase 263 components]
  provides: [WD-03 Sessions page, WD-04 Billing page]
  affects: [web/src/app/sessions/page.tsx, web/src/app/billing/page.tsx]
tech_stack:
  added: []
  patterns: [LiveDataTable with ColumnDef<Session>, useToast context hook, skeleton init pattern]
key_files:
  created: []
  modified:
    - web/src/app/sessions/page.tsx
    - web/src/app/billing/page.tsx
decisions:
  - Toast uses context-based useToast() hook (not module-level toast.success) per existing Toast component API
  - Sessions page uses LiveDataTable's built-in loading/empty handling plus explicit EmptyState wrapper
  - Billing initialising state uses 800ms timeout to avoid skeleton flash on fast WS connections
metrics:
  duration: 3m
  completed: 2026-03-30
---

# Phase 264 Plan 02: Sessions + Billing Pages Summary

Redesigned Sessions and Billing pages using Phase 263 primitives (LiveDataTable, Toast, Skeleton, EmptyState, StatusBadge)

## One-liner

Sessions page rewritten with sortable LiveDataTable (7 typed columns, skeleton loading, EmptyState); Billing page upgraded with Toast feedback on all 5 actions, skeleton init, EmptyState, AlertTriangle icon, and active/paused stat pills.

## Tasks Completed

### Task 1: Sessions page -- LiveDataTable with sort and status badges

Sessions page was already rewritten with all required Phase 263 components from a prior session. Verified complete:
- LiveDataTable with typed `ColumnDef<Session>` columns (Pod, Type, Track, Sim, Car Class, Started, Status)
- StatusBadge rendering in Status column
- Skeleton loading via LiveDataTable `loading` prop
- EmptyState with ClipboardList icon when no sessions
- Toast success on first data load (one-time ref guard)
- Session count chip in header
- No deprecated colours

**Commit:** `caca5a54` (committed alongside Task 2)

### Task 2: Billing page -- skeleton loading + EmptyState + Toast feedback

Added Phase 263 component integration to existing billing page:
- **Toast feedback:** 5 toast calls added -- handleStart, handleEnd, handlePauseResume (dynamic message), handleExtend, handleCancelToken
- **Skeleton init:** `initialising` state with 800ms timeout; renders 4 skeleton cards in grid while WS initializes
- **EmptyState:** Replaced plain div with `<EmptyState icon={Server} headline="No pods connected" hint="..."/>`
- **AlertTriangle:** Replaced `&#9888;` HTML entity with Lucide `AlertTriangle` icon in billing warnings
- **Stat pills:** Added active sessions count and paused count pills in header using design tokens

All existing handler functions and WS state destructuring preserved exactly as-is.

**Commit:** `caca5a54`

## Commits

| Hash | Message |
|------|---------|
| `caca5a54` | feat(264-02): redesign Sessions + Billing pages with Phase 263 components |

## Verification Results

- `tsc --noEmit` -- both pages compile clean (zero errors)
- No deprecated colours (`FF4400`, `#ff4400`, `rp-red-light`) -- PASS
- LiveDataTable present in sessions/page.tsx -- PASS
- 6 toast references in billing/page.tsx (1 import + 5 action calls) -- PASS

## Deviations from Plan

None -- plan executed exactly as written. Sessions page was already complete from a prior implementation pass; billing page required all 5 planned changes.

## Known Stubs

None -- both pages are fully wired to live data sources (api.listSessions for sessions, useWebSocket for billing).
