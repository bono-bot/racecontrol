# Racing Point RaceControl — System Architecture

## Overview

RaceControl is a Rust/Axum + Next.js monorepo powering Racing Point eSports venue operations: 8 sim racing pods, billing, game management, fleet orchestration, AI diagnostics, and cloud sync.

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
| **launch-ac.bat** | VMS SimLauncher clone: SP + MP modes via Content Manager. cmd.exe parents acs.exe (not rc-agent). CM locations: Pod 1 = `SIM 1\Downloads\content-manager\`, Pods 2-8 = `User\Desktop\`. Fallback: direct acs.exe if CM not found. |
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
11. **VMS zero-block launch** — `launch-ac.bat` is a transient subprocess (like VMS SimLauncher). rc-agent spawns it and returns in <1s. Event loop discovers game PID via `find_game_pid()`.
12. **SetConsoleCtrlHandler** — Logs termination reason (Ctrl+C, Close, Logoff, Shutdown, taskkill) to `termination.log` and cleans sentinel files before dying
13. **CSV lap fallback** — Laps saved to `laps-offline.csv` when WS disconnected. Never lose lap data.
14. **AC plugin shared memory** — Custom `rcpmf_telemetry` buffer with 64-car array, leaderboard, camera control. Spin-wait protocol prevents reader/writer races. rc-agent reads this instead of `acpmf_*` directly.
15. **asInvoker manifest** — External `.manifest` file prevents Windows from elevating rc-agent (anti-cheat compatibility, VMS pattern)

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
