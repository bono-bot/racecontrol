# Roadmap: Racing Point Operations (Unified)

## Completed Milestones

<details>
<summary>v1.0 RaceControl HUD & Safety — 5 phases, 15 plans (Shipped 2026-03-13)</summary>

See [milestones/v1.0-ROADMAP.md](milestones/v1.0-ROADMAP.md) for full phase details and plan breakdown.

Phases: State Wiring & Config Hardening → Watchdog Hardening → WebSocket Resilience → Deployment Pipeline Hardening → Blanking Screen Protocol

</details>

<details>
<summary>v2.0 Kiosk URL Reliability — 6 phases, 12 plans (Shipped 2026-03-14)</summary>

Phases: Diagnosis → Server-Side Pinning → Pod Lock Screen Hardening → Edge Browser Hardening → Staff Dashboard Controls → Customer Experience Polish

</details>

<details>
<summary>v3.0 Leaderboards, Telemetry & Competitive — Phases 12–13.1 complete, 14–15 paused (2026-03-15)</summary>

Phases complete: Data Foundation → Leaderboard Core → Pod Fleet Reliability (inserted)
Phases paused: Events and Championships (Phase 14), Telemetry and Driver Rating (Phase 15) — deferred until v4.0 completes.

</details>

<details>
<summary>v4.0 Pod Fleet Self-Healing — Phases 16–22 (Shipped 2026-03-16)</summary>

Phases: Firewall Auto-Config → WebSocket Exec → Startup Self-Healing → Watchdog Service → Deploy Resilience → Fleet Health Dashboard → Pod 6/7/8 Recovery and Remote Restart Reliability

</details>

<details>
<summary>v4.5 AC Launch Reliability — Phases 28–32 (Shipped 2026-03-16)</summary>

Phases: Billing-Game Lifecycle → Game Crash Recovery → Launch Resilience → Multiplayer Server Lifecycle → Synchronized Group Play

Key: billing↔game lifecycle wired end-to-end; CM fallback diagnostics; acServer.exe auto-start/stop on booking/billing; kiosk self-serve multiplayer with per-pod PINs; coordinated group launch + continuous race mode + join failure recovery.

</details>

<details>
<summary>v5.0 RC Bot Expansion — Phases 23–26 (Shipped 2026-03-16)</summary>

Phases: Protocol Contract + Concurrency Safety → Crash, Hang, Launch + USB Bot Patterns → Billing Guard + Server Bot Coordinator → Lap Filter, PIN Security, Telemetry + Multiplayer

</details>

<details>
<summary>v5.5 Billing Credits — Phases 33–35 (Shipped 2026-03-17)</summary>

Phases: DB Schema + Billing Engine → Admin Rates API → Credits UI

Key: billing_rates DB table + non-retroactive additive algorithm + in-memory rate cache; four CRUD endpoints for staff rate management; every user-facing screen replaced rupees with credits.

</details>

<details>
<summary>Cloud Services v1.0 — Bots & API Gateway — 9 commits, 4 repos (Shipped 2026-03-07, Bono)</summary>

Repos: racingpoint-api-gateway, racingpoint-whatsapp-bot, racingpoint-discord-bot, racingpoint-google

Features: Express.js API gateway merging bot data + racecontrol + Calendar; WhatsApp bot with automated booking + Claude API + direct mode fallback; Discord bot with modal booking; Google shared services (Calendar attendees, Gmail reply, OAuth refresh).

No GSD phases — retroactively catalogued from commit history.

</details>

<details>
<summary>Admin Dashboard v1.0 — Staff Operations Panel — 14 commits (Shipped 2026-03-08, Bono + James)</summary>

Repo: racingpoint-admin (Next.js/TypeScript)

Features: Full admin dashboard (cafe, inventory, sales, purchases, finance); receipt scanner + bank statement matching; waivers with signature viewing; HR + hiring + marketing pages; Racing Point rebrand; Docker support; kiosk control with per-pod blanking; sessions with staff attribution; wallet log; server-side rc-core proxy.

No GSD phases — retroactively catalogued from commit history.

</details>

<details>
<summary>Comms Link v1.0 — James-Bono Communication — 8 phases, 14 plans (Shipped 2026-03-12)</summary>

Repo origin: comms-link | Archive: archive/comms-link-v1.0/

Phases: WebSocket Connection → Reconnection Reliability → Heartbeat → Watchdog Core → Watchdog Hardening → Alerting → Logbook Sync → Coordination & Daily Ops

Key: persistent WS with PSK auth, auto-reconnect with queue flush, Claude watchdog with cooldown DI, bidirectional LOGBOOK.md sync, IST-windowed daily summaries, WhatsApp/email alerts.

</details>

<details>
<summary>Comms Link v2.0 — Reliable AI-to-AI Communication — 6 phases, 14 plans (Shipped 2026-03-20)</summary>

Repo origin: comms-link | Archive: archive/comms-link-v2.0/

Phases: Protocol Foundation → Process Supervisor → Reliable Delivery Wiring → Remote Execution → Observability → Graceful Degradation

Key: ACK protocol with sequence numbers + WAL-backed message queue; mid-session process supervisor + Task Scheduler watchdog-of-watchdog; bidirectional task routing with correlation IDs; 13-command remote execution with 3-tier approval (auto/notify/approve); MetricsCollector + /relay/metrics endpoint; REALTIME > EMAIL_FALLBACK > OFFLINE_QUEUE graceful degradation. 437 tests, 34 requirements.

</details>

<details>
<summary>AC Launcher v1.0 — Full AC Launch Experience — 9 phases, 20 plans (Shipped 2026-03-14)</summary>

Repo origin: ac-launcher | Archive: archive/ac-launcher-v1.0/

Phases: Session Types & Race Mode → Difficulty Tiers → Billing Synchronization → Safety Enforcement → Content Validation & Filtering → Mid-Session Controls → Curated Presets → Staff/PWA Integration → Multiplayer Enhancement

Key: 5 difficulty tiers, billing synced to in-game start, safety presets enforced, content validation, mid-session controls, curated presets, multiplayer with lobby enrichment.

</details>

<details>
<summary>Ops Toolkit v1.0 — Fleet Management CLI — 5 phases, Phase 1 complete (In Progress, started 2026-03-17)</summary>

Repo origin: deploy-staging (ops.bat + ops.conf.bat)

Phases: Foundation & Safety (done) → Health & Status → Pod Deploy & Operations → Server Operations → Build & Polish

Key: Single ops.bat entry point replacing 310 scattered scripts; centralized config (ops.conf.bat); safety blocklist prevents pod binary execution on James; dual-mode (menu + CLI); 27 requirements across foundation, pod ops, server ops, and build automation.

</details>

## Archived Milestones

<details>
<summary>v6.0 Salt Fleet Management — Phases 36–40 (DEPRECATED 2026-03-25, superseded by v22.0)</summary>

**Original goal:** Replace pod-agent/remote_ops HTTP with SaltStack. BLOCKED on Intel VT-x disabled in BIOS (server CPU: Intel Core Ultra 9 285K). v22.0 Feature Management & OTA Pipeline supersedes all Salt config distribution and deploy orchestration. Remaining Salt value (remote shell exec) is covered by existing comms-link relay + rc-agent exec.

**Phases 36-40:** All NOT STARTED. No code, no binaries, no infrastructure deployed. Only Phase 36 directory exists (WSL2 Infrastructure — empty planning).

**Decision:** DEPRECATED. Do not implement. v22.0 covers all use cases.

</details>

## Current Milestone

### v7.0 E2E Test Suite (Phases 41–44)

**Milestone Goal:** Comprehensive end-to-end test coverage for the full kiosk→server→agent→game launch pipeline — Playwright browser tests for all 5 sim wizard flows, curl-based API pipeline tests for billing/launch/game-state lifecycle, deploy verification for binary swap and port conflict detection, and a single master `run-all.sh` entry point reusable for future services (POS, Admin Dashboard).

### v8.0 RC Bot Autonomy (Phases 45–49)

**Milestone Goal:** Raise rc-agent autonomy from 6/10 to 8/10 — fix the CLOSE_WAIT socket leak causing 5/8 pods to self-relaunch every 5 minutes, install panic hooks for FFB safety on crash, deploy local Ollama (qwen3:0.6b + rp-debug model) to all 8 pods so AI diagnosis is instant and offline-capable, add dynamic server-fetched kiosk allowlist to eliminate the #1 manual intervention, auto-end orphaned billing sessions, and auto-reset pods after session end.

### v9.0 Tooling & Automation (Phases 51–56)

**Milestone Goal:** Install the tooling layer that makes James+Claude more effective — CLAUDE.md project context + 5 custom skills so Claude always knows pod IPs and naming conventions, MCP servers for Google Workspace (Gmail/Sheets/Calendar) and racecontrol REST API access, deployment automation so staging auto-starts and every deploy runs a verified canary-first flow, structured JSON logs in racecontrol and rc-agent with error-rate email alerts, Netdata fleet monitoring on server and all 8 pods, WhatsApp P0 alerts to Uday, and a weekly fleet uptime report.

### v17.0 Cloud Platform (Phases 120–129)

**Milestone Goal:** Deploy three web properties (customer PWA, admin panel, live dashboard) to racingpoint.cloud subdomains with remote booking + PIN-based zero-staff game launch, hardened cloud-local sync, CI/CD, and health monitoring. Planning artifacts in `pwa/.planning/` (PWA phases 1-10 → unified 120-129). **3/10 phases complete (30%).**

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

**Standing Rules Gate (v22.0):** All future phases across all milestones MUST run `bash test/gate-check.sh --pre-deploy` before shipping. No phase can be marked complete without passing the standing rules gate. This is enforced by the OTA pipeline automatically for binary deploys, and manually for documentation/planning-only phases.

- ~~**Phase 36-40: v6.0 Salt Fleet Management**~~ — DEPRECATED 2026-03-25, superseded by v22.0. See Archived Milestones.
- [x] **Phase 41: Test Foundation** - Shared shell library, pod IP map, Playwright config, and cargo-nextest configured — the skeleton every other test script sources (completed 2026-03-18)
- [x] **Phase 42: Kiosk Source Prep + Browser Smoke** - data-testid attributes added to kiosk wizard components, pre-test cleanup fixture built, page smoke tests confirm all routes load in a real browser with no SSR/JS errors (completed 2026-03-18)
- [x] **Phase 43: Wizard Flows + API Pipeline Tests** - All 5 sim wizard flows tested per-step in Playwright, API pipeline tests for billing lifecycle and game state, per-game launch validation with PID check, Steam dialog dismissal (completed 2026-03-18)
- [x] **Phase 44: Deploy Verification + Master Script** - Deploy verify script (binary swap, port conflict, agent reconnect), fleet health validation, run-all.sh phase-gated orchestrator, AI debugger error routing (completed 2026-03-18)
- [x] **Phase 45: CLOSE_WAIT Fix + Connection Hygiene** - Fix remote_ops HTTP server socket leak causing 100-134 CLOSE_WAIT sockets on 5/8 pods, fix fleet_health.rs client connection reuse, add SO_REUSEADDR to UDP sockets, mark all sockets non-inheritable, separate health endpoint from exec slot pool. **E2E (v7.0):** Add `tests/e2e/fleet/close-wait.sh` sourcing lib/common.sh + lib/pod-map.sh — verify CLOSE_WAIT count <5 on all pods after 30min soak, verify no 429 slot exhaustion on /health (completed 2026-03-19)
- [x] **Phase 46: Crash Safety + Panic Hook** - Install std::panic::set_hook() to zero FFB + show error lock screen + log crash before exit, check all server port bindings at startup (remote_ops :8090, lock screen :18923, overlay :18925), FFB zero retry logic (3x attempts with escalation), startup health verification message to server. **E2E (v7.0):** Add `tests/e2e/fleet/startup-verify.sh` — after agent restart, verify BootVerification message received by server within 30s, all ports bound, correct build_id (completed 2026-03-19)
- [x] **Phase 47: Local LLM Fleet Deployment** - Ollama + qwen3:0.6b + rp-debug model installed and verified on all 8 pods, rc-agent TOML pointing to localhost:11434, ai_debugger feeds Windows Event Viewer + rc-bot-events.log to LLM (PodErrorContext), Ollama timeout 120s→30s. **E2E (v7.0):** Add `tests/e2e/fleet/ollama-health.sh` — verify `curl localhost:11434/api/tags` returns rp-debug on all 8 pods, verify `ollama generate` returns valid response <5s on each pod
 (completed 2026-03-19)
- [x] **Phase 48: Dynamic Kiosk Allowlist** - Server endpoint GET /api/v1/config/kiosk-allowlist, admin panel UI for adding/removing allowed processes, rc-agent fetches allowlist on startup + every 5 min, merges with hardcoded baseline, LLM-based process classifier for unknown processes (ALLOW/BLOCK/ASK). **E2E (v7.0):** Add `tests/e2e/api/kiosk-allowlist.sh` — curl CRUD on allowlist API, verify rc-agent picks up new process within 5min, Playwright test for admin panel UI (completed 2026-03-19)
- [x] **Phase 49: Session Lifecycle Autonomy** - Auto-end orphaned billing sessions after configurable threshold (TOML: auto_end_orphan_session_secs), auto-reset pod to idle 30s after session end, game crash pauses billing with auto-resume on relaunch (max 2 retries before auto-end), fast WS reconnect path (skip relaunch if reconnect succeeds within 30s). **E2E (v7.0):** Add `tests/e2e/api/session-lifecycle.sh` — create billing session, verify auto-end after timeout, verify pod reset to idle, verify billing pause on simulated crash (completed 2026-03-19)
- [x] **Phase 50: LLM Self-Test + Fleet Health** - self_test.rs with 18 deterministic probes (WS, lock screen, remote ops, overlay, debug server, 5 UDP ports, HID, Ollama, CLOSE_WAIT, single instance, disk, memory, shader cache, build_id, billing state, session ID, GPU temp, Steam), local LLM verdict (HEALTHY/DEGRADED/CRITICAL) with correlation and auto-fix recommendations, server /api/v1/pods/{id}/self-test endpoint, expanded auto-fix patterns 8-14 (DirectX, shader cache, memory, DLL, Steam, performance, network). **E2E (v7.0):** Add `tests/e2e/fleet/pod-health.sh` — trigger self-test on all 8 pods via API, assert all HEALTHY, wire into run-all.sh as final phase gate (completed 2026-03-19)

- [x] **Phase 51: CLAUDE.md + Custom Skills** - Project context file + 5 slash commands so Claude auto-loads venue context and James can trigger structured workflows from any session (completed 2026-03-20)
- [x] **Phase 52: MCP Servers** - Google Workspace MCP (Gmail, Sheets, Calendar) and rc-ops-mcp (racecontrol REST API) wired into Claude Code (completed 2026-03-20)
- [x] **Phase 53: Deployment Automation** - Staging HTTP server and webterm auto-start on boot, post-deploy verify script, canary-first gate enforced (completed 2026-03-20)
- [x] **Phase 54: Structured Logging + Error Rate Alerting** - racecontrol and rc-agent emit structured JSON logs with daily rotation; error-rate email alerting (completed 2026-03-20)
- [x] **Phase 55: Netdata Fleet Deploy** - Netdata agent on server (.23) and all 8 pods via rc-agent :8090 exec, live system metrics dashboards
 (completed 2026-03-20)
- [x] **Phase 56: WhatsApp Alerting + Weekly Report** - P0 events trigger WhatsApp to Uday; weekly automated email report with sessions, uptime %, credits, incidents (completed 2026-03-20)

## v10.0 Conspit Link — Full Capability Unlock

Fix stuck-rotation safety bug, unlock all Conspit Link 2.0 features (per-game FFB presets, auto game switching, telemetry dashboards, shift lights, RGB), and automate fleet-wide config management via rc-agent.

- [x] **Phase 57: Session-End Safety** - Fix stuck-rotation bug: close ConspitLink before HID commands, use fxm.reset + axis.idlespring (not estop), gradual force ramp, auto-restart ConspitLink after
- [ ] **Phase 58: ConspitLink Process Hardening** - Harden watchdog with crash-count tracking, graceful restart (never taskkill /F), config backup + JSON integrity verification, minimize survives restarts
- [ ] **Phase 59: Auto-Switch Configuration** - Fix broken auto game detection by placing Global.json at C:\RacingPoint\ (runtime path), update GameToBaseConfig.json mappings to venue presets
- [ ] **Phase 60: Pre-Launch Profile Loading** - rc-agent pre-loads correct preset BEFORE game launch (not relying solely on ConspitLink auto-detect), safe fallback for unrecognized games
- [ ] **Phase 61: FFB Preset Tuning** - Create venue-tuned .Base presets for AC (900deg), F1 25 (360deg), ACC/ACE, AC Rally (~800deg) starting from Yifei Ye pro presets, store in version control
- ~~**Phase 62: Fleet Config Distribution**~~ — SUPERSEDED by v22.0 Config Push (CP-01 to CP-06). v22.0 provides WebSocket-based config push with schema validation, per-pod queuing, offline delivery via sequence-number ack, and audit logging. Do not implement a separate config distribution system.
- [ ] **Phase 63: Fleet Monitoring** - rc-agent reports active preset, config hashes, firmware version per pod; racecontrol dashboard shows fleet config status at a glance
- [ ] **Phase 64: Telemetry Dashboards** - Enable wheel LCD showing RPM/speed/gear for all 4 venue games, verify GameSettingCenter.json telemetry fields, document UDP port chain
- [ ] **Phase 65: Shift Lights & RGB Lighting** - Auto RPM shift lights for AC/ACC, manual RPM thresholds for F1 25/AC Rally, RGB button lighting tied to telemetry (DRS, ABS, TC, flags)
## v10.0 Connectivity & Redundancy

Make server .23 IP permanently stable, establish reliable James↔Server↔Bono remote exec paths, sync venue config to cloud, and deliver automatic pod failover to Bono's VPS when .23 goes down — with self-healing failback when .23 recovers.

- [x] **Phase 67: Config Sync** - racecontrol.toml changes detected by SHA-256 hash, sanitized (credentials/paths stripped), and pushed to Bono via comms-link sync_push; Bono applies TOML-based config (venue/pods/branding) to cloud racecontrol. Billing rates and game catalog already synced via DB-level cloud_sync. (completed 2026-03-20)
 (completed 2026-03-20)
- [x] **Phase 67: Config Sync** - racecontrol.toml changes detected by sha2 hash, sanitized, and pushed to Bono via comms-link sync_push; Bono applies config to cloud racecontrol (completed 2026-03-20)
- [x] **Phase 68: Pod SwitchController** - rc-agent CoreConfig gains failover_url; WS reconnect loop uses Arc<RwLock<String>> for runtime URL switching; SwitchController AgentMessage triggers switch without restart; self_monitor suppression guard prevents relaunch during intentional failover
 (completed 2026-03-20)
- [x] **Phase 69: Health Monitor & Failover Orchestration** - James probes .23 every 5s; 3-down/2-up hysteresis + 60s minimum outage window gates auto-failover; James sends task_request to Bono to activate cloud primary; racecontrol broadcasts SwitchController to all pods; pods confirm .23 unreachable before switching; Uday notified via email + WhatsApp (completed 2026-03-21)
- [x] **Phase 70: Failback & Data Reconciliation** - James detects .23 recovery (2-up threshold); cloud sessions merged to local DB before .23 resumes primary; racecontrol broadcasts SwitchController with original URL; Uday notified on failback (completed 2026-03-21)

<details>
<summary>v11.0 Agent & Sentry Hardening — 4 phases, 10 plans (Shipped 2026-03-21)</summary>

Phases: rc-common Foundation + rc-sentry Core Hardening → rc-sentry Endpoint Expansion + Integration Tests → Critical Business Tests → rc-agent Decomposition

See [milestones/v11.0-ROADMAP.md](milestones/v11.0-ROADMAP.md) for full phase details.

</details>

## v12.0 Operations Security

Lock down the Racing Point operations stack — audit all exposed endpoints and PII, enforce JWT auth on billing/session APIs, add admin PIN gate, HTTPS for browser traffic, harden kiosk escape vectors, encrypt customer PII at rest, and add audit trails for compliance. Gradual hardening: biggest holes first, then layer defenses.

- [x] **Phase 75: Security Audit & Foundations** - Inventory all exposed endpoints, trace PII locations, move secrets to env vars, auto-generate JWT key (completed 2026-03-20)
- [x] **Phase 76: API Authentication & Admin Protection** - JWT enforcement on all sensitive routes, admin PIN gate with argon2, rate limiting, bot auth, pod HMAC, session integrity (completed 2026-03-20)
- [x] **Phase 77: Transport Security** - HTTPS for PWA/admin browser traffic, self-signed LAN certs, Let's Encrypt for cloud, security response headers
 (completed 2026-03-20)
- [x] **Phase 78: Kiosk & Session Hardening** - Chrome lockdown, hotkey blocking, USB disable, session-scoped tokens, anomaly auto-pause with WhatsApp alert (completed 2026-03-21)
- [x] **Phase 79: Data Protection** - AES-256-GCM on PII columns, deterministic phone hash for lookups, log redaction, customer data export/deletion (completed 2026-03-21)
- [x] **Phase 80: Audit Trail & Defense in Depth** - Admin action logging, WhatsApp alerts on sensitive actions, PIN rotation alerts, cloud sync HMAC signing (completed 2026-03-21)

## v13.0 Multi-Game Launcher

Launch games other than AC (F1 25, iRacing, AC EVO, EA WRC, LMU) from kiosk/PWA with PlayableSignal-gated billing, per-game telemetry capture, and multi-game leaderboard integration. Extends existing SimAdapter trait and GameProcess — zero new crate dependencies.

**v22.0 integration:** Future game additions should use Cargo feature gates (CF-01) for telemetry modules and feature flags (FF-01) for per-pod game enablement. AC EVO telemetry (Phase 86) already uses a compile-time feature flag — future games should use runtime feature flags via the v22.0 flag registry instead.

- [x] **Phase 81: Game Launch Core** - Launch profiles, process monitoring, kiosk integration, crash recovery for 5 games (completed 2026-03-21)
- [x] **Phase 82: Billing and Session Lifecycle** - PlayableSignal per game, billing accuracy, per-game rates, clean lifecycle (completed 2026-03-21)
- [x] **Phase 83: F1 25 Telemetry** - Extend existing F1 25 UDP adapter for LapCompleted events with sector splits (completed 2026-03-21)
- [x] **Phase 84: iRacing Telemetry** - Shared memory reader with session transition handling and pre-flight checks (completed 2026-03-21)
- [x] **Phase 85: LMU Telemetry** - rFactor 2 shared memory reader for Le Mans Ultimate lap data (completed 2026-03-21)
- [x] **Phase 86: AC EVO Telemetry** - Best-effort shared memory reader using ACC struct layout, feature-flagged (completed 2026-03-21)
- ~~**Phase 87: EA WRC Telemetry**~~ — ARCHIVED 2026-03-25: No EA WRC units at venue, no near-term demand. If WRC is added, implement UDP adapter at that time.
- [x] **Phase 88: Leaderboard Integration** - Multi-game lap storage, track name normalization, endpoint updates (completed 2026-03-21)

## v14.0 HR & Marketing Psychology

Embed 12 behavioral psychology frameworks into RacingPoint's existing systems — centralized psychology engine with notification throttling, customer progression (driving passport, badges), peak-end session design, retention loops (streaks, variable rewards), community rituals (Discord), pricing psychology (anchoring, scarcity), staff gamification (opt-in leaderboards, badges, challenges), and HR/hiring enhancements (SJTs, Cialdini campaigns).

- [x] **Phase 89: Psychology Foundation** - Notification budget, psychology engine module, DB schema, and badge criteria storage (completed 2026-03-21)
- [x] **Phase 90: Customer Progression** - Driving passport with track/car collections, badge system, and profile showcase
 (completed 2026-03-21)
- [x] **Phase 91: Session Experience** - PB confetti celebrations, peak-end session reports, and real-time PB toasts (completed 2026-03-21)
- [x] **Phase 92: Retention Loops** - Visit streaks, PB-beaten notifications, variable rewards, and loss-framed membership nudges
 (completed 2026-03-21)
- [x] **Phase 93: Community & Tribal Identity** - Discord weekly rituals, record alerts, and RacingPoint Driver identity language (completed 2026-03-21)
- [x] **Phase 94: Pricing & Conversion** - Anchoring/decoy pricing display, real-time pod scarcity, commitment ladder, and social proof (completed 2026-03-24)
- [x] **Phase 95: Staff Gamification** - Opt-in performance leaderboard, skill badges, team challenges, and peer recognition (completed 2026-03-24)
- [x] **Phase 96: HR & Hiring Psychology** - Hiring bot SJTs, Cialdini campaign templates, review nudge optimization, and employee recognition (completed 2026-03-24)

## v17.0 Cloud Platform (Phases 120–129)

Deploy three existing web properties (customer PWA, admin panel, live dashboard) to racingpoint.cloud subdomains, add remote booking with PIN-based zero-staff game launch, and harden the cloud-local sync layer for production reliability.

**Planning artifacts:** `pwa/.planning/` (separate GSD project, phases numbered 1-10 there → mapped to 120-129 here)

- [ ] **Phase 120: Cloud Infrastructure** - DNS, Caddy reverse proxy, Docker Compose, firewall, and swap on VPS
- [ ] **Phase 121: API + PWA Cloud Deploy** - Customer PWA and cloud API live at racingpoint.cloud with HTTPS
- [x] **Phase 122: Sync Hardening** - Reservations table, wallet authority, anti-loop tags, sync health endpoint (completed 2026-03-21)
- [x] **Phase 123: Remote Booking + PIN Generation** - Customer books from phone, receives 6-char PIN via WhatsApp (completed 2026-03-21)
- [x] **Phase 124: Kiosk PIN Launch** - Customer enters PIN at venue kiosk, pod assigned, game auto-launches (completed 2026-03-21)
- [ ] **Phase 125: Admin Panel Cloud Deploy** - Business admin panel live at admin.racingpoint.cloud
- [ ] **Phase 126: Dashboard Cloud Deploy** - Live ops dashboard at dashboard.racingpoint.cloud
- [ ] **Phase 127: CI/CD Pipeline** - Automated build and deploy on push to main. **v22.0 integration:** Use OTA-08 (deploy state machine) for both cloud and local deploy orchestration — shared pipeline architecture with wave-based rollout, health gates, and auto-rollback. Standing rules gate (gate-check.sh) required before any deploy.
- [ ] **Phase 128: Health Monitoring + Alerts** - Container health checks with WhatsApp alerts on failure
- [ ] **Phase 129: Operational Hardening** - Split-brain handling, rate limiting, production edge cases

## Phase Details

### Phase 36: WSL2 Infrastructure
**NOTE (v22.0):** v6.0 Salt Fleet Management scope overlaps significantly with v22.0 — config push (CP-01-06) supersedes Salt's config distribution, OTA pipeline (OTA-01-10) supersedes Salt's deploy orchestration. Remaining Salt value: remote shell exec on pods (not covered by v22.0 WebSocket config push). Review during Phase 182 whether Salt phases 36-40 should be narrowed to remote exec only or fully deprecated in favor of v22.0.
**Goal**: James's machine (.27) runs a reachable salt-master — WSL2 Ubuntu 24.04 with mirrored networking so pods on 192.168.31.x can reach the master directly, both firewall layers open (Windows Defender + Hyper-V), salt-api running for racecontrol server integration, and the full stack auto-starts on Windows boot
**Depends on**: Phase 35 (v5.5 Credits UI — last completed phase)
**Requirements**: INFRA-01, INFRA-02, INFRA-03, INFRA-04, INFRA-05
**Success Criteria** (what must be TRUE):
  1. `Test-NetConnection 192.168.31.27 -Port 4505` from Pod 8 returns `TcpTestSucceeded: True` — WSL2 mirrored mode is active and the Hyper-V firewall layer is open
  2. `salt-call --local test.ping` inside WSL2 Ubuntu returns True — salt-master process is running and responding
  3. A curl request to `http://192.168.31.27:8000/login` from the racecontrol server (.23) returns a 200 with a token — salt-api is reachable from the server subnet
  4. After a full reboot of James's machine, salt-master and salt-api are running within 60 seconds without manual intervention — Windows Task Scheduler autostart is working
**Plans**: 2 plans

Plans:
- [ ] 36-01-PLAN.md — WSL2 mirrored networking + salt-master 3008 install + Hyper-V firewall rule (INFRA-01, INFRA-02, INFRA-03)
- [ ] 36-02-PLAN.md — salt-api rest_cherrypy config + Windows Task Scheduler autostart (INFRA-04, INFRA-05)

### Phase 37: Pod 8 Minion Bootstrap
**Goal**: Pod 8 is a verified salt minion — silently installed with explicit ID `pod8`, Defender exclusions applied before the installer runs so binaries are not quarantined, Windows Service recovery configured so the minion restarts itself after a stop, key accepted on master, and `salt 'pod8' cmd.run 'whoami'` succeeds; install.bat is rewritten to bootstrap salt-minion instead of pod-agent
**Depends on**: Phase 36
**Requirements**: MINION-01, MINION-02, MINION-03, MINION-04
**Success Criteria** (what must be TRUE):
  1. `salt 'pod8' test.ping` returns True from James's WSL2 terminal — Pod 8 minion is connected and key is accepted
  2. `salt 'pod8' cmd.run 'whoami'` returns the pod's Windows user — remote execution works end-to-end through the WSL2 master
  3. `sc qfailure salt-minion` on Pod 8 shows restart actions at 5s, 10s, 30s — the minion self-restarts after a stop (working around the confirmed Salt Windows service restart bug)
  4. `salt 'pod8' test.ping` still returns True 30 seconds after `sc stop salt-minion` — the sc failure recovery kicked in and restarted the minion service
  5. The rewritten install.bat contains no pod-agent kill, no :8090 firewall rule, and no pod-agent binary reference — only Defender exclusions + rc-agent copy + salt-minion MSI bootstrap
**Plans**: 3 plans

Plans:
- [ ] 37-01-PLAN.md — Pod 8 minion install: Defender exclusions + silent EXE install with id:pod8 + sc failure config + key accept (MINION-01, MINION-02, MINION-04)
- [ ] 37-02-PLAN.md — Rewrite install.bat: strip pod-agent sections, add salt-minion bootstrap, verify on Pod 8 (MINION-03)

### Phase 38: salt_exec.rs + Server Module Migration
**Goal**: racecontrol has a new `salt_exec.rs` module that wraps salt-api REST calls via the existing reqwest client, and all four modules that currently call port 8090 (deploy.rs, fleet_health.rs, pod_monitor.rs, pod_healer.rs) are rewritten to use salt_exec — verified end-to-end against Pod 8 with Pod 8 canary deploy succeeding
**Depends on**: Phase 37
**Requirements**: SALT-01, SALT-02, SALT-03, SALT-04, SALT-05
**Success Criteria** (what must be TRUE):
  1. `cargo test -p racecontrol-crate` passes with salt_exec.rs compiled — the `[salt]` section in racecontrol.toml and `SaltClient` in AppState are wired without breaking existing tests
  2. `fleet_health.rs` reports Pod 8 as `minion_reachable: true` in the staff dashboard — `salt_exec.ping()` replaces the old HTTP health check and the field name is updated
  3. A deploy triggered from racecontrol to Pod 8 via `salt_exec.cp_get_file()` + `salt_exec.cmd_run()` completes with the new rc-agent binary running on the pod — the Python HTTP server + curl pipeline is no longer needed for this operation
  4. `pod_monitor.rs` restarts the rc-agent Windows service on Pod 8 via `salt_exec.service_restart()` — confirmed by checking pod agent reconnect after the restart
  5. `pod_healer.rs` runs a healing command on Pod 8 via `salt_exec.cmd_run()` and the result is logged — all diagnostic parse logic in pod_healer is unchanged, only the transport layer changed
**Plans**: 3 plans

Plans:
- [ ] 38-01-PLAN.md — salt_exec.rs module: SaltClient, cmd_run, cp_get_file, ping, ping_all, service_restart; [salt] config section; AppState wiring (SALT-01)
- [ ] 38-02-PLAN.md — fleet_health.rs + pod_monitor.rs migration to salt_exec; minion_reachable rename (SALT-03, SALT-04)
- [ ] 38-03-PLAN.md — pod_healer.rs + deploy.rs migration to salt_exec; cp.get_file vs curl decision applied to deploy (SALT-02, SALT-05)

### Phase 39: remote_ops.rs Removal
**Goal**: remote_ops.rs is permanently deleted from rc-agent — but only after characterization tests cover the billing lifecycle WebSocket path, every caller is confirmed migrated, and Pod 8 runs a full billing session without panics; all port 8090 references are purged from Rust source, deploy scripts, training data, and docs
**Depends on**: Phase 38
**Requirements**: PURGE-01, PURGE-02, PURGE-03, PURGE-04, PURGE-05, FLEET-01
**Success Criteria** (what must be TRUE):
  1. Characterization tests for the billing lifecycle WebSocket path (session start, game launch, billing tick, session end, lock screen) are green before any file is deleted — Refactor Second rule satisfied
  2. `grep -r "remote_ops\|8090\|pod.agent" crates/rc-agent/src/` returns no matches — all references purged from rc-agent Rust source including firewall.rs port 8090 rule and main.rs startup call
  3. `cargo build --release -p rc-agent-crate` succeeds and `cargo test` passes — rc-agent compiles cleanly without the remote_ops module
  4. No references to pod-agent or port 8090 remain in deploy scripts, training data pairs, or operational docs — confirmed by grep across the full repo
  5. Pod 8 completes a full billing session (start → game launch → billing ticks → session end → lock screen) with the new rc-agent binary that has no remote_ops module — no panics, no blank screens, billing amounts correct
**Plans**: 3 plans

Plans:
- [ ] 39-01-PLAN.md — Characterization tests: WebSocket billing lifecycle path covering AppState fields touched by remote_ops.rs (PURGE-01 prerequisite, FLEET-01 prerequisite)
- [ ] 39-02-PLAN.md — Delete remote_ops.rs + purge all Rust source references (firewall.rs, main.rs, constants) + cargo build clean (PURGE-01, PURGE-02, PURGE-05)
- [ ] 39-03-PLAN.md — Purge pod-agent references from scripts/docs/training data + Port 8090 firewall rule removal from install.bat and netsh configs + Pod 8 canary billing lifecycle verify (PURGE-03, PURGE-04, FLEET-01)

### Phase 40: Fleet Rollout
**Goal**: All 8 pods and the server (.23) are running salt-minion 3008 with accepted keys, `salt '*' test.ping` returns 9 True responses, every pod runs rc-agent without remote_ops, and staff can deploy a new rc-agent binary to any pod via Salt from James's machine — the pod-agent era is over
**Depends on**: Phase 39
**Requirements**: MINION-05, FLEET-02, FLEET-03
**Success Criteria** (what must be TRUE):
  1. `salt '*' test.ping` from James's WSL2 terminal returns 9 True responses (pod1–pod8 + server) — all minion keys are accepted and all nodes are reachable
  2. The staff fleet health dashboard shows all 8 pods as `minion_reachable: true` — fleet_health.rs is pulling live Salt ping results
  3. Staff deploys a new rc-agent.exe to Pod 3 via Salt (as a rollout verification step) and the pod reconnects to racecontrol within 30 seconds — the full deploy workflow via Salt works end-to-end without the Python HTTP server
  4. No active billing sessions are interrupted during the rolling minion installation across pods 1–7 + server — install.bat canary discipline preserved (Pod 8 already done, remaining pods installed one at a time)
**Plans**: 3 plans

Plans:
- [ ] 40-01-PLAN.md — Install salt-minion on pods 1–7 + server via updated install.bat; accept all keys; fleet-wide test.ping (MINION-05, FLEET-02)
- [ ] 40-02-PLAN.md — Verify full deploy workflow via Salt to all pods; confirm staff dashboard shows all minion_reachable; close port 8090 on all pods (FLEET-03)

### Phase 41: Test Foundation
**Goal**: Every test script has a shared library to source — `lib/common.sh` with pass/fail/skip/info helpers, `lib/pod-map.sh` with all 8 pod IPs, Playwright installed with bundled Chromium and `playwright.config.ts` configured for sequential single-worker runs against the live venue server, and cargo-nextest configured for Rust crate tests with per-process isolation
**Depends on**: Phase 40 (v6.0 Fleet Rollout — last v6.0 phase; may also start independently as v7.0 infrastructure)
**Requirements**: FOUND-01, FOUND-02, FOUND-03, FOUND-05
**Success Criteria** (what must be TRUE):
  1. Any shell script that sources `lib/common.sh` can call `pass "message"`, `fail "message"`, and `skip "message"` and the output is consistently color-coded with correct exit code tracking — the shared library works
  2. `lib/pod-map.sh` is sourced once and all 8 pod IPs (192.168.31.x) are available as variables to any script in the suite — no more hardcoded IPs scattered across scripts
  3. `npx playwright test --list` from `tests/e2e/` shows discovered spec files and the Playwright config reports `workers: 1`, `fullyParallel: false`, and `baseURL` set from `RC_BASE_URL` — Playwright is installed and configured correctly
  4. `cargo nextest run -p racecontrol-crate` exits 0 with per-process test isolation active — cargo-nextest is configured and Rust crate tests pass under it
**Plans**: 2 plans

Plans:
- [ ] 41-01-PLAN.md — Shared shell library (lib/common.sh, lib/pod-map.sh) + refactor existing scripts (FOUND-01, FOUND-02)
- [ ] 41-02-PLAN.md — Playwright install + config + cargo-nextest install + config (FOUND-03, FOUND-05)

### Phase 42: Kiosk Source Prep + Browser Smoke
**Goal**: The kiosk wizard components have `data-testid` attributes on every interactive element (game selector, track selector, car selector, wizard step indicators, next/back buttons), a pre-test cleanup fixture stops stale games and ends stale billing before each run, and the browser smoke spec confirms every kiosk route returns 200 in a real Chromium instance with no SSR errors, no React error boundaries, and no uncaught JS exceptions
**Depends on**: Phase 41
**Requirements**: FOUND-04, FOUND-06, FOUND-07, BROW-01, BROW-07
**Success Criteria** (what must be TRUE):
  1. `npx playwright test smoke.spec.ts` passes — all kiosk routes (`/`, `/kiosk`, `/kiosk/book`, `/kiosk/pods`) return HTTP 200 in a real Chromium browser with no `pageerror` events and no React error boundary text visible in the DOM
  2. A Playwright spec that selects the game picker by `[data-testid="sim-select"]` and clicks the AC option by `[data-testid="game-option-ac"]` successfully opens the AC wizard — data-testid attributes are present and functional in the live kiosk
  3. Running the pre-test cleanup fixture against a pod with a stale billing session results in that session being ended and the pod returning to Idle state before any test assertion runs — cleanup is idempotent and safe to run on a clean pod
  4. A Playwright test that fails captures a PNG screenshot and a DOM snapshot in `tests/e2e/results/screenshots/` automatically — the screenshot-on-failure hook is wired
  5. Tab, Enter, and Escape key navigation through the wizard reaches the expected step — keyboard navigation simulation works against the live kiosk
**Plans**: 2 plans

Plans:
- [ ] 42-01-PLAN.md — Add data-testid attributes to kiosk wizard source files (FOUND-06)
- [ ] 42-02-PLAN.md — Pre-test cleanup fixture + browser smoke spec + keyboard nav (FOUND-04, FOUND-07, BROW-01, BROW-07)

### Phase 43: Wizard Flows + API Pipeline Tests
**Goal**: All 5 sim wizard flows are exercised step-by-step in Playwright (AC: 13-step full flow; F1 25/EVO/Rally/iRacing: 5-step simplified flow), experience filtering and staff mode bypass are validated in the browser, and curl-based API tests confirm the full billing lifecycle, per-game launch with PID verification, game state machine transitions, and Steam dialog auto-dismissal on Pod 8
**Depends on**: Phase 42
**Requirements**: BROW-02, BROW-03, BROW-04, BROW-05, BROW-06, API-01, API-02, API-03, API-04, API-05
**Success Criteria** (what must be TRUE):
  1. `npx playwright test wizard.spec.ts` passes for all 5 sim types — AC wizard reaches the review step via all 13 steps with track and car selections confirmed; non-AC wizard reaches review via exactly 5 steps with no `select_track` or `select_car` steps present in the DOM
  2. Staff mode test (`?staff=true&pod=pod-8`) navigates the full booking flow without the phone/OTP step appearing — the staff bypass path is exercised end-to-end
  3. The experience filtering spec confirms that selecting F1 25 shows only F1 25 experiences and the Custom button is absent from the DOM — per-game filtering works correctly
  4. `api/billing.sh` exits 0 — a billing session is created, the launch gate rejects a launch request without an active session, the session timer ticks, and the session is ended cleanly
  5. `api/launch.sh` exits 0 for each installed sim — each game reaches `Launching` state with a PID returned or a confirmed `Launching` state within 60s; game state cycles through Idle→Launching→Running→Idle; Steam dialog dismissal via WM_CLOSE is attempted and logged
**Plans**: 2 plans

Plans:
- [ ] 43-01-PLAN.md — Playwright wizard spec: AC flow, non-AC flow, staff mode, experience filtering, UI navigation (BROW-02, BROW-03, BROW-04, BROW-05, BROW-06)
- [ ] 43-02-PLAN.md — Shell API scripts: billing lifecycle + per-game launch with state polling, Steam dismiss, error screenshot (API-01, API-02, API-03, API-04, API-05)

### Phase 44: Deploy Verification + Master Script
**Goal**: A single `run-all.sh` entry point runs all test phases in sequence, aborts on preflight failure, collects exit codes from each phase, writes a `results/summary.json`, and exits with the total failure count — making it usable as a pre-deploy gate; deploy verification confirms binary swap, detects EADDRINUSE after kiosk restart, and validates all 8 pods reconnect after a rolling restart
**Depends on**: Phase 43
**Requirements**: DEPL-01, DEPL-02, DEPL-03, DEPL-04
**Success Criteria** (what must be TRUE):
  1. `bash tests/e2e/run-all.sh` runs all four phases in sequence, prints a summary table with pass/fail counts per phase, exits 0 when all tests pass, and exits with the failure count when any test fails — the master entry point works as a pre-deploy gate
  2. `deploy/verify.sh` detects an EADDRINUSE condition after kiosk restart, polls until port 3300 is free (up to 30s), and only then starts the new kiosk process — the port-free poll loop prevents the documented bind failure
  3. `deploy/verify.sh` verifies binary size changed after a swap, confirms racecontrol process is running on port 8080, and checks `/api/v1/fleet/health` shows all 8 agents reconnected — the full deploy verification sequence completes against Pod 8 as canary
  4. Test failures and error screenshots captured during the run are passed to the AI debugger error log — the `DEPL-04` routing is wired and a test failure produces an entry in the AI debugger input
**Plans**: 2 plans

Plans:
- [ ] 44-01-PLAN.md — Deploy verification script: binary swap, port conflict, fleet health, AI debugger routing (DEPL-01, DEPL-02, DEPL-04)
- [ ] 44-02-PLAN.md — Master run-all.sh orchestrator: phase-gated sequential runner with summary.json (DEPL-03)

### Phase 45: CLOSE_WAIT Fix + Connection Hygiene
**Goal**: Eliminate the CLOSE_WAIT socket leak on port 8090 that causes 5/8 pods to accumulate 100-134 stuck sockets and trigger unnecessary self-relaunches every ~5 minutes — fix the remote_ops axum server to properly close HTTP connections, fix fleet_health.rs to reuse a shared reqwest client, add SO_REUSEADDR to all UDP game telemetry sockets, mark UDP sockets non-inheritable (matching ea30ca3 treatment for :8090), and increase exec slots from 4→8 or separate health checks from exec pool
**Depends on**: None (can proceed independently). Uses v7.0 E2E: lib/common.sh, lib/pod-map.sh, run-all.sh
**Requirements**: CONN-HYG-01 through CONN-HYG-05
**Success Criteria** (what must be TRUE):
  1. After 30 minutes of normal fleet_health polling, no pod has >5 CLOSE_WAIT sockets on :8090 — leak is eliminated
  2. Pod self-relaunches from CLOSE_WAIT strike counter drop to zero across 8-hour monitoring window
  3. After rc-agent self-relaunch, all 5 UDP ports bind successfully (no error 10048) — SO_REUSEADDR applied
  4. fleet_health.rs uses a single shared reqwest::Client with connection pooling — no per-request clients
  5. Health endpoint requests never return 429 (slot exhaustion) — separated from exec pool or pool expanded
  6. `bash tests/e2e/fleet/close-wait.sh` passes — E2E verification of CLOSE_WAIT count <5 on all 8 pods after 30min soak test
**Plans**: 2 plans

Plans:
- [ ] 45-01-PLAN.md — rc-agent socket hygiene: Connection:close middleware, UDP SO_REUSEADDR + non-inherit, OnceLock Ollama client, MAX_CONCURRENT_EXECS 4->8 (CONN-HYG-01, CONN-HYG-02, CONN-HYG-03, CONN-HYG-04, CONN-HYG-05)
- [ ] 45-02-PLAN.md — fleet_health.rs pool_max_idle_per_host(0) + close-wait.sh E2E test (CONN-HYG-01)

### Phase 46: Crash Safety + Panic Hook
**Goal**: rc-agent never leaves a pod in an unsafe state after a crash — custom panic hook zeroes FFB and shows error lock screen, all server port bindings are checked at startup with clear error messages on failure, FFB zero retries 3 times before escalating, and a BootVerification message is sent to the server after all subsystems initialize
**Depends on**: None (can proceed independently). Uses v7.0 E2E: lib/common.sh, lib/pod-map.sh
**Requirements**: SAFETY-01 through SAFETY-05
**Success Criteria** (what must be TRUE):
  1. A simulated panic in rc-agent (test mode) results in FFB zeroed, "System Error" shown on lock screen, crash logged to rc-bot-events.log, and clean process exit — no orphaned game processes
  2. If port 18923 (lock screen) or 8090 (remote ops) is already in use, rc-agent logs a clear error and exits within 5s — no silent failure
  3. FFB zero failure on first attempt triggers 2 retries at 100ms intervals and logs the final result — verified by test
  4. Server receives BootVerification message within 30s of rc-agent startup showing: WS connected, lock screen port bound, remote ops port bound, HID status, UDP port status
  5. `cargo test -p rc-agent-crate` passes with all new safety tests green
  6. `bash tests/e2e/fleet/startup-verify.sh` passes — E2E verification that BootVerification arrives on all pods after rolling restart
**Plans**: 2 plans

Plans:
- [ ] 46-01-PLAN.md — FFB zero retry + StartupReport protocol extension + server-side fleet health update (SAFETY-03, SAFETY-04, SAFETY-05)
- [ ] 46-02-PLAN.md — Panic hook + port-bind signaling + BootVerification wiring + startup-verify.sh E2E (SAFETY-01, SAFETY-02, SAFETY-04, SAFETY-05)

### Phase 47: Local LLM Fleet Deployment
**Goal**: Every pod runs Ollama locally with the rp-debug model (qwen3:0.6b base, Racing Point system prompt), rc-agent queries localhost:11434 for AI diagnosis with Windows Event Viewer + rc-bot-events.log fed as context (PodErrorContext), Ollama timeout reduced to 30s, and pattern memory pre-seeded with 7 deterministic fix patterns
**Depends on**: Phase 45 (CLOSE_WAIT fix — so local Ollama diagnosis is meaningful). Uses v7.0 E2E: lib/common.sh, lib/pod-map.sh
**Requirements**: LLM-01 through LLM-04
**Success Criteria** (what must be TRUE):
  1. `ollama list` on all 8 pods returns `rp-debug:latest` (522 MB, qwen3:0.6b base) — DONE (deployed 2026-03-19)
  2. rc-agent TOML on all pods has `ollama_url = "http://127.0.0.1:11434"` and `ollama_model = "rp-debug"`
  3. ai_debugger.rs includes PodErrorContext (rc-bot-events.log + Windows Event Viewer + CLOSE_WAIT count + known patterns) in every LLM prompt — DONE (coded 2026-03-19)
  4. debug-memory.json on each pod is pre-seeded with the 7 deterministic fix patterns (success_count=1) — instant replay from first boot
  5. `bash tests/e2e/fleet/ollama-health.sh` passes — E2E verification that rp-debug model responds <5s on all 8 pods
**Plans**: 2 plans

Plans:
- [ ] 47-01-PLAN.md — Modelfile expansion (14 keywords) + seed-debug-memory.sh (7 patterns) (LLM-03, LLM-04)
- [ ] 47-02-PLAN.md — ollama-health.sh E2E test: model presence + response time <5s (LLM-01, LLM-02)

### Phase 48: Dynamic Kiosk Allowlist
**Goal**: Staff can add allowed processes via the admin panel instead of requiring code changes + rebuild + redeploy to all 8 pods — server stores allowlist in DB, serves it via API, rc-agent fetches it on startup and every 5 minutes, merges with hardcoded baseline, and local LLM classifies unknown processes as ALLOW/BLOCK/ASK
**Depends on**: Phase 47 (local LLM needed for process classification). Uses v7.0 E2E: lib/common.sh, Playwright for admin panel
**Requirements**: ALLOW-01 through ALLOW-05
**Success Criteria** (what must be TRUE):
  1. `GET /api/v1/config/kiosk-allowlist` returns the merged allowlist (hardcoded + DB additions)
  2. Admin panel has a "Kiosk Allowlist" section where staff can add/remove process names
  3. rc-agent picks up a newly added process within 5 minutes without restart or redeploy
  4. Unknown process triggers local LLM classification (ALLOW/BLOCK/ASK) — no kill without classification
  5. No false lockdowns occur when a Windows system process runs on any pod
  6. `bash tests/e2e/api/kiosk-allowlist.sh` passes — CRUD test on allowlist API + `npx playwright test allowlist.spec.ts` for admin panel
**Plans**: 2 plans

Plans:
- [ ] 48-01-PLAN.md — Server-side kiosk_allowlist DB table + CRUD API endpoints + admin panel UI (ALLOW-01, ALLOW-02, ALLOW-05)
- [ ] 48-02-PLAN.md — rc-agent server-fetched allowlist poll + LLM process classifier + E2E test script (ALLOW-03, ALLOW-04, ALLOW-05)

### Phase 49: Session Lifecycle Autonomy
**Goal**: rc-agent autonomously handles session end-of-life — auto-ends orphaned billing after configurable timeout, resets pod to idle after session, pauses billing on game crash with auto-resume, and fast-reconnects WebSocket without full relaunch when server blips
**Depends on**: Phase 46 (crash safety must be in place before autonomous billing actions). Uses v7.0 E2E: lib/common.sh, api pipeline tests
**Requirements**: SESSION-01 through SESSION-04
**Success Criteria** (what must be TRUE):
  1. After billing_active=true with no game_pid for 5 minutes (configurable via `auto_end_orphan_session_secs`), rc-agent auto-ends session via server API — no human intervention needed
  2. 30 seconds after session end, pod automatically returns to PinEntry/ScreenBlanked state — no "Session Complete!" stuck forever
  3. On game crash (CRASH-01), billing is paused within 5s. If game relaunches successfully, billing resumes. After 2 failed relaunches, session auto-ends.
  4. When WebSocket drops, if reconnect succeeds within 30s, no self-relaunch occurs — existing state preserved
  5. Orphaned session auto-end triggers a notification to the server for staff visibility
  6. `bash tests/e2e/api/session-lifecycle.sh` passes — billing create → orphan timeout → auto-end → pod reset verified via API
**Plans**: 2 plans

Plans:
- [ ] 49-01-PLAN.md � Protocol + schema + orphan auto-end + idle PinEntry + blank_timer target (SESSION-01, SESSION-02)
- [ ] 49-02-PLAN.md � Crash recovery state machine + WS grace window + E2E test (SESSION-03, SESSION-04)

### Phase 50: LLM Self-Test + Fleet Health
**Goal**: rc-agent runs 18 deterministic self-test probes at startup and on-demand (WS, lock screen, remote ops, overlay, debug server, 5 UDP ports, HID, Ollama, CLOSE_WAIT, single instance, disk, memory, shader cache, build_id, billing state, session ID, GPU temp, Steam), feeds results to local LLM for a HEALTHY/DEGRADED/CRITICAL verdict with correlation analysis and auto-fix recommendations, server exposes /api/v1/pods/{id}/self-test endpoint for fleet-wide health checks, and auto-fix patterns 8-14 are wired into ai_debugger.rs (DirectX reset, shader cache clear, memory pressure, DLL repair, Steam restart, performance throttle, network adapter reset)
**Depends on**: Phase 46 (panic hook for safe error handling) + Phase 47 (local LLM for verdict generation). Uses v7.0 E2E: lib/common.sh, lib/pod-map.sh, run-all.sh
**Requirements**: SELFTEST-01 through SELFTEST-06
**Success Criteria** (what must be TRUE):
  1. `self_test.rs` runs all 18 probes and returns a JSON result with probe name, status (pass/fail/skip), and detail string for each — no probe panics or hangs (10s timeout per probe)
  2. Local LLM (rp-debug) receives all 18 probe results and returns a structured verdict: HEALTHY (all pass), DEGRADED (non-critical failures), or CRITICAL (WS/lock screen/billing failures) with correlation analysis linking related probe failures
  3. `GET /api/v1/pods/{id}/self-test` triggers self-test on the target pod via WebSocket command, returns the full probe results + LLM verdict within 30s
  4. Auto-fix patterns 8-14 are implemented in ai_debugger.rs and triggered by corresponding probe failures: DirectX (shader cache clear + device reset), memory (process trim), DLL (sfc scan), Steam (restart), performance (power plan), network (adapter reset)
  5. `cargo test -p rc-agent-crate` passes with all self-test probe tests green
  6. `bash tests/e2e/fleet/pod-health.sh` passes — triggers self-test on all 8 pods via API, asserts all HEALTHY, wired into run-all.sh as final phase gate
**Plans**: 3 plans

Plans:
- [ ] 50-01-PLAN.md — self_test.rs 18 probes + LLM verdict + protocol extensions + startup integration (SELFTEST-01, SELFTEST-02, SELFTEST-06)
- [ ] 50-02-PLAN.md — Auto-fix patterns 8-14 in ai_debugger.rs (SELFTEST-04)
- [ ] 50-03-PLAN.md — Server endpoint + WS plumbing + agent handler + pod-health.sh E2E + run-all.sh integration (SELFTEST-03, SELFTEST-05)

### Phase 51: CLAUDE.md + Custom Skills
**Goal**: Claude Code sessions always start with full Racing Point context — pod IPs, crate names, naming conventions, constraints — and James can trigger structured deploy and incident workflows with single slash commands, no manual copy-paste of context
**Depends on**: Nothing (James workstation only, zero install)
**Requirements**: SKILL-01, SKILL-02, SKILL-03, SKILL-04, SKILL-05
**Success Criteria** (what must be TRUE):
  1. Opening any Claude Code session in the racecontrol repo, Claude immediately knows all 8 pod IPs, crate names, binary naming rules, and the 4-tier debug order without James typing any context
  2. `/rp:deploy` builds rc-agent, stages the binary, and outputs the pendrive deploy command — James never needs to remember cargo flags or paths
  3. `/rp:deploy-server` stops the old racecontrol process, swaps the binary, and confirms :8080 returns 200 — the full server deploy flow in one command
  4. `/rp:pod-status pod-8` returns rc-agent WS status, billing state, and last heartbeat for Pod 8 by querying /fleet/health with the correct IP injected automatically
  5. `/rp:incident "Pod 3 lock screen blank"` returns a structured 4-tier response: deterministic checks first, memory patterns, Ollama diagnosis steps, then cloud escalation
**Plans**: 2 plans

Plans:
- [ ] 51-01-PLAN.md — CLAUDE.md: project context (pod IPs, crate names, naming rules, constraints, 4-tier debug order) (SKILL-01)
- [ ] 51-02-PLAN.md — Custom skills: /rp:deploy, /rp:deploy-server, /rp:pod-status, /rp:incident (SKILL-02, SKILL-03, SKILL-04, SKILL-05)

### Phase 52: MCP Servers
**Goal**: Claude Code can query Gmail, Google Sheets, Google Calendar, and the racecontrol REST API directly from any session — James describes what he needs in plain language and Claude fetches live data without manual curl or browser lookups
**Depends on**: Phase 51 (CLAUDE.md must document MCP tool names so skills can reference them)
**Requirements**: MCP-01, MCP-02, MCP-03, MCP-04
**Success Criteria** (what must be TRUE):
  1. Claude Code reads the latest Gmail messages in james@racingpoint.in without James opening a browser — MCP-01 Google Workspace MCP connected via existing racingpoint-google OAuth
  2. Claude Code reads a cell range from a Racing Point Google Sheet and writes computed values back — MCP-02 Sheets read/write works end-to-end
  3. Claude Code lists today's Google Calendar events for james@racingpoint.in — MCP-03 Calendar read works
  4. Claude Code calls GET /api/v1/fleet/health on the local racecontrol server and returns structured pod statuses — MCP-04 rc-ops-mcp running on James's machine and responding
**Plans**: 2 plans

Plans:
- [ ] 52-01-PLAN.md — Google Workspace MCP: wire racingpoint-google OAuth into Claude Code MCP config, verify Gmail + Sheets + Calendar (MCP-01, MCP-02, MCP-03)
- [ ] 52-02-PLAN.md — rc-ops-mcp: Node.js MCP server exposing racecontrol REST endpoints; Claude Code MCP config entry (MCP-04)

### Phase 53: Deployment Automation
**Goal**: The staging HTTP server and webterm start automatically when James's machine boots; every deploy runs a verification script confirming binary size changed, /health returns 200, and all agents reconnected; the deploy script enforces canary-first and requires explicit approval before fleet rollout
**Depends on**: Phase 51 (deploy skills reference the verify script)
**Requirements**: DEPLOY-01, DEPLOY-02, DEPLOY-03
**Success Criteria** (what must be TRUE):
  1. After a cold reboot of James's machine, the staging HTTP server (deploy-staging/) and webterm (port 9999) are running within 60 seconds without James opening any terminal — autostart confirmed
  2. Running the post-deploy verify script outputs: binary size changed (before vs after bytes), /api/v1/fleet/health returns HTTP 200, and all 8 agents show ws_connected:true — exits non-zero on any failure
  3. The deploy script deploys to Pod 8 first, waits for verify to pass, then prints a confirmation prompt and refuses to proceed to all pods until James explicitly approves — canary gate is enforced, not advisory
**Plans**: 2 plans

Plans:
- [ ] 53-01-PLAN.md — Task Scheduler ONLOGON autostart for staging HTTP :9998 + webterm :9999 + auto-start.sh test (DEPLOY-01)
- [ ] 53-02-PLAN.md — /rp:deploy-fleet skill: canary Pod 8 + verify.sh + approval gate + sequential fleet deploy (DEPLOY-02, DEPLOY-03)

### Phase 54: Structured Logging + Error Rate Alerting
**Goal**: racecontrol and rc-agent write structured JSON logs to daily-rotating files so incidents can be investigated with jq; racecontrol watches its own error rate and emails James and Uday when it exceeds a configurable threshold
**Depends on**: Phase 53 (deployment automation must be in place before Rust code changes are deployed)
**Requirements**: MON-01, MON-02, MON-03
**Success Criteria** (what must be TRUE):
  1. After a racecontrol restart, the logs directory on the server shows racecontrol-YYYY-MM-DD.jsonl with JSON entries containing timestamp, level, and message fields — structured logs with daily rotation
  2. After an rc-agent restart on any pod, the logs directory shows rc-agent-YYYY-MM-DD.jsonl with JSON entries including pod_id, timestamp, level, and message fields
  3. Triggering 5 consecutive errors in racecontrol within 1 minute (configurable via error_rate_threshold and error_rate_window_secs in racecontrol.toml) sends an email to james@racingpoint.in and usingh@racingpoint.in within 2 minutes — rate-limited, no second email for 30 minutes
**Plans**: 3 plans

Plans:
- [ ] 54-01-PLAN.md — racecontrol structured logging: tracing-subscriber JSON format + daily file rotation via tracing-appender (MON-01)
- [ ] 54-02-PLAN.md — rc-agent structured logging: same tracing-subscriber JSON setup + daily rotation (MON-02)
- [ ] 54-03-PLAN.md — Error rate alerting in racecontrol: sliding window counter + threshold config + email via send_email.js (MON-03)

### Phase 55: Netdata Fleet Deploy
**Goal**: Netdata agent is installed on the racecontrol server (.23) and all 8 pods, collecting real-time CPU/RAM/disk/network metrics with auto-generated dashboards — pods deployed via rc-agent :8090 exec without physical access
**Depends on**: Phase 54 (structured logs must be in place before Netdata install)
**Requirements**: MON-04, MON-05
**Success Criteria** (what must be TRUE):
  1. Navigating to http://192.168.31.23:19999 in James's browser shows the Netdata dashboard for the racecontrol server with live CPU, RAM, disk, and network charts updating in real time
  2. Navigating to http://192.168.31.89:19999 (Pod 1) shows a Netdata dashboard — all 8 pods have Netdata running after fleet deploy via rc-agent :8090 exec
  3. The Netdata install on each pod completed via rc-agent remote exec confirmed by exec log showing successful install command per pod — no pendrive needed
**Plans**: 2 plans

Plans:
- [ ] 55-01-PLAN.md — Download MSI + deploy script + verification script + server install (MON-04)
- [ ] 55-02-PLAN.md — Canary Pod 8 + fleet rollout Pods 1-7 via rc-agent :8090 exec (MON-05)

### Phase 56: WhatsApp Alerting + Weekly Report
**Goal**: Uday receives a WhatsApp message within 60 seconds of a P0 event and a recovery notification when it clears; every Monday morning an email lands in Uday's inbox summarizing the previous week's fleet performance
**Depends on**: Phase 54 (error rate alerting provides event hooks; structured logs feed the weekly report query)
**Requirements**: MON-06, MON-07
**Success Criteria** (what must be TRUE):
  1. Simulating a P0 event (stopping racecontrol so all pods lose WS connection) results in Uday receiving a WhatsApp message via racingpoint-whatsapp-bot within 60 seconds — message includes event type, timestamp in IST, and pod count affected
  2. When all P0 conditions resolve (all pods reconnect), a resolved WhatsApp message is sent to Uday within 60 seconds of recovery
  3. The weekly report email arrives in usingh@racingpoint.in every Monday between 08:00 and 08:05 IST with: total sessions, uptime % per pod, total credits billed, and numbered incident list from the error rate alert log
**Plans**: 2 plans

Plans:
- [ ] 56-01-PLAN.md — P0 WhatsApp alert: hook into racingpoint-whatsapp-bot; trigger on all-pods-offline + billing crash; resolved notification (MON-06)
- [ ] 56-02-PLAN.md — Weekly report: scheduled task; query racecontrol DB for sessions/uptime/credits; compose + email via send_email.js (MON-07)

### Phase 57: Session-End Safety
**Goal**: When a game session ends, the wheelbase returns to center safely within 2 seconds — no stuck rotation, no snap-back, no staff intervention
**Depends on**: Phase 52 (existing rc-agent codebase; no v9.0 dependency)
**Requirements**: SAFE-01, SAFE-02, SAFE-03, SAFE-04, SAFE-05, SAFE-06, SAFE-07
**Success Criteria** (what must be TRUE):
  1. Wheelbase returns to center within 2 seconds of any game closing on any pod — no stuck rotation
  2. Centering force ramps up gradually (no sudden snap that could injure a customer's hands)
  3. ConspitLink is closed before HID safety commands fire, eliminating P-20 contention
  4. ConspitLink restarts automatically after the safety sequence with verified JSON config intact
  5. ESTOP code path remains available but is never triggered during routine session ends
**Plans**: 3 plans

Plans:
- [x] 57-01-PLAN.md — HID commands: fxm_reset + idlespring + power cap constant + Clone derive + unit tests (SAFE-02, SAFE-03, SAFE-04, SAFE-05)
- [x] 57-02-PLAN.md — safe_session_end orchestrator: close ConspitLink + HID sequence + wire 10 call sites in main.rs (SAFE-01, SAFE-06, SAFE-07)
- [x] 57-03-PLAN.md — Startup power cap + manual hardware verification on canary pod (SAFE-01, SAFE-04)

### Phase 58: ConspitLink Process Hardening
**Goal**: ConspitLink stays running reliably across all sessions with crash recovery, config integrity, and kiosk compliance
**Depends on**: Phase 57
**Requirements**: PROC-01, PROC-02, PROC-03, PROC-04
**Success Criteria** (what must be TRUE):
  1. ConspitLink automatically restarts after a crash with crash count tracked (never using taskkill /F)
  2. Config files are backed up before any write and verified via JSON parse after every restart
  3. ConspitLink window stays minimized even after restarts — kiosk lock screen always visible
**Plans**: 2 plans

Plans:
- [ ] 58-01-PLAN.md — Hardened restart with crash-count, config backup/verify, minimize retry (PROC-01, PROC-02, PROC-03, PROC-04)
- [ ] 58-02-PLAN.md — Wire watchdog to hardened restart + Pod 8 hardware verification (PROC-01, PROC-04)

### Phase 59: Auto-Switch Configuration
**Goal**: ConspitLink automatically detects which game is running and loads the correct FFB preset without staff action
**Depends on**: Phase 58
**Requirements**: PROF-01, PROF-02, PROF-04
**Success Criteria** (what must be TRUE):
  1. Global.json exists at C:\RacingPoint\ on every pod (the runtime read path ConspitLink actually uses)
  2. GameToBaseConfig.json mappings point to Racing Point venue presets for all 4 active games
  3. Launching AC, F1 25, ACC/AC EVO, or AC Rally causes ConspitLink to auto-load the matching preset
**Plans**: 4 plans

Plans:
- [ ] 59-01-PLAN.md — Implement ensure_auto_switch_config + unit tests + main.rs wiring
- [ ] 59-02-PLAN.md — Deploy to Pod 8 canary + human verification
- [ ] 59-03-PLAN.md — [GAP] Add 4th game key from Pod 8 inspection + redeploy
- [ ] 59-04-PLAN.md — [GAP] Human verification of auto-switch on Pod 8 hardware

### Phase 60: Pre-Launch Profile Loading
**Goal**: rc-agent ensures the correct preset is loaded BEFORE the game starts, with a safe fallback if the game is unrecognized
**Depends on**: Phase 59
**Requirements**: PROF-03, PROF-05
**Success Criteria** (what must be TRUE):
  1. rc-agent loads the correct game preset before launching the game process (not relying solely on ConspitLink auto-detect)
  2. If an unrecognized game launches, a safe default preset is applied (conservative force, centered spring)
**Plans**: 1 plan

Plans:
- [ ] 60-01-PLAN.md -- Implement pre_load_game_preset + wire into LaunchGame handler (PROF-03, PROF-05)

### Phase 61: FFB Preset Tuning
**Goal**: Every venue game has a tuned FFB preset that feels right on the Ares 8Nm hardware, with correct steering angles and force limits
**Depends on**: Phase 59
**Requirements**: FFB-01, FFB-02, FFB-03, FFB-04, FFB-05, FFB-06
**Success Criteria** (what must be TRUE):
  1. Assetto Corsa has a venue-tuned .Base preset (based on Yifei Ye pro preset) with 900-degree steering
  2. F1 25 has a custom venue-tuned .Base preset with 360-degree steering
  3. ACC/AC EVO has a venue-tuned .Base preset (based on Yifei Ye pro preset) with appropriate steering angle
  4. AC Rally has a custom venue-tuned .Base preset with ~800-degree steering
  5. All presets are stored in version control under .planning/presets/ for reproducibility
**Plans**: 3 plans

Plans:
- [ ] 61-01: TBD
- [ ] 61-02: TBD

### Phase 62: Fleet Config Distribution
**Goal**: Configs validated on one pod can be pushed to all 8 pods atomically via rc-agent, with drift detection
**Depends on**: Phase 60, Phase 61
**NOTE (v22.0):** This phase is SUPERSEDED by v22.0 Config Push (CP-01 to CP-06, Phases 177-178). v22.0 delivers WebSocket config push, offline queue, hot-reload, schema versioning, audit log, and admin validation — covering all FLEET-01 through FLEET-06 requirements plus more. Do not implement this phase independently. See Phase 182 for integration plan.
**Requirements**: FLEET-01, FLEET-02, FLEET-03, FLEET-04, FLEET-05, FLEET-06
**Success Criteria** (what must be TRUE):
  1. racecontrol can push a config update to any/all pods via WebSocket and rc-agent writes it atomically (temp file + rename)
  2. Global.json is written to BOTH the install directory and C:\RacingPoint\ on every config push
  3. ConspitLink is gracefully stopped before config write and restarted after, with JSON integrity verified
  4. Config checksums are included in pod heartbeats so racecontrol can detect drift across the fleet
  5. A golden config directory in the repo serves as the version-controlled master for all pod configs
**Plans**: 3 plans

Plans:
- [ ] 62-01: TBD
- [ ] 62-02: TBD

### Phase 63: Fleet Monitoring
**Goal**: racecontrol dashboard shows the config state of every pod at a glance — active preset, config hash, firmware version
**Depends on**: Phase 62
**Requirements**: MON-01, MON-02, MON-03, MON-04
**Success Criteria** (what must be TRUE):
  1. rc-agent reports the currently active ConspitLink preset name for its pod
  2. rc-agent reports config file hashes (Settings.json, Global.json, GameToBaseConfig.json) for drift detection
  3. rc-agent reports ConspitLink firmware version for the connected Ares wheelbase
  4. racecontrol dashboard displays fleet config status showing all 8 pods with their preset/hash/firmware at a glance
**Plans**: 3 plans

Plans:
- [ ] 63-01: TBD

### Phase 64: Telemetry Dashboards
**Goal**: The wheel LCD shows useful telemetry (RPM, speed, gear) for every venue game
**Depends on**: Phase 59
**Requirements**: TELE-01, TELE-02, TELE-06
**Success Criteria** (what must be TRUE):
  1. Wheel LCD displays RPM, speed, and gear data while playing any of the 4 venue games
  2. GameSettingCenter.json has all required telemetry fields enabled for AC, F1 25, ACC/AC EVO, and AC Rally
  3. UDP port chain is documented: game output port -> ConspitLink receive port (20778)
**Plans**: 3 plans

Plans:
- [ ] 64-01: TBD

### Phase 65: Shift Lights & RGB Lighting
**Goal**: Shift light LEDs and RGB button lighting respond to live game telemetry for an immersive customer experience
**Depends on**: Phase 64
**Requirements**: TELE-03, TELE-04, TELE-05
**Success Criteria** (what must be TRUE):
  1. Shift light LEDs illuminate at correct RPM thresholds for AC and ACC (using Auto RPM configs)
  2. Shift light LEDs illuminate at correct RPM thresholds for F1 25 and AC Rally (using manual RPM thresholds)
  3. RGB button lighting responds to telemetry events (DRS, ABS, TC, flags) per game where supported
**Plans**: 3 plans

Plans:
- [ ] 65-01: TBD

## v10.0 Connectivity & Redundancy — Phase Details

### Phase 66: Infrastructure Foundations
**Goal**: The network foundation is stable — server .23 always gets IP 192.168.31.23, James can run commands on .23 via rc-agent :8090 over Tailscale, and James can delegate tasks to Bono's VPS via comms-link exec_request
**Depends on**: Phase 65 (or can start in parallel — no code dependency)
**Requirements**: INFRA-01, INFRA-02, INFRA-03
**Success Criteria** (what must be TRUE):
  1. Router DHCP reservation table shows MAC 10-FF-E0-80-B1-A7 permanently bound to 192.168.31.23 — server never drifts again after reboot or lease expiry
  2. James can POST to rc-agent :8090 exec endpoint via Tailscale IP and receive command output from server .23
  3. James can send an exec_request via comms-link INBOX.md and Bono executes the command on the VPS, returning result via comms-link
**Plans**: 5 plans

Plans:
- [ ] 66-01-PLAN.md — DHCP reservation + static IP on server .23 (INFRA-01)
- [ ] 66-02-PLAN.md — Server exec verification via rc-agent :8090 over Tailscale/LAN (INFRA-02)
- [ ] 66-03-PLAN.md — Comms-link exec wiring: Bono ExecHandler + 4 failover commands (INFRA-03)
- [ ] 66-04-PLAN.md — James exec_request send endpoint (INFRA-03, gap closure)
- [ ] 66-05-PLAN.md — Infrastructure verification checkpoint: router + Bono deploy + round-trip (INFRA-01, INFRA-03, gap closure)

### Phase 67: Config Sync
**Goal**: TOML-based venue configuration (pod definitions, branding, venue metadata) from racecontrol.toml is mirrored to Bono's cloud racecontrol so failover has a current config to run on. Note: billing rates and game catalog are DB-based and already synced via cloud_sync.rs SYNC_TABLES -- this phase covers TOML config only.
**Depends on**: Phase 66
**Requirements**: SYNC-01, SYNC-02, SYNC-03
**Success Criteria** (what must be TRUE):
  1. After editing racecontrol.toml on .23, James observes a sync_push message in comms-link within 60s containing the updated config snapshot
  2. The pushed config payload contains no credentials, passwords, or local Windows paths -- only venue metadata, pod definitions, and branding
  3. Bono's cloud racecontrol stores the received venue/pods/branding snapshot in AppState.venue_config within 5 minutes of a local change
**Plans**: 2 plans

Plans:
- [ ] 67-01-PLAN.md -- James-side config watcher + sanitizer (SYNC-01, SYNC-02)
- [ ] 67-02-PLAN.md -- Cloud racecontrol config_snapshot handler (SYNC-03)

### Phase 68: Pod SwitchController
**Goal**: Any rc-agent pod can switch its WebSocket target from .23 to Bono's VPS and back at runtime without a process restart, and self_monitor will not fight the intentional switch
**Depends on**: Phase 66
**Requirements**: FAIL-01, FAIL-02, FAIL-03, FAIL-04
**Success Criteria** (what must be TRUE):
  1. rc-agent rc-agent.toml has a failover_url field pointing to Bono's racecontrol Tailscale address — all 8 pods configured after pendrive deploy
  2. On Pod 8 canary: sending a SwitchController AgentMessage causes rc-agent to reconnect to the new URL within 15s without rc-agent.exe restarting
  3. self_monitor.rs does not trigger a relaunch during the 60s window after a SwitchController is received (last_switch_time guard active)
  4. Switching back to .23 URL works identically — pod reconnects and resumes normal billing heartbeat
**Plans**: 2 plans

Plans:
- [ ] 68-01-PLAN.md -- Protocol + Config + HeartbeatStatus contracts with unit tests (FAIL-01, FAIL-03, FAIL-04)
- [ ] 68-02-PLAN.md -- Runtime URL switching + self-monitor guard wiring (FAIL-02, FAIL-03, FAIL-04)

### Phase 69: Health Monitor & Failover Orchestration
**Goal**: James automatically detects when .23 is unreachable, waits to confirm it is not a transient AC-launch CPU spike, then coordinates with Bono to promote cloud racecontrol as primary and switch all pods — with Uday notified
**Depends on**: Phase 67, Phase 68
**Requirements**: HLTH-01, HLTH-02, HLTH-03, HLTH-04, ORCH-01, ORCH-02, ORCH-03, ORCH-04
**Success Criteria** (what must be TRUE):
  1. James's health probe loop shows server .23 HTTP + WS checks running every 5s in racecontrol logs — status visible to James without manual intervention
  2. When .23 is powered off, automatic failover fires only after a continuous 60s outage window — a 3s CPU spike during AC launch does not trigger failover
  3. After failover fires: all 8 pods are connected to Bono's VPS WebSocket within 30s of racecontrol broadcasting SwitchController
  4. A pod that still has .23 reachable (split-brain scenario) does not honor the SwitchController until its own LAN probe confirms .23 is down
  5. Uday receives an email and WhatsApp notification within 2 minutes of failover completing, stating which URL pods switched to
**Plans**: 4 plans

Plans:
- [ ] 69-01-PLAN.md — Health probe FSM + failover orchestrator + wiring in james/index.js (HLTH-01, HLTH-02, HLTH-03, ORCH-01, ORCH-04)
- [ ] 69-02-PLAN.md — Broadcast endpoint on racecontrol + split-brain guard on rc-agent (ORCH-02, ORCH-03)
- [ ] 69-03-PLAN.md — Bono secondary watchdog for venue power outage (HLTH-04)
- [ ] 69-04-PLAN.md — Gap closure: notification fixes (COMMAND_REGISTRY + WhatsApp + email) (ORCH-04)

### Phase 70: Failback & Data Reconciliation
**Goal**: When .23 comes back online, sessions created during failover are merged into local DB, and pods automatically reconnect to .23 — Uday notified of the all-clear
**Depends on**: Phase 69
**Requirements**: BACK-01, BACK-02, BACK-03, BACK-04
**Success Criteria** (what must be TRUE):
  1. James detects server .23 recovery using the 2-up threshold (2 consecutive successful probes) — no manual action needed
  2. Any billing sessions that ran on Bono's VPS during the outage appear in the local .23 SQLite DB after failback sync completes
  3. After failback: all 8 pods are connected to .23's WebSocket within 30s of racecontrol broadcasting SwitchController with the original URL
  4. Uday receives an email and WhatsApp notification confirming the venue is back on local server and the outage duration
**Plans**: 2 plans

Plans:
- [ ] 70-01-PLAN.md — import_sessions Rust endpoint (INSERT OR IGNORE for cloud session sync)
- [ ] 70-02-PLAN.md — HealthMonitor server_recovery + FailoverOrchestrator.initiateFailback() + COMMAND_REGISTRY + wiring




## v11.0 Agent & Sentry Hardening -- Phase Details

### Phase 71: rc-common Foundation + rc-sentry Core Hardening
**Goal**: rc-sentry's three live correctness failures are fixed and rc-common gains the feature-gated exec primitive that both callers will share -- with the tokio contamination boundary verified before any code migrates
**Depends on**: Phase 70 (can start in parallel -- no code dependency on v10.0)
**Requirements**: SHARED-01, SHARED-02, SHARED-03, SHARD-01, SHARD-02, SHARD-03, SHARD-04, SHARD-05
**Success Criteria** (what must be TRUE):
  1. `cargo build --bin rc-sentry` succeeds and `cargo tree -p rc-sentry` shows no tokio dependency -- the feature gate boundary is enforced
  2. A long-running command sent to rc-sentry is killed and returns a timeout error after timeout_ms -- hung threads no longer accumulate
  3. rc-sentry rejects a 5th concurrent exec request with HTTP 429 -- unbounded thread spawning is capped at 4
  4. rc-sentry log output shows structured tracing lines with timestamps and levels instead of raw eprintln -- observable in the terminal when rc-sentry starts
  5. A command producing >64KB output is truncated to 64KB before the response is sent -- no buffer overflow on large dir /s outputs
**Plans**: 2 plans

Plans:
- [ ] 71-01-PLAN.md -- rc-common exec.rs with feature-gated run_cmd_sync/run_cmd_async + tokio isolation verification
- [ ] 71-02-PLAN.md -- rc-sentry hardening: timeout, truncation, concurrency cap, TCP read fix, tracing

### Phase 72: rc-sentry Endpoint Expansion + Integration Tests
**Goal**: rc-sentry becomes a complete fallback operations tool with process visibility, file inspection, and health confirmation -- all endpoints covered by integration tests running against an ephemeral port
**Depends on**: Phase 71
**Requirements**: SEXP-01, SEXP-02, SEXP-03, SEXP-04, SHARD-06, TEST-04
**Success Criteria** (what must be TRUE):
  1. `curl http://192.168.31.89:8091/health` returns JSON with uptime, version, concurrent exec slots used, and hostname -- operators can confirm sentry is alive when rc-agent is down
  2. `curl http://192.168.31.89:8091/version` returns the binary version and git commit hash baked in at build time via build.rs
  3. `curl 'http://192.168.31.89:8091/files?path=C:\RacingPoint'` returns a directory listing -- staff can verify binaries are present during incident response
  4. `curl http://192.168.31.89:8091/processes` returns running processes with PID, name, and memory -- staff can confirm rc-agent.exe is running when WS is down
  5. `cargo test -p rc-sentry` passes all endpoint integration tests (/ping, /exec, /health, /version, /files, /processes) against an ephemeral port
  6. Sending SIGTERM or Ctrl+C to rc-sentry causes it to drain active connections before exiting -- no abrupt mid-response kills
**Plans**: 2 (researched + planned 2026-03-21)

Plans:
- [ ] 72-01: TBD
- [ ] 72-02: TBD

### Phase 73: Critical Business Tests
**Goal**: billing_guard and failure_monitor have unit test coverage verifying their state machine logic before any structural refactoring; FfbBackend trait seam enables FFB controller tests without real HID hardware
**Depends on**: Phase 71 (rc-common available; no dependency on Phase 72)
**Requirements**: TEST-01, TEST-02, TEST-03
**Success Criteria** (what must be TRUE):
  1. `cargo test -p rc-agent billing_guard` passes -- stuck session detection (BILL-02) and idle drift (BILL-03) are covered by named unit tests that assert the correct auto-end behavior
  2. `cargo test -p rc-agent failure_monitor` passes -- game freeze detection (CRASH-01) and launch timeout (CRASH-02) are covered without requiring a live game process
  3. `cargo test -p rc-agent ffb` passes with FfbBackend trait injected -- tests run on James workstation without a real wheelbase connected and without sending any HID command to live hardware
**Plans**: 2 plans

Plans:
- [ ] 73-01-PLAN.md — FfbBackend trait seam + mockall mock tests (TEST-03)
- [ ] 73-02-PLAN.md — billing_guard timer tests (TEST-01) + failure_monitor requirement-named tests (TEST-02)

### Phase 74: rc-agent Decomposition
**Goal**: rc-agent main.rs is reduced from ~3,400 lines to ~150 lines by extracting config types, AppState, WebSocket handler, and event loop into focused modules -- each module under 500 lines and testable in isolation
**Depends on**: Phase 73 (characterization tests must be green before any structural change -- standing rule: Refactor Second)
**Requirements**: DECOMP-01, DECOMP-02, DECOMP-03, DECOMP-04
**Success Criteria** (what must be TRUE):
  1. `wc -l crates/rc-agent/src/main.rs` reports fewer than 500 lines -- the bulk of startup and state logic is now in separate modules
  2. `cargo test -p rc-agent` passes with all Phase 73 tests still green after every extraction step -- no regression in billing_guard, failure_monitor, or FFB tests
  3. `cargo build --release --bin rc-agent` produces a binary that passes the Phase 50 pod self-test on Pod 8 canary -- behaviorally identical to pre-decomposition binary
  4. config.rs, app_state.rs, ws_handler.rs, and event_loop.rs each exist as named source files under crates/rc-agent/src/ -- decomposition is observable in the file tree
**Plans**: 4 plans

Plans:
- [ ] 74-01-PLAN.md -- Extract config types and load/validate functions to config.rs (DECOMP-01)
- [ ] 74-02-PLAN.md -- Extract pre-loop shared state to AppState struct in app_state.rs (DECOMP-02)
- [ ] 74-03-PLAN.md -- Extract WebSocket message handler to ws_handler.rs (DECOMP-03)
- [ ] 74-04-PLAN.md -- Extract inner select! loop to event_loop.rs with ConnectionState (DECOMP-04)

## v12.0 Operations Security -- Phase Details

### Phase 75: Security Audit & Foundations
**Goal**: Complete understanding of the current security posture and secure secret management before any auth work begins
**Depends on**: Nothing (first phase of v12.0)
**Requirements**: AUDIT-01, AUDIT-02, AUDIT-03, AUDIT-04, AUDIT-05
**Success Criteria** (what must be TRUE):
  1. Every API route (80+) has a documented classification: public, customer, staff, admin, or service
  2. Every location where customer PII is stored or logged is identified (SQLite columns, log files, bot messages, cloud sync payloads, localStorage)
  3. JWT signing key and all secrets load from environment variables, not from racecontrol.toml
  4. A cryptographically random JWT key is auto-generated on first run if no key is set
  5. CORS, HTTPS, and auth state is documented for every service (racecontrol, rc-agent, kiosk, dashboard, cloud)
**Plans**: 2 plans

Plans:
- [ ] 75-01-PLAN.md -- Security audit document (endpoint inventory, PII map, CORS/HTTPS/auth state)
- [ ] 75-02-PLAN.md -- Secrets env var migration + JWT key auto-generation

### Phase 76: API Authentication & Admin Protection
**Goal**: No unauthenticated request can manipulate billing, start sessions, or access the admin panel
**Depends on**: Phase 75
**Requirements**: AUTH-01, AUTH-02, AUTH-03, AUTH-04, AUTH-05, AUTH-06, ADMIN-01, ADMIN-02, ADMIN-03, SESS-01, SESS-02, SESS-03
**Success Criteria** (what must be TRUE):
  1. A curl request to any billing or session endpoint without a valid JWT returns 401 Unauthorized
  2. The admin dashboard requires a PIN/password before any page loads -- no content visible without authentication
  3. Admin PIN is stored as an argon2 hash -- no plaintext PIN exists anywhere in config or database
  4. After 5 failed PIN/OTP attempts from an IP, further attempts are rate-limited (429 response)
  5. A Discord/WhatsApp bot command to start a session checks wallet balance before launching -- zero-balance users are rejected
  6. Pod agent endpoints (8090/8091) reject requests without valid HMAC signatures
  7. Session launch atomically deducts balance and creates billing record -- no race condition can produce a free session
**Plans**: 6 plans

Plans:
- [x] 76-01-PLAN.md -- Route classification + staff JWT middleware (AUTH-01,02,03, SESS-01)
- [x] 76-02-PLAN.md -- Admin login + argon2 PIN hashing (ADMIN-01,02)
- [x] 76-03-PLAN.md -- rc-agent service key auth (AUTH-06)
- [x] 76-04-PLAN.md -- Rate limiting + bot wallet check + session integrity (AUTH-04,05, SESS-02,03)
- [x] 76-05-PLAN.md -- Dashboard frontend PIN gate + idle timeout (ADMIN-01,03)
- [ ] 76-06-PLAN.md -- Switch permissive to strict JWT enforcement (AUTH-01,02,03, SESS-01)

### Phase 77: Transport Security
**Goal**: All browser-to-server traffic (PWA and admin dashboard) is encrypted in transit
**Depends on**: Phase 76
**Requirements**: TLS-01, TLS-02, TLS-03, TLS-04, KIOSK-06
**Success Criteria** (what must be TRUE):
  1. Customer PWA loads over HTTPS -- browser shows secure connection indicator
  2. Admin dashboard loads over HTTPS on the LAN
  3. Cloud endpoints (racingpoint.cloud) serve valid Let's Encrypt TLS certificates
  4. Pods can be migrated one-by-one from HTTP to HTTPS via dual-port support (8080 HTTP + 8443 HTTPS)
  5. Security response headers (CSP, X-Frame-Options, X-Content-Type-Options, HSTS) are present on all HTML responses
**Plans**: 2 plans
Plans:
- [ ] 77-01-PLAN.md — TLS cert generation module, ServerConfig extension, rcgen + axum-server deps (TLS-02, TLS-04)
- [ ] 77-02-PLAN.md — Dual-port HTTPS wiring, security headers, CORS update, kiosk API_BASE fix, TLS-03 Bono coordination (TLS-01, TLS-03, TLS-04, KIOSK-06)

### Phase 78: Kiosk & Session Hardening
**Goal**: A customer sitting at a pod cannot escape the kiosk, access other users' data, or keep a session running after payment expires
**Depends on**: Phase 76
**Requirements**: KIOSK-01, KIOSK-02, KIOSK-03, KIOSK-04, KIOSK-05, KIOSK-07, SESS-04, SESS-05
**Success Criteria** (what must be TRUE):
  1. Chrome DevTools, extensions, file:// protocol, and address bar are inaccessible on pod kiosk browsers
  2. Win+R, Alt+Tab, Ctrl+Alt+Del, Alt+F4, and Sticky Keys shortcuts are blocked on pod machines
  3. USB mass storage devices are rejected when plugged into pod machines
  4. Kiosk PWA cannot navigate to /admin or /staff routes -- server rejects with 403
  5. When a billing session ends, the kiosk locks automatically within 10 seconds -- no continued access
  6. A kiosk escape attempt (unauthorized process detected, DevTools open) triggers automatic session pause and WhatsApp alert
**Plans**: 3 plans

Plans:
- [ ] 78-01-PLAN.md — Pod lockdown: Edge kiosk flags, keyboard hook enhancement, USB/accessibility/TaskMgr registry (KIOSK-01, KIOSK-02, KIOSK-03, KIOSK-04)
- [ ] 78-02-PLAN.md — Network source tagging middleware + staff route protection from pod IPs (KIOSK-07, KIOSK-05)
- [ ] 78-03-PLAN.md — Session-scoped kiosk tokens + KioskLockdown auto-pause billing + WhatsApp alert (SESS-04, SESS-05)

### Phase 79: Data Protection
**Goal**: Customer PII is encrypted at rest and scrubbed from logs, with self-service data export and deletion
**Depends on**: Phase 77
**Requirements**: DATA-01, DATA-02, DATA-03, DATA-04, DATA-05, DATA-06
**Success Criteria** (what must be TRUE):
  1. Opening the SQLite database directly shows encrypted (unreadable) values for phone, email, name, and guardian_phone columns
  2. OTP login still works -- phone number lookup uses a deterministic hash, display uses reversible decryption
  3. Application logs and bot messages contain no raw phone numbers, emails, or names -- all PII is redacted
  4. A customer can request a JSON export of their own data via the PWA
  5. A customer can request deletion of their account and all associated data
**Plans**: 3 plans

Plans:
- [ ] 79-01-PLAN.md -- Crypto foundation: FieldCipher + key management + AppState wiring
- [ ] 79-02-PLAN.md -- Schema migration + query updates + log redaction
- [ ] 79-03-PLAN.md -- Customer data export + deletion endpoints

### Phase 80: Audit Trail & Defense in Depth
**Goal**: Every sensitive admin action is logged and alertable, with remaining security gaps closed
**Depends on**: Phase 76
**Requirements**: ADMIN-04, ADMIN-05, ADMIN-06, AUTH-07
**Success Criteria** (what must be TRUE):
  1. Every wallet topup, pricing change, session override, fleet exec, and terminal command is recorded in an append-only audit_log table with timestamp, actor, and action details
  2. Admin login and sensitive actions (wallet topup, fleet exec) trigger a WhatsApp notification to Uday
  3. If the admin PIN has not been changed in 30+ days, Uday receives an alert prompting rotation
  4. Cloud sync payloads are signed with HMAC-SHA256 including timestamp and nonce -- replayed or tampered payloads are rejected
**Plans**: 2 (researched + planned 2026-03-21)

Plans:
- [ ] 80-01-PLAN.md -- Audit trail infrastructure + WA alerts on admin actions (ADMIN-04, ADMIN-05)
- [ ] 80-02-PLAN.md -- PIN rotation alerting + HMAC-SHA256 sync signing (ADMIN-06, AUTH-07)

## v13.0 Multi-Game Launcher -- Phase Details

### Phase 81: Game Launch Core
**Goal**: Staff can launch any supported game on any pod from the kiosk, see what is running where, and recover from crashes without manual intervention
**Depends on**: Nothing (first phase of v13.0)
**Requirements**: LAUNCH-01, LAUNCH-02, LAUNCH-03, LAUNCH-04, LAUNCH-05, LAUNCH-06
**Success Criteria** (what must be TRUE):
  1. Staff selects F1 25, iRacing, AC EVO, EA WRC, or LMU from kiosk and the game launches on the target pod with safe defaults
  2. Customer can request a game from PWA/QR and staff sees the request in kiosk for confirmation
  3. If a game crashes or hangs, rc-agent detects it within 30 seconds, cleans up stale processes, and alerts staff with relaunch option
  4. Kiosk and fleet health dashboard show which game is running on which pod in real time
  5. Each game has a TOML launch profile defining exe path, launch args, and safe defaults
**Plans**: 3 plans

Plans:
- [ ] 81-01-PLAN.md -- Backend: non-AC crash recovery + DashboardEvent variant + PWA game request endpoint
- [ ] 81-02-PLAN.md -- Frontend: GamePickerPanel direct launch + GameLaunchRequestBanner + game logos + pod card display
- [ ] 81-03-PLAN.md -- TOML game profiles + end-to-end build verification + kiosk visual checkpoint

### Phase 82: Billing and Session Lifecycle
**Goal**: Customers are charged only for actual gameplay time, with billing starting when the game is playable and stopping cleanly on exit or crash
**Depends on**: Phase 81
**Requirements**: BILL-01, BILL-02, BILL-03, BILL-04, BILL-05
**Success Criteria** (what must be TRUE):
  1. Billing does not start during loading screens or shader compilation -- only when the game reports a playable state
  2. Each game has a configurable credit-per-minute rate in the billing_rates table
  3. When a game exits normally, crashes, or the session ends, billing stops automatically
  4. The full session lifecycle (launch, loading, playable, gameplay, exit, cleanup) is observable in logs and kiosk state
**Plans**: 2 (researched + planned 2026-03-21)

Plans:
- [ ] 82-01-PLAN.md — Server billing foundation: shared types, DB migration, per-game rate engine, protocol update
- [ ] 82-02-PLAN.md — Agent PlayableSignal dispatch: per-sim billing triggers, 30s exit grace timer, Loading state
- [ ] 82-03-PLAN.md — UI updates: admin pricing Game column, kiosk Loading state badge with count-up timer

### Phase 83: F1 25 Telemetry
**Goal**: F1 25 lap times and sector splits are captured and emitted as structured events
**Depends on**: Phase 82
**Requirements**: TEL-F1-01, TEL-F1-02, TEL-F1-03
**Success Criteria** (what must be TRUE):
  1. F1 25 UDP telemetry is received on port 20777 during gameplay
  2. Lap times and sector splits are extracted from telemetry packets after each completed lap
  3. Each completed lap emits an AgentMessage::LapCompleted with sim_type = F1_25
**Plans**: 2 (researched + planned 2026-03-21)

Plans:
- [ ] 83-01: TBD

### Phase 84: iRacing Telemetry
**Goal**: iRacing lap times and sector splits are captured via shared memory with reliable session transition handling
**Depends on**: Phase 82
**Requirements**: TEL-IR-01, TEL-IR-02, TEL-IR-03, TEL-IR-04
**Success Criteria** (what must be TRUE):
  1. iRacing shared memory is read using winapi OpenFileMappingA during active sessions
  2. When iRacing transitions between races, the adapter re-opens the shared memory handle without losing data
  3. Lap times and sector splits are extracted and emitted as LapCompleted events with correct timing
  4. On launch, the adapter checks irsdkEnableMem=1 in app.ini and warns staff if missing
**Plans**: 2 (researched + planned 2026-03-21)

Plans:
- [ ] 84-01: TBD
- [ ] 84-02: TBD

### Phase 85: LMU Telemetry
**Goal**: Le Mans Ultimate lap times are captured via rFactor 2 shared memory plugin
**Depends on**: Phase 82
**Requirements**: TEL-LMU-01, TEL-LMU-02, TEL-LMU-03
**Success Criteria** (what must be TRUE):
  1. LMU shared memory is read using rFactor 2 shared memory plugin mapped files ($rFactor2SMMP_*)
  2. Lap times and sector splits are extracted from rF2 scoring data after each completed lap
  3. Each completed lap emits a LapCompleted event with sim_type = LMU
**Plans**: 2 (researched + planned 2026-03-21)

Plans:
- [ ] 85-01: TBD

### Phase 86: AC EVO Telemetry
**Goal**: AC EVO telemetry is captured on a best-effort basis with graceful degradation when data is unavailable
**Depends on**: Phase 82
**Requirements**: TEL-EVO-01, TEL-EVO-02, TEL-EVO-03
**Success Criteria** (what must be TRUE):
  1. AC EVO shared memory is read using ACC-format struct layout when data is available
  2. If telemetry fields are unpopulated or the API changes, the adapter logs a warning and continues without crashing
  3. When lap data is available, it is emitted as LapCompleted with sim_type = AC_EVO
**Plans**: 2 (researched + planned 2026-03-21)

Plans:
- [ ] 86-01: TBD

### Phase 87: EA WRC Telemetry
**Goal**: EA WRC stage times are captured via UDP and mapped to the lap schema for leaderboard compatibility
**Depends on**: Phase 82
**Requirements**: TEL-WRC-01, TEL-WRC-02, TEL-WRC-03
**Success Criteria** (what must be TRUE):
  1. EA WRC UDP telemetry is received on port 20432 using JSON-configured packet format
  2. Stage completion times are captured and mapped to the laps schema
  3. If WRC telemetry config is unavailable, the game still launches and billing works
**Plans**: 2 (researched + planned 2026-03-21)

Plans:
- [ ] 87-01: TBD

### Phase 88: Leaderboard Integration
**Goal**: Lap and stage times from all games appear on the existing Racing Point leaderboard with correct track names
**Depends on**: Phase 83, Phase 84, Phase 85, Phase 86, Phase 87 (at least one adapter producing data)
**Requirements**: LB-01, LB-02, LB-03
**Success Criteria** (what must be TRUE):
  1. Lap/stage times from all adapters are stored in the existing laps table with a sim_type field
  2. A track name mapping table translates per-game track identifiers to Racing Point canonical track names
  3. Existing leaderboard endpoints serve multi-game data and support filtering by sim_type
**Plans**: 2 (researched + planned 2026-03-21)

Plans:
- [ ] 88-01: TBD
- [ ] 88-02: TBD

### Phase 106: Structured Log Labels

**Goal:** Every rc-agent tracing call carries a structured target: label and build_id propagates via root span — enabling per-module RUST_LOG filtering and binary-traceable log lines
**Requirements**: LOG-01 (build_id root span), LOG-02 (module target labels), LOG-03 (strip bracket prefixes), LOG-04 (full test suite green)
**Depends on:** Phase 105
**Plans:** 1/1 plans complete

Plans:
- [ ] 106-01-PLAN.md — build_id root span + main.rs migration (66 calls)
- [ ] 106-02-PLAN.md — ws_handler, event_loop, ac_launcher migration (164 calls)
- [ ] 106-03-PLAN.md — ffb_controller, ai_debugger, kiosk, lock_screen migration (114 calls)
- [ ] 106-04-PLAN.md — remote_ops, self_monitor, self_heal, game_process, overlay, billing_guard, pre_flight migration (79 calls)
- [ ] 106-05-PLAN.md — small files + sim modules migration (68 calls)
- [ ] 106-06-PLAN.md — final audit + full test suite verification

---

## v14.0 HR & Marketing Psychology -- Phase Details

### Phase 89: Psychology Foundation
**Goal**: The platform has a centralized psychology engine with notification throttling so every subsequent phase can trigger badges, streaks, and messages without spamming customers
**Depends on**: Nothing (first phase in v14.0)
**Requirements**: FOUND-01, FOUND-02, FOUND-03, FOUND-04, FOUND-05
**Success Criteria** (what must be TRUE):
  1. No customer receives more than 2 proactive WhatsApp messages per day, enforced at the system level
  2. A new psychology.rs module exists in RaceControl that centralizes badge evaluation, streak tracking, and notification dispatch
  3. Badge criteria are stored as JSON rows in the database and can be modified without code changes
  4. Notifications route through a priority queue that selects the correct channel (WhatsApp, Discord, or PWA)
  5. All psychology tables (achievements, streaks, driving_passport, nudge_queue, staff_badges, staff_challenges) exist in the database
**Plans**: 3 (researched + planned 2026-03-21)

Plans:
- [ ] 89-01: DB schema (7 tables + indexes) + psychology.rs skeleton with types and JSON criteria evaluation
- [ ] 89-02: Badge evaluation, streak tracking, notification budget enforcement, queue dispatcher with channel routing
- [ ] 89-03: Integration wiring: post_session_hooks, dispatcher startup, seed badges, API endpoints

### Phase 90: Customer Progression
**Goal**: Customers can see their driving journey as a passport with track/car collections and earned badges, driving return visits through Zeigarnik-motivated completion
**Depends on**: Phase 89
**Requirements**: PROG-01, PROG-02, PROG-03, PROG-04, PROG-05
**Success Criteria** (what must be TRUE):
  1. Customer can open a driving passport page in the PWA showing which tracks and cars they have driven
  2. Passport displays tiered collections (Starter/Explorer/Legend) so newcomers see achievable near-term goals
  3. Returning customers see their existing lap history already backfilled into the passport on first load
  4. Customers earn badges for milestones (first lap, 10 tracks, 100 laps, PB streak) and can view them on their profile page
**Plans**: 2 (researched + planned 2026-03-21)

Plans:
- [ ] 90-01: Backend — driving passport upsert in persist_lap, backfill function, catalog accessors, customer API endpoints (/customer/passport, /customer/badges)
- [ ] 90-02: Frontend — PWA /passport page with tiered collections, badge showcase on /profile, api.ts methods

### Phase 91: Session Experience
**Goal**: Every sim racing session ends on a high note with PB celebrations and peak-end-optimized reports
**Depends on**: Phase 90
**Requirements**: SESS-01, SESS-02, SESS-03, SESS-04
**Success Criteria** (what must be TRUE):
  1. When a customer sets a personal best during a session, a confetti animation plays on their PWA session view
  2. Session-end reports show the best moment first before displaying averages
  3. Session-end report includes percentile ranking ("faster than 73% of drivers")
  4. Customer sees a real-time toast notification in the PWA when they set a PB during an active session
**Plans**: 2 plans

Plans:
- [ ] 91-01-PLAN.md — Backend: PbAchieved event broadcast, shared percentile function, enhanced session detail API, active session events polling endpoint, NPM dependencies
- [ ] 91-02-PLAN.md — Frontend: Confetti component, sonner Toaster, peak-end session detail rewrite, active session PB polling with toast

### Phase 92: Retention Loops
**Goal**: Customers are drawn back through streaks, loss-framed notifications, and unpredictable bonus rewards
**Depends on**: Phase 91
**Requirements**: RET-01, RET-02, RET-03, RET-04, RET-05, RET-06
**Success Criteria** (what must be TRUE):
  1. System tracks each customer's weekly visit streak with a 1-week grace period
  2. When someone beats a customer's PB, that customer receives a WhatsApp notification (throttled, within budget)
  3. Customers occasionally receive surprise bonus credits on PBs (15%) or milestones (10%), capped at 5% of spend
  4. Membership expiry warnings use loss-framed copy
  5. Streak-at-risk WhatsApp nudge sent 2 days before grace period expires
**Plans**: 2 (researched + planned 2026-03-21)

Plans:
- [ ] 92-01-PLAN.md — Backend: variable_reward_log table, 4 retention functions (PB beaten notify, variable rewards, streak-at-risk, membership expiry), wiring in lap_tracker/billing/scheduler/passport API
- [ ] 92-02-PLAN.md — PWA: Passport streak card with grace urgency indicator, longest streak display, full build verification

### Phase 93: Community & Tribal Identity
**Goal**: Discord becomes a living community hub with automated weekly rituals and record alerts
**Depends on**: Phase 90
**Requirements**: COMM-01, COMM-02, COMM-03, COMM-04
**Success Criteria** (what must be TRUE):
  1. Discord bot posts a formatted weekly leaderboard summary automatically
  2. When a new track record is set, the Discord bot announces it within 1 hour
  3. All customer-facing copy uses "RacingPoint Driver" instead of "customer"
  4. Discord has weekly time trial challenge posts and tournament bracket update posts
**Plans**: 2 plans (planned 2026-03-21)

Plans:
- [ ] 93-01-PLAN.md — Discord bot scheduler: weekly leaderboard, track record alerts, time trial/tournament ritual posts (COMM-01, COMM-02, COMM-04)
- [ ] 93-02-PLAN.md — Copy identity sweep: replace "customer" with "driver" across WhatsApp bot, Discord bot, and PWA (COMM-03)

### Phase 94: Pricing & Conversion
**Goal**: Booking and pricing experience uses anchoring, real scarcity, and social proof to increase conversion
**Depends on**: Phase 89
**Requirements**: PRICE-01, PRICE-02, PRICE-03, PRICE-04
**Success Criteria** (what must be TRUE):
  1. Pricing page displays 3-tier structure with middle tier visually emphasized (decoy/anchoring)
  2. Booking wizard shows real-time pod availability from live RaceControl data
  3. System tracks each customer's commitment ladder position and surfaces next-step nudges
  4. Booking page displays real social proof using actual data
**Plans**: 2 plans

Plans:
- [x] 94-01-PLAN.md — Backend: pricing display + social proof endpoints, DB migration, commitment ladder logic
- [x] 94-02-PLAN.md — Frontend: kiosk PricingDisplay + ScarcityBanner, web /book page with social proof
### Phase 95: Staff Gamification
**Goal**: Staff who opt in can see performance, earn badges, participate in team challenges, and give recognition
**Depends on**: Phase 89
**Requirements**: STAFF-01, STAFF-02, STAFF-03, STAFF-04, STAFF-05
**Success Criteria** (what must be TRUE):
  1. Opted-in staff see a performance leaderboard in venue dashboard
  2. Staff earn skill badges based on observable actions, not manager assignment
  3. Team challenges with collective goals appear with progress tracking
  4. Staff can give kudos to colleagues visible in dashboard
  5. Participation is per-employee opt-in, never mandatory
**Plans**: 2 (researched + planned 2026-03-21)

Plans:
- [x] 95-01-PLAN.md — Backend: DB migrations, seed badges, gamification API endpoints + Admin dashboard page

### Phase 96: HR & Hiring Psychology
**Goal**: Hiring pipeline uses SJTs, campaigns apply Cialdini principles, review nudges are optimized
**Depends on**: Phase 89
**Requirements**: HR-01, HR-02, HR-03, HR-04, HR-05
**Success Criteria** (what must be TRUE):
  1. Hiring bot presents 3 hospitality-specific SJT scenarios
  2. Hiring bot includes realistic job preview content
  3. WhatsApp campaign templates use Cialdini principles (3+ ready-to-send)
  4. Review nudge copy uses loss-framed messaging with peak-end timing
  5. Admin dashboard has employee recognition page
**Plans**: 2 (researched + planned 2026-03-21)

Plans:
- [x] 96-01-PLAN.md — Backend: SJT/job preview/campaign/nudge tables + seed data + API endpoints + Admin recognition page

## Progress

**Execution Order:**
Phases execute in numeric order: 36 → 37 → 38 → 39 → 40 → 41 → 42 → 43 → 44 → 45 → 46 → 47 → 48 → 49 → 50 → 51 → 52 → 53 → 54 → 55 → 56

Note: v8.0 phases 45–50 build on v7.0's shipped E2E infrastructure (lib/common.sh, lib/pod-map.sh, Playwright, run-all.sh). Every phase includes E2E test scripts wired into run-all.sh as new fleet/ and api/ test phases. Phases 45 (CLOSE_WAIT) and 46 (Panic Hook) have no dependencies. Phase 47 (LLM Fleet) depends on 45. Phase 48 (Dynamic Allowlist) depends on 47 (needs local LLM for process classifier). Phase 49 (Session Lifecycle) depends on 46. Phase 50 (Self-Test) depends on 46+47 and is the capstone — its pod-health.sh becomes the final gate in run-all.sh.

Note: Phase 36 (WSL2 Infrastructure) is the non-negotiable critical path — the mirrored networking and Hyper-V firewall must be verified from an actual pod before any minion is installed or any Rust code is written. Phase 37 (Pod 8 Canary) validates the networking with a real minion and rewrites install.bat — this template is reused in Phase 40. Phase 38 (salt_exec.rs) must compile and be tested against live Pod 8 before any module is considered migrated. Phase 39 (remote_ops.rs Removal) requires characterization tests before any deletion — Refactor Second standing rule. Phase 40 (Fleet Rollout) is the irreversible step; no billing session should be interrupted.

For v7.0: Phase 41 (Foundation) must complete before any script can source the shared library. Phase 42 (Kiosk Source Prep) must add data-testid attributes before Phase 43 wizard specs can select wizard elements. Phase 43 (Wizard + API) must complete before Phase 44 can wire run-all.sh around phase scripts that do not yet exist.

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. State Wiring & Config Hardening | v1.0 | 3/3 | Complete | 2026-03-13 |
| 2. Watchdog Hardening | v1.0 | 3/3 | Complete | 2026-03-13 |
| 3. WebSocket Resilience | v1.0 | 3/3 | Complete | 2026-03-13 |
| 4. Deployment Pipeline Hardening | v1.0 | 3/3 | Complete | 2026-03-13 |
| 5. Blanking Screen Protocol | v1.0 | 3/3 | Complete | 2026-03-13 |
| 6. Diagnosis | v2.0 | 2/2 | Complete | 2026-03-13 |
| 7. Server-Side Pinning | v2.0 | 2/2 | Complete | 2026-03-14 |
| 8. Pod Lock Screen Hardening | v2.0 | 3/3 | Complete | 2026-03-14 |
| 9. Edge Browser Hardening | v2.0 | 1/1 | Complete | 2026-03-14 |
| 10. Staff Dashboard Controls | v2.0 | 2/2 | Complete | 2026-03-14 |
| 11. Customer Experience Polish | v2.0 | 2/2 | Complete | 2026-03-14 |
| 12. Data Foundation | v3.0 | 2/2 | Complete | 2026-03-14 |
| 13. Leaderboard Core | v3.0 | 5/5 | Complete | 2026-03-15 |
| 13.1. Pod Fleet Reliability | v3.0 | 3/3 | Complete | 2026-03-15 |
| 14. Events and Championships | v3.0 | 5/5 | Complete | 2026-03-16 |
| 15. Telemetry and Driver Rating | v3.0 | 0/? | Deferred | - |
| 16. Firewall Auto-Config | v4.0 | 1/1 | Complete | 2026-03-15 |
| 17. WebSocket Exec | v4.0 | 3/3 | Complete | 2026-03-15 |
| 18. Startup Self-Healing | v4.0 | 2/2 | Complete | 2026-03-15 |
| 19. Watchdog Service | v4.0 | 2/2 | Complete | 2026-03-15 |
| 20. Deploy Resilience | v4.0 | 2/2 | Complete | 2026-03-15 |
| 21. Fleet Health Dashboard | v4.0 | 2/2 | Complete | 2026-03-15 |
| 22. Pod 6/7/8 Recovery + Remote Restart Reliability | v4.0 | 2/2 | Complete | 2026-03-16 |
| 28. Billing-Game Lifecycle | v4.5 | 2/2 | Complete | 2026-03-16 |
| 29. Game Crash Recovery | v4.5 | 2/2 | Complete | 2026-03-16 |
| 30. Launch Resilience | v4.5 | 2/2 | Complete | 2026-03-16 |
| 31. Multiplayer Server Lifecycle | v4.5 | 2/2 | Complete | 2026-03-16 |
| 32. Synchronized Group Play | v4.5 | 2/2 | Complete | 2026-03-16 |
| 23. Protocol Contract + Concurrency Safety | v5.0 | 2/2 | Complete | 2026-03-16 |
| 24. Crash, Hang, Launch + USB Bot Patterns | v5.0 | 4/4 | Complete | 2026-03-16 |
| 25. Billing Guard + Server Bot Coordinator | v5.0 | 4/4 | Complete | 2026-03-16 |
| 26. Lap Filter, PIN Security, Telemetry + Multiplayer | v5.0 | 4/4 | Complete | 2026-03-16 |
| 27. Tailscale Mesh + Internet Fallback | v5.0 | 5/5 | Complete | 2026-03-16 |
| 33. DB Schema + Billing Engine | v5.5 | 1/1 | Complete | 2026-03-17 |
| 34. Admin Rates API | v5.5 | 1/1 | Complete | 2026-03-17 |
| 35. Credits UI | v5.5 | 1/1 | Complete | 2026-03-17 |
| 36. WSL2 Infrastructure | v6.0 | 0/2 | Not started | - |
| 37. Pod 8 Minion Bootstrap | v6.0 | 0/2 | Not started | - |
| 38. salt_exec.rs + Server Module Migration | v6.0 | 0/3 | Not started | - |
| 39. remote_ops.rs Removal | v6.0 | 0/3 | Not started | - |
| 40. Fleet Rollout | v6.0 | 0/2 | Not started | - |
| 41. Test Foundation | v7.0 | Complete    | 2026-03-18 | 2026-03-19 |
| 42. Kiosk Source Prep + Browser Smoke | 2/2 | Complete    | 2026-03-18 | - |
| 43. Wizard Flows + API Pipeline Tests | 2/2 | Complete    | 2026-03-18 | - |
| 44. Deploy Verification + Master Script | 2/2 | Complete   | 2026-03-18 | - |
| 45. CLOSE_WAIT Fix + Connection Hygiene | 2/2 | Complete   | 2026-03-19 | - |
| 46. Crash Safety + Panic Hook | 2/2 | Complete   | 2026-03-19 | - |
| 47. Local LLM Fleet Deployment | 2/2 | Complete   | 2026-03-19 | - |
| 48. Dynamic Kiosk Allowlist | 2/2 | Complete   | 2026-03-19 | - |
| 49. Session Lifecycle Autonomy | 2/2 | Complete    | 2026-03-19 | - |
| 50. LLM Self-Test + Fleet Health | 3/3 | Complete    | 2026-03-19 | - |
| 51. CLAUDE.md + Custom Skills | 2/2 | Complete    | 2026-03-20 | - |
| 52. MCP Servers | 2/2 | Complete    | 2026-03-20 | - |
| 53. Deployment Automation | 2/2 | Complete    | 2026-03-20 | - |
| 54. Structured Logging + Error Rate Alerting | 3/3 | Complete    | 2026-03-20 | - |
| 55. Netdata Fleet Deploy | 2/2 | Complete   | 2026-03-20 | - |
| 56. WhatsApp Alerting + Weekly Report | 2/2 | Complete    | 2026-03-20 | - |
| 57. Session-End Safety | 2/3 | Complete    | 2026-03-20 | - |
| 58. ConspitLink Process Hardening | 1/2 | In Progress|  | - |
| 59. Auto-Switch Configuration | 4/4 | Complete    | 2026-03-24 | - |
| 60. Pre-Launch Profile Loading | 1/1 | Complete   | 2026-03-24 | - |
| 61. FFB Preset Tuning | v10.0 | 0/? | Not started | - |
| 62. Fleet Config Distribution | v10.0 | 0/? | Not started | - |
| 63. Fleet Monitoring | v10.0 | 0/? | Not started | - |
| 64. Telemetry Dashboards | v10.0 | 0/? | Not started | - |
| 65. Shift Lights & RGB Lighting | v10.0 | 0/? | Not started | - |
| 66. Infrastructure Foundations | 5/5 | Complete    | 2026-03-20 | - |
| 67. Config Sync | 2/2 | Complete    | 2026-03-20 | - |
| 68. Pod SwitchController | 2/2 | Complete    | 2026-03-20 | - |
| 69. Health Monitor & Failover Orchestration | 4/4 | Complete    | 2026-03-21 | - |
| 70. Failback & Data Reconciliation | 2/2 | Complete    | 2026-03-21 | - |
| 71. rc-common Foundation + rc-sentry Core Hardening | 2/2 | Complete    | 2026-03-20 | - |
| 72. rc-sentry Endpoint Expansion + Integration Tests | 2/2 | Complete    | 2026-03-20 | - |
| 73. Critical Business Tests | 2/2 | Complete    | 2026-03-20 | - |
| 74. rc-agent Decomposition | 4/4 | Complete    | 2026-03-21 | - |
| 75. Security Audit & Foundations | 2/2 | Complete    | 2026-03-20 | - |
| 76. API Authentication & Admin Protection | 6/6 | Complete    | 2026-03-20 | - |
| 77. Transport Security | 2/2 | Complete    | 2026-03-20 | - |
| 78. Kiosk & Session Hardening | 3/3 | Complete    | 2026-03-21 | - |
| 79. Data Protection | 3/3 | Complete    | 2026-03-21 | - |
| 80. Audit Trail & Defense in Depth | 2/2 | Complete    | 2026-03-21 | - |
| 81. Game Launch Core | 3/3 | Complete   | 2026-03-21 | - |
| 82. Billing and Session Lifecycle | 3/3 | Complete | 2026-03-21 | - |
| 83. F1 25 Telemetry | 1/1 | Complete | 2026-03-21 | - |
| 84. iRacing Telemetry | 2/2 | Complete | 2026-03-21 | - |
| 85. LMU Telemetry | 2/2 | Complete | 2026-03-21 | - |
| 86. AC EVO Telemetry | 1/1 | Complete | 2026-03-21 | - |
| ~~87. EA WRC Telemetry~~ | — | ARCHIVED | 2026-03-25 | No WRC units at venue |
| 88. Leaderboard Integration | 2/2 | Complete | 2026-03-21 | - |
| 89. Psychology Foundation | 3/3 | Complete    | 2026-03-21 | - |
| 90. Customer Progression | 1/2 | Complete    | 2026-03-21 | - |
| 91. Session Experience | 2/2 | Complete | 2026-03-21 |
| 92. Retention Loops | 2/2 | Complete   | 2026-03-21 | - |
| 93. Community & Tribal Identity | 2/2 | Complete   | 2026-03-21 | - |
| 94. Pricing & Conversion | v14.0 | 2/2 | Complete | 2026-03-24 |
| 95. Staff Gamification | v14.0 | 1/1 | Complete | 2026-03-24 |
| 96. HR & Hiring Psychology | v14.0 | 1/1 | Complete | 2026-03-24 |
# Roadmap: v11.1 Pre-Flight Session Checks

## Overview

Every customer session begins with automated health verification. On BillingStarted, rc-agent runs 8-10 targeted checks concurrently (tokio::join! with a 5-second hard timeout), attempts one auto-fix per failure, and either clears into the active session or blocks the pod with a "Maintenance Required" lock screen. Staff are notified exactly once per fault transition via WebSocket and kiosk badge. The implementation is a pure integration exercise — zero new Rust crates, one new module (pre_flight.rs), and surgical modifications to four existing files. Build order is compiler-dependency-driven: rc-common protocol first, then lock screen state, then check logic, then handler wiring, then server-side staff UX.

## Phases

- [x] **Phase 97: rc-common Protocol + pre_flight.rs Framework + Hardware Checks** - New AgentMessage variants, pre_flight.rs module with concurrent check gate, HID wheelbase check, ConspitLink two-stage check with auto-restart, orphan game kill with PID-targeted safe-kill, and disable_preflight config flag
 (completed 2026-03-21)
- [ ] **Phase 98: MaintenanceRequired Lock Screen + Display Checks** - New LockScreenState variant with show_maintenance_required(), ClearMaintenance handler, 30-second auto-retry loop, display checks (HTTP probe :18923, GetWindowRect), and pod-unavailable server marking
- [ ] **Phase 99: System + Network + Billing Checks + BillingStarted Handler Wiring** - Billing stuck-session check, disk and memory probes, WS stability check, complete handler integration in ws_handler.rs, self_test.rs pub(crate) helper extraction, alert rate-limiting
- [ ] **Phase 100: Staff Visibility — Kiosk Badge + Fleet Health + Manual Clear** - Kiosk dashboard maintenance badge per pod, "Clear Maintenance" staff action (PIN-gated), pod marked unavailable in fleet health, preflight_alert_cooldown_secs config

## Phase Details

### Phase 97: rc-common Protocol + pre_flight.rs Framework + Hardware Checks
**Goal**: The foundational layer exists and compiles — new AgentMessage variants in rc-common are available to rc-agent, pre_flight.rs owns the concurrent check gate with a hard 5-second timeout, and the three highest-value hardware checks (HID wheelbase, ConspitLink process+config, orphaned game PID-targeted kill) run correctly with one auto-fix attempt each
**Depends on**: Phase 89 (v14.0 Psychology Foundation)
**Requirements**: PF-01, PF-02, PF-03, PF-07, HW-01, HW-02, HW-03, SYS-01
**Success Criteria** (what must be TRUE):
  1. cargo build --bin rc-agent and cargo build --bin racecontrol both succeed after rc-common protocol.rs changes — compiler validates new AgentMessage variants exist before any rc-agent code references them
  2. On a healthy pod with wheelbase connected and ConspitLink running, pre_flight::run() returns PreFlightResult::Pass within 5 seconds and logs "pre-flight passed" — concurrent gate executes without blocking the WS receive loop
  3. When a test simulates ConspitLink not running, rc-agent spawns ConspitLink, waits for process to appear, and if successful returns Pass — HW-03 auto-restart path executes once and stops
  4. When an orphaned game PID is in AppState (game_process is Some but billing_active is false), pre_flight kills that specific PID via taskkill /F /PID — name-based kill is never used and active sessions are never touched
  5. When disable_preflight = true in rc-agent.toml, BillingStarted proceeds directly to show_active_session() with no pre_flight::run() call — rollback escape hatch works
**Plans**: 2 plans
Plans:
- [ ] 97-01-PLAN.md — rc-common protocol variants + PreflightConfig
- [ ] 97-02-PLAN.md — pre_flight.rs module + ws_handler.rs gate wiring

### Phase 98: MaintenanceRequired Lock Screen + Display Checks
**Goal**: A pod that fails pre-flight shows a branded "Maintenance Required — Staff Notified" lock screen and stays blocked with two explicit exit paths — staff sends ClearMaintenance from kiosk, or 30 seconds of successful auto-retry self-clears the pod; display checks (HTTP probe and window rect) are wired into the pre-flight gate
**Depends on**: Phase 90
**Requirements**: PF-04, PF-05, PF-06, DISP-01, DISP-02
**Success Criteria** (what must be TRUE):
  1. When pre-flight fails and auto-fix cannot resolve it, the pod lock screen transitions to "Maintenance Required — Staff Notified" — customer never sees a raw error message or desktop
  2. A PreFlightFailed AgentMessage arrives at racecontrol within 5 seconds of the pod entering MaintenanceRequired, containing the list of failed check names — server has the information it needs to mark the pod
  3. Every 30 seconds while in MaintenanceRequired, the pod re-runs pre-flight silently; if all checks pass, the pod self-clears to Idle state without staff action — auto-retry loop works
  4. When racecontrol sends a ClearMaintenance message to a pod, the pod transitions from MaintenanceRequired to Idle and accepts the next BillingStarted — staff manual clear path works
  5. A GET to http://localhost:18923 returns HTTP 200 and the window rect of the lock screen Edge window is centered within 5% of the primary monitor center — display checks pass on a healthy pod
**Plans**: 2 plans

### Phase 99: System + Network + Billing Checks + BillingStarted Handler Wiring
**Goal**: All remaining checks are live (billing stuck-session, disk, memory, WebSocket stability) and the pre-flight gate is wired into ws_handler.rs — every BillingStarted now triggers the complete concurrent check gate before any session state is mutated; staff alerts fire exactly once per MaintenanceRequired entry, not once per failure
**Depends on**: Phase 91
**Requirements**: SYS-02, SYS-03, SYS-04, NET-01, STAFF-04
**Success Criteria** (what must be TRUE):
  1. When billing_active is true at BillingStarted time (stuck session from previous customer), pre-flight reports BillingStuck failure and does not start a new session — local atomic check, no HTTP round-trip
  2. When disk free on C: drops below 1GB in the sysinfo probe, pre-flight blocks the session and fires MaintenanceRequired; when disk is above 1GB, the check passes silently — disk probe runs every BillingStarted
  3. When WebSocket has been connected for less than 10 seconds or has disconnected and reconnected within the last 10 seconds, NET-01 reports a warning but does not block the session — flap-detection logic is correct
  4. Running pre-flight 20 consecutive times on a fully healthy pod produces zero failures and zero MaintenanceRequired transitions — no false positives from probe logic
  5. When a pod is already in MaintenanceRequired and BillingStarted arrives, racecontrol does not book the pod for a new customer — the server rejects the booking before it reaches the pod, and the pod also guards the BillingStarted arm with a state check
**Plans**: 2 plans

### Phase 100: Staff Visibility — Kiosk Badge + Fleet Health + Manual Clear
**Goal**: Staff can see at a glance which pods are in maintenance (Racing Red badge on kiosk dashboard), view failure reasons (PIN-gated), and manually clear a pod from the dashboard; maintenance pods appear as unavailable in fleet health; alert cooldown prevents notification floods
**Depends on**: Phase 92
**Requirements**: STAFF-01, STAFF-02, STAFF-03, STAFF-04
**Success Criteria** (what must be TRUE):
  1. When a pod enters MaintenanceRequired, the kiosk fleet grid shows a Racing Red (#E10600) "Maintenance" badge on that pod's card within one polling cycle — staff see the problem without checking logs
  2. A staff member who knows the PIN can click the maintenance badge to see failure details (failed check names, timestamp); the failure details are not visible on the kiosk grid without PIN confirmation — customer privacy maintained on venue TV screens
  3. Clicking "Clear Maintenance" on the kiosk dashboard (with PIN) sends ClearMaintenance to the pod, the pod transitions to Idle, and the dashboard badge disappears within one polling cycle
  4. In the fleet health dashboard (/api/v1/fleet/health), a pod in MaintenanceRequired shows status "maintenance" (not "healthy" or "offline") — fleet visibility is accurate
  5. If a pod enters MaintenanceRequired repeatedly within the preflight_alert_cooldown_secs window, only one WhatsApp/email alert fires — Uday does not receive notification floods from rapid re-entry
**Plans**: 2 plans

## Progress

**Execution Order:** 97 → 98 → 99 → 93

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 97. rc-common Protocol + Framework + Hardware | 2/2 | Complete   | 2026-03-21 |
| 98. MaintenanceRequired Lock Screen + Display | TBD | Not started | - |
| 99. System + Network + Billing + Handler Wiring | TBD | Not started | - |
| 100. Staff Visibility — Badge + Fleet + Manual Clear | TBD | Not started | - |

## v12.1 E2E Process Guard (continued)

- [x] **Phase 101: Protocol Foundation** - completed 2026-03-21
- [x] **Phase 102: Whitelist Schema + Config + Fetch Endpoint** - completed 2026-03-21
- [x] **Phase 103: Pod Guard Module** - completed 2026-03-21
- [x] **Phase 104: Server Guard Module + Alerts** - racecontrol process_guard.rs receiving violations, kiosk notification badge, email escalation, and fleet health integration
 (completed 2026-03-21)
- [x] **Phase 105: Port Audit + Scheduled Tasks + James Binary** - Listening port enforcement, scheduled task audit, and standalone rc-process-guard binary for James workstation
 (completed 2026-03-21)

## v12.1 Phase Details (continued)

### Phase 104: Server Guard Module + Alerts
**Goal**: The racecontrol server receives all pod violations, displays an active-violation badge on the staff kiosk, escalates repeat offenders to email, and surfaces violation counts in the fleet health endpoint
**Depends on**: Phase 103
**Requirements**: ALERT-02, ALERT-03, ALERT-05, DEPLOY-02
**Success Criteria** (what must be TRUE):
  1. Staff kiosk shows notification badge for active unacknowledged violations
  2. GET /api/v1/fleet/health includes violation_count_24h and last_violation_at per pod
  3. Three kills in 5 minutes triggers email to Uday with machine ID, process name, kill count
  4. racecontrol own guard reports CRITICAL if rc-agent.exe detected on server
Plans:
- [ ] 104-01: TBD
- [ ] 104-02: TBD

### Phase 105: Port Audit + Scheduled Tasks + James Binary
**Goal**: Listening ports audited against approved list, non-whitelisted scheduled tasks flagged, and James runs standalone rc-process-guard reporting via HTTP
**Depends on**: Phase 104
**Requirements**: PORT-01, PORT-02, AUTO-03, DEPLOY-03
**Success Criteria** (what must be TRUE):
  1. Non-whitelisted listening port process killed within one scan cycle
  2. Non-whitelisted scheduled task flagged in audit log with name, path, action
  3. rc-process-guard.exe on James POSTs violations via HTTP — never WebSocket
  4. James whitelist passes first run without false positives on legitimate tooling
Plans:
- [ ] 105-01: TBD
- [ ] 105-02: TBD


## v15.0 AntiCheat Compatibility

Audit and harden all pod-side RaceControl behaviors so that rc-agent, rc-sentry, and kiosk software never trigger anti-cheat detection in F1 25 (EA Javelin), iRacing (EOS), LMU (EAC), AC EVO, or EA WRC -- preventing customer account bans. Must complete before v13.0 Multi-Game Launcher deploys to customers.

- [x] **Phase 107: Behavior Audit + Certificate Procurement** - Exhaustive risk inventory of every pod-side behavior classified per anti-cheat system; ConspitLink signed-binary audit; code signing certificate procured and integrated into build pipeline (completed 2026-03-21)
- [x] **Phase 108: Keyboard Hook Replacement** - SetWindowsHookEx hook fully removed and replaced with GPO registry keys (NoWinKeys, DisableTaskMgr); kiosk lockdown verified without any hook on Pod 8 (completed 2026-03-21)
- [x] **Phase 109: Safe Mode State Machine** - safe_mode.rs module; WMI Win32_ProcessStartTrace event subscription for sub-second game detection; 30-second exit cooldown; process guard, Ollama queries, and registry writes gated; safe mode startup default (completed 2026-03-21)
- [x] **Phase 110: Telemetry Gating** - shm_connect_allowed() guard defers shared memory adapter connect by 5s; UDP sockets scoped to active game; AC EVO telemetry feature-flagged off by default (completed 2026-03-21)
- [ ] **Phase 111: Code Signing + Per-Game Canary Validation** - rc-agent.exe and rc-sentry.exe signed via signtool in deploy pipeline; staff test session per game (F1 25, iRacing, LMU) on Pod 8 with safe mode active; billing continuity verified during safe mode. **v22.0 integration:** Use OTA-10 (SHA256 binary identity in release manifest) for binary verification instead of custom hash checks. Use OTA-02 (canary Pod 8 wave) for canary deployment — do not build a separate canary system. Standing rules gate (gate-check.sh) required before deploy.

## v15.0 AntiCheat Compatibility -- Phase Details

### Phase 107: Behavior Audit + Certificate Procurement
**Goal**: The team has an exhaustive, classified inventory of every pod-side behavior that could trigger anti-cheat, and a code signing certificate is procured and integrated into the build pipeline before any canary testing begins
**Depends on**: Phase 106 (last shipped phase) -- can start in parallel with ongoing milestones
**Requirements**: AUDIT-01, AUDIT-02, AUDIT-03, AUDIT-04
**Success Criteria** (what must be TRUE):
  1. Staff can open a risk inventory document listing every pod-side behavior (keyboard hooks, process monitoring, shared memory access, UDP ports, registry writes, unsigned binaries) with CRITICAL/HIGH/MEDIUM/LOW severity per anti-cheat system (EA Javelin, iRacing EOS, LMU EAC, Kunos, EA WRC) -- no behavior is unlisted
  2. ConspitLink is audited via Sysinternals Process Monitor on Pod 8: the audit report documents whether ConspitLink installs kernel drivers, performs DLL injection, or opens handles to game processes -- the risk level is known before v15.0 canary testing begins
  3. All 8 pods have their Windows 11 edition verified (winver output documented) -- the Keyboard Filter vs GPO registry key decision for Phase 108 is made with confirmed facts, not assumptions
  4. Ops team has a per-game anti-cheat compatibility matrix document (racecontrol.toml safe_mode_subsystems section + ops reference doc) covering what is safe to run while each game is active
**Plans**: 2 plans

Plans:
- [ ] 107-01-PLAN.md — Risk inventory of all rc-agent anti-cheat behaviors + pod OS edition verification (AUDIT-01, AUDIT-03)
- [ ] 107-02-PLAN.md — ConspitLink ProcMon audit + per-game compatibility matrix (AUDIT-02, AUDIT-04)

### Phase 108: Keyboard Hook Replacement
**Goal**: The SetWindowsHookEx global keyboard hook installed by Phase 78 is fully removed from rc-agent source and permanently replaced by GPO registry key writes -- kiosk lockdown is equally effective without any hook, and no hook install/uninstall cycle is ever visible to a running anti-cheat driver
**Depends on**: Phase 107 (audit must confirm Pod OS edition and hook risk classification before replacement begins)
**Requirements**: HARD-01, VALID-03
**Success Criteria** (what must be TRUE):
  1. grep -r SetWindowsHookEx crates/rc-agent/src/ returns no matches -- the hook is fully removed from Rust source, not gated or disabled conditionally
  2. On a Pod 8 canary build, pressing the Windows key, Alt+Tab, and Ctrl+Shift+Esc while the kiosk is active produces no response -- GPO registry keys (NoWinKeys=1, DisableTaskMgr=1) are enforced and kiosk lockdown is intact without any hook
  3. Task Manager cannot be opened by a customer at the pod after the hook replacement -- VALID-03 is satisfied by the registry key path, not by the removed hook
  4. cargo build --release --bin rc-agent succeeds and cargo test -p rc-agent passes with the hook removal in place -- no compilation errors from the removal
**Plans**: 2 plans

Plans:
- [ ] 108-01-PLAN.md — Replace keyboard hook with GPO registry keys + Pod 8 canary verification (HARD-01, VALID-03)

### Phase 109: Safe Mode State Machine
**Goal**: rc-agent automatically enters a defined safe mode within 1 second of a protected game launching, disables all risky subsystems (process guard, Ollama queries, registry writes) for the duration of the game plus a 30-second cooldown, and defaults to safe mode at startup if a protected game is already running -- billing, lock screen, and WebSocket exec are unaffected throughout
**Depends on**: Phase 108 (hook must be replaced before safe mode design locks in -- safe mode no longer needs to manage hook state)
**Requirements**: SAFE-01, SAFE-02, SAFE-03, SAFE-04, SAFE-05, SAFE-06, SAFE-07
**Success Criteria** (what must be TRUE):
  1. When F1 25, iRacing, or LMU launches on Pod 8, rc-agent logs safe_mode entering within 1 second of the process creation event -- WMI Win32_ProcessStartTrace subscription fires before anti-cheat driver initialization completes
  2. While safe mode is active, rc-agent does not attempt to kill any process, does not send queries to Ollama, and does not write to any registry key -- verified by log inspection during a 10-minute protected game session
  3. After the protected game exits, safe mode remains active for exactly 30 seconds before deactivating -- verified by log timestamps on Pod 8 during a test session
  4. When rc-agent starts and F1 25 is already running (simulated by pre-launching game before agent start), rc-agent initializes directly into safe mode without a Normal state transition -- startup default is safe
  5. Billing ticks, lock screen state transitions, WebSocket keepalive, and rc-agent heartbeat all continue normally throughout a protected game session in safe mode -- no billing gaps or disconnects
**Plans**: 2 plans

Plans:
- [ ] 109-01-PLAN.md — SafeMode struct, WMI watcher, startup detection, AppState integration (SAFE-01, SAFE-02, SAFE-03)
- [ ] 109-02-PLAN.md — Event loop wiring, subsystem gates, SAFE-07 verification (SAFE-01 through SAFE-07)

### Phase 110: Telemetry Gating
**Goal**: Shared memory telemetry readers for iRacing and LMU defer their MapViewOfFile connection until 5 seconds after the game process is stable, UDP telemetry sockets exist only while their corresponding game is active, and AC EVO telemetry is feature-flagged off by default until its anti-cheat status is confirmed at full release
**Depends on**: Phase 109 (safe mode state machine must exist before shm_connect_allowed() has a state to check)
**Requirements**: HARD-03, HARD-04, HARD-05
**Success Criteria** (what must be TRUE):
  1. The iRacing shared memory adapter does not call OpenFileMapping or MapViewOfFile within the first 5 seconds of iRacing launching -- log shows shm_connect deferred before the first connect attempt; the connection uses named shared memory, never ReadProcessMemory or a game PID handle
  2. The LMU (rFactor 2) shared memory adapter applies the same 5-second deferred connect -- log shows shm_connect deferred for LMU on launch
  3. UDP telemetry sockets for F1 25 (port 20777) and iRacing (port 6789) are bound only while their respective game is in Running state and are closed within 5 seconds of game exit -- netstat on Pod 8 confirms no orphaned UDP bindings after game closes
  4. When racecontrol.toml has ac_evo_telemetry_enabled = false (the default), no shared memory mapping attempt is made for AC EVO even when the game is running -- feature flag is enforced at the adapter initialization path
**Plans**: 2 plans

Plans:
- [ ] 110-01-PLAN.md -- Shared memory deferred connect (5s) + AC EVO feature flag (HARD-03, HARD-05)
- [ ] 110-02-PLAN.md -- UDP socket lifecycle gating to Running state (HARD-04)

### Phase 111: Code Signing + Per-Game Canary Validation
**NOTE (v22.0):** Binary identity (SHA256 hash) and canary Pod 8 pattern are formalized in v22.0 OTA-02 and OTA-10. Code signing should integrate with the OTA release manifest (OTA-01) — signtool runs during the build step, signed hash goes into release-manifest.toml. Per-game canary validation should use the v22.0 canary wave infrastructure rather than a separate process.
**Goal**: rc-agent.exe and rc-sentry.exe are code signed with an OV certificate and signtool is integrated into the deploy pipeline; each protected game (F1 25, iRacing, LMU) completes a full staff test session on Pod 8 with safe mode active, signed binaries running, and no anti-cheat warnings logged -- billing continuity is verified throughout
**Depends on**: Phase 107 (certificate procurement -- OV cert must be in hand), Phase 109 (safe mode), Phase 110 (telemetry gating)
**Requirements**: HARD-02, VALID-01, VALID-02
**Success Criteria** (what must be TRUE):
  1. signtool verify /pa rc-agent.exe returns Successfully verified -- the binary carries a valid OV code signing certificate recognized by Windows
  2. signtool verify /pa rc-sentry.exe returns Successfully verified -- both pod binaries are signed
  3. A staff member completes a full test session for each of F1 25, iRacing, and LMU on Pod 8 (launch game, play 5 minutes, exit game) with safe mode active and signed binaries; the session produces no anti-cheat warning dialogs, no game disconnections attributed to third-party software, and rc-agent logs show safe mode entry and exit with correct timing
  4. Billing lifecycle (session start, per-minute ticks, session end) produces correct credit amounts during a safe mode test session -- no billing gaps caused by safe mode subsystem suspension
**Plans**: 2 plans

Plans:
- [ ] 111-01-PLAN.md — Build and deploy latest rc-agent to Pod 8 (canary)
- [ ] 111-02-PLAN.md — Per-game canary validation (F1 25, iRacing, LMU) + billing continuity + HARD-02 deferred

## v15.0 Progress

**Execution Order:** 107 -> 108 -> 109 -> 110 -> 111

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 107. Behavior Audit + Certificate Procurement | 2/2 | Complete    | 2026-03-21 |
| 108. Keyboard Hook Replacement | 1/1 | Complete    | 2026-03-21 |
| 109. Safe Mode State Machine | 2/2 | Complete    | 2026-03-21 |
| 110. Telemetry Gating | 2/2 | Complete    | 2026-03-21 |
| 111. Code Signing + Per-Game Canary Validation | 1/2 | In Progress|  |


## v16.0 Security Camera AI & Attendance

Transform Racing Point's existing 13-camera Dahua setup into an automated face-recognition attendance system. Starting with reliable RTSP frame access through a relay, then building the local AI pipeline (SCRFD detection + ArcFace recognition on RTX 4070 via ort), enrollment and attendance logging, alerts, and finally dashboard camera monitoring with NVR playback proxying. All inference runs locally -- zero cloud API cost, zero internet dependency, sub-10ms latency.

- [x] **Phase 112: RTSP Infrastructure & Camera Pipeline** - Relay service, multi-camera management, health monitoring, and people tracker integration (completed 2026-03-21)
- [x] **Phase 113: Face Detection & Privacy Foundation** - SCRFD face detection on GPU via ort, plus DPDP Act consent framework (completed 2026-03-21)
- [x] **Phase 114: Face Recognition & Quality Gates** - ArcFace embedding extraction, cosine similarity matching, quality filtering, and lighting normalization (completed 2026-03-21)
- [x] **Phase 115: Face Enrollment System** - Profile management, multi-angle capture, and face database population (completed 2026-03-21)
- [x] **Phase 116: Attendance Engine** - Auto-log entry timestamps on recognition, staff clock-in/clock-out with shift tracking (completed 2026-03-21)
- [x] **Phase 117: Alerts & Notifications** - Dashboard notifications, desktop popups, and unknown person alerts (completed 2026-03-21)
- [x] **Phase 118: Live Camera Feeds** - MJPEG proxy for real-time camera viewing in dashboard (completed 2026-03-21)
- [x] **Phase 119: NVR Playback Proxy** - Query Dahua NVR API for stored footage, serve through dashboard with event markers (completed 2026-03-21)

## v16.0 Security Camera AI & Attendance -- Phase Details

### Phase 112: RTSP Infrastructure & Camera Pipeline
**Goal**: Reliable frame access from all attendance cameras with zero disruption to existing systems
**Depends on**: Nothing (first v16.0 phase)
**Requirements**: CAM-01, CAM-02, CAM-03, CAM-04
**Success Criteria** (what must be TRUE):
  1. RTSP relay (go2rtc or mediamtx) runs on James and proxies streams from entrance (.8) and reception (.15, .154) cameras without dropping connections over 24+ hours
  2. rc-sentry-ai crate exists with retina-based frame extraction pulling frames at 2-5 FPS from each camera via the relay
  3. Stream health endpoint at :8096 reports per-camera status and auto-reconnects within 30 seconds of a camera dropout
  4. Existing people tracker at :8095 continues working unaffected -- it reads from the relay instead of directly from cameras
**Plans**: 4 plans

Plans:
- [ ] 112-01: RTSP relay setup and camera wiring
- [ ] 112-02: rc-sentry-ai crate scaffold and retina frame extraction
- [ ] 112-03: Stream health monitoring and auto-reconnect
- [ ] 112-04: People tracker migration to relay

### Phase 113: Face Detection & Privacy Foundation
**Goal**: Detect faces in camera frames on the GPU, with legal compliance for biometric data collection
**Depends on**: Phase 112
**Requirements**: FACE-01, PRIV-01
**Success Criteria** (what must be TRUE):
  1. SCRFD model runs via ort with CUDA on the RTX 4070 and detects faces in live camera frames with bounding boxes and 5-point landmarks
  2. Detection completes in under 10ms per frame and handles multiple simultaneous faces
  3. DPDP Act consent mechanism is implemented -- consent signage requirements documented, data retention policy enforced, and audit logging records all biometric data access
**Plans**: 3 plans

Plans:
- [ ] 113-01: ONNX Runtime setup with CUDA and SCRFD model loading
- [ ] 113-02: Face detection pipeline integration with camera frames
- [ ] 113-03: DPDP consent framework and audit logging

### Phase 114: Face Recognition & Quality Gates
**Goal**: Identify detected faces by matching embeddings against enrolled faces, rejecting poor-quality captures
**Depends on**: Phase 113
**Requirements**: FACE-02, FACE-03, FACE-04
**Success Criteria** (what must be TRUE):
  1. ArcFace model generates 512-D embeddings from aligned face crops, and cosine similarity matching correctly identifies enrolled persons with confidence above threshold (~0.4-0.5)
  2. Quality gates reject blurry frames (Laplacian variance below threshold), extreme side-profile poses (yaw > 30 degrees), and faces smaller than 200x200px before sending to recognition
  3. Lighting normalization handles entrance camera backlight conditions -- recognition accuracy remains consistent across morning, midday, and evening lighting
  4. Face tracker deduplicates across frames so the same person walking through is recognized once per cooldown period, not on every frame
**Plans**: 3 plans

Plans:
- [ ] 114-01-PLAN.md -- Quality gates (blur, pose, size) and CLAHE lighting normalization
- [ ] 114-02-PLAN.md -- ArcFace recognizer and face alignment
- [ ] 114-03-PLAN.md -- Embedding gallery, face tracker, config, and pipeline integration

### Phase 115: Face Enrollment System
**Goal**: Staff can add, update, and remove face profiles to build the recognition database
**Depends on**: Phase 114
**Requirements**: ENRL-01, ENRL-02
**Success Criteria** (what must be TRUE):
  1. Staff can create a person profile (name, role, phone) and associate face photos via an API endpoint
  2. Multi-angle enrollment captures 3-5 quality frames from different angles, rejecting images that fail quality gates, and stores embeddings in SQLite
  3. Staff can update or delete a person's face profile, and the in-memory embedding gallery reflects changes immediately
  4. Duplicate detection prevents enrolling the same person twice by checking new embeddings against existing ones
**Plans**: 2 plans

Plans:
- [ ] 115-01-PLAN.md -- Data layer: extend DB CRUD (phone, get/list/update/delete) + gallery mutations + enrollment types
- [ ] 115-02-PLAN.md -- Enrollment API: HTTP handlers, photo processing pipeline, duplicate detection, main.rs wiring

### Phase 116: Attendance Engine
**Goal**: Automatically log attendance when recognized faces appear on camera, with staff shift tracking
**Depends on**: Phase 115
**Requirements**: ATTN-01, ATTN-02
**Success Criteria** (what must be TRUE):
  1. When an enrolled person is recognized at an entrance/reception camera, an attendance entry is logged with person ID, camera, timestamp, and confidence -- without any manual action
  2. Cross-camera deduplication prevents duplicate attendance entries when the same person is seen by entrance (.8) and then reception (.15/.154) cameras within a configurable window (default 5-10 minutes)
  3. Staff members have automatic clock-in on first recognition of the day and clock-out after configurable minimum hours, with shift history queryable via API
  4. Attendance API serves "who is present now" and "attendance history" endpoints that racecontrol can consume for dashboard display
**Plans**: 3 plans

Plans:
- [ ] 116-01: Attendance logging and cross-camera deduplication
- [ ] 116-02: Staff clock-in/clock-out state machine
- [ ] 116-03: Attendance API endpoints and racecontrol integration

### Phase 117: Alerts & Notifications
**Goal**: Staff and Uday are notified in real time about attendance events and unknown persons
**Depends on**: Phase 116
**Requirements**: ALRT-01, ALRT-02, ALRT-03
**Success Criteria** (what must be TRUE):
  1. Attendance events (customer arrival, staff clock-in/out) appear as real-time notifications in the racecontrol dashboard via WebSocket broadcast
  2. James machine displays a desktop popup with sound when a person is detected at the entrance camera
  3. Unknown (unrecognized) faces trigger a distinct alert that appears in both the dashboard and as a desktop notification, with the face crop visible for staff review
**Plans**: 3 plans

Plans:
- [ ] 117-01: Dashboard WebSocket notifications (DashboardEvent variants)
- [ ] 117-02: Desktop popup and sound notifications on James
- [ ] 117-03: Unknown person alert pipeline with face crop display

### Phase 118: Live Camera Feeds
**Goal**: Staff can view live camera feeds directly in the racecontrol dashboard
**Depends on**: Phase 112
**Requirements**: MNTR-01
**Success Criteria** (what must be TRUE):
  1. Dashboard displays live MJPEG streams from entrance and reception cameras with under 2-second latency
  2. MJPEG proxy endpoint at rc-sentry-ai serves frames that render natively in browser img tags -- no video player library required
  3. Live feed does not degrade face detection performance -- frame serving is independent of the AI pipeline
**Plans**: 2 plans

Plans:
- [ ] 118-01: MJPEG proxy endpoint serving camera frames
- [ ] 118-02: Dashboard live feed UI component

### Phase 119: NVR Playback Proxy
**Goal**: Staff can review past footage from the Dahua NVR through the dashboard without accessing the NVR directly
**Depends on**: Phase 118
**Requirements**: MNTR-02
**Success Criteria** (what must be TRUE):
  1. Dashboard provides a time-range selector that queries the Dahua NVR at .18 for stored footage and streams it through rc-sentry-ai
  2. Attendance event markers overlay on the playback timeline so staff can jump to moments when specific persons were detected
  3. Playback works for all 3 attendance cameras and does not interfere with the NVR's ongoing recording
**Plans**: 3 plans

Plans:
- [ ] 119-01-PLAN.md -- NVR CGI API client and config (nvr.rs, NvrConfig, digest auth)
- [ ] 119-02-PLAN.md -- Playback proxy endpoints (search, stream, events)
- [ ] 119-03-PLAN.md -- Dashboard playback page with timeline and event markers
## v16.0 Progress

**Execution Order:** 112 -> 113 -> 114 -> 115 -> 116 -> 117 -> 118 -> 119
(Phase 118 depends only on Phase 112, so it could run in parallel with 113-117 if needed)

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 112. RTSP Infrastructure & Camera Pipeline | 4/4 | Complete    | 2026-03-21 |
| 113. Face Detection & Privacy Foundation | 3/3 | Complete    | 2026-03-21 |
| 114. Face Recognition & Quality Gates | 3/3 | Complete    | 2026-03-21 |
| 115. Face Enrollment System | 2/2 | Complete    | 2026-03-21 |
| 116. Attendance Engine | 3/3 | Complete    | 2026-03-21 |
| 117. Alerts & Notifications | 3/3 | Complete    | 2026-03-21 |
| 118. Live Camera Feeds | 2/2 | Complete    | 2026-03-21 |
| 119. NVR Playback Proxy | 3/3 | Complete    | 2026-03-21 |

## v17.0 Cloud Platform -- Phase Details

> Planning artifacts live in `pwa/.planning/` (PWA phases 1-10 → unified phases 120-129)

### Phase 120: Cloud Infrastructure
**Goal**: All racingpoint.cloud subdomains resolve, terminate TLS, and route to running containers on the VPS
**Depends on**: Nothing (first phase)
**Requirements**: INFRA-01, INFRA-02, INFRA-03, INFRA-06, INFRA-07
**Success Criteria** (what must be TRUE):
  1. Visiting app.racingpoint.cloud, admin.racingpoint.cloud, dashboard.racingpoint.cloud, and api.racingpoint.cloud in a browser shows HTTPS with valid Let's Encrypt certificates
  2. All four containers (Caddy + 3 frontends) are running via Docker Compose with memory limits and healthchecks
  3. VPS firewall blocks all inbound ports except 80 and 443
  4. VPS has 2GB swap enabled and containers survive under memory pressure
**Plans**: 2 plans (Wave 1: config files + verify script, Wave 2: Bono VPS deployment)

Plans:
- [ ] 01-01: Caddyfile, compose.yml, Dockerfile port fix, verification script (INFRA-02, INFRA-03)
- [ ] 01-02: Bono VPS deployment — DNS, firewall, swap, compose up, cert switch (INFRA-01, INFRA-06, INFRA-07)

### Phase 121: API + PWA Cloud Deploy
**Goal**: Customers can access the PWA from any device and use existing features (login, wallet, sessions, leaderboards) via the cloud API
**Depends on**: Phase 120
**Requirements**: PWA-01, PWA-02, PWA-03, PWA-04, PWA-05, API-01, API-02
**Success Criteria** (what must be TRUE):
  1. Customer can open app.racingpoint.cloud on their phone and log in with phone + WhatsApp OTP
  2. Customer can view their profile, wallet balance, session history, and leaderboards from the cloud PWA
  3. Customer can top up wallet via Razorpay from the cloud PWA and see updated balance after sync
  4. PWA is installable to home screen (manifest, service worker, icons all working)
  5. All existing customer API endpoints at api.racingpoint.cloud return correct synced data
**Plans**: 2 plans

### Phase 122: Sync Hardening
**Goal**: Cloud-local sync is financially correct, loop-free, and exposes health status for all tables needed by admin and dashboard
**Depends on**: Phase 121
**Requirements**: SYNC-01, SYNC-02, SYNC-03, SYNC-04, SYNC-06, SYNC-07
**Plans**: 3 plans (complete)

### Phase 123: Remote Booking + PIN Generation
**Goal**: Customer can book an experience from their phone at home and receive a PIN for venue redemption
**Depends on**: Phase 122
**Requirements**: BOOK-01, BOOK-02, BOOK-03, BOOK-04, BOOK-05, BOOK-06, BOOK-07, API-04
**Plans**: 3 plans (complete)

### Phase 124: Kiosk PIN Launch
**Goal**: Customer enters PIN at venue kiosk and the game auto-launches on an assigned pod with zero staff interaction
**Depends on**: Phase 123
**Requirements**: KIOSK-01, KIOSK-02, KIOSK-03, KIOSK-04, KIOSK-05, KIOSK-06
**Plans**: 2 plans (complete)

### Phase 125: Admin Panel Cloud Deploy
**Goal**: Uday can manage all business operations remotely from admin.racingpoint.cloud
**Depends on**: Phase 122
**Requirements**: ADMIN-01, ADMIN-02, ADMIN-03, ADMIN-04, ADMIN-05, API-03
**Success Criteria** (what must be TRUE):
  1. Admin panel at admin.racingpoint.cloud requires authentication before any page loads
  2. Uday can view revenue reports, booking history, and customer data from his phone
  3. Uday can configure pricing tiers, experiences, and kiosk settings remotely and changes sync to local server
  4. All existing admin API endpoints work correctly on the cloud instance with synced data
**Plans**: 2 plans

### Phase 126: Dashboard Cloud Deploy
**Goal**: Uday can monitor live venue operations from dashboard.racingpoint.cloud
**Depends on**: Phase 122
**Requirements**: DASH-01, DASH-02, DASH-03, DASH-04, DASH-05
**Success Criteria** (what must be TRUE):
  1. Dashboard at dashboard.racingpoint.cloud requires authentication (admin-only)
  2. Dashboard shows real-time pod status grid for all 8 pods, updated via polling
  3. Dashboard shows today's revenue, active sessions, and billing timers
  4. Dashboard shows connection status indicator reflecting cloud-to-local sync health
**Plans**: 2 plans

### Phase 127: CI/CD Pipeline
**NOTE (v22.0):** Cloud deploy should use the same OTA pipeline architecture (OTA-08 state machine, OTA-03 health gates) as local fleet deploy. The deploy state machine, health gates, and standing rules gates from v22.0 Phases 179+181 should be the foundation — extended for cloud targets (PM2 on Bono's VPS via comms-link relay) rather than building a separate CI/CD system.
**Goal**: Pushing to main automatically builds and deploys all services to the VPS
**Depends on**: Phase 120
**Requirements**: INFRA-04
**Success Criteria** (what must be TRUE):
  1. Pushing a commit to main in GitHub triggers a GitHub Actions workflow that builds Docker images and deploys them to the VPS via SSH
  2. Failed builds do not deploy — only successful builds reach production
**Plans**: 2 plans

### Phase 128: Health Monitoring + Alerts
**Goal**: Container failures and resource exhaustion are detected and reported automatically via WhatsApp
**Depends on**: Phase 120
**Requirements**: INFRA-05
**Success Criteria** (what must be TRUE):
  1. When a container crashes, restarts, or goes OOM, a WhatsApp alert is sent to Uday within 2 minutes
  2. Container healthchecks detect unresponsive services and trigger automatic restart
**Plans**: 2 plans

### Phase 129: Operational Hardening
**Goal**: Production edge cases (extended outages, brute force, sync conflicts) are handled gracefully
**Depends on**: Phase 124, Phase 125
**Requirements**: SYNC-05, API-05
**Success Criteria** (what must be TRUE):
  1. During an extended internet outage, cloud bookings queue as pending_sync and local server confirms them post-reconnection without data loss
  2. Authentication endpoints (login, OTP verify, PIN entry) are rate-limited to prevent brute force attacks
  3. After connectivity is restored, pending bookings resolve within two sync cycles
**Plans**: 2 plans

## v17.0 Progress

**Execution Order:** 120 -> 121 -> 122 -> 123 -> 124 (critical path)
Parallel after 122: 125+126 | Parallel after 120: 127+128 | 129 after 124+125

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 120. Cloud Infrastructure | 0/2 | Planned | - |
| 121. API + PWA Cloud Deploy | 0/? | Not started | - |
| 122. Sync Hardening | 3/3 | Complete | 2026-03-21 |
| 123. Remote Booking + PIN Generation | 3/3 | Complete | 2026-03-21 |
| 124. Kiosk PIN Launch | 2/2 | Complete | 2026-03-21 |
| 125. Admin Panel Cloud Deploy | 0/? | Not started | - |
| 126. Dashboard Cloud Deploy | 0/? | Not started | - |
| 127. CI/CD Pipeline | 0/? | Not started | - |
| 128. Health Monitoring + Alerts | 0/? | Not started | - |
| 129. Operational Hardening | 0/? | Not started | - |

## v18.0 Seamless Execution -- Phase Details

> All implementation lives in `C:/Users/bono/racingpoint/comms-link` (separate repo from racecontrol).
> Build order: Foundation → Shell Relay → Chain Orchestration → Delegation + Audit → Advanced Chain Features

## Phases (v18.0)

- [x] **Phase 130: Protocol Foundation + Dynamic Registry** - Protocol types, DynamicCommandRegistry, per-command env isolation (completed 2026-03-21)
- [x] **Phase 131: Shell Relay** - Separate APPROVE-only handler for arbitrary binary execution with hardened approval gate (completed 2026-03-21)
- [x] **Phase 132: Chain Orchestration** - ExecResultBroker + ChainOrchestrator for sequential multi-step execution (completed 2026-03-21)
- [x] **Phase 133: Task Delegation + Audit Trail** - Claude-to-Claude Promise delegation and append-only exec audit log (completed 2026-03-21)
- [x] **Phase 134: Advanced Chain Features + Integration Hardening** - Templates, output templating, per-step retry, pause/resume, registry introspection (completed 2026-03-21)

### Phase 130: Protocol Foundation + Dynamic Registry
**Goal**: All new message types are defined and either side can register runtime commands without touching the static registry
**Depends on**: Nothing (first v18.0 phase)
**Requirements**: DREG-01, DREG-02, DREG-03, DREG-04, DREG-05
**Success Criteria** (what must be TRUE):
  1. James or Bono can POST a new command definition at runtime and immediately invoke it over the WS connection without redeploying comms-link
  2. An attempt to register a binary not in the hardcoded ALLOWED_BINARIES list is rejected with an error — the command is never stored
  3. All 20 existing static commands (git_log, pm2_status, etc.) work identically after the dynamic layer is added — no behavior change
  4. A dynamic command with allowedEnvKeys receives only those keys at execution; no other env vars leak from safeEnv into the command environment
  5. Lookup always tries the dynamic Map first, falling through to the static COMMAND_REGISTRY on a miss
**Plans**: 2 plans
Plans:
- [ ] 130-01-PLAN.md — Protocol types + DynamicCommandRegistry class
- [ ] 130-02-PLAN.md — Unified lookup integration + HTTP registration + persistence

### Phase 131: Shell Relay
**Goal**: Either AI can execute an arbitrary approved binary on the other's machine, but only after Uday approves via WhatsApp
**Depends on**: Phase 130
**Requirements**: SHRL-01, SHRL-02, SHRL-03, SHRL-04, SHRL-05
**Success Criteria** (what must be TRUE):
  1. James sends a shell_request to Bono; Uday receives a WhatsApp message showing the exact binary and full args before anything runs
  2. Approving from WhatsApp causes the command to execute on Bono and return stdout/stderr/exitCode — no other tier or shortcut path triggers execution
  3. An attempt to escalate shell relay to AUTO or NOTIFY tier is rejected at the handler — the tier is hardcoded APPROVE and the payload value is ignored
  4. A binary not in the shell relay allowlist (node, git, pm2, cargo, systemctl, curl, sqlite3, taskkill, shutdown, net, wmic) is rejected before any approval request is sent to Uday
  5. Shell relay uses execFile with shell:false and the same sanitized env as static commands — no shell injection surface exists
**Plans**: 2 plans
Plans:
- [ ] 131-01-PLAN.md — ShellRelayHandler class + TDD tests + James/Bono wiring

### Phase 132: Chain Orchestration
**Goal**: Either side can execute a multi-step chain where each step receives the previous step's output and the whole chain returns one structured result
**Depends on**: Phase 130
**Requirements**: CHAIN-01, CHAIN-02, CHAIN-03, CHAIN-04, CHAIN-05
**Success Criteria** (what must be TRUE):
  1. A chain_request with 3 steps executes them in order — step 2 receives step 1's stdout as input and step 3 receives step 2's stdout
  2. If step 2 exits with code != 0, step 3 does not execute — the chain returns a chain_result marked FAILED with steps 1 and 2 populated and step 3 absent
  3. A step with continue_on_error: true allows the chain to proceed to the next step even when that step exits non-zero
  4. A single chain_result message is returned after all steps complete, containing every step's stdout, stderr, exitCode, and durationMs in a structured array
  5. A chain that exceeds its chain-level timeout is aborted mid-execution and returns a chain_result with a TIMEOUT status
**Plans**: 2 plans
Plans:
- [ ] 132-01-PLAN.md — TDD ExecResultBroker + ChainOrchestrator classes
- [ ] 132-02-PLAN.md — Wire into james/index.js + bono/index.js + refactor FailoverOrchestrator

### Phase 133: Task Delegation + Audit Trail
**Goal**: Either AI can transparently delegate a chain to the other machine and receive results, with every execution logged to an append-only audit file on both sides
**Depends on**: Phase 132
**Requirements**: DELEG-01, DELEG-02, DELEG-03, AUDIT-01, AUDIT-02, AUDIT-03
**Success Criteria** (what must be TRUE):
  1. James asks a user question requiring a Bono-side command — James sends a delegate_request, Bono executes the chain, and James integrates the result into its reply without the user seeing any relay scaffolding
  2. Bono can symmetrically delegate to James — the delegation protocol works in both directions over the same WS connection
  3. After any remote execution (exec, shell relay, chain, or delegation), both the requesting machine and the executing machine have a new line in data/exec-audit.jsonl containing execId, command, requester, exitCode, durationMs, and tier
  4. Chain audit entries include chainId and stepIndex so a multi-step failure is traceable as one coherent event in the log, not isolated step entries
  5. The audit file is append-only and does not truncate on daemon restart — entries accumulate across sessions
**Plans**: 2 plans
Plans:
- [ ] 133-01-PLAN.md — AuditLogger class + delegation protocol types (TDD)
- [ ] 133-02-PLAN.md — Wire delegation + audit into james/index.js + bono/index.js

### Phase 134: Advanced Chain Features + Integration Hardening
**Goal**: Chains support templates, output substitution, per-step retry, survive WS reconnects, and either AI can query what commands the other exposes
**Depends on**: Phase 132, Phase 133
**Requirements**: CHAIN-06, CHAIN-07, CHAIN-08, CHAIN-09, DREG-06
**Success Criteria** (what must be TRUE):
  1. James sends a chain_request referencing a named template ("deploy-bono") by name — the chain executes using the step definitions from chains.json without repeating the step spec in every request
  2. A chain step with {{prev_stdout}} in its args receives the actual stdout from the previous step substituted in — the literal placeholder string never reaches the shell
  3. A chain step with retry: {count: 3, backoffMs: 500} re-executes up to 3 times on non-zero exit before the chain treats that step as failed
  4. When the WS connection drops mid-chain, the chain state persists and resumes from the interrupted step after reconnection — no chain restarts from the beginning due to a transient disconnect
  5. Either AI can query the other's command registry and receive a list of command names, descriptions, and approval tiers — binary paths and raw args are never returned in the introspection response
**Plans**: 2 plans
Plans:
- [x] 134-01-PLAN.md — Chain templates, output templating, per-step retry (ChainOrchestrator)
- [x] 134-02-PLAN.md — Chain state persistence (pause/resume) + registry introspection

## v18.0 Progress

**Execution Order:** 130 -> 131 -> 132 -> 133 -> 134
(Phase 131 depends only on 130; Phase 132 also depends only on 130 — both can run in parallel after 130)

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 130. Protocol Foundation + Dynamic Registry | 2/2 | Complete    | 2026-03-21 |
| 131. Shell Relay | 1/1 | Complete    | 2026-03-21 |
| 132. Chain Orchestration | 2/2 | Complete    | 2026-03-21 |
| 133. Task Delegation + Audit Trail | 2/2 | Complete    | 2026-03-21 |
| 134. Advanced Chain Features + Integration Hardening | 2/2 | Complete    | 2026-03-21 |

---

## v18.1 Seamless Execution Hardening -- Phase Details

> All implementation lives in `C:/Users/bono/racingpoint/comms-link` (separate repo from racecontrol).
> Two phases: Windows daemon recovery (James-only, Task Scheduler + HKLM Run key) and code-only chain + visibility fixes.

## Phases (v18.1)

- [x] **Phase 135: Daemon Recovery** - Task Scheduler watchdog + HKLM Run key boot start for James comms-link daemon (RECOV-01 through RECOV-04)
 (completed 2026-03-22)
- [x] **Phase 136: Chain Endpoint + Visibility** - Fix /relay/chain/run 504, route chain_result through ExecResultBroker, add health probe + degradation status (CHAIN-10, CHAIN-11, VIS-01, VIS-02, VIS-03)
 (completed 2026-03-22)

### Phase 135: Daemon Recovery
**Goal**: James comms-link daemon survives crashes and reboots — auto-restarts within 30s after a crash and starts automatically on Windows boot
**Depends on**: Nothing (first v18.1 phase)
**Requirements**: RECOV-01, RECOV-02, RECOV-03, RECOV-04
**Success Criteria** (what must be TRUE):
  1. After killing the comms-link Node process on James, the daemon is running again within 30 seconds without any manual action
  2. After rebooting James's Windows machine, comms-link is running and connected before any user interaction
  3. Bono receives a WhatsApp or email notification when the James daemon crashes and again when it recovers
  4. james_watchdog.ps1 detects a stopped comms-link process and restarts it — the watchdog itself is managed by Task Scheduler with a repeat interval
**Plans**: 2 plans
Plans:
- [ ] 135-01-PLAN.md — Create james_watchdog.ps1 and register-comms-watchdog.js
- [ ] 135-02-PLAN.md — Register Task Scheduler task, verify boot Run key, integration test

### Phase 136: Chain Endpoint + Visibility
**Goal**: /relay/chain/run returns chain results synchronously (no 504), and callers can always tell whether the relay is connected before sending
**Depends on**: Phase 135
**Requirements**: CHAIN-10, CHAIN-11, VIS-01, VIS-02, VIS-03
**Success Criteria** (what must be TRUE):
  1. POST /relay/chain/run returns a chain_result JSON body within the chain timeout — not a 504 gateway timeout
  2. chain_result WS messages arriving at james/index.js are routed through ExecResultBroker.handleResult() so the HTTP caller's promise resolves
  3. GET /relay/health returns a JSON body with connection mode (connected/disconnected) and last heartbeat timestamp
  4. POST /relay/exec/run returns HTTP 503 with a descriptive error message when the WS to Bono is disconnected — not a hang or silent failure
  5. Exec skills (comms-link skill wrappers) call /relay/health before sending and surface connection status to the caller when the relay is down
**Plans**: 2 plans
Plans:
- [x] 136-01-PLAN.md — Add chain_result WS handler to route through ExecResultBroker (CHAIN-10, CHAIN-11)
- [x] 136-02-PLAN.md — Enhance /relay/health response, guard /relay/exec/run, update skill (VIS-01, VIS-02, VIS-03)

## v18.1 Progress

**Execution Order:** 135 -> 136

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 135. Daemon Recovery | 2/2 | Complete    | 2026-03-22 |
| 136. Chain Endpoint + Visibility | 2/2 | Complete    | 2026-03-22 |


## v17.0 AI Debugger Autonomy & Self-Healing

Close 6 architectural gaps that prevented the system from self-healing when Edge died or stacked on pods. Add browser watchdog, continuous idle health checks, AI debugger action execution, healer-driven Edge recovery via WS protocol, and proactive WARN log scanning.

- [x] **Phase 137: Browser Watchdog** - rc-agent polls browser_process liveness every 30s, detects Edge stacking (>5 processes), and kills all before relaunch; close_browser() purges all msedge and WebView2 processes; watchdog suppressed during anti-cheat safe mode (completed 2026-03-22)
- [x] **Phase 138: Idle Health Monitor** - rc-agent runs check_window_rect + check_lock_screen_http every 60s when no billing session; self-heals via close_browser + launch_browser before alerting; sends IdleHealthFailed after 3 consecutive failures; skipped during active billing sessions
 (completed 2026-03-22)
- [x] **Phase 139: Healer Edge Recovery** - Pod healer adds HealAction::RelaunchLockScreen for failed lock screen HTTP checks; healer sends ForceRelaunchBrowser WS message to pod; rc-agent handles ForceRelaunchBrowser via close_browser + launch_browser (completed 2026-03-22)
- [x] **Phase 140: AI Action Execution Whitelist** - AI debugger Tier 3/4 responses parsed for structured safe actions; whitelist includes kill_edge, relaunch_lock_screen, restart_rcagent, kill_game, clear_temp; actions logged to activity_log; process-kill actions blocked during safe mode
- [x] **Phase 141: WARN Log Scanner** - Pod healer scans racecontrol log for WARN count each cycle; threshold (>50/5min) triggers AI escalation; recurring identical WARNs grouped and deduplicated before escalation (completed 2026-03-22)

## v17.0 Phase Details

### Phase 137: Browser Watchdog
**Goal**: rc-agent autonomously detects and recovers from Edge liveness failures and stack buildup without any human intervention or server involvement
**Depends on**: Phase 136 (can start in parallel -- no direct code dependency on v18.1)
**Requirements**: BWDOG-01, BWDOG-02, BWDOG-03, BWDOG-04
**Success Criteria** (what must be TRUE):
  1. When msedge.exe disappears between watchdog polls (simulated by taskkill from server), rc-agent relaunches Edge within 30 seconds and the lock screen is visible again
  2. When msedge.exe count exceeds 5 (simulated by repeated Edge launches), rc-agent kills all msedge.exe and msedgewebview2.exe processes before launching a clean Edge instance
  3. After close_browser() executes, no msedge.exe or msedgewebview2.exe processes remain -- the spawned child reference alone is not sufficient to clear stacked browsers
  4. When anti-cheat safe mode is active (protected game running), watchdog polling does not execute taskkill -- the game session continues uninterrupted
**Plans**: 2 plans

Plans:
- [ ] 137-01-PLAN.md -- close_browser() safe mode gate + count_edge_processes helper + tests (BWDOG-03, BWDOG-04)
- [ ] 137-02-PLAN.md -- Browser watchdog loop in event_loop: liveness poll + stacking detection (BWDOG-01, BWDOG-02)

### Phase 138: Idle Health Monitor
**Goal**: Pods continuously verify their own health during idle periods and self-heal display failures before they require human intervention or server escalation
**Depends on**: Phase 137 (shares close_browser + launch_browser; watchdog must be stable first)
**Requirements**: IDLE-01, IDLE-02, IDLE-03, IDLE-04
**Success Criteria** (what must be TRUE):
  1. When no billing session is active, the idle health check loop fires every 60 seconds and logs a health result -- confirmed by log inspection over a 3-minute idle window
  2. When the lock screen HTTP probe fails (port :18923 returns error), rc-agent calls close_browser + launch_browser and the lock screen is accessible again within 30 seconds -- no server action needed
  3. After 3 consecutive idle health failures without recovery, the server receives an IdleHealthFailed WebSocket message identifying the pod and failure type
  4. During an active billing session, the idle health check loop does not fire -- no interference with running games or telemetry collection
**Plans**: 3 plans

Plans:
- [x] 138-01-PLAN.md -- IdleHealthFailed protocol variant in rc-common (IDLE-03)
- [ ] 138-02-PLAN.md -- Agent idle health loop: 60s interval, billing/safe-mode skip, HTTP+rect probes, self-heal, hysteresis (IDLE-01, IDLE-02, IDLE-03, IDLE-04)
- [ ] 138-03-PLAN.md -- Server handler: IdleHealthFailed ws/mod.rs arm + FleetHealthStore fields + fleet API (IDLE-03)

### Phase 139: Healer Edge Recovery
**Goal**: The racecontrol pod healer can trigger a full Edge relaunch on any pod via a new WS protocol message -- no SSH, no exec endpoint, just the existing WebSocket connection
**Depends on**: Phase 137 (rc-agent close_browser + launch_browser must be reliable before healer calls them)
**Requirements**: HEAL-01, HEAL-02, HEAL-03
**Success Criteria** (what must be TRUE):
  1. When pod_healer detects a lock screen HTTP failure for a pod, the healer logs HealAction::RelaunchLockScreen and a ForceRelaunchBrowser message appears in the server outbound WS queue for that pod
  2. The ForceRelaunchBrowser WS message reaches the pod rc-agent and triggers close_browser + launch_browser -- the lock screen is accessible on HTTP probe within 30 seconds of the healer action
  3. RelaunchLockScreen does not conflict with an active billing session -- the healer checks billing state before dispatching (standing rule #10: recovery systems must not fight each other)
**Plans**: 2 plans

Plans:
- [x] 139-01-PLAN.md -- HealAction::RelaunchLockScreen in pod_healer.rs + ForceRelaunchBrowser in protocol.rs (HEAL-01, HEAL-02)
- [ ] 139-02-PLAN.md -- rc-agent ForceRelaunchBrowser WS handler + billing-state guard (HEAL-03)

### Phase 140: AI Action Execution Whitelist
**Goal**: The AI debugger can act on its own Tier 3/4 recommendations for pre-approved safe actions rather than just logging suggestions -- with all actions audited and blocked during anti-cheat safe mode
**Depends on**: Phase 138 (idle health infrastructure available; AI debugger already exists in ai_debugger.rs)
**Requirements**: AIACT-01, AIACT-02, AIACT-03, AIACT-04
**Success Criteria** (what must be TRUE):
  1. When the AI debugger returns a Tier 3 response containing kill_edge or relaunch_lock_screen in structured format, rc-agent executes the action without any manual approval
  2. An AI response containing an action not on the whitelist (e.g. arbitrary shell command) is logged as rejected and no action is taken -- only the 5 pre-approved actions are ever executed
  3. Every executed AI action produces an activity_log entry showing the action name, source model (ollama model ID), and whether it succeeded or failed
  4. When anti-cheat safe mode is active, any AI-suggested action that kills processes is blocked and logged as blocked: safe mode active -- the protected game continues uninterrupted
**Plans**: 2 plans

Plans:
- [x] 140-01-PLAN.md -- Safe action parser in ai_debugger.rs: structured response parsing + 5-entry whitelist enum (AIACT-01, AIACT-02)
- [x] 140-02-PLAN.md -- Action executor with activity_log writes + safe mode gate + server-side parsing (AIACT-03, AIACT-04)

### Phase 141: WARN Log Scanner
**Goal**: Racecontrol proactively detects degraded conditions by scanning its own logs for WARN accumulation and escalates to AI before a cascade becomes an incident
**Depends on**: Phase 139 (healer infrastructure; scanner runs in the same healer cycle)
**Requirements**: WARN-01, WARN-02, WARN-03
**Success Criteria** (what must be TRUE):
  1. Every healer cycle, the WARN count for the last 5 minutes is computed from the racecontrol log and visible in healer debug logs -- the scan is observable without triggering escalation
  2. When WARN count exceeds 50 in a 5-minute window, the AI debugger receives a query with a representative log snippet -- escalation fires exactly once per threshold breach, not on every subsequent cycle
  3. When the same WARN message fires 10+ times in 5 minutes, it appears once in the AI escalation payload with a count annotation instead of 10 raw lines -- the AI receives signal, not noise
**Plans**: 2 plans

Plans:
- [ ] 141-01-PLAN.md -- WARN log scanner in healer cycle: 5-min rolling window + threshold counter (WARN-01, WARN-02)
- [ ] 141-02-PLAN.md -- WARN deduplication + grouped escalation payload + AI query dispatch (WARN-03)

## v17.0 Progress

**Execution Order:** 137 -> 138 -> 139 -> 140 -> 141
Note: Phase 137 (Browser Watchdog) is the critical foundation -- close_browser reliability directly gates Phase 138 (Idle Health) and Phase 139 (Healer Recovery). Phases 140 (AI Actions) and 141 (WARN Scanner) are independent of each other but both depend on their respective foundation phases.

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 137. Browser Watchdog | 2/2 | Complete    | 2026-03-22 |
| 138. Idle Health Monitor | 3/3 | Complete    | 2026-03-22 |
| 139. Healer Edge Recovery | 2/2 | Complete    | 2026-03-22 |
| 140. AI Action Execution Whitelist | 1/2 | Complete    | 2026-03-22 |
| 141. WARN Log Scanner | 2/2 | Complete    | 2026-03-22 |

---

## v18.2 Debugging & Quality Gates

Fix the root cause of bugs slipping through GSD execution — 135 unit tests caught 0/8 integration bugs in v18.0. Reorganize bloated standing rules, build an integration test script that starts real daemons and verifies round-trip message flow, and wire it into GSD as a post-execution gate.

> All implementation lives in `C:/Users/bono/racingpoint/comms-link` and `CLAUDE.md` standing rules.

## Phases (v18.2)

- [x] **Phase 142: Rules Hygiene** - Reorganize CLAUDE.md standing rules into named categories, prune obsolete/duplicate rules, add justification comments, sync standing-rules.md (RULES-01 through RULES-04)
- [x] **Phase 143: Integration Test Suite** - Build comms-link integration test script: start real daemons, send WS exec/chain/delegation, verify round-trip results; cross-platform syntax check; contract tests for parameter agreements (INTEG-01 through INTEG-05) (completed 2026-03-22)
- [x] **Phase 144: GSD Quality Gate** - Wire integration tests into GSD execute-phase as automatic post-execution verification; single-command invocation; failures block phase completion (GATE-01 through GATE-03) (completed 2026-03-22)

## v18.2 Phase Details

### Phase 142: Rules Hygiene
**Goal**: CLAUDE.md standing rules are organized so they are actually followed — categorized, pruned of obsolete entries, annotated with justifications, and in sync with standing-rules.md memory file
**Depends on**: Nothing (first v18.2 phase)
**Requirements**: RULES-01, RULES-02, RULES-03, RULES-04
**Success Criteria** (what must be TRUE):
  1. CLAUDE.md Standing Process Rules section has named categories (Deploy, Comms, Code Quality, Process, Debugging) and every rule falls under exactly one category
  2. Every rule that was superseded by v18.0 or duplicated across sections is removed — rule count decreases from current baseline
  3. Every remaining rule has a one-line justification comment explaining why it exists, written so a future session can evaluate whether it is still relevant
  4. standing-rules.md and CLAUDE.md standing rules are in sync — no rule exists in one file but not the other
**Plans**: 2 plans

Plans:
- [x] 142-01-PLAN.md — Reorganize CLAUDE.md rules into 6 categories, prune 3 redundant rules, add justifications, sync standing-rules.md (RULES-01 through RULES-04)

### Phase 143: Integration Test Suite
**Goal**: A single script starts real comms-link daemons on James and verifies that WS exec, chain, delegation, syntax, and contract behaviors all work end-to-end against live processes — no mocks
**Depends on**: Phase 142 (rules must be clean before building tests; no code dependency)
**Requirements**: INTEG-01, INTEG-02, INTEG-03, INTEG-04, INTEG-05
**Success Criteria** (what must be TRUE):
  1. Running the integration test script starts a real comms-link daemon, sends an exec_request over WS, and asserts the exec_result contains correct command, stdout, exitCode, and durationMs fields
  2. A chain_request with 2 steps completes and the chain_result carries the matching chainId plus all step outputs in the correct order
  3. A message sent with from:james is relayed and the relay record preserves the from field exactly — no field is dropped or coerced
  4. node --check runs against all comms-link source files on both James (Windows) and verifies they would pass on Bono (Linux) — cross-platform syntax gate catches require/import mismatches before deploy
  5. Contract tests assert: chainId is passed through unmodified end-to-end, from field is preserved across all message types, MessageType enum values route to the correct handler
**Plans**: 2 plans

Plans:
- [ ] 143-01-PLAN.md — Integration test scaffold: daemon start/stop harness, exec_request round-trip, from-field relay test (INTEG-01, INTEG-03)
- [ ] 143-02-PLAN.md — Chain integration test + cross-platform syntax check + contract tests (INTEG-02, INTEG-04, INTEG-05)

### Phase 144: GSD Quality Gate
**Goal**: Integration tests run automatically as part of GSD phase verification — a single command invokes them and failures prevent a phase from being marked complete
**Depends on**: Phase 143 (integration test suite must exist before it can be wired as a gate)
**Requirements**: GATE-01, GATE-02, GATE-03
**Success Criteria** (what must be TRUE):
  1. Running `node test/integration.js` (or `bash test/e2e.sh`) from the comms-link repo root completes all integration tests and exits 0 on success, non-zero on failure — single-command invocation with no manual steps
  2. The GSD execute-phase verifier runs the integration test command automatically after any comms-link phase execution — the gate fires without James manually triggering it
  3. When any integration test fails, the phase cannot be marked complete in GSD — the failure is surfaced in the verifier output with the failing test name and the actual vs expected values
**Plans**: 2 plans

Plans:
- [x] 144-01-PLAN.md — Integration test entry point: single command, structured output, exit code contract (GATE-01)
- [x] 144-02-PLAN.md — Wire integration tests into GSD execute-phase verifier; failure blocks completion (GATE-02, GATE-03)

## v18.2 Progress

**Execution Order:** 142 -> 143 -> 144

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 142. Rules Hygiene | 1/1 | Complete    | 2026-03-22 |
| 143. Integration Test Suite | 2/2 | Complete    | 2026-03-22 |
| 144. GSD Quality Gate | 2/2 | Complete    | 2026-03-22 |

---

## v16.1 Camera Dashboard Pro

Transform the basic 13-camera snapshot grid into a professional NVR dashboard — hybrid streaming (cached snapshots for grid + WebRTC for fullscreen), configurable layouts (1/4/9/16), camera naming, drag-to-rearrange, and dual deployment (rc-sentry-ai :8096 + server web dashboard :3200).

> Implementation lives in `rc-sentry-ai` (Rust/Axum), `cameras.html` (embedded), and `web/src/app/cameras/page.tsx` (Next.js).

## Phases (v16.1)

- [x] **Phase 145: go2rtc Infrastructure** - Register all 13 NVR cameras in go2rtc, configure CORS, verify WebRTC signaling works from both deployment origins (INFRA-01, INFRA-02) (completed 2026-03-22)
- [x] **Phase 146: Backend Config and API** - Camera config in rc-sentry-ai.toml with display_name/display_order, /api/v1/cameras endpoint, PUT /api/v1/cameras/layout endpoint, camera-layout.json persistence (INFRA-03, INFRA-04, LYOT-04) (completed 2026-03-22)
- [x] **Phase 147: cameras.html Dashboard Rewrite** - Full rewrite of embedded dashboard with layout modes, drag-to-rearrange, WebRTC fullscreen, all UI/UX requirements, deployed at /cameras/live (LYOT-01, LYOT-02, LYOT-03, LYOT-05, UIUX-01, UIUX-02, UIUX-03, UIUX-04, UIUX-05, STRM-01, STRM-02, STRM-03, STRM-04, DPLY-01) (completed 2026-03-22)
- [x] **Phase 148: Web Dashboard Page** - Standalone camera dashboard page in Next.js web dashboard at /cameras, feature-identical to cameras.html (DPLY-02, DPLY-03) (completed 2026-03-22)

## v16.1 Phase Details

### Phase 145: go2rtc Infrastructure
**Goal**: go2rtc is configured, verified, and ready to serve WebRTC for all 13 cameras — no frontend WebRTC code is written until this phase confirms the infrastructure works
**Depends on**: Nothing (first v16.1 phase; go2rtc already running on James .27:1984)
**Requirements**: INFRA-01, INFRA-02
**Success Criteria** (what must be TRUE):
  1. All 13 NVR cameras are reachable via go2rtc stream names (ch1-ch13) and a test WebRTC session opens for at least one camera using the go2rtc web UI at :1984
  2. `curl -X OPTIONS http://192.168.31.27:1984/api/webrtc` returns `Access-Control-Allow-Origin: *` — CORS is verified working before any frontend code is written
  3. Snapshot polling via SnapshotCache and a simultaneous go2rtc WebRTC connection to the same channel both succeed without the NVR dropping either connection — NVR coexistence strategy is decided and tested on live hardware
  4. NVR credentials (Admin@123) do not appear in any go2rtc.yaml stream name or in any URL visible to browser JavaScript — all 13 streams are accessible by canonical name only
**Plans**: 1 plan
Plans:
- [ ] 145-01-PLAN.md — Register 13 NVR channels + CORS in go2rtc, restart, verify WebRTC + snapshot coexistence

### Phase 146: Backend Config and API
**Goal**: rc-sentry-ai serves a complete camera metadata API that both frontend targets can use, and user layout preferences persist across sessions via server-side storage
**Depends on**: Phase 145 (stream names from go2rtc.yaml must match what the API returns)
**Requirements**: INFRA-03, INFRA-04, LYOT-04
**Success Criteria** (what must be TRUE):
  1. `GET /api/v1/cameras` returns display_name, display_order, nvr_channel, and zone for all 13 cameras — the response shape is complete and no field is null for any camera
  2. Camera friendly names (e.g. "Pod Area", "Cashier", "Entrance") are read from rc-sentry-ai.toml and appear in the API response — stream IDs never appear as display labels
  3. `PUT /api/v1/cameras/layout` with a reorder payload writes atomically to camera-layout.json and `GET /api/v1/cameras/layout` returns the saved layout on the next page load — state survives rc-sentry-ai restart
  4. rc-sentry-ai.toml is never written to at runtime — all mutable user preferences go to camera-layout.json only
**Plans**: 2 plans
Plans:
- [ ] 146-01-PLAN.md — Extend CameraConfig with display_name/display_order/zone and update /api/v1/cameras response
- [ ] 146-02-PLAN.md — Add GET/PUT /api/v1/cameras/layout endpoints with camera-layout.json persistence

### Phase 147: cameras.html Dashboard Rewrite
**Goal**: Staff can monitor all 13 cameras from the rc-sentry-ai embedded dashboard with professional NVR controls — layout switching, drag-to-rearrange, and instant WebRTC fullscreen
**Depends on**: Phase 146 (API must return complete camera metadata before frontend is written)
**Requirements**: LYOT-01, LYOT-02, LYOT-03, LYOT-05, UIUX-01, UIUX-02, UIUX-03, UIUX-04, UIUX-05, STRM-01, STRM-02, STRM-03, STRM-04, DPLY-01
**Success Criteria** (what must be TRUE):
  1. Staff can open /cameras/live at :8096 and immediately see all 13 camera tiles filling the viewport with no scrollbars — the grid is edge-to-edge with a compact toolbar above
  2. Clicking the 1x1/2x2/3x3/4x4 toolbar buttons switches the grid layout without any tile being destroyed and rebuilt — the transition is smooth with no flash
  3. Staff can drag any camera tile to a new position in the grid; after releasing, the new order is saved and persists across page reload
  4. Clicking a camera tile opens a fullscreen view with live WebRTC video via go2rtc within a few seconds — hovering for 500ms before clicking visibly reduces the cold-start delay
  5. Closing the fullscreen view (click X or press Escape) tears down the WebRTC connection completely — verified by go2rtc /api/streams showing 0 viewers within 5 seconds
  6. Each camera tile shows a green/yellow/red status indicator and the tile's friendly display name from the API — offline cameras are visually distinct from live ones
**Plans**: 3 plans
Plans:
- [ ] 147-01-PLAN.md — Core HTML structure + CSS grid layout modes + status indicators + snapshot polling
- [ ] 147-02-PLAN.md — Drag-to-rearrange + zone grouping + layout persistence
- [ ] 147-03-PLAN.md — WebRTC fullscreen + singleton + pre-warm + loading state

### Phase 148: Web Dashboard Page
**Goal**: The same professional camera dashboard is accessible from the server web dashboard at :3200 with an identical feature set — staff can use either deployment interchangeably
**Depends on**: Phase 147 (cameras.html proves the full feature set; Next.js implementation follows the same patterns)
**Requirements**: DPLY-02, DPLY-03
**Success Criteria** (what must be TRUE):
  1. Staff can open /cameras in the web dashboard at :3200 and see the same 13-camera grid with layout controls, drag-to-rearrange, and WebRTC fullscreen — no features are missing compared to cameras.html
  2. A layout change made at :8096 (cameras.html) is reflected when /cameras at :3200 is opened — server-side camera-layout.json is the shared source of truth for both deployments
  3. Next.js hydration completes without mismatch errors — localStorage is only read inside useEffect with a hydrated flag, never in a useState initializer
**Plans**: 1 plan
Plans:
- [ ] 148-01-PLAN.md — Complete page.tsx rewrite with all 12 camera dashboard features (layouts, drag, zones, WebRTC, pre-warm)

## v16.1 Progress

**Execution Order:** 145 -> 146 -> 147 -> 148
Note: Phases 145 and 146 are strictly sequential infrastructure prerequisites. Phase 147 (cameras.html) must complete before Phase 148 (Next.js) to validate the full feature set in vanilla JS before the React implementation begins.

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 145. go2rtc Infrastructure | 1/1 | Complete   | 2026-03-22 |
| 146. Backend Config and API | 2/2 | Complete    | 2026-03-22 |
| 147. cameras.html Dashboard Rewrite | 3/3 | Complete    | 2026-03-22 |
| 148. Web Dashboard Page | 1/1 | Complete    | 2026-03-22 |

---

## v19.0 Cafe Inventory, Ordering & Marketing

Deliver a complete cafe operations layer for Racing Point eSports and Cafe -- menu management, self-service and staff-assisted ordering with shared wallet billing, real-time inventory tracking with three-channel alerts, promotional deals engine, and automated marketing content. Built as new Rust modules and Next.js pages within the existing racecontrol monolith, reusing wallet, WhatsApp, email, and auth infrastructure.

### Phase 149: Menu Data Model & CRUD
**Goal**: Admin can create and manage cafe items with all required fields, and items persist correctly in the database
**Depends on**: Nothing (first phase -- foundation for everything)
**Requirements**: MENU-02, MENU-03, MENU-04, MENU-05
**Success Criteria** (what must be TRUE):
  1. Admin can add a new cafe item with name, description, category, selling price, and cost price via the admin dashboard
  2. Admin can edit any field of an existing cafe item and see the change reflected immediately
  3. Admin can delete a cafe item and it no longer appears anywhere
  4. Admin can toggle an item between available and unavailable, and unavailable items are hidden from customer-facing views
  5. Categories are managed as a controlled list (not free-text entry)
**Plans**: 2 plans

Plans:
- [ ] 149-01-PLAN.md -- Backend: SQLite schema (cafe_categories + cafe_items) + Rust cafe.rs CRUD module + route registration
- [ ] 149-02-PLAN.md -- Frontend: TypeScript types + api methods + sidebar nav + /cafe admin page with side panel

### Phase 150: Menu Import
**Goal**: Admin can bulk-import cafe menu items from Excel/CSV spreadsheets with preview-and-confirm flow, plus upload item images
**Depends on**: Phase 149
**Requirements**: MENU-01, MENU-06
**Success Criteria** (what must be TRUE):
  1. Admin can upload a PDF or spreadsheet file and see a preview of parsed items before confirming import
  2. Import validates each item (price > 0, name non-empty, category in known list) and flags errors for correction
  3. No items are published until admin explicitly confirms the import preview
  4. Admin can upload an image for any cafe item, and the image is stored and associated with that item
**Plans**: 2 plans

Plans:
- [x] 150-01-PLAN.md -- Backend: Cargo deps, DB migration, XLSX/CSV parsing, import/confirm endpoints, image upload handler, static serving
- [x] 150-02-PLAN.md -- Frontend: TypeScript types + API methods, import modal with preview table, image column with upload

### Phase 151: Menu Display
**Goal**: Customers and staff can browse the complete cafe menu with correct pricing, categories, and images
**Depends on**: Phase 150
**Requirements**: MENU-07, MENU-08
**Success Criteria** (what must be TRUE):
  1. POS displays cafe items grouped by category with name and selling price
  2. PWA displays cafe items grouped by category with images, descriptions, and selling price
  3. Unavailable items do not appear in either POS or PWA views
  4. Menu loads within 2 seconds on the cafe WiFi network
**Plans**: 2 plans

Plans:
- [ ] 151-01-PLAN.md -- POS cafe menu display (kiosk CafeMenuPanel)
- [ ] 151-02-PLAN.md -- PWA cafe menu page (customer-facing /cafe)

### Phase 152: Inventory Tracking
**Goal**: Admin has full visibility into stock levels and can manage inventory for all countable items
**Depends on**: Phase 149
**Requirements**: INV-01, INV-02, INV-04, INV-05, INV-09
**Success Criteria** (what must be TRUE):
  1. Admin can set and view stock quantities for countable items (bottles, buns, packaged snacks)
  2. Items are correctly categorized as countable (stock-tracked) or uncountable (availability toggle only)
  3. Admin can record a restock event and see the stock quantity increase accordingly
  4. Admin can set a low-stock threshold per countable item
  5. Inventory dashboard shows all items with current stock, threshold status, and countable/uncountable designation
**Plans**: 2 plans

Plans:
- [x] 152-01-PLAN.md -- Backend: DB migration + inventory columns + restock API endpoint
- [ ] 152-02-PLAN.md -- Frontend: TypeScript types + inventory tab UI + restock flow

### Phase 153: Inventory Alerts
**Goal**: Staff never misses a low-stock situation -- alerts fire through three independent channels when thresholds are breached
**Depends on**: Phase 152
**Requirements**: INV-06, INV-07, INV-08
**Success Criteria** (what must be TRUE):
  1. When a countable item's stock drops to or below its threshold, a WhatsApp alert is sent to the admin (once per breach, with cooldown)
  2. A warning banner appears in the admin dashboard for any item below its low-stock threshold
  3. An email alert fires when a low-stock threshold is breached
  4. Repeated threshold checks for the same item do not spam alerts (cooldown/dedup works)
**Plans**: 2 plans

Plans:
- [ ] 153-01-PLAN.md -- Backend: cafe_alerts module (WA + email), last_stock_alert_at cooldown, low-stock API route
- [ ] 153-02-PLAN.md -- Frontend: LowStockItem type + api method + warning banner in cafe admin page

### Phase 154: Ordering Core
**Goal**: Customers can order cafe items and pay from their RP wallet -- the core value delivery
**Depends on**: Phase 151, Phase 152
**Requirements**: ORD-01, ORD-02, ORD-03, ORD-04, ORD-07, ORD-08, INV-03
**Success Criteria** (what must be TRUE):
  1. Customer can browse the cafe menu in PWA, add items to a cart, and submit an order
  2. Staff can enter a cafe order via POS on behalf of a customer
  3. Order total is deducted from the customer's existing RP wallet balance
  4. Each completed order has a unique receipt number and transaction ID
  5. Items with zero stock cannot be added to an order (out-of-stock blocking prevents it)
  6. Concurrent orders for the last unit of an item do not both succeed (atomic stock decrement + wallet deduction)
  7. Stock quantities auto-decrement when countable items are sold
**Plans**: 3 plans

Plans:
- [ ] 154-01-PLAN.md -- Backend: cafe_orders table, atomic place_order handler, receipt generation, stock info in public menu
- [ ] 154-02-PLAN.md -- PWA: cart state, checkout flow, wallet balance display, order submission
- [ ] 154-03-PLAN.md -- POS kiosk: order builder, customer selection, staff-assisted ordering

### Phase 155: Receipts & Order History
**Goal**: Every order produces a physical receipt and digital record that staff and customers can reference
**Depends on**: Phase 154
**Requirements**: ORD-05, ORD-06, ORD-09
**Success Criteria** (what must be TRUE):
  1. Completing an order triggers a thermal receipt print for cafe staff to prepare the order
  2. Customer receives their order receipt via WhatsApp after order confirmation
  3. Customer can view their full cafe order history in the PWA
**Plans**: 2 plans

Plans:
- [x] 155-01-PLAN.md -- Backend: WhatsApp receipt dispatch, thermal print dispatch, GET /customer/cafe/orders/history endpoint
- [x] 155-02-PLAN.md -- PWA: /cafe/orders order history page with expand/collapse rows, human verify checkpoint

### Phase 156: Promotions Engine
**Goal**: Admin can create and configure promotional deals that drive cafe revenue
**Depends on**: Phase 149
**Requirements**: PROMO-01, PROMO-02, PROMO-03, PROMO-04
**Success Criteria** (what must be TRUE):
  1. Admin can create a combo deal that bundles specific items at a discounted price
  2. Admin can create a happy hour discount with start/end times in IST
  3. Admin can create a gaming+cafe combo bundle (game session + cafe item at bundle price)
  4. Admin can configure stacking rules -- which promos can combine and which are exclusive
  5. Promos activate and deactivate automatically based on their configured time windows
**Plans**: 2 plans

Plans:
- [ ] 156-01-PLAN.md -- Backend: cafe_promos DB migration, CRUD handlers (cafe_promos.rs), admin routes
- [ ] 156-02-PLAN.md -- Admin UI: Promos tab on /cafe page with dynamic form per type, CRUD, stacking group

### Phase 157: Promotions Integration
**Goal**: Active promos are visible to customers and staff, and discounts apply automatically at checkout
**Depends on**: Phase 154, Phase 156
**Requirements**: PROMO-05, PROMO-06
**Success Criteria** (what must be TRUE):
  1. Active promos display in both POS and PWA during their applicable time windows
  2. When a customer's cart meets promo conditions, the discount is applied automatically at checkout
  3. Applied promo is recorded with the order (traceable which discount was used)
  4. Promo pricing is calculated server-side only (not client-side)
**Plans**: 2 plans

Plans:
- [x] 157-01-PLAN.md — Backend: list_active_promos endpoint, evaluate_promos engine, DB migration, place_cafe_order_inner wiring
- [ ] 157-02-PLAN.md — Frontend: promo banner display in PWA and POS, applied discount in checkout confirmation

### Phase 158: Marketing & Content
**Goal**: Cafe promos and menu updates reach customers through auto-generated visual content and broadcast messages
**Depends on**: Phase 156
**Requirements**: MKT-01, MKT-02
**Success Criteria** (what must be TRUE):
  1. Admin can generate promo graphics (menu images, daily specials) suitable for Instagram stories/posts with one click
  2. Generated graphics reflect current menu items, prices, and active promos
  3. Admin can trigger a WhatsApp broadcast of promo messages to the customer list (using a separate number from the operational bot)
  4. Generated content uses Racing Point brand identity (colors, fonts, logo)
**Plans**: 2 plans

Plans:
- [ ] 158-01-PLAN.md — Backend: satori PNG generation API route (Next.js) + Rust broadcast endpoint with 24h rate limit
- [ ] 158-02-PLAN.md — Admin UI: Marketing tab on /cafe page with Generate Graphic buttons per promo and WhatsApp broadcast form

## v19.0 Progress

**Execution Order:** 149 -> 150 -> 151 -> 152 -> 153 -> 154 -> 155 -> 156 -> 157 -> 158
Note: Phase 152 can start after 149 (parallel with 150/151). Phase 156 can start after 149 (parallel with inventory/ordering).

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 149. Menu Data Model & CRUD | 2/2 | Complete    | 2026-03-22 |
| 150. Menu Import | 2/2 | Complete    | 2026-03-22 |
| 151. Menu Display | 2/2 | Complete    | 2026-03-22 |
| 152. Inventory Tracking | 2/2 | Complete    | 2026-03-22 |
| 153. Inventory Alerts | 2/2 | Complete    | 2026-03-22 |
| 154. Ordering Core | 3/3 | Complete    | 2026-03-22 |
| 155. Receipts & Order History | 2/2 | Complete    | 2026-03-22 |
| 156. Promotions Engine | 2/2 | Complete    | 2026-03-22 |
| 157. Promotions Integration | 2/2 | Complete    | 2026-03-22 |
| 158. Marketing & Content | 2/2 | Complete    | 2026-03-22 |

## v17.1 Watchdog-to-AI Migration

Replace all dumb restart-loop watchdogs with intelligent AI-driven recovery that detects patterns, escalates intelligently, and never causes more problems than it solves. Single recovery authority per machine, no fighting between systems.

- [x] **Phase 159: Recovery Consolidation Foundation** - Single recovery authority per machine, decision logging, and anti-cascade guard to prevent recovery systems fighting each other (completed 2026-03-22)
- [x] **Phase 160: RC-Sentry AI Migration** - Replace rc-sentry blind restart loop with pattern memory, Ollama escalation, decision logging, and graceful restart detection
 (completed 2026-03-22)
- [x] **Phase 161: Pod Monitor Merge** - Merge pod_monitor into pod_healer as single recovery authority with billing-aware WoL and graduated response (completed 2026-03-22)
- [x] **Phase 162: James Watchdog Migration** - Replace james_watchdog.ps1 with Rust-based AI monitor using pattern memory, graduated response, and Bono escalation (completed 2026-03-22)

## Phase Details

### Phase 159: Recovery Consolidation Foundation
**Goal**: No two recovery systems on any machine can restart the same process — single authority established, every recovery action is logged, and a cascade guard halts all recovery and alerts staff if 3+ actions fire within 60s
**Depends on**: Phase 158 (last completed phase)
**Requirements**: CONS-01, CONS-02, CONS-03
**Success Criteria** (what must be TRUE):
  1. A recovery authority registry exists (in racecontrol.toml or AppState) that maps each process to exactly one owner — no unregistered recovery system can trigger a restart
  2. Every restart, kill, and WoL decision is written to a recovery log (timestamp, machine, process, authority, reason) — an operator can read the log and understand why any recovery fired
  3. When 3+ recovery actions fire across any systems within 60 seconds, all automated recovery pauses and Uday receives a WhatsApp alert with the cascade summary — the system never silently spirals
  4. The anti-cascade guard distinguishes normal multi-pod recovery bursts (server downtime → all 8 pods restart) from true cascade scenarios — server-down restarts do not falsely trigger the guard
**Plans**: 2 plans

Plans:
- [x] 159-01-PLAN.md — rc-common recovery contracts (RecoveryAuthority, ProcessOwnership, RecoveryDecision, RecoveryLogger)
- [ ] 159-02-PLAN.md — CascadeGuard wired into AppState and pod_healer

### Phase 160: RC-Sentry AI Migration
**Goal**: rc-sentry stops blindly restarting rc-agent and instead checks pattern memory, distinguishes graceful restarts from real crashes, escalates to Ollama for unknown patterns, and logs every decision — blind 5s health poll + restart loop replaced end-to-end
**Depends on**: Phase 159
**Requirements**: SENT-01, SENT-02, SENT-03, SENT-04
**Success Criteria** (what must be TRUE):
  1. When the same crash pattern recurs 3+ times within 10 minutes, rc-sentry does not restart rc-agent — it queries Ollama instead and the restart is blocked until AI responds or timeout fires
  2. When rc-agent performs a graceful self-restart (RCAGENT_SELF_RESTART sentinel file present), rc-sentry detects it and does not escalate — no false-positive Ollama queries or staff alerts on intentional restarts
  3. Every restart decision made by rc-sentry is written to the activity log with: timestamp, crash pattern matched (or "unknown"), action taken, and outcome — the log shows a full audit trail
  4. When Ollama is queried for an unknown crash pattern, the response (or timeout) is recorded in debug-memory.json — the same pattern is handled faster on next occurrence
**Plans**: 2 plans

Plans:
- [ ] 160-01-PLAN.md — Sentinel detection + RecoveryLogger wiring (SENT-03, SENT-04)
- [ ] 160-02-PLAN.md — Pattern-aware escalation + pre-restart Ollama query (SENT-01, SENT-02)

### Phase 161: Pod Monitor Merge
**Goal**: pod_monitor and pod_healer become a single recovery authority — pod_monitor's WoL/restart logic merges into pod_healer, the separate restart path is deleted, maintenance-offline pods are never woken, and recovery uses a 4-step graduated response instead of immediate restart
**Depends on**: Phase 159
**Requirements**: PMON-01, PMON-02, PMON-03
**Success Criteria** (what must be TRUE):
  1. A pod in MAINTENANCE_MODE is never woken by WoL or restarted — pod_monitor checks billing_active and maintenance flags before triggering any recovery action
  2. There is one code path for pod recovery (pod_healer) — the old pod_monitor restart logic is deleted and cargo grep finds no duplicate restart triggers
  3. A first pod failure waits 30 seconds before any action; second failure triggers Tier 1 fix (rc-agent service restart); third failure escalates to AI; fourth failure alerts staff — the graduated response is observable in the recovery log
  4. Staff can observe the current recovery tier for any pod from the fleet health dashboard — the dashboard shows "waiting / Tier 1 / AI escalation / staff alert" state per pod
**Plans**: 2 plans

Plans:
- [x] 161-01-PLAN.md — Graduated recovery tracker + billing/maintenance gate (PMON-01, PMON-03)
- [x] 161-02-PLAN.md — Strip restart/WoL from pod_monitor, single recovery authority (PMON-02)

### Phase 162: James Watchdog Migration
**Goal**: james_watchdog.ps1 is replaced by a Rust binary that monitors Ollama, Claude Code, comms-link, and webterm with pattern memory and graduated response — blind 2-minute PowerShell restart loop eliminated, Bono is alerted on repeated failures instead of silent restarts continuing indefinitely
**Depends on**: Phase 159
**Requirements**: JWAT-01, JWAT-02, JWAT-03
**Success Criteria** (what must be TRUE):
  1. james_watchdog.ps1 is no longer running on James (.27) — the Rust monitor binary has replaced it in the Windows Task Scheduler and HKLM Run entries
  2. When any monitored service (Ollama, Claude Code, comms-link, webterm) fails once, the monitor waits and retries before acting — a single transient failure does not trigger an immediate restart
  3. When a monitored service fails 3+ times within a session, the Rust monitor sends a comms-link WS message to Bono with the service name, failure count, and last known error — Bono receives the alert without James or Uday needing to notice
  4. Pattern memory persists across monitor restarts (debug-memory.json on James) — known failure patterns are recognized and acted on immediately without re-learning from scratch
**Plans**: 2 plans

Plans:
- [ ] 162-01-PLAN.md — Core james monitor binary: failure_state, bono_alert, james_monitor (JWAT-01, JWAT-02, JWAT-03)
- [ ] 162-02-PLAN.md — Deploy binary, Task Scheduler registration, retire james_watchdog.ps1 (JWAT-01)

## v17.1 Progress

**Execution Order:** 159 -> 160 -> 161 -> 162
Note: Phases 160, 161, and 162 all depend on Phase 159 (foundation). Phases 160/161/162 can execute in parallel after 159 completes.

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 159. Recovery Consolidation Foundation | 2/2 | Complete    | 2026-03-22 |
| 160. RC-Sentry AI Migration | 2/2 | Complete    | 2026-03-22 |
| 161. Pod Monitor Merge | 2/2 | Complete    | 2026-03-22 |
| 162. James Watchdog Migration | 2/2 | Complete    | 2026-03-22 |


## v21.0 Cross-Project Sync & Stabilization

**Milestone Goal:** All Racing Point repos work in sync - shared contracts, no dead code, no known bugs, unified deploy, every service verified running at runtime.

- [x] **Phase 170: Repo Hygiene & Dependency Audit** - Archive dead repos, catalogue non-git folders, normalize git config, audit npm and cargo dependencies (completed 2026-03-22)
- [x] **Phase 171: Bug Fixes** - Fix all 4 known bugs blocking daily operations (pods DB desync, orphan PowerShell, process guard allowlist, Variable_dump.exe) (completed 2026-03-22)
- [x] **Phase 172: Standing Rules Sync** - Propagate standing rules to all active repos, sync to Bono VPS, add automated compliance check script (completed 2026-03-23)
- [x] **Phase 173: API Contracts** - Document all API boundaries, extract shared TypeScript types, generate OpenAPI specs, add contract tests and CI drift prevention (completed 2026-03-22)
- [x] **Phase 174: Health Monitoring & Unified Deploy** - Add /health to all services, central health check script, clean deploy-staging, unified deploy scripts and runbook, verify all services running at runtime (completed 2026-03-22)
- [x] **Phase 175: E2E Validation** - Run full 231-test suite on POS and Kiosk, cross-sync tests, triage and fix all critical failures
 (completed 2026-03-23)

## Phase Details

### Phase 170: Repo Hygiene & Dependency Audit
**Goal**: Dead repos are archived, non-git folders are catalogued, all active repos have consistent git config and .gitignore, all npm and cargo dependencies are audited for vulnerabilities
**Depends on**: Nothing (first phase)
**Requirements**: REPO-01, REPO-02, REPO-03, DEPS-01, DEPS-02, DEPS-03
**Success Criteria** (what must be TRUE):
  1. game-launcher, ac-launcher, and conspit-link repos show Archived status on GitHub with a README noting the merger target
  2. Non-git folders (bat-sandbox, computer-use, glitch-frames, marketing, serve, voice-assistant) each have a documented decision: archived, deleted, or kept with written rationale
  3. Every active repo returns consistent git user.name/email and has a .gitignore that excludes build artifacts, node_modules, and secrets
  4. npm audit on all Node.js repos shows zero high/critical vulnerabilities, or each vulnerability is documented with an upgrade-or-defer decision
  5. cargo audit on all Rust crates shows zero vulnerabilities, or each is documented with an upgrade-or-defer decision
**Plans:** 3/3 plans complete
Plans:
- [ ] 170-01-PLAN.md — Archive dead repos and catalogue non-git folders
- [ ] 170-02-PLAN.md — Normalize git config and .gitignore across active repos
- [ ] 170-03-PLAN.md — Audit npm and cargo dependencies for vulnerabilities

### Phase 171: Bug Fixes
**Goal**: All 4 known bugs blocking daily operations are patched and deployed across all 8 pods and the server
**Depends on**: Phase 170
**Requirements**: BUG-01, BUG-02, BUG-03, BUG-04
**Success Criteria** (what must be TRUE):
  1. After racecontrol server restart with an empty database, the kiosk fleet view shows all 8 pods (not "Waiting for pods") - verified live on the running server
  2. After a pod restarts, no orphan powershell.exe processes appear in Task Manager - verified on at least 2 pods
  3. Process guard report_only mode emits a report listing all processes seen on live pods, with the allowlist file committed to deploy-staging
  4. After pod boot, Variable_dump.exe does not appear in Task Manager - verified on at least 2 pods
**Plans**: 1 plan

Plans:
- [ ] 171-01-PLAN.md — Patch all 4 bugs: auto-seed pods on startup (BUG-01), process guard report_only config (BUG-03), confirm bat fixes for orphan PowerShell (BUG-02) and Variable_dump (BUG-04)

### Phase 172: Standing Rules Sync
**Goal**: Relevant standing rules from racecontrol CLAUDE.md are propagated to every active repo, Bono VPS repos are updated with matching rules, and a compliance check script verifies rule presence across all repos in one command
**Depends on**: Phase 171
**Requirements**: RULE-01, RULE-02, RULE-03
**Success Criteria** (what must be TRUE):
  1. Each active repo (racingpoint-admin, comms-link, deploy-staging, kiosk) has a CLAUDE.md with the standing rules subset relevant to that repo domain
  2. Bono VPS repos have the same standing rules applied - verified by reading a Bono repo CLAUDE.md via comms-link relay
  3. Running the compliance check script from James prints "All repos compliant" and exits 0, or exits non-zero listing exactly which repos are missing which rule categories
**Plans**: 3 plans

Plans:
- [x] 172-01-PLAN.md — Write CLAUDE.md rule subsets for all James-side active repos (14 repos)
- [x] 172-02-PLAN.md — Update comms-link CLAUDE.md categories + write compliance check script
- [x] 172-03-PLAN.md — Sync Bono VPS repos via relay and run compliance verification

### Phase 173: API Contracts
**Goal**: Every API boundary is documented, shared TypeScript types are extracted for kiosk and admin API communication, OpenAPI specs are generated for racecontrol REST endpoints, contract tests break on drift, and a CI check enforces this on every PR
**Depends on**: Phase 172
**Requirements**: CONT-01, CONT-02, CONT-03, CONT-04, CONT-05, CONT-06
**Success Criteria** (what must be TRUE):
  1. A single document lists every API boundary (racecontrol<->kiosk, racecontrol<->admin, racecontrol<->comms-link, racecontrol<->rc-agent) with endpoint names, request/response shapes, and ownership
  2. Kiosk and racecontrol share TypeScript type definitions from a common source: a type mismatch causes a TypeScript compile error, not a runtime error
  3. Admin and racecontrol share TypeScript type definitions from a common source: same compile-time guarantee
  4. An OpenAPI spec file exists for racecontrol REST endpoints and renders correctly in a browser (Swagger UI or equivalent)
  5. Contract tests run via npm test or cargo test and fail if a request/response shape changes without updating the contract definition
  6. A CI workflow runs contract tests on every PR and blocks merge on failure
**Plans**: 4 plans

Plans:
- [x] 173-01-PLAN.md — API boundary document (all 4 boundaries, key shapes)
- [ ] 173-02-PLAN.md — shared-types package + kiosk type wiring
- [ ] 173-03-PLAN.md — admin wiring + OpenAPI spec + Swagger UI
- [ ] 173-04-PLAN.md — contract tests (Vitest, fixtures) + GitHub Actions CI

### Phase 174: Health Monitoring & Unified Deploy
**Goal**: Every running service exposes /health, a central script polls all services and reports status, deploy-staging has a clean git status, and unified deploy scripts plus a runbook cover every service with post-deploy health verification built in
**Depends on**: Phase 173
**Requirements**: HLTH-01, HLTH-02, HLTH-03, REPO-04, REPO-05, DEPL-01, DEPL-02, DEPL-03, DEPL-04
**Success Criteria** (what must be TRUE):
  1. Every service (racecontrol :8080, kiosk :3300, web dashboard :3200, comms-link relay :8766, rc-sentry :8096) returns HTTP 200 from /health with a JSON body containing at minimum a status field
  2. Running check-health.sh from James prints a pass/fail line for each service and exits non-zero if any service is down
  3. After any deploy, the health check script runs automatically and its output is visible before the deploy is marked complete
  4. deploy-staging has zero untracked or modified files (git status clean) - all 714 previously dirty files triaged and committed, deleted, or gitignored
  5. A single deploy script deploys each service by name and runs the health check on completion
  6. The deployment runbook is committed to the repo with step-by-step procedures and one-command rollback instructions for each service
**Plans**: 5 plans

Plans:
- [x] 174-01-PLAN.md — Add /health to kiosk and web dashboard Next.js apps
- [x] 174-02-PLAN.md — Fix comms-link relay /health, verify racecontrol + rc-sentry
- [x] 174-03-PLAN.md — Triage deploy-staging 719 dirty files (gitignore + commit)
- [ ] 174-04-PLAN.md — Create check-health.sh and deploy.sh with post-deploy health check
- [ ] 174-05-PLAN.md — Deployment runbook + REPO-04/REPO-05 live verification checkpoint

### Phase 175: E2E Validation
**Goal**: The full 231-test E2E suite executes on both POS and Kiosk, cross-cutting sync tests verify real-time state propagation, and every test failure is fixed or documented as a known issue with root cause
**Depends on**: Phase 174
**Requirements**: E2E-01, E2E-02, E2E-03, E2E-04
**Success Criteria** (what must be TRUE):
  1. All 231 tests from E2E-TEST-SCRIPT.md execute against POS (:3200) with no tests skipped due to environment issues - a results report exists
  2. All 231 tests from E2E-TEST-SCRIPT.md execute against Kiosk (:8000) with no tests skipped due to environment issues - a results report exists
  3. Cross-cutting sync tests pass: a billing action on POS is reflected on Kiosk within the expected timeout, and a kiosk action is reflected on POS
  4. Every test failure has a triage entry: "fixed in this phase" with a commit hash, or "known issue" with root cause documented and a follow-up item filed
**Plans**: 2 plans

Plans:
- [ ] 175-01-PLAN.md — Build run-e2e.sh test runner + E2E-REPORT-TEMPLATE.md (E2E-01, E2E-02 framework)
- [ ] 175-02-PLAN.md — Cross-sync test guide, triage structure, human execution checkpoint (E2E-03, E2E-04)

## v21.0 Progress

**Execution Order:** 170 -> 171 -> 172 -> 173 -> 174 -> 175

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 170. Repo Hygiene & Dependency Audit | 3/3 | Complete    | 2026-03-22 |
| 171. Bug Fixes | 0/TBD | Complete    | 2026-03-22 |
| 172. Standing Rules Sync | 3/3 | Complete    | 2026-03-22 |
| 173. API Contracts | 4/4 | Complete    | 2026-03-22 |
| 174. Health Monitoring & Unified Deploy | 5/5 | Complete    | 2026-03-22 |
| 175. E2E Validation | 2/2 | Complete    | 2026-03-23 |

---

## v22.0 Feature Management & OTA Pipeline

### Phases

- [x] **Phase 176: Protocol Foundation + Cargo Gates** - Lay the rc-common protocol types and Cargo feature gate policy that every downstream phase depends on (completed 2026-03-24)
- [x] **Phase 177: Server-Side Registry + Config Foundation** - Build the server feature flag registry, config push channel, and REST endpoints that the admin UI and agent consumer both require (completed 2026-03-24)
- [x] **Phase 178: Agent & Sentry Consumer** - Wire rc-agent and rc-sentry to receive and apply flag updates, config pushes, and OTA download messages — rc-agent over WebSocket, rc-sentry via local config from rc-agent (completed 2026-03-24)
- [x] **Phase 179: OTA Pipeline** - Implement the full state-machine-driven release pipeline -- canary, staged rollout, session-gated binary swap, health gates, and auto-rollback
- [x] **Phase 180: Admin Dashboard UI** - Deliver operator-facing feature toggle and OTA release pages in the admin dashboard (completed 2026-03-25)
- [x] **Phase 181: Standing Rules Gate** - Codify all 41+ standing rules as machine-enforceable checks and wire them as pipeline gates
- [x] **Phase 182: Cross-Milestone Integration** - All active milestones updated to use v22.0's OTA pipeline, feature flags, and config push (completed 2026-03-25)

## Phase Details

### Phase 176: Protocol Foundation + Cargo Gates
**Goal**: All new WebSocket message variants, shared types, and Cargo feature gate structure exist in rc-common so every downstream phase can reference them without coordination
**Depends on**: Nothing (first phase of this milestone)
**Requirements**: CF-01, CF-02, CF-03, CF-04, PFC-01
**Success Criteria** (what must be TRUE):
  1. rc-agent compiles with --no-default-features and with default features -- both cargo build invocations succeed in CI
  2. rc-sentry compiles with --no-default-features and with default features -- both cargo build invocations succeed in CI
  3. AgentMessage and CoreToAgentMessage enums have an Unknown catch-all variant with #[serde(other)] -- older binaries silently ignore unknown message types instead of crashing. All existing serde tests still pass.
  4. The 7 new WebSocket message variants (FlagSync, ConfigPush, OtaDownload, OtaAck, ConfigAck, KillSwitch, FlagCacheSync) are present in rc-common protocol.rs and accepted by serde
  5. The single-binary-tier policy is documented in rc-common or CLAUDE.md -- no per-pod compile-time variant scheme exists
  6. rc-agent Cargo.toml lists ai-debugger and process-guard as optional features; default features include both. Telemetry excluded (too entangled with billing/game state).
  7. rc-sentry Cargo.toml lists watchdog, tier1-fixes, and ai-diagnosis as optional features; default features include all three; bare build (--no-default-features) produces a remote-exec-only binary
**Plans**: 3 plans
Plans:
- [ ] 176-01-PLAN.md — Protocol forward-compat (Unknown catch-all + 7 new WS message stubs)
- [ ] 176-02-PLAN.md — Cargo feature gates for rc-agent and rc-sentry
- [ ] 176-03-PLAN.md — CI minimal-build verification + single-binary-tier policy doc

### Phase 177: Server-Side Registry + Config Foundation
**Goal**: Operators can create and read feature flags and queue config pushes via REST endpoints, with all changes persisted to SQLite and an audit log recording every mutation
**Depends on**: Phase 176
**Requirements**: FF-01, FF-02, FF-03, CP-01, CP-02, CP-04, CP-05, CP-06, SYNC-01
**Success Criteria** (what must be TRUE):
  1. A named boolean flag can be created with a fleet-wide default and a per-pod override via POST /api/v1/flags -- the value persists across server restart
  2. A config push (billing rate change, game limit, etc.) submitted via REST is queued per-pod and delivered to a connected pod within seconds; the delivery is recorded in the audit log table with timestamp, field, old value, new value, and pushed_by
  3. A config push with an invalid value (negative billing rate, empty allowlist) is rejected at the server with HTTP 400 and a field-level error message -- the invalid value is never queued for pods
  4. The feature flag and config push endpoints are documented in the OpenAPI spec with shared TypeScript types in packages/shared-types/ -- the contract test passes
  5. Offline pods receive queued config pushes on reconnect via sequence-number-based ack; no push is silently lost
**Plans**: 4 plans
Plans:
- [x] 177-01-PLAN.md -- Feature flag registry: DB tables, AppState cache, flags.rs CRUD, broadcast, audit
- [x] 177-02-PLAN.md -- Config push: validation, queuing, delivery, WS handlers for FlagCacheSync + ConfigAck
- [x] 177-03-PLAN.md -- Cross-project sync: TypeScript types, OpenAPI spec, contract tests
- [x] 177-04-PLAN.md -- Gap closure: per-pod override resolution in WS broadcast + REQUIREMENTS.md update

### Phase 178: Agent & Sentry Consumer
**Goal**: rc-agent and rc-sentry receive flag updates, config pushes, and OTA download messages — rc-agent over WebSocket with hot-reload and offline cache, rc-sentry via local config file push from rc-agent (rc-sentry has no WS connection to server). Both write sentinel files before binary swap.
**Depends on**: Phase 176
**Requirements**: FF-04, FF-05, FF-07, FF-08, CP-03, SYNC-03, CF-04
**Success Criteria** (what must be TRUE):
  1. A flag toggle on the server propagates to all connected pods within seconds -- game launch and billing guard code reads the updated flag on the next invocation without a binary restart
  2. After the server connection drops, rc-agent reads flags-cache.json on startup and operates with last-known flags -- no panic, no config reset, no default fallback that contradicts the last server-pushed value
  3. Kill-switch flags (kill_*) override all other flag logic -- when a kill switch is set on the server, rc-agent halts the associated feature path on the next invocation regardless of other flag state
  4. Hot-reloadable config fields (billing rates, game limits, process guard whitelist, debug verbosity) update in-memory without restarting rc-agent; fields excluded from hot-reload (port bindings, WS URL) are documented and never accepted as hot updates
  5. New WS message types are added to the shared TypeScript types package and a contract test verifies the rc-common Rust types and TypeScript types agree on field names and shapes
**Plans**: 3 plans
Plans:
- [x] 178-01-PLAN.md -- In-memory flag cache, disk persistence, WS handlers (FlagSync, KillSwitch), FlagCacheSync on reconnect
- [ ] 178-02-PLAN.md -- ConfigPush hot-reload, ConfigAck flow, sentry-flags.json bridge, LaunchGame flag gate
- [ ] 178-03-PLAN.md -- TypeScript WS message types + contract tests (SYNC-03)

### Phase 179: OTA Pipeline
**Goal**: New rc-agent and rc-sentry releases can be deployed to the full fleet via a state-machine-driven pipeline that gates every wave on health checks, skips pods with active billing sessions, auto-rolls back on failure, and is impossible to interrupt without a trace. rc-sentry deploys independently from rc-agent (different binary, different restart sequence) but shares the same canary-first pipeline and health gate infrastructure.
**Depends on**: Phase 177, Phase 178
**Requirements**: OTA-01, OTA-02, OTA-03, OTA-04, OTA-05, OTA-06, OTA-07, OTA-08, OTA-09, OTA-10, SYNC-02, SYNC-05
**Success Criteria** (what must be TRUE):
  1. A release manifest (release-manifest.toml) exists for every deployment attempt and locks binary SHA256, config schema version, frontend build_id, git commit, and timestamp as one bundle -- no manifest means no deploy starts
  2. Deploying a new rc-agent binary to the fleet always hits Pod 8 first; the pipeline waits for health gate pass (WS connected, HTTP reachable, SHA256 matches manifest, no error spike) before advancing to the next wave
  3. A pod with an active billing session is skipped during its wave and retried after session end -- billing data is never lost due to a mid-session binary swap
  4. When the health gate fails after any wave, affected pods automatically revert to rc-agent-prev.exe and restart; the previous binary is always present on the pod and is never overwritten by the swap step
  5. The pipeline state (idle, building, staging, canary, staged-rollout, health-checking, completed, rolling-back) is persisted to deploy-state.json and survives a server restart -- an interrupted deploy can be resumed, not re-run from scratch
  6. rc-sentry, pod_monitor, and WoL all check the ota-in-progress.flag sentinel before triggering restarts during a deploy window -- no recovery system fights the OTA restarter
  7. Binary identity uses SHA256 content hash, not git commit hash -- a docs-only commit does not trigger a redeploy
**Plans**: 4 plans

Plans:
- [ ] 179-01-PLAN.md -- ReleaseManifest, PipelineState, SHA256, deploy-state.json persistence
- [ ] 179-02-PLAN.md -- Wave orchestrator, health gate, session gating
- [ ] 179-03-PLAN.md -- Auto-rollback, OTA sentinel coordination, check-health.sh extension
- [ ] 179-04-PLAN.md -- API endpoints, pipeline integration, human verification

### Phase 180: Admin Dashboard UI
**Goal**: Operators can toggle feature flags per-pod or fleet-wide and trigger OTA releases from the admin dashboard, with live wave progress, pod drain status, and rollback controls visible without a terminal
**Depends on**: Phase 177
**Requirements**: FF-06, SYNC-04
**Success Criteria** (what must be TRUE):
  1. The admin dashboard has a Feature Flags page with toggle switches for every registered flag, a scope selector for fleet-wide vs per-pod override, and immediate propagation to connected pods on toggle -- no deploy or restart required
  2. The fleet health table shows a flag divergence column -- pods whose cached flags differ from the server registry are highlighted
  3. The admin dashboard has an OTA Releases page showing current pipeline state, wave progress, per-pod deploy status (pending, deploying, draining, complete, failed), and a one-click rollback button that triggers revert to the previous known-good release
  4. Pods with active billing sessions show a "draining" status during OTA rather than being skipped silently -- the operator can see which pods are waiting and why
  5. Feature flag and config push changes made via the dashboard cascade visibly to all affected components (racecontrol, rc-agent, kiosk, admin) per the cross-process update standing rule -- the dashboard reflects the post-propagation state
**Plans**: 1 plan

Plans:
- [ ] 180-01-PLAN.md -- Feature Flags page + OTA Releases page + API types + nav entries

### Phase 181: Standing Rules Gate
**Goal**: Every standing rule is classified as AUTO, HUMAN-CONFIRM, or INFORMATIONAL and the appropriate enforcement mechanism fires at every pipeline step -- no gate can be bypassed, and HUMAN-CONFIRM rules pause the pipeline with a named checklist
**Depends on**: Phase 179
**Requirements**: SR-01, SR-02, SR-03, SR-04, SR-05, SR-06, SR-07, SYNC-06
**Success Criteria** (what must be TRUE):
  1. Every standing rule in CLAUDE.md is tagged AUTO, HUMAN-CONFIRM, or INFORMATIONAL -- the classification list is committed to the repo as a reference document
  2. The pre-deploy gate script (gate-check.sh) runs before wave 1 of every OTA and checks: cargo test green, no unwrap in diff, static CRT config present, LOGBOOK updated, bat files clean ASCII -- the pipeline blocks if any check fails
  3. The post-deploy verification gate runs after each wave and checks: build_id matches manifest, fleet health passes, billing session roundtrip works, no error spike -- the pipeline blocks the next wave if any check fails
  4. There is no force-continue or skip-gate command -- the only exit from a failed gate is rollback
  5. HUMAN-CONFIRM rules cause the pipeline to pause and emit a named operator checklist; the pipeline resumes only after explicit operator confirmation of each checklist item
  6. CLAUDE.md has a new OTA Pipeline standing rules section covering: always preserve prev binary, never deploy without manifest, billing sessions drain before swap, OTA sentinel file protocol, config push never through fleet exec endpoint -- Bono receives these rules via standing rules sync
  7. gate-check.sh extends the v21.0 run-all.sh E2E framework as a superset -- it does not create a parallel test system
**Plans**: 3 plans

Plans:
- [ ] 181-01-PLAN.md -- Standing rules registry + OTA Pipeline rules in CLAUDE.md
- [ ] 181-02-PLAN.md -- gate-check.sh pre-deploy and post-wave gate script
- [ ] 181-03-PLAN.md -- Pipeline integration (Paused state) + Bono sync

### Phase 182: Cross-Milestone Integration
**Goal**: All active milestones updated to use v22.0's OTA pipeline, feature flags, and config push — overlapping phases superseded or merged, future phases gain v22.0 as a dependency
**Depends on**: Phase 181
**Requirements**: XMIL-01, XMIL-02, XMIL-03, XMIL-04, XMIL-05, XMIL-06
**Success Criteria** (what must be TRUE):
  1. v6.0 Salt Fleet Management phases 36-40 are reviewed — fleet config distribution aspects superseded by v22.0 config push are marked, Salt scope narrowed to remote exec only (or deprecated if v22.0 covers it)
  2. v10.0 Phase 62 (Fleet Config Distribution) is marked superseded by v22.0 CP-01 to CP-06 — no duplicate config push system exists
  3. v13.0 Multi-Game Launcher incomplete phases (82-88) updated to use Cargo feature gates (CF-01) for game telemetry modules and feature flags (FF-01) for per-pod game enablement
  4. v15.0 Phase 111 (Code Signing + Per-Game Canary) updated to use OTA-10 (SHA256 binary identity) and OTA-02 (canary Pod 8) — no duplicate canary infrastructure
  5. v17.0 Phase 127 (CI/CD Pipeline) updated to use OTA-08 (deploy state machine) — cloud deploy and local deploy share the same pipeline architecture
  6. All future phases across all milestones include a standing rules gate dependency — no phase can ship without running gate-check.sh
**Plans**: 1 plan

Plans:
- [ ] 187-01-PLAN.md � Sentry-aware relaunch logic + build verification

## v22.0 Progress

**Execution Order:** 176 -> 177 -> 178 (parallel with 177 after 176) -> 179 -> 180 (parallel with 179 after 177) -> 181 -> 182

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 176. Protocol Foundation + Cargo Gates | 3/3 | Complete    | 2026-03-24 |
| 177. Server-Side Registry + Config Foundation | 4/4 | Complete    | 2026-03-24 |
| 178. Agent-Side Consumer | 3/3 | Complete    | 2026-03-24 |
| 179. OTA Pipeline | 3/3 | Complete    | 2026-03-25 |
| 180. Admin Dashboard UI | 1/1 | Complete    | 2026-03-25 |
| 181. Standing Rules Gate | 3/3 | Complete | 2026-03-25 |
| 182. Cross-Milestone Integration | 1/1 | Complete    | 2026-03-25 |

---

## v17.1 Watchdog-to-AI Migration

**Goal:** Replace dumb restart-loop watchdogs with intelligent AI-driven recovery. Detect -> pattern memory -> Tier 1 fix -> escalate to AI -> alert staff after 3+ failures. Single recovery authority per machine, no fighting between self_monitor / rc-sentry / pod_monitor / WoL.

**Phase start:** 183 (last phase in roadmap was 182)

## Phases

- [x] **Phase 183: Recovery Events API** - Server-side recovery events endpoint that all pod-side phases report to (completed 2026-03-24)
- [x] **Phase 184: rc-sentry Crash Handler Upgrade** - Spawn verification, graduated Tier 1-4 recovery, pattern memory wired into crash handler (completed 2026-03-24)
- [x] **Phase 185: pod_healer WoL Coordination** - Recovery authority enforcement and context-aware Wake-on-LAN with intent checking (completed 2026-03-24)
- [x] **Phase 186: MAINTENANCE_MODE Auto-Clear** - JSON diagnostic payload, 30-min auto-clear, WhatsApp staff alert (completed 2026-03-24)
- [x] **Phase 187: self_monitor Coordination** - rc-agent self_monitor yields to rc-sentry, PowerShell relaunch becomes rare fallback (completed 2026-03-24)
- [x] **Phase 188: James Watchdog + rc-watchdog Grace Window** - James-local AI watchdog replaces james_watchdog.ps1; rc-watchdog adds grace window (completed 2026-03-24)

## Phase Details

### Phase 183: Recovery Events API
**Goal**: The racecontrol server exposes a recovery events endpoint so all recovery authorities (rc-sentry, pod_healer, self_monitor) can report attempts and query each other's recent actions -- enabling cross-machine recovery visibility without any pod-to-pod communication
**Depends on**: Nothing (server-side only, prerequisite for all other phases)
**Requirements**: COORD-04
**Success Criteria** (what must be TRUE):
  1. POST /api/v1/recovery/events accepts a recovery event from rc-sentry (pod_id, process, action, spawn_verified, server_reachable) and returns 201 -- confirmed by curl from James's machine pointing at server .23
  2. GET /api/v1/recovery/events?pod_id=pod-8&since_secs=120 returns all recovery events for Pod 8 in the last 2 minutes -- pod_healer can query this before deciding to send WoL
  3. The in-memory ring buffer is capped at 200 events and does not grow without bound -- confirmed by pushing 250 events and observing the oldest are dropped
  4. If the fleet alert endpoint (POST /api/v1/fleet/alert) does not already exist, it is added alongside recovery.rs in this phase so Tier 4 WhatsApp escalation has a working target
  5. Server rebuild and deploy to .23 completes successfully -- build_id on /api/v1/health matches git rev-parse --short HEAD
**Plans**: 1 plan

Plans:
- [ ] 183-01-PLAN.md — Recovery events ring buffer, POST/GET handlers, deploy to server

### Phase 184: rc-sentry Crash Handler Upgrade
**Goal**: rc-sentry's crash handler executes Tier 1 deterministic fixes, checks Tier 2 pattern memory for instant replay, queries Tier 3 Ollama for unknown patterns, escalates to staff after 3+ failures, verifies that spawned processes actually started, and reports every attempt to the recovery events API -- replacing blind restart-loop with a 4-tier graduated response
**Depends on**: Phase 183 (recovery events API must exist before rc-sentry reports to it)
**Requirements**: SPAWN-01, SPAWN-02, SPAWN-03, GRAD-01, GRAD-02, GRAD-03, GRAD-04, GRAD-05
**Success Criteria** (what must be TRUE):
  1. After rc-agent is killed on Pod 8, rc-sentry polls /health at 500ms intervals for up to 10s after restart -- spawn_verified: true only appears in the recovery event when HTTP 200 is received; a process that silently fails to start produces spawn_verified: false
  2. A crash pattern matching debug-memory.json fires the recorded fix instantly before any Ollama query -- confirmed by observing the pattern memory hit in rc-sentry logs with no Ollama round-trip latency
  3. A crash with a matching Tier 1 fix (stale socket, zombie process, config corruption) is resolved without Ollama -- Tier 1 fires first, Ollama query only follows if Tier 1 fails
  4. After 3+ failed recovery attempts on one pod, a WhatsApp alert reaches Uday with pod ID, failure count, and last error -- staff escalation fires and does not repeat before the configured cooldown
  5. Recovery events with server_reachable: false are tagged inconclusive and do not count toward the MAINTENANCE_MODE threshold -- server-down disconnects never trigger pod lockout
  6. All GUI process restarts route through Session 1 spawn path (WTSQueryUserToken + CreateProcessAsUser) -- std::process::Command is never used for interactive processes on pods
  7. Pod 8 canary: kill rc-agent, observe graduated response in rc-sentry logs (Tier 0 hysteresis -> Tier 1 -> Tier 2 -> restart -> spawn verify -> recovery event posted) -- full pipeline runs end-to-end before fleet deploy
**Plans**: 3 plans

Plans:
- [ ] 184-01-PLAN.md — Graduated crash handler with spawn verification, server_reachable, recovery event reporting (Tier 1+2)
- [ ] 184-02-PLAN.md — Tier 3 Ollama diagnosis + Tier 4 WhatsApp escalation
- [ ] 184-03-PLAN.md — Session 1 spawn path for GUI process launches

### Phase 185: pod_healer WoL Coordination
**Goal**: pod_healer queries the recovery events API before escalating to Wake-on-LAN -- if rc-sentry already restarted the pod with spawn_verified: true within the last 60 seconds, WoL is skipped; a WOL_SENT sentinel is written via rc-sentry before sending WoL so all recovery systems see the escalation
**Depends on**: Phase 183 (recovery events API), Phase 184 (rc-sentry must be reporting events before pod_healer can query them)
**Requirements**: COORD-01, COORD-02, COORD-03, MAINT-04
**Success Criteria** (what must be TRUE):
  1. When rc-sentry restarts rc-agent with spawn_verified: true and pod_healer detects the pod as offline within 60s of that event, pod_healer skips WoL -- confirmed by observing "skipping WoL, sentry restarted within grace window" in pod_healer logs
  2. When pod_healer does escalate to WoL, it writes WOL_SENT sentinel via rc-sentry /exec before sending the magic packet -- all recovery systems can see the WoL decision via sentinel check
  3. ProcessOwnership registry enforcement is wired at all call sites in rc-sentry, self_monitor, pod_monitor, and the WoL path -- no two recovery authorities can claim the same process simultaneously
  4. GRACEFUL_RELAUNCH sentinel reliably distinguishes intentional restarts from crashes -- a deliberate rc-agent self-restart does not trigger pod_healer WoL within the deconfliction window
  5. Recovery intent file (recovery-intent.json) is written before any restart attempt and expires after 2 minutes -- a pod that enters MAINTENANCE_MODE while a recovery intent is active does not trigger a new recovery action from a different authority
**Plans**: 2 plans

Plans:
- [ ] 185-01-PLAN.md -- Recovery coordination primitives (ProcessOwnership, RecoveryIntentStore, GRACEFUL_RELAUNCH sentinel checking)
- [ ] 185-02-PLAN.md -- Context-aware WoL (recovery event query, MAINTENANCE_MODE check, WOL_SENT sentinel)

### Phase 186: MAINTENANCE_MODE Auto-Clear
**Goal**: MAINTENANCE_MODE stops being a silent permanent pod killer -- it now carries a JSON diagnostic payload (reason, timestamp, restart count), auto-clears after 30 minutes or when WOL_SENT sentinel exists, and sends a WhatsApp alert to staff the moment it activates on any pod
**Depends on**: Phase 185 (WOL_SENT sentinel written by pod_healer is what triggers the immediate auto-clear path)
**Requirements**: MAINT-01, MAINT-02, MAINT-03
**Success Criteria** (what must be TRUE):
  1. After 3 rapid rc-agent crashes triggering MAINTENANCE_MODE, a WhatsApp message arrives within 60 seconds containing pod ID, reason, restart count, and timestamp in IST -- staff knows a pod is locked without needing SSH
  2. After 30 minutes, MAINTENANCE_MODE auto-clears and an rc-agent restart is attempted -- pod recovers without manual "del MAINTENANCE_MODE" intervention
  3. When WOL_SENT sentinel is present alongside MAINTENANCE_MODE, the auto-clear fires immediately (not after 30 minutes) -- WoL from pod_healer breaks the deadlock
  4. Reading C:\RacingPoint\MAINTENANCE_MODE on a pod in maintenance shows valid JSON with reason, timestamp, restart_count, and diagnostic_context fields -- not an empty file
  5. pod_healer reads MAINTENANCE_MODE JSON via rc-sentry /files before sending WoL -- WoL is never sent to a pod in maintenance mode unless the auto-clear condition is met, eliminating the WoL-into-maintenance infinite loop
**Plans**: 1 plan

Plans:
- [ ] 186-01-PLAN.md — JSON maintenance payload, 30-min auto-clear, WOL_SENT immediate clear, WhatsApp alert on activation

### Phase 187: self_monitor Coordination
**Goal**: rc-agent's self_monitor yields to rc-sentry when sentry is reachable -- instead of spawning a PowerShell process to relaunch itself (leaking 90MB per restart), self_monitor writes GRACEFUL_RELAUNCH and exits cleanly, letting rc-sentry handle the restart through the verified Session 1 spawn path
**Depends on**: Phase 184 (rc-sentry must be upgraded and stable before self_monitor defers to it)
**Requirements**: SELF-01, SELF-02
**Success Criteria** (what must be TRUE):
  1. When self_monitor detects a crash condition with rc-sentry reachable on TCP :8091, no PowerShell process is spawned -- tasklist shows zero orphan powershell.exe processes from self_monitor after the restart cycle
  2. When rc-sentry is unreachable (stopped for testing), self_monitor falls back to the PowerShell+DETACHED_PROCESS path -- rc-agent still relaunches even without sentry supervision
  3. Three-state verification passes on Pod 8: (a) sentry up + kill agent -- sentry restarts agent, no PowerShell spawned; (b) sentry down + kill agent -- PowerShell fallback relaunches agent; (c) sentry down + kill agent + restart sentry -- sentry takes over the already-running agent without double-restart
  4. The port :8090 double-bind race condition no longer occurs -- GRACEFUL_RELAUNCH sentinel prevents rc-sentry from issuing a simultaneous restart while self_monitor's PowerShell path is still starting the new process
**Plans**: 1 plan

Plans:
- [ ] 187-01-PLAN.md — Sentry-aware relaunch logic + build verification

### Phase 188: James Watchdog + rc-watchdog Grace Window
**Goal**: james_watchdog.ps1's blind 2-minute service check is replaced by a Rust-based AI watchdog using shared ollama.rs from rc-common with graduated Tier 1-4 response; rc-watchdog adds a 30-second grace window that reads sentry-restart-breadcrumb.txt before acting, plus spawn verification after session1 launch
**Depends on**: Phase 184 (ollama.rs must be moved to rc-common before rc-watchdog can share it; rc-sentry must be stable before rc-watchdog defers to it)
**Requirements**: JAMES-01, JAMES-02, JAMES-03
**Success Criteria** (what must be TRUE):
  1. james_watchdog.ps1 is deleted from deploy-staging; rc-watchdog (Rust binary) monitors comms-link, go2rtc, rc-sentry-ai, and Ollama with health-poll verification -- confirmed by tasklist on James's machine showing rc-watchdog.exe, not a powershell.exe running the old script
  2. A service failure on James's machine triggers graduated response: count 1 waits, count 2 restarts, count 3 queries Ollama for diagnosis, count 4+ sends WhatsApp alert to Uday -- blind immediate restart is eliminated
  3. After rc-watchdog triggers a restart, it polls the target service's health endpoint at 500ms intervals for up to 10 seconds before declaring success -- spawn_verified: true only if the health endpoint responds with HTTP 200
  4. When sentry-restart-breadcrumb.txt is less than 30 seconds old, rc-watchdog skips its restart action -- confirmed by manually touching the breadcrumb file and observing "grace window active, skipping restart" in rc-watchdog log
**Plans**: 1 plan

Plans:
- [ ] 188-01-PLAN.md — Shared ollama.rs, spawn verification, sentry breadcrumb grace window

## v17.1 Progress

**Execution Order:** 183 -> 184 -> 185 -> 186 -> 187 -> 188

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 183. Recovery Events API | 1/1 | Complete    | 2026-03-24 |
| 184. rc-sentry Crash Handler Upgrade | 3/3 | Complete    | 2026-03-24 |
| 185. pod_healer WoL Coordination | 2/2 | Complete    | 2026-03-24 |
| 186. MAINTENANCE_MODE Auto-Clear | 1/1 | Complete    | 2026-03-24 |
| 187. self_monitor Coordination | 1/1 | Complete    | 2026-03-24 |
| 188. James Watchdog + rc-watchdog Grace Window | 1/1 | Complete    | 2026-03-24 |

---

## v23.0 Audit Protocol v4.0 — Automated Fleet Audit System

**Goal:** Transform the manual 60-phase AUDIT-PROTOCOL v3.0 into a single-command automated runner that produces structured, comparable results across the entire fleet — no copy-paste, no manual tracking, no missed checks.

**Phase start:** 189 (last phase was 188)

## Phases

- [x] **Phase 189: Core Scaffold and Shared Primitives** - audit.sh entry point, lib/core.sh with all safe wrapper functions, JSON schema, auth token acquisition, prerequisites check
 (completed 2026-03-25)
- [x] **Phase 190: Phase Scripts Tiers 1-9 (Sequential Baseline)** - Port v3.0 phases 1-34 as non-interactive bash functions; sequential execution baseline; mode and tier selectors (completed 2026-03-25)
- [x] **Phase 191: Parallel Engine and Phase Scripts Tiers 10-18** - lib/parallel.sh with file-based semaphore; port v3.0 phases 35-60; audit runtime reduced from ~24 min to ~6 min (completed 2026-03-25)
- [x] **Phase 192: Intelligence Layer** - Delta tracking, known-issue suppression with expiry, severity scoring, Markdown report generation, results storage (completed 2026-03-25)
- [x] **Phase 193: Auto-Fix, Notifications, and Results Management** - Safe auto-fix engine with whitelist, Bono/WhatsApp notifications, git commit of results (completed 2026-03-25)

## Phase Details

### Phase 189: Core Scaffold and Shared Primitives
**Goal**: Operators can run `bash audit/audit.sh --mode quick` and receive a valid structured JSON result file — auth token obtained automatically, all primitives working correctly, Windows quoting and curl pitfalls mitigated before any check is built on top of them
**Depends on**: Nothing (first phase of v23.0)
**Requirements**: RUN-01, RUN-02, RUN-03, RUN-05, RUN-06, RUN-07, RUN-08, RUN-09, RUN-10, EXEC-01, EXEC-02, EXEC-07
**Success Criteria** (what must be TRUE):
  1. `bash audit/audit.sh --mode quick` runs to completion without hanging — auth token obtained from /api/v1/terminal/auth using PIN from env var, jq/curl/ssh prerequisites validated, and a result JSON file written to audit/results/
  2. Running the audit when jq is not installed prints a clear error and exits non-zero within 2 seconds — no cryptic downstream failures
  3. `audit/lib/core.sh` functions `emit_result`, `http_get`, `safe_remote_exec`, and `safe_ssh_capture` are callable from a test script and produce correctly structured JSON output
  4. When venue-closed state is detected (no active billing session + outside 09:00-22:00 IST fallback), a phase that checks pod display hardware emits `status: QUIET` instead of `status: FAIL` in its JSON result
  5. Every phase result JSON record contains `mode`, `venue_state`, `timestamp` (IST), `phase`, `tier`, `host`, `status`, and `message` fields — the schema required by delta tracking in Phase 192 is established from the first run
**Plans**: 3 plans

Plans:
- [ ] 189-01-PLAN.md — audit.sh entry point + stub lib/core.sh: mode parsing, prereq checks (jq/curl/AUDIT_PIN), IST result dir init, auth acquisition, exit codes 0/1/2
- [ ] 189-02-PLAN.md — lib/core.sh full implementation: emit_result (9-field JSON), http_get (quote strip), safe_remote_exec (temp file pattern), safe_ssh_capture (banner protection), get_session_token (AUDIT_PIN from env), venue_state_detect, ist_now
- [ ] 189-03-PLAN.md — Phase 01 Fleet Inventory proof-of-concept: phase01.sh (server+pods health), QUIET logic for closed venue, audit.sh phase runner wiring, end-to-end validation

### Phase 190: Phase Scripts Tiers 1-9 (Sequential Baseline)
**Goal**: All v3.0 phases 1-34 (tiers 1-9) run non-interactively in sequential mode and produce correct PASS/WARN/FAIL/QUIET results — verified against the live fleet before parallelism is introduced
**Depends on**: Phase 189 (lib/core.sh primitives must exist and work before any phase check is built on them)
**Requirements**: RUN-04, EXEC-05, EXEC-06
**Success Criteria** (what must be TRUE):
  1. `bash audit/audit.sh --mode standard` runs all phases in tiers 1-9 sequentially to completion against the live fleet — every offline pod produces QUIET or FAIL within its 10s timeout, not a hung process
  2. `bash audit/audit.sh --tier 2` runs only tier 2 phases and exits — tier selector works correctly
  3. `bash audit/audit.sh --phase 07` runs only phase 07 and exits — phase selector works correctly
  4. Every phase script in audit/phases/tier1/ through audit/phases/tier9/ exits 0 always — errors are encoded in the JSON result, never as bash exit codes that would abort the runner
  5. A phase checking a service on server .23 produces a result within 10 seconds even when .23 is unreachable — timeout enforcement works and no single offline host blocks the audit
**Plans**: 3 plans

Plans:
- [ ] 190-01-PLAN.md — Tiers 1-3: infrastructure checks (server health, build IDs, network), core services (racecontrol, rc-agent, rc-sentry), display/UX (lock screen, Edge, blanking)
- [ ] 190-02-PLAN.md — Tiers 4-6: billing/session lifecycle, games/hardware (AC, FFB, USB), notifications (WhatsApp, comms-link)
- [ ] 190-03-PLAN.md — Tiers 7-9: cloud/PWA (Bono VPS, app.racingpoint.cloud), security (JWT, firewall, process guard), data/analytics (laps DB, leaderboard, telemetry)

### Phase 191: Parallel Engine and Phase Scripts Tiers 10-18
**Goal**: All 60 v3.0 phases are ported and the audit runtime drops from ~24 minutes to ~6 minutes via parallel pod queries — file-based semaphore enforces the 4-concurrent-connection cap, no output interleaving, no ARP flood on the venue LAN
**Depends on**: Phase 190 (all sequential phase scripts must be verified correct before parallelism is introduced)
**Requirements**: EXEC-03, EXEC-04
**Success Criteria** (what must be TRUE):
  1. `bash audit/audit.sh --mode full` completes in under 8 minutes on the live fleet — parallel execution is working and reducing total runtime compared to sequential baseline
  2. At no point during a full audit run are more than 4 simultaneous connections open to pod hosts — the file-based semaphore is enforced
  3. All 60 phase scripts exist in audit/phases/ across 18 tier directories — complete v3.0 port with no phases missing
  4. The result JSON after a full parallel run contains exactly one result record per phase per host — no duplicate or missing records from background job output interleaving
  5. When 3 pods are offline simultaneously during a full run, the audit still completes within 8 minutes — offline pods time out individually without blocking other parallel jobs
**Plans**: 3 plans

Plans:
- [ ] 191-01-PLAN.md — lib/parallel.sh: file-based semaphore (max 4 concurrent), pod_loop helper, 200ms stagger, audit.sh tier 10-18 dispatch
- [ ] 191-02-PLAN.md — Tiers 10-12 phase scripts (phases 45-53): Ops/Compliance, E2E Journeys, Code Quality
- [ ] 191-03-PLAN.md — Tiers 13-18 phase scripts (phases 54-60): Registry, Data Integrity, Test Suites, Cloud, Customer Flows, Cross-System Chains

### Phase 192: Intelligence Layer
**Goal**: Two consecutive audit runs produce a delta report that correctly identifies regressions (PASS to FAIL), improvements (FAIL to PASS), and new issues — and known recurring issues can be suppressed with mandatory expiry dates so they appear as SUPPRESSED rather than cluttering the FAIL list
**Depends on**: Phase 191 (delta tracking requires at least 2 completed audit runs with JSON output; JSON schema with mode/venue_state fields must be established in Phase 189)
**Requirements**: INTL-01, INTL-02, INTL-03, INTL-04, INTL-05, INTL-06, INTL-07, INTL-08, RSLT-01, RSLT-02, RSLT-04
**Success Criteria** (what must be TRUE):
  1. After two consecutive audit runs where one phase degrades from PASS to FAIL, the generated Markdown report contains a "Regressions" section listing that phase — delta tracking correctly identifies and surfaces the degradation
  2. A PASS to QUIET transition (venue was open, then closed) does NOT appear as a regression in the delta report — mode-aware and venue-state-aware comparison prevents false regressions from context changes
  3. Adding a suppress.json entry for a known failing phase causes that phase to appear as SUPPRESSED (with reason) in subsequent reports — not silently hidden and not as a FAIL
  4. A suppress.json entry whose `expires_date` is in the past is automatically ignored — the phase reverts to its actual status without manual cleanup
  5. `audit/results/YYYY-MM-DD_HH-MM/` contains both a `-report.md` and a `-summary.json` file after every audit run — dual output format works for both human reading and machine parsing
**Plans**: 4 plans

Plans:
- [ ] 192-01-PLAN.md — Results storage: audit/results/YYYY-MM-DD_HH-MM/ directory, latest symlink, results/index.json audit run history (RSLT-01, RSLT-02, RSLT-04)
- [ ] 192-02-PLAN.md — lib/delta.sh: jq-based join on phase/host between runs; REGRESSION/IMPROVEMENT/PERSISTENT/NEW_ISSUE/STABLE categories; mode-aware and venue-state-aware comparison (INTL-01, INTL-02)
- [ ] 192-03-PLAN.md — suppress.json schema + expiry enforcement + SUPPRESSED status emission (INTL-03, INTL-04, INTL-08); severity scoring P1/P2/P3 per phase (INTL-05)
- [ ] 192-04-PLAN.md — generate-report.sh: Markdown report with tier tables, delta section, suppressed section, fix log, overall verdict; summary JSON with counts (INTL-06, INTL-07)

### Phase 193: Auto-Fix, Notifications, and Results Management
**Goal**: Operators can run `bash audit/audit.sh --mode full --auto-fix --notify --commit` and get the full pipeline — safe fixes executed on idle pods, Bono notified via comms-link, Uday's phone gets a WhatsApp summary with P1/P2 counts, and results committed to git — all gated so no fix ever touches an active billing session
**Depends on**: Phase 192 (auto-fix must not be enabled until PASS/FAIL signals are confirmed accurate from at least 3 consecutive runs; notifications send delta summaries that require Phase 192's delta tracking)
**Requirements**: FIX-01, FIX-02, FIX-03, FIX-04, FIX-05, FIX-06, FIX-07, FIX-08, NOTF-01, NOTF-02, NOTF-03, NOTF-04, NOTF-05, RSLT-03
**Success Criteria** (what must be TRUE):
  1. Running `bash audit/audit.sh --mode standard --auto-fix` on a pod with an active billing session produces SKIP_ACTIVE_SESSION in the fix log for that pod — `is_pod_idle()` gate works and no fix action is attempted on a pod earning revenue
  2. Running the same command on an idle pod with a stale MAINTENANCE_MODE sentinel file clears the sentinel and logs the before/after state to the fix log — safe fix FIX-04 works end-to-end
  3. Running without `--auto-fix` flag produces zero fix actions even when fixable issues are detected — auto-fix is off by default
  4. Running with `--notify` sends a message to Bono via comms-link and appends an entry to INBOX.md — dual-channel notification works and a notification failure does not abort or fail the audit run
  5. Running with `--commit` commits the results directory to git — audit results are preserved in version history
**Plans**: 3 plans

Plans:
- [ ] 193-01-PLAN.md — lib/fixes.sh: approved-fixes whitelist, is_pod_idle() gate, OTA_DEPLOYING/MAINTENANCE_MODE sentinel checks, FIX-01 through FIX-08 implementations, per-fix audit log
- [ ] 193-02-PLAN.md — lib/notify.sh: comms-link WS relay + INBOX.md dual-channel (NOTF-01, NOTF-02), WhatsApp via Bono relay Evolution API (NOTF-03), --notify flag gate (NOTF-04), delta summary in notification (NOTF-05)
- [ ] 193-03-PLAN.md — RSLT-03 git commit of results; end-to-end integration test of full pipeline with all flags

## v23.0 Progress

**Execution Order:** 189 -> 190 -> 191 -> 192 -> 193

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 189. Core Scaffold and Shared Primitives | 3/3 | Complete    | 2026-03-25 |
| 190. Phase Scripts Tiers 1-9 (Sequential Baseline) | 3/3 | Complete    | 2026-03-25 |
| 191. Parallel Engine and Phase Scripts Tiers 10-18 | 3/3 | Complete    | 2026-03-25 |
| 192. Intelligence Layer | 4/4 | Complete    | 2026-03-25 |
| 193. Auto-Fix, Notifications, and Results Management | 3/3 | Complete    | 2026-03-25 |


---

## v24.0 Game Launch & Billing Rework -- Self-Improving Launch Engine

**Milestone Goal:** Rework the game launcher and billing system to be resilient, accurate, and self-improving. Games launch flawlessly, billing starts precisely when the car is on-track and controllable, and every launch/billing event feeds a metrics foundation that makes the system smarter over time.

**Phases:** 194-200 (7 phases, 63 requirements)
**Started:** 2026-03-26

### v24.0 Phases

- [ ] **Phase 194: Pod ID Normalization** - Canonical pod ID format everywhere, eliminating billing_alt_id and inconsistent lookups
- [ ] **Phase 195: Metrics Foundation** - SQLite + JSONL dual storage for every launch, billing, and crash event with queryable APIs
- [ ] **Phase 196: Game Launcher Structural Rework** - Trait-based per-game launchers, billing gate fixes, state machine corrections, and error propagation
- [ ] **Phase 197: Launch Resilience & AC Hardening** - Dynamic timeouts, pre-launch checks, auto-retry, error taxonomy, AC polling-based waits, and arg parsing fixes
- [ ] **Phase 198: On-Track Billing** - PlayableSignal rework per game, WaitingForGame dashboard state, billing pause/resume on crash, and timeout/multiplayer fixes
- [ ] **Phase 199: Crash Recovery** - Clean state reset, history-informed recovery selection, auto-relaunch with preserved args, staff alerting, and safe mode/grace timer fixes
- [ ] **Phase 200: Self-Improving Intelligence** - Combo reliability scores, low-success warnings, alternative suggestions, admin launch matrix, and rolling-window self-tuning

## v24.0 Phase Details

### Phase 194: Pod ID Normalization
**Goal**: Every system component uses one canonical pod ID format -- no more billing_alt_id workarounds, no more lookups failing because game_launcher uses "pod-1" while billing uses "pod_1"
**Depends on**: Nothing (foundation for all v24.0 work)
**Requirements**: PODID-01, PODID-02, PODID-03
**Success Criteria** (what must be TRUE):
  1. **FORMAT TEST**: `POST /api/v1/games/launch` with pod_id="pod-1", "pod_1", "POD_1", "Pod-1" all resolve to the same billing session and game tracker -- run 4 curl requests, all return same session_id
  2. **GREP CLEAN**: `grep -rn "billing_alt_id\|replace.*pod.*-.*_\|replace.*pod.*_.*-" crates/` returns ZERO hits -- all alt-format workarounds removed
  3. **UNIT TEST**: `normalize_pod_id()` function has tests for: "pod-1"→canonical, "pod_1"→canonical, "POD_1"→canonical, "Pod-1"→canonical, "pod-99"→canonical, ""→error, "garbage"→error
  4. **CROSS-MAP CONSISTENCY**: Start billing for "pod_1", launch game for "pod-1", check game_state for "POD_1" -- all three operations reference the same in-memory state. Verified by: billing timer lookup succeeds, game tracker lookup succeeds, agent_senders lookup succeeds
  5. **REGRESSION**: All existing `cargo test` pass after normalization changes -- zero test regressions
**Plans**: TBD

### Phase 195: Metrics Foundation
**Goal**: Every game launch, billing event, and crash recovery is recorded in dual storage (SQLite for queries, JSONL for immutable audit) with queryable APIs -- the data backbone that powers dynamic timeouts, intelligence, and debugging
**Depends on**: Phase 194 (metrics must use canonical pod IDs)
**Requirements**: METRICS-01, METRICS-02, METRICS-03, METRICS-04, METRICS-05, METRICS-06, METRICS-07
**Success Criteria** (what must be TRUE):
  1. **LAUNCH RECORDING**: After a game launch on Pod 8, `SELECT * FROM launch_events WHERE pod_id='pod-8' ORDER BY created_at DESC LIMIT 1` returns a row with: pod_id, sim_type, car, track, outcome, error_taxonomy, duration_to_playable_ms, attempt_number -- all fields populated, none NULL
  2. **DUAL WRITE**: Same launch event appears in both SQLite and `launch-events.jsonl` -- line count in JSONL matches row count in SQLite for same time window
  3. **JSONL FALLBACK**: Simulate DB failure (rename DB file), trigger a launch event -- event appears in JSONL file with `"db_fallback": true` flag. Restore DB, next event writes to both. Zero events lost
  4. **BILLING ACCURACY**: After launch + PlayableSignal, `SELECT launch_command_at, playable_signal_at, billing_start_at, (billing_start_at - launch_command_at) as delta_ms FROM billing_events WHERE session_id='X'` returns non-null timestamps with measurable delta -- timing gap is quantified per session
  5. **CRASH RECORDING**: After a game crash + recovery, `SELECT failure_mode, recovery_action, recovery_outcome, recovery_duration_ms FROM recovery_events WHERE pod_id='pod-8' ORDER BY created_at DESC LIMIT 1` returns: taxonomy enum (not "unknown"), action tried, outcome (success/fail), duration in ms
  6. **STATS API**: `GET /api/v1/metrics/launch-stats?pod=pod-8&game=assetto_corsa` returns JSON with: `success_rate` (float 0-1), `avg_time_to_track_ms` (int), `p95_time_to_track_ms` (int), `total_launches` (int), `common_failure_modes` (array of {mode, count}) -- all computed from actual launch_events data
  7. **BILLING API**: `GET /api/v1/metrics/billing-accuracy` returns: `avg_delta_ms`, `max_delta_ms`, `sessions_with_zero_delta` (count), `false_playable_signals` (count) -- computed from billing_events data
  8. **ERROR LOGGING**: `log_game_event()` DB insert failure produces a `tracing::error!` log entry AND writes to JSONL fallback -- grep server logs for "launch_event insert failed" confirms error is visible
**Plans**: TBD

### Phase 196: Game Launcher Structural Rework
**Goal**: The monolithic launch_game() is decomposed into per-game trait implementations with correct billing gates, state machine transitions, and error propagation -- structural bugs fixed before adding resilience features
**Depends on**: Phase 194 (normalized pod IDs used in all billing checks)
**Requirements**: LAUNCH-01, LAUNCH-02, LAUNCH-03, LAUNCH-04, LAUNCH-05, LAUNCH-06, LAUNCH-07, STATE-01, STATE-02, STATE-03, STATE-04, STATE-05, STATE-06
**Success Criteria** (what must be TRUE):
  1. **TRAIT ARCHITECTURE**: `grep -rn "impl GameLauncher for" crates/racecontrol/` shows AcLauncher, F1Launcher, IRacingLauncher -- three separate implementations. Each has `launch()`, `validate_args()`, `cleanup()` methods
  2. **BILLING GATE — DEFERRED**: Start a deferred billing session (waiting_for_game), then call launch_game() -- launch SUCCEEDS (currently fails because gate only checks active_timers). Verify: game reaches Launching state
  3. **BILLING GATE — PAUSED**: Pause an active billing session, then call launch_game() -- launch REJECTED with error "billing session is paused". Verify: HTTP 400 response with clear message
  4. **BILLING GATE — TOCTOU**: End billing session during launch validation (simulated timing) -- launch fails cleanly with "billing session expired" instead of proceeding with orphaned game
  5. **DOUBLE LAUNCH — STOPPING**: Set game state to Stopping, then send launch request -- REJECTED with "game still stopping on pod". Verify: no new tracker created
  6. **STOPPING TIMEOUT**: Set game state to Stopping, wait 30 seconds without agent confirmation -- state auto-transitions to Error. Verify: `game_state == Error` after 30s, dashboard broadcast sent
  7. **DISCONNECTED AGENT**: Disconnect agent for pod-8, send launch request -- tracker transitions to Error IMMEDIATELY (not after 120s). Dashboard shows Error state within 1 second
  8. **FEATURE FLAG BLOCK**: Disable game_launch flag on pod-8, send launch request -- server receives explicit `GameStateUpdate { state: Error, message: "game_launch feature disabled" }`. Tracker state is Error, not stuck in Launching
  9. **INVALID JSON**: Send launch_args with malformed JSON `{"corrupt` -- launch REJECTED with parse error. Content validation NOT bypassed. Verify: HTTP 400, no game tracker created
  10. **BROADCAST RELIABILITY**: dashboard_tx.send() failure logged at warn level -- `grep "dashboard broadcast failed" server.log` shows warning when channel is full. Event not silently dropped
  11. **EXTERNALLY TRACKED**: Restart server while game is running on pod -- agent reports Running, tracker created with `externally_tracked=true` and `launch_args=None`. Auto-relaunch knows it cannot retry this game
**Plans**: TBD

### Phase 197: Launch Resilience & AC Hardening
**Goal**: Game launches are resilient with dynamic timeouts tuned from historical data, pre-launch health checks, structured error taxonomy, auto-retry with clean state reset, and AC-specific reliability improvements -- launch failures recover automatically in under 60 seconds
**Depends on**: Phase 195 (dynamic timeouts query launch_events), Phase 196 (per-game launchers provide clean extension points)
**Requirements**: LAUNCH-08, LAUNCH-09, LAUNCH-10, LAUNCH-11, LAUNCH-12, LAUNCH-13, LAUNCH-14, LAUNCH-15, LAUNCH-16, LAUNCH-17, LAUNCH-18, LAUNCH-19, AC-01, AC-02, AC-03, AC-04
**Success Criteria** (what must be TRUE):
  1. **DYNAMIC TIMEOUT**: Insert 10 launch_events for AC/ks_ferrari_sf15t/spa/pod-8 with avg 25s duration. Next launch timeout = median(25s) + 2*stdev ≈ 35-40s, NOT hardcoded 120s. Verify: `grep "dynamic timeout" server.log` shows computed value
  2. **DEFAULT TIMEOUT**: First-ever launch on a new combo (no history) uses 120s for AC, 90s for F1/iRacing. Verify: `grep "default timeout" server.log` shows game-specific default
  3. **PRE-LAUNCH HEALTH**: Before AC launch, pre-flight checks verify: no orphan acs.exe (tasklist), disk > 1GB (wmic), no MAINTENANCE_MODE sentinel, no OTA_DEPLOYING sentinel. Any failure → launch rejected with specific error. Verify: create MAINTENANCE_MODE file, attempt launch → rejected with "MAINTENANCE_MODE active"
  4. **CLEAN STATE RESET**: Kill acs.exe mid-launch, trigger crash detection → system kills ALL 13 game exe names, deletes game.pid, clears shared memory adapter. Verify: `tasklist /FI "IMAGENAME eq acs.exe"` returns empty, game.pid absent, launch state reset to Idle
  5. **AUTO-RETRY**: After clean state reset, auto-retry fires with same launch_args (same car/track/session). Verify: server log shows "Race Engineer: relaunching AC on pod-8 (attempt 1/2)" with matching launch_args JSON hash
  6. **ERROR TAXONOMY**: Game crashes with exit code 0xC0000005 (access violation) → error classified as `ProcessCrash(3221225477)`, not "unknown". Verify: `SELECT error_taxonomy FROM launch_events ORDER BY created_at DESC LIMIT 1` returns "ProcessCrash" with exit code
  7. **NO MAINTENANCE_MODE**: Crash game 5 times rapidly on pod-8 → MAINTENANCE_MODE sentinel NOT created by game launcher. Verify: `test -f C:\RacingPoint\MAINTENANCE_MODE` returns false. Launch crash counter is separate from pod health counter
  8. **STAFF ALERT**: After 2 failed auto-retries → WhatsApp sent to Uday with: pod number, game, car, track, error taxonomy, exit codes, "suggested action: try different car" (if history shows this car fails often). Verify: WhatsApp message received with structured content
  9. **RELAUNCH FIX — NULL ARGS**: Tracker with launch_args=None (externally tracked) → manual relaunch rejected with "original launch args unavailable, please relaunch from kiosk". Not silent failure with empty args
  10. **RACE ENGINEER ATOMIC**: Rapid duplicate errors on pod-8 (2 Error events in <100ms) → counter increments ONCE atomically (single write lock). Only 1 relaunch spawned, not 2. Verify: server log shows exactly "attempt 1/2", not two "attempt 1/2" entries
  11. **TIMEOUT → RELAUNCH**: Game stuck in Launching for dynamic_timeout seconds → timeout fires → Race Engineer auto-relaunch triggered (not just Error state with no recovery). Verify: server log shows timeout THEN relaunch attempt
  12. **AC POLLING WAITS**: AC launch post-kill wait polls for acs.exe absence (max 5s) instead of hardcoded 2s sleep. AC load wait polls for AC window handle (max 30s) instead of 8s sleep. Verify: `grep "sleep\|Sleep" ac_launcher.rs` returns zero hits for the old hardcoded values
  13. **CM TIMEOUT**: Content Manager timeout increased to 30s with 5s progress logging. Verify: server log shows "CM progress: checking acs.exe..." at 5s, 10s, 15s, 20s, 25s intervals
  14. **CM FRESH PID**: After CM failure → direct acs.exe fallback → `find_game_pid()` called for fresh PID. Verify: tracker PID matches actual acs.exe PID from tasklist, not stale CM PID
  15. **STOP LOGGING**: `stop_game()` logs sim_type (not empty string ""). Verify: `SELECT sim_type FROM game_launch_events WHERE event_type='stopping'` returns non-empty value
  16. **ARG PARSING**: Launch args with spaces in path (`C:\Program Files\Steam\steamapps\common\F1 25\F1_25.exe`) handled correctly. Verify: game launches, no "file not found" from split_whitespace bug
**Plans**: TBD

### Phase 198: On-Track Billing
**Goal**: Billing starts only when the customer car is on-track and controllable, pauses on crash, resumes on successful relaunch -- customers are never charged for loading screens, shader compilation, or crashed games
**Depends on**: Phase 196 (correct billing gates), Phase 197 (launch resilience provides clean state for billing transitions)
**Requirements**: BILL-01, BILL-02, BILL-03, BILL-04, BILL-05, BILL-06, BILL-07, BILL-08, BILL-09, BILL-10, BILL-11, BILL-12
**Success Criteria** (what must be TRUE):
  1. **AC ON-TRACK**: Launch AC on Pod 8 → during shader compilation (status=LOADING), query billing timer → `driving_seconds == 0`, status shows "WaitingForGame". After car reaches track (status=LIVE + speedKmh > 0) → `driving_seconds` starts incrementing. Delta between launch and billing start = actual load time, NOT zero
  2. **AC FALSE LIVE**: AC reports AcStatus::Live but speed stays 0 and no steering input for 5s (stuck in replay/menu) → billing does NOT start. PlayableSignal requires Live + (speed > 0 OR steerAngle != 0) within 5s window
  3. **F1 25 ON-TRACK**: Launch F1 25 → billing does NOT start during menu/loading. After UDP telemetry on port 20777 shows m_sessionType > 0 AND m_speed > 0 → billing starts. Verify: `billing_events` shows `playable_signal_at` timestamp AFTER `launch_command_at` with measurable delta
  4. **iRACING ON-TRACK**: Launch iRacing → billing starts when shared memory IsOnTrack=true AND IsOnTrackCar=true. Verify: billing delta matches actual load time to track
  5. **KIOSK LOADING STATE**: During game load, kiosk WebSocket receives `BillingSessionStatus::WaitingForGame` → kiosk timer displays "Loading..." (not countdown). After PlayableSignal → status changes to Active → countdown begins. Verify: kiosk WS message sequence shows WaitingForGame → Active transition
  6. **FAILED PLAYABLE**: Launch game, kill it before PlayableSignal fires → billing NEVER starts. `SELECT * FROM billing_sessions WHERE pod_id='pod-8' ORDER BY created_at DESC LIMIT 1` shows status='cancelled_no_playable'. Staff alert sent. Customer charged ₹0
  7. **CRASH PAUSE/RESUME**: Game running + billing active → crash game → billing status changes to PausedGamePause immediately (same tick). Relaunch game → PlayableSignal fires → billing resumes. Verify: `total_paused_seconds` in billing_sessions shows exact crash recovery duration. Customer NOT charged for recovery time
  8. **90S FALLBACK GUARD**: For EVO/WRC/Forza (no telemetry), if game CRASHES before 90s → false Live NOT emitted. Verify: `game.is_running()` returns false → Error emitted instead of Live. Billing NOT started for crashed game
  9. **AC TIMER SYNC**: Dynamic threshold from historical launch data replaces hardcoded 120s. DB UPDATE uses single Utc::now() call (not two separate calls). billing_alt_id removed (uses canonical pod ID). Failed DB UPDATE logged at error level. Verify: server log shows "AC timer sync: threshold=Xs from historical data"
  10. **MULTIPLAYER SILENT DOWNGRADE**: group_session_members DB query fails → billing start REJECTED with logged error, NOT silently treated as single-player. Verify: `grep "group_session_members query failed" server.log` shows error. No billing session created
  11. **ORPHAN CLEANUP**: Multiplayer 60s timeout evicts non-connected pods → their WaitingForGameEntry is REMOVED from map. If pod comes online at T+61 → it does NOT start billing as accidental solo session. Verify: `waiting_for_game.len()` decreases after timeout cleanup
  12. **CONFIGURABLE TIMEOUTS**: All billing timeouts in racecontrol.toml: `multiplayer_wait_timeout_secs = 60`, `pause_auto_end_timeout_secs = 600`, `launch_timeout_per_attempt_secs = 180`, `idle_drift_threshold_secs = 300`, `offline_grace_secs = 300`. Change multiplayer_wait to 90 → restart server → multiplayer wait is 90s. Verify: `grep "multiplayer timeout" server.log` shows 90s
**Plans**: TBD

### Phase 199: Crash Recovery
**Goal**: When a game crashes during launch or mid-session, the system performs a full clean-slate reset and relaunches within 60 seconds total, with recovery actions informed by historical success data -- the customer session continues with minimal interruption
**Depends on**: Phase 197 (clean state reset logic), Phase 198 (billing pause/resume on crash)
**Requirements**: RECOVER-01, RECOVER-02, RECOVER-03, RECOVER-04, RECOVER-05, RECOVER-06, RECOVER-07
**Success Criteria** (what must be TRUE):
  1. **CLEANUP SPEED**: Crash AC on Pod 8 → measure time from crash detection to "all processes killed + PID cleared + state reset to Idle" → must be <10 seconds. Verify: `grep "clean state reset complete" agent.log` shows timestamp within 10s of crash
  2. **RELAUNCH SPEED**: Full cycle: crash → cleanup → relaunch → game process spawned → must be <60 seconds total. Verify: `SELECT duration_ms FROM recovery_events ORDER BY created_at DESC LIMIT 1` returns <60000
  3. **PRESERVED ARGS**: Crash AC mid-session → auto-relaunch uses SAME car, track, session_type, difficulty, AI count. Verify: compare launch_args JSON hash from original launch and relaunch attempt — must match
  4. **NULL ARGS GUARD**: Tracker with launch_args=None → crash → auto-relaunch SKIPPED with log "cannot auto-relaunch: no launch_args". Staff alerted. Verify: no LaunchGame message sent, dashboard shows "Manual relaunch required"
  5. **HISTORY-INFORMED**: Insert 20 recovery_events for AC/pod-8: "kill+clean+relaunch" succeeds 18/20, "clean_shader_cache+relaunch" succeeds 2/20. Next crash on same combo → system chooses "kill+clean+relaunch" as Tier 1. Verify: `grep "recovery action selected" server.log` shows "kill_clean_relaunch (90% historical success)"
  6. **BILLING PAUSE NOTIFICATION**: After 2 failed auto-retries → billing paused → DashboardEvent::BillingPaused broadcast + WhatsApp alert with: pod, game, error taxonomy, 2 exit codes, suggested alternative combo. Verify: kiosk shows "Session paused — staff notified". WhatsApp received with structured message
  7. **EXIT GRACE GUARD**: Crash game during recovery (attempt 1 running) → exit grace timer (30s) NOT armed because crash_recovery != Idle. Verify: `grep "exit grace armed" agent.log` returns ZERO hits during recovery window. No premature AcStatus::Off
  8. **SAFE MODE PERSISTENCE**: Crash protected game (AC) → safe mode stays ACTIVE throughout recovery (attempt 1 + attempt 2). Verify: `grep "safe mode deactivated" agent.log` returns ZERO hits between crash and recovery completion. Process guard scans suppressed during entire recovery
  9. **CONCURRENT PROTECTION**: Two rapid crashes on same pod (<100ms apart) → only ONE recovery sequence initiated. Counter increments once. Verify: server log shows exactly one "Race Engineer: relaunching" entry, not two
**Plans**: TBD

### Phase 200: Self-Improving Intelligence
**Goal**: The system uses accumulated launch data to warn about unreliable combos, suggest alternatives, and display reliability insights to staff -- every launch makes the system smarter without manual threshold tuning
**Depends on**: Phase 195 (metrics data), Phase 197 (dynamic timeouts consume intelligence), Phase 199 (recovery actions consume intelligence)
**Requirements**: INTEL-01, INTEL-02, INTEL-03, INTEL-04, INTEL-05
**Success Criteria** (what must be TRUE):
  1. **RELIABILITY TABLE**: `SELECT * FROM combo_reliability WHERE game='assetto_corsa' AND pod='pod-8'` shows rows with: combo_hash, success_rate (0.0-1.0), avg_time_to_track_ms, p95_time_to_track_ms, total_launches, common_failure_modes (JSON), last_updated. Updated after every launch (check last_updated matches latest launch timestamp)
  2. **LOW SUCCESS WARNING**: Insert 10 launch_events for AC/ks_ferrari_sf15t/nurburgring/pod-5 with 4 successes (40% rate). Call `POST /api/v1/games/launch` for same combo → response includes `"warning": "This combination has a 40% success rate on this pod (4/10 launches)"`. Verify: warning field present in JSON response
  3. **NO WARNING FOR GOOD COMBOS**: Same launch but for AC/ks_ferrari_sf15t/spa/pod-8 with 95% success rate → response has NO warning field. Verify: warning field absent or null
  4. **MINIMUM LAUNCHES**: Combo with only 3 launches (below 5 minimum) → no warning regardless of success rate. Verify: insufficient data, system uses defaults
  5. **ALTERNATIVES API**: `GET /api/v1/games/alternatives?game=assetto_corsa&car=ks_ferrari_sf15t&track=nurburgring&pod=pod-5` returns JSON array of top 3 alternatives with: `{car, track, success_rate, avg_time_ms, total_launches}` sorted by success_rate DESC, all >90%. Verify: each alternative has success_rate > 0.90
  6. **ALTERNATIVES SIMILARITY**: Alternatives prefer same-track-different-car OR same-car-different-track over random combos. Verify: at least 1 of top 3 shares either the same car or same track as the request
  7. **ADMIN MATRIX**: `GET /api/v1/admin/launch-matrix?game=assetto_corsa` returns per-pod rows with: pod_id, total_launches, success_rate, avg_time_ms, top_3_failure_modes, flagged (boolean: success_rate < 0.70). Verify: pods with <70% are flagged=true
  8. **ROLLING WINDOW**: Insert old launch_events (45 days ago) with 100% success, recent events (last 7 days) with 50% success → combo_reliability reflects recent 30-day window (50%), not all-time (75%). Verify: success_rate matches 30-day window calculation
  9. **AUTO-TUNING PROOF**: After 20 new launches, dynamic timeout for the combo changes from default 120s to historical-derived value. Pre-launch health check adapts (if 80% of failures on this pod are disk-related, disk check runs first). Verify: no manual config changes needed, system computes from data
**Plans**: TBD

## v24.0 Progress

**Execution Order:** 194 -> 195 -> 196 -> 197 -> 198 -> 199 -> 200

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 194. Pod ID Normalization | 0/0 | Not started | - |
| 195. Metrics Foundation | 0/0 | Not started | - |
| 196. Game Launcher Structural Rework | 0/0 | Not started | - |
| 197. Launch Resilience & AC Hardening | 0/0 | Not started | - |
| 198. On-Track Billing | 0/0 | Not started | - |
| 199. Crash Recovery | 0/0 | Not started | - |
| 200. Self-Improving Intelligence | 0/0 | Not started | - |
