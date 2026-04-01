---
phase: 291-dashboard-api-wiring
plan: "01"
subsystem: web-frontend
tags: [dashboard, tsdb, metrics, recharts, api-client]
dependency_graph:
  requires: [286-tsdb-query-api]
  provides: [DASH-01]
  affects: [web/src/app/metrics, web/src/lib/api/tsdb]
tech_stack:
  added: []
  patterns: [fetchApi-reuse, recharts-AreaChart, useEffect-setInterval-30s]
key_files:
  created:
    - web/src/lib/api/tsdb.ts
    - web/src/app/metrics/page.tsx
  modified:
    - web/src/components/Sidebar.tsx
decisions:
  - "Reuse existing fetchApi from @/lib/api (auth + 30s timeout + 401 redirect included for free)"
  - "Promise.allSettled for metric queries — one failing metric does not block the entire dashboard"
  - "chartData stored in Map<string, TimePoint[]> — O(1) lookup per metric name"
  - "Added Metrics nav link adjacent to Fleet Health in Sidebar (BarChart2 icon, existing import)"
metrics:
  duration: "15 min"
  completed_date: "2026-04-01"
  tasks_completed: 1
  files_changed: 3
---

# Phase 291 Plan 01: Dashboard API Wiring Summary

**One-liner:** TSDB API client (tsdb.ts) + recharts sparkline dashboard at /metrics calling Phase 286 REST endpoints with exact Rust struct type mapping.

## What Was Built

### web/src/lib/api/tsdb.ts
TSDB metrics API client with three exported functions:
- `fetchMetricNames()` — GET /metrics/names, unwraps `resp.names`
- `fetchMetricSnapshot(pod?)` — GET /metrics/snapshot?pod=N, unwraps `resp.metrics`
- `fetchMetricQuery(metric, from, to, pod?, resolution?)` — GET /metrics/query with URLSearchParams

TypeScript interfaces match Rust structs in `crates/racecontrol/src/api/metrics_query.rs` exactly:
- `SnapshotEntry.name` (not `metric_name`)
- `SnapshotEntry.pod: number | null` (not `pod_id: string`)
- `SnapshotEntry.updated_at: number` unix epoch (not ISO string)
- `NamesResponse.names: string[]` wrapped object (not bare array)

### web/src/app/metrics/page.tsx (301 lines)
Dashboard page with:
- `"use client"` directive, wrapped in `DashboardLayout`
- State: names, snapshot, selectedPod, timeRange, chartData (Map), loading, error
- Time range picker: 1h / 6h / 24h / 7d / 30d button group
- Pod selector: derived from snapshot unique pod values + "All" option
- 30s auto-refresh via `setInterval(loadData, 30_000)` in useEffect
- Headline cards: one card per metric name showing latest snapshot value
- Sparkline charts: recharts `AreaChart` per metric, X-axis = formatted ts, Y-axis = value
- Tailwind dark theme: bg-zinc-900/bg-zinc-800 cards, Racing Red `#E10600` accent

### web/src/components/Sidebar.tsx
Added `{ href: "/metrics", label: "Metrics", Icon: BarChart2 }` after Fleet Health entry. `BarChart2` was already imported.

## Verification

All acceptance criteria passed:
- `grep -c "TODO" web/src/lib/api/tsdb.ts` → 0
- No `metric_name`, `pod_id`, or `recorded_at` as live field names (comments only)
- `fetchMetricNames`, `fetchMetricSnapshot`, `fetchMetricQuery` all present
- `/metrics/names`, `/metrics/snapshot`, `/metrics/query` all present
- `setInterval` present in page.tsx
- `30_000` (30s) interval present
- `/metrics` nav link in Sidebar.tsx
- page.tsx is 301 lines (>80 minimum)
- `npx tsc --noEmit` — zero errors

## Deviations from Plan

None — plan executed exactly as written.

## Known Stubs

None — all three functions call real endpoints. Dashboard renders real API data.

## Self-Check: PASSED
