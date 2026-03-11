# RaceControl Directory Structure & File Locations

## Root Layout

```
/root/racecontrol/
├── Cargo.toml                    # Workspace definition (3 crates)
├── Cargo.lock                    # Dependency lock
├── crates/                       # Rust crates
│   ├── rc-common/                # Shared types and protocol
│   ├── rc-core/                  # Central HTTP server (port 8080)
│   └── rc-agent/                 # Gaming PC agent
├── pwa/                          # Next.js customer web app
├── kiosk/                        # Next.js staff kiosk interface
├── web/                          # Legacy web assets (CSS, static)
├── .planning/                    # Architecture documentation
├── docs/                         # Technical guides
├── deploy/                       # Docker, systemd configs
├── scripts/                      # Build and deploy scripts
├── pod-scripts/                  # Scripts for pod gaming PCs
├── data/                         # Runtime databases, caches
├── target/                       # Compiled Rust binaries (debug/release)
└── training/                     # Training materials, test data
```

---

## Rust Workspace: 3 Crates

### 1. rc-common: Shared Protocol & Types

**Location**: `crates/rc-common/src/`

**Purpose**: Single source of truth for data structures and messages shared between rc-core and rc-agent.

**Files**:

- **`lib.rs`**: Module re-exports
  ```rust
  pub mod types;
  pub mod protocol;
  pub mod udp_protocol;
  ```

- **`types.rs`** (~200 lines)
  - `SimType` enum: AssettoCorsa, AssettoCorsaEvo, IRacing, LeMansUltimate, F125, Forza
  - `PodInfo` struct: Pod identity, status, current driver, game state
  - `Driver` struct: Customer profile (id, name, email, phone, steam_guid, iracing_id, stats)
  - `SessionType` enum: Practice, Qualifying, Race, Hotlap
  - `SessionStatus` enum: Pending, Active, Completed, Cancelled
  - `DrivingState` enum: Active, Idle, Stopped (from HID wheelbase input)
  - `GameState` enum: Loading, Menu, Paused, Racing, Finished (from telemetry)
  - `PodStatus` enum: Offline, Idle, InSession, Error, Disabled
  - `PodConfig` struct: Wheelbase settings, game paths, telemetry ports

- **`protocol.rs`** (~150 lines)
  - `CoreMessage` enum (server → agent):
    - `LaunchGame { pod_id, sim_type, experience_id }`
    - `StopGame { pod_id }`
    - `SetLockScreen { pod_id, state }`
    - `ShowOverlay { pod_id, widget_type, data }`
    - `UpdatePodConfig { pod_id, config }`
    - `Ping`
  - `AgentMessage` enum (agent → server):
    - `PodStateUpdate { pod_id, state }`
    - `TelemetryFrame { pod_id, frame_data }`
    - `Lap { pod_id, lap_data }`
    - `SessionEvent { pod_id, event }`
    - `Pong`

- **`udp_protocol.rs`** (~200 lines)
  - Frame deserialization for F1 25, AC, iRacing, Forza, LMU
  - Lap detection logic (sector times, session state)
  - Fuel/tire/damage parsing for telemetry overlay

**Cargo.toml** (`crates/rc-common/Cargo.toml`):
```toml
[package]
name = "rc-common"
version.workspace = true
edition.workspace = true

[dependencies]
serde.workspace = true
serde_json.workspace = true
chrono.workspace = true
uuid.workspace = true
tokio.workspace = true
```

---

### 2. rc-core: Central HTTP Server

**Location**: `crates/rc-core/src/`

**Port**: 8080 (default)

**Purpose**: RESTful API, WebSocket hub, billing engine, cloud sync orchestrator.

**Cargo.toml** (`crates/rc-core/Cargo.toml`):
```toml
[package]
name = "rc-core"
version.workspace = true

[dependencies]
rc-common.workspace = true
axum = "0.7"
tokio.workspace = true
serde.workspace = true
serde_json.workspace = true
chrono.workspace = true
uuid.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
tower-http = { version = "0.5", features = ["cors", "trace"] }
jsonwebtoken.workspace = true
sqlx = { version = "0.8", features = ["sqlite", "runtime-tokio"] }
reqwest = { version = "0.12", features = ["json"] }
futures.workspace = true
rand.workspace = true
anyhow.workspace = true
thiserror.workspace = true
```

**Core Modules** (in `src/`):

- **`main.rs`** (~300 lines)
  - Entry point
  - Loads `racecontrol.toml` config
  - Initializes SQLite database
  - Spawns background tasks (pod_monitor, cloud_sync, scheduler)
  - Starts Axum router on port 8080
  - Configures middleware (CORS, tracing, JWT error handling)
  - Listens for WebSocket upgrades at `/ws`

- **`state.rs`** (~150 lines)
  - `AppState` struct: In-memory pod state, database, cloud client, active sessions, action queue
  - Wrapped in `Arc<RwLock<AppState>>` for thread-safe access
  - Methods: `get_pod()`, `update_pod()`, `get_active_session()`, `queue_action()`

- **`api/`** — REST endpoint handlers
  - **`mod.rs`**: Module re-exports
  - **`routes.rs`** (~2000 lines)
    - **Customer routes** (`/customer/`):
      - `GET /customer/me` — Current user profile + wallet balance
      - `GET /customer/sessions` — Booking history with lap data
      - `GET /customer/sessions/{id}` — Session detail + telemetry graph
      - `GET /customer/sessions/{id}/share` — Shareable session report (percentile, consistency, PB)
      - `POST /customer/book` — Initiate 8-step booking wizard
      - `GET /customer/ac/catalog` — 36 featured AC tracks/cars
      - `GET /customer/packages` — Preset packages (Date Night, Birthday, Corporate, Squad, Student)
      - `GET /customer/membership` — Current tier + hours tracking
      - `POST /customer/membership` — Subscribe to membership tier
    - **Pod routes** (`/pods/`):
      - `GET /pods` — List all pods with current state
      - `GET /pods/{id}` — Pod detail (status, driver, game, installed_games)
      - `POST /pods/{id}/launch` — Start game on pod
      - `POST /pods/{id}/stop` — Kill game (staff only)
      - `POST /pods/{id}/screen` — Per-pod blanking (lock screen state)
      - `GET /pods/{id}/metrics` — Pod uptime, session count, avg play time
    - **Billing routes** (`/billing/`):
      - `POST /billing/session/start` — Begin charging for drive time
      - `POST /billing/session/end` — Stop charging, compute final bill
      - `GET /billing/pricing` — Dynamic pricing for current time
      - `GET /billing/history` — Customer's past bills
    - **Terminal routes** (`/terminal/`):
      - `POST /terminal/auth` — Verify 4-digit PIN for staff access
      - `POST /terminal/{pod_id}/launch` — Direct game launch (kiosk staff)
      - `POST /terminal/{pod_id}/stop` — Direct game kill
    - **Admin routes** (`/admin/`):
      - `GET /admin/dashboard` — Venue KPIs
      - `POST /admin/pricing/rules` — Configure dynamic pricing
      - `POST /admin/coupons` — Create discount codes
      - `GET /admin/audit-log` — Double-entry bookkeeping audit trail
    - **Public routes**:
      - `GET /public/leaderboard` — Top 100 all-time
      - `GET /public/leaderboard/{track}` — Top 100 per track
      - `GET /public/time-trial` — Weekly time trial leaderboard

- **`auth/`** — JWT authentication
  - **`mod.rs`** (~100 lines)
    - `JwtClaims` struct: user_id, role, exp, iat
    - `generate_jwt()` — Create token on login (24h expiry)
    - `verify_jwt()` middleware — Validate signature, expiry
    - `get_current_user()` extractor — Extract user_id from header
    - `compute_employee_pin()` — Daily rotating PIN from hash(jwt_secret + date)

- **`billing.rs`** (~400 lines)
  - `BillingSession` struct: Pod, driver, timestamps, drive_time_ms, idle_time_ms, paise_charged
  - `start_session()` — Create billing row, debit wallet at session START (not end)
  - `end_session()` — Finalize session, write total paise charged
  - `compute_dynamic_price()` — Apply multipliers from pricing_rules table
  - Timer loop (100ms tick): Accumulate drive_time or idle_time based on DrivingState
  - Post-session hooks: referral rewards (₹100 referrer, ₹50 referee), review nudge scheduling, membership hours
  - SQL queries: insert billing_sessions, update wallets, query pricing_rules

- **`config.rs`** (~80 lines)
  - `Config` struct with sections:
    - `[venue]`: name, location, timezone
    - `[server]`: host, port (default 127.0.0.1:8080)
    - `[auth]`: jwt_secret, employee_pin_base
    - `[database]`: path (default /tmp/racecontrol.db)
    - `[cloud]`: api_url, sync_interval_secs
    - `[games]`: paths to game executables
  - Reads from `racecontrol.toml` or falls back to defaults
  - Validation: Warn if default JWT secret unchanged

- **`db/mod.rs`** (~200 lines)
  - SQLite connection pool abstraction
  - `Database` struct wrapping sqlx::SqlitePool
  - `initialize_db()` — Create tables if not exist (idempotent)
  - Tables created:
    - `pods` (id, name, ip, mac, sim_type, installed_games, status, ...)
    - `drivers` (id, name, email, phone, steam_guid, iracing_id, total_laps, ...)
    - `billing_sessions` (id, pod_id, driver_id, start_time, end_time, drive_time_ms, paise_charged, ...)
    - `personal_bests` (driver_id, track, car, lap_time_ms, sectors, timestamp)
    - `track_records` (track, car, lap_time_ms, driver_id, timestamp)
    - `pricing_rules` (id, day_of_week, hour_start, hour_end, multiplier, is_active)
    - `coupons` (id, code, discount_type, value, is_active, created_at)
    - `wallets` (driver_id, balance_paise, updated_at)
    - `journal_entries` (id, account_id, debit_paise, credit_paise, timestamp)
    - `audit_log` (id, table_name, record_id, operation, before, after, timestamp)
    - `referrals` (id, referrer_id, referee_id, claimed_at, reward_paise)
    - `review_nudges` (id, driver_id, created_at, completed_at, reward_claimed)

- **`ws/mod.rs`** (~300 lines)
  - WebSocket handler
  - Endpoint: `GET /ws?pod_id={id}`
  - Message loop: Receive CoreMessage, forward to action_queue; Receive AgentMessage, broadcast to clients
  - Broadcast channels: `broadcast::channel()` per pod
  - On connect: Subscribe to pod's broadcast channel
  - On disconnect: Unsubscribe
  - Heartbeat: Send `Ping` every 30s, expect `Pong` or flag as offline

- **`cloud_sync.rs`** (~400 lines)
  - Runs on background task spawned in main.rs
  - Pull (Cloud → Venue) every 30s:
    - Drivers, wallets, pricing_tiers, pricing_rules, kiosk_experiences, kiosk_settings
  - Push (Venue → Cloud) every 30s:
    - Laps, track_records, personal_bests, billing_sessions, pods, drivers, wallets, wallet_transactions
  - CRDT conflict resolution: MAX(updated_at) — newest write wins
  - HTTP client: reqwest for cloud API calls
  - Retry logic: Exponential backoff on 5xx

- **`pod_monitor.rs`** (~150 lines)
  - Background task spawned in main.rs
  - Heartbeat checker: Runs every 2 minutes
  - Logic: If no message from pod in 2 minutes, set status to Offline
  - Status transitions: Idle ↔ InSession, Error on crash
  - Auto-recovery trigger: If offline, call wol::send_magic_packet()

- **`pod_healer.rs`** (~100 lines)
  - Detects crashed games, orphaned processes
  - Sends `StopGame` on detection
  - Restarts rc-agent via WoL if process dead

- **`game_launcher.rs`** (~200 lines)
  - Orchestrates game launch flow
  - Receives `POST /pods/{id}/launch` with booking details
  - Validates customer wallet balance (must be > session cost estimate)
  - Calls `billing::start_session()` (debits wallet immediately)
  - Queues `LaunchGame` message to rc-agent over WebSocket
  - Waits for `GameState::Racing` telemetry confirmation
  - Returns session ID to customer

- **`pod_reservation.rs`** (~250 lines)
  - 8-step booking wizard:
    1. Sim selection
    2. Track/Car selection
    3. Duration
    4. Difficulty
    5. Friends (invite to group booking)
    6. Customization (assists, damage, weather)
    7. Review
    8. Checkout (payment)
  - Checks pod availability for requested time slot
  - Checks customer wallet balance
  - Returns booking summary with cost estimate

- **`lap_tracker.rs`** (~200 lines)
  - Consumes `Lap` messages from rc-agent
  - Inserts into `personal_bests` and `track_records` tables
  - Computes percentile rank vs. same track/car
  - Broadcasts lap notifications to dashboard

- **`catalog.rs`** (~100 lines)
  - Hosts 36 featured AC tracks, 41 featured cars
  - Endpoints:
    - `GET /customer/ac/catalog` — All featured
    - `GET /customer/ac/tracks` — Tracks only
    - `GET /customer/ac/cars` — Cars only
  - Data hardcoded in Rust (could move to DB)

- **`friends.rs`** (~150 lines)
  - Friend request management (send, accept, reject, block)
  - Presence tracking (online, offline, in_session)
  - Group booking: Reserve multiple pods for team session

- **`multiplayer.rs`** (~100 lines)
  - Coordinate multiplayer lobbies (AC server mode)
  - Pod allocation for team events

- **`action_queue.rs`** (~80 lines)
  - FIFO queue: LaunchGame, StopGame, SetLockScreen
  - Prevents race conditions from simultaneous client commands
  - Consumer task processes one command at a time
  - Awaits WebSocket ack before next command

- **`scheduler.rs`** (~100 lines)
  - Background cron jobs:
    - Pricing rule updates (apply multipliers on schedule)
    - Referral payout batching
    - Membership hour resets (monthly)

- **`ai.rs`** (~150 lines)
  - Claude API integration for coaching
  - `POST /ai/coaching` — Sector deltas, tips
  - `POST /ai/chat` — Customer support chatbot

- **`accounting.rs`** (~200 lines)
  - Double-entry bookkeeping (LIVE as of Mar 9)
  - Auto-posts journal entries on every wallet credit/debit/refund
  - Account enum: Assets, Liabilities, Equity, Revenue, Expenses, etc.
  - Queries: `trial_balance()`, `profit_loss()`, `balance_sheet()`

- **`activity_log.rs`** (~80 lines)
  - Audit trail (LIVE as of Mar 9)
  - Auto-logs on pricing_tiers, pricing_rules, coupons CRUD
  - Before/after JSON snapshots
  - Query via `GET /admin/audit-log`

- **`wallet.rs`** (~200 lines)
  - Customer credit management
  - `add_credit()` — Increase balance
  - `debit()` — Deduct paise (for billing, cafe, merchandise)
  - `apply_coupon()` — Reduce balance by discount amount
  - Sync with cloud: upsert checks `updated_at` before overwriting

- **`udp_heartbeat.rs`** (~80 lines)
  - Listens on UDP ports (heartbeat from pods)
  - Receives telemetry frames from gaming PCs
  - Updates pod.last_seen timestamp

- **`wol.rs`** (~50 lines)
  - Wake-on-LAN packet generation and sending
  - MAC address lookup from PodInfo

- **`error_aggregator.rs`** (~100 lines)
  - Collects pod/game errors for alerting
  - Tracks error rates, thresholds

- **`ac_server.rs`** (~150 lines)
  - Assetto Corsa specifics: stracker protocol (UDP server list, live track state)

- **`ac_camera.rs`** (~100 lines)
  - AC camera control for replay clips

- **`remote_terminal.rs`** (~100 lines)
  - Staff terminal for kiosk access
  - PIN authentication (`POST /terminal/auth`)
  - Direct game launch without customer booking

---

### 3. rc-agent: Gaming PC Agent

**Location**: `crates/rc-agent/src/`

**Port**: 18923 (WebSocket to rc-core)

**Purpose**: Runs on each gaming PC. Manages game lifecycle, captures telemetry, handles UI overlays.

**Cargo.toml** (`crates/rc-agent/Cargo.toml`):
```toml
[package]
name = "rc-agent"
version.workspace = true

[dependencies]
rc-common.workspace = true
tokio.workspace = true
serde.workspace = true
serde_json.workspace = true
chrono.workspace = true
uuid.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
futures.workspace = true
tokio-tungstenite = "0.23"
anyhow.workspace = true
thiserror.workspace = true
hidapi = "2"
winapi = { version = "0.3", features = ["processthreadsapi", "winnt"] }
```

**Core Modules** (in `src/`):

- **`main.rs`** (~300 lines)
  - Entry point
  - Loads AgentConfig from TOML (pod ID, core server URL, game paths, wheelbase VID/PID)
  - Spawns threads for:
    - Game process manager
    - Lock screen UI
    - Overlay renderer
    - Kiosk UI
    - UDP telemetry listeners (6 ports in parallel)
    - Driving detector (HID wheelbase input)
    - WebSocket sender/receiver
  - Connects to rc-core WebSocket: `ws://core:8080/ws?pod_id={id}`

- **`game_process.rs`** (~400 lines)
  - `GameProcessManager` struct: Spawn, monitor, kill game processes
  - Methods:
    - `launch_game(sim_type, experience_id)` → Spawn EXE, set env vars
    - `poll_status()` → Check if process running, crashed, exited
    - `stop_game()` → Send SIGTERM/SIGKILL
    - Parse command line: `game.exe --track "Monza" --car "Ferrari" --difficulty "Expert"`
  - Tracks: game PID, CPU/memory usage, CPU affinity (pin to cores 0-3 for stability)

- **`sims/`** — Game-specific telemetry parsing
  - **`mod.rs`**: `SimAdapter` trait definition
    - `parse_frame(bytes: &[u8]) → TelemetryData`
    - `detect_lap(current, previous) → bool`
    - `extract_sectors(frame) → Vec<u64>`
  - **`assetto_corsa.rs`** (~300 lines): AC UDP protocol
    - Port: 9996
    - Packet format: speed, position, velocity, rpm, fuel, tire_temps, etc.
    - Lap detection: `lap_count` change
    - Sector detection: Waypoint array indices
  - **`f1_25.rs`** (~400 lines): F1 25 telemetry (COMPLETE as of Mar 5)
    - Port: 20777
    - Packet format: vehicle data, session data, event data
    - Lap detection: lap_distance resets
    - Sector detection: sector times in packet

- **`driving_detector.rs`** (~250 lines)
  - `DrivingDetector` struct: HID input monitoring
  - Methods:
    - `poll_input()` → Read OpenFFBoard steering, throttle, brake, clutch
    - `is_active()` → steering >5°, throttle >10%, brake >10%
    - `detect_idle_period()` → Flag inactive after 10s
  - HID API: Use hidapi crate for cross-platform USB access
  - Vendor ID: 0x1209, Product ID: 0xFFB0 (OpenFFBoard)
  - Polling: Every 10ms

- **`lock_screen.rs`** (~300 lines)
  - `LockScreenManager` struct: Display/hide fullscreen overlay
  - Rendering: Windows API (DirectX or GDI), Linux Xlib
  - Shows: Customer name, track, timer, fuel gauge, current speed
  - Receives `SetLockScreen` messages from rc-core

- **`overlay.rs`** (~250 lines)
  - `OverlayManager` struct: In-game telemetry overlay
  - Shows: Delta time, fuel estimate, lap times, sector splits
  - Updates from `TelemetryFrame` events
  - Renders via game-native overlay API or injected DLL (game-specific)

- **`kiosk.rs`** (~500 lines)
  - `KioskManager` struct: Staff interface
  - Features:
    - Staff PIN login (4-digit, verified against rc-core)
    - Pod status display (online/idle/in-session)
    - Quick launch buttons for games (AC, F1, iRacing, etc.)
    - Active session timer (elapsed drive time, estimated total)
    - Current fuel gauge, seat position, telemetry graph
  - Commands sent to rc-core: `LaunchGame`, `StopGame`, `ShowOverlay`
  - UI: Windows API or web-based (Electron or native webview)

- **`ai_debugger.rs`** (~150 lines)
  - AI coaching integration (optional)
  - Hooks into lap data
  - Sends queries to rc-core `/ai/coaching` endpoint
  - Displays coaching tips in overlay (delta time, braking points, gear selection)

- **`udp_heartbeat.rs`** (~80 lines)
  - Sends `Pong` to rc-core every 2 minutes over WebSocket
  - rc-core flags offline if no heartbeat for 6 minutes

- **`debug_server.rs`** (~100 lines)
  - Local HTTP server for debugging (port 3000)
  - Endpoint: `http://pod_ip:3000/status` returns PodInfo JSON
  - Used by James for on-site troubleshooting

- **`ac_launcher.rs`** (~150 lines)
  - Assetto Corsa launch specifics
  - Command line builder (track, car, difficulty, assists)

---

## Frontend: Next.js Applications

### 1. PWA: Customer Web App

**Location**: `/root/racecontrol/pwa/`

**Port**: 3500 (deployed)

**Framework**: Next.js 16, React 18

**Key Pages** (`pages/` or `app/` directory):
- `/` — Landing page
- `/login` — Authentication
- `/register` — Customer signup
- `/dashboard` — Home after login (active sessions, stats, leaderboard)
- `/book` — 8-step booking wizard
- `/book/active` — Current session (timer, telemetry, fuel gauge)
- `/book/group` — Group booking with friends
- `/sessions` — Session history
- `/sessions/[id]` — Session detail (telemetry graph, lap times, leaderboard rank, share button)
- `/stats` — Customer stats (total laps, best laps, lap distribution)
- `/leaderboard` — Personal leaderboard
- `/leaderboard/public` — Venue-wide public leaderboard
- `/telemetry` — Live telemetry dashboard (pod status, session timers)
- `/friends` — Friend list, requests, block list
- `/tournaments` — Join/view tournaments (bracket view, registration)
- `/coaching` — AI lap coaching (sector deltas, trends, tips)
- `/ai` — AI chatbot for support
- `/profile` — Customer profile (name, email, phone, stats)
- `/scan` — Scan QR code (referral, friend add, tournament join)
- `/terminal` — Staff terminal (PIN login, quick launch)

**APIs Called**:
- `GET /api/me` → rc-core `/customer/me`
- `POST /api/book` → rc-core `/customer/book`
- `GET /api/sessions` → rc-core `/customer/sessions`
- `GET /api/ac/catalog` → rc-core `/customer/ac/catalog`
- WebSocket to rc-core `/ws` for live pod status, telemetry

**Styling**: Tailwind CSS (or custom CSS in `/web/`)

**Environment**: `.env.local` contains rc-core API base URL

### 2. Kiosk: Staff UI

**Location**: `/root/racecontrol/kiosk/`

**Port**: 3400 (deployed as venue dashboard)

**Framework**: Next.js 16, React 18

**Purpose**: On-pod staff interface for game launching, session monitoring

**Key Features**:
- Staff PIN login (4-digit)
- Pod status grid (online/idle/in-session)
- Quick launch buttons (AC, F1, iRacing, Forza, LMU)
- Active session timer (elapsed time, estimated total)
- Current fuel, seat position, telemetry overlay
- Customer search by name/phone
- Bill generation (for cafe, merchandise)
- Bottom ticker (active sessions)

**APIs Called**:
- `POST /api/terminal/auth` → rc-core `/terminal/auth` (PIN verify)
- `POST /api/terminal/{pod_id}/launch` → rc-core `/terminal/{pod_id}/launch`
- `GET /api/pods` → rc-core `/pods`
- WebSocket to rc-core `/ws` for live updates

**Styling**: Custom CSS (on-site testing)

---

## Supporting Directories

### `/scripts/`
Build, deploy, and test scripts.

**Common files**:
- `build.sh` — Compile Rust (rc-core, rc-agent), Next.js (PWA, kiosk)
- `deploy.sh` — Copy binaries to cloud, venue servers
- `test.sh` — Run Rust unit tests
- `docker-build.sh` — Build Docker image for rc-core
- `setup.sh` — Initialize databases, create directories

### `/pod-scripts/`
Scripts run on each gaming PC.

**Common files**:
- `rc-agent-systemd.service` — systemd unit to start rc-agent on boot
- `install-rc-agent.sh` — Copy rc-agent binary, create directories
- `game-paths.toml` — Map SimType → game executable path on Windows

### `/deploy/`
Deployment configurations.

**Common files**:
- `docker-compose.yml` — Run rc-core + db in containers
- `nginx.conf` — Reverse proxy config (if using Nginx)
- `systemd/rc-core.service` — systemd unit for rc-core on cloud
- `env.example` — Template for environment variables

### `/docs/`
Technical documentation.

**Common files**:
- `API.md` — OpenAPI spec for rc-core REST endpoints
- `PROTOCOL.md` — WebSocket message format and examples
- `SETUP.md` — Local development setup (install Rust, Node.js, databases)
- `CLOUD-SYNC.md` — Cloud ↔ venue sync architecture

### `/data/`
Runtime databases and cache.

**Common files**:
- `racecontrol.db` — SQLite venue database (path configurable in racecontrol.toml)
- `racecontrol.toml` — Config file (venue name, server port, JWT secret, game paths)
- `.cloud-sync-state` — Last sync timestamp (JSON)

### `/assets/`
Static images, logos, icons.

**Common files**:
- `logo.png`, `favicon.ico`
- `track-thumbnails/` — Track preview images (AC)
- `car-thumbnails/` — Car preview images (AC)

### `/training/`
Training materials and test data.

**Common files**:
- `sample-sessions/` — Example lap data (JSON)
- `test-database-seed.sql` — SQL to populate test data
- `README.md` — Training guide for staff

### `/.planning/codebase/` (NEW)
Architecture and structure documentation.

**Files**:
- `ARCHITECTURE.md` — This document (patterns, layers, data flow)
- `STRUCTURE.md` — This document (directory layout, file locations)

---

## Configuration Files

### `racecontrol.toml` (Venue Config)

**Location**: `/root/racecontrol/data/racecontrol.toml` or `/etc/racecontrol/racecontrol.toml`

**Example**:
```toml
[venue]
name = "RacingPoint Hyderabad"
location = "Madhapur, Hyderabad, Telangana"
timezone = "Asia/Kolkata"

[server]
host = "127.0.0.1"
port = 8080

[auth]
jwt_secret = "your-secret-key-here-change-in-production"
employee_pin_base = "racing-point"

[database]
path = "/tmp/racecontrol.db"

[cloud]
api_url = "https://api.racingpoint.cloud"
sync_interval_secs = 30

[games]
assetto_corsa = "C:\\Program Files\\Assetto Corsa\\assettocorsa.exe"
f1_25 = "C:\\Program Files\\F1 25\\F1.exe"
iracing = "C:\\Program Files\\iRacing\\iRacingLauncher.exe"

[wheelbase]
vendor_id = "0x1209"
product_id = "0xFFB0"

[telemetry_ports]
ac = 9996
f1_25 = 20777
iracing = 6789
forza = 5300
lmu = 5555
```

### `Cargo.toml` (Rust Workspace)

**Location**: `/root/racecontrol/Cargo.toml`

Defines workspace members (rc-common, rc-core, rc-agent) and shared dependencies.

### `package.json` (PWA & Kiosk)

**Locations**:
- `/root/racecontrol/pwa/package.json`
- `/root/racecontrol/kiosk/package.json`

**Common scripts**:
- `npm run dev` — Start dev server (port 3500 for PWA, 3400 for kiosk)
- `npm run build` — Optimize for production
- `npm run start` — Run production build
- `npm test` — Jest tests (if configured)

---

## Naming Conventions

### Modules
- **snake_case** for filenames: `game_process.rs`, `pod_monitor.rs`
- **snake_case** for module names (Rust): `mod pod_monitor;`
- **PascalCase** for struct/type names: `PodInfo`, `BillingSession`, `GameState`
- **UPPERCASE** for constants: `DEFAULT_TIMEOUT_MS`, `MAX_PODS`

### Database
- **snake_case** for table names: `billing_sessions`, `track_records`
- **snake_case** for column names: `driver_id`, `started_at`, `paise_charged`

### Routes
- **kebab-case** for URL paths: `/customer/me`, `/pods/{id}/launch`, `/terminal/auth`
- **snake_case** for query parameters: `?pod_id=1`, `?page_size=10`

### Environment Variables
- **UPPERCASE** with underscores: `RC_CORE_PORT=8080`, `JWT_SECRET=...`, `DATABASE_PATH=/tmp/racecontrol.db`

---

## Build & Deployment Targets

### Rust Compilation
```bash
# Debug
cargo build

# Release (optimized)
cargo build --release

# Specific crate
cargo build -p rc-core --release

# Output binary locations
target/release/rc-core       # rc-core executable
target/release/rc-agent      # rc-agent executable
```

### Next.js Build
```bash
# PWA
cd pwa && npm run build → .next/ output

# Kiosk
cd kiosk && npm run build → .next/ output
```

### Docker Deployment
```bash
docker build -f deploy/Dockerfile -t racingpoint/rc-core:latest .
docker run -p 8080:8080 -v /tmp/racecontrol.db:/app/data/racecontrol.db ...
```

---

## Key File Relationships

```
pwa/ (Next.js PWA)
  ├── calls → rc-core API (port 8080)
  └── WebSocket → rc-core /ws

kiosk/ (Next.js Kiosk)
  └── calls → rc-core API (port 8080)

rc-core/ (Rust server)
  ├── imports → rc-common (types, protocol)
  ├── reads/writes → /tmp/racecontrol.db (SQLite)
  ├── calls → Cloud API (sync)
  └── WebSocket ↔ rc-agent (gaming PC)

rc-agent/ (Rust agent, runs on pod)
  ├── imports → rc-common (types, protocol)
  ├── spawns → Game EXE (AC, F1, iRacing, etc.)
  ├── listens → UDP 9996, 20777, 6789, 5300, 5555 (telemetry)
  ├── reads → Wheelbase HID input (OpenFFBoard)
  └── WebSocket → rc-core /ws
```

---

## Summary Table

| Component | Type | Location | Port | Purpose |
|-----------|------|----------|------|---------|
| **rc-core** | Rust | `crates/rc-core/src/` | 8080 | HTTP API, WebSocket hub, billing, cloud sync |
| **rc-agent** | Rust | `crates/rc-agent/src/` | WebSocket to 8080 | Game launcher, telemetry capture, lock screen, kiosk |
| **rc-common** | Rust lib | `crates/rc-common/src/` | — | Shared types, protocol messages, UDP format |
| **PWA** | Next.js | `pwa/` | 3500 | Customer web app (booking, sessions, leaderboard) |
| **Kiosk** | Next.js | `kiosk/` | 3400 | Staff interface (PIN login, quick launch) |
| **SQLite** | Database | `/tmp/racecontrol.db` | — | Venue persistent storage |
| **Cloud API** | REST | — | HTTPS | Sync endpoint for cloud ↔ venue replication |

