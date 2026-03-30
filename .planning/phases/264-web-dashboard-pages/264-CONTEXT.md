# Phase 264: Web Dashboard Pages - Context

**Gathered:** 2026-03-30
**Status:** Ready for planning

<domain>
## Phase Boundary
Redesign all 8 web dashboard pages using Phase 263 primitives. Dashboard home with KPI tiles, Pods page with F1 timing tower, Sessions, Billing, Fleet Health grid, Leaderboards, Settings, and all remaining pages updated to new AppShell.
Requirements: WD-01..WD-08
</domain>

<decisions>
## Implementation Decisions
### Layout Pattern
- All pages use AppShell (sidebar + top bar)
- Dashboard home: 4-5 MetricCards at top, PodStripList below, recent activity feed
- Pods: F1 timing tower on left, detail drawer on right
- Fleet Health: responsive pod grid with status indicators
- Leaderboards: F1-style table with PB/session-best highlighting
- Billing: wallet management, transaction history, session cards
- Settings: venue config, theme preview

### Claude's Discretion
Page layouts, data fetching patterns, responsive breakpoints, chart library choice.
</decisions>

<code_context>
## Existing Code Insights
- 25+ existing page directories in web/src/app/
- Each page fetches from http://192.168.31.23:8080/api/v1/...
- WebSocket dashboard connection for real-time updates
- Existing pages: pods, sessions, billing, leaderboards, settings, cameras, drivers, events, etc.
</code_context>

<specifics>
- Dashboard home must show live KPIs (active sessions, pods online, revenue today)
- Pods page must have F1 timing tower vertical pod strip
- All pages must use consistent AppShell from Phase 263
</specifics>

<deferred>
- Animated route transitions
- Command palette
</deferred>
