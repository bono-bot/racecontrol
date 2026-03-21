# Phase 2: API + PWA Cloud Deploy - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Deploy the existing customer PWA and cloud racecontrol API to racingpoint.cloud using the Docker infrastructure from Phase 1. No new features — this is deploying existing code to the cloud environment so customers can access it from any device.

</domain>

<decisions>
## Implementation Decisions

### API Configuration
- `NEXT_PUBLIC_API_URL` set to `https://api.racingpoint.cloud/api/v1` as Docker build ARG — matches Caddy routing from Phase 1
- Share existing WhatsApp/Twilio credentials — same Business account, OTP works regardless of origin
- Share existing Razorpay keys — same merchant account, payments route to same account
- Share same JWT signing secret via env var — tokens work across local and cloud (enables SSO-like behavior)

### Build & Deploy Strategy
- `docker compose build` from `/opt/racingpoint/` on VPS — compose.yml already has build context and ARG for NEXT_PUBLIC_API_URL
- Updates via `git pull && docker compose up -d --build` on VPS — Bono pulls latest, rebuilds (CI/CD deferred to Phase 8)
- Cloud racecontrol binary managed by Bono separately — runs on host, not in Docker, existing deploy flow
- Fresh SQLite database + let cloud_sync populate — sync already handles all tables bidirectionally, no manual seeding needed

### PWA Cloud Identity
- Update PWA manifest: `start_url: "/"`, `scope: "/"` — verify name/short_name are correct for cloud install
- No custom service worker needed — Next.js standalone + manifest is sufficient for PWA installability
- No CORS configuration needed — Caddy proxies `api.racingpoint.cloud` to `:8080` on same origin
- Set `NEXT_PUBLIC_IS_CLOUD="true"` as Docker build ARG — existing code uses this to toggle cloud-specific features (booking page)

### Claude's Discretion
- Exact Docker build arg injection method in compose.yml
- Whether to add a healthcheck endpoint specific to cloud API
- Any additional environment variables needed for cloud racecontrol

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `pwa/Dockerfile` — Multi-stage Alpine build, standalone output, port 3100, already accepts NEXT_PUBLIC_API_URL as build ARG
- `pwa/public/manifest.json` — PWA manifest, needs verification for cloud domain
- `pwa/src/lib/api.ts` — API_BASE_URL from NEXT_PUBLIC_API_URL with localhost:8080 fallback
- `cloud/compose.yml` — From Phase 1, already has PWA service with build context

### Established Patterns
- `NEXT_PUBLIC_API_URL` baked at build time via Docker ARG (not runtime env)
- `NEXT_PUBLIC_IS_CLOUD` used in booking page to detect cloud mode
- All API calls go through `pwa/src/lib/api.ts` centralized client
- Cloud racecontrol runs same binary as local with cloud-specific config

### Integration Points
- Caddy routes `app.racingpoint.cloud` → PWA container (:3100)
- Caddy routes `api.racingpoint.cloud` → host racecontrol (:8080)
- cloud_sync.rs handles bidirectional data sync between local and cloud
- WhatsApp OTP sent via existing WhatsApp Business API

</code_context>

<specifics>
## Specific Ideas

- The compose.yml from Phase 1 already has the PWA service defined — this phase activates it with correct build args
- Cloud racecontrol binary is already running on Bono's VPS — API endpoints already available at :8080
- Cloud sync is already operational (Phase 3 shipped) — data flows automatically

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 02-api-pwa-cloud-deploy*
*Context gathered: 2026-03-22 via Smart Discuss*
