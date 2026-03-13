---
phase: 03-websocket-resilience
plan: "03"
subsystem: kiosk-frontend
tags: [websocket, react, performance, ux, debounce, memoization]
dependency_graph:
  requires: []
  provides: [kiosk-disconnect-debounce, kiosk-pod-card-memo]
  affects: [kiosk/src/hooks/useKioskSocket.ts, kiosk/src/components/KioskPodCard.tsx]
tech_stack:
  added: []
  patterns: [useRef-debounce, React.memo-default-equality]
key_files:
  created: []
  modified:
    - kiosk/src/hooks/useKioskSocket.ts
    - kiosk/src/components/KioskPodCard.tsx
decisions:
  - disconnectTimerRef is useRef not useState -- timer state change must not trigger re-render
  - if (disconnectTimerRef.current === null) guard prevents stacking timers across repeated onclose calls during retry loop
  - React.memo uses default shallow equality -- Map copy in setPods preserves object identity for unchanged pod entries, so default equality is correct
  - Sub-components (TransmissionToggle, FfbToggle, BlankScreenButton) are NOT memoized -- they have local state that memo could interfere with
  - No custom comparator on React.memo -- simpler and correct given reference-stable unchanged pod objects
metrics:
  duration: "2 min"
  completed_date: "2026-03-13"
  tasks_completed: 2
  files_modified: 2
---

# Phase 3 Plan 03: Kiosk Disconnect Debounce + React.memo Summary

**One-liner:** 15s disconnect debounce via disconnectTimerRef prevents false "Disconnected" flashes during game launches; React.memo on KioskPodCard ensures only the changed pod card re-renders.

## Tasks Completed

| # | Task | Commit | Files |
|---|------|--------|-------|
| 1 | Add 15s disconnect debounce to useKioskSocket | db052a2 | kiosk/src/hooks/useKioskSocket.ts |
| 2 | Wrap KioskPodCard with React.memo | f3b278d | kiosk/src/components/KioskPodCard.tsx |

## What Was Built

### Task 1: 15s Disconnect Debounce (CONN-02)

`useKioskSocket.ts` now holds a `disconnectTimerRef` (`useRef`, not `useState`) that defers the `setConnected(false)` call by 15 seconds when the WebSocket closes. Key behaviors:

- **onclose:** Starts a 15s timer only if one isn't already running (`if (disconnectTimerRef.current === null)` guard prevents timer stacking during rapid retry cycles). The 3s reconnect retry fires immediately and is completely independent of the UI debounce.
- **onopen:** Cancels any pending timer. If a retry succeeds within 15s, the staff header stays green with zero flash.
- **useEffect cleanup:** Cancels the timer on unmount to prevent `setConnected` calls on an unmounted component.

Staff-visible result: CPU spikes from game launches that drop the WS for under 15s are invisible. After 15s of confirmed absence, the header correctly shows disconnected.

### Task 2: React.memo on KioskPodCard (PERF-04)

`KioskPodCard.tsx` is now `export const KioskPodCard = React.memo(function KioskPodCard(...))`. Default shallow equality is used because `setPods((prev) => { const next = new Map(prev); next.set(pod.id, pod); return next; })` in `useKioskSocket.ts` creates a new Map while preserving the original object references for all unchanged pod entries. React.memo's default comparator sees same-reference props and skips re-render for the 7 unchanged cards.

## Decisions Made

- `disconnectTimerRef` uses `useRef` not `useState` -- a timer that changes its own stored value must not trigger a re-render of the hook's consumers.
- Timer stacking guard (`=== null` check) is essential because `onclose` can fire multiple times across the 3s retry loop while the 15s window is active.
- No custom comparator on React.memo -- default is correct and simpler given the Map copy pattern in `setPods`.
- Sub-components (`TransmissionToggle`, `FfbToggle`, `BlankScreenButton`) are intentionally NOT memoized -- they hold local state (`mode`, `preset`, `blanked`) that React.memo could interfere with if props stay the same but internal state needs to change.

## Verification Results

All 7 plan verification checks passed:

- [x] `disconnectTimerRef` is `useRef` (not `useState`) -- no re-render on timer change
- [x] `onclose` does NOT call `setConnected(false)` immediately
- [x] `onopen` clears `disconnectTimerRef` if pending
- [x] `useEffect` cleanup clears timer
- [x] `KioskPodCard` export is `React.memo(function KioskPodCard(...))`
- [x] No custom comparator on `React.memo`
- [x] 3s reconnect retry interval is UNCHANGED
- [x] `npx next build` succeeds without errors (verified after each task)

## Deviations from Plan

None - plan executed exactly as written.

## Self-Check: PASSED

All files confirmed present. All commits confirmed in git log.
