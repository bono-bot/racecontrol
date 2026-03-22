# Phase 7: Dashboard Cloud Deploy - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning
**Source:** Smart Discuss (infrastructure deployment phase)

<domain>
## Phase Boundary

Deploy the existing live ops dashboard (racecontrol/web/) to dashboard.racingpoint.cloud. The dashboard is already in the racecontrol repo, has a working Dockerfile (Alpine, port 3200), and Caddy already has the routing configured. compose.yml already has the dashboard service defined — needs API URL suffix fix and Caddy depends_on.

</domain>

<decisions>
## Implementation Decisions

### Deployment Config
- Dashboard Dockerfile uses `node:22-alpine`, port 3200 (already fixed in Phase 1)
- compose.yml needs `NEXT_PUBLIC_API_URL` to include `/api/v1` suffix — dashboard's `api.ts` uses the env var directly without appending path
- Dashboard needs adding to Caddy `depends_on` (same fix as admin in Phase 6)
- No NEXT_PUBLIC_IS_CLOUD needed — dashboard doesn't use this flag
- Dashboard authenticates via same staff PIN → JWT flow

### Data & Sync
- Dashboard reads from cloud racecontrol API (pod status, sessions, revenue)
- Real-time updates via polling (WebSocket for pod grid deferred — cloud racecontrol doesn't expose WS to dashboard)

### Claude's Discretion
- All implementation choices — pure infrastructure deployment

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `web/Dockerfile` — node:22-alpine, standalone, port 3200, accepts NEXT_PUBLIC_API_URL as build ARG
- `cloud/compose.yml` — dashboard service already defined with correct build context and healthcheck

### Established Patterns
- Phase 2/6 pattern: fix compose.yml → send Bono deploy instructions → automated checkpoint verify
- Dashboard `web/src/lib/api.ts` uses `NEXT_PUBLIC_API_URL` directly (no suffix appended)

### Integration Points
- Caddy routes dashboard.racingpoint.cloud → dashboard:3200
- Dashboard → cloud racecontrol API at :8080

</code_context>

<specifics>
## Specific Ideas

- Minimal changes needed — compose.yml API URL fix + Caddy depends_on
- dashboard.racingpoint.cloud already returns HTTP 200 from Caddy — just serving wrong content

</specifics>

<deferred>
## Deferred Ideas

None

</deferred>

---

*Phase: 07-dashboard-cloud-deploy*
*Context gathered: 2026-03-22 via Smart Discuss (infrastructure deployment)*
