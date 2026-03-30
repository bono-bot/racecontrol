---
phase: 263-web-primitive-components
plan: 01
subsystem: ui
tags: [react, tailwind, design-system, components, toast, skeleton]

requires:
  - phase: 261-design-system-foundation
    provides: shared-tokens/tokens.css with rp-* design tokens

provides:
  - StatusBadge with 7-class racing flag color system (green/red/amber/grey/blue/orange/yellow)
  - MetricCard KPI tile with title/value/delta/alert/loading states
  - ToastProvider + useToast context-based notification system
  - Skeleton/SkeletonCard/SkeletonRow/EmptyState loading primitives

affects: [263-02 PodCard redesign, 263-03 AppShell, 264 dashboard pages]

tech-stack:
  added: []
  patterns: [racing-flag-color-map, context-based-toast, shimmer-skeleton]

key-files:
  created:
    - web/src/components/MetricCard.tsx
    - web/src/components/Toast.tsx
    - web/src/components/Skeleton.tsx
  modified:
    - web/src/components/StatusBadge.tsx

key-decisions:
  - "Custom toast implementation over Sonner to keep bundle lean (zero new deps)"
  - "Racing flag system uses semantic class names (green/red/amber) mapped to rp-* tokens, not direct hex"
  - "Skeleton base accepts style prop for percentage widths in SkeletonRow"

patterns-established:
  - "Racing flag color map: STATUS_TO_FLAG maps every status string to one of 7 semantic classes, FLAG_STYLES maps classes to bg/text/dot Tailwind classes"
  - "Context-based toast: ToastProvider at layout root, useToast() hook from any child"
  - "Skeleton composition: base Skeleton + domain-specific wrappers (SkeletonCard, SkeletonRow)"

requirements-completed: [SC-01, SC-02, SC-09, SC-10]

duration: 3min
completed: 2026-03-30
---

# Phase 263 Plan 01: Web Primitive Components Summary

**Four leaf-node UI primitives: StatusBadge with racing flag colors, MetricCard KPI tile, context-based Toast notifications, and Skeleton/EmptyState loading states**

## Tasks Completed

| # | Task | Commit | Key Changes |
|---|------|--------|-------------|
| 1 | StatusBadge racing flag redesign + MetricCard | `60666255` | Replaced ad-hoc color map with 7-class flag system; new MetricCard with delta arrows |
| 2 | Toast system + Skeleton/EmptyState | `01179ffd` | ToastProvider/useToast context queue; Skeleton/SkeletonCard/SkeletonRow/EmptyState |

## Implementation Details

### StatusBadge (redesigned)
- 7 semantic flag classes: green, red, amber, grey, blue, orange, yellow (+ purple for timing)
- All 20+ status strings mapped via `STATUS_TO_FLAG` lookup
- Uses rp-* design tokens exclusively (bg-rp-green/20, text-rp-red, etc.)
- Pulsing dot for active states (in_session, active, running, launching, loading, waiting_for_game)
- Zero hex literals, zero rp-red-light references

### MetricCard (new)
- Props: title, value, unit, delta, deltaLabel, alert, loading
- Value undefined renders em-dash placeholder
- Delta: green arrow-up for positive, red arrow-down for negative, grey for zero
- Alert mode: red border accent (border-rp-red)
- Loading mode: inline skeleton (no Skeleton.tsx import needed)

### Toast (new)
- Context-based: ToastProvider wraps layout, useToast() hook anywhere
- 4 types: success (green), error (red), warning (amber), info (blue)
- Auto-dismiss: 4000ms default, 6000ms for errors, override via duration prop
- Max 5 visible, FIFO queue, fixed top-right stack
- Inline SVG icons per type, dismiss button on each toast

### Skeleton/EmptyState (new)
- Skeleton: base shimmer div with animate-pulse + bg-rp-border
- SkeletonCard: 3-row card matching PodCard layout
- SkeletonRow: 5-cell row with percentage widths (16/28/20/16/12%)
- EmptyState: centered icon + headline + optional hint text

## Deviations from Plan

None - plan executed exactly as written.

## Known Stubs

None - all components are fully functional primitives with no data source dependencies.

## Verification Results

- TypeScript: 0 errors across all 4 files
- No rp-red-light references in any component
- All files exist at correct paths
- ToastProvider and useToast both exported from Toast.tsx
- EmptyState, SkeletonCard, SkeletonRow all exported from Skeleton.tsx

## Self-Check: PASSED

- StatusBadge.tsx: FOUND
- MetricCard.tsx: FOUND
- Toast.tsx: FOUND
- Skeleton.tsx: FOUND
- Commit 60666255: FOUND
- Commit 01179ffd: FOUND
