# Phase 287: Metrics Dashboard - Context

**Gathered:** 2026-04-01
**Status:** Ready for planning
**Mode:** Auto-generated (discuss skipped, stub API approach)

<domain>
## Phase Boundary

Next.js /metrics page in racingpoint-admin with sparkline charts, pod selector, time range picker, 30s auto-refresh, and headline snapshot numbers. Phase 286 (Query API) is NOT yet executed — this phase uses STUB/MOCK data that matches the planned API contracts. Stubs will be replaced with real API calls when Phase 286 ships.

</domain>

<decisions>
## Implementation Decisions

### API Approach (STUB MODE)
- Create `lib/api/metrics.ts` with stub functions that return realistic mock data
- Stub functions match the exact contract of Phase 286's planned endpoints:
  - `fetchMetricsQuery(metric, from, to, pod?)` → `{ points: [{ts, value}], resolution: string }`
  - `fetchMetricNames()` → `string[]`
  - `fetchMetricsSnapshot(pod?)` → `{ metrics: [{metric_name, pod_id, value, recorded_at}] }`
- Each stub has a `// TODO: Replace with real API call when Phase 286 ships` comment
- Stubs generate plausible time-series data (sine wave + noise for demo)

### Dashboard Layout
- New route: `src/app/(dashboard)/metrics/page.tsx`
- Follow existing admin patterns: useSWR for data fetching, Racing Point brand colors
- Headline numbers row at top (current snapshot values)
- Sparkline grid below (one chart per metric, filterable by pod)
- Time range picker and pod selector as controls above the chart grid
- 30s auto-refresh via SWR refreshInterval

### Chart Library
- Use recharts (already in package.json ^3.7.0)
- Sparkline = AreaChart with no axes for compact view, full LineChart on click/expand
- Racing Point brand: #E10600 red accent, #222222 card bg, #333333 borders

### Claude's Discretion
- Chart sizing, grid layout (responsive columns)
- Exact sparkline styling (gradient fill, stroke width)
- Loading/empty/error states
- Whether to add navigation link to sidebar

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `src/app/(dashboard)/fleet/page.tsx` — PodCard pattern, useSWR, toast, useAuth
- `src/lib/api/fleet.ts` — API client pattern (fetcher function + typed responses)
- `src/app/(dashboard)/layout.tsx` — Sidebar navigation
- `src/hooks/useAuth.ts` — Auth hook
- `recharts` ^3.7.0 already installed
- Brand: bg-rp-card (#222222), border-rp-border (#333333), Racing Red #E10600

### Established Patterns
- useSWR with refreshInterval for auto-polling
- Type-safe API clients in `lib/api/`
- Dashboard pages in `(dashboard)/` route group
- Responsive grid with Tailwind CSS
- No `any` in TypeScript (standing rule)

### Integration Points
- Add `metrics` route to `(dashboard)/` directory
- Add sidebar link in layout.tsx
- API base URL from environment or relative path

</code_context>

<specifics>
## Specific Ideas

No specific requirements beyond ROADMAP success criteria.

</specifics>

<deferred>
## Deferred Ideas

- Real API integration (Phase 286 dependency — stub replacement)
- Metric annotations (v2)
- Custom dashboard layouts (v2)

</deferred>
