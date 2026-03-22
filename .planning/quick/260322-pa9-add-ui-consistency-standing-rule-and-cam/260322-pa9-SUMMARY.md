---
phase: quick
plan: pa9
subsystem: cameras-ui, standing-rules
tags: [ui, keyboard-nav, standing-rules, cameras]
dependency_graph:
  requires: []
  provides: [UI-CONSISTENCY-RULE, CAM-FULLSCREEN-NAV, CAM-GRID-NAV]
  affects: [cameras.html, standing-rules.md, CLAUDE.md]
tech_stack:
  added: []
  patterns: [DOM creation without innerHTML, keyboard event delegation]
key_files:
  created: []
  modified:
    - C:/Users/bono/.claude/projects/C--Users-bono/memory/standing-rules.md
    - C:/Users/bono/racingpoint/racecontrol/CLAUDE.md
    - C:/Users/bono/racingpoint/racecontrol/crates/rc-sentry-ai/cameras.html
decisions:
  - Used textContent + Unicode escapes (\u2039, \u203A) for nav button labels — no innerHTML (XSS-safe)
  - navigateFullscreen filters only online/non-disconnected cameras to skip offline tiles
  - kbd-focus is purely visual/logical — no tabindex on tiles to avoid interfering with existing focus management
  - focusedTileIndex reset in buildGrid() prevents stale index after camera list refresh
metrics:
  duration: ~15min
  completed: 2026-03-22
  tasks_completed: 2
  files_modified: 3
---

# Quick Task pa9: Add UI Consistency Standing Rule and Camera Keyboard Navigation — Summary

**One-liner:** UI-must-reflect-config-truth rule added to standing-rules.md + CLAUDE.md; cameras.html gains fullscreen prev/next cycling (arrow keys + on-screen buttons) and grid tile keyboard focus navigation (arrow keys + Enter).

---

## Tasks Completed

| # | Task | Commit | Status |
|---|------|--------|--------|
| 1 | Add UI consistency standing rule to standing-rules.md and CLAUDE.md | `904e3155` | Done |
| 2 | Add fullscreen prev/next nav and grid keyboard navigation to cameras.html | `84637515` | Done |
| 3 | Human verify: fullscreen nav + grid keyboard nav | — | Awaiting human verify |

---

## What Was Built

### Task 1: UI Consistency Standing Rule

Added the following rule to both `standing-rules.md` (Code Quality > General) and `CLAUDE.md` (Code Quality > General, cascade-synced per Cascade Updates rule):

> **UI must reflect config truth** — no hardcoded camera lists, names, or layouts. All UI must read from API/config dynamically. If the backend config changes, the UI must update without code changes.
> _Why: v16.1 cameras dashboard was initially built with hardcoded 13-camera arrays. When cameras were added/removed from NVR config, the UI showed stale/phantom tiles. Dynamic fetch from /api/v1/cameras fixed it — this rule prevents regression._

### Task 2: cameras.html Navigation Features

**Fullscreen prev/next navigation:**
- CSS `.fs-nav` buttons with hover-reveal (opacity 0 → 1 on hover/focus)
- `.fs-nav.prev` (left: 12px) and `.fs-nav.next` (right: 12px) positioned center-vertically
- `navigateFullscreen(direction)` function: filters only online cameras, cycles modulo array length
- Unified `keydown` handler: ArrowLeft/ArrowRight call `navigateFullscreen` when fullscreen is visible
- On-screen buttons click → `navigateFullscreen(-1/1)` — textContent-only, no innerHTML

**Grid keyboard navigation:**
- CSS `.cam.kbd-focus` — red outline (#E10600) to highlight focused tile
- `focusedTileIndex` state variable (reset on `buildGrid()` to avoid stale index after refresh)
- `handleGridKeydown(e)`: ArrowRight/Left/Up/Down move focus, Enter opens fullscreen for focused tile
- `getGridColumns()`: returns 1/2/3/4 based on `window.innerWidth` breakpoints matching CSS media queries
- Called by unified `keydown` handler when fullscreen is NOT visible

---

## Deviations from Plan

None — plan executed exactly as written.

---

## Self-Check

- [x] standing-rules.md contains "UI must reflect config truth" (1 match)
- [x] CLAUDE.md contains "UI must reflect config truth" (1 match)
- [x] cameras.html contains navigateFullscreen (6 matches)
- [x] cameras.html contains handleGridKeydown (3 matches)
- [x] cameras.html contains fs-nav (8 matches)
- [x] Commits 904e3155 and 84637515 pushed to main

## Self-Check: PASSED
