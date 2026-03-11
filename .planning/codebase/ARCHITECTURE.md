# RaceControl Architecture

## System Overview

RaceControl is a **Rust + Next.js monorepo** that manages a distributed sim racing venue with 8 gaming pods. The system consists of three Rust crates (rc-common, rc-core, rc-agent) and three Next.js frontends (kiosk, pwa, web), all coordinated via WebSocket and HTTP APIs.

```
┌─────────────────────────────────────────────────────────────────┐
│                      RaceControl System                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                   │
│  [Cloud] ◄────────────────────────────────────────────────────┐ │
│  (Bono)  HTTP sync every 30s          ┌──────────────────┐   │ │
│    ▲     & cloud authoritative        │   rc-core        │   │ │
│    │     for drivers, pricing         │  Axum Server     │   │ │
│    │                                   │  Port 8080       │   │ │
│    │     ┌──────────────┐              └────────┬─────────┘   │ │
│    │     │ Pricing      │                       │             │ │
│    │     │ Drivers      │             ┌─────────┴──────┬──────────┤
│    │     │ Tournaments  │             │                │          │
│    └─────┤ (master)     │             │                │          │
│          └──────────────┘             │                │          │
│                                        │                │          │
│                          ┌─────────────▼────┐   ┌──────▼───────┐ │
│                          │  SQLite Database │   │  WebSocket   │ │
│                          │   (local auth)   │   │  Multiplexer │ │
│                          │   (local billing)│   │              │ │
│                          └──────────────────┘   └──────┬───────┘ │
│                                                         │         │
│                     ┌───────────────────────────────────┼─────────┤
│                     │                                   │         │
│    ┌────────────────▼────────────┐  ┌──────────────────▼───────┐ │
│    │    rc-agent (Pod 1-8)       │  │    Web/Mobile Clients    │ │
│    │  • Game lifecycle mgmt      │  │                          │ │
│    │  • Wheelbase USB HID        │  │  • Kiosk (port 3300)     │ │
│    │  • Lock screen QR auth      │  │  • PWA (port 3100)       │ │
│    │  • Driving detector         │  │  • Dashboard (port 3200) │ │
│    │  • AI auto-fix              │  │                          │ │
│    │  • Game launch (AC, F1, etc)│  └──────────────────────────┘ │
│    └────────────────────────────┘                                │
│                                                                   │
│  [Each Pod] 192.168.31.x                                        │
│  • RTX 4070 simulation PC                                       │
│  • Conspit Ares 8Nm wheelbase (OpenFFBoard)                     │
│  • Game telemetry via UDP (AC, F1 25, iRacing, etc)            │
│                                                                   │
└─────────────────────────────────────────────────────────────────┘
```

---

## Crate Architecture

### 1. **rc-common** (Shared Library)
**Location:** `crates/rc-common/src`
**Purpose:** Shared types and protocol definitions across all crates

#### Modules:
- **`types.rs`** (80 lines)
  - `SimType` enum: AssettoCorsa, AssettoCorsaEvo, IRacing, F125, LeMansUltimate, Forza
  - `PodInfo`: Pod identity, status, sim type, IP, driver context
  - `PodStatus` enum: Offline, Idle, InSession, Error, Disabled
  - `Driver`: Driver profile with totals
  - `SessionInfo`, `TelemetryFrame`, `LapData`: Race data
  - `DrivingState`, `GameState`: Pod state tracking
  - `BillingSessionInfo`, `PricingTier`: Billing domain

- **`protocol.rs`** (60+ lines)
  - `AgentMessage` enum: Register, Heartbeat, Telemetry, LapCompleted, SessionUpdate, etc.
  - `CoreToAgentMessage` enum: Registered, StartSession, StopSession, UpdateConfig
  - Request/response DTOs for HTTP

- **`udp_protocol.rs`**
  - UDP telemetry frame parsers (AC, F1 25, iRacing telemetry)

**Total Lines:** 1,613
**Key Abstractions:**
- Serialization via `serde` (all types must serialize/deserialize for WebSocket and HTTP)
- SimType → Pod simulation mapping
- Billing session lifecycle

---

### 2. **rc-core** (Central Server)
**Location:** `crates/rc-core/src`
**Binary:** `racecontrol` (port 8080)
**Purpose:** Central orchestrator for all venue operations

#### Key Modules:

**Core Infrastructure (1,500+ LOC)**
- **`main.rs`** (253 lines)
  - Axum web server initialization
  - mDNS pod discovery
  - Tracing/logging setup
  - Database initialization
  - JWT middleware for auth

- **`state.rs`** (202 lines)
  - `AppState`: Global mutable state
  - Pod registry (HashMap<String, PodInfo>)
  - WebSocket multiplexer (agent_senders, dashboard broadcast)
  - Billing, game launcher, AC server managers
  - Rate limiting & OTP tracking

- **`config.rs`** (320 lines)
  - Config struct: venue, server, database, cloud, pods, auth, AI debugger, AC server
  - Loads from `racecontrol.toml` or env vars
  - Defaults for development

- **`db/mod.rs`** (60+ lines)
  - SQLite pool initialization
  - Schema migration (drivers, pods, sessions, laps, billing, wallets)
  - WAL mode enabled for concurrency
  - Foreign key constraints enabled

**WebSocket & Real-time (60+ lines)**
- **`ws/mod.rs`**
  - `agent_ws`: Pod agent connection handler
    - Registers pod, creates mpsc channel
    - Forwards CoreToAgentMessage commands back to agents
    - Listens for AgentMessage (heartbeat, telemetry, lap data)
  - `dashboard_ws`: Client connection handler
    - Subscribes to broadcast DashboardEvent stream
    - Authorizes via JWT
  - Stale connection cleanup via conn_id atomics

**Pod Management (927+ LOC)**
- **`pod_monitor.rs`** (234 lines)
  - Monitors pod heartbeats
  - Detects offline/crashed pods
  - Updates pod status in registry

- **`pod_healer.rs`** (693 lines)
  - Automatic recovery for crashed/stalled pods
  - WoL (Wake-on-LAN) for powered-down pods
  - Restart strategies based on failure mode
  - Protected process list (explorer.exe, dwm.exe, rc-agent.exe, pod-agent.exe)

- **`pod_reservation.rs`** (229 lines)
  - Tracks pod availability
  - Allocates pods to sessions
  - Handles pod transitions (idle → in_session → idle)

- **`wol.rs`** (88 lines)
  - Wake-on-LAN magic packet generation

**Billing & Accounting (1,700+ LOC)**
- **`billing.rs`** (1,434 lines)
  - Core billing engine
  - Session-based billing (30min/₹700, 60min/₹900, 5min trial, 10s idle cutoff)
  - Pause/resume, refunds, splits
  - Tiered pricing with early-bird & bulk discounts
  - Rate limits prevent oversell

- **`accounting.rs`**
  - Revenue reconciliation
  - Payment tracking
  - Financial reporting

- **`wallet.rs`** (267 lines)
  - Customer wallet/credit management
  - Recharge history

**Game Management (454+ lines)**
- **`game_launcher.rs`**
  - Launch/stop game instances on pods
  - Telemetry subscription
  - Crash detection & automatic restart

- **`ac_server.rs`**
  - Assetto Corsa server lifecycle (start/stop)
  - LAN session management
  - Grid setup, qualifying/race modes

- **`ac_camera.rs`**
  - Dahua security camera control
  - RTSP subtype=1 streaming
  - Pan/tilt/zoom for spectating

- **`catalog.rs`** (354 lines)
  - AC track/car catalog (36 tracks, 325 cars)
  - Difficulty presets
  - Experience metadata

**Session & Lap Tracking (150+ LOC)**
- **`lap_tracker.rs`**
  - Lap completion detection from telemetry
  - Sector times, best lap tracking
  - Grid/session metadata

- **`multiplayer.rs`** (1,009 lines)
  - Group sessions (friends, tournaments, casual)
  - Scoring rules
  - Leaderboard generation
  - Tournament state machine

**Authentication & Authorization (150+ LOC)**
- **`auth/mod.rs`**
  - JWT generation/validation
  - PIN authentication (4-digit lock screen)
  - OTP via SMS (Evolution API)
  - Session token management

**AI & Debugging (693+ LOC)**
- **`ai.rs`**
  - AI debugging prompt generation
  - Ollama local LLM integration (venue-only)
  - Anthropic API fallback
  - Proactive error analysis

- **`error_aggregator.rs`** (301 lines)
  - Collects API errors across endpoints
  - 5-minute rolling buckets
  - Escalates high-frequency errors to AI

- **`remote_terminal.rs`** (184 lines)
  - SSH-like command execution on pods
  - PIN-based authentication
  - Session token with 24h expiry

**Data Synchronization (828+ LOC)**
- **`cloud_sync.rs`**
  - Pull drivers, pricing, tournaments from cloud every 30s
  - Push local billing/lap/session data
  - UUID mismatch resolution (local vs cloud)
  - Turso/SQLite cloud database support

**Scheduling & Utilities**
- **`scheduler.rs`** (478 lines)
  - Periodic tasks (pod healer, cloud sync, watchdog)
  - Cron-like scheduling
  - Exponential backoff for failures

- **`friends.rs`** (297 lines)
  - Friend list management
  - Group invites
  - Multiplayer session coordination

- **`activity_log.rs`**
  - Audit trail for pod events

**API Routes**
- **`api/mod.rs`** (routes::api_routes)
- **`api/routes.rs`** (100+ lines)
  - REST endpoints organized by domain:
    - `/health` - server status
    - `/pods/*` - pod lifecycle (list, register, wake, shutdown, enable, disable, restart)
    - `/drivers/*` - driver CRUD
    - `/sessions/*` - session management
    - `/laps/*` - lap data
    - `/leaderboard/*` - rankings
    - `/billing/*` - billing operations
    - `/games/*` - game launch/stop
    - `/ac/*` - AC server & content
    - `/auth/*` - PIN/OTP assignment
    - `/pricing/*` - pricing tier management

**Total rc-core Lines:** ~18,728 (across 30 modules)

---

### 3. **rc-agent** (Pod Client)
**Location:** `crates/rc-agent/src`
**Binary:** `rc-agent` (runs on each pod, connects to rc-core via WebSocket)
**Purpose:** On-pod game lifecycle, wheelbase monitoring, AI auto-fix

#### Key Modules:

**Initialization & Lifecycle (100+ LOC)**
- **`main.rs`** (100 lines)
  - TOML config load (`rc-agent-podX.toml`)
  - WebSocket client connection to rc-core
  - Spawns 5 main task channels: driving_detector, game_process, ai_debugger, lock_screen, overlay

**Game & Process Management (454+ LOC)**
- **`game_process.rs`**
  - Track game executable state (launched, running, crashed, stopped)
  - Named pipe communication for game state
  - Crash detection via exit code or process timeout
  - Auto-restart on crash (with cooldown)

- **`ac_launcher.rs`**
  - Assetto Corsa launch via Content Manager
  - FORCE_START=1, HIDE_MAIN_MENU=1 in gui.ini
  - Telemetry UDP listener setup
  - Track/car pre-configuration

- **`sims/mod.rs`** (SimAdapter trait)
  - `SimAdapter` trait: connect, is_connected, read_telemetry, poll_lap_completed, session_info, disconnect
  - Implementations: assetto_corsa.rs, f1_25.rs

- **`sims/assetto_corsa.rs`**
  - AC UDP telemetry parser
  - Physics update frequency (default 100 Hz)
  - Session state machine (practice → qualifying → race)

- **`sims/f1_25.rs`**
  - F1 25 UDP telemetry parser
  - ERS energy management state

**Wheelbase & Input Monitoring (60+ LOC)**
- **`driving_detector.rs`**
  - USB HID for Conspit Ares 8Nm wheelbase (VID:0x1209 PID:0xFFB0)
  - Detects steering/throttle/brake input
  - Reports `DrivingState` to rc-core
  - 10-second idle threshold for billing cutoff

- **`udp_heartbeat.rs`**
  - Sends/receives heartbeat UDP packets
  - Pod ↔ rc-core connectivity check

**Lock Screen & Authentication (80+ LOC)**
- **`lock_screen.rs`**
  - Fullscreen "Secure Exit" UI
  - QR code generation for customer auth
  - PIN validation via rc-core
  - Blocks customer access to system settings
  - Exit button enabled only on valid PIN/QR scan

**UI & Display (100+ LOC)**
- **`kiosk.rs`**
  - Fullscreen kiosk window management
  - Display manager for multi-monitor setup
  - Prevents taskbar/window switching

- **`overlay.rs`**
  - In-game HUD overlay (speed, timing, boost)
  - TCP server (port 9000) for overlay communication
  - Ready/sync with game launch

**AI Debugging & Auto-Fix (693+ LOC)**
- **`ai_debugger.rs`**
  - Captures `PodStateSnapshot` at crash time
  - Calls Ollama (local on James's GPU) or Claude API
  - Auto-fix suggestions:
    - Stale socket cleanup
    - Game process management
    - Temp file cleanup (%temp%)
    - WerFault (Windows error reporting) cleanup
  - Protected process list prevents OS/system damage
  - Execution via `tokio::spawn_blocking` with 60s timeout
  - Pipeline logging for debugging

**Debug Server (80+ LOC)**
- **`debug_server.rs`**
  - HTTP debug endpoint (port 8090)
  - Pod diagnostics
  - Live state introspection

**Total rc-agent Lines:** ~1,400 (across 13 modules)

---

## Data Flow Architecture

### Pod Lifecycle Flow
```
1. Pod boots → rc-agent starts
2. rc-agent connects to rc-core via WebSocket
3. rc-agent sends Register(PodInfo) → core adds to pod registry
4. core broadcasts DashboardEvent("pod_registered") to kiosk/pwa

5. Customer books session → core sends StartSession to rc-agent
6. rc-agent launches game, initializes wheelbase, lock screen
7. Game runs → telemetry UDP → rc-agent reads & sends Telemetry frames
8. rc-agent monitors DrivingState, sends updates
9. Game detects lap → rc-agent sends LapCompleted

10. Customer exits → lock screen requires PIN
11. PIN validated → core sends StopSession
12. rc-agent shuts game, resets pod → billing stops
13. Pod idle → ready for next session

14. If game crashes → rc-agent auto-detects crash
    → sends crash report to rc-core
    → rc-core triggers ai_debugger
    → Ollama analyzes pod state & suggests fixes
    → rc-agent executes auto-fix (if safe)
    → Game auto-restarts
```

### Billing Flow
```
1. Customer starts session → session_id, pod_id, duration generated
2. Billing engine calculates tier (trial/30min/60min) & rate
3. Timer starts → every 10s: check driving_state
4. If idle > 10s → pause billing (customer can resume)
5. If driving → accrue to billing_session
6. Customer ends session → final cost calculated
7. Deduct from wallet (cloud-synced)
8. Log to activity_log for audit
```

### Cloud Sync Flow
```
Every 30s:
1. rc-core pulls drivers, pricing, tournaments from cloud
2. rc-core pushes billing sessions, lap times to cloud
3. ID mismatch resolution: match by phone+email
4. Local data always authoritative for billing/laps
5. Cloud data always authoritative for drivers/pricing
```

---

## WebSocket Protocol

### Pod Agent ↔ rc-core (agent_ws)

**Pod → Core: `AgentMessage`**
```
Register(PodInfo)        // Pod boots, announces itself
Heartbeat(PodInfo)       // Every 5s, pod status update
Telemetry(TelemetryFrame)// Game physics frame (~100 Hz)
LapCompleted(LapData)    // Lap detected
SessionUpdate(SessionInfo)// Session state change
DrivingStateUpdate       // Steering/throttle detected
GameStateUpdate          // Game launched/crashed/stopped
AiDebugResult            // Auto-fix suggestion result
PinEntered { pod_id, pin }// Lock screen PIN entry
Disconnect { pod_id }    // Pod shutting down
```

**Core → Pod: `CoreToAgentMessage`**
```
Registered { pod_id }    // Ack pod registration
StartSession(SessionInfo)// Launch game with these settings
StopSession { session_id }// Graceful shutdown
UpdateConfig             // Update rc-agent config
SetScreen { mode }       // Wake/lock/blank display
```

### Dashboard Client ↔ rc-core (dashboard_ws)

**Core → Client: `DashboardEvent`**
```
PodStatusUpdate(PodInfo) // Pod status changed
TelemetryFrame          // Live telemetry
LapCompleted            // Real-time lap results
BillingUpdate           // Session cost updated
SessionStatusUpdate     // Session state
ChatMessage             // AI debugging chat
```

**Client → Core: `DashboardCommand`**
```
Subscribe { pod_id }    // Watch specific pod
Unsubscribe { pod_id }  // Stop watching
SendChatMessage { text }// AI debug chat input
```

---

## API Endpoint Organization

### Pod Management
- `GET /pods` → list all pods
- `GET /pods/{id}` → pod details
- `POST /pods/{id}/wake` → power on
- `POST /pods/{id}/shutdown` → power off
- `POST /pods/{id}/restart` → restart
- `POST /pods/{id}/enable` → enable pod (unmask from disabled list)
- `POST /pods/{id}/disable` → disable pod (prevent auto-recovery)

### Driver Management
- `GET /drivers` → all drivers
- `GET /drivers/{id}` → driver profile + stats
- `POST /drivers` → register new driver
- `GET /drivers/{id}/full-profile` → full history

### Session & Booking
- `GET /sessions` → active/past sessions
- `POST /sessions` → create new session
- `GET /sessions/{id}` → session details
- `POST /bookings` → create booking

### Billing
- `POST /billing/start` → start billing session
- `GET /billing/active` → current active sessions
- `POST /billing/{id}/stop` → end session & charge
- `POST /billing/{id}/pause` → pause (idle)
- `POST /billing/{id}/extend` → add time
- `POST /billing/{id}/refund` → refund session
- `GET /billing/report/daily` → revenue report

### Games
- `POST /games/launch` → launch game on pod
- `POST /games/stop` → stop game
- `GET /games/active` → running games
- `GET /games/pod/{pod_id}` → game state on pod

### AC Server
- `GET /ac/presets` → saved track/car combos
- `POST /ac/session/start` → start LAN session
- `POST /ac/session/stop` → end session
- `GET /ac/content/tracks` → all tracks (36)
- `GET /ac/content/cars` → all cars (325)

### Authentication
- `POST /auth/assign` → assign PIN to customer
- `POST /auth/cancel/{id}` → revoke PIN
- `GET /auth/pending` → pending PIN requests

### Leaderboard & Stats
- `GET /leaderboard/{track}` → top times on track
- `GET /sessions/{id}/laps` → lap times for session
- `GET /laps` → all lap records

---

## Frontend Architecture

### 1. **Kiosk (In-Venue, Port 3300)**
**Location:** `kiosk/src`
**Purpose:** On-site reception rig management & customer onboarding

**Framework:** Next.js 16 + React 19 + TailwindCSS

**Key Pages:**
- `page.tsx` - Home/splash
- `book/page.tsx` - Experience booking (track/car/difficulty selector)
- `pod/[number]/page.tsx` - Individual pod control panel
- `control/page.tsx` - Master pod array control
- `debug/page.tsx` - Staff debugging interface
- `settings/page.tsx` - Venue configuration
- `spectator/page.tsx` - Live spectating feeds
- `staff/page.tsx` - Staff login & PIN management

**Key Components:**
- `KioskPodCard.tsx` - Pod status card
- `ExperienceSelector.tsx` - Track/car/difficulty picker
- `DriverRegistration.tsx` - QR + PIN entry
- `LiveTelemetry.tsx` - Real-time speed, gear, throttle
- `SessionTimer.tsx` - Billing countdown
- `F1Speedometer.tsx` - RPM gauge
- `GameConfigurator.tsx` - Game settings preset manager

**Hooks:**
- `useKioskSocket.ts` - WebSocket to rc-core dashboard endpoint
- `useSetupWizard.ts` - Multi-step experience setup state

**API:**
- `lib/api.ts` - HTTP client for REST endpoints

### 2. **PWA (Mobile Customer, Port 3100)**
**Location:** `pwa/src`
**Purpose:** Mobile customer experience (booking, stats, friends)

**Framework:** Next.js 16 + React 19 + Recharts (graphs)

**Key Pages:**
- `page.tsx` - Dashboard
- `book/page.tsx` - Booking interface
- `book/active/page.tsx` - Active sessions
- `book/group/page.tsx` - Group/tournament booking
- `sessions/page.tsx` - Session history
- `sessions/[id]/page.tsx` - Session detail + lap replay
- `leaderboard/page.tsx` - Global rankings
- `leaderboard/public/page.tsx` - Public leaderboard
- `stats/page.tsx` - Personal statistics
- `telemetry/page.tsx` - Live telemetry visualization
- `coaching/page.tsx` - AI coaching insights
- `friends/page.tsx` - Friend management
- `profile/page.tsx` - User profile & settings
- `login/page.tsx` - QR scan + OTP login
- `register/page.tsx` - New driver signup
- `scan/page.tsx` - QR scanner for pod pairing
- `tournaments/page.tsx` - Event/tournament browser
- `ai/page.tsx` - AI debugging chat interface
- `terminal/page.tsx` - Web terminal for Uday (staff only)

**Components:**
- `SessionCard.tsx` - Session summary
- `TelemetryChart.tsx` - Lap visualization (recharts)
- `BottomNav.tsx` - Mobile navigation

### 3. **Web Dashboard (Admin, Port 3200)**
**Location:** `web/src` (legacy, may be deprecated)
**Purpose:** Admin panel for operations

**Framework:** Next.js + React + TailwindCSS

---

## Key Data Structures

### PodInfo
```rust
pub struct PodInfo {
    pub id: String,                     // UUID
    pub number: u32,                    // Pod 1-8
    pub name: String,                   // "Pod 1 - AC"
    pub ip_address: String,             // 192.168.31.x
    pub mac_address: Option<String>,    // For WoL
    pub sim_type: SimType,              // AssettoCorsa, F1_25, etc
    pub status: PodStatus,              // Offline, Idle, InSession, Error, Disabled
    pub current_driver: Option<String>, // Driver UUID
    pub current_session_id: Option<String>,
    pub last_seen: Option<DateTime<Utc>>,
    pub driving_state: Option<DrivingState>,
    pub billing_session_id: Option<String>,
    pub game_state: Option<GameState>,
    pub current_game: Option<SimType>,
}
```

### TelemetryFrame
```rust
pub struct TelemetryFrame {
    pub pod_id: String,
    pub timestamp: DateTime<Utc>,
    pub speed_kmh: f32,
    pub throttle: f32,      // 0.0-1.0
    pub brake: f32,
    pub steering: f32,      // -1.0 to +1.0
    pub gear: i32,
    pub rpm: f32,
    pub fuel: f32,
    pub lap_distance: f32,  // meters in lap
    pub track_temperature: f32,
    pub car_id: String,
    pub track_id: String,
}
```

### BillingSessionInfo
```rust
pub struct BillingSessionInfo {
    pub id: String,                 // UUID
    pub driver_id: String,
    pub pod_id: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub paused_at: Option<DateTime<Utc>>,
    pub duration_ms: u64,           // Billable time
    pub idle_ms: u64,               // Paused time (not billed)
    pub cost_credits: u64,          // Final cost
    pub pricing_tier_id: String,    // Which pricing was used
    pub status: BillingStatus,      // Active, Paused, Completed, Refunded
    pub refund_reason: Option<String>,
}
```

---

## Module Dependencies

```
rc-common (types + protocol)
  ├── serde
  ├── chrono
  └── uuid

rc-core (central server)
  ├── rc-common
  ├── axum (web framework)
  ├── tower-http (cors, fs, trace)
  ├── sqlx (sqlite async)
  ├── mdns-sd (pod discovery)
  ├── tokio-tungstenite (websocket)
  ├── jsonwebtoken (JWT)
  ├── reqwest (HTTP client)
  └── toml (config)

rc-agent (pod client)
  ├── rc-common
  ├── tokio-tungstenite (websocket to core)
  ├── hidapi (USB wheelbase HID)
  ├── sysinfo (process monitoring)
  ├── mdns-sd (discover core)
  ├── qrcode (lock screen QR)
  ├── dirs-next (Documents path)
  ├── winapi (Windows process mgmt)
  └── reqwest (AI debugger HTTP)

kiosk (next.js frontend)
  ├── next 16
  ├── react 19
  └── tailwindcss 4

pwa (next.js frontend)
  ├── next 16
  ├── react 19
  ├── tailwindcss 4
  ├── html5-qrcode (QR scanner)
  └── recharts (charting)
```

---

## Configuration & Environment

**Config File:** `C:\Users\bono\racingpoint\racecontrol\racecontrol.toml`

**Sections:**
- `[venue]` - Name, location, timezone
- `[server]` - Host (0.0.0.0), port (8080)
- `[database]` - SQLite path (./data/racecontrol.db)
- `[cloud]` - Cloud API URL, sync interval (30s), Turso credentials
- `[pods]` - Pod count (8), discovery enabled, healer interval (120s)
- `[branding]` - Primary color (#E10600), theme (dark)
- `[ai_debugger]` - Ollama URL, model name, Anthropic API key
- `[ac_server]` - AC server path, data directory
- `[auth]` - JWT secret, OTP/PIN expiry (600s, 300s)
- `[integrations]` - Discord webhook, WhatsApp contact
- `[watchdog]` - Heartbeat timeout (30s), restart cooldown (120s)

---

## Performance & Scaling

- **WebSocket Multiplexing:** 8 pods + unlimited clients on single rc-core instance
- **Database:** SQLite with WAL mode, 5 connection pool
- **Cloud Sync:** 30s interval pull/push, ID resolution by phone+email
- **Billing:** Sub-10ms calculation per session state check
- **Telemetry:** 100 Hz UDP frames from game, ~1000/s ingestion capacity
- **Pod Healing:** Every 120s, monitors heartbeat, auto-recovers stalled pods

---

## Testing & Validation

**Unit Tests:** 47 tests across 3 crates
- Protocol serialization (rc-common)
- Driving detector logic (rc-agent)
- Billing calculations (rc-core)
- Cloud sync ID resolution (rc-core)

**Integration:**
- Pod registration → core registry
- Heartbeat → status update
- Session start → game launch
- Billing → cost calculation & refund

---

## Deployment & CI/CD

**Binaries:**
- `rc-core` (racecontrol.exe) - Central server
- `rc-agent` (rc-agent.exe) - Pod client (x1 per pod)
- `pod-agent` (pod-agent.exe) - Pod HTTP daemon for remote deploy

**Artifacts:**
- Kiosk: Next.js build (`.next/`)
- PWA: Next.js build (`.next/`)
- Docker: Dockerfile for cloud deployment (Bono's VPS)

**Deploy Kit:** `D:\pod-deploy\` on James's machine
- Includes: install.bat, binaries, configs, watchdog setup
- Usage: `install.bat <pod_number>` on each pod

---

## Key Design Decisions

1. **Three Rust Crates:** Separation of concerns (types, server, agent)
2. **SQLite:** Venue-local authoritative for billing; no cloud dependency required
3. **WebSocket + HTTP:** Real-time events (WS) + request-response (HTTP)
4. **mDNS Discovery:** Pod → Core discovery without static IPs
5. **Ollama Local GPU:** AI debugging on venue (James's RTX 4070), not cloud
6. **Protected Processes:** Auto-fix never kills explorer.exe, dwm.exe, rc-agent.exe
7. **Cloud Sync Eventual Consistency:** 30s pull/push with local-first fallback
8. **Billing Timer:** 10s idle threshold to allow customer brief pauses
9. **Multi-Sim Support:** Abstract SimAdapter trait for AC, F1 25, iRacing, etc.
10. **Event-Driven UI:** Dashboard updates via broadcast channel, not polling
