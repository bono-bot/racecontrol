# Technology Stack: Cloud Deployment

**Project:** Racing Point Cloud Platform
**Researched:** 2026-03-21
**Overall confidence:** HIGH

## Recommended Stack

### Reverse Proxy & TLS

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| Caddy | 2.11.x | Reverse proxy, automatic HTTPS, subdomain routing | Automatic Let's Encrypt provisioning and renewal with zero config. A 10-line Caddyfile replaces 80+ lines of Nginx config plus Certbot cron jobs. Wildcard subdomain support is built-in since 2.10. Already battle-tested for exactly this use case (multiple Docker containers behind subdomains on a single VPS). |

**Confidence:** HIGH -- Caddy is the consensus choice for small-to-mid VPS deployments in 2025-2026. Nginx is viable but adds unnecessary operational burden for this scale.

### Container Orchestration

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| Docker Compose | v2.x (bundled with Docker Engine) | Multi-container orchestration | Already used in the existing stack. Defines all 4 services (Caddy + 3 Next.js apps) in a single `compose.yml`. No Kubernetes, no Portainer -- unnecessary complexity for 3 containers. |
| Docker Engine | 27.x | Container runtime | Standard. Already on the VPS (Bono manages it). |

**Confidence:** HIGH -- Docker Compose is the standard for single-VPS multi-container deployments.

### Container Images

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| node:22-alpine | 22-alpine | Base image for all 3 Next.js apps | Already in use. Multi-stage Dockerfile with standalone output keeps images at ~150-200MB. Non-root user (`nextjs:nodejs`) already configured. |
| caddy:2-alpine | 2-alpine | Reverse proxy container | Official image, ~40MB, auto-updates certs. Mount Caddyfile + data/config volumes. |

**Confidence:** HIGH -- Matches existing Dockerfiles exactly.

### CI/CD (Deploy Pipeline)

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| GitHub Actions | N/A | Build images, deploy via SSH | Free for private repos (2000 min/month). Push-to-main triggers build + deploy. Uses `appleboy/ssh-action` to SSH into VPS and run `docker compose pull && docker compose up -d`. |

**Confidence:** MEDIUM -- GitHub Actions is standard, but the simpler alternative (SSH + git pull + docker compose build on-VPS) may be preferable given Bono already manages deploys manually. Start with manual SSH deploys, add GitHub Actions later if deploy frequency warrants it.

### Data Sync

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| Existing cloud_sync.rs | N/A (custom) | Bidirectional SQLite sync between cloud and local | Already implemented, working, HMAC-signed. Supports relay mode (2s) and HTTP fallback (30s). Handles authority rules (cloud-authoritative for drivers/pricing, local-authoritative for billing/laps). Do NOT replace with cr-sqlite, LiteSync, or any CRDT library -- they solve a different problem (multi-writer conflict resolution) and would require rewriting the entire sync layer for zero benefit. |

**Confidence:** HIGH -- The existing sync is purpose-built for this exact use case with authority rules that CRDTs cannot express.

### DNS & Domain

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| Cloudflare DNS | N/A | DNS management for racingpoint.cloud | Free tier. Add A records for `app`, `admin`, `dashboard` subdomains pointing to VPS IP (72.60.101.58). Caddy handles TLS directly -- do NOT enable Cloudflare proxy (orange cloud) as it interferes with Caddy's ACME challenges. DNS-only mode (grey cloud). |

**Confidence:** MEDIUM -- Assumes racingpoint.cloud is registered and DNS is manageable.

### Monitoring

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| Docker healthchecks | N/A | Container liveness | Built into compose.yml. Each Next.js container gets a healthcheck. |
| Existing WhatsApp alerts | N/A | Alerting | Already implemented in Phase 80. Extend to cover cloud container health. |

**Confidence:** HIGH -- No need for Prometheus/Grafana at this scale.

## Architecture: Caddyfile

The entire reverse proxy config for 3 subdomains:

```caddyfile
app.racingpoint.cloud {
    reverse_proxy pwa:3100
}

admin.racingpoint.cloud {
    reverse_proxy admin:3300
}

dashboard.racingpoint.cloud {
    reverse_proxy dashboard:3200
}
```

Caddy automatically obtains Let's Encrypt TLS certificates for all 3 domains, renews them before expiry, redirects HTTP to HTTPS, and serves HTTP/2 and HTTP/3.

## Architecture: compose.yml

```yaml
services:
  caddy:
    image: caddy:2-alpine
    restart: unless-stopped
    ports:
      - "80:80"
      - "443:443"
      - "443:443/udp"  # HTTP/3
    volumes:
      - ./Caddyfile:/etc/caddy/Caddyfile
      - caddy_data:/data
      - caddy_config:/config

  pwa:
    build:
      context: ./pwa
      args:
        NEXT_PUBLIC_API_URL: https://app.racingpoint.cloud/api/v1
    restart: unless-stopped
    expose:
      - "3100"

  admin:
    build:
      context: ./racingpoint-admin
      args:
        NEXT_PUBLIC_API_URL: https://admin.racingpoint.cloud/api/v1
    restart: unless-stopped
    expose:
      - "3300"

  dashboard:
    build:
      context: ./web
      args:
        NEXT_PUBLIC_API_URL: https://dashboard.racingpoint.cloud/api/v1
    restart: unless-stopped
    expose:
      - "3200"

volumes:
  caddy_data:
  caddy_config:
```

Key points: `expose` (not `ports`) means containers are only reachable via Caddy, not directly from the internet. `caddy_data` volume persists TLS certificates across restarts. All containers share the default Docker network, so Caddy resolves `pwa`, `admin`, `dashboard` by service name.

## What NOT to Use

| Technology | Why Not |
|------------|---------|
| **Nginx + Certbot** | More config, manual cert renewal cron, more failure modes. Caddy does the same with 10% of the config. |
| **Traefik** | Overkill for 3 static services. Traefik shines with dynamic container scaling/discovery. |
| **Kubernetes / K3s** | Massive overkill. 3 containers on 1 VPS. Docker Compose is the right tool. |
| **Portainer** | Adds a management UI for 4 containers. `docker compose up -d` is sufficient. |
| **cr-sqlite / LiteSync / CRDTs** | Existing cloud_sync.rs handles bidirectional sync with authority rules. CRDTs solve multi-writer conflicts, but this system has clear write-authority per table. |
| **Litestream** | One-directional replication only. Does not support bidirectional sync. |
| **PostgreSQL / MySQL** | SQLite is working, embedded, zero-ops. Single-venue data volume does not justify a DB server. |
| **Vercel / Railway / Fly.io** | Cost scales with traffic. A EUR 4/month Hetzner VPS handles all 3 apps. Rust backend already runs on the VPS. |
| **PM2** | Node process manager. Unnecessary when Docker handles restarts and lifecycle. |

## Resource Estimates (Hetzner VPS)

| Service | Memory | CPU | Disk |
|---------|--------|-----|------|
| Caddy | ~20MB | Negligible | ~50MB (certs) |
| PWA (Next.js) | ~150-200MB | Low | ~200MB (image) |
| Admin (Next.js) | ~150-200MB | Low | ~200MB (image) |
| Dashboard (Next.js) | ~150-200MB | Low | ~200MB (image) |
| racecontrol (Rust) | ~50MB | Low | ~30MB (binary) |
| **Total** | **~600-700MB** | Low | **~700MB** |

A Hetzner CX22 (2 vCPU, 4GB RAM, ~EUR 4/month) handles this comfortably.

## Open Questions

- Is racingpoint.cloud already registered and DNS pointed to the VPS?
- Does the Hetzner VPS firewall allow ports 80/443 inbound?
- Does the admin app (separate repo) need to be cloned into the same directory structure for the compose.yml to reference it?
- WebSocket passthrough for the dashboard -- Caddy handles this natively, but the dashboard's WS endpoint URL will need updating from `ws://192.168.31.23:8080` to `wss://dashboard.racingpoint.cloud`

---

*Stack research: 2026-03-21*
