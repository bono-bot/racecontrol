---
phase: 201-frontend-integration-type-sync
plan: "03"
subsystem: web-dashboard
tags: [frontend, billing, games, metrics, types, status-badge]
dependency_graph:
  requires:
    - 201-01 (BillingSessionStatus 10-variant type in @racingpoint/types)
  provides:
    - StatusBadge with all 10 BillingSessionStatus + 6 GameState variants
    - Metrics API client (getLaunchStats, getBillingAccuracy, getLaunchMatrix, getAlternatives)
    - /games/reliability launch matrix page
    - Web billing page correct button visibility for waiting_for_game
  affects:
    - web dashboard :3200 billing page
    - web dashboard :3200 billing history page
    - web dashboard :3200 games page
    - web dashboard :3200/games/reliability (new page)
tech_stack:
  added:
    - web/src/lib/api/metrics.ts (new typed API client module)
    - web/src/app/games/reliability/page.tsx (new Next.js page)
  patterns:
    - StatusBadge driven by complete color Record — no default-grey fallback for known statuses
    - Metrics API client co-located in web/src/lib/api/ subdirectory using same API_BASE pattern
    - Row color-coding via rowBgClass() helper (red/amber/green by success_rate thresholds)
key_files:
  created:
    - web/src/lib/api/metrics.ts
    - web/src/app/games/reliability/page.tsx
  modified:
    - web/src/lib/api.ts
    - web/src/components/StatusBadge.tsx
    - web/src/app/billing/page.tsx
    - web/src/app/billing/history/page.tsx
    - web/src/app/games/page.tsx
decisions:
  - StatusBadge uses a flat COLORS Record covering all known status strings (pod, game, billing) — single source of truth for badge colors, no per-page color maps
  - Metrics types declared locally in web/src/lib/api/metrics.ts instead of importing @racingpoint/types — avoids adding a build-time dependency on shared-types in the web package; kept in sync via parity script
  - billing/history: replaced 5-entry statusColors inline map with StatusBadge import — consistent rendering, immediate coverage of all 10 variants
  - isPaused in billing/page now covers paused_disconnect and paused_game_pause in addition to paused_manual — shows Resume button for all three paused states
metrics:
  duration_minutes: 10
  tasks_completed: 2
  tasks_total: 2
  files_created: 2
  files_modified: 5
  tests_passing: 0
  completed_date: "2026-03-26"
---

# Phase 201 Plan 03: Web Frontend Type Sync Summary

**One-liner:** Web billing page updated for all 10 billing states with correct button visibility, StatusBadge covers all variants with distinct colors, typed metrics API client created, and /games/reliability launch matrix page added.

## Tasks Completed

| Task | Description | Commit |
|------|-------------|--------|
| 1 | Update web types, StatusBadge, billing pages | `ca702e19` |
| 2 | Metrics API client, games page update, launch matrix page | `3913f93c` |

## What Was Built

### Task 1: Types, StatusBadge, billing pages

**web/src/lib/api.ts** — Updated two type definitions:
- `BillingSession.status`: expanded from 6 to 10 variants (`waiting_for_game`, `paused_disconnect`, `paused_game_pause`, `cancelled_no_playable` added)
- `GameState`: expanded from 5 to 6 variants (`"loading"` added)

**web/src/components/StatusBadge.tsx** — Complete rewrite with:
- `COLORS` Record covering all known statuses (pod, game, billing) — 22 entries
- `STATUS_LABELS` for display text overrides: `waiting_for_game` → "Loading...", `paused_disconnect` → "Disconnected", `paused_game_pause` → "Game Crashed", `cancelled_no_playable` → "Never Started"
- `PULSING` set for animated dot indicators on active/transitional states
- `dotColor()` helper for per-status indicator color

**web/src/app/billing/page.tsx** — Button logic updates:
- `isPaused` now checks `paused_disconnect` and `paused_game_pause` in addition to `paused_manual`
- `isWaitingForGame` flag hides End/Pause/Extend buttons and shows StatusBadge instead
- Added `StatusBadge status="waiting_for_game"` display when session is in loading state

**web/src/app/billing/history/page.tsx** — Status cell updates:
- Removed stale 5-entry `statusColors` inline map
- Imported and applied `StatusBadge` component in status table cell
- Removed unused `fetchApi` import

### Task 2: Metrics API client, games update, reliability page

**web/src/lib/api/metrics.ts (new)** — Typed API client:
- `getLaunchStats(params?)` → GET /api/v1/metrics/launch-stats (optional game + pod filters)
- `getBillingAccuracy()` → GET /api/v1/metrics/billing-accuracy
- `getAlternatives(params)` → GET /api/v1/games/alternatives
- `getLaunchMatrix(game)` → GET /api/v1/admin/launch-matrix?game={game}
- Local type declarations for `LaunchStatsResponse`, `BillingAccuracyResponse`, `AlternativeCombo`, `LaunchMatrixRow`, `FailureMode`

**web/src/app/games/page.tsx** — Game state column:
- Added "State" data row using `<StatusBadge status={gameInfo.game_state} />` in active game cards
- Added "Reliability Matrix" link button in page header
- Added `Link` import from `next/link`

**web/src/app/games/reliability/page.tsx (new)** — Launch matrix page:
- Game selector dropdown (6 supported games, default: assetto_corsa)
- Fetches data via `getLaunchMatrix(game)` on game change
- Table with columns: Pod, Total Launches, Success Rate, Avg Time, Top Failures, Status
- Row color-coding: `bg-red-900/20` (<70%), `bg-amber-900/20` (70-90%), `bg-green-900/20` (>90%)
- Success rate in matching color (red/amber/emerald)
- Flagged badge (red with pulse dot) when `row.flagged === true`
- Top failures: failure mode names with counts as comma-separated list
- Loading skeleton, error state, empty state ("No launch data available")
- Color legend showing thresholds

## Verification Results

| Check | Result |
|-------|--------|
| `web tsc --noEmit` | PASS (exit 0) |
| `grep "waiting_for_game" web/src/lib/api.ts` | PASS |
| `grep "paused_disconnect" web/src/lib/api.ts` | PASS |
| `grep "waiting_for_game" web/src/components/StatusBadge.tsx` | PASS |
| `grep "paused_game_pause" web/src/components/StatusBadge.tsx` | PASS |
| `grep "cancelled_no_playable" web/src/components/StatusBadge.tsx` | PASS |
| `grep "waiting_for_game" web/src/app/billing/page.tsx` | PASS |
| `file web/src/lib/api/metrics.ts exists` | PASS |
| `4 metrics functions in metrics.ts` | PASS (grep -c returns 4) |
| `grep "game_state" web/src/app/games/page.tsx` | PASS |
| `file web/src/app/games/reliability/page.tsx exists` | PASS |
| `grep "getLaunchMatrix" web/src/app/games/reliability/page.tsx` | PASS |
| `grep "flagged" web/src/app/games/reliability/page.tsx` | PASS |
| 4 new states across web/src/ | PASS (20 occurrences) |

## Deviations from Plan

None - plan executed exactly as written.

The games page uses a card grid layout (not a table), so "game_state column" was implemented as a data row within each card's game info section — semantically equivalent and consistent with the existing page design.

## Self-Check

### Created files exist:
- `web/src/lib/api/metrics.ts` — FOUND
- `web/src/app/games/reliability/page.tsx` — FOUND

### Commits exist:
- `ca702e19` — FOUND (Task 1)
- `3913f93c` — FOUND (Task 2)

## Self-Check: PASSED
