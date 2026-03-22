---
phase: 147-cameras-html-dashboard-rewrite
plan: 02
subsystem: ui
tags: [vanilla-js, css-grid, cameras, nvr, dashboard, drag-and-drop, zone-grouping]

# Dependency graph
requires:
  - phase: 147-01
    provides: cameras.html foundation with tile DOM, fetchCameras, fetchLayout, buildGrid, saveLayout
provides:
  - cameras.html: drag-to-rearrange + zone grouping + full layout persistence (grid mode + camera order)
affects:
  - 147-03 (WebRTC fullscreen builds on top of unchanged tile/fullscreen DOM)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "HTML5 drag-and-drop: dragstart/dragover(e.preventDefault)/dragleave/dragend/drop event chain"
    - "Zone grouping: ZONE_ORDER array drives rendering order; cameras.forEach groups into map"
    - "collapsedZones object persists collapse state across buildGrid calls (drag reorder preserves collapse)"
    - "saveLayout PUT includes camera_order: cameras.map(c => c.nvr_channel).filter(Boolean)"
    - "fetchLayout reorders cameras via orderMap index sort (O(n log n), handles missing channels)"

key-files:
  created: []
  modified:
    - crates/rc-sentry-ai/cameras.html

key-decisions:
  - "dragstart stores dragSrcChannel as int (parseInt); drop reads targetChannel as int for findIndex to match"
  - "collapsedZones persists across buildGrid: zone header click toggles directly on rendered tiles without rebuild"
  - "Zone header click listener attached after all tiles in zone rendered so closure captures zoneCameras array"
  - "fetchLayout reorders via sort+orderMap instead of ordered-push to handle cameras missing from saved order"

requirements-completed: [LYOT-02, LYOT-03, LYOT-05]

# Metrics
duration: 8min
completed: 2026-03-22
---

# Phase 147 Plan 02: Drag-to-Rearrange + Zone Grouping + Layout Persistence Summary

**HTML5 drag-and-drop camera reordering with auto-save on drop, zone-grouped collapsible headers (ENTRANCE/RECEPTION/PODS/OTHER), and full camera_order persistence via PUT /api/v1/cameras/layout**

## Performance

- **Duration:** ~8 min
- **Started:** 2026-03-22T14:00:00+05:30
- **Completed:** 2026-03-22T14:08:00+05:30
- **Tasks:** 1/1 complete
- **Files modified:** 1

## Accomplishments

- HTML5 drag-and-drop on all camera tiles: dragstart/dragover/dragleave/dragend/drop events
- Drop handler reorders `cameras` array (splice+insert) then calls buildGrid() + saveLayout() immediately
- No save button — auto-save on drop was user decision per CONTEXT.md
- Zone grouping: cameras grouped by zone field in fixed order (entrance, reception, pods, other)
- Zone headers: collapsible, show camera count "ENTRANCE (2)", triangle arrow rotates 90deg when collapsed
- collapsedZones object preserves collapse state across buildGrid calls (drag reorder doesn't reset collapse)
- Drag visual feedback: 60% opacity ghost (.cam.dragging), dashed #E10600 border on drop target (.cam.drag-over)
- fetchLayout now uses sort+orderMap pattern for camera_order reordering (matches plan spec exactly)
- saveLayout PUT body includes camera_order from cameras array (was already in plan 01 but made explicit)
- Zero innerHTML in file (all DOM via createElement per security requirement)
- cargo check -p rc-sentry-ai exits 0 (9 pre-existing warnings, 0 errors)

## Task Commits

1. **Task 1: Drag-to-rearrange + zone grouping + persistence** - `a026a190` (feat)

## Files Created/Modified

- `crates/rc-sentry-ai/cameras.html` - Added drag-and-drop, zone grouping, collapsible headers; 201 net lines added

## Decisions Made

- `dragSrcChannel` stored as `parseInt` so `findIndex` comparison works correctly (nvr_channel is integer in API)
- `collapsedZones[zone]` toggled in-place on click handler; no full rebuild needed for collapse toggle
- Zone header click listener attached after zone's tiles are appended so `tileElements` map has the entries
- `fetchLayout` reorder changed from ordered-push pattern (plan 01) to sort+orderMap (plan 02 spec); both work but sort handles channels missing from saved order more cleanly

## Deviations from Plan

None — plan executed exactly as written.

## Issues Encountered

None — cargo check passed on first attempt with 9 pre-existing warnings (same as plan 01, not introduced by this change).

## User Setup Required

None.

## Next Phase Readiness

- cameras.html now has complete layout features (modes + drag reorder + zone grouping + persistence)
- Plan 03 (WebRTC fullscreen) replaces the `openFullscreen()` stub; tile DOM and tileElements map are unchanged
- Zone collapse state is managed client-side (collapsedZones object); not persisted to server — zone_filter in PUT is null

---
*Phase: 147-cameras-html-dashboard-rewrite*
*Completed: 2026-03-22*
