---
phase: 148-web-dashboard-page
plan: 01
subsystem: web-dashboard
tags: [react, nextjs, webrtc, cameras, dashboard]
dependency_graph:
  requires:
    - rc-sentry-ai camera API (GET /api/v1/cameras, GET/PUT /api/v1/cameras/layout)
    - go2rtc WebSocket signaling (ws://192.168.31.27:1984/api/ws)
    - rc-sentry-ai snapshot proxy (GET /api/v1/cameras/nvr/:channel/snapshot)
  provides:
    - cameras page at /cameras on :3200 with full feature parity to cameras.html
  affects:
    - web/src/app/cameras/page.tsx (complete rewrite)
tech_stack:
  added: []
  patterns:
    - useRef for WebRTC singleton management (avoids stale closure in cleanup)
    - camerasRef shadow for interval callbacks (avoids stale closure on cameras state)
    - native HTML5 DnD (no external library per user decision)
    - connectWebRTC helper extracted as pure function (not a hook) for pre-warm reuse
key_files:
  created: []
  modified:
    - web/src/app/cameras/page.tsx
decisions:
  - "148-01: Native HTML5 DnD used (no @dnd-kit) per plan instruction"
  - "148-01: camerasRef kept in sync with cameras state so polling interval callbacks see current order"
  - "148-01: resetControlsTimer uses plain setTimeout (not Promise wrapper) — previous draft had incorrect async pattern"
  - "148-01: catch blocks use bare catch{} not catch(_){} to avoid ESLint unused-vars warning"
  - "148-01: Pre-warm connection promoted to fullscreen if channel matches, including already-arrived tracks via getReceivers()"
  - "148-01: -m-6 negative margin applied to outer wrapper to cancel DashboardLayout p-6 for edge-to-edge grid"
requirements-completed: [DPLY-02, DPLY-03]
metrics:
  duration: "~45 minutes"
  completed_date: "2026-03-22T13:55:00+05:30"
  tasks_completed: 2
  tasks_total: 2
  files_modified: 1
---

# Phase 148 Plan 01: Camera Dashboard Web Page Summary

Complete rewrite of `web/src/app/cameras/page.tsx` as a professional React/Next.js camera dashboard with full feature parity to `cameras.html` from Phase 147.

## What Was Built

A single `"use client"` React component (849 lines) that wraps inside `DashboardLayout` and delivers all 12 features from the reference HTML implementation:

1. **Camera grid + snapshot thumbnails** — fetches from `/api/v1/cameras`, polls snapshots at configurable rate via preload Image pattern, updates `imgRefs` in-place without re-render
2. **Layout mode buttons** — 1/4/9/16 toolbar buttons switching `grid-cols-{N}` with 300ms CSS transition, active button highlighted in rp-red
3. **Zone grouping with collapsible headers** — cameras grouped by zone field in ZONE_ORDER, headers show arrow+count, click toggles `collapsedZones` state, tiles hidden via `display:none`
4. **Native HTML5 drag-to-rearrange** — `draggable="true"`, `onDragStart/Over/Leave/End/Drop`, splice-and-insert reorder in cameras array, auto-saves to server
5. **Layout persistence** — PUT `/api/v1/cameras/layout` with `{grid_mode, camera_order, zone_filter:null}` on mode change and drag-drop; shared camera-layout.json with cameras.html
6. **WebRTC fullscreen overlay** — fixed inset-0 z-50 overlay with fade-in animation, video element with autoPlay/playsInline/muted, snapshot poster as fallback
7. **Singleton WebRTC management** — `pcRef`/`wsRef` refs, `teardownRtc()` called before new connection, `beforeunload` and `visibilitychange` cleanup
8. **Pre-warm on hover (500ms)** — `handleTileMouseEnter` sets 500ms timer, creates pre-warm connection stored in `preWarmPcRef`/`preWarmWsRef`, green pulsing outline via CSS animation, promoted to active on fullscreen open
9. **Snapshot fallback on WebRTC failure** — video poster set to snapshot URL, "Live unavailable — showing snapshot" badge shown for 5s on `failed`/`disconnected`/`closed` state
10. **Fullscreen controls auto-hide** — controls fade out after 3s, reset on `mousemove` over overlay
11. **Refresh rate selector** — `<select>` with 1fps/0.5fps/0.2fps options, restarts polling interval on change
12. **Status indicators** — green/yellow/red dots per status, 40% opacity + OFFLINE text overlay for offline cameras

## Commits

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Complete cameras/page.tsx rewrite | a74c35c8 | web/src/app/cameras/page.tsx |
| 2 | Verify camera dashboard in browser | checkpoint:human-verify | — approved by user |

## Verification

- TypeScript: `npx tsc --noEmit` — no errors
- ESLint: `npx eslint src/app/cameras/page.tsx` — no errors, no warnings (after fixing catch blocks and removing Promise wrapper)
- Zero `any` types in file
- `"use client"` at line 1
- `DashboardLayout` imported and used as wrapper
- `SENTRY_BASE = "http://192.168.31.27:8096"` present
- `GO2RTC_WS = "ws://192.168.31.27:1984/api/ws"` present
- 849 lines (exceeds 400-line minimum)
- Browser verification: PASSED — user confirmed all 12 features working at http://192.168.31.23:3200/cameras, feature parity with cameras.html at :8096/cameras/live confirmed

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Removed erroneous Promise wrapper in resetControlsTimer**
- **Found during:** Task 1 implementation
- **Issue:** Initial draft accidentally wrapped setTimeout in `new Promise<void>` which created a stale ref assignment
- **Fix:** Replaced with direct `setTimeout(() => setControlsVisible(false), 3000)` call
- **Files modified:** web/src/app/cameras/page.tsx
- **Commit:** a74c35c8

**2. [Rule 2 - Quality] Changed `catch (_)` to bare `catch` blocks**
- **Found during:** ESLint run after Task 1
- **Issue:** `catch (_)` triggered `@typescript-eslint/no-unused-vars` warnings for the `_` identifier
- **Fix:** Used bare `catch {}` syntax (TypeScript 4.0+ feature, valid in Next.js)
- **Files modified:** web/src/app/cameras/page.tsx
- **Commit:** a74c35c8

## Status

COMPLETE — both tasks done. User approved checkpoint:human-verify confirming full feature parity at :3200/cameras.

## Next Phase Readiness

- DPLY-02 and DPLY-03 requirements fulfilled: standalone camera dashboard at /cameras on :3200 with identical feature set to cameras.html
- v16.1 Camera Dashboard Pro milestone complete
- All 9 success criteria from the plan met and user-verified

## Self-Check: PASSED

- [x] web/src/app/cameras/page.tsx — 849 lines, exists
- [x] Commit a74c35c8 — exists in git log
- [x] TypeScript: no errors
- [x] ESLint: no errors
- [x] Browser verification: PASSED (user approved checkpoint)
