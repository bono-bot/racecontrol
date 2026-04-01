# Infrastructure Evolution Roadmap: v34.0 - v37.0

> Created: 2026-04-01 | Source: Aspirational architecture audit + venue-scale design session
> Status: BACKLOG — not initialized as GSD milestones yet
> Depends on: v32.0 (Meshed Intelligence) + v33.0 (Billing Integrity) shipping first

## Origin

This roadmap was designed by analyzing an aspirational architecture document that described
enterprise-grade infrastructure (NATS, Redis, Prometheus, MLflow, Kubeflow, Kubernetes, etcd,
MinIO/S3, Data Lake) and mapping each component to a **venue-scale alternative** that delivers
~80% of the benefit at ~10% of the complexity.

**Standing rule applied:** "Extend, don't replace" — WS + mpsc + SQLite + DashMap sufficient
for 8 pods. External dependencies only when current solutions hit scaling limits.

---

## System Context Flowchart

```mermaid
flowchart TD
    subgraph CURRENT["Current State (v32.0)"]
        direction TB
        C1["WebSocket + mpsc\n(messaging)"]
        C2["SQLite WAL + DashMap\n(state & cache)"]
        C3["Custom JSON /metrics API\n(observability)"]
        C4["Ollama + OpenRouter\n5-tier AI diagnosis"]
        C5["TOML files + feature flags\n(config)"]
        C6["Local files + SCP\n(backup/deploy)"]
        C7["PSK + JWT\n(security)"]
    end

    subgraph EVOLUTION["Evolution Path (v34-v37)"]
        direction TB
        E1["v34.0: SQLite TSDB\n+ metrics dashboard\n+ Prometheus export"]
        E2["v35.0: Model evaluations\n+ KB promotion ladder\n+ retrain pipeline"]
        E3["v36.0: Server-pushed config\n+ policy rules engine\n+ game presets"]
        E4["v37.0: Backup pipeline\n+ cloud sync v2\n+ fleet deploy automation"]
        E5["v38.0: Venue CA + mTLS\n+ RBAC + audit chain"]
    end

    subgraph ASPIRATIONAL["Aspirational (Deferred)"]
        direction TB
        A1["NATS JetStream\n(fleet > 16 pods)"]
        A2["Redis\n(multi-process server)"]
        A3["PostgreSQL\n(venue 2 confirmed)"]
        A4["Kubernetes\n(never for Windows pods)"]
        A5["MLflow / Kubeflow\n(> 20 models in rotation)"]
    end

    C3 --> E1
    C4 --> E2
    C5 --> E3
    C6 --> E4
    C7 --> E5

    E1 -.->|"trigger: >100 metrics at >1Hz"| A1
    E2 -.->|"trigger: >20 models"| A5
    E3 -.->|"trigger: multi-server"| A2
    E4 -.->|"trigger: venue 2"| A3

    style CURRENT fill:#1a1a2e,stroke:#4ecca3,color:#fff
    style EVOLUTION fill:#0f3460,stroke:#3282b8,color:#fff
    style ASPIRATIONAL fill:#2d132c,stroke:#ee4540,color:#fff
```

---

## Dependency Graph

```mermaid
flowchart LR
    V32["v32.0\nMeshed Intelligence\n(ACTIVE)"] --> V33["v33.0\nBilling Integrity\n(ACTIVE)"]
    V33 --> V34["v34.0\nMetrics TSDB\n+ Dashboards"]
    V34 --> V35["v35.0\nModel Lifecycle\n+ Retraining"]
    V34 --> V36["v36.0\nConfig & Policy\nEngine"]
    V35 --> V37["v37.0\nData Durability\n+ Multi-Venue"]
    V36 --> V37
    V37 --> V38["v38.0\nSecurity\nHardening"]

    style V32 fill:#006400,stroke:#4ecca3,color:#fff
    style V33 fill:#006400,stroke:#4ecca3,color:#fff
    style V34 fill:#0f3460,stroke:#3282b8,color:#fff
    style V35 fill:#0f3460,stroke:#3282b8,color:#fff
    style V36 fill:#0f3460,stroke:#3282b8,color:#fff
    style V37 fill:#0f3460,stroke:#3282b8,color:#fff
    style V38 fill:#0f3460,stroke:#3282b8,color:#fff
```

**Parallelism:** v35.0 and v36.0 can partially overlap (independent domains).
v37.0 needs all data stores from v34-v36 to exist before backing them up.
v38.0 is last — hardens the final attack surface.

---

## v34.0 — Time-Series Metrics & Operational Dashboards

**Theme:** Give the custom metrics infrastructure time-series depth so you can answer
"what happened last Tuesday at 8pm" without grepping JSONL logs.

**Why now:** v32.0 builds autonomous action loops. v34.0 makes those loops observable
and queryable. You cannot tune predictions, pricing, or scheduling without historical
trend data.

**Aspirational components replaced:**
- Prometheus TSDB --> SQLite metrics_tsdb + rollups
- Grafana --> Custom Next.js /metrics page
- Alert Router --> WhatsApp threshold alerts

```mermaid
flowchart TD
    subgraph V34["v34.0 — Metrics & Dashboards (Phases 285-289)"]
        direction TB

        P285["Phase 285: Metrics Ring Buffer\n- SQLite metrics_samples table\n- 1-min resolution, 7-day raw\n- Hourly/daily rollups (90-day)\n- Captures: CPU, GPU temp, FPS,\n  billing, revenue, WS connections"]

        P286["Phase 286: Metrics Query API\n- GET /metrics/query?name=...&from=...&to=...\n- /metrics/names (list all)\n- /metrics/snapshot (current values)"]

        P287["Phase 287: Metrics Dashboard\n- Next.js /metrics page\n- Sparkline charts (recharts)\n- Pod selector, time range picker\n- 30s auto-refresh"]

        P288["Phase 288: Prometheus Export\n- GET /metrics/prometheus\n- Exposition format endpoint\n- Zero-cost future option\n- No Prometheus server deployed"]

        P289["Phase 289: Metric Alert Thresholds\n- TOML-configured alert_rules\n- Evaluated every 60s against TSDB\n- Fires to WhatsApp alerter\n- Replaces hardcoded thresholds"]

        P285 --> P286 --> P287
        P286 --> P288
        P285 --> P289
    end

    style V34 fill:#0f3460,stroke:#4ecca3,color:#fff
```

### Key Files
| Phase | New/Modified File | Purpose |
|-------|------------------|---------|
| 285 | `crates/racecontrol/src/metrics_tsdb.rs` (new) | SQLite time-series store with rollups |
| 286 | `crates/racecontrol/src/api/metrics_query.rs` (new) | Query + snapshot + names endpoints |
| 287 | `racingpoint-admin/src/app/metrics/page.tsx` (new) | Sparkline dashboard page |
| 288 | `crates/racecontrol/src/api/metrics_query.rs` (extend) | Prometheus exposition format |
| 289 | `crates/racecontrol/src/alert_engine.rs` (extend) | TOML-driven threshold alerts |

---

## v35.0 — Structured Retraining & Model Lifecycle

**Theme:** Close the continuous learning loop. Solutions that work get promoted;
models that underperform get demoted; the system gets measurably smarter each week.

**Why now:** v32.0 builds the KB hardening pipeline (observation stage). v34.0 gives
you metrics to measure model accuracy. v35.0 wires them together.

**Aspirational components replaced:**
- MLflow --> SQLite model_evaluations table
- Kubeflow --> Cron + JSONL export
- Feature Store --> fleet_solutions + model_evaluations

```mermaid
flowchart TD
    subgraph V35["v35.0 — Model Lifecycle (Phases 290-294)"]
        direction TB

        P290["Phase 290: Model Evaluation Store\n- SQLite model_evaluations table\n- Every AI diagnosis writes:\n  prediction, actual, correct, cost\n- Weekly rollup: accuracy, cost-per-correct"]

        P291["Phase 291: Full KB Promotion Ladder\n- Shadow (silent, compare to human)\n- Canary (Pod 8 only)\n- Quorum (3+ pods confirm)\n- Tier 1 (deterministic rule, $0)\n- 6-hour cron evaluation"]

        P292["Phase 292: Model Reputation + Auto-Demotion\n- 7-day accuracy < 30% = remove\n- Dashboard: per-model trends\n- Cost efficiency ranking\n- Current roster status"]

        P293["Phase 293: Retrain Data Export\n- Weekly cron: JSONL training data\n- Evaluations + solutions\n- Ollama/Unsloth-compatible format\n- Pipeline ready, manual trigger"]

        P294["Phase 294: Intelligence Report v2\n- Weekly WhatsApp to Uday:\n  model accuracy rankings,\n  KB promotions, cost savings,\n  prediction trends"]

        P290 --> P291
        P290 --> P292
        P290 --> P293
        P291 --> P294
        P292 --> P294
    end

    style V35 fill:#0f3460,stroke:#4ecca3,color:#fff
```

### Key Files
| Phase | New/Modified File | Purpose |
|-------|------------------|---------|
| 290 | `crates/racecontrol/src/fleet_kb.rs` (extend) | Evaluation tables + write functions |
| 291 | `crates/racecontrol/src/fleet_kb.rs` (extend) | Promotion pipeline with cron trigger |
| 292 | `crates/racecontrol/src/ai.rs` + admin page (new) | Model roster management + dashboard |
| 293 | `scripts/export-training-data.sh` (new) | Weekly JSONL export |
| 294 | `crates/racecontrol/src/fleet_report.rs` (extend) | Enhanced weekly report |

---

## v36.0 — Config Management & Policy Engine

**Theme:** Centralize configuration so every pod runs from server-pushed config,
not local TOML files that drift.

**Why now:** With metrics (v34) and model lifecycle (v35) working, the next
bottleneck is config drift across 8 pods.

**Aspirational components replaced:**
- etcd Policy Store --> SQLite + WS config push
- Config Management --> Typed schema + admin UI

```mermaid
flowchart TD
    subgraph V36["v36.0 — Config & Policy (Phases 295-299)"]
        direction TB

        P295["Phase 295: Config Schema & Validation\n- Typed Rust AgentConfig struct\n- serde validation attributes\n- Invalid fields = fallback + warning\n- Schema version for forward compat"]

        P296["Phase 296: Server-Pushed Config\n- SQLite pod_configs table\n- Config pushed via WS on connect\n- Hot-reload: thresholds, flags, budget\n- Cold (restart): ports, paths"]

        P297["Phase 297: Config Editor UI\n- Admin /config page\n- Per-pod editor with diff view\n- One-click push, bulk ops\n- Audit log of all changes"]

        P298["Phase 298: Game Preset Library\n- Server-managed car/track/session presets\n- Pushed to pods via config channel\n- Historical reliability scores\n- Unreliable combos flagged"]

        P299["Phase 299: Policy Rules Engine\n- IF metric_condition THEN action\n- Actions: change config, alert,\n  toggle flag, adjust budget\n- SQLite-backed, editable via admin"]

        P295 --> P296 --> P297
        P296 --> P298
        P296 --> P299
    end

    style V36 fill:#0f3460,stroke:#4ecca3,color:#fff
```

### Key Files
| Phase | New/Modified File | Purpose |
|-------|------------------|---------|
| 295 | `crates/rc-common/src/config_schema.rs` (new) | Typed config with validation |
| 296 | `crates/racecontrol/src/config_push.rs` (extend) | WS-based config distribution |
| 297 | `racingpoint-admin/src/app/config/page.tsx` (new) | Config editor UI |
| 298 | `crates/racecontrol/src/catalog.rs` (extend) | Game preset management |
| 299 | `crates/racecontrol/src/policy_engine.rs` (new) | Lightweight rule engine |

---

## v37.0 — Data Durability & Multi-Venue Readiness

**Theme:** Ensure operational data survives hardware failure and prepare the data
layer for a potential second venue.

**Why now:** With metrics, model lifecycle, and config centralized, the biggest
risk is data loss. SQLite on a single server disk is a single point of failure.

**Aspirational components replaced:**
- MinIO/S3 --> Local backup + SCP to Bono VPS
- Data Lake --> JSONL archives + SQLite events table
- GitOps Deploy --> Automated binary rollout with canary

```mermaid
flowchart TD
    subgraph V37["v37.0 — Data Durability (Phases 300-304)"]
        direction TB

        P300["Phase 300: SQLite Backup Pipeline\n- Hourly .backup (WAL-safe)\n- Local rotation: 7 daily + 4 weekly\n- Nightly SCP to Bono VPS\n- Staleness alert if > 2 hours"]

        P301["Phase 301: Cloud Data Sync v2\n- Extend cloud_sync.rs\n- Sync: solutions, evaluations, rollups\n- Server-authoritative for solutions\n- Cloud-authoritative for cross-venue"]

        P302["Phase 302: Structured Event Archive\n- All events --> SQLite events table\n  + daily JSONL\n- 90-day SQLite, JSONL to VPS\n- Venue-scale 'Data Lake'"]

        P303["Phase 303: Multi-Venue Schema Prep\n- Add venue_id to all tables\n- Default: racingpoint-hyd-001\n- No functional change\n- Design doc for venue 2 trigger"]

        P304["Phase 304: Fleet Deploy Automation\n- POST /api/v1/fleet/deploy\n- Canary-first (Pod 8)\n- Health verify + auto-rollout\n- Auto-rollback on failure"]

        P300 --> P301
        P300 --> P302
        P301 --> P303
        P302 --> P303
        P303 --> P304
    end

    style V37 fill:#0f3460,stroke:#4ecca3,color:#fff
```

### Key Files
| Phase | New/Modified File | Purpose |
|-------|------------------|---------|
| 300 | `scripts/backup-databases.sh` (new) | Automated backup + rotation |
| 301 | `crates/racecontrol/src/cloud_sync.rs` (extend) | Multi-table cloud sync |
| 302 | `crates/racecontrol/src/activity_log.rs` (extend) | Structured event schema |
| 303 | DB migration + `docs/MULTI-VENUE-ARCHITECTURE.md` | Schema + design doc |
| 304 | `crates/racecontrol/src/ota_pipeline.rs` (extend) | Canary deploy with rollback |

---

## v38.0 — Security Hardening & Operational Maturity

**Theme:** Harden the security posture after all data flows are established.

**Why now:** With data flowing through metrics, config, and cloud sync channels,
the attack surface has grown. This is the right time to harden.

**Aspirational components replaced:**
- mTLS --> Self-signed venue CA + mutual TLS
- IAM --> SQLite-backed RBAC
- Audit --> Hash-chained append-only logs

```mermaid
flowchart TD
    subgraph V38["v38.0 — Security (Phases 305-309)"]
        direction TB

        P305["Phase 305: TLS for Internal HTTP\n- Self-signed venue CA\n- mTLS on :8080 (server) + :8090 (agents)\n- Axum TLS config\n- Tailscale for remote (already encrypted)"]

        P306["Phase 306: WS Auth Hardening\n- PSK + per-pod JWT (24h expiry)\n- 1h pre-refresh rotation\n- Invalid token = disconnect + alert"]

        P307["Phase 307: Audit Log Integrity\n- Append-only hash chain\n- SHA-256 linking each entry\n- Tamper detection alert\n- Covers: config, deploys, billing, admin"]

        P308["Phase 308: RBAC for Admin\n- Roles: cashier, manager, superadmin\n- JWT role claims\n- Every endpoint checks role\n- Admin UI gating"]

        P309["Phase 309: Security Audit Script\n- Automated scan: ports, TLS,\n  JWT, default creds, chain integrity\n- Security scorecard JSON output"]

        P305 --> P306
        P305 --> P307
        P306 --> P308
        P307 --> P309
        P308 --> P309
    end

    style V38 fill:#0f3460,stroke:#4ecca3,color:#fff
```

### Key Files
| Phase | New/Modified File | Purpose |
|-------|------------------|---------|
| 305 | `crates/racecontrol/src/tls.rs` (extend) + `scripts/generate-venue-ca.sh` | Venue CA + mTLS |
| 306 | `crates/racecontrol/src/ws/` + `crates/rc-agent/src/` | JWT rotation on WS |
| 307 | `crates/racecontrol/src/activity_log.rs` (extend) | Hash-chained audit |
| 308 | `crates/racecontrol/src/auth/` + admin UI | Role-based access |
| 309 | `scripts/security-audit.sh` (new) | Automated security scan |

---

## Aspirational --> Venue-Scale Mapping (Complete)

| Aspirational Component | Venue-Scale Replacement | Milestone | Upgrade Trigger |
|---|---|---|---|
| Prometheus TSDB | SQLite metrics_tsdb | v34.0 | >100 metrics at >1Hz |
| Grafana | Custom Next.js | v34.0 | Never (custom is better) |
| MLflow | SQLite model_evaluations | v35.0 | >20 models |
| Kubeflow | Cron + JSONL export | v35.0 | Ollama supports fine-tuning |
| etcd | SQLite + WS config push | v36.0 | Multi-server racecontrol |
| PostgreSQL | SQLite WAL | v37.0 prep | Venue 2 confirmed |
| MinIO/S3 | Local backup + SCP | v37.0 | Artifacts > 50GB |
| NATS | WS + mpsc | Deferred | Fleet > 16 pods |
| Redis | DashMap | Deferred | Multi-process server |
| Kubernetes | Bare metal + watchdog | Deferred | Never (Windows pods) |
| mTLS | Self-signed venue CA | v38.0 | -- |
| Slack/SMTP | WhatsApp Evolution API | Not needed | Uday uses WhatsApp |

---

## Estimated Coverage

~80% of aspirational architecture benefits at ~10% complexity and ~5% infrastructure cost.

## When to Initialize

Each milestone should be initialized with `/gsd:new-milestone` only when the preceding
milestone ships. Requirements may shift — don't lock in details too early.
Phase numbers are provisional and will be assigned at initialization time.
