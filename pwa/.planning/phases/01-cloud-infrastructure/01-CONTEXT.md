# Phase 1: Cloud Infrastructure - Context

**Gathered:** 2026-03-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Set up DNS, Caddy reverse proxy, Docker Compose, firewall, and swap on Bono's VPS (72.60.101.58) so that all racingpoint.cloud subdomains resolve, terminate TLS, and route to running containers. This is pure infrastructure — no application code changes.

**Key constraint:** One customer = one account = one pod at a time. Cloud must mirror this same constraint from local. No multi-pod bookings from a single customer account.

</domain>

<decisions>
## Implementation Decisions

### Caddy Placement
- Caddy runs **in Docker** (not on host) — keeps everything in a single compose.yml, Bono can `docker compose up -d` and everything works
- caddy:2-alpine image, ~40MB
- Caddyfile mounted as volume from repo
- `caddy_data` volume persists TLS certificates across restarts
- Caddy handles automatic Let's Encrypt, HTTP→HTTPS redirect, HTTP/2 and HTTP/3

### DNS & Domains
- **4 subdomains**, all A records pointing to 72.60.101.58:
  - `app.racingpoint.cloud` → PWA (customer-facing)
  - `admin.racingpoint.cloud` → racingpoint-admin (business ops)
  - `dashboard.racingpoint.cloud` → web/ dashboard (live ops)
  - `api.racingpoint.cloud` → cloud racecontrol API (:8080)
- Cloudflare DNS in **DNS-only mode** (grey cloud, no proxy) — so Caddy can do its own ACME challenges
- Low TTL (300s) initially for fast iteration
- Caddy obtains individual Let's Encrypt certs per subdomain (not wildcard)

### Repo Structure & Admin
- racingpoint-admin is a **separate repo** — cloned alongside racecontrol on Bono's VPS
- compose.yml lives in a dedicated **deploy directory** on the VPS (e.g., `/opt/racingpoint/`)
- compose.yml references each app by relative path:
  - `./racecontrol/pwa` for PWA
  - `./racecontrol/web` for dashboard
  - `./racingpoint-admin` for admin
- No git submodules — simple directory structure on VPS

### Container Configuration
- All frontends use `expose` (not `ports`) — only reachable through Caddy
- Cloud racecontrol binary runs directly on host (already deployed by Bono) at :8080 — Caddy proxies to `host.docker.internal:8080` or `172.17.0.1:8080`
- Memory limits per container:
  - PWA: 512MB
  - Admin: 512MB (needs more for better-sqlite3)
  - Dashboard: 512MB
  - Caddy: 128MB
- Docker Compose healthchecks: `curl -f http://localhost:PORT/ || exit 1` for each frontend

### VPS Setup
- Firewall: allow inbound 80, 443 only. SSH (22) already allowed.
- Swap: 2GB swapfile (`fallocate -l 2G /swapfile && mkswap && swapon`)
- Add swap to `/etc/fstab` for persistence across reboots
- Total expected memory: ~600-700MB active, well within 4GB VPS

### Cloud-Local Alignment
- Cloud racecontrol runs the same binary as local, with cloud-specific config
- Same database schema (SQLite), synced via existing cloud_sync.rs
- Same API endpoints — frontends just point NEXT_PUBLIC_API_URL to cloud
- One customer, one account, one active pod reservation — enforced at API level (same logic as local)
- Cloud cannot directly control pods — all pod commands go through local server via sync

### Claude's Discretion
- Exact Caddyfile header configuration (HSTS, security headers)
- Docker network configuration details
- Exact healthcheck intervals and retry counts
- Whether to add a `watchtower` or similar auto-update container

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Infrastructure Research
- `pwa/.planning/research/STACK.md` — Full stack recommendation including Caddy config, compose.yml template, resource estimates
- `pwa/.planning/research/ARCHITECTURE.md` — Component boundaries, data flow between cloud and local

### Existing Dockerfiles
- `pwa/Dockerfile` — PWA container (node:22-alpine, standalone, port 3100)
- `web/Dockerfile` — Dashboard container (node:22-alpine, standalone, port 3000 — needs updating to 3200)
- `racingpoint-admin/Dockerfile` — Admin container (node:22-bookworm-slim, better-sqlite3 native build)

### Project Context
- `racecontrol/CLAUDE.md` — Full network map, server services table (ports), deploy rules
- `pwa/.planning/PROJECT.md` — Cloud platform vision, constraints, data authority model

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `pwa/Dockerfile` — Multi-stage build, already working for standalone Next.js deploy
- `web/Dockerfile` — Same pattern, port needs changing from 3000 to 3200
- `racingpoint-admin/Dockerfile` — Different base (bookworm-slim for native deps), already working
- Cloud racecontrol binary — already deployed and running on Bono's VPS

### Established Patterns
- All Next.js apps use `output: standalone` in next.config.ts
- `NEXT_PUBLIC_API_URL` passed as Docker build ARG (baked at build time)
- Non-root user (`nextjs:nodejs`) in all containers
- Port convention: PWA=3100, Dashboard=3200, Kiosk/Admin=3300

### Integration Points
- Caddy → frontend containers (reverse proxy by Docker service name)
- Caddy → cloud racecontrol (reverse proxy to host port 8080)
- Cloud racecontrol → local racecontrol (existing cloud_sync.rs, HMAC-signed)
- DNS → Caddy (A records for 4 subdomains)

</code_context>

<specifics>
## Specific Ideas

- Cloud must mirror local's one-customer-one-pod constraint — no divergence between environments
- Bono manages the VPS, James prepares the configs — coordinate via comms-link
- All configs (Caddyfile, compose.yml) should be committed to the repo so both AIs can review

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 01-cloud-infrastructure*
*Context gathered: 2026-03-21*
