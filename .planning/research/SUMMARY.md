# v32.0 Research Summary — MMA Council (GPT-5.4 + Claude Opus 4.6 + Gemini 3.1 Pro + 2x Sonar)

## Stack Additions

- **circuitbreaker-rs** — async circuit breaker for Rust/Tokio (per-node + per-action)
- **DashMap** — concurrent hashmap for blast radius limiter (already in deps)
- **bincode** — fast gossip serialization (10x faster than JSON for mesh)
- **Evolution API v2** — WhatsApp via Bono VPS (`/message/sendText`, `/message/sendMedia`)
- No new crates for promotion pipeline — uses existing SQLite KB + tokio channels

## Architecture Consensus (3/3 models)

### Event Bus: broadcast + mpsc hybrid
- `broadcast<FleetEvent>` for fan-out alerts (predictive, anomaly, fix outcomes)
- `mpsc<Incident>` for work queues into tier engine
- `watch<PolicySnapshot>` for rollout policy, budget caps, enabled rules
- Actor-per-subsystem owns state; PodSupervisor manages health/timers

### KB Hardening: Promotion Ladder
1. **Observed** — fix applied once, outcome recorded
2. **Shadow** — fix runs alongside existing pipeline, log-only (1 week or N applications)
3. **Canary** — applied on 1 pod, verified
4. **Quorum** — 3+ successes across 2+ pods
5. **Deterministic Rule** — typed `Rule { matchers, preconditions, action, verifier, ttl, confidence, provenance }`
- Promotion criteria: success_rate >= 0.90, total_applications >= 25, multi-context wins, better MTTR
- Background `interval` task compiles to versioned registry, distributed via gossip hashes

### Runaway Prevention: 3-Layer Governor
1. **Per-node circuit breaker** — 40% fail rate trips, 2-min cooldown, auto-escalate
2. **AI budget governor** — AtomicU64 token tracking, max concurrent AI calls, skip consensus when low
3. **Blast radius limiter** — DashMap<NodeId, ActiveFix>, max 2/10 nodes under simultaneous fix, RAII FixGuard

### WhatsApp Evolution API v2
- **Tier 5 escalation:** `POST /message/sendText/:instance` with severity, asset, issue, AI actions tried, impact, dashboard link
- **Weekly report:** `POST /message/sendMedia/:instance` with chart image + caption summary
- **Auth:** `apikey` header, number in international format without +
- **Formatting:** `*bold*`, `_italic_`, `~strike~`, ``` ```monospace``` ```, `-` bullets
- **Rate limiting:** Dedupe by incident ID, 10-30min cooldown, 1-3 msgs/60s burst cap
- **Fallback:** SMS/call if WhatsApp unread past SLA window

## Watch Out For

1. **Shadow mode is critical** — promoting a confidently-wrong fix to deterministic rule can cascade fleet-wide
2. **Blast radius limiter is the most important guard** — prevents correlated failures across fleet
3. **WhatsApp is high-signal channel only** — P1 escalation + weekly report, not event stream
4. **Circuit breaker per-action AND per-dependency** — separate failing remediation from degraded API
5. **Idempotency keys on every executor action** — `node + rule_version + incident_fingerprint`
6. **Never hold locks across async gossip** — existing standing rule, applies to new event bus too
