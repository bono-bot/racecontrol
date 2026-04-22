# Admin Panel — current placement vs target placement

**Date:** 2026-04-23
**Audit basis:** `bono-bot/racingpoint-admin` route inventory + 4-surface outbound HTTP audit (Kiosk + WhatsApp + rc-agent + admin) + existing graphify outputs (`racecontrol/graphify-out-unified/` 17682 nodes, 48790 edges across 22 repos)
**Companion:** [`GATEWAY-CONTRACT.md`](GATEWAY-CONTRACT.md), [`A1-IMPLEMENTATION-NOTES.md`](A1-IMPLEMENTATION-NOTES.md)

---

## Tool note

Existing graphify outputs (`graphify-out-admin-api/`, `graphify-out-kiosk/`, `graphify-out-pwa/`, `graphify-out-rc-agent/`, `graphify-out-cross-process/`, `graphify-out-unified/`) are **code-AST level** — they map function calls, symbol references, and community detection within and across repos. They cannot directly produce a conceptual data-flow diagram showing "PWA → admin → racecontrol" because that's a network-call relationship, not an AST one. The unified graph's 2601 cross-repo edges are label-match heuristics (`confidence_tier=AMBIGUOUS`, `confidence=0.5`) per `UNIFY_REPORT.md` — not authoritative for HTTP wiring.

Mermaid is the right tool for the conceptual layer. The graphify outputs ground the **counts** (nodes per repo) and confirm the **scope** (which surfaces exist). Future work could extend graphify to ingest HTTP-call edges via static analysis of `fetch()` / `reqwest::` call sites — that's a Tier 5 polish item in `project_graphify_utilization_roadmap.md`.

---

## Current placement (2026-04-23)

```mermaid
graph LR
    subgraph Surfaces[Client surfaces]
        PWA[PWA<br/>customer phones]
        Kiosk[Kiosk<br/>per-pod display<br/>60+ REST + 1 WS]
        WhatsApp[WhatsApp bot<br/>23 REST]
        Discord[Discord bot<br/>?? REST<br/>not audited]
        RcAgent[rc-agent<br/>Pods 1-8 + POS<br/>11 endpoints<br/>~9000/day]
    end

    subgraph Admin[Admin Panel :3201 / cloud]
        AdminUI[Admin UI<br/>56 pages]
        Proxy[/api/rc/[...path]<br/>universal proxy<br/>used ONLY by AdminUI/]
        Islands[Local-API islands<br/>HR x4 + finance + sales<br/>+ purchases + analytics<br/>+ calendar<br/>SQLite-only]
    end

    Brain[("racecontrol :8080<br/>Rust/Axum<br/>~201 admin endpoints")]

    PWA -->|direct :8080| Brain
    Kiosk -->|direct :8080<br/>60+ REST + WS| Brain
    WhatsApp -->|direct :8080<br/>23 REST| Brain
    Discord -.->|direct? unknown| Brain
    RcAgent -->|direct :8080<br/>~9k/day| Brain

    AdminUI --> Proxy
    Proxy -->|/api/v1/...| Brain
    AdminUI --> Islands
    Islands -.->|never reaches| Brain

    style Admin fill:#fff3cd,stroke:#856404
    style Proxy fill:#d4edda,stroke:#155724
    style Islands fill:#f8d7da,stroke:#721c24
    style Brain fill:#cce5ff,stroke:#004085
```

**Annotations:**
- Admin proxy exists and works correctly — but only the Admin UI itself uses it (one consumer)
- Every other surface (Kiosk / WhatsApp / rc-agent / Discord / PWA) calls racecontrol direct on `:8080`, completely bypassing admin
- Local-API islands route data INTO admin (admin.db SQLite) but never propagate to racecontrol — the brain doesn't see this data
- Discord audit blocked: source not in local mirror

**Implications:**
- Admin has no visibility into customer-facing flows (bookings via WhatsApp, kiosk launches, lap times via rc-agent)
- Settings made in Admin only affect Admin UI, not any other surface — pricing/menu/hours updates don't reach the surfaces
- The `02-cloud-admin` 22-stub-route problem cited in `plan_admin_panel_spinal_cord_gap_20260422.md` reflects this: cloud admin's `/api/*` routes are scaffolds because nobody is consuming them
- This is the "parallel UI" model my earlier analysis assumed — the Uday 2026-04-23 doctrine clarification makes it the WRONG model

---

## Target placement (post-Tracks B+C+D migration)

```mermaid
graph LR
    subgraph Surfaces[Client surfaces]
        PWA[PWA<br/>customer phones]
        Kiosk[Kiosk<br/>per-pod display]
        WhatsApp[WhatsApp bot]
        Discord[Discord bot]
        RcAgent[rc-agent<br/>Pods 1-8 + POS]
    end

    subgraph Admin[Admin Panel<br/>now also gateway]
        AdminUI[Admin UI]
        Gateway[/api/rc/[...path]<br/>multi-auth gateway<br/>__health + __metrics/]
        Settings[(Settings store<br/>pricing + hours + menu)]
    end

    Brain[("racecontrol :8080<br/>brain / source of truth")]

    PWA -->|/api/rc/...<br/>via admin proxy| Gateway
    Kiosk -->|REST: /api/rc/...| Gateway
    Kiosk -.->|WS exempt:<br/>direct :8080/ws/dashboard| Brain
    WhatsApp -->|/api/rc/...<br/>x-terminal-secret| Gateway
    Discord -->|/api/rc/...| Gateway
    RcAgent -->|/api/rc/...<br/>X-Service-Key<br/>polling + diagnostics| Gateway
    RcAgent -.->|critical-path exempt:<br/>billing stop-service<br/>+ CSV fallback direct| Brain

    AdminUI --> Gateway
    Gateway -->|/api/v1/...| Brain
    Settings -->|push to surfaces<br/>config endpoint| PWA
    Settings -->|push to surfaces| Kiosk
    Settings -->|push to surfaces| WhatsApp
    Settings -->|push to surfaces| RcAgent
    Settings -->|push to brain<br/>state mutation| Brain

    style Admin fill:#d4edda,stroke:#155724
    style Gateway fill:#28a745,stroke:#155724,color:#fff
    style Settings fill:#28a745,stroke:#155724,color:#fff
    style Brain fill:#cce5ff,stroke:#004085
```

**Annotations:**
- Every surface routes its racecontrol calls through `/api/rc/...` on admin
- Admin has full visibility into all data in transit (audit, observability, request-id correlation)
- Settings made in Admin propagate two ways: outbound to surfaces (config push) AND inbound to racecontrol (state mutation)
- WS exempt (kiosk `/ws/dashboard`) — admin doesn't relay WS; kiosk hits :8080 direct (decision recorded in `GATEWAY-CONTRACT.md` §5)
- rc-agent critical paths exempt (billing stop-service, CSV fallback) — service-key auth + latency-sensitive, kept direct per `A1-IMPLEMENTATION-NOTES.md` recommendation

---

## Migration map — what changes, per surface

| Surface | Current | Target | Track | Effort |
|---|---|---|---|---|
| Admin UI itself | `/api/rc/[...path]` ✓ | unchanged | — | done |
| Kiosk REST | direct `:8080`, 60+ calls | `/api/rc/...` via admin | B | env swap + smoke (~1 day per pod, parallelizable) |
| Kiosk WS | direct `:8080/ws/dashboard` | unchanged (EXEMPT) | — | none |
| WhatsApp REST | direct via `RACECONTROL_URL`, 23 calls | `/api/rc/...` via admin (preserve `x-terminal-secret`) | B | env swap on Bono VPS bot |
| Discord REST | unknown (audit blocked) | `/api/rc/...` via admin | D | needs source access first |
| rc-agent polling (3 endpoints, 6912/day) | direct `:8080` | `/api/rc/...` via admin (with desync) | C | TOML `admin_url` + per-pod stagger, after 1wk A2 metrics |
| rc-agent diagnostics (KB search, audit-check) | direct `:8080` | `/api/rc/...` via admin | C | same as polling |
| rc-agent billing critical (stop-service, CSV) | direct `:8080` | unchanged (EXEMPT) | — | none |
| PWA | direct (assumed; not audited) | `/api/rc/...` via admin | (TBD with Uday) | env swap |
| Admin local-API islands | admin.db SQLite, off-spine | PROMOTE 2 (finance, analytics) → racecontrol+admin proxy<br/>REMOVE 1 (calendar) → PWA<br/>HR (4 pages): defer to Uday | island-cleanup | per-island design |

---

## Existing graphify counts (per `UNIFY_REPORT.md` 2026-04-21)

| Repo | Nodes | Edges | Role in spine |
|---|---|---|---|
| racecontrol | 12527 | 39654 | brain |
| comms-link | 811 | 1079 | sidecar (relay; not in spine) |
| admin | 407 | 488 | gateway (small surface area — proxy is 1 file) |
| whatsapp-bot | 409 | 734 | surface |
| pod-agent | 52 | 79 | DEPRECATED — replaced by rc-agent inside racecontrol |
| bono-whatsapp-bot | 290 | 547 | bono-side mirror of whatsapp |
| bono-discord-bot | 84 | 79 | surface — audit-blocked |
| bono-cloud-dashboard | 63 | 62 | sub-app (separate from cloud admin?) |
| bono-api-gateway | 66 | 93 | **noteworthy** — bono already started a gateway pattern; worth investigation |
| memory | 1261 | 1399 | meta (this knowledge graph) |

**`bono-api-gateway` exists but is small (66 nodes).** Could be: (a) early/abandoned attempt at this exact pattern, (b) something different (e.g. external API aggregation). Worth a 5-minute look before assuming admin gateway is greenfield. Action item: read `bono-mirror/racingpoint-api-gateway/graphify-out/graph.json` or check git history.

---

## What graphify can NOT do here (and how to extend it)

The "cross-repo edges" added by `unify.mjs` (2601 in current run) are label-match heuristics — same function name across repos. That's NOT a network-call edge. Two surfaces could both have `function getPods()` with totally different implementations and graphify would link them anyway.

**Tier 5 polish (per `project_graphify_utilization_roadmap.md`):** add a custom extractor that scans surface code for `fetch("..." )`, `axios.<method>("..." )`, `reqwest::Client::<method>("..." )` and emits an edge `surface_code_node → endpoint_node` (where endpoint_node is identified by URL pattern matching racecontrol routes). That would make the current diagram regenerable from code. Until then: hand-built mermaid grounded in audit findings is the source of truth.

---

## What we're explicitly NOT diagramming

- Internal racecontrol architecture (handlers, services, DB layer) — that's `docs/ARCHITECTURE.md`'s job
- Pod hardware topology — `CLAUDE.md` Network Map
- Comms-link relay flow — separate from spine (it's the bono-james async channel, not customer data path)
- WhatsApp Evolution API + Anthropic + OpenRouter — those are external dependencies, not surfaces in the spine

---

## Open questions for Uday on the diagram

1. **PWA placement** — your concurrent PWA session is presumably about wiring PWA. Does the target diagram match your intent? If PWA already routes via admin in your current branch, the "PWA: direct (assumed)" label can be retired.
2. **Cloud admin behavior in current diagram** — the diagram treats venue + cloud admin as one box. If the cloud admin is supposed to route to cloud racecontrol (different brain), the target needs a tenant-split version. Worth a separate diagram.
3. **HR data model** — local-API islands include 4 HR pages on admin.db SQLite. If HR data should live in racecontrol long-term, that's a backend schema add, not just a UI promote. Defer or scope?
