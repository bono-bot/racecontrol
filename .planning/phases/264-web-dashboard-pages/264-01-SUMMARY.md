---
phase: 264-web-dashboard-pages
plan: 01
subsystem: web-dashboard
tags: [frontend, dashboard, pods, websocket, f1-timing-tower]
dependency_graph:
  requires: [Phase 263 SC-02 MetricCard, SC-03 PodCard, SC-01 StatusBadge, SC-06 CountdownTimer, SC-10 Skeleton/EmptyState]
  provides: [WD-01 Dashboard Home, WD-02 Pods Page]
  affects: [web/src/app/page.tsx, web/src/app/pods/page.tsx, web/src/lib/api.ts]
tech_stack:
  added: []
  patterns: [F1 timing tower vertical strip, detail drawer, KPI MetricCard row]
key_files:
  created: []
  modified:
    - web/src/app/page.tsx
    - web/src/app/pods/page.tsx
    - web/src/lib/api.ts
decisions:
  - "Revenue KPI uses dailyBillingReport() instead of billingTimers.size for accuracy"
  - "Pods page uses backdrop overlay + fixed drawer pattern for detail panel"
  - "Fleet health fetched on drawer open, not polling"
metrics:
  duration: "~15 min"
  completed: "2026-03-30"
---

# Phase 264 Plan 01: Dashboard Home and Pods Page Summary

Redesigned dashboard home with 4 MetricCard KPI tiles and F1 timing tower pod strip, plus pods page with timing tower, detail drawer, and live WS data.

## Changes Made

### Task 1: Dashboard Home (page.tsx)

Rewrote `web/src/app/page.tsx` to use Phase 263 components:

- **MetricCard KPI row** (grid 2-col / 4-col): Active Sessions (billingTimers.size), Pods Online (filtered count), Revenue Today (fetched from dailyBillingReport on mount), Queue (pendingAuthTokens.size)
- **TelemetryBar** preserved between KPI row and pod strip
- **F1 timing tower pod strip**: Replaced grid layout with `space-y-1` vertical PodCard list, sorted by pod.number ascending
- **EmptyState**: Uses SC-10 EmptyState with MonitorIcon for zero-pod state
- All existing useWebSocket destructuring preserved (connected, pods, latestTelemetry, recentLaps, billingTimers, pendingAuthTokens, sendCommand)

### Task 2: Pods Page (pods/page.tsx)

Rewrote `web/src/app/pods/page.tsx`:

- **Switched from api.listPods() to useWebSocket()** for real-time pod data
- **KPI sub-header pills**: Online (green), Racing (red), Offline (grey) count pills
- **F1 timing tower strip**: Clickable PodCard rows sorted by pod.number
- **Detail drawer** (fixed right, w-80, z-50): Opens on pod row click with backdrop overlay
  - Header: Pod number (mono) + name + close button
  - Status section: StatusBadge, uptime, WS connected, build_id (fetched from fleetHealth API)
  - Active session: driver name, tier, CountdownTimer (SC-06)
  - Pending token: driver name, PIN/QR display, Cancel Assignment button
  - Start Session button (disabled when offline) opens BillingStartModal
- **Loading state**: 8 Skeleton rows while WS initializes
- **EmptyState**: SC-10 EmptyState for zero-pod state

### API Changes (api.ts)

- Added `fleetHealth()` method: `GET /api/v1/fleet/health` returning `PodFleetStatus[]`
- Added `PodFleetStatus` interface: pod_number, ws_connected, http_reachable, version, build_id, uptime_secs, last_seen

## Verification Results

- TypeScript compilation: 0 errors (`npx tsc --noEmit`)
- Deprecated colours: 0 hits for FF4400 or rp-red-light in either file
- Both files exceed minimum line count (page.tsx: ~115 lines, pods/page.tsx: ~245 lines)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added fleetHealth API method**
- **Found during:** Task 2
- **Issue:** Plan references `api.fleetHealth()` but it did not exist in api.ts
- **Fix:** Added `fleetHealth()` method and `PodFleetStatus` interface to api.ts
- **Files modified:** web/src/lib/api.ts

**2. [Rule 2 - Missing functionality] EmptyState import path adjustment**
- **Found during:** Task 1
- **Issue:** Plan says import from `@/components/EmptyState` but EmptyState is exported from `@/components/Skeleton`
- **Fix:** Used correct import path `{ EmptyState } from "@/components/Skeleton"`
- **Files modified:** web/src/app/page.tsx, web/src/app/pods/page.tsx

**3. [Rule 2 - Missing functionality] Added backdrop overlay for drawer**
- **Found during:** Task 2
- **Issue:** Plan specifies drawer but no backdrop; clicking outside drawer should close it
- **Fix:** Added `bg-black/40` backdrop div behind drawer for dismissal
- **Files modified:** web/src/app/pods/page.tsx

## Known Stubs

None. All data flows are wired to live WS data or API calls.

## Self-Check: PENDING

Commits pending Bash permission approval. Files verified to exist and compile clean.
