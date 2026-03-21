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

## Current Milestone

### v6.0 Salt Fleet Management (Phases 36–40)

**Milestone Goal:** Replace the custom pod-agent/remote_ops HTTP endpoint (port 8090) with SaltStack — salt-master on WSL2 James (.27), salt-minion on all 8 pods + server (.23), salt_exec.rs as the server-side integration seam, remote_ops.rs deleted from rc-agent, and deploy workflow fully migrated to Salt.

### v7.0 E2E Test Suite (Phases 41–44)

**Milestone Goal:** Comprehensive end-to-end test coverage for the full kiosk→server→agent→game launch pipeline — Playwright browser tests for all 5 sim wizard flows, curl-based API pipeline tests for billing/launch/game-state lifecycle, deploy verification for binary swap and port conflict detection, and a single master `run-all.sh` entry point reusable for future services (POS, Admin Dashboard).

### v8.0 RC Bot Autonomy (Phases 45–49)

**Milestone Goal:** Raise rc-agent autonomy from 6/10 to 8/10 — fix the CLOSE_WAIT socket leak causing 5/8 pods to self-relaunch every 5 minutes, install panic hooks for FFB safety on crash, deploy local Ollama (qwen3:0.6b + rp-debug model) to all 8 pods so AI diagnosis is instant and offline-capable, add dynamic server-fetched kiosk allowlist to eliminate the #1 manual intervention, auto-end orphaned billing sessions, and auto-reset pods after session end.

### v9.0 Tooling & Automation (Phases 51–56)

**Milestone Goal:** Install the tooling layer that makes James+Claude more effective — CLAUDE.md project context + 5 custom skills so Claude always knows pod IPs and naming conventions, MCP servers for Google Workspace (Gmail/Sheets/Calendar) and racecontrol REST API access, deployment automation so staging auto-starts and every deploy runs a verified canary-first flow, structured JSON logs in racecontrol and rc-agent with error-rate email alerts, Netdata fleet monitoring on server and all 8 pods, WhatsApp P0 alerts to Uday, and a weekly fleet uptime report.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [ ] **Phase 36: WSL2 Infrastructure** - WSL2 Ubuntu 24.04 with mirrored networking, salt-master 3008 LTS, salt-api, and Hyper-V firewall rules running on James (.27) and verified reachable from the pod subnet
- [ ] **Phase 37: Pod 8 Minion Bootstrap** - Salt minion 3008 installed on Pod 8 canary with explicit minion ID, Defender exclusions pre-applied, sc failure recovery configured, key accepted, and install.bat rewritten without pod-agent sections
- [ ] **Phase 38: salt_exec.rs + Server Module Migration** - New salt_exec.rs Rust module wrapping salt-api REST calls, all four server-side modules (deploy.rs, fleet_health.rs, pod_monitor.rs, pod_healer.rs) migrated from pod-agent HTTP to Salt
- [ ] **Phase 39: remote_ops.rs Removal** - Characterization tests written covering the WebSocket path, remote_ops.rs deleted from rc-agent, all port 8090 references purged from Rust source and deploy scripts, cargo build clean, Pod 8 canary billing lifecycle verified
- [ ] **Phase 40: Fleet Rollout** - Salt minion deployed to all 8 pods + server via updated install.bat, all keys accepted, salt '*' test.ping returns 9 True, deploy workflow fully migrated to Salt
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
- [ ] **Phase 62: Fleet Config Distribution** - Push configs to all 8 pods via rc-agent WebSocket, atomic writes (temp+rename), Global.json to both paths, graceful CL stop/write/restart/verify cycle
- [ ] **Phase 63: Fleet Monitoring** - rc-agent reports active preset, config hashes, firmware version per pod; racecontrol dashboard shows fleet config status at a glance
- [ ] **Phase 64: Telemetry Dashboards** - Enable wheel LCD showing RPM/speed/gear for all 4 venue games, verify GameSettingCenter.json telemetry fields, document UDP port chain
- [ ] **Phase 65: Shift Lights & RGB Lighting** - Auto RPM shift lights for AC/ACC, manual RPM thresholds for F1 25/AC Rally, RGB button lighting tied to telemetry (DRS, ABS, TC, flags)
## v10.0 Connectivity & Redundancy

Make server .23 IP permanently stable, establish reliable James↔Server↔Bono remote exec paths, sync venue config to cloud, and deliver automatic pod failover to Bono's VPS when .23 goes down — with self-healing failback when .23 recovers.

- [x] **Phase 67: Config Sync** - racecontrol.toml changes detected by SHA-256 hash, sanitized (credentials/paths stripped), and pushed to Bono via comms-link sync_push; Bono applies TOML-based config (venue/pods/branding) to cloud racecontrol. Billing rates and game catalog already synced via DB-level cloud_sync. (completed 2026-03-20)
 (completed 2026-03-20)
- [x] **Phase 67: Config Sync** - racecontrol.toml changes detected by sha2 hash, sanitized, and pushed to Bono via comms-link sync_push; Bono applies config to cloud racecontrol (completed 2026-03-20)
- [x] **Phase 68: Pod SwitchController** - rc-agent CoreConfig gains failover_url; WS reconnect loop uses Arc<RwLock<String>> for runtime URL switching; SwitchController AgentMessage triggers switch without restart; self_monitor suppression guard prevents relaunch during intentional failover (completed 2026-03-20)
- [x] **Phase 69: Health Monitor & Failover Orchestration** - James probes .23 every 5s; 3-down/2-up hysteresis + 60s minimum outage window gates auto-failover; James sends task_request to Bono to activate cloud primary; racecontrol broadcasts SwitchController to all pods; pods confirm .23 unreachable before switching; Uday notified via email + WhatsApp (completed 2026-03-21)
- [ ] **Phase 70: Failback & Data Reconciliation** - James detects .23 recovery (2-up threshold); cloud sessions merged to local DB before .23 resumes primary; racecontrol broadcasts SwitchController with original URL; Uday notified on failback

## v11.0 Agent & Sentry Hardening

Harden rc-sentry into a reliable fallback operations tool, extract shared exec patterns into rc-common, write characterization tests before decomposing rc-agent main.rs, and add unit/integration tests for critical safety paths.

- [x] **Phase 71: rc-common Foundation + rc-sentry Core Hardening** - rc-common exec.rs feature-gated (sync/async), rc-sentry timeout enforcement, output truncation, concurrency cap, partial TCP read fix, structured logging (completed 2026-03-20)
- [x] **Phase 72: rc-sentry Endpoint Expansion + Integration Tests** - /health, /version, /files, /processes endpoints; endpoint integration tests for all routes
 (completed 2026-03-20)
- [x] **Phase 73: Critical Business Tests** - billing_guard and failure_monitor unit tests before any refactoring; FfbBackend trait seam for FFB controller tests (completed 2026-03-20)
- [ ] **Phase 74: rc-agent Decomposition** - config.rs, app_state.rs, ws_handler.rs, event_loop.rs extracted from main.rs in strict risk order

## v12.0 Operations Security

Lock down the Racing Point operations stack — audit all exposed endpoints and PII, enforce JWT auth on billing/session APIs, add admin PIN gate, HTTPS for browser traffic, harden kiosk escape vectors, encrypt customer PII at rest, and add audit trails for compliance. Gradual hardening: biggest holes first, then layer defenses.

- [x] **Phase 75: Security Audit & Foundations** - Inventory all exposed endpoints, trace PII locations, move secrets to env vars, auto-generate JWT key (completed 2026-03-20)
- [x] **Phase 76: API Authentication & Admin Protection** - JWT enforcement on all sensitive routes, admin PIN gate with argon2, rate limiting, bot auth, pod HMAC, session integrity (completed 2026-03-20)
- [x] **Phase 77: Transport Security** - HTTPS for PWA/admin browser traffic, self-signed LAN certs, Let's Encrypt for cloud, security response headers (completed 2026-03-20)
- [x] **Phase 78: Kiosk & Session Hardening** - Chrome lockdown, hotkey blocking, USB disable, session-scoped tokens, anomaly auto-pause with WhatsApp alert (completed 2026-03-21)
- [ ] **Phase 79: Data Protection** - AES-256-GCM on PII columns, deterministic phone hash for lookups, log redaction, customer data export/deletion
- [ ] **Phase 80: Audit Trail & Defense in Depth** - Admin action logging, WhatsApp alerts on sensitive actions, PIN rotation alerts, cloud sync HMAC signing

## v13.0 Multi-Game Launcher

Launch games other than AC (F1 25, iRacing, AC EVO, EA WRC, LMU) from kiosk/PWA with PlayableSignal-gated billing, per-game telemetry capture, and multi-game leaderboard integration. Extends existing SimAdapter trait and GameProcess — zero new crate dependencies.

- [ ] **Phase 81: Game Launch Core** - Launch profiles, process monitoring, kiosk integration, crash recovery for 5 games
- [ ] **Phase 82: Billing and Session Lifecycle** - PlayableSignal per game, billing accuracy, per-game rates, clean lifecycle
- [ ] **Phase 83: F1 25 Telemetry** - Extend existing F1 25 UDP adapter for LapCompleted events with sector splits
- [ ] **Phase 84: iRacing Telemetry** - Shared memory reader with session transition handling and pre-flight checks
- [ ] **Phase 85: LMU Telemetry** - rFactor 2 shared memory reader for Le Mans Ultimate lap data
- [ ] **Phase 86: AC EVO Telemetry** - Best-effort shared memory reader using ACC struct layout, feature-flagged
- [ ] **Phase 87: EA WRC Telemetry** - JSON-configured UDP telemetry with stage-to-lap mapping
- [ ] **Phase 88: Leaderboard Integration** - Multi-game lap storage, track name normalization, endpoint updates

## Phase Details

### Phase 36: WSL2 Infrastructure
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
**Plans**: 3 plans

Plans:
- [ ] 59-01: TBD

### Phase 60: Pre-Launch Profile Loading
**Goal**: rc-agent ensures the correct preset is loaded BEFORE the game starts, with a safe fallback if the game is unrecognized
**Depends on**: Phase 59
**Requirements**: PROF-03, PROF-05
**Success Criteria** (what must be TRUE):
  1. rc-agent loads the correct game preset before launching the game process (not relying solely on ConspitLink auto-detect)
  2. If an unrecognized game launches, a safe default preset is applied (conservative force, centered spring)
**Plans**: 3 plans

Plans:
- [ ] 60-01: TBD

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
**Plans**: TBD

Plans:
- [ ] 70-01: TBD


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
**Plans**: TBD

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
**Plans**: TBD

Plans:
- [ ] 79-01: TBD
- [ ] 79-02: TBD
- [ ] 79-03: TBD

### Phase 80: Audit Trail & Defense in Depth
**Goal**: Every sensitive admin action is logged and alertable, with remaining security gaps closed
**Depends on**: Phase 76
**Requirements**: ADMIN-04, ADMIN-05, ADMIN-06, AUTH-07
**Success Criteria** (what must be TRUE):
  1. Every wallet topup, pricing change, session override, fleet exec, and terminal command is recorded in an append-only audit_log table with timestamp, actor, and action details
  2. Admin login and sensitive actions (wallet topup, fleet exec) trigger a WhatsApp notification to Uday
  3. If the admin PIN has not been changed in 30+ days, Uday receives an alert prompting rotation
  4. Cloud sync payloads are signed with HMAC-SHA256 including timestamp and nonce -- replayed or tampered payloads are rejected
**Plans**: TBD

Plans:
- [ ] 80-01: TBD
- [ ] 80-02: TBD

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
**Plans**: TBD

Plans:
- [ ] 82-01: TBD
- [ ] 82-02: TBD

### Phase 83: F1 25 Telemetry
**Goal**: F1 25 lap times and sector splits are captured and emitted as structured events
**Depends on**: Phase 82
**Requirements**: TEL-F1-01, TEL-F1-02, TEL-F1-03
**Success Criteria** (what must be TRUE):
  1. F1 25 UDP telemetry is received on port 20777 during gameplay
  2. Lap times and sector splits are extracted from telemetry packets after each completed lap
  3. Each completed lap emits an AgentMessage::LapCompleted with sim_type = F1_25
**Plans**: TBD

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
**Plans**: TBD

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
**Plans**: TBD

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
**Plans**: TBD

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
**Plans**: TBD

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
**Plans**: TBD

Plans:
- [ ] 88-01: TBD
- [ ] 88-02: TBD

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
| 59. Auto-Switch Configuration | v10.0 | 0/? | Not started | - |
| 60. Pre-Launch Profile Loading | v10.0 | 0/? | Not started | - |
| 61. FFB Preset Tuning | v10.0 | 0/? | Not started | - |
| 62. Fleet Config Distribution | v10.0 | 0/? | Not started | - |
| 63. Fleet Monitoring | v10.0 | 0/? | Not started | - |
| 64. Telemetry Dashboards | v10.0 | 0/? | Not started | - |
| 65. Shift Lights & RGB Lighting | v10.0 | 0/? | Not started | - |
| 66. Infrastructure Foundations | 5/5 | Complete    | 2026-03-20 | - |
| 67. Config Sync | 2/2 | Complete    | 2026-03-20 | - |
| 68. Pod SwitchController | 2/2 | Complete    | 2026-03-20 | - |
| 69. Health Monitor & Failover Orchestration | 3/3 | Complete   | 2026-03-21 | - |
| 70. Failback & Data Reconciliation | v10.0-CR | 0/? | Not started | - |
| 71. rc-common Foundation + rc-sentry Core Hardening | 2/2 | Complete    | 2026-03-20 | - |
| 72. rc-sentry Endpoint Expansion + Integration Tests | 2/2 | Complete    | 2026-03-20 | - |
| 73. Critical Business Tests | 2/2 | Complete    | 2026-03-20 | - |
| 74. rc-agent Decomposition | 1/4 | In Progress|  | - |
| 75. Security Audit & Foundations | 2/2 | Complete    | 2026-03-20 | - |
| 76. API Authentication & Admin Protection | 6/6 | Complete    | 2026-03-20 | - |
| 77. Transport Security | 2/2 | Complete    | 2026-03-20 | - |
| 78. Kiosk & Session Hardening | 3/3 | Complete    | 2026-03-21 | - |
| 79. Data Protection | v12.0 | 0/? | Not started | - |
| 80. Audit Trail & Defense in Depth | v12.0 | 0/? | Not started | - |
| 81. Game Launch Core | v13.0 | 0/3 | Not started | - |
| 82. Billing and Session Lifecycle | v13.0 | 0/2 | Not started | - |
| 83. F1 25 Telemetry | v13.0 | 0/1 | Not started | - |
| 84. iRacing Telemetry | v13.0 | 0/2 | Not started | - |
| 85. LMU Telemetry | v13.0 | 0/1 | Not started | - |
| 86. AC EVO Telemetry | v13.0 | 0/1 | Not started | - |
| 87. EA WRC Telemetry | v13.0 | 0/1 | Not started | - |
| 88. Leaderboard Integration | v13.0 | 0/2 | Not started | - |
