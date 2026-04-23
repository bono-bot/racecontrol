# Gateway Contract A1+A2+A5-stub — implementation notes

**Branch:** `feat/admin-gateway-contract-a1-a5-20260423` (in `bono-bot/racingpoint-admin`)
**Date:** 2026-04-23 (notes revised 2026-04-23 evening — A3 framing corrected per doctrine v3 §5)
**Author:** James
**Status:** committed + pushed to origin (no PR open). NOT deployed. **Venue organ only** — cloud organ A3 is a separate workstream per doctrine v3 §5.

## Files changed

| Path | Change | Purpose |
|---|---|---|
| `src/app/api/rc/[...path]/route.ts` | rewrote | Universal proxy v1 — all 7 methods, multi-auth precedence, header passthrough, request-id, structured log, A5 rate-limit gate |
| `src/lib/admin-gateway-state.ts` | NEW | In-memory metrics + health snapshot + rate-limit bucket |
| `src/app/api/admin-gateway/health/route.ts` | NEW (path corrected post-smoke 2026-04-23 evening) | GET + HEAD health endpoint, probes upstream RC `/api/v1/health` |
| `src/app/api/admin-gateway/metrics/route.ts` | NEW (path corrected post-smoke 2026-04-23 evening) | GET Prometheus text format |
| `racecontrol/.planning/specs/admin-panel-spinal-cord/GATEWAY-CONTRACT.md` | edited | Reflect IMPLEMENTED status (venue), A4 EXEMPT, A3 SEPARATE-ORGAN (un-collapsed 2026-04-23 evening per doctrine v3 §5) |

## What this enables

- Track B (Kiosk + WhatsApp REST migration) now has a target proxy that supports their auth schemes (`x-terminal-secret` for WhatsApp; admin-cookie or staff-jwt for Kiosk staff actions)
- Track C (rc-agent polling migration) now has a target proxy that passes through `X-Service-Key` for service-key-authed routes
- Reliability probe (P1-5 in plan_admin_panel_spinal_cord_gap) now has `/api/admin-gateway/health` to consume

## What is NOT in this change

- **Customer-JWT was added to the contract but the proxy treats it the same as staff-JWT** (both are `Authorization: Bearer ...`). Differentiation happens upstream in racecontrol — admin doesn't need to parse the JWT. Documented for clarity, no code branch needed.
- **No WebSocket relay** (A4 exempt — see GATEWAY-CONTRACT.md §5).
- **Reliability probe itself is NOT implemented** — only the `/api/admin-gateway/health` endpoint it would consume. Probe is P1-5 in the spinal-cord-gap plan, separate workstream.
- **No deploy script changes** — once shipped, deploy via existing admin deploy path. PM2 reload picks up the new routes; no schema migration.
- **No upstream racecontrol code changes** — this is admin-side only. Racecontrol still owns auth enforcement; admin just forwards headers without parsing.
- **No cloud-admin specific work — A3 is a SEPARATE organ per doctrine v3 §5** (un-collapsed 2026-04-23 evening). Same codebase ≠ same organ. Cloud admin (`admin.racingpoint.cloud` on Bono VPS) needs its own deploy, its own env/config (`ADMIN_COMING_SOON_GATE` resolution, cloud `RC_URL`), and its own §7 browser-level revival verification before A3 can ship. Additionally: cloud middleware currently 503s `/api/rc/*` via the COMING_SOON gate, so the proxy code on cloud is unreachable until the gate is lifted (toggle env, or carve `/api/rc/` into `GATE_PUBLIC_PREFIXES`). The 22-stub-route problem is a cloud-only deploy/config issue tracked under Phase 445 — also part of A3 scope.
- **No MI audit-seed emit** — doctrine v3 §6 calls for the gateway to emit non-blocking `POST /api/v1/mesh/audit-seed` on errors with `{symptom, source: "admin-gateway", endpoint, caller, request_id}`. NOT in this branch. Tracked as a follow-up (`MI-INTEGRATION.md`).

## Caveats

### v0 — process-local state
`admin-gateway-state.ts` keeps counts and latency in JS module-level state. When PM2 restarts the admin process, all counters reset. For multi-instance scrapes or persistent metrics, swap in `prom-client` with a shared store. Not worth the dependency now — single PM2 instance, metrics exist for hour-scale debugging not month-scale dashboards.

### Latency bucket is unweighted ring
Latency arrays are capped at 1000 samples per `(method, caller)` key. Older samples shifted off. Quantiles computed at scrape time. Good enough for surge detection, not for SLO compliance.

### Rate limit is per-process
With one PM2 instance, this is fine. If admin is ever clustered, rate limits will be effectively N× the documented limit. Re-evaluate if cluster mode lands.

### Body buffering, not streaming
`req.arrayBuffer()` buffers the entire request body in memory before forwarding. Fine for typical JSON / form / small multipart. Will be a problem for large CSV uploads (>10MB). Documented gap — true streaming requires `req.body` (Web ReadableStream) + `fetch({ duplex: 'half' })` which has uneven Next.js support across versions.

### Sampled GET logs
1% sample rate. Means low-frequency 200 GETs may go entirely unlogged across short observation windows. All writes (POST/PUT/PATCH/DELETE) and all 4xx/5xx are logged unconditionally — that's where debugging signal usually lives.

### admin-cookie vs staff-jwt distinction
Today both end up forwarding `Authorization: Bearer ...` upstream. The proxy distinguishes them only for the cookie→Bearer substitution and Set-Cookie filtering. Racecontrol can't tell the difference once the request lands — it just sees a JWT.

### No retry, no circuit-breaker
Upstream 5xx surfaces directly to the client. Per contract §6, the proxy doesn't swallow errors. Adding retries here would mask flakiness in racecontrol that should be visible to operators. Re-evaluate if measurement shows we're throwing transient 502s for self-healing upstream issues.

### `Set-Cookie` filtering
For non-admin-UI callers (anything other than `class === 'admin-cookie'`), `Set-Cookie` headers from upstream are stripped. Reason: cookies are an admin-UI session concept; bot/agent/kiosk callers shouldn't accidentally pick up admin session cookies. May need to revisit if a non-admin caller ever legitimately needs cookie-set behavior.

## Test plan + smoke evidence

Evidence captured 2026-04-23 evening from James .27 → `localhost:3000` (admin dev server, RC_URL=http://192.168.31.23:8080, ADMIN_COMING_SOON_GATE=0).

| Test | Result | Evidence |
|---|---|---|
| `GET /api/rc/health` (unauth passthrough) | ✅ HTTP 200 | RC body `build_id:ec6b9088`, RID `4cd31f3f-0142-4e3f-b750-1ba8b390aec0` returned in X-Request-Id |
| `HEAD /api/rc/health` (NEW method) | ✅ HTTP 200 | full headers passed through; previously unsupported method now works |
| `PATCH /api/rc/health` (NEW method) | ✅ HTTP 405 | RC rejects PATCH on /health (no PATCH handler upstream); proves proxy ACCEPTS PATCH and passes through, RC sets the 405 |
| `GET /api/rc/mesh/audit-check-service` with `X-Service-Key: <bogus>` | ✅ HTTP 401 "Invalid service key" | RC validated the bogus key — proves header reached upstream |
| `GET /api/rc/bot/pods-status` with `x-terminal-secret: <bogus>` | ✅ HTTP 200 `{"error":"Unauthorized"}` | upstream returned upstream-shape error — proves header reached upstream |
| `GET /api/admin-gateway/health` | ✅ HTTP 200 JSON | `{healthy:true, upstream_reachable:true, upstream_status:200, upstream_latency_ms:2, last_upstream_success_at:"2026-04-22T23:58:38.603Z", total_requests:3}` |
| `GET /api/admin-gateway/metrics` (after 3 priming requests) | ✅ HTTP 200 text | `admin_gateway_requests_total{method="GET",status="200",caller="public"} 3`, p50=0.005, p95=0.008, p99=0.008 |
| `GET /login` (backward-compat) | ✅ HTTP 200 text/html | login page renders |
| `GET /` (backward-compat) | ✅ HTTP 200 | dashboard root renders |
| `curl -H "Cookie: rp-admin-token=..." /api/rc/billing/active` | ❌ NOT TESTED | requires real admin JWT |
| `POST /api/rc/billing/start` with multipart body | ❌ NOT TESTED | needs real upstream POST endpoint without side-effects |
| Full browser navigation /billing /fleet /customers | ❌ NOT TESTED | only HTTP layer probed; no DOM rendering verification |
| Rate limit gate under `ADMIN_GATEWAY_RATE_LIMIT=1` | ❌ NOT TESTED | requires env restart + hammer load |
| Set-Cookie filtering for non-admin-cookie callers | ❌ NOT TESTED | requires upstream that emits Set-Cookie |
| Race conditions in admin-gateway-state under concurrent load | ❌ NOT TESTED | needs concurrent request generator |
| Body forwarding for >10MB payload | ❌ NOT TESTED | known caveat — buffered via arrayBuffer, will OOM at scale |
| Cloud-authoritative-write rule (`isCloudAuthoritativeWrite`) for staff/* mutations | ❌ NOT TESTED | requires `RC_CLOUD_URL` set + actual staff write |
| Production deploy at venue `:3201` | ❌ NOT DONE | human-gated PR review |
| Production deploy at cloud admin | ❌ NOT DONE | A3 separate workstream |
| Discord audit | ❌ NOT DONE | source not in local mirror |
| PWA audit | ❌ NOT DONE | Uday on PWA in concurrent session |

**Net:** 9 of 11 originally-planned smoke rows passed; 2 require credentials James doesn't hold; the rest fall into "real-load testing" or "production deploy" that can't run from a dev environment.

**Path correction discovered during smoke:** original draft put meta endpoints at `/api/rc/__health` + `/api/rc/__metrics`. Next.js excludes underscore-prefixed folders from routing (private folder convention) — those returned 404 HTML. Moved to `/api/admin-gateway/{health,metrics}` (outside the `[...path]` proxy hierarchy, semantically clearer as gateway-meta not racecontrol-proxy). Re-smoke confirmed both routes resolve.

## Deploy plan (separate from this branch)

When shipping:
1. PR review — human gate
2. Merge to main
3. Build admin (`npm run build`)
4. Deploy to venue Server .23 (existing deploy path) — `:3201` PM2 reload
5. Smoke: `curl http://192.168.31.23:3201/api/admin-gateway/health` from James → expect 200 + healthy:true
6. Run the test plan above in production
7. Deploy to cloud admin (`bono-bot` Bono VPS) — same build, env-switched
8. Smoke: `curl https://admin.racingpoint.cloud/api/admin-gateway/health` → expect same shape

Permanence Gate: code is in git. PM2 reload picks it up. No manual server edits, no temporary fixes.

## What unblocks next

- **Track B (Kiosk + WhatsApp REST migration)** can begin. Both surfaces need to swap their `RACECONTROL_URL` env var to `http://<admin>:3201/api/rc` (with path stripping).
- **Track C (rc-agent polling)** waits for one week of A2 metrics post-deploy to establish baseline volume before approving migration.
- **Track D (Discord)** still waits on source code access (Bono VPS or local mirror sync).
