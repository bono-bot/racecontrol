---
phase: 263-web-primitive-components
plan: 02
subsystem: web-components
tags: [podcard, countdown-timer, pinpad, appshell, sidebar, f1-timing]
dependency_graph:
  requires: [263-01]
  provides: [PodCard-F1-row, CountdownTimer-SVG-ring, PinPad, AppShell, Sidebar-Lucide]
  affects: [264-web-dashboard-pages, 265-kiosk-pages]
tech_stack:
  added: [SVG-radial-ring]
  patterns: [F1-timing-tower-row, composite-component, provider-wrapper]
key_files:
  created:
    - web/src/components/PinPad.tsx
    - web/src/components/AppShell.tsx
  modified:
    - web/src/components/PodCard.tsx
    - web/src/components/CountdownTimer.tsx
    - web/src/components/Sidebar.tsx
    - web/src/components/DashboardLayout.tsx
decisions:
  - PodCard uses w-1 left bar (not w-1.5 or border-l-4) for slim F1 timing tower aesthetic
  - CountdownTimer SVG uses viewBox 0 0 100 100 with r=40 giving circumference 251.2
  - PinPad auto-resets internal state 100ms after onComplete fires to prevent double-submit
  - Sidebar fleet health polling at 10s, server health at 15s (staggered to reduce API load)
  - PodFleetStatus type defined locally in Sidebar (not imported from shared types package)
metrics:
  duration: 4min
  completed: "2026-03-30T10:44:00Z"
  tasks_completed: 2
  tasks_total: 2
  files_changed: 6
---

# Phase 263 Plan 02: Composite Components Summary

F1 timing tower PodCard with left-edge status bar and SVG radial countdown ring, plus PinPad extraction and AppShell/Sidebar motorsport chrome.

## Task Results

### Task 1: PodCard F1 timing row + CountdownTimer radial SVG + PinPad
**Commit:** `b6492574`

**PodCard.tsx** -- Redesigned from card-based layout to F1 timing tower horizontal row:
- `flex items-stretch rounded-lg border overflow-hidden` with `w-1 flex-shrink-0` left status bar
- Status-to-color mapping: green (idle/connected), red (in_session/active), red-500 (error/disconnected), yellow (pending/stopping), grey (offline/completed), blue-400 (launching/loading/maintenance)
- Pod number in bold mono, inline StatusBadge, driver name, sim label, compact CountdownTimer
- Pending token section with ExpiryCountdown and cancel button in a compact inline layout
- All existing props preserved (Pod, BillingSession, AuthTokenInfo, onCancelToken)

**CountdownTimer.tsx** -- Replaced progress bar with SVG radial ring:
- `<svg viewBox="0 0 100 100">` with background circle (stroke #333) and progress circle
- `strokeDasharray="251.2"` with `strokeDashoffset={251.2 * (1 - remaining/allocated)}`
- Threshold colors: normal = rp-red, isLow (<300s) = amber #f59e0b, isCritical (<60s) = red #ef4444 + animate-pulse
- `compact` prop: renders w-20 h-20 ring only (no driving state row) -- used by PodCard
- Full variant: w-28 h-28 ring + driving state indicator (active/idle/no telemetry)
- `formatCountdown` export preserved for external consumers

**PinPad.tsx** -- New reusable 6-digit PIN input:
- Props: onComplete, onReset, disabled, error, loading, digits (default 6)
- PIN display with filled bullets and empty boxes, Racing Red accent on filled
- 3x4 grid numpad (1-9, CLR, 0, backspace)
- Keyboard handler: 0-9 adds digit, Backspace removes, Escape clears
- Auto-submit on 6th digit, auto-reset after onComplete fires

### Task 2: AppShell + Sidebar redesign + DashboardLayout wire-up
**Commit:** `a55cc31e`

**AppShell.tsx** -- New minimal wrapper injecting ToastProvider:
- Wraps children with `<ToastProvider>` from plan 01
- Allows any page inside DashboardLayout to call `useToast()` without per-page wrapping

**Sidebar.tsx** -- Full redesign with Lucide icons and fleet health:
- 21 Lucide icon imports replacing emoji strings (LayoutDashboard, Cpu, Gamepad2, etc.)
- Active nav: `bg-rp-red/10 text-rp-red border-l-4 border-rp-red pl-3` (left border, not right)
- Inactive nav: `text-neutral-400 hover:text-white hover:bg-rp-card pl-4`
- Fleet health heatmap: 8 dots (pods 1-8) polling `/api/v1/fleet/health` every 10s via fetchPublic
  - Green dot = ws_connected, yellow = http_reachable only, grey = unreachable
- Server WS indicator: polls `/api/v1/health` every 15s, green/red dot
- Both indicators above Presenter View / Kiosk Mode links

**DashboardLayout.tsx** -- Updated to wrap with AppShell:
- Added `import AppShell from "./AppShell"` and wrapped entire return in `<AppShell>`
- BackButton and parentMap logic unchanged

## Verification Results

1. TypeScript `tsc --noEmit`: 0 errors
2. `stroke-dashoffset` present in CountdownTimer.tsx: PASS
3. `border-l-4` present in Sidebar.tsx: PASS (active nav left border)
4. `fleet/health` present in Sidebar.tsx: PASS (polling endpoint)
5. `rp-red-light` in any component: 0 hits (removed)
6. PinPad.tsx and AppShell.tsx exist: PASS

## Deviations from Plan

None -- plan executed exactly as written.

## Known Stubs

None -- all components are fully wired with real data sources and callbacks.
