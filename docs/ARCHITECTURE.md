# Racing Point RaceControl — System Architecture

## Overview

RaceControl is a Rust/Axum + Next.js monorepo powering Racing Point eSports venue operations: 8 sim racing pods, billing, game management, fleet orchestration, AI diagnostics, and cloud sync.

### Table of Contents

| # | Section | What It Covers |
|---|---------|----------------|
| 1 | [Workspace Crates](#1-workspace-crates) | 10 crates, dependency graph, key dependencies |
| 2 | [Deployment Topology](#2-deployment-topology) | Server, pods, POS, James, Bono VPS — what runs where |
| 3 | [Frontend Apps](#3-frontend-apps) | Web, Kiosk, PWA, Admin — Next.js apps |
| 4 | [Data Architecture](#4-data-architecture) | SQLite WAL, cloud sync dual authority |
| 5 | [WebSocket Architecture](#5-websocket-architecture) | Agent, dashboard, mesh WS channels + protocol types |
| 6 | [Shared State (AppState)](#6-shared-state-appstate) | Server in-memory state structure |
| 7 | [Major Functional Modules](#7-major-functional-modules) | Server, agent, AC plugin module tables |
| 8 | [Authentication & Authorization](#8-authentication--authorization) | JWT roles, middleware stack, rotation |
| 9 | [Recovery & Self-Healing](#9-recovery--self-healing-architecture) | 5-tier diagnosis, recovery systems, sentinels |
| 10 | [Build & Deploy](#10-build--deploy) | Build commands, deploy order, critical rules |
| 11 | [Key Architectural Patterns](#11-key-architectural-patterns) | 15 patterns: broadcast, locking, sentinels, boot resilience |
| 12 | [Network Map](#12-network-map) | All device IPs (LAN + Tailscale) |
| 13 | [Meshed Intelligence (MI)](#13-meshed-intelligence-mi) | 5-tier autonomous AI diagnosis, OpenRouter, budget, gossip |
| 14 | [Unified MMA Protocol](#14-unified-mma-protocol) | 4-step convergence engine, model pool, audit workflow |
| 15 | [Config Management & Policy Engine](#15-config-management--policy-engine) | AgentConfig push, game presets, policy rules |
| 16 | [Billing Architecture](#16-billing-architecture) | FSM, per-minute, multiplayer, refund, crash recovery |
| 17 | [Game Launch Architecture](#17-game-launch-architecture) | Launch chain, WaitingForLive, multi-sim, crash recovery |
| 18 | [Fleet Operations](#18-fleet-operations) | Deploy pipeline, pod healer, fleet healer, WoL |
| 19 | [Customer Journey (Acts 1-4)](#19-customer-journey-acts-1-4) | Registration → racing → session end → venue ops |
| 20 | [GSD Development Workflow](#20-gsd-development-workflow) | How milestones are planned, executed, and shipped |
| 21 | [Cognitive Gate Protocol (CGP)](#21-cognitive-gate-protocol-cgp) | AI quality enforcement — 5 hard gates + backlog gate |

---

## 1. Workspace Crates

| Crate | Binary | Runs On | Port | Purpose |
|-------|--------|---------|------|---------|
| **racecontrol** | `racecontrol.exe` | Server .23, Bono VPS | 8080 | Core server: REST API, WebSocket, SQLite DB, billing, game state, fleet coordination, cloud sync |
| **rc-agent** | `rc-agent.exe` | Pods 1-8, POS .20 | 8090 | Pod agent: game launcher, WS client, lock screen, FFB, telemetry, process guard, AI self-heal |
| **rc-sentry** | `rc-sentry.exe` | Pods 1-8, POS .20 | 8091 | Pod exec daemon (Session 0): remote commands, restart rc-agent, deploy target |
| **rc-common** | *(lib only)* | All crates | — | Shared types: WS protocol, errors, config, boot resilience helpers |
| **rc-watchdog** | `rc-watchdog.exe` | James .27 | — | Fleet healer: monitors 10 services, crash recovery, Ollama/OpenRouter diagnosis, escalation |
| **rc-sentry-ai** | `rc-sentry-ai.exe` | James .27 | — | Computer vision: YOLOv8 face detection on 3 cameras (ONNX inference) |
| **rc-guardian** | `rc-guardian.exe` | Bono VPS | — | Layer 3 external monitoring: server health polling, failure detection, escalation |
| **rc-process-guard** | `rc-process-guard.exe` | Pods, James | — | Process allowlist enforcement: blocks unauthorized executables |
| **rc-installer** | `rc-installer.exe` | Pods (one-time) | — | Zero-dependency pod setup: user creation, config deployment |
| **weekly-report** | `weekly-report.exe` | Server | — | Scheduled report generation |

### Dependency Graph

```
rc-common (shared lib — no external deps)
    |
    +-- racecontrol (server)
    +-- rc-agent (pod)
    +-- rc-watchdog (healer)
    +-- rc-sentry (exec daemon)
    +-- rc-sentry-ai (vision)
    +-- rc-guardian (monitor)
    +-- rc-process-guard
    +-- weekly-report
```

All crates depend on `rc-common`. No crate depends on another binary crate.

### Key Workspace Dependencies

- **tokio** 1.x (full) — async runtime
- **axum** 0.8 — HTTP/WS framework
- **sqlx** 0.8 (sqlite, tokio) — database
- **serde/serde_json** — serialization
- **tracing/tracing-subscriber** — structured logging (UTC timestamps!)
- **jsonwebtoken** 9 — JWT auth
- **argon2** 0.5 — password hashing
- **aes-gcm** 0.10 — field-level PII encryption

---

## 2. Deployment Topology

### 2.1 Server (.23) — 192.168.31.23

| Service | Port | Start Method |
|---------|------|-------------|
| racecontrol | 8080 | `start-racecontrol.bat` (HKLM Run) + PowerShell watchdog |
| Web Dashboard | 3200 | Scheduled Task (Next.js standalone) |
| Kiosk | 3300 | Scheduled Task (Next.js standalone) |
| Admin Dashboard | 3201 | Scheduled Task (Next.js standalone) |
| SQLite DB | embedded | `./data/racecontrol.db` (WAL mode) |

**Responsibilities:** Central auth (JWT), game state machine, billing engine, pod fleet coordination (WS), cloud sync (30s push/pull), WebSocket server for all clients.

### 2.2 Pods 1-8 — 192.168.31.[89,33,28,88,86,87,38,91]

| Service | Port | Session | Details |
|---------|------|---------|---------|
| rc-agent | 8090 | Session 1 (interactive) | Game launch, WS client, lock screen, FFB, billing client |
| rc-sentry | 8091 | Session 0 (service) | Exec daemon, health polling, deploy target |
| rc-process-guard | embedded | — | Allowlist enforcement (fetched from server every 5min) |
| Lock Screen HTTP | 18923 | — | Local lock screen/timer page served to Edge |
| Debug endpoint | 18924 | — | `/debug` — edge_process_count, lock_screen_state |

**CRITICAL:** rc-agent MUST run in Session 1 (interactive desktop). Session 0 cannot launch games, Edge, overlays, or any GUI. RCWatchdog service uses `WTSQueryUserToken + CreateProcessAsUser` for Session 1 restarts.

### 2.3 POS PC (.20) — 192.168.31.20

Same binaries as pods (single-binary-tier policy). No game launching, no FFB. Focuses on billing operations via web dashboard (:3200/billing).

### 2.4 James Workstation (.27) — 192.168.31.27

| Service | Port | Purpose |
|---------|------|---------|
| go2rtc | 1984 | 13x Dahua RTSP cameras, API |
| comms-link relay | 8766 | James-side of James-Bono WS tunnel |
| Ollama | 11434 | Local LLM: qwen2.5:3b, llama3.1:8b |
| rc-sentry-ai | — | Face detection (YOLOv8) on 3 cameras |
| rc-watchdog | — | Fleet healer: monitors 10 services, auto-diagnose, escalate |
| webterm | 9999 | Remote terminal for Uday's phone |

### 2.5 Bono VPS (Cloud) — 72.60.101.58

| Service | Port | Purpose |
|---------|------|---------|
| racecontrol (cloud) | 8080 | Cloud-authoritative: drivers, pricing, catalog. PM2-managed |
| comms-link | 8765 | Bono-side of relay tunnel |
| Web Dashboard | 3200 | Remote staff access |
| Kiosk | 3300 | Testing |
| Admin | 3201 | Remote admin |
| Customer PWA | 3500 | PUBLIC: racingpoint.cloud |
| WhatsApp Bot | 3000 | Customer/staff messaging via Evolution API |
| nginx | 80/443 | Reverse proxy, HTTPS termination |

### Quick Reference: What Runs Where

| Service | Server .23 | Pods 1-8 | POS .20 | James .27 | Bono VPS |
|---------|:----------:|:--------:|:-------:|:---------:|:--------:|
| racecontrol (8080) | X | | | | X |
| rc-agent (8090) | | X | X | | |
| rc-sentry (8091) | | X | X | | |
| Web (3200) | X | | | | X |
| Kiosk (3300) | X | | | | X |
| PWA (3500) | | | | | X |
| Admin (3201) | X | | | | X |
| go2rtc (1984) | | | | X | |
| Ollama (11434) | | | | X | |
| rc-watchdog | | | | X | |
| comms-link | | | | X (8766) | X (8765) |

---

## 3. Frontend Apps

| App | Port | basePath | Deployment | Users |
|-----|------|----------|------------|-------|
| Web Dashboard (`web/`) | 3200 | — | Server + Cloud | Staff (operations) |
| Kiosk (`kiosk/`) | 3300 | `/kiosk` | Server + Cloud | Pod displays (Edge) |
| Customer PWA (`pwa/`) | 3500 | — | Cloud ONLY | Customers (racingpoint.cloud) |
| Admin (`racingpoint-admin/`) | 3201 | — | Server + Cloud | Admin staff |

All use Next.js 16, React 19, TypeScript, Tailwind CSS, standalone output mode.

### Shared Packages

- **`packages/shared-types/`** — TypeScript contract types: Pod, BillingSession, Driver, GameState, FeatureFlag, WS payloads
- **`packages/shared-tokens/`** — Design tokens: colors, spacing, fonts (Racing Red `#E10600`, Asphalt Black `#1A1A1A`)
- **`packages/contract-tests/`** — API contract verification (request/response schemas)

---

## 4. Data Architecture

### SQLite (Server)

- **Engine:** SQLite with WAL mode (concurrent readers + single writer)
- **Pool:** `SqlitePool` max 5 connections
- **Pragmas:** `journal_mode=WAL`, `busy_timeout=5000ms`, `synchronous=NORMAL`, `foreign_keys=ON`
- **Core Tables:** drivers, pods, sessions, laps, personal_bests, track_records, billing_sessions, wallets, wallet_transactions, cafe_items, cafe_orders, feature_flags, audit_log, system_events, and 50+ more

### Cloud Sync (30s cycle)

```
Venue racecontrol (LOCAL authority: billing, laps, game state)
       |  push: billing sessions, laps, game events
       v
Bono racecontrol (CLOUD authority: drivers, pricing, catalog)
       |  pull: drivers, pricing, feature flags
       v
Venue racecontrol (merges cloud data into local DB)
```

---

## 5. WebSocket Architecture

### Pod Agent Connection

1. Pod rc-agent connects: `wss://server:8080/ws/agent?token=<psk>&jwt=<pod_jwt>`
2. Server validates PSK (bootstrap) or JWT (steady-state)
3. Agent sends `Register { pod_id, pod_number, version }`
4. Server stores sender in `agent_senders: RwLock<HashMap<pod_id, mpsc::Sender<CoreMessage>>>`
5. Server can push: `LaunchGame`, `StopGame`, `BlankScreen`, `ConfigPush`, etc.

### Dashboard WebSocket (`/ws/dashboard`)

- Browsers connect for real-time updates
- Broadcast channel: `dashboard_tx: broadcast::Sender<DashboardEvent>`
- Events: game state changes, session progress, leaderboard updates
- Churn metric: `connects_per_min > 10` = stale frontend build

### Mesh Intelligence WebSocket (`/ws/ai-channel`)

- Pod agents + server share diagnostic insights via gossip protocol
- Aggregated at server for fleet-wide learning

### Protocol Types (rc-common)

- `AgentMessage::Register` — pod identifies itself
- `CoreMessage::LaunchGame(AcLaunchParams)` — server-to-pod command
- `CoreMessage::StopGame(GameState)` — stop game
- `CoreMessage::BlankScreen` — trigger lock screen
- `CoreMessage::GameStateUpdate` — pod-to-server state change
- `DashboardEvent` — broadcast to all connected dashboards

---

## 6. Shared State (AppState)

```rust
pub struct AppState {
    pub config: Config,                              // racecontrol.toml
    pub db: SqlitePool,                             // SQLite WAL
    pub pods: RwLock<HashMap<String, PodInfo>>,     // 8 pods in-memory
    pub dashboard_tx: broadcast::Sender<DashboardEvent>,
    pub billing: BillingManager,                    // FSM: Idle->Active->Completed->Refunded
    pub game_launcher: GameManager,                 // Game launch sequencer
    pub ac_server: AcServerManager,                 // AC server lifecycle
    pub port_allocator: PortAllocator,              // Ports 5000-5100 for AC servers
    pub agent_senders: RwLock<HashMap<String, mpsc::Sender<CoreMessage>>>,
    pub feature_flags: RwLock<HashMap<String, FeatureFlagRow>>,
    pub field_cipher: FieldCipher,                  // AES-GCM for PII
    pub http_client: reqwest::Client,               // Shared client
    // ... 20+ more fields
}
```

**Pattern:** Routes extract `State<Arc<AppState>>`, access via RwLock/Mutex. Clone/snapshot data, drop lock, then do async work (never hold lock across `.await`).

---

## 7. Major Functional Modules

### Server (racecontrol)

| Module | Purpose |
|--------|---------|
| **billing.rs / billing_fsm.rs** | BillingManager FSM: Idle -> Active -> Completed -> Refunded |
| **billing_replay.rs** | Recover billing state after server crashes |
| **wallet.rs / accounting.rs** | Customer wallets, financial ledger (debit/credit) |
| **game_launcher.rs** | GameManager — route launches to pods via WS |
| **game_doctor.rs** | Diagnose game launch failures |
| **pod_monitor.rs** | Heartbeat polling (:8090/health every 10s) |
| **pod_healer.rs** | 3-tier graduated recovery: Tier 1 restart via sentry → Tier 2 WoL → Tier 3 AI + WhatsApp |
| **fleet_healer.rs** | Correlated failure detection (3+ pods same symptom = fleet-wide pattern), SSH repair dispatch |
| **fleet_kb.rs** | Mesh intelligence KB: fleet_solutions, fleet_experiments, audit_known_issues tables |
| **wol.rs** | Wake-on-LAN: context-aware magic packet with MAINTENANCE_MODE/OTA pre-checks |
| **lobby.rs** | Synchronous multiplayer lobby (LobbyManager, ready-check, 120s timeout) |
| **auth/middleware.rs** | JWT validation, RBAC (customer, cashier, manager, superadmin) |
| **auth/rate_limit.rs** | tower_governor rate limiting per IP |
| **mesh_handler.rs** | Pod gossip aggregation, distributed learning |
| **cloud_sync.rs** | 30s push/pull with Bono VPS |
| **config_push.rs** | Server-pushed config changes to pods via WS |
| **flags.rs** | Feature flags (FF-01+, runtime toggles) |
| **metrics.rs / metrics_tsdb.rs** | Prometheus metrics, time-series storage |
| **whatsapp_alerter.rs** | Alert routing via WhatsApp |
| **remote_terminal.rs** | SSH-like terminal to pods |
| **deployment/ota_pipeline.rs** | Binary deployment & rollback |
| **maintenance_engine.rs** | Problem -> diagnosis -> fix playbook execution |

### Pod Agent (rc-agent)

| Module | Purpose |
|--------|---------|
| **ac_launcher.rs** | VMS SimLauncher clone: zero-block launch via `launch-ac.bat` subprocess. Config writing + bat spawn + immediate return (<1s). Event loop discovers PID. |
| **game_process.rs** | Generic game execution framework |
| **lock_screen.rs** | Windows blanking/kiosk mode via Edge. NVIDIA Surround failure detection. |
| **overlay.rs** | HUD overlay (league table, FFB meter) |
| **driving_detector.rs** | Steering wheel input monitoring (OpenFFBoard) |
| **ffb_controller.rs** | Force feedback tuning (NM adjustment) |
| **diagnostic_engine.rs** | AI-driven self-healing (Tier 1-4) |
| **knowledge_base.rs** | Local SQLite KB for Tier 2 diagnostics |
| **process_guard.rs** | Allowlist enforcement (fetch from server every 5min) |
| **safe_mode.rs** | WMI watcher for anti-cheat safe mode. Tasklist fallback if PowerShell fails. |
| **remote_ops.rs** | HTTP server (:8090, receives commands from server/sentry) |
| **ws_handler.rs** | WebSocket handling + GAME_LAUNCHING sentinel (RAII guard) |
| **csv_lap_fallback.rs** | VMS pattern: saves laps to `C:\RacingPoint\laps-offline.csv` when WS disconnected |
| **off_track_detector.rs** | Debounced isValidLap transition detection (1s on, 0.5s off) |
| **launch_verifier.rs** | 4-stage launch verification: ProcessAlive → SharedMemory → OnTrack |
| **dxgi_capture.rs** | D3D11 desktop screenshot for diagnostics |
| **self_heal.rs** | Tier 1 deterministic fixes (kill orphans, clear temp) |
| **inactivity_monitor.rs** | Detect idle sessions (5min -> auto-stop) |
| **session_enforcer.rs** | Billing session lifecycle |

### AC Python Plugin (`plugins/assetto_corsa/`)

Runs INSIDE the AC process. VMS `SPageFile_VMSC_AC` clone.

| File | Purpose |
|------|---------|
| **RaceControl.py** | Entry point: acMain, acUpdate, acShutdown + render callback |
| **rclib/classes.py** | Shared memory struct: 64-car CarData, BoardData leaderboard, camera control |
| **rclib/telemetry.py** | Writer: spin-wait protocol, on-track via render timing, multi-car, leaderboard |

**Shared memory:** `rcpmf_telemetry` — rc-agent reads this instead of `acpmf_*` directly.

**Status protocol (7 states):** UNINITIALIZED(0), BLANKED(1), WRITING(2), READING(3), IDLE(4), INVALID(5), SHUTDOWN(6)

**Data flow:**
```
AC Engine → acpmf_* shared memory → RC Plugin (Python, 60Hz)
  → rcpmf_telemetry (custom shared memory with 64 cars + leaderboard)
  → rc-agent reads safely (no raw pointer crash risk)
  → WS to server → dashboard/leaderboard/spectator
  → CSV fallback on WS disconnect
```

### Deploy Files (`deploy/`)

| File | Purpose |
|------|---------|
| **launch-ac.bat** | VMS SimLauncher clone. SP: direct acs.exe (race.ini pre-written by rc-agent; CM `--race` flag doesn't exist). MP: Content Manager via `acmanager://race/online` URI (handles server join handshake). cmd.exe parents acs.exe (not rc-agent). CM locations: Pod 1 = `SIM 1\Downloads\content-manager\`, Pods 2-8 = `User\Desktop\`. Fallback: direct acs.exe if CM not found. |
| **steam_appid.txt** | `480` (Spacewar). Prevents Steam from killing rc-agent. VMS ships same. |
| **rc-agent.exe.manifest** | `asInvoker` elevation. Prevents anti-cheat from flagging elevated process. |
| **README-deploy-files.md** | Documentation for pod deploy files |

---

## 8. Authentication & Authorization

### JWT Roles

| Role | Access |
|------|--------|
| **customer** | Profile, booking, wallet, friends, multiplayer |
| **cashier** (legacy: "staff") | Pod management, billing, games, drivers |
| **manager** | cashier + financial reports, audit logs, rate management, disputes |
| **superadmin** | Everything: system config, deploy, feature flags, policy rules |

### Middleware Stack

1. `require_staff_jwt()` — 401 if missing/invalid JWT
2. `require_role_manager()` — 403 if not manager/superadmin
3. `require_role_superadmin()` — 403 if not superadmin
4. `require_non_pod_source()` — rejects requests from pod IPs
5. `tower_governor` — rate limiting (5 req/min) on auth endpoints
6. Service auth: `X-Service-Key` header for inter-service calls

### JWT Rotation

Supports current + previous secret for grace period during key rotation.

---

## 9. Recovery & Self-Healing Architecture

### 5-Tier Diagnosis

| Tier | Method | Where | When |
|------|--------|-------|------|
| 0 | **Audit KB** | Server | Check `audit_known_issues` table first |
| 1 | **Deterministic** | Pod-local | Stale sockets, orphan processes, temp files — no AI |
| 2 | **Knowledge Base** | Pod-local SQLite | Past solutions from similar symptoms |
| 3 | **Local LLM** | James Ollama :11434 | qwen2.5:3b diagnosis |
| 4 | **Cloud AI** | OpenRouter | Escalation — not auto-triggered |

### Recovery Systems

- **rc-agent self_monitor** — detects own health degradation
- **RCWatchdog** (Windows Service) — restarts rc-agent in Session 1 after crash
- **rc-sentry** — exec daemon, can restart rc-agent from outside
- **server pod_healer** — 3-tier graduated recovery: Tier 1 restart via sentry → Tier 2 WoL → Tier 3 AI + WhatsApp
- **server fleet_healer** — correlated failure detection (3+ pods same symptom = fleet-wide pattern)
- **rc-watchdog on James** — monitors fleet, AI diagnosis, escalation
- **MAINTENANCE_MODE** sentinel — blocks restarts after 3 crashes in 10min (auto-clears after 30min)
- **GAME_LAUNCHING** sentinel — RAII guard in ws_handler, suppresses watchdog restart during game launch (5min TTL)
- **SetConsoleCtrlHandler** — logs `termination.log` on external kill, cleans sentinels before exit

**WARNING:** These systems can fight each other. See CLAUDE.md "Cross-Process Recovery Awareness".

---

## 10. Build & Deploy

### Build

```bash
# Static CRT (no vcruntime140.dll dependency)
cargo build --release --bin racecontrol    # Server
cargo build --release --bin rc-agent       # Pod
cargo build --release --bin rc-sentry      # Pod exec daemon

# IMPORTANT: touch build.rs after git commit to refresh GIT_HASH
touch crates/<crate>/build.rs
```

### Deploy Order

1. **Canary:** Pod 8 first, verify, then Pods 1-7
2. **Server:** `deploy-staging/deploy-server.sh` (12-model MMA-hardened, 8-step with auto-rollback)
3. **Cloud:** git push -> Bono relay git_pull -> rebuild -> verify BOTH
4. **Frontends:** Rebuild ALL 3 (kiosk, web, admin) after ANY server deploy

### Critical Deploy Rules

- Hash-based binary naming: `rc-agent-<hash>.exe`
- Previous binary preserved 72hr: `*-prev.exe`
- DEPLOY PARITY: local deploy MUST also deploy to cloud
- Security gate (`gate-check.sh`) before any deploy
- Manifest verification (`release-manifest.toml`) with SHA256

---

## 11. Key Architectural Patterns

1. **Broadcast Channels** — Fan-out to dashboards without blocking game launch path
2. **RwLock + Clone** — Read-heavy state: clone snapshot, drop lock, then work
3. **Never hold lock across `.await`** — Clone in tight `{}` block
4. **SQLx compile-time validation** — `query_as!()` macros
5. **Feature flags at runtime** — Single binary, runtime toggling (FF-01+)
6. **Sentinel files** — MAINTENANCE_MODE, OTA_DEPLOYING, GAME_LAUNCHING, HEAL_IN_PROGRESS, RCAGENT_SELF_RESTART (RAII guards with TTL auto-expiry)
7. **Boot resilience** — `spawn_periodic_refetch()` for any data fetched at startup
8. **Service key auth** — Constant-time comparison for inter-service HTTP
9. **Mesh gossip** — Pods learn from each other via aggregated error patterns
10. **Cloud sync dual authority** — Venue authoritative on billing/laps, cloud on drivers/pricing
11. **VMS zero-block launch** — SP: direct `acs.exe` spawn with `DETACHED_PROCESS` (no bat, no console). MP: `launch-ac.bat` for Content Manager URI. rc-agent returns in <1s. Event loop discovers game PID via `find_game_pid()`.
12. **Console isolation** — `FreeConsole()` at startup detaches from `start-rcagent.bat`'s console. SP game launch uses `DETACHED_PROCESS`. Prevents CTRL_CLOSE_EVENT crash (P1 fix 2026-04-07). `SetConsoleCtrlHandler` logs termination reason to `termination.log` as safety net.
13. **SHM snapshot reads** — AC shared memory reads use `IsBadReadPtr` + `copy_nonoverlapping` into local buffer before parsing. Eliminates TOCTOU race between `verify_shm_alive()` and pointer dereference.
14. **CSV lap fallback** — Laps saved to `laps-offline.csv` when WS disconnected. Never lose lap data.
15. **AC plugin shared memory** — Custom `rcpmf_telemetry` buffer with 64-car array, leaderboard, camera control. Spin-wait protocol prevents reader/writer races. rc-agent reads this instead of `acpmf_*` directly.
16. **asInvoker manifest** — External `.manifest` file prevents Windows from elevating rc-agent (anti-cheat compatibility, VMS pattern)

---

## 12. Network Map

| Device | LAN IP | Tailscale IP | User |
|--------|--------|-------------|------|
| Server | 192.168.31.23 | 100.125.108.37 | ADMIN |
| Pod 1 | 192.168.31.89 | 100.92.122.89 | User |
| Pod 2 | 192.168.31.33 | 100.105.93.108 | User |
| Pod 3 | 192.168.31.28 | 100.69.231.26 | User |
| Pod 4 | 192.168.31.88 | 100.75.45.10 | User |
| Pod 5 | 192.168.31.86 | 100.110.133.87 | User |
| Pod 6 | 192.168.31.87 | 100.127.149.17 | User |
| Pod 7 | 192.168.31.38 | 100.82.196.28 | User |
| Pod 8 | 192.168.31.91 | 100.98.67.67 | User |
| POS | 192.168.31.20 | 100.95.211.1 | POS |
| James | 192.168.31.27 | — | bono |
| NVR | 192.168.31.18 | — | admin |
| Bono VPS | 72.60.101.58 | 100.70.177.44 | root |

---

## 13. Meshed Intelligence (MI)

### 13.1 Architecture Overview

Meshed Intelligence is the autonomous self-healing AI system embedded in every rc-agent and coordinated by the server. When an anomaly is detected, the system escalates through 5 tiers of increasing cost and capability:

| Tier | Method | Where | Cost | When |
|------|--------|-------|------|------|
| 0 | **Audit KB** | Server | $0 | Check `audit_known_issues` table for seeded findings |
| 1 | **Deterministic** | Pod-local | $0 | Kill orphans, clear stale sentinels, temp files — no AI |
| 2 | **Knowledge Base** | Pod-local SQLite | $0 | Lookup `mesh_kb.db` for past solutions (confidence > 0.8) |
| 3 | **Single Model** | OpenRouter (Qwen3) | ~$0.05 | One model diagnosis for novel issues |
| 4 | **5-Model MMA** | OpenRouter (5 models) | ~$4.30 | Full parallel diagnosis with consensus voting |

The tier engine (`tier_engine.rs`) processes `DiagnosticEvent` messages from the diagnostic engine and runs tiers sequentially until the issue is resolved or all tiers are exhausted. Key safety features:

- **Circuit breaker** (C1): Skips Tier 3/4 after 3 consecutive OpenRouter failures, 5-minute cooldown
- **Budget pre-check** (C3): Verifies remaining daily budget before Tier 3/4 model calls
- **Event dedup** (T7): Same trigger type within 5 minutes collapses to a single action
- **Rollback tracking** (T10): Records outcome for model-suggested fixes
- **Path traversal guard** (Gemini P1): Sentinel file deletion restricted to `C:\RacingPoint\`

### 13.2 MI Modules

#### Agent-Side (rc-agent)

| Module | Purpose |
|--------|---------|
| **diagnostic_engine.rs** | Anomaly detection: 9 trigger classes, 5-minute periodic scan + event-triggered. Detection only — does NOT apply fixes. |
| **tier_engine.rs** | 5-tier decision tree: processes DiagnosticEvent, runs tiers sequentially, circuit breaker, dedup |
| **openrouter.rs** | OpenRouter API client: Tier 3 single-model, Tier 4 five-model parallel diagnosis. Retry with exponential backoff. |
| **knowledge_base.rs** | Local SQLite KB (`C:\RacingPoint\mesh_kb.db`): solution storage, lookup by problem_hash, confidence scoring, TTL expiry |
| **mesh_gossip.rs** | Solution propagation over WS: Pod announces solutions/experiments to server, receives fleet broadcasts |
| **budget_tracker.rs** | Daily cost tracking per node. Hard ceiling: $10/day/pod, $5/day/POS, $20/day/server. Resets at midnight IST. |
| **mma_engine.rs** | Unified MMA Protocol v3.0 convergence engine: 4-step DIAGNOSE-PLAN-EXECUTE-VERIFY with model reputation tracking |
| **predictive_maintenance.rs** | Threshold-based proactive detection: 9 predictors (GPU temp, disk space, socket accumulation, orphan processes, stuck sentinels) |
| **night_ops.rs** | After-hours maintenance: runs when WS disconnected >30min. Full health check, apply pending fixes, MMA on lingering issues, log cleanup, morning readiness report. |
| **cognitive_gate.rs** | CGP enforcement as pure functions ($0 cost). Phase A (pre-action): G0, G5, G7. Phase D (post-action): G1, G2, G4, G8, G9. |
| **diagnosis_planner.rs** | Plans diagnostic approach based on trigger type and history |
| **diagnostic_log.rs** | Structured diagnostic event logging |
| **self_heal.rs** | Tier 1 deterministic fixes: kill orphans, clear temp, restart Edge |
| **failure_monitor.rs** | Tracks failure patterns and error rates for diagnostic triggering |
| **game_doctor.rs** | 12-point game launch failure diagnosis |
| **mma_cache.rs** | Caches MMA results to avoid redundant model calls |
| **model_reputation.rs** | Tracks per-model accuracy across MMA runs for demotion/promotion |
| **kb_hardening.rs** | KB integrity checks and solution validation |
| **kb_promotion_store.rs** | Persists promotion state for KB solutions |
| **eval_rollup.rs** | Aggregates evaluation results across diagnostic runs |
| **retrain_export.rs** | Exports diagnostic data for model fine-tuning |

#### Server-Side (racecontrol)

| Module | Purpose |
|--------|---------|
| **fleet_kb.rs** | Central fleet KB: `fleet_solutions`, `fleet_experiments`, `fleet_incidents` tables. CRUD for mesh handler and promotion pipeline. |
| **promotion.rs** | Background task (60s interval): promotes candidates to fleet_verified (3+ successes, 2+ pods), detects systemic patterns (3+ pods same problem within 5min), expires stale solutions. |
| **mesh_handler.rs** | Pod gossip aggregation: receives MeshSolutionAnnounce/MeshExperimentAnnounce, stores in fleet KB, broadcasts to other pods |
| **mesh_cloud_sync.rs** | Syncs fleet KB solutions between venue and cloud |
| **pod_healer.rs** | 3-tier graduated recovery: collect diagnostics via `/exec`, apply rule-based fixes, escalate to AI (Ollama then Anthropic) |
| **fleet_healer.rs** | Layer 2 SSH-based healing via Tailscale: fingerprint symptoms, detect fleet-wide patterns, apply deterministic fixes with billing safety and canary rollout |
| **self_healing.rs** | Server-side self-healing coordinator |
| **maintenance_engine.rs** | Problem-diagnosis-fix playbook execution |

### 13.3 Data Flow

```
Anomaly on Pod N
    |
    diagnostic_engine.rs (9 trigger classes, 5-min scan)
    |  emits DiagnosticEvent via mpsc channel
    v
    tier_engine.rs (Q1-Q4 decision gate)
    |
    Q1: knowledge_base.rs lookup (mesh_kb.db)
    |   Hit (confidence >= 0.8) → apply fix → DONE (or Q4 background)
    |   Miss → Q2
    |
    Q2: mesh_gossip.rs — fleet experiment check
    |   Another pod diagnosing same issue? → WAIT 120s
    |   No → Q3
    |
    Q3: budget_tracker.rs pre-check
    |   Budget exhausted? → Tier 1+2 only (free)
    |   Budget OK → openrouter.rs
    |       Tier 3: single Qwen3 call (~$0.05)
    |       Tier 4: mma_engine.rs 5-model parallel (~$4.30)
    |
    Fix applied → knowledge_base.rs stores solution
    |
    mesh_gossip.rs → MeshSolutionAnnounce to server
    |
    Server: mesh_handler.rs → fleet_kb.rs stores in fleet DB
    |
    promotion.rs (60s cycle) → promotes to fleet_verified
    |
    Server: MeshSolutionBroadcast → all connected pods learn
```

### 13.4 OpenRouter Model Pool

5 models with role-specific system prompts trained from MMA audit methodologies:

| Model | Role | Strength | Typical Cost |
|-------|------|----------|-------------|
| **Qwen3 235B** | Scanner | Exhaustive enumeration, volume coverage, broad surface-area scanning | ~$0.05 |
| **DeepSeek R1** | Reasoner | Absence detection, state machine stuck states, logic bugs | ~$0.43 |
| **DeepSeek V3** | Code Expert | Rust/Windows code patterns, Session 0/1, process lifecycle | ~$0.10 |
| **MiMo v2 Pro** | SRE | Operational gaps, stuck states, idempotency, "3am failures" | ~$0.10 |
| **Gemini 2.5 Pro** | Security | Credential scanning, auth checklists, config errors | ~$0.10 |

Tier 3 uses Qwen3 alone (~$0.05). Tier 4 fires all 5 in parallel (~$4.30 total), with a concurrency limiter of max 2 parallel Tier 4 jobs per pod to prevent thundering herd across 8 pods.

### 13.5 Budget System

| Node Type | Daily Limit | Reserve | Reset |
|-----------|-------------|---------|-------|
| Pod (1-8) | $10/day | $2 min reserve | Midnight IST |
| POS | $5/day | $2 min reserve | Midnight IST |
| Server | $20/day | $2 min reserve | Midnight IST |

Monthly soft alert at $50. When ceiling is hit, Tiers 3+4 are blocked and the system falls back to Tier 1+2 (free deterministic). Budget status is exposed via the health endpoint.

### 13.6 Diagnostic Triggers

The diagnostic engine monitors 9 anomaly classes:

| Trigger | Condition |
|---------|-----------|
| `health_check_fail` | rc-agent HTTP health not responding (self-check) |
| `process_crash` | WerFault or abnormal exit detected via sysinfo |
| `game_launch_fail` | launch_started_at > 90s elapsed + no game_pid |
| `display_mismatch` | edge_process_count == 0 when lock_screen_state == blanked |
| `error_spike` | >5 error-level log lines in the last 60s |
| `ws_disconnect` | ws_connected == false for >30s |
| `sentinel_unexpected` | Unexpected sentinel file present |
| `violation_spike` | Process guard violation_count delta >50 in 5 min |
| `periodic` | Scheduled 5-minute scan (always runs) |

### 13.7 Configuration

**Agent config** (`rc-agent.toml` `[ai_debugger]` section):
```toml
[ai_debugger]
enabled = true
scan_interval_secs = 300    # 5-minute periodic scan
budget_daily = 10.0         # $10/day/pod
```

**Environment:** `OPENROUTER_KEY` — API key for OpenRouter. Never hardcoded. Read from env var at runtime.

**Training mode** (`racecontrol.toml` `[mma]` section):
```toml
[mma]
training_mode = true
training_start = "2026-03-31"
training_end = "2026-04-29"
daily_budget_pod = 15.0
daily_budget_server = 25.0
```

### 13.8 Predictive Maintenance Thresholds

| ID | Metric | Threshold | Action |
|----|--------|-----------|--------|
| PRED-01 | ConspitLink reconnection rate | >3/hour | USB alert |
| PRED-02 | Edge process count trending to 0 | 2 consecutive scans | Memory leak restart |
| PRED-03 | GPU temperature | >80C | Thermal alert |
| PRED-04 | rc-agent restarts/day | >2 | Stability alert |
| PRED-05 | Disk space | <10 GB | Auto-cleanup |
| PRED-06 | Error spike across 3+ pods | Simultaneous | Systemic alert (server coordinator) |
| PRED-07 | CLOSE_WAIT socket accumulation | >20 | Port exhaustion alert (MiMo SRE) |
| PRED-08 | Orphan PowerShell count | >3 | Memory leak from self-restart (MiMo SRE) |
| PRED-09 | MAINTENANCE_MODE age | >30 min | Stuck sentinel alert (R1 Reasoner) |

---

## 14. Unified MMA Protocol

### 14.1 Overview

The Unified MMA Protocol v3.0 is the multi-model AI diagnostic reasoning engine used when Tier 3/4 is invoked. Full spec: `.planning/specs/UNIFIED-MMA-PROTOCOL.md`.

### 14.2 4-Step Convergence Engine

```
Step 1: DIAGNOSE  →  5 models × N iterations → consensus on ALL problems
Step 2: PLAN      →  5 models × N iterations → consensus on fix plans
Step 3: EXECUTE   →  5 models × N iterations → consensus on best solution
Step 4: VERIFY    →  deterministic checks + 3-model adversarial sanity check
```

- **Consensus:** 3/5 majority agreement per finding
- **Iterations:** Minimum 2, maximum 4 per step. Converged when iteration N produces <2 new findings vs N-1.
- **Backtracking:** Step 4 failure triggers partial retry (Steps 3-4), then full backtrack to Step 1. Max 3 full backtracks before multi-channel escalation and SAFE_MODE.

### 14.3 Model Pool (10 models per step, 5 selected per iteration)

| Slot | Role (required) | Example Models |
|------|-----------------|----------------|
| 1 | Reasoner | DeepSeek R1 0528, GPT-5.4 Nano, Kimi K2.5 |
| 2 | Code Expert | DeepSeek V3.2, Grok Code Fast, Qwen3 Coder |
| 3 | SRE/Ops | MiMo v2 Pro, Nemotron 3 Super, MiMo v2 Flash |
| 4 | Domain Specialist | Varies by issue domain |
| 5 | Generalist | Qwen3 235B, Gemini 2.5 Flash, Mistral Medium |

**Vendor diversity:** Each 5-model iteration must include >=1 reasoner + >=1 code expert + >=1 SRE. Max 2 per vendor family. Min 3 vendor families.

### 14.4 When to Run MMA

- Before milestone ship (all models)
- After security incident
- New crate or service
- Cross-system bridge deploy (MANDATORY)
- User requests "MMA audit"

### 14.5 Budget

~$2-5 per full audit via OpenRouter. Session budget: $5 unless approved for more. Step timeouts: 60s per model, 5min per step. 3+ timeouts triggers provider fallback.

### 14.6 Script

```bash
cd ~/racingpoint/racecontrol
export OPENROUTER_KEY="..."
node scripts/multi-model-audit.js           # v3.0 consensus mode (default)
DRY_RUN=1 node scripts/multi-model-audit.js # dry run — no API calls
MMA_SESSION_BUDGET=10 node scripts/multi-model-audit.js  # budget override
```

### 14.7 Model Reputation Tracking

The `mma_engine.rs` tracks per-model accuracy across MMA runs. After Step 4 verification, each model's diagnosis is scored against the deterministic outcome. Models that consistently identify correct minority opinions get promoted; models that consistently disagree with verified outcomes get demoted. Reputation is in-memory (resets on restart).

---

## 15. Config Management & Policy Engine

### 15.1 AgentConfig Push via WebSocket

Server pushes configuration changes to pods over the existing WebSocket connection using the `ConfigPush` channel (CP-01). Config is never pushed through the fleet exec endpoint.

**Endpoints:**

| Method | Path | Auth | Purpose |
|--------|------|------|---------|
| POST | `/api/v1/config/push` | Staff JWT | Validate, queue, and deliver field-level config to pods |
| GET | `/api/v1/config/push/queue` | Staff JWT | View per-pod delivery queue |
| GET | `/api/v1/config/audit` | Staff JWT | View audit log of all config push events |
| POST | `/api/v1/config/pod/{pod_id}` | Staff JWT | Store full AgentConfig for a specific pod |
| GET | `/api/v1/config/pod/{pod_id}` | Staff JWT | Retrieve stored AgentConfig for a pod |

**Flow:** Admin UI edits config fields -> POST to server -> server validates schema + computes SHA-256 hash -> queues delivery -> pushes via WS `ConfigPush` message to target pods (or all connected pods if no target specified) -> pods apply and ACK.

### 15.2 Game Preset Library

Pre-configured game launch templates with reliability scoring. Stored in `game_presets` table.

**Endpoints:**

| Method | Path | Auth | Purpose |
|--------|------|------|---------|
| GET | `/api/v1/presets` | Public | List all presets with reliability scores |
| POST | `/api/v1/presets` | Staff JWT | Create preset |
| GET | `/api/v1/presets/{id}` | Public | Get single preset |
| PUT | `/api/v1/presets/{id}` | Staff JWT | Update preset |
| DELETE | `/api/v1/presets/{id}` | Staff JWT | Soft-delete (sets enabled=0) |

**Reliability scoring:** Aggregated from `combo_reliability` table — `AVG(success_rate)` where `SUM(total_launches) >= 5`. Scores aggregate across all pods so a preset is flagged unreliable if it fails on any pod.

### 15.3 Policy Rules Engine

IF-metric-THEN-action rules evaluated against real-time telemetry. Stored in `policy_rules` table with evaluation log in `policy_eval_log`.

**Endpoints:**

| Method | Path | Auth | Purpose |
|--------|------|------|---------|
| GET | `/api/v1/policy/rules` | Staff JWT | List all rules |
| POST | `/api/v1/policy/rules` | Staff JWT | Create rule |
| PUT | `/api/v1/policy/rules/{id}` | Staff JWT | Update rule |
| DELETE | `/api/v1/policy/rules/{id}` | Staff JWT | Delete rule |
| GET | `/api/v1/policy/eval-log` | Staff JWT | Evaluation log (last 500) |

**Conditions:** `gt` (greater than), `lt` (less than), `eq` (equals). Rules compare a metric value against a threshold and fire an action when the condition is met.

---

## 16. Billing Architecture

### 16.1 FSM States

Billing is driven by a strict finite state machine in `billing_fsm.rs`. Every status mutation goes through `validate_transition()` — there is no other path.

```
Pending → WaitingForGame → Active → Completed
                                  → EndedEarly
                                  → Cancelled
              → CancelledNoPlayable

Active → PausedGamePause    → Resume → Active
Active → PausedDisconnect   → Resume → Active
Active → PausedManual       → Resume → Active
Active → PausedCrashRecovery → Resume → Active
```

**Events driving transitions:**

| Event | From | To |
|-------|------|----|
| StartWaiting | Pending | WaitingForGame |
| GameLive | WaitingForGame | Active |
| Pause | Active | PausedGamePause |
| CrashPause | Active | PausedCrashRecovery |
| Disconnect | Active | PausedDisconnect |
| PauseManual | Active | PausedManual |
| Resume | Any Paused | Active |
| End | Active or Paused | Completed |
| EndEarly | Active or PausedCrashRecovery | EndedEarly |
| Cancel | Active or Paused | Cancelled |
| CancelNoPlayable | WaitingForGame | CancelledNoPlayable |

### 16.2 Split Sessions (FSM-07)

Each parent billing session can have child splits with independent allocated_seconds and their own status lifecycle: `Pending -> Active -> Completed | Cancelled`.

### 16.3 Per-Minute Billing

Billing ticks are broadcast via WS with a monotonic `tick_seq` (u64) so kiosk/agent can ignore stale ticks after WS reconnect. The timer runs only while in `Active` state — all `Paused*` states freeze the clock.

### 16.4 Dynamic Pricing

`compute_dynamic_price()` looks up `pricing_rules` table by day-of-week and hour. Rules support multiplier and flat adjustment. Minimum enforced price: 100 paise (Rs.1) to prevent free/negative sessions.

### 16.5 Refund Flow

- **EndedEarly:** Pro-rata refund for unused time, computed from `wallet_debit_paise`
- **Cancelled:** Full refund
- **CancelledNoPlayable:** Full refund (game never became playable)
- **Completed:** No refund (time fully consumed)

All end paths converge on `authoritative_end_session()` in `billing_fsm.rs` — the single source of truth for session termination.

### 16.6 Crash Recovery (SESSION-03)

When a game crashes during an active billing session: billing pauses (`PausedCrashRecovery`), up to 2 automatic relaunch attempts (60s each). If both fail, the session auto-ends via the same path as orphan detection. The customer is not charged for crash time.

---

## 17. Game Launch Architecture

### 17.1 Launch Chain

The game launch follows the VMS zero-block pattern:

1. **Server** receives launch request, validates, sends `LaunchGame(AcLaunchParams)` via WS to pod agent
2. **rc-agent** receives command in `ws_handler.rs`, sets `GAME_LAUNCHING` sentinel (RAII guard, 5-min TTL)
3. **ac_launcher.rs**: Kill existing AC processes -> Write `race.ini` -> Launch `acs.exe`:
   - **SP mode (v44+):** Direct `Command::new("acs.exe")` with `DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP`. No bat, no cmd.exe, no console inheritance. Prevents CTRL_CLOSE_EVENT crash (P1 fix `d616ee10`).
   - **MP mode:** `launch-ac.bat` subprocess with `CREATE_NO_WINDOW` for Content Manager URI handling (`acmanager://race/online`)
4. **Event loop** returns immediately (<1s). `LaunchState` transitions to `WaitingForLive`
5. **game_check_interval** polls for game PID via `find_game_pid()`
6. **launch_verifier.rs**: 4-stage verification: ProcessAlive -> SharedMemory -> OnTrack
7. When AC shared memory (`rcpmf_telemetry`) reports valid telemetry, state transitions to `Live`
8. Server billing transitions from `WaitingForGame` to `Active`

### 17.2 LaunchState Enum

```rust
enum LaunchState {
    Idle,
    WaitingForLive {
        launched_at: Instant,
        attempt: u8, // 1 or 2
    },
    Live,
}
```

- **AC timeout:** 60s (CSP + AC typically loads in <30s)
- **Generic sim timeout:** 180s (no telemetry detection for EVO, WRC, Forza, FH5)
- **Auto-retry:** Once on timeout, cancel on second failure (no charge)
- **Process death:** If game process dies during WaitingForLive, skip retry

### 17.3 Crash Recovery State Machine (SESSION-03)

```rust
enum CrashRecoveryState {
    Idle,
    PausedWaitingRelaunch {
        attempt: u8,            // 1 or 2
        timer: Sleep,           // 30s per attempt
        last_sim_type: SimType,
        last_launch_args: Option<String>,
    },
    AutoEndPending,
}
```

### 17.4 Multi-Sim Support

The agent supports multiple simulators. `ac_launcher.rs` handles Assetto Corsa specifically; `game_process.rs` provides the generic framework. `DifficultyTier` maps to AC's `AI_LEVEL`: Rookie, Amateur, SemiPro, Pro, Alien.

### 17.5 Difficulty & Assists

Racing-themed difficulty tiers control `AI_LEVEL` only. Assists are completely independent (user decision). `AI_AGGRESSION` is not currently used.

---

## 18. Fleet Operations

### 18.1 Deploy Pipeline (fleet_deploy.rs)

Rolling binary deployments orchestrated by the server:

**Wave Layout:**

| Wave | Pods | Role |
|------|------|------|
| Wave 1 (canary) | Pod 8 | Canary — failure halts entire deploy |
| Wave 2 | Pods 1-4 | Main wave — per-pod rollback on failure, deploy continues |
| Wave 3 | Pods 5-7 | Final wave |

**Request:** `POST /api/v1/fleet/deploy` with `binary_hash` (SHA-256), `binary_url` (staging HTTP), `scope` (all/canary/specific), `wave_delay_secs` (default 5), `force` (override peak-hour lock).

**Deploy scopes:** `all` (canonical wave order), `canary_only` (Pod 8 only), `specific` (named pod set).

### 18.2 Graduated Pod Recovery (pod_healer.rs)

Runs every 2 minutes per connected pod. Three tiers of increasing intervention:

| Tier | Action | Authority |
|------|--------|-----------|
| 1 | Rule-based fixes via `/exec`: kill zombie sockets, clear temp files | Autonomous |
| 2 | WoL magic packet to wake unresponsive pods | Autonomous |
| 3 | AI diagnosis (Ollama -> Anthropic) + WhatsApp alert to staff | Escalation |

Protected processes (never killed): `rc-agent.exe`, `acs.exe`, `conspitlink2.0.exe`, `msedge.exe`, `explorer.exe`, `steam.exe`, plus system processes.

rc-agent restarts are deferred to `pod_monitor.rs` which owns the shared `EscalatingBackoff`. The healer reads backoff state for cooldown gating but does NOT advance it.

### 18.3 Fleet Healing (fleet_healer.rs)

Layer 2 SSH-based healing via Tailscale for dark/broken pods that cannot be reached via HTTP:

- SSH into pods using Tailscale IPs (user: `User`, timeout: 10s connect, 30s command)
- Fingerprint symptoms and detect fleet-wide patterns
- Apply deterministic fixes with billing safety checks and canary rollout
- Full audit trail of all SSH commands and outcomes

### 18.4 Wake-on-LAN (wol.rs)

Context-aware magic packet sender on the venue LAN broadcast (`192.168.31.255:9`):

- Constructs standard WoL magic packet (6x 0xFF + 16x MAC)
- Checks pod heal lease before waking to avoid waking pods under active heal control
- Also provides `shutdown_pod()` via rc-agent `/exec` endpoint

### 18.5 Additional Server Modules (Not Elsewhere Documented)

| Module | Purpose |
|--------|---------|
| **fleet_health.rs** | Fleet health aggregation endpoint (`/api/v1/fleet/health`) |
| **fleet_alert.rs** | Fleet-level alert generation and routing |
| **fleet_report.rs** | Periodic fleet status reports |
| **deploy_awareness.rs** | Tracks deploy state across fleet for coordination |
| **venue_shutdown.rs** | Coordinated venue shutdown sequence |
| **venue_state.rs** | Venue open/closed state management |
| **backup_pipeline.rs** | Automated DB backup pipeline |
| **snapshot_manager.rs** | Point-in-time state snapshots |
| **error_aggregator.rs** | Cross-pod error pattern aggregation |
| **error_rate.rs** | Error rate tracking and trending |
| **escalation.rs** | Multi-channel escalation (WhatsApp, email, comms-link) |
| **alert_engine.rs** | Alert generation, routing, and deduplication |
| **metric_alerts.rs** | Metric threshold-based alerting |
| **notification_outbox.rs** | Outbox pattern for reliable notification delivery |
| **synthetic_monitor.rs** | Synthetic monitoring probes for service health |
| **dependency_chain.rs** | Service dependency tracking for cascade analysis |
| **cascade_guard.rs** | Prevents cascading failures across dependent services |
| **recovery.rs** | Recovery coordination and authority tracking |
| **optimization_engine.rs** | Performance optimization recommendations |
| **business_aggregator.rs** | Business metrics aggregation (revenue, utilization) |
| **business_forecast.rs** | Revenue and demand forecasting |
| **driver_rating.rs** | Driver skill rating system |
| **psychology.rs** | Gamification and pricing psychology engine |
| **feedback_loop.rs** | Customer feedback collection and analysis |
| **dynamic_pricing.rs** | Time-of-day and demand-based pricing |
| **pricing_bridge.rs** | Bridge between pricing rules and billing |
| **visits.rs** | Customer visit tracking |
| **pod_reservation.rs** | Pod time-slot reservation |
| **reservation.rs** | General reservation management |
| **friends.rs** | Social features (friend lists, group sessions) |
| **scheduler.rs** | Task scheduling and cron-like jobs |
| **action_queue.rs** | Queued action execution |
| **data_collector.rs** | Telemetry data collection pipeline |
| **telemetry_store.rs** | Time-series telemetry storage |
| **server_diagnostics.rs** | Server self-diagnostics |
| **server_ops.rs** | Server operational commands (:8090 on server) |
| **bono_relay.rs** | Relay bridge to Bono VPS |
| **ollama_client.rs** | Local Ollama LLM client for Tier 3 diagnosis |
| **email_alerts.rs** | Email-based alerting |
| **whatsapp_escalation.rs** | WhatsApp escalation for critical issues |

### 18.6 Additional Agent Modules (Not Elsewhere Documented)

| Module | Purpose |
|--------|---------|
| **sentinel_watcher.rs** | Monitors sentinel file state (MAINTENANCE_MODE, OTA_DEPLOYING, etc.) |
| **startup_cleanup.rs** | Boot-time cleanup of stale state |
| **startup_log.rs** | Logs startup sequence for diagnostics |
| **pre_flight.rs** | Pre-flight checks before game launch |
| **steam_checks.rs** | Verifies Steam is running and logged in |
| **iracing_checks.rs** | iRacing-specific pre-launch validation |
| **content_scanner.rs** | Scans installed game content (cars, tracks) |
| **firewall.rs** | Windows firewall rule management |
| **self_monitor.rs** | Agent self-health monitoring |
| **self_test.rs** | Agent self-test suite |
| **kiosk.rs** | Kiosk browser management (Edge) |
| **billing_guard.rs** | Billing session lifecycle guards |
| **revenue_protection.rs** | Revenue protection rules (prevents free play) |
| **experience_collector.rs** | Customer experience telemetry collection |
| **experience_score.rs** | Per-session experience quality scoring |
| **experience_actions.rs** | Automated actions based on experience score |
| **feature_flags.rs** | Runtime feature flag management (periodic re-fetch) |
| **debug_server.rs** | Debug HTTP endpoint (:18924) |
| **mdns_discovery.rs** | mDNS-based server discovery |
| **weekly_report.rs** | Weekly operational report generation |
| **tls.rs** | TLS configuration for secure connections |
| **udp_heartbeat.rs** | UDP-based heartbeat for low-overhead health checks |

---

## 19. Customer Journey (Acts 1-4)

The complete customer lifecycle at Racing Point eSports, from walking in to leaving. Designed with Uday (2026-04-03).

### 19.1 Act 1 — Registration, Payment, Session Start

```
Customer arrives
    |
    +-- PWA registration (app.racingpoint.cloud): phone → WhatsApp OTP → name, DOB, waiver
    |   OR walk-in registration (:3300/register): name + DOB + waiver, no phone
    |   OR parent adds "Racers" (up to 3 linked children) from PWA /racers
    |
    v
Payment at counter (POS :3200/billing)
    |  Staff uses WalletTopupModal → cash/UPI/card → wallet credits
    |  WhatsApp receipt if phone on file
    |  Parent wallet covers all linked racers
    v
Session start (POS :3200/billing)
    |  Staff clicks idle pod → BillingStartModal
    |  Selects driver → pricing tier → wallet debited
    |  Session enters "waiting_for_game"
    v
Game launch (Staff Kiosk :3300/staff OR customer idle screen)
    |  Staff: select game, configure, launch
    |  Agent: WaitingForLive → car on track → AcStatus::Live → billing Active
```

**Key roles:**
- **POS** (`:3200/billing`) — money operations ONLY: wallet top-up, billing start, transaction history, refunds
- **Staff Kiosk** (`:3300/staff`) — game operations: configure game, launch, end session, game switch
- **PWA** (`app.racingpoint.cloud`) — customer self-service: registration, racers, wallet view, stats

**Pricing model:**

| Mode | Rate | Timer | Wallet Debit |
|------|------|-------|-------------|
| Per-minute | ₹25/min | Counts UP | Hold at start → debit every 60s → reconcile |
| Package 30min | ₹700 (₹23/min) | Counts DOWN | Full upfront, pro-rated refund on early end |
| Package 60min | ₹900 (₹15/min) | Counts DOWN | Full upfront, pro-rated refund on early end |
| Trial (5min) | Free | Counts DOWN | None — AC only, curated presets |

### 19.2 Act 2 — Racing Session

**Billing triggers per game:**

| Game | Trigger | Detection Method |
|------|---------|-----------------|
| Assetto Corsa | Car on track | `AcStatus::Live` via shared memory (`rcpmf_telemetry`) |
| F1 25 | Car on track | UDP port 20777, speed > 0 |
| iRacing | `IsOnTrack = true` | iRSDK shared memory |
| Le Mans Ultimate | `IsOnTrack = true` | rF2 shared memory |
| Forza / FH5 / WRC / EVO | 180s process fallback | `is_running()` check |

**Mid-session events:**
- **Game crash:** Billing auto-pauses → apology screen + 30s countdown → auto-relaunch (up to 2 attempts) → billing resumes when car hits track. Customer loses zero time.
- **Game switch:** Billing pauses → staff configures new game → launches → car hits track → billing resumes. Switch time is free.
- **Per-minute auto-end:** Wallet hits ₹0 → session ends automatically.
- **Package auto-end:** Timer reaches zero → session ends automatically.
- **Package upgrade (30→60 only):** Debit difference (₹200) from wallet, extend timer.

### 19.3 Act 3 — Session End & Post-Session

**Early exit pricing (packages):**
- 0-29 min: `minutes × per_minute_rate` (₹25/min)
- 30-59 min: `₹700 + (minutes - 30) × ₹25`
- 60+ min: `₹900` (cap)
- Refund = amount_paid - actual_cost → credited to wallet

**Pod screen (60 seconds, then auto-idle):**
- Session summary: best lap, total laps, time played
- Leaderboard position: "#4 All-Time at Monza"
- Review/follow incentive QR codes
- Walk-in registration nudge (QR → PWA → ₹50 signup bonus)
- NO wallet balance shown (privacy)

**Background cleanup (during 60s summary):**
Kill game processes → kill ConspitLink → reset FFB wheel → clear temp files → verify rc-agent healthy → pod ready for next customer.

**Receipts (3x redundancy):** WhatsApp (if phone) + PWA push + thermal print (Epson 80mm, on request).

**Review incentives:** Google review = ₹50 credits, Instagram follow = ₹25 credits. Staff-verified before credits deposited.

### 19.4 Act 4 — Venue Operations

**Startup:** PCs power on → automated pre-flight checks per pod (rc-agent Session 1, WS connected, Edge count > 0, no stale game processes, FFB detected, disk space, no MAINTENANCE_MODE) → all green = open for business.

**Freedom Mode:** Admin-only toggle per pod (Eagle icon). rc-agent stays running but all restrictions lifted — blanking screen off, process guard disabled, staff can use desktop freely. No billing allowed on Freedom Mode pods.

**Shutdown:** "Shut Down All" from staff kiosk (with confirmation). Active sessions reconciled before shutdown. Dangerous actions behind "..." menu with confirmation dialogs.

**Staff permissions (2 roles only):**

| Role | Can Do | Cannot Do |
|------|--------|-----------|
| **Staff** | Start/end sessions, launch games, wallet top-up, cafe, pod restart/shutdown | Refunds, Freedom Mode, add staff, pricing, admin |
| **Admin** (Uday) | Everything — absolute access | — |

---

## 20. GSD Development Workflow

GSD ("Get Stuff Done") is the AI-assisted development methodology used for all Racing Point milestones. Full tooling via `/gsd:*` slash commands.

### 20.1 Lifecycle

```
/gsd:new-project  →  PROJECT.md + ROADMAP.md
    |
    v (per milestone)
/gsd:new-milestone  →  requirements + phase breakdown
    |
    v (per phase)
/gsd:discuss-phase  →  gather context, resolve ambiguity
    |
/gsd:plan-phase     →  PLAN.md with tasks + dependencies + verification
    |
/gsd:execute-phase  →  code + tests + atomic commits
    |
/gsd:verify-work    →  UAT against original requirements
    |
/gsd:ship           →  PR + review + merge (+ MMA audit for milestones)
```

### 20.2 Planning Artifacts

| File | Location | Purpose |
|------|----------|---------|
| `PROJECT.md` | `.planning/` | Project identity, goals, tech stack |
| `ROADMAP.md` | `.planning/` | Phase breakdown with checkbox tracking |
| `PLAN.md` | `.planning/phases/<N>/` | Per-phase task breakdown |
| `RESEARCH.md` | `.planning/phases/<N>/` | Pre-planning research |
| `SUMMARY.md` | `.planning/phases/<N>/` | Post-execution summary |
| `VERIFICATION.md` | `.planning/phases/<N>/` | Goal-backward verification |
| `UI-SPEC.md` | `.planning/phases/<N>/` | Frontend design contract |
| `UI-REVIEW.md` | `.planning/phases/<N>/` | Post-build visual audit |

### 20.3 Shipped Milestones (49+ total, updated 2026-04-06)

Key milestones in shipping order (most recent first):

| Version | Name | Phases | Shipped | Key Deliverable |
|---------|------|--------|---------|-----------------|
| v43.0 | Self-Audit & Visual Regression | 325-328 | 2026-04-06 | Page crawler, visual regression, deploy enforcement, AI self-audit |
| v42.0 | Meshed Intelligence Migration | 321-324 | 2026-04-07 | MI engine in rc-sentry, MMA+cognitive gate, peer gossip, coordinated launch |
| v40.0 | Game Launch Reliability | 311-314 | 2026-04-03 | WS ACK protocol, GameState resilience, billing atomicity |
| v39.0 | Session Trace ID & Metrics | 310 | 2026-04-02 | session_id propagation for E2E traceability |
| v38.0 | Security Hardening & Ops Maturity | 305-309 | 2026-04-02 | mTLS, WS auth, RBAC, audit logs, security audit script |
| v37.0 | Data Resilience & Multi-Venue Prep | 300-303 | 2026-04-02 | SQLite backup, cloud sync v2, event archive, multi-venue schema |
| v36.0 | Config Management & Policy Engine | 295-299 | 2026-04-01 | AgentConfig, server-pushed config, game presets, policy rules |
| v35.0 | Structured Retraining & Model Lifecycle | 290-294 | 2026-04-01 | Model evaluation store, KB promotion, retrain data export |
| v34.0 | Time-Series Metrics & Dashboards | 285-291 | 2026-04-01 | SQLite TSDB, dashboard, Prometheus, WhatsApp alerts |
| v32.0 | Autonomous Meshed Intelligence | 273-279 | 2026-04-01 | MI wire producers, intelligence report v2 |
| v31.0 | Autonomous Survival System | 265-268 | 2026-04-06 | Server deployed, closed (pods have newer builds) |
| v28.0 | Leaderboard & Telemetry | 251-255 | 2026-03-29 | Telemetry persistence, driver ratings, real-time WS |
| v27.0 | Workflow Integrity & Compliance | 251-260 | 2026-03-29 | DB foundation, financial atomicity, FSM hardening, security |
| v26.0 | Meshed Intelligence | 229-240 | 2026-03-28 | 5-tier AI diagnosis, 9 modules, gossip mesh |
| v25.0 | Debug-First-Time-Right | — | 2026-03-26 | Error catalog, diagnostic playbook, log locations |
| v24.0 | Game Launch & Billing Rework | 195-201 | 2026-03-26 | PlayableSignal, launch flow rework |
| v23.0 | Audit Protocol v4.0 | — | 2026-03-25 | 60-phase automated runner, parallel engine |
| v22.0 | Feature Management & OTA | — | 2026-03-25 | Feature flags, OTA pipeline, standing rules |
| v20.0 | Admin Dashboard | — | 2026-03-24 | Fleet/billing/drivers/events/games/control room |
| v19.0 | Cafe Inventory & Ordering | — | 2026-03-22 | Menu/inventory/ordering/receipts/promotions |
| v18.0 | Seamless Execution | — | 2026-03-22 | Relay, chain orchestration, quality gates |
| v17.0 | Cloud Platform | — | 2026-03-22 | VPS deploy, PWA at racingpoint.cloud |
| v3.0 | Billing & POS | — | 2026-03-24 | Wallet, sessions, kiosk/web/admin POS |

**Active milestones:** None — all milestones shipped as of 2026-04-07

**Standing rule:** After completing any milestone via `/gsd:complete-milestone`, update BOTH this table AND `~/.claude/projects/C--Users-bono/memory/gsd-projects.md`.

### 20.4 Subagent Gates (Mandatory per phase type)

| Phase Type | Required Agent | Artifact |
|------------|---------------|----------|
| Any frontend | `gsd-ui-researcher` → `gsd-ui-auditor` | UI-SPEC.md + UI-REVIEW.md |
| Multi-phase milestone (3+) | `gsd-integration-checker` | Integration check |
| Business logic | `gsd-nyquist-auditor` | Test coverage audit |
| New milestone | `gsd-codebase-mapper` | Refresh codebase docs |

---

## 21. Cognitive Gate Protocol (CGP)

CGP v4.3 is the AI quality enforcement system that prevents the "claim done without verifying" failure mode. Full spec: `COGNITIVE-GATE-PROTOCOL.md`.

### 21.1 Why CGP Exists

RLHF trains AI to produce completion-signaling language. 45.4% of AI PRs claim unimplemented changes. Without CGP, James declares "done" from proxy metrics (health 200, build_id match) while actual behavior is broken.

### 21.2 Five Hard Gates (Hook-Enforced)

| Gate | Trigger | Enforcement |
|------|---------|-------------|
| **H1** | Before any action tool | Hook blocks until `PROBLEM: ... PLAN: ...` produced |
| **H2** | Completion claims | Fix and verify must be in SEPARATE messages |
| **H3** | Before "done/fixed/PASS" | Exact behavior + raw output + WHERE (machine) + NOT TESTED list |
| **H4** | Before "all/everywhere" | Grep + per-target enumeration with evidence BEFORE assertion |
| **H5** | User correction | Mandatory G9: root cause + structural fix. Session target: 0 |

### 21.3 Backlog Gate (v4.3)

- `backlog-enforce.js` scans memory for undeployed/pending work every prompt
- WIP >= 3 incomplete items blocks new feature work
- COMMITTED ≠ SHIPPED — must be deployed + verified on target machines
- "Next session" banned as disposition

### 21.4 Permanence Gate

Every fix must survive redeploy. Source code (git) = permanent. Manual server edit = temporary. Temp fixes must have a deploy script or root cause fix.

### 21.5 Hook Enforcement (4 layers)

| Layer | File | Type |
|-------|------|------|
| 1 | `CLAUDE.md` (all projects) | Declarative rules |
| 2 | `cgp-session-inject.js` | UserPromptSubmit — injects gate reminders |
| 3 | `cgp-enforce.js` | PreToolUse — BLOCKS action tools until G0 produced |
| 4 | `cgp-cleanup.js` | SessionStart — cleans stale state |

### 21.6 Session Metrics

Reported at session end: `Claims: N | Corrections: N | FCR: N% | G9s: N`

Target: 0 corrections per session. Every correction triggers a G9 (root cause + structural fix).
