# Admin Gateway Contract — v1

**Status:** ratified (PACT-20260424-001/002/003/004 PROCEED per comms-link/PACTS.md).
**Referenced by:** `racingpoint-admin/src/app/api/rc/[...path]/route.ts:13`.
**Origin doctrine:** Uday 2026-04-23, "admin panel is the bidirectional spinal cord."

## 1. Role

Admin is the bidirectional spinal cord between Racing Point clients and the
RaceControl backend:

- **Read path:** PWA / kiosk / POS / web → Admin → RC
- **Write path:** Operator → Admin → RC (applies to client-originated writes
  via Admin proxy too)
- **Source-of-truth:** RC holds state; Admin is the conduit, not a store.

Clients do not call RC directly in the normal flow. Direct-to-RC paths are
exceptions, not defaults — enumerated explicitly in §4.

## 2. URL shape

| Hop | Path |
|---|---|
| Client (browser) | `/api/customer/<path>` (PWA) or admin-internal relative `/api/rc/<path>` |
| Admin gateway | `/api/rc/<path>` |
| RaceControl | `/api/v1/<path>` |

Admin rewrites `/api/rc/*` → `${RC_URL}/api/v1/*` at the proxy route handler
(`racingpoint-admin/src/app/api/rc/[...path]/route.ts`). PWA additionally
rewrites `/api/customer/*` → `${ADMIN_GATEWAY_URL}/api/rc/*` at Next.js
server (`pwa/next.config.ts:rewrites`).

Three-hop chain for PWA: browser → PWA Next.js server → Admin → RC.

## 3. Auth classes (admin-gateway-state.ts `CallerClass`)

| Class | Header detected | substituteAuth | Notes |
|---|---|---|---|
| `service-key` | `X-Service-Key` | no | Server-to-server; identity-key = first 16 chars |
| `terminal-secret` | `x-terminal-secret` | no | POS / cloud-venue sync |
| `staff-jwt` | `Authorization: Bearer` | no | Operator console |
| `kiosk-pin` | `x-kiosk-pin` | no | Pod kiosk staff PIN |
| `admin-cookie` | `rp_admin_session` cookie | **yes** — substitutes Bearer | Admin UI |
| `customer-jwt` | passed as Authorization Bearer by PWA | no | Customer session |
| `public` | none | no | Anonymous endpoints (leaderboard, cafe menu, public driver records) |

Cookies pass through only for `admin-cookie` callers. Other classes strip
cookies from response (see `STRIP_HEADERS_RES` + cookie logic in route.ts).

## 4. Cloud-authoritative writes

POST/PUT/PATCH/DELETE to:
- `staff/*` (except `staff/validate-pin`)
- `admin/staff`

route to `RC_CLOUD_URL` when set. Other methods and paths route to `RC_URL`.

Rationale: staff account mutations are centrally-authoritative — venue RC
may be offline during ops, cloud RC is the persistence tier for these.

## 5. A4 — WebSocket relay exemption (permanent)

WebSocket connections are NOT proxied through admin. Kiosk, PWA, and
pod-edge agents open WebSockets directly to RC on port 8080:

- Kiosk: `ws://<host>:8080/ws/dashboard`
- PWA: (future telemetry channels if added)
- rc-agent: `ws://192.168.31.23:8080/ws/agent`

Rationale: latency budget for racing telemetry + streaming bidirectional
data does not tolerate a Next.js Node.js hop. Admin stays HTTP/REST.
WebSocket auth is handled by RC's own auth middleware on the ws upgrade.

## 6. A4-extended — pod-edge infrastructure exemption (PACT-20260424-003)

The following pod-edge services are **infrastructure agents, not clients**,
and are permanently exempt from spinal-cord routing. They call RC directly
on the LAN:

- `rc-agent` (pod process): HTTP `/api/v1/*` + WS `/ws/agent`
- `rc-sentry` (pod fallback watchdog): HTTP `/api/v1/recovery/events`,
  pod health probes
- `rc-watchdog` (pod survival reporter): HTTP `/api/v1/pods/<pod>/watchdog-*`
- `rc-ops-mcp` (James-local MCP tooling): HTTP `/api/v1/*` — operator
  tooling, not customer/operator surface

Rationale:
1. These services exist for latency-critical pod↔server signalling
   (billing, fleet broadcast, crash reporting, predictive maintenance).
2. Admin is cloud-deployed; routing through admin = cloud-single-point-of-
   failure for fleet.
3. They authenticate via `X-Service-Key` or `x-terminal-secret` — the same
   auth classes admin already classifies; re-routing does not add identity.
4. "Client" per spinal-cord doctrine = PWA / kiosk / POS / customer-facing
   apps. rc-agent and kin are backend plumbing.

Migration of any of these to spinal-cord routing requires a new PACT vote
with latency-budget evidence and failure-mode analysis.

## 7. Response-header contract

Stripped before passthrough (see `STRIP_HEADERS_RES` in route.ts):
- `transfer-encoding`
- `connection`
- `content-encoding` (Node fetch auto-decompresses; double-decode breaks
  browser — caught in browser-DOM smoke 2026-04-23)
- `content-length`

Always added:
- `X-Request-Id` — generated if client didn't supply; propagated to RC.

## 8. Rate limit (A5 stub)

`checkRateLimit(identityKey, class)` gate implemented but disabled unless
`ADMIN_GATEWAY_RATE_LIMIT=1` env set. Returns 429 with `Retry-After` when
tripped. Per-class limits live in `admin-gateway-state.ts`.

## 9. Observability

- Structured stdout log per request — 100% of writes and errors, 1% sample
  on GETs (`SAMPLE_GET_RATE = 0.01`).
- MI signal emission via `recordMiSymptom({...})` on:
  - RC unreachable (`admin_gateway_upstream_unreachable`, P1)
  - RC 5xx responses (`admin_gateway_upstream_5xx_<status>`, P2)
  - Rate-limit trips (`admin_gateway_rate_limited_<class>`, P3)

## 10. Client-side compliance (admin repo)

`racingpoint-admin/scripts/check-spinal-cord.mjs` prebuild guard blocks
admin-internal bypass patterns:
- `NEXT_PUBLIC_API_URL` anywhere in `src/`
- Hardcoded `192.168.31.23:8080`
- Hardcoded `localhost:8080` or `:7777`

Exempts enumerated with per-line WHY in the guard. See
`racingpoint-admin/scripts/check-spinal-cord.mjs` EXEMPT list.

## 11. Client adoption status (2026-04-24)

| Client | Route | Status |
|---|---|---|
| Admin UI pages | direct `/api/rc/*` | landed (Wave A + prior) |
| PWA | `/api/customer/*` rewrite | **staged** on `feat/pwa-spinal-cord-b001-20260424` (PACT-20260424-001 PROCEED); not yet deployed — requires next venue rebuild + env flip |
| web | `/api/customer/*` (reuse pattern) | **deferred** (PACT-20260424-002 PROCEED-sequence-after-001-canary) |
| kiosk HTTP | `/api/customer/*` | deferred (same as web; WS stays A4-exempt) |
| POS Chrome kiosk | inherits kiosk bundle | deferred |
| Cloud PWA / dashboard | repoint cloud compose.yml | **deferred** (PACT-20260424-004 PROCEED-defer-until-001-proves) |
| whatsapp-bot (Bono VPS) | `ADMIN_GATEWAY_URL/api/rc/*` | Bono-domain (HANDOFF-20260424-005) |
| rc-agent / rc-sentry / rc-watchdog / rc-ops-mcp | direct | **permanent exemption** (§6 above, PACT-20260424-003 PROCEED) |

## 12. Operational "admin IS spinal cord" readiness criteria

Claim becomes operationally true when ALL of:

1. PACT-001 code deployed at venue (PWA rebuild on Server .23 with
   `NEXT_PUBLIC_API_URL=/api/customer` + `ADMIN_GATEWAY_URL=http://192.168.31.23:3201`).
2. PACT-001 canary: 48h stable traffic, admin gateway logs show PWA
   customer-class traffic > 90% of PWA API volume.
3. PACT-002 web/kiosk rebuild landed same pattern.
4. PACT-004 cloud compose rebuilt with same envs on Bono VPS.
5. HANDOFF-005 whatsapp-bot migrated (Bono-side).
6. Traffic measurement from admin gateway log: aggregate client-class
   traffic (non-infra) routes ≥95% through admin, with rc-agent-class
   infrastructure the only sub-95% tail.

Until then: "admin is the intended spinal cord, with doctrine ratified,
admin-internal compliance landed, and client code staged. Client traffic
migration in progress."

---

_Sync sources: `racingpoint-admin/src/app/api/rc/[...path]/route.ts`,
`racingpoint-admin/scripts/check-spinal-cord.mjs`,
`comms-link/PACTS.md` (20260424-001..006 + 008),
`~/.claude/projects/C--Users-bono/memory/feedback_admin_portal_source_of_truth.md`,
`~/.claude/projects/C--Users-bono/memory/plan_pwa_through_admin_spinal_cord_20260423.md`._
