# Phase 263: Web Primitive Components - Context

**Gathered:** 2026-03-30
**Status:** Ready for planning

<domain>
## Phase Boundary
Build all shared web components (SC-01 through SC-10) and redesign login page (LP-01, LP-02). These are the building blocks all dashboard pages will compose from. F1 timing tower inspired PodCard, motorsport StatusBadge, radial CountdownTimer, TanStack Table LiveDataTable, LeaderboardTable with AnimatePresence, Toast system, loading skeletons.
Requirements: SC-01..SC-10, LP-01, LP-02
</domain>

<decisions>
## Implementation Decisions
### Design Direction
- F1 timing tower style for PodCard (vertical strip, left-edge color bar, driver/timer/status)
- Racing flag color system for StatusBadge (green=ready, amber=pending, red=fault, grey=offline, blue=maintenance)
- Radial progress ring for CountdownTimer (neutral→amber→red thresholds)
- TanStack Table for LiveDataTable (web only, NOT kiosk)
- AnimatePresence + layout for LeaderboardTable row reordering
- 6-digit PinPad reusable component
- Sonner or custom toast for notifications
- Skeleton screens matching card layouts

### Claude's Discretion
Component internals, prop interfaces, animation durations, exact Tailwind classes.
</decisions>

<code_context>
## Existing Code Insights
### Reusable Assets
- web/src/components/PodCard.tsx — existing, needs redesign
- web/src/components/StatusBadge.tsx — existing, needs redesign
- web/src/components/CountdownTimer.tsx — existing, needs redesign
- web/src/components/DashboardLayout.tsx — existing AppShell equivalent
- web/src/components/Sidebar.tsx — existing, needs motorsport redesign

### Established Patterns
- All components are React client components ("use client")
- API fetching via fetch() with NEXT_PUBLIC_API_URL
- WebSocket via native WebSocket in useEffect hooks
</code_context>

<specifics>
- PodCard must show: pod number, status bar, driver name, countdown timer, game icon
- LeaderboardTable must handle WS reconnect (cleanup on unmount) — highest risk component
- Login page: 6-digit PIN, Racing Red accents, "RaceControl" branding, error states
- AppShell: collapsible sidebar with Lucide icons, fleet health strip in footer
</specifics>

<deferred>
- Command palette (Ctrl+K)
- Revenue sparkline in MetricCard
</deferred>
