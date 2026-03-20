---
phase: 77-transport-security
plan: 02
subsystem: infra
tags: [tls, https, security-headers, cors, tower-helmet, axum-server, csp, hsts]

# Dependency graph
requires:
  - phase: 77-01
    provides: tls.rs module with load_or_generate_rustls_config, ServerConfig tls_port/cert_path/key_path fields
provides:
  - Dual-port HTTP 8080 + HTTPS 8443 server startup
  - Security response headers (CSP, X-Frame-Options, X-Content-Type-Options, HSTS)
  - CORS predicate accepting HTTPS origins from LAN subnet
  - Protocol-aware kiosk API_BASE (no mixed content)
affects: [kiosk, deployment, pod-agents]

# Tech tracking
tech-stack:
  added: [tower-helmet 0.3 (security headers middleware)]
  patterns: [dual-port server with tokio::spawn for HTTPS, HelmetLayer::blank() with selective headers]

key-files:
  created: []
  modified:
    - crates/racecontrol/src/main.rs
    - kiosk/src/lib/api.ts

key-decisions:
  - "HelmetLayer::blank() instead of with_defaults() -- avoids COEP/COOP/upgrade-insecure-requests that would break kiosk proxy and CDN fonts"
  - "HSTS max-age=300 (5 min) for initial deploy safety -- increase after 1 week stable operation"
  - "CSP use_defaults: false with explicit directives only -- no block-all-mixed-content or upgrade-insecure-requests during testing"
  - "HTTPS uses .into_make_service() (no ConnectInfo) -- rate limiting via tower_governor only on HTTP port for now"
  - "racingpoint.cloud CORS changed from .contains() to exact == match -- security fix preventing subdomain spoofing"

patterns-established:
  - "Dual-port pattern: HTTPS listener spawned via tokio::spawn, HTTP listener blocks main thread"
  - "Security headers via tower-helmet blank + selective enable, not with_defaults"

requirements-completed: [TLS-01, TLS-03, TLS-04, KIOSK-06]

# Metrics
duration: 5min
completed: 2026-03-20
---

# Phase 77 Plan 02: Dual-Port HTTPS + Security Headers Summary

**Dual-port HTTP/HTTPS server with tower-helmet security headers (CSP, HSTS 300s, X-Frame-Options DENY), HTTPS CORS, and protocol-aware kiosk API_BASE**

## Performance

- **Duration:** 5 min
- **Started:** 2026-03-20T14:15:24Z
- **Completed:** 2026-03-20T14:20:49Z
- **Tasks:** 2 auto + 1 checkpoint (human-verify)
- **Files modified:** 2

## Accomplishments
- Dual-port server: HTTP 8080 (pod agents, unchanged) + HTTPS 8443 (customer WiFi browsers)
- Security response headers on all responses via tower-helmet: CSP, X-Frame-Options DENY, X-Content-Type-Options nosniff, HSTS max-age=300
- CORS predicate updated to accept https:// origins from 192.168.31.* subnet; racingpoint.cloud changed to exact match (security fix)
- Kiosk PWA API_BASE derives protocol and port from window.location -- no mixed content on HTTPS

## Task Commits

Each task was committed atomically:

1. **Task 1: Wire HTTPS listener, security headers, and CORS update in main.rs** - `cde40a7` (feat)
2. **Task 2: Fix kiosk API_BASE for HTTPS and coordinate TLS-03 with Bono** - `e165e7b` (feat)
3. **Task 3: Verify HTTPS listener and security headers** - checkpoint:human-verify (programmatic verification by orchestrator)

## Files Created/Modified
- `crates/racecontrol/src/main.rs` - Dual-port startup, security_headers_layer(), CORS HTTPS origins, racingpoint.cloud exact match
- `kiosk/src/lib/api.ts` - Protocol-aware API_BASE using window.location.protocol + host

## Decisions Made
- Used HelmetLayer::blank() with selective .enable() instead of with_defaults() -- the defaults include CrossOriginEmbedderPolicy (require-corp), CrossOriginOpenerPolicy, upgrade-insecure-requests in CSP, which would break the kiosk reverse proxy (fonts from CDN) and mixed HTTP/HTTPS testing
- HSTS max-age set to 300 seconds (5 minutes) per research pitfall guidance -- prevents browser lockout during testing phase
- CSP set with use_defaults: false to avoid block-all-mixed-content and upgrade-insecure-requests directives that would interfere with the HTTP-to-HTTPS migration
- HTTPS listener uses .into_make_service() without ConnectInfo -- tower_governor rate limiting (PeerIpKeyExtractor) only works on HTTP port; acceptable since HTTPS consumers are customer browsers, not rate-limited staff routes
- racingpoint.cloud CORS check changed from .contains() to exact == "https://app.racingpoint.cloud" to prevent subdomain spoofing (e.g., evil-racingpoint.cloud.attacker.com)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] HelmetLayer::with_defaults() replaced with blank() + selective enable**
- **Found during:** Task 1 (security headers layer)
- **Issue:** Plan suggested HelmetLayer::with_defaults().enable(csp) but with_defaults() includes COEP require-corp, COOP same-origin, upgrade-insecure-requests which would break kiosk proxy and CDN font loading
- **Fix:** Used HelmetLayer::blank() with only the 4 required headers (CSP, XFrameOptions, XContentTypeOptions, HSTS)
- **Files modified:** crates/racecontrol/src/main.rs
- **Verification:** cargo build --release succeeds
- **Committed in:** cde40a7

---

**Total deviations:** 1 auto-fixed (1 bug prevention)
**Impact on plan:** Necessary to avoid breaking existing kiosk proxy functionality. No scope creep.

## Issues Encountered
None

## User Setup Required
To enable HTTPS on the live server:
1. Add `tls_port = 8443` to `[server]` section of `C:\RacingPoint\racecontrol.toml`
2. Restart racecontrol service
3. Self-signed cert auto-generated on first HTTPS request to `C:\RacingPoint\tls\{cert,key}.pem`

## Next Phase Readiness
- Transport security foundation complete (TLS cert gen + dual-port + headers)
- Ready for production deploy after human verification of HTTPS listener
- Bono notified for TLS-03 Let's Encrypt verification on cloud VPS

---
*Phase: 77-transport-security*
*Completed: 2026-03-20*
