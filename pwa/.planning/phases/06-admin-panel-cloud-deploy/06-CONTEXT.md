# Phase 6: Admin Panel Cloud Deploy - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning
**Source:** Smart Discuss (infrastructure deployment phase)

<domain>
## Phase Boundary

Deploy the existing racingpoint-admin panel to admin.racingpoint.cloud using the Docker infrastructure from Phase 1. The admin panel already exists as a separate repo (racingpoint-admin) with a working Dockerfile. Caddy already has the admin.racingpoint.cloud routing configured. This phase activates the admin service in Docker Compose.

</domain>

<decisions>
## Implementation Decisions

### Deployment Strategy
- racingpoint-admin is a **separate repo** — already cloned on VPS at `/opt/racingpoint/racingpoint-admin`
- compose.yml already has the admin service defined — needs build context path and correct env vars
- Admin panel uses `better-sqlite3` (native module) — requires `node:22-bookworm-slim` base (NOT Alpine)
- Admin connects to cloud racecontrol API at `https://api.racingpoint.cloud` (same as PWA)
- Admin authenticates via existing staff PIN → JWT flow (same auth as local)

### Authentication
- Admin panel requires authentication before any page loads — existing PIN-based auth
- Share same JWT secret as local and PWA (already configured in Phase 2)
- No new auth code needed — just deployment

### Data & Sync
- Admin reads from cloud racecontrol API which syncs from local via cloud_sync.rs
- Config changes (pricing tiers, experiences, kiosk settings) sync back to local server
- Cloud-authoritative for admin config, local-authoritative for billing — existing sync rules apply

### Claude's Discretion
- Exact NEXT_PUBLIC_API_URL value for admin build
- Whether admin needs NEXT_PUBLIC_IS_CLOUD env var
- Docker Compose healthcheck details for admin service

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `racingpoint-admin/Dockerfile` — node:22-bookworm-slim, better-sqlite3 native build, port 3300
- `cloud/compose.yml` — admin service already defined (may need build context fix)
- `cloud/Caddyfile` — admin.racingpoint.cloud routing already configured

### Established Patterns
- Phase 2 pattern: update compose.yml build args → send Bono deploy instructions → checkpoint verify
- Admin panel is a Next.js app with `output: standalone` (same as PWA)
- Port convention: Admin = 3300

### Integration Points
- Caddy routes admin.racingpoint.cloud → admin:3300
- Admin → cloud racecontrol API at :8080 (via api.racingpoint.cloud or direct)
- Admin changes sync to local via cloud_sync.rs

</code_context>

<specifics>
## Specific Ideas

- Follow exact same pattern as Phase 2 (PWA deploy) — proven approach
- Bono already has racingpoint-admin cloned on VPS
- admin.racingpoint.cloud already returns HTTP 200 from Caddy (placeholder) — just needs the actual admin container

</specifics>

<deferred>
## Deferred Ideas

None — deployment phase, stays within scope

</deferred>

---

*Phase: 06-admin-panel-cloud-deploy*
*Context gathered: 2026-03-22 via Smart Discuss (infrastructure deployment)*
