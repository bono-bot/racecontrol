# Admin Panel Gateway Contract — v1 (A1+A2+A5-stub IMPLEMENTED on venue, A3 SEPARATE for cloud, A4 EXEMPT)

**Status:** IMPLEMENTED in branch `feat/admin-gateway-contract-a1-a5-20260423` for the **venue** organ. Cloud admin (A3) is a SEPARATE organ that must implement the same contract but is independently deployed and tested. See `A1-IMPLEMENTATION-NOTES.md` for what landed and caveats.
**Date:** 2026-04-23 (revised 2026-04-23 evening — A3 un-collapsed per doctrine v3 §5)
**Companion docs:**
- Doctrine v3 (canonical): `~/.claude/projects/C--Users-bono/memory/project_admin_panel_operator_model.md` §5 (two-skeletons exoskeleton model — A3 is its own organ, NOT a same-repo collapse)
- Two-spine sync: `TWO-SPINE-SYNC.md` (spines do not talk directly; sync via existing brain-to-brain Phase 301)
- MI integration: `MI-INTEGRATION.md` (gateway emits `/api/v1/mesh/audit-seed` on errors — runtime spine-to-brain channel)
- Audit findings: `~/.claude/projects/C--Users-bono/memory/plan_admin_panel_spinal_cord_gap_20260422.md`
- Current venue proxy (reference impl): `racingpoint-admin/src/app/api/rc/[...path]/route.ts`

---

## Purpose

Define the contract that every Admin Panel proxy (venue `:3201`, cloud `admin.racingpoint.cloud`) must implement so that ALL client surfaces (PWA, Kiosk, Billing, WhatsApp, Discord, rc-agent) can route through admin instead of calling racecontrol `:8080` directly.

**Without a contract, surface migrations break under proxy gaps.** The current venue proxy supports only one auth mode and drops most headers — it cannot serve the other surfaces as-is. This spec is the prerequisite for migration tracks B / C / D.

---

## 1. Routing

| Property | Rule |
|---|---|
| Inbound path prefix | `/api/rc/...` (preserved from current venue proxy) |
| Outbound path | `/api/v1/...` on racecontrol backend |
| Path mapping | `/api/rc/<segments>` → `/api/v1/<segments>` (concatenate segments with `/`) |
| Query string | preserved verbatim |
| Body | streamed through unchanged for non-GET/HEAD (no JSON re-serialization, supports multipart) |

---

## 2. HTTP methods

| Method | Status |
|---|---|
| GET | required |
| POST | required |
| PUT | required |
| PATCH | **required** (currently missing — `accounting`, `pricing` use PATCH) |
| DELETE | required |
| HEAD | **required** (currently missing — health probes need it) |
| OPTIONS | required for CORS preflight (cloud only — venue is same-origin) |

---

## 3. Auth modes (the critical gap)

The proxy must accept and forward **all** auth schemes used by client surfaces. Detection rule: pick auth based on incoming request shape — no hardcoded surface-type header needed.

| Auth mode | Inbound shape | Outbound forwarding | Used by |
|---|---|---|---|
| **Admin JWT (cookie)** | `Cookie: <COOKIE_NAME>=<jwt>` | `Authorization: Bearer <jwt>` | Admin UI itself |
| **Staff JWT (header)** | `Authorization: Bearer <jwt>` | passthrough as-is | WhatsApp staff endpoints, kiosk staff actions |
| **Customer JWT (header)** | `Authorization: Bearer <jwt>` (token has `customer` claim) | passthrough as-is | PWA, kiosk customer flows |
| **X-Service-Key** | `X-Service-Key: <key>` | passthrough as-is | rc-agent billing/CSV, WhatsApp some routes |
| **x-terminal-secret** | `x-terminal-secret: <secret>` | passthrough as-is | WhatsApp bot routes |
| **Kiosk-PIN** | `X-Kiosk-PIN: <pin>` | passthrough as-is | kiosk customer auth (per audit finding) |
| **Public (no auth)** | none | none | health probes, leaderboard reads, kiosk allowlist polling |

**Rule:** the proxy must NOT inject its own auth when an inbound auth header is already present. Only when the request comes from the Admin UI itself (cookie present, no Authorization header) does the proxy substitute the admin JWT.

**Multi-auth precedence** (when more than one is present): `X-Service-Key` > `x-terminal-secret` > `Authorization` > `Cookie`. Reason: service keys are explicit machine identity; cookies are inferred from session.

---

## 4. Headers (passthrough policy)

| Direction | Policy |
|---|---|
| Inbound → outbound | passthrough by default. Strip only: `Host`, `Content-Length` (recomputed), `Connection`, `Transfer-Encoding` |
| Outbound → inbound | passthrough by default. Strip only: `Set-Cookie` for non-Admin-UI callers (cookies are admin-UI-only concern) |
| Added by proxy | `X-Forwarded-For: <client-ip>`, `X-Forwarded-Host: <admin-host>`, `X-Request-Id: <uuid>` (if not already set) |

**Critical add:** `Idempotency-Key`, `If-Match`, `If-None-Match`, `Range`, `Accept`, `Accept-Encoding` must passthrough (currently dropped by venue proxy). Booking calls use idempotency keys; ETag/304 caching needs If-Match.

---

## 5. WebSocket relay — EXEMPT from v0/v1 spine rule (DECIDED 2026-04-23)

**Decision:** WS routes are EXEMPT. Kiosk and other surfaces continue to call `ws://<racecontrol>:8080/ws/...` directly. Revisit when:
- Measured WS volume justifies the runtime complexity of a sidecar process, OR
- Admin-side WS-level inspection becomes a hard requirement (audit, fan-out, message rewriting).

**Rationale:**
- Next.js handler model doesn't support raw WebSocket upgrade — would require a separate Node process (e.g. `ws` server or `socket.io`) or a frontend reverse proxy (nginx/caddy) running alongside admin
- Kiosk `/ws/dashboard` is the only known WS consumer in current audits — single surface, real-time critical, latency-sensitive
- HTTP traffic (60+ kiosk REST + 23 WhatsApp REST + ~9k/day rc-agent polling) is the dominant volume — proxying that captures 99%+ of spine value with 1× the engineering cost
- Spine doctrine intentionally left room for "real-time telemetry firehose" exemption (`project_admin_panel_operator_model.md` §3 spine-bypass detection rule)

If WS proxying lands later, the eventual contract was:

| Aspect | Rule |
|---|---|
| Inbound path | `/api/rc/ws/...` |
| Outbound | `ws://<rc-backend>/ws/...` |
| Protocol | bidirectional relay — admin acts as transparent passthrough |
| Auth | inherits from upgrade request headers (cookie or `Authorization`) |
| Idle timeout | inherit racecontrol's (no admin-side cap) |
| Reconnect | client's responsibility — admin doesn't buffer disconnected sessions |

---

## 6. Timeouts + retries

| Path category | Timeout | Retry policy |
|---|---|---|
| Polling reads (whitelist, allowlist, mesh-key, flags) | 10s | none — caller retries on next interval |
| Standard reads | 5s | none — surface latency to caller |
| Standard writes | 10s | none — writes are not idempotent unless `Idempotency-Key` present |
| Long writes (export, deploy, OTA) | 60s | none |
| WebSocket | n/a | n/a |

The proxy must NEVER swallow upstream errors silently. 5xx from racecontrol passes through with original status + body.

---

## 7. Rate limiting

| Caller class | Limit |
|---|---|
| Admin UI (cookie auth) | 100 req/sec per session (generous — UI bursts on dashboard refresh) |
| rc-agent (X-Service-Key) | 50 req/sec per pod (pod_id derived from key or path) |
| WhatsApp/Discord (terminal-secret) | 30 req/sec per bot |
| Kiosk (kiosk-PIN or customer-JWT) | 20 req/sec per pod |
| Public (no auth) | 5 req/sec per IP |

429 response when exceeded. Limits enforced per (caller-id, path-prefix) tuple to avoid one chatty endpoint starving another.

---

## 8. Observability

| Signal | Required |
|---|---|
| Per-request log line | `ts | request_id | method | path | caller_class | status | latency_ms | upstream_status` |
| Per-request log destination | stdout (PM2 captures) — sample 1% of GETs, 100% of writes, 100% of 4xx/5xx |
| Metric: `admin_gateway_requests_total{method, status, caller}` | Prometheus-scrapeable on `/api/admin-gateway/metrics` |
| Metric: `admin_gateway_request_duration_seconds{method, caller}` | summary (p50/p95/p99 + sum + count), same labels |
| Health probe | `GET /api/admin-gateway/health` returns proxy self-state + upstream RC reachability + last-success timestamp |
| Trace propagation | passthrough `traceparent` / `tracestate` headers |

**Path note (2026-04-23 evening correction):** original draft put meta endpoints at `/api/rc/__health` + `/api/rc/__metrics`. Next.js excludes folders prefixed with `_` from routing (private folder convention), so those returned 404 HTML on smoke. Moved to `/api/admin-gateway/{health,metrics}` — outside the `[...path]` proxy hierarchy, semantically clearer (gateway-meta, not racecontrol-proxy).

The reliability probe (drafted but not running per memory) consumes `/api/admin-gateway/health` to alert on spine outages.

---

## 9. Cloud-vs-venue split

The venue proxy already routes some writes to `RC_CLOUD_URL` (`isCloudAuthoritativeWrite` rule for staff/admin mutations). Contract preserves this:

| Surface origin | Default backend | Cloud-authoritative override |
|---|---|---|
| Venue admin (`:3201`) | venue racecontrol `:8080` | staff/* writes (except validate-pin) → `RC_CLOUD_URL` |
| Cloud admin (`admin.racingpoint.cloud`) | cloud racecontrol | reads can fall back to venue `RC_VENUE_URL` if cloud has no data (cross-tenant reads disabled — security boundary) |

**Cloud admin TBD:** the 22 stub routes flagged in `plan_admin_panel_spinal_cord_gap_20260422.md` block cloud spine. This contract assumes Uday's A/B/C decision lands; if A (revert to coming-soon) the cloud half of this contract is deferred.

---

## 10. Failure modes (preserved + extended)

| Failure | HTTP | error_code | Body |
|---|---|---|---|
| `RC_URL` env missing | 503 | `RC_URL_MISSING` | hint to set env |
| Upstream unreachable | 502 | `RC_UNREACHABLE` | detail |
| Upstream returns non-JSON | 502 | `INVALID_RC_RESPONSE` | first 200 chars of raw body |
| No auth token (admin-UI path only) | 401 | `NO_TOKEN` | — |
| Auth rejected by upstream | upstream status | upstream code | upstream body |
| Rate limit exceeded | 429 | `RATE_LIMITED` | `retry_after_seconds` |
| Path not allowed (deny-list, e.g. `/admin/internal/*`) | 403 | `PATH_FORBIDDEN` | — |

---

## 11. Implementation phasing — STATUS

**Phase A1 — venue proxy extension** ✅ IMPLEMENTED 2026-04-23
- Extended `src/app/api/rc/[...path]/route.ts` with all 7 methods, multi-auth precedence, header-passthrough deny-list, request-id, body-as-arrayBuffer, structured stdout log
- Backward compatible: existing admin-UI cookie flow works unchanged
- See `A1-IMPLEMENTATION-NOTES.md` for caveats

**Phase A2 — observability** ✅ IMPLEMENTED 2026-04-23 (path corrected after smoke)
- `/api/admin-gateway/health` returns proxy + upstream RC reachability + last success/failure timestamps
- `/api/admin-gateway/metrics` emits Prometheus text format (admin_gateway_requests_total counter + duration_seconds summary)
- Sampled request logging (100% writes/errors, 1% GETs)
- Reliability probe wiring NOT done — probe doesn't exist yet (P1-5 in plan_admin_panel_spinal_cord_gap)
- **Smoke verified 2026-04-23 evening** from James .27 → localhost:3000 dev server: health JSON returns `{healthy:true, upstream_reachable:true, upstream_status:200, upstream_latency_ms:2}`; metrics returns Prometheus counter + summary with real values from priming traffic (3 requests counted, p50=5ms p95=8ms p99=8ms). See `A1-IMPLEMENTATION-NOTES.md` smoke evidence section.

**Phase A3 — cloud admin port** 🟡 SEPARATE ORGAN (un-collapsed 2026-04-23 evening per doctrine v3 §5)
- **Earlier "A3 collapsed because same repo" framing was wrong-shape.** Same codebase ≠ same organ. The two organs (venue admin `:3201` + cloud admin `admin.racingpoint.cloud`) must implement the **same contract** but stay **independently deployed, configured, and verified**. See `TWO-SPINE-SYNC.md` and doctrine §5 for the exoskeleton model.
- The branch currently lands the contract on the venue organ only. Cloud organ needs its own:
  - Deploy run (separate Bono VPS PM2 reload — not auto-covered by venue deploy)
  - Env/config (`RC_URL` → cloud RC; `ADMIN_COMING_SOON_GATE` resolution; cloud-authoritative-write rules)
  - Browser-level revival verification per doctrine §7 (tsc-green ≠ functional organ)
  - 22-stub-route fix (Phase 445 follow-up; cloud-only problem; would not surface on venue)
- The cloud organ is currently gated by `ADMIN_COMING_SOON_GATE !== '0'` → middleware 503s `/api/rc/*` before the new proxy code runs. A3 also covers the gate-removal sequencing (Option 2 lands → toggle gate off, OR carve `/api/rc/` into `GATE_PUBLIC_PREFIXES`).
- Uday A/B/C cloud decision still pending for the broader cloud-admin-restoration question. A3 stays open until cloud organ has its own §7 revival evidence.

**Phase A4 — WebSocket relay** 🚫 EXEMPT (decided in §5)
- Kiosk `/ws/dashboard` continues to hit `:8080` direct
- Re-evaluate if measured volume or audit requirements force the issue

**Phase A5 — rate limiting** 🟡 STUB IMPLEMENTED 2026-04-23
- `checkRateLimit()` in `admin-gateway-state.ts` with per-class limits per §7
- Disabled by default. Enable with `ADMIN_GATEWAY_RATE_LIMIT=1` env
- Will turn on once A2 metrics show real surface migration volume

Surface migrations (Tracks B/C/D) NOW UNBLOCKED. Track B (Kiosk + WhatsApp REST) is ready to start. Track C (rc-agent polling) waits for first week of A2 metrics. Track D (Discord) still waits on source access.

---

## 12. Open questions for Uday

1. **WS relay via admin: yes, no, or "exempt /ws/* from spine rule"?** (§5)
2. **Kiosk-PIN auth header name** — does `X-Kiosk-PIN` exist or do I need to coordinate with kiosk team on header naming?
3. **Cloud admin A/B/C decision** still blocks Phase A3 (A3 is a SEPARATE organ per doctrine v3 §5, not the same-repo collapse — needs its own deploy + revival test). Can A/B/C be unblocked, or does A3 stay deferred?
4. **Rate limit class for "kiosk staff actions"** — staff sometimes use kiosk for admin tasks; does that fall under kiosk class (20 req/sec) or staff class?
5. **Are there auth modes I missed?** This list comes from 4 surface audits + memory; if PWA uses a 7th auth mode, A1 needs to handle it before kiosk migration.

---

## Out of scope for this contract

- Anything that touches racecontrol backend code (this is admin-side only)
- Surface-side migration code (covered by Tracks B/C/D plans)
- Cloud admin's 22 stub-route fix (covered by separate Phase 445 / Cloud A/B/C plan)
- Reliability probe implementation (drafted in `03-RELIABILITY-PROBE-SPEC.md` per memory, but file doesn't exist on disk — needs creation)
