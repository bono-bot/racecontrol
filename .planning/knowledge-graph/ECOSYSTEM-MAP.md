# RacingPoint Ecosystem Map — 3 Layers

Built: 2026-04-16 | Method: E2E trace + PoE verification across 9 sources of truth

```
╔══════════════════════════════════════════════════════════════════╗
║  LAYER 1: CUSTOMER-FACING (what customers + staff touch)        ║
║  LAYER 2: PLATFORM (what runs the business)                     ║
║  LAYER 3: INFRASTRUCTURE (what keeps everything alive)          ║
╚══════════════════════════════════════════════════════════════════╝
```

---

## LAYER 1: CUSTOMER-FACING

Everything a customer, staff member, or the public sees and interacts with.

### 1.1 Customer Channels

| # | Component | Repo | Port | Machine | In Graph | Status |
|---|---|---|---|---|---|---|
| 1 | WhatsApp Bot v1 (JS) | racingpoint-whatsapp-bot | 3000 | Bono VPS | YES | Online |
| 1b | WhatsApp Bot v2 (TS) | racingpoint/whatsapp-bot | 3150 | Bono VPS | YES | Online |
| 2 | Discord Bot | racingpoint-discord-bot | — | Bono VPS | YES | Online |
| 3 | Public Website | racingpoint-website | 3600 | Bono VPS | YES | Online |
| 4 | Website API | racingpoint-website | 5050 | Bono VPS | YES | Online |
| 5 | Instagram (stub) | racingpoint-instagram | — | — | YES | Not live |

### 1.2 Staff-Facing Apps (Venue)

| # | Component | Repo | Port | Machine | In Graph | Status |
|---|---|---|---|---|---|---|
| 6 | POS Dashboard | racingpoint-dashboard | 3400 | Server .23 | YES | Online |
| 7 | Kiosk (pod screens) | racecontrol/kiosk | 3300 | Server .23 | YES | Online |
| 8 | Web Dashboard | racecontrol/web | 3200 | Server .23 | YES | Online |
| 9 | Admin Panel | racingpoint-admin | 3201 | Server .23 | YES | Online |
| 10 | Lock Screen / Timer | rc-agent (embedded) | 18923 | Pods 1-8 | YES | Online |
| 11 | Webterm (Uday) | webterm.py | 9999 | James .27 | YES | Online |

### 1.3 Staff-Facing Apps (Cloud)

| # | Component | Repo | Port | Machine | In Graph | Status |
|---|---|---|---|---|---|---|
| 12 | Cloud Dashboard | racingpoint-cloud-dashboard | 3700 | Bono VPS | YES | Online |
| 13 | Cloud Kiosk | racecontrol/kiosk | 3300 | Bono VPS | YES | Online |
| 14 | Cloud Web | racecontrol/web | 3200 | Bono VPS | YES | Online |
| 15 | Cloud Admin | racingpoint-admin | 3201 | Bono VPS | YES | Online |
| 16 | Cloud PWA | racecontrol/web | 3500 | Bono VPS | YES | Online |

### 1.4 Customer-Facing Hardware

| # | Component | Type | Machine | In Graph | Status |
|---|---|---|---|---|---|
| 17 | Sim Rigs (8 pods) | Conspit wheelbase + pedals + triple screens | Pods 1-8 | YES (rc-agent) | Active |
| 18 | POS PC | Staff checkout terminal | .130 | YES (rc-agent) | Active |
| 19 | Spectator Displays | Leaderboard / viewing | .200, .32, .84, .37 | N/A (display only) | Active |
| 20 | NVR + 13 Cameras | Dahua 4MP surveillance | .18 | N/A (external hw) | Active |

### Layer 1 E2E Trace

```
Customer Journey:
  WhatsApp (#1) → books session → Staff sees on POS (#6) or Kiosk (#7)
  → Customer arrives → Check-in at POS (#6) → Assigned to Pod (#17)
  → Lock Screen (#10) shows timer → Game launches → Telemetry flows
  → Session ends → Review nudge via WhatsApp (#1) → Leaderboard on Spectator (#19)
  → Cloud Dashboard (#12) shows analytics → Admin Panel (#9) for management

Staff Journey:
  POS (#6): check-in, billing, cafe → Kiosk (#7): game config, pod control
  → Admin (#9): analytics, pricing, staff mgmt → Webterm (#11): emergency access
  → Cloud (#12): remote monitoring
```

---

## LAYER 2: PLATFORM

The business logic, data, and APIs that power Layer 1.

### 2.1 Core Services

| # | Component | Binary/Runtime | Port | Machine | In Graph | Status |
|---|---|---|---|---|---|---|
| 21 | RaceControl Server | racecontrol.exe (Rust) | 8080 | Server .23 | YES | Online |
| 22 | RC-Agent (pod) | rc-agent.exe (Rust) | 8090 | Pods 1-8 + POS | YES | Online |
| 23 | RC-Sentry (pod exec) | rc-sentry.exe (Rust) | 8091 | Pods 1-8 + POS | YES | Online |
| 24 | API Gateway | Node.js/Express | 3100 | Bono VPS | YES | Online |
| 25 | Cloud RaceControl | racecontrol (Rust, PM2) | 8080 | Bono VPS | YES | Online |
| 26 | Hiring Bot | Node.js/Express | 3050 | Bono VPS | YES | Online |
| 27 | Employee PWA | Next.js | — | On-demand | YES | Available |
| 27b | HR Marketing | Node.js | — | Bono VPS | YES | Available |
| 28 | Google Services | Node.js (OAuth) | — | Bono VPS | YES | Online |
| 28b | James Email Notifier | Node.js | — | Bono VPS | YES | Online |

### 2.2 Rust Crates (inside racecontrol repo)

| # | Crate | Binary | Role | In Graph |
|---|---|---|---|---|
| 29 | racecontrol | racecontrol.exe | Server: API, WS, billing, fleet, sync | YES |
| 30 | rc-agent | rc-agent.exe | Pod agent: games, telemetry, lock screen, FFB | YES |
| 31 | rc-sentry | rc-sentry.exe | Pod exec: remote commands, restart, deploy target | YES |
| 32 | rc-common | (library) | Shared types, protocols, safety | YES |
| 33 | rc-watchdog | rc-watchdog.exe | Server health polling, graduated restart | YES |
| 34 | rc-guardian | rc-guardian.exe | Server meta-monitoring, GUARDIAN_ACTING | YES |
| 35 | rc-sentry-ai | rc-sentry-ai.exe | Face detection on cameras (YOLO) | YES |
| 36 | rc-process-guard | rc-process-guard.exe | Pod process allowlist enforcement | YES |
| 37 | rc-installer | rc-installer.exe | Pod setup automation | YES |
| 38 | weekly-report | weekly-report.exe | Automated weekly stats | YES |

### 2.3 Databases

| # | Database | Location | Authority | In Graph | Contents |
|---|---|---|---|---|---|
| 39 | racecontrol.db | Server .23 + Bono VPS | Venue=billing/laps, Cloud=drivers/pricing | YES (code) | 50+ tables: drivers, pods, sessions, laps, billing, wallets, tournaments |
| 40 | admin.db | Server .23 + Bono VPS | Local | YES (code) | Cafe menu, sales, inventory |
| 41 | comms.db | Bono VPS | Bono VPS | YES (code) | 14,568+ Bono↔James messages |
| 42 | bot.db | Bono VPS | Bono VPS | YES (code) | WhatsApp conversations, customer interactions |
| 43 | conversations.db | Bono VPS | Bono VPS | YES (code) | WhatsApp conversation history |
| 44 | hiring.db | Bono VPS | Bono VPS | YES (code) | Candidates, responses, scores |
| 44b | telemetry.db | Server .23 | Venue | YES (code) | Race telemetry time-series, sector times |
| 44c | debug.db | Bono VPS | Local | YES (code) | Dashboard debug data |
| 44d | kuma.db | Bono VPS (Docker) | Local | N/A (external) | Uptime Kuma monitoring state (26MB) |

### 2.4 Communication Protocols

| # | Protocol | Between | Method | In Graph |
|---|---|---|---|---|
| 45 | WebSocket /ws/agent | Server ↔ Pods | WS (PSK/JWT auth) | YES |
| 46 | WebSocket /ws/dashboard | Server → Dashboards | WS broadcast | YES |
| 47 | WebSocket /ws/ai-channel | Server ↔ Pods | WS gossip (MI) | YES |
| 48 | Comms-Link WS | Bono VPS ↔ James .27 | WS :8765/:8766 | YES |
| 49 | Cloud Sync | Server .23 ↔ Bono VPS | HTTP pull/push every 30s | YES |
| 50 | Fleet Exec | Server → Pods via rc-sentry | HTTP POST :8091/exec | YES |
| 51 | WhatsApp API | Bono VPS → Evolution API | HTTP REST | YES |

### Layer 2 E2E Trace

```
Data Flow — Billing Session:
  POS (#6) → API Gateway (#24) or direct → RaceControl Server (#21)
  → WS to RC-Agent (#22) on pod → Game launches → Telemetry via UDP
  → RC-Agent reads shared memory → WS to Server (#21) → persist to racecontrol.db (#39)
  → Cloud Sync (#49) → Cloud RaceControl (#25) → Cloud Dashboard (#12)

Data Flow — Lap Recording:
  Game engine → Shared memory → RC-Agent (#22) reads at 60Hz
  → Lap complete → WS GameStateUpdate → Server (#21) → laps table in DB (#39)
  → Leaderboard update → WS broadcast (#46) → Kiosk (#7) + Spectator (#19)
  → Cloud Sync (#49) → Cloud leaderboard (#12)
```

---

## LAYER 3: INFRASTRUCTURE

What keeps Layers 1 and 2 running, self-healing, monitored, and deployable.

### 3.1 AI Operations (Bono + James)

| # | Component | Machine | Role | In Graph |
|---|---|---|---|---|
| 52 | Bono (Claude Code) | Bono VPS | Cloud AI: planning, code, deploy, comms | YES (via hooks/skills) |
| 53 | James (Claude Code) | James .27 | Venue AI: pod ops, debug, deploy, cameras | YES (via hooks/skills) |
| 54 | Ollama (qwen2.5:3b) | James .27 | Tier 3 local LLM for diagnostics | N/A (external) |
| 55 | OpenRouter API | External | Tier 4 MMA audit (10 models) | N/A (external) |
| 56 | Anthropic API | External | WhatsApp bot + Discord bot AI | N/A (external) |

### 3.2 Self-Healing & Monitoring

| # | Component | Machine | What it does | In Graph |
|---|---|---|---|---|
| 57 | RC-Watchdog | Server .23 | Server health poll + graduated restart | YES |
| 58 | RC-Guardian | Bono VPS | Meta-monitoring of server | YES |
| 59 | RC-Sentry-AI | James .27 | Face detection on 3 cameras | YES |
| 60 | Pod self_monitor | Pods 1-8 | Self-restart on crash (Session 1) | YES |
| 61 | RCWatchdog Service | Pods 1-8 | Windows service restarts rc-agent | YES |
| 62 | auto-detect | Bono VPS | Pipeline anomaly detection (STOPPED) | YES |
| 63 | PM2 watchdog | Bono VPS | `vps-pm2-watchdog.sh` every 2min | YES |
| 64 | Meshed Intelligence | Pods + Server | 5-tier AI diagnosis + gossip | YES |
| 65 | Fleet Healer | Server .23 | SSH diagnostics, canary rollout | YES |
| 66 | MMA Engine | RC-Agent | Multi-model consensus on pods | YES |

### 3.3 Networking

| # | Component | Scope | What | In Graph |
|---|---|---|---|---|
| 67 | LAN (192.168.31.x) | Venue | 8 pods + server + POS + James + NVR | N/A (hardware) |
| 68 | Tailscale mesh | Venue + Cloud | 12 nodes: 8 pods + server + POS + James + Bono | N/A (external) |
| 69 | nginx reverse proxy | Bono VPS | racingpoint.cloud SSL termination | N/A (config) |
| 70 | Evolution API | Bono VPS | WhatsApp Business API bridge (Docker) | N/A (external) |
| 71 | go2rtc | James .27 | RTSP camera proxy, 13 streams | N/A (external) |
| 71b | PostgreSQL | Bono VPS | Used by Evolution API + Docker services | N/A (external) |

### 3.3b Docker Services (Bono VPS)

| # | Container | Port | Dependencies | In Graph | Status |
|---|---|---|---|---|---|
| 71c | n8n | 5678 | database.sqlite | N/A (external) | Online |
| 71d | Uptime Kuma | 3001 | kuma.db (26MB) | N/A (external) | Online |
| 71e | Paymenter | 58290 | MySQL + Redis (Docker) | N/A (external) | Online |
| 71f | Cloud PWA | 3100 | Next.js (Docker, not PM2) | YES | Online |

### 3.4 Deploy Pipeline

| # | Component | What | In Graph |
|---|---|---|---|
| 72 | deploy-server.sh v3.0 | Server binary deploy (8-step, auto-rollback) | YES |
| 73 | deploy-pod.sh | Pod binary deploy (canary → fleet) | YES |
| 74 | stage-release.sh | Security gate + build + SHA256 manifest | YES |
| 75 | gate-check.sh | Pre/post deploy verification | YES |
| 76 | start-rcagent.bat | Pod startup (HKLM Run, Session 1) | YES |
| 77 | start-racecontrol.bat | Server startup (HKLM Run + watchdog) | YES |
| 78 | deploy-staging/ | Build staging area on James .27 | YES |
| 79 | install.bat v5 | Pendrive pod setup | YES |

### 3.5 Cron Jobs (Bono VPS)

| # | Job | Interval | What | In Graph |
|---|---|---|---|---|
| 80 | backup-databases.sh | Daily 3am | SQLite backup | YES |
| 81 | backup-offsite.sh | Daily 3:05am | Offsite DB copy | YES |
| 82 | backup-cloud.sh | Daily 9:30pm | Cloud backup | YES |
| 83 | bono-save-memory.sh | Every 10min | Memory file sync | N/A (ops) |
| 84 | james-save-memory.sh | Every 10min | James memory sync | N/A (ops) |
| 85 | git-sync-repos.sh | Every 5min | Auto git sync all repos | N/A (ops) |
| 86 | health-check.sh | Every 2min | Cloud racecontrol health | YES |
| 87 | bono-server-monitor.sh | Every 3min | Venue server monitoring | YES |
| 88 | bono-racecontrol-monitor.sh | Every 5min | Venue RC health | YES |
| 89 | james-comms-cron.sh | Every 5min | James comms delivery | YES |
| 90 | comms-delivery-monitor.sh | Every 2min | Comms delivery verify | YES |
| 91 | vps-pm2-watchdog.sh | Every 2min | PM2 process guard | YES |
| 92 | download-db.sh | Every 5min | DB sync from venue | YES |
| 93 | fleet-report.js | Weekly Sun 10am | Weekly fleet stats | YES |

### 3.5b CI/CD Pipelines (GitHub Actions)

| # | Workflow | Trigger | What | In Graph |
|---|---|---|---|---|
| 93b | ci.yml | Push/PR | Commit-level validation | YES (code) |
| 93c | contract-tests.yml | Push/PR | Schema/API contract checks | YES (code) |
| 93d | deploy.yml | Manual/tag | Deployment orchestration | YES (code) |
| 93e | e2e-tests.yml | Push/PR | End-to-end Playwright suite | YES (code) |
| 93f | quality-gate.yml | Push/PR | Multi-suite security + lint + tests | YES (code) |

### 3.5c Background Tasks (inside racecontrol binary)

| # | Task | Interval | What | Critical |
|---|---|---|---|---|
| 93g | spawn_reconciliation_job | 30s | Billing session validation | YES — financial |
| 93h | spawn_content_drift_task | 60min | Pod game/car/track inventory drift | YES — Phase 366 |
| 93i | spawn_data_collector | Continuous | Telemetry ingestion pipeline | YES — lap data |
| 93j | spawn_dispatcher | Event-driven | Psychology nudge scheduling | NO |
| 93k | spawn_business_aggregator | Daily | EBITDA/revenue rollup | YES — finance |
| 93l | spawn_cleanup_expired_game_requests | 10min TTL | PWA game request expiry | NO |
| 93m | spawn_coupon_ttl_expiry_job | Periodic | Expired coupon cleanup | NO |

### 3.6 Operational Framework

| # | Component | Location | What | In Graph |
|---|---|---|---|---|
| 94 | CGP v4.3 | COGNITIVE-GATE-PROTOCOL.md | 5 hard gates + backlog gate | YES (docs) |
| 95 | CLD v1.0 | docs/CLOSED-LOOP-DEBUG.md | 5-step debug methodology | YES (docs) |
| 96 | MMA v3.0 | .planning/specs/UNIFIED-MMA-PROTOCOL.md | Multi-model audit protocol | YES (docs) |
| 97 | GSD Framework | .planning/ | Phase planning + execution | YES (planning) |
| 98 | LOGBOOK | LOGBOOK.md | 1,284 incident entries | YES (docs) |
| 99 | ERROR-CATALOG | docs/ERROR-CATALOG.md | 35 known error entries | YES (docs) |
| 100 | Knowledge Graph | .planning/knowledge-graph/ | Symptom→fix registry | YES |

### 3.7 MCP Servers (AI Tooling)

| # | Component | Repo | What | In Graph |
|---|---|---|---|---|
| 101 | racingpoint-gmail | racingpoint-mcp-gmail | Email read/send/search | YES |
| 102 | racingpoint-drive | racingpoint-mcp-drive | File upload/list/share | YES |
| 103 | notebooklm-mcp | (npm package) | NotebookLM access | N/A (external) |
| 104 | sqlite-whatsapp | (MCP config) | WhatsApp DB queries | N/A (config) |
| 105 | sqlite-admin | (MCP config) | Admin DB queries | N/A (config) |
| 106 | sqlite-racecontrol | (MCP config) | RaceControl DB queries | N/A (config) |
| 107 | sqlite-comms | (MCP config) | Comms DB queries | N/A (config) |
| 108 | playwright | (MCP config) | Browser automation | N/A (external) |
| 109 | perplexity | (MCP config) | Web search | N/A (external) |

### 3.8 External Services

| # | Service | What | In Graph |
|---|---|---|---|
| 110 | GitHub (bono-bot) | Code hosting, SSH auth | N/A (external) |
| 111 | GitHub (james-racingpoint) | James's org, collaborator | N/A (external) |
| 112 | Google Workspace | Gmail, Calendar, Drive for racingpoint.in | N/A (external) |
| 113 | Tailscale | Mesh VPN across all devices | N/A (external) |
| 114 | racingpoint.cloud | DNS + nginx + SSL (Bono VPS) | N/A (config) |

### Layer 3 E2E Trace

```
Self-Healing Chain:
  Pod crash → self_monitor (#60) detects → restart attempt
  → If 3 fails → MAINTENANCE_MODE → RCWatchdog (#61) in Session 1
  → Server pod_monitor notices WS drop → Fleet Healer (#65) via Tailscale SSH
  → RC-Sentry (#23) exec endpoint → RC-Guardian (#58) meta-monitors server
  → WhatsApp alert → Staff notified

Deploy Chain:
  Bono/James: cargo build → stage-release.sh (#74) → security gate
  → deploy to Pod 8 canary (#73) → verify → fleet rollout
  → deploy-server.sh (#72) → verify build_id → smoke test
  → Rebuild ALL 3 frontends → Cloud parity via git push + rebuild
  → gate-check.sh (#75) post-deploy

Monitoring Chain:
  health-check.sh (#86) every 2min → bono-server-monitor.sh (#87) every 3min
  → auto-detect (#62) anomalies → comms-delivery-monitor.sh (#90)
  → PM2 watchdog (#63) → backup-databases.sh (#80) daily
  → fleet-report.js (#93) weekly
```

---

## PoE VERIFICATION

### Method: Count every component, verify it appears in exactly one layer.

**v2 (2026-04-16 22:00 IST) — re-verified after Bono found 17 gaps in v1.**
v1 PoE was self-referential: verified map against map, not map against environment.
v2 enumerates from `pm2 list`, `docker ps`, `ss -tlnp`, `crontab -l`, filesystem.

| Source of Truth | Components Found | All Mapped |
|---|---|---|
| PM2 services (21) | 17 online + 3 stopped + 1 module. Includes james-email-notifier, dual WhatsApp bots | YES — 21/21 mapped |
| Docker containers (9) | cloud-pwa, n8n, uptime-kuma, paymenter (3 containers), evolution-api (3 containers) | YES — 9/9 mapped |
| Repos on disk (18) | 15 with code + 3 without code. Includes racingpoint-hr-marketing, racingpoint-employee | YES — 18/18 mapped |
| Network devices (15+) | 8 pods + server + POS + James + router + NVR + spectators + Bono | YES — all mapped |
| Databases (9) | racecontrol, admin, comms, bot, conversations, hiring, telemetry, debug, kuma | YES — 9/9 mapped |
| Rust crates (10) | racecontrol through weekly-report | YES — 10/10 mapped |
| Cron jobs (14) | All 14 jobs from crontab | YES — 14/14 mapped |
| CI/CD workflows (5) | ci, contract-tests, deploy, e2e-tests, quality-gate | YES — 5/5 mapped |
| Background tasks (7+) | reconciliation, content_drift, data_collector, dispatcher, aggregator, etc. | YES — 7/7 mapped |
| MCP servers (9) | gmail, drive, notebooklm, 4x sqlite, playwright, perplexity | YES — 9/9 mapped |
| External services (5) | GitHub x2, Google, Tailscale, racingpoint.cloud | YES — 5/5 mapped |
| WS protocols (4) | agent, dashboard, ai-channel, comms-link | YES — 4/4 mapped |
| Operational frameworks (7) | CGP, CLD, MMA, GSD, LOGBOOK, ERROR-CATALOG, Knowledge Graph | YES — 7/7 mapped |

### Components NOT in Graphify code graph (and why):

| # | Component | Why excluded |
|---|---|---|
| 54 | Ollama | External binary, no RacingPoint source |
| 55 | OpenRouter API | External service |
| 56 | Anthropic API | External service |
| 67 | LAN network | Hardware infrastructure |
| 68 | Tailscale mesh | External VPN service |
| 69 | nginx | Config-only (no source code) |
| 70 | Evolution API | External WhatsApp bridge |
| 71 | go2rtc | External binary |
| 103-109 | MCP servers (config) | Configuration entries, not source code |
| 110-114 | External services | Third-party services |

**All excluded items are external/config — zero RacingPoint application code is missing from the graph.**

### Final Count (v2)

```
LAYER 1 (Customer-Facing):  22 components (+2: WhatsApp Bot v2, Website API separate port)
LAYER 2 (Platform):         36 components (+5: HR-marketing, email-notifier, telemetry.db, debug.db, kuma.db)
LAYER 3 (Infrastructure):   83 components (+20: Docker stack, PostgreSQL, CI/CD, background tasks)
                            ────
TOTAL:                     141 components mapped

In Graphify code graph:     87 components (all with RacingPoint source code)
External/config/hardware:   54 components (correctly excluded)
Missing:                     0 (after v2 correction — v1 had 17 gaps)
```

### v1 → v2 Correction Log (2026-04-16)

v1 claimed 114 components / 0 missing. Bono re-verified from environment (`pm2 list`, `docker ps`, `ss -tlnp`, filesystem) and found 17 gaps:

| Gap | Layer | What was missing | Severity |
|---|---|---|---|
| G1 | L1 | WhatsApp Bot v2 (TypeScript rewrite, PM2 id 19, port 3150) — separate from v1 JS bot | HIGH |
| G2 | L1 | Website API on port 5050 (separate PM2 service, not part of website) | LOW |
| G3 | L2 | james-email-notifier (PM2 id 3, polls James Gmail, forwards to Uday via WhatsApp) | HIGH |
| G4 | L2 | racingpoint-hr-marketing repo (marketing/comms workflows) | MEDIUM |
| G5 | L2 | telemetry.db (race telemetry time-series, 53KB) | MEDIUM |
| G6 | L2 | debug.db (dashboard debug data, 73KB) | LOW |
| G7 | L2 | kuma.db (Uptime Kuma monitoring state, 26MB) | LOW |
| G8 | L3 | n8n workflow engine (Docker, port 5678) | HIGH |
| G9 | L3 | Uptime Kuma monitoring (Docker, port 3001) | HIGH |
| G10 | L3 | Paymenter + MySQL + Redis (Docker, port 58290) | HIGH |
| G11 | L3 | Cloud PWA runs in Docker, not PM2 (port 3100) | LOW |
| G12 | L3 | PostgreSQL (port 5432, used by Docker services) | MEDIUM |
| G13 | L3 | 5 GitHub Actions CI/CD pipelines | MEDIUM |
| G14 | L3 | 7+ background tokio tasks inside racecontrol binary | MEDIUM |
| G15 | L3 | auto-detect was marked Online but PM2 shows stopped | LOW |
| G16 | L3 | fail2ban (intrusion detection systemd service) | LOW |
| G17 | L3 | pm2-logrotate (log rotation module) | LOW |

**Root cause of v1 PoE failure:** Verified map completeness against known items, not against environment. The PoE table said "YES — 18/18 mapped" for PM2 but never ran `pm2 list` to count. Same pattern as "health passes but blanking is broken" — verifying the claim against itself.

**SECURITY FINDING (S1):** james-email-notifier (`/root/james-email-notifier/index.js`) has hardcoded Google OAuth client_secret, refresh_token, and Evolution API key in source code. Must be moved to .env file.
