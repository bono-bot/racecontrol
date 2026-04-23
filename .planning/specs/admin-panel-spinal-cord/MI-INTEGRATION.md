# MI integration — gateway as nervous-system endpoint

**Date:** 2026-04-23
**Status:** DESIGN (no code yet)
**Companion:** `GATEWAY-CONTRACT.md`, `project_admin_panel_operator_model.md` doctrine §6, `feedback_query_mi_before_spec.md`

## Doctrine recap (Uday §6)

> Mesh Intelligence is the nervous system (via graphify). It's the links that connect to all the parts of the spinal cord and the brain.

MI is **runtime connective tissue**, not a search index. Graphify is the diagnostic imaging tool that lets us SEE the nervous system — used at design time for static structure. MI carries dynamic signals at runtime.

## What this means for the gateway

The admin gateway is a high-traffic vantage point where every spine signal passes. Errors at the gateway are diagnostic gold for MI. The gateway should:

1. **Emit symptom signals to MI on errors** — every 4xx/5xx becomes a non-blocking POST to MI
2. **Query MI for diagnostic context on errors** — when admin renders an error to the operator, surface known fixes from MI
3. **Subscribe to MI updates** — if a known issue is fixed in MI, the gateway should know (e.g. don't keep alerting operator about the same incident)

## Symptom emission spec

**When:** any gateway-level error or notable event:
- Upstream 5xx (RC unreachable, RC returned 502/503/504)
- Upstream 4xx that indicates contract drift (401 with valid token, 422 schema mismatch)
- Latency anomaly (p99 latency over a window > 2× p50 baseline)
- Auth-mode mismatch (request hit gateway with malformed auth header)
- Rate limit triggered (per-class 429s)

**Endpoint:** `POST /api/v1/mesh/audit-seed-service` (service-key authed, already exists per audit)

**Payload:**
```json
{
  "findings": [{
    "problem_key": "admin_gateway_<class>",
    "severity": "P1|P2|P3",
    "symptom_patterns": ["upstream_502", "endpoint=/api/v1/billing/start"],
    "source": "admin-gateway",
    "request_id": "<uuid>",
    "endpoint": "/api/v1/billing/start",
    "caller": "kiosk-pin",
    "upstream_status": 502,
    "upstream_url": "http://192.168.31.23:8080",
    "first_seen": "<iso>",
    "fix_status": "unknown",
    "escalation_message": "Admin gateway: <N> upstream 502s in last 5min on /api/v1/billing/*"
  }]
}
```

**Constraints:**
- **Non-blocking:** spawn into background; gateway response to client must not wait on MI emit
- **Sampled / debounced:** don't emit one POST per error — bucket by problem_key over 30s, emit aggregate counts. Otherwise a flood of 502s creates a flood of MI posts
- **Service-key auth:** gateway needs the mesh service key; pull from env (`RC_SERVICE_KEY`) or admin-gateway-state cache
- **Best-effort:** if MI emit fails, log at warn and drop. Never let MI emit failure cascade into client error

## MI query spec

**When:** admin renders any error page (502 from gateway, 5xx from upstream) to the operator

**Pattern:**
1. On error, before returning HTML to operator, query: `GET /api/v1/mesh/solutions/search?q=<error_class>&limit=3`
2. If hits exist, render: "Known issue: <fix_action>. See <solution_id>."
3. If no hits, render generic error + offer "Report this to MI" button (which fires audit-seed)

**Constraints:**
- **Don't block on slow MI:** 1s timeout. If MI doesn't answer, render generic error
- **Cache hits:** if same error_class queried < 60s ago, reuse last result

## MI subscription spec (later phase, not v0)

When MI promotes a fix for a previously-unknown problem, the gateway should know so it can stop emitting symptoms for the same problem. Mechanism options:

- **Pull:** gateway re-queries MI every 5min for status of its currently-emitting problem_keys. Drop from emission list if `fix_status: code_fixed`.
- **Push:** MI broadcasts a `MeshSolutionPromoted` event over racecontrol's WS bus. Gateway subscribes. Uses the existing kiosk WS infra pattern.

Defer until A1 ships and we measure real symptom volume.

## Why this matters per the doctrine

Without MI integration, the gateway is a dumb passthrough — every operator sees every error and reasons from scratch. With MI integration:

- The body **learns** from spine errors (errors propagate to a place that can detect patterns)
- Operator **inherits prior diagnoses** (don't redebug what the fleet already solved)
- The brain (RC) sees a richer view of the body (gateway isn't just opaque proxy; it's an annotated channel)

This is the difference between a passive cable and a sensor-rich nerve.

## Implementation cost (rough)

- ~40 lines: bucketed symptom emitter in `admin-gateway-state.ts` (debounce + non-blocking POST)
- ~30 lines: MI query in error renderer
- Service key plumbing: ~10 lines + env var
- Test coverage: 5-6 unit tests for the bucketing + transmission logic

Total: ~1 day implementation. Defer cycle (subscription / push): another 2 days when needed.

## What this is NOT solving

- MI's own correctness (it's a separate system; gateway just emits)
- Cross-spine MI federation (each spine emits to its own brain's MI; brains can sync MI state via the existing 30s sync if needed)
- Operator UX for MI hits (out of scope for gateway; lives in admin error rendering)
