# Gateway Contract A1+A2+A5-stub — implementation notes

**Branch:** `feat/admin-gateway-contract-a1-a5-20260423` (in `bono-bot/racingpoint-admin`)
**Date:** 2026-04-23
**Author:** James
**Status:** committed locally, not pushed yet, not deployed

## Files changed

| Path | Change | Purpose |
|---|---|---|
| `src/app/api/rc/[...path]/route.ts` | rewrote | Universal proxy v1 — all 7 methods, multi-auth precedence, header passthrough, request-id, structured log, A5 rate-limit gate |
| `src/lib/admin-gateway-state.ts` | NEW | In-memory metrics + health snapshot + rate-limit bucket |
| `src/app/api/rc/__health/route.ts` | NEW | GET + HEAD health endpoint, probes upstream RC `/api/v1/health` |
| `src/app/api/rc/__metrics/route.ts` | NEW | GET Prometheus text format |
| `racecontrol/.planning/specs/admin-panel-spinal-cord/GATEWAY-CONTRACT.md` | edited | Reflect IMPLEMENTED status, A4 EXEMPT, A3 COLLAPSED |

## What this enables

- Track B (Kiosk + WhatsApp REST migration) now has a target proxy that supports their auth schemes (`x-terminal-secret` for WhatsApp; admin-cookie or staff-jwt for Kiosk staff actions)
- Track C (rc-agent polling migration) now has a target proxy that passes through `X-Service-Key` for service-key-authed routes
- Reliability probe (P1-5 in plan_admin_panel_spinal_cord_gap) now has `/api/rc/__health` to consume

## What is NOT in this change

- **Customer-JWT was added to the contract but the proxy treats it the same as staff-JWT** (both are `Authorization: Bearer ...`). Differentiation happens upstream in racecontrol — admin doesn't need to parse the JWT. Documented for clarity, no code branch needed.
- **No WebSocket relay** (A4 exempt — see GATEWAY-CONTRACT.md §5).
- **Reliability probe itself is NOT implemented** — only the `__health` endpoint it would consume. Probe is P1-5 in the spinal-cord-gap plan, separate workstream.
- **No deploy script changes** — once shipped, deploy via existing admin deploy path. PM2 reload picks up the new routes; no schema migration.
- **No upstream racecontrol code changes** — this is admin-side only. Racecontrol still owns auth enforcement; admin just forwards headers without parsing.
- **No cloud-admin specific work** — A3 collapsed because it's the same repo. When this branch deploys to cloud admin, the proxy upgrade lands there too. The 22-stub-route problem is a separate deploy/config concern unaffected by this PR.

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

## Test plan (NOT YET EXECUTED)

H3 evidence required before marking A1 "verified":

| Test | Surface | Expectation |
|---|---|---|
| `curl /api/rc/health` (unauth) | bash | passes through to RC `/api/v1/health` (which is public) — 200 |
| `curl -H "Cookie: rp-admin-token=..." /api/rc/billing/active` | bash | substitutes Bearer, 200 with active sessions |
| `curl -H "X-Service-Key: <key>" /api/rc/pods/mesh-service-key` | bash | passes header through, expected 200 |
| `curl -H "x-terminal-secret: <secret>" /api/rc/bot/pods-status` | bash | passes header through, expected 200 |
| `curl -X PATCH ...` | bash | proxy accepts PATCH (previously rejected) |
| `curl -X HEAD /api/rc/health` | bash | proxy accepts HEAD |
| `curl /api/rc/__health` | bash | returns proxy + upstream state JSON |
| `curl /api/rc/__metrics` | bash | returns Prometheus text |
| `curl -X POST /api/rc/billing/start ... -d <large-multipart>` | bash | binary body forwards intact |
| Browser flow: log into admin, navigate /billing, /fleet, /customers | chrome-devtools | existing pages still render — backward compat |
| Repeat under `ADMIN_GATEWAY_RATE_LIMIT=1` with hammering | bash | 429 response shape correct |

All of the above need to run from James .27 against a fresh local `npm run dev` first, then against staging deploy on Server .23 before marking A1 deployed. **Currently NOT TESTED — code-complete only.**

## Deploy plan (separate from this branch)

When shipping:
1. PR review — human gate
2. Merge to main
3. Build admin (`npm run build`)
4. Deploy to venue Server .23 (existing deploy path) — `:3201` PM2 reload
5. Smoke: `curl http://192.168.31.23:3201/api/rc/__health` from James → expect 200 + healthy:true
6. Run the test plan above in production
7. Deploy to cloud admin (`bono-bot` Bono VPS) — same build, env-switched
8. Smoke: `curl https://admin.racingpoint.cloud/api/rc/__health` → expect same shape

Permanence Gate: code is in git. PM2 reload picks it up. No manual server edits, no temporary fixes.

## What unblocks next

- **Track B (Kiosk + WhatsApp REST migration)** can begin. Both surfaces need to swap their `RACECONTROL_URL` env var to `http://<admin>:3201/api/rc` (with path stripping).
- **Track C (rc-agent polling)** waits for one week of A2 metrics post-deploy to establish baseline volume before approving migration.
- **Track D (Discord)** still waits on source code access (Bono VPS or local mirror sync).
