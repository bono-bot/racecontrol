# Phase 1: Cloud Infrastructure - Research

**Researched:** 2026-03-22
**Domain:** VPS infrastructure (DNS, reverse proxy, Docker Compose, firewall, swap)
**Confidence:** HIGH

## Summary

Phase 1 is pure infrastructure -- no application code changes. The goal is to make four subdomains (app, admin, dashboard, api) resolve to the VPS, terminate TLS via Caddy, and route to their respective containers/services. All decisions have been locked in CONTEXT.md: Caddy runs in Docker, Cloudflare DNS in DNS-only mode, containers use `expose` not `ports`, cloud racecontrol runs on the host at :8080.

The stack is well-established and documented. Caddy 2 with automatic ACME/Let's Encrypt is the consensus choice for small VPS deployments. Docker Compose v2 handles multi-container orchestration. The existing Dockerfiles for PWA (port 3100), Dashboard (port 3000, needs update to 3200), and Admin (port 3300) are already working. The main implementation work is: (1) create DNS A records in Cloudflare, (2) write a Caddyfile with security headers, (3) write compose.yml with memory limits and healthchecks, (4) configure UFW firewall, (5) enable 2GB swap.

**Primary recommendation:** Create all config files (Caddyfile, compose.yml) in the repo under a `deploy/` directory. Bono clones/pulls on the VPS and runs `docker compose up -d`. Coordinate with Bono via comms-link for DNS and firewall changes that require VPS access.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Caddy runs **in Docker** (not on host) -- single compose.yml, caddy:2-alpine image, Caddyfile mounted as volume, `caddy_data` volume for TLS cert persistence
- **4 subdomains**, all A records pointing to 72.60.101.58: app.racingpoint.cloud (PWA :3100), admin.racingpoint.cloud (Admin :3300), dashboard.racingpoint.cloud (Dashboard :3200), api.racingpoint.cloud (cloud racecontrol :8080)
- Cloudflare DNS in **DNS-only mode** (grey cloud) -- Caddy does its own ACME challenges
- Low TTL (300s) initially
- Individual Let's Encrypt certs per subdomain (not wildcard)
- racingpoint-admin is a **separate repo** -- cloned alongside racecontrol on VPS
- compose.yml lives in `/opt/racingpoint/` on VPS, references apps by relative path
- All frontends use `expose` (not `ports`) -- only reachable through Caddy
- Cloud racecontrol binary runs on host at :8080 -- Caddy proxies to `host.docker.internal:8080` or `172.17.0.1:8080`
- Memory limits: PWA 512MB, Admin 512MB, Dashboard 512MB, Caddy 128MB
- Docker Compose healthchecks: `curl -f http://localhost:PORT/ || exit 1`
- Firewall: allow inbound 80, 443 only (SSH 22 already allowed)
- Swap: 2GB swapfile via fallocate, added to /etc/fstab

### Claude's Discretion
- Exact Caddyfile header configuration (HSTS, security headers)
- Docker network configuration details
- Exact healthcheck intervals and retry counts
- Whether to add a `watchtower` or similar auto-update container

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| INFRA-01 | DNS A records for app, admin, dashboard, api subdomains pointing to VPS (72.60.101.58) | Cloudflare DNS-only mode with 300s TTL. 4 A records. Verified approach works with Caddy ACME. |
| INFRA-02 | Caddy reverse proxy routes each subdomain to correct container with auto-TLS (Let's Encrypt) | Caddy 2.11.x in Docker, Caddyfile with 4 site blocks, automatic ACME, security headers snippet. caddy_data volume persists certs. |
| INFRA-03 | Docker Compose orchestrates all services (Caddy + 3 frontends + racecontrol) with memory limits and healthchecks | compose.yml with deploy.resources.limits.memory, healthcheck with curl, expose-only networking. Racecontrol on host proxied via extra_hosts. |
| INFRA-06 | VPS firewall allows ports 80/443 inbound, all other ports blocked externally | UFW: default deny incoming, allow 22/tcp, 80/tcp, 443/tcp. Must allow SSH first before enabling. |
| INFRA-07 | Swap enabled on VPS (2GB) to prevent OOM with 3 Next.js containers | fallocate 2G swapfile, mkswap, swapon, /etc/fstab entry for persistence. |
</phase_requirements>

## Standard Stack

### Core
| Component | Version | Purpose | Why Standard |
|-----------|---------|---------|--------------|
| Caddy | 2-alpine (2.11.x) | Reverse proxy, auto-TLS, security headers | Automatic Let's Encrypt with zero config. 10-line Caddyfile replaces 80+ lines Nginx + Certbot cron. |
| Docker Compose | v2 (bundled with Docker Engine) | Multi-container orchestration | Standard for single-VPS. Already on VPS. |
| Docker Engine | 27.x | Container runtime | Already installed on Bono's VPS. |
| UFW | System package | Firewall management | Standard Ubuntu firewall tool, already available on Hetzner VPS. |

### Container Images
| Image | Tag | Purpose | Size |
|-------|-----|---------|------|
| caddy | 2-alpine | Reverse proxy | ~40MB |
| node | 22-alpine | PWA + Dashboard base | ~150-200MB per app |
| node | 22-bookworm-slim | Admin base (native deps) | ~250MB |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Caddy | Nginx + Certbot | More config, manual cert renewal cron, more failure modes |
| Caddy | Traefik | Overkill for 3 static services; Traefik shines with dynamic scaling |
| Docker Compose | K3s/Kubernetes | Massive overkill for 4 containers on 1 VPS |
| UFW | iptables directly | UFW is a user-friendly wrapper; iptables for advanced needs only |

## Architecture Patterns

### Recommended Directory Structure on VPS
```
/opt/racingpoint/
  compose.yml          # Docker Compose file
  Caddyfile            # Caddy reverse proxy config
  racecontrol/         # git clone of racecontrol repo
    pwa/               # PWA Dockerfile here
    web/               # Dashboard Dockerfile here
  racingpoint-admin/   # git clone of admin repo (separate)
```

### Pattern 1: Caddy-in-Docker with Host Service Proxy
**What:** Caddy runs inside Docker alongside the frontend containers. For the API (racecontrol on host at :8080), Caddy uses `extra_hosts` to resolve `host.docker.internal`.
**When to use:** When a service runs on the host but other services are in Docker.
**Example:**
```yaml
# compose.yml
services:
  caddy:
    image: caddy:2-alpine
    restart: unless-stopped
    extra_hosts:
      - "host.docker.internal:host-gateway"
    ports:
      - "80:80"
      - "443:443"
      - "443:443/udp"  # HTTP/3
    volumes:
      - ./Caddyfile:/etc/caddy/Caddyfile:ro
      - caddy_data:/data
      - caddy_config:/config
    deploy:
      resources:
        limits:
          memory: 128M
    healthcheck:
      test: ["CMD", "caddy", "version"]
      interval: 30s
      timeout: 5s
      retries: 3
```

```caddyfile
# Caddyfile
(security_headers) {
    header {
        Strict-Transport-Security "max-age=31536000; includeSubDomains; preload"
        X-Content-Type-Options "nosniff"
        X-Frame-Options "DENY"
        Referrer-Policy "strict-origin-when-cross-origin"
        Permissions-Policy "interest-cohort=()"
        -Server
    }
}

app.racingpoint.cloud {
    import security_headers
    reverse_proxy pwa:3100
}

admin.racingpoint.cloud {
    import security_headers
    reverse_proxy admin:3300
}

dashboard.racingpoint.cloud {
    import security_headers
    reverse_proxy dashboard:3200
}

api.racingpoint.cloud {
    import security_headers
    reverse_proxy host.docker.internal:8080
}
```

### Pattern 2: Memory-Limited Containers with Healthchecks
**What:** Each container has a deploy.resources.limits.memory cap and a healthcheck.
**When to use:** Always on resource-constrained VPS.
**Example:**
```yaml
  pwa:
    build:
      context: ./racecontrol/pwa
      args:
        NEXT_PUBLIC_API_URL: https://api.racingpoint.cloud
    restart: unless-stopped
    expose:
      - "3100"
    deploy:
      resources:
        limits:
          memory: 512M
    healthcheck:
      test: ["CMD", "wget", "--no-verbose", "--tries=1", "--spider", "http://localhost:3100/"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 40s
```

**Note on healthcheck command:** The existing Dockerfiles use `node:22-alpine` which does NOT include `curl`. Use `wget --spider` instead (wget is included in Alpine), or install curl in the Dockerfile. For the admin container (bookworm-slim), curl is available.

### Pattern 3: UFW Firewall Lockdown
**What:** Default-deny inbound, allow only SSH + HTTP + HTTPS.
**When to use:** Any internet-facing VPS.
**Example:**
```bash
# CRITICAL: Allow SSH first to avoid lockout
sudo ufw allow 22/tcp
sudo ufw allow 80/tcp
sudo ufw allow 443/tcp
sudo ufw default deny incoming
sudo ufw default allow outgoing
sudo ufw enable
sudo ufw status verbose
```

### Anti-Patterns to Avoid
- **Using `ports` instead of `expose`:** Exposes containers directly to internet, bypassing Caddy. Only Caddy should have `ports`.
- **Cloudflare orange cloud (proxy mode):** Interferes with Caddy's ACME HTTP-01 challenges. Must use DNS-only (grey cloud).
- **Not persisting caddy_data volume:** Caddy re-requests certs on every restart, hitting Let's Encrypt rate limits (50 certs/domain/week).
- **Enabling UFW before allowing SSH:** Locks you out of VPS. Always `ufw allow 22/tcp` first.
- **Forgetting extra_hosts for host service:** Caddy in Docker cannot reach host's port 8080 without `host.docker.internal:host-gateway`.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| TLS certificate management | Certbot cron + Nginx reload | Caddy automatic ACME | Caddy handles obtain, renew, OCSP stapling, HTTP->HTTPS redirect automatically |
| Container health monitoring | Custom script polling containers | Docker Compose healthcheck | Built-in, triggers restart on failure, visible in `docker ps` |
| Firewall rules | Raw iptables commands | UFW | Human-readable, persistent across reboots, standard Ubuntu tool |
| Process supervision | systemd unit files for each app | Docker Compose restart policy | `restart: unless-stopped` handles crashes automatically |

## Common Pitfalls

### Pitfall 1: Let's Encrypt Rate Limits
**What goes wrong:** Requesting too many certificates during testing/iteration hits the 50 certs/domain/week limit.
**Why it happens:** Each `docker compose up` with a fresh caddy_data volume triggers new cert requests.
**How to avoid:** (1) Use named `caddy_data` volume that persists. (2) During testing, use Let's Encrypt staging endpoint: add `acme_ca https://acme-staging-v02.api.letsencrypt.org/directory` to Caddyfile global options. (3) Remove staging config once confirmed working.
**Warning signs:** Caddy logs showing "too many certificates already issued" or ACME errors.

### Pitfall 2: Port 80 Blocked
**What goes wrong:** ACME HTTP-01 challenge fails because port 80 is not reachable.
**Why it happens:** Firewall blocks port 80, or Cloudflare proxy intercepts the challenge.
**How to avoid:** (1) UFW must allow port 80. (2) Cloudflare must be DNS-only (grey cloud). (3) Verify with `curl -v http://app.racingpoint.cloud` from outside.
**Warning signs:** Caddy logs with "ACME challenge failed" or "connection refused on port 80".

### Pitfall 3: Alpine Containers Missing curl
**What goes wrong:** Healthcheck `curl -f http://localhost:PORT/` fails because Alpine base image does not include curl.
**Why it happens:** node:22-alpine is minimal -- includes wget but not curl.
**How to avoid:** Use `wget --no-verbose --tries=1 --spider http://localhost:PORT/` for Alpine containers. Or add `RUN apk add --no-cache curl` to Dockerfiles.
**Warning signs:** Container marked as unhealthy immediately after start, healthcheck exit code 127 (command not found).

### Pitfall 4: Docker Compose Memory Limits Syntax
**What goes wrong:** `mem_limit` (v2 syntax) silently ignored or deprecated warnings.
**Why it happens:** Docker Compose v3+ uses `deploy.resources.limits.memory` syntax.
**How to avoid:** Use the `deploy` key with nested `resources.limits.memory`. This works in Docker Compose v2 CLI (the tool) with Compose Specification format.
**Warning signs:** Container using more memory than expected, no OOM kills when expected.

### Pitfall 5: NEXT_PUBLIC_API_URL Pointing to Wrong Subdomain
**What goes wrong:** PWA sends API requests to `app.racingpoint.cloud/api/v1` instead of `api.racingpoint.cloud/api/v1`.
**Why it happens:** NEXT_PUBLIC_API_URL is baked at build time. If set incorrectly in compose.yml build args, it cannot be changed at runtime.
**How to avoid:** Set `NEXT_PUBLIC_API_URL: https://api.racingpoint.cloud` as a build arg for all frontends. All frontends should point to the same API subdomain.
**Warning signs:** CORS errors, 404s on API routes, network tab showing wrong origin.

### Pitfall 6: Swap Not Persisted
**What goes wrong:** Swap disappears after VPS reboot, containers OOM-kill under pressure.
**Why it happens:** `swapon` is session-only; need `/etc/fstab` entry.
**How to avoid:** Add `/swapfile none swap sw 0 0` to /etc/fstab after creating swap.
**Warning signs:** `free -h` shows 0 swap after reboot.

## Code Examples

### Complete Caddyfile
```caddyfile
# Source: Caddy official docs + security header best practices
{
    # Uncomment next line during initial testing to avoid Let's Encrypt rate limits:
    # acme_ca https://acme-staging-v02.api.letsencrypt.org/directory
}

(security_headers) {
    header {
        Strict-Transport-Security "max-age=31536000; includeSubDomains; preload"
        X-Content-Type-Options "nosniff"
        X-Frame-Options "DENY"
        Referrer-Policy "strict-origin-when-cross-origin"
        Permissions-Policy "interest-cohort=()"
        -Server
    }
}

app.racingpoint.cloud {
    import security_headers
    reverse_proxy pwa:3100
}

admin.racingpoint.cloud {
    import security_headers
    reverse_proxy admin:3300
}

dashboard.racingpoint.cloud {
    import security_headers
    reverse_proxy dashboard:3200
}

api.racingpoint.cloud {
    import security_headers
    reverse_proxy host.docker.internal:8080
}
```

### Complete compose.yml
```yaml
# Source: Docker Compose Specification + project CONTEXT.md decisions
services:
  caddy:
    image: caddy:2-alpine
    restart: unless-stopped
    extra_hosts:
      - "host.docker.internal:host-gateway"
    ports:
      - "80:80"
      - "443:443"
      - "443:443/udp"
    volumes:
      - ./Caddyfile:/etc/caddy/Caddyfile:ro
      - caddy_data:/data
      - caddy_config:/config
    deploy:
      resources:
        limits:
          memory: 128M
    healthcheck:
      test: ["CMD", "caddy", "version"]
      interval: 30s
      timeout: 5s
      retries: 3
    depends_on:
      pwa:
        condition: service_healthy
      admin:
        condition: service_healthy
      dashboard:
        condition: service_healthy

  pwa:
    build:
      context: ./racecontrol/pwa
      args:
        NEXT_PUBLIC_API_URL: https://api.racingpoint.cloud
    restart: unless-stopped
    expose:
      - "3100"
    deploy:
      resources:
        limits:
          memory: 512M
    healthcheck:
      test: ["CMD", "wget", "--no-verbose", "--tries=1", "--spider", "http://localhost:3100/"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 40s

  admin:
    build:
      context: ./racingpoint-admin
      args:
        NEXT_PUBLIC_API_URL: https://api.racingpoint.cloud
    restart: unless-stopped
    expose:
      - "3300"
    deploy:
      resources:
        limits:
          memory: 512M
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:3300/"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 40s

  dashboard:
    build:
      context: ./racecontrol/web
      args:
        NEXT_PUBLIC_API_URL: https://api.racingpoint.cloud
    restart: unless-stopped
    expose:
      - "3200"
    deploy:
      resources:
        limits:
          memory: 512M
    healthcheck:
      test: ["CMD", "wget", "--no-verbose", "--tries=1", "--spider", "http://localhost:3200/"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 40s

volumes:
  caddy_data:
  caddy_config:
```

### UFW Firewall Setup
```bash
# Run on VPS (Bono executes)
sudo ufw allow 22/tcp        # SSH - MUST be first
sudo ufw allow 80/tcp         # HTTP (ACME challenges + redirect)
sudo ufw allow 443/tcp        # HTTPS
sudo ufw default deny incoming
sudo ufw default allow outgoing
sudo ufw enable
sudo ufw status verbose
```

### Swap Setup
```bash
# Run on VPS (Bono executes)
sudo fallocate -l 2G /swapfile
sudo chmod 600 /swapfile
sudo mkswap /swapfile
sudo swapon /swapfile
echo '/swapfile none swap sw 0 0' | sudo tee -a /etc/fstab
free -h  # Verify swap is active
```

### DNS Records (Cloudflare)
```
Type  Name        Content         TTL   Proxy
A     app         72.60.101.58    300   DNS only
A     admin       72.60.101.58    300   DNS only
A     dashboard   72.60.101.58    300   DNS only
A     api         72.60.101.58    300   DNS only
```

### Dashboard Dockerfile Port Fix
The existing `web/Dockerfile` exposes port 3000 but the convention is 3200. The Dockerfile needs `ENV PORT=3200` and `EXPOSE 3200` (currently 3000).

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Nginx + Certbot cron | Caddy auto-ACME | Caddy v2 (2020+) | Zero-config TLS, no cron jobs |
| `mem_limit` in compose | `deploy.resources.limits.memory` | Compose Spec v3+ | Unified syntax for Compose and Swarm |
| `docker-compose` (v1 binary) | `docker compose` (v2 plugin) | 2023 | v1 EOL, v2 is the standard |
| Manual iptables | UFW | Long-standing | Human-readable, persistent rules |

## Open Questions

1. **Is racingpoint.cloud already registered and DNS managed in Cloudflare?**
   - What we know: VPS IP is 72.60.101.58, domain is referenced throughout project docs
   - What's unclear: Whether domain is registered and Cloudflare nameservers are configured
   - Recommendation: Verify before Phase 1 execution. This is a blocker.

2. **Docker Engine version on VPS**
   - What we know: Bono manages the VPS, Docker is installed
   - What's unclear: Exact version; `deploy.resources.limits.memory` requires Docker Engine 19.03+
   - Recommendation: Run `docker --version` and `docker compose version` on VPS to confirm

3. **Watchtower for auto-updates**
   - What we know: Claude's discretion per CONTEXT.md
   - Recommendation: **Do NOT add Watchtower** for v1. Auto-pulling images is dangerous without CI/CD gating. Manual `docker compose pull && up -d` is safer. Revisit in Phase 8 (CI/CD).

4. **NEXT_PUBLIC_API_URL path structure**
   - What we know: Existing Dockerfiles accept it as build arg. Current value includes `/api/v1` suffix in STACK.md examples.
   - What's unclear: Whether frontends append `/api/v1` themselves or need it in the env var
   - Recommendation: Check PWA source for how NEXT_PUBLIC_API_URL is used (bare domain vs with path). Set consistently across all 3 frontends.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Manual verification (infrastructure phase -- no unit tests) |
| Config file | N/A |
| Quick run command | `curl -sI https://app.racingpoint.cloud` |
| Full suite command | See verification script below |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| INFRA-01 | DNS A records resolve to VPS | smoke | `dig +short app.racingpoint.cloud` returns 72.60.101.58 | N/A |
| INFRA-02 | Caddy routes subdomains with valid TLS | smoke | `curl -sI https://app.racingpoint.cloud \| grep -i strict-transport` | N/A |
| INFRA-03 | Containers running with memory limits | smoke | `docker compose ps` + `docker stats --no-stream` | N/A |
| INFRA-06 | Firewall blocks non-80/443 | smoke | `sudo ufw status verbose` | N/A |
| INFRA-07 | 2GB swap active | smoke | `free -h \| grep Swap` shows 2.0G | N/A |

### Sampling Rate
- **Per task:** Run relevant curl/dig commands after each change
- **Phase gate:** Full verification script (all 4 subdomains HTTPS + cert check + firewall audit + swap check)

### Verification Script
```bash
#!/bin/bash
# Run from any machine with internet access
echo "=== INFRA-01: DNS Resolution ==="
for sub in app admin dashboard api; do
  ip=$(dig +short $sub.racingpoint.cloud)
  echo "$sub.racingpoint.cloud -> $ip"
  [ "$ip" = "72.60.101.58" ] && echo "  PASS" || echo "  FAIL"
done

echo "=== INFRA-02: TLS Certificates ==="
for sub in app admin dashboard api; do
  cert=$(echo | openssl s_client -connect $sub.racingpoint.cloud:443 -servername $sub.racingpoint.cloud 2>/dev/null | openssl x509 -noout -issuer 2>/dev/null)
  echo "$sub.racingpoint.cloud: $cert"
  echo "$cert" | grep -qi "let's encrypt\|R3\|R10\|R11\|E5\|E6" && echo "  PASS" || echo "  FAIL"
done

echo "=== INFRA-02: Security Headers ==="
for sub in app admin dashboard api; do
  hsts=$(curl -sI https://$sub.racingpoint.cloud | grep -i strict-transport)
  echo "$sub.racingpoint.cloud: $hsts"
  [ -n "$hsts" ] && echo "  PASS" || echo "  FAIL"
done

echo "=== INFRA-03: Container Status ==="
# Run on VPS
# docker compose -f /opt/racingpoint/compose.yml ps
# docker stats --no-stream

echo "=== INFRA-06: Firewall ==="
# Run on VPS
# sudo ufw status verbose

echo "=== INFRA-07: Swap ==="
# Run on VPS
# free -h | grep Swap
```

### Wave 0 Gaps
- [ ] `web/Dockerfile` port needs changing from 3000 to 3200
- [ ] Verify racingpoint.cloud domain is registered and DNS is in Cloudflare
- [ ] Verify Docker Engine version on VPS supports deploy.resources.limits

## Sources

### Primary (HIGH confidence)
- [Caddy official docs - Automatic HTTPS](https://caddyserver.com/docs/automatic-https) - ACME behavior, staging endpoint
- [Caddy official docs - header directive](https://caddyserver.com/docs/caddyfile/directives/header) - Security header syntax
- [Docker Compose Deploy Specification](https://docs.docker.com/reference/compose-file/deploy/) - Memory limits syntax
- [Docker Hub - caddy](https://hub.docker.com/_/caddy) - Image version 2.11.x confirmed
- Existing Dockerfiles in repo (pwa/Dockerfile, web/Dockerfile) - Working multi-stage builds

### Secondary (MEDIUM confidence)
- [DigitalOcean UFW Guide](https://www.digitalocean.com/community/tutorials/how-to-set-up-a-firewall-with-ufw-on-ubuntu) - UFW setup commands verified
- [Docker Recipes - Caddy Reverse Proxy](https://docker.recipes/web-servers/caddy-reverse-proxy) - Compose patterns
- [Caddy Community Forum](https://caddy.community/t/setting-reverse-proxy-from-withing-docker-caddy-to-localhost-service/15369) - host.docker.internal pattern

### Tertiary (LOW confidence)
- None -- all findings verified against official docs

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - Caddy + Docker Compose is well-documented consensus for this exact use case
- Architecture: HIGH - All patterns verified against official docs, existing Dockerfiles confirm approach
- Pitfalls: HIGH - Common issues well-documented in community forums and official docs

**Research date:** 2026-03-22
**Valid until:** 2026-04-22 (stable infrastructure, slow-moving domain)
