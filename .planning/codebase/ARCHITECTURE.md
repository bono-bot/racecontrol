# RaceControl Architecture

## Overview

RaceControl is a Rust-based sim racing venue management system with a 3-crate workspace architecture. It provides real-time pod (gaming PC) control, billing, telemetry processing, and customer-facing web interfaces for a simulation racing venue.

**Core Purpose**: Manage sim racing sessions at a venue with multiple pods, handle billing via drive-time, track lap telemetry, and provide live dashboard monitoring.

**Tech Stack**:
- Backend: Rust (Axum web framework, Tokio async runtime)
- Frontend: Next.js 16 (PWA) + Kiosk UI
- Real-time: WebSockets + UDP telemetry
- Hardware: OpenFFBoard wheelbases (VID:0x1209 PID:0xFFB0), USB HID steering input, UDP game telemetry
- Database: SQLite (venue), PostgreSQL (cloud)
- Games Supported: Assetto Corsa, Assetto Corsa Evo, iRacing, F1 25, Forza Motorsport, Le Mans Ultimate

---

## 3-Crate Workspace Structure

### Workspace Layout
`/root/racecontrol/Cargo.toml` defines a Rust workspace with three interdependent crates:

```
[workspace]
members = [
    "crates/rc-common",      # Shared types and protocol
    "crates/rc-core",        # Central server (port 8080)
    "crates/rc-agent",       # Pod/gaming PC agent
]
```

**Shared Dependencies** (in workspace.dependencies):
- `serde` + `serde_json` for serialization
- `tokio` (full features) for async runtime
- `chrono` for timestamps
- `uuid` for IDs
- `tracing` for observability
- `jsonwebtoken` for JWT auth
- `thiserror`, `anyhow` for error handling

---

## rc-common: Shared Protocol & Types

**Location**: `crates/rc-common/src/`

**Purpose**: Single source of truth for data structures and protocol messages shared between rc-core (server) and rc-agent (gaming PC).

### Key Files

#### `types.rs` - Domain Entities
Core domain types used throughout the system:

- **SimType** (enum): AssettoCorsa, AssettoCorsaEvo, IRacing, LeMansUltimate, F125, Forza
- **PodInfo** (struct): Pod identity and state
  - `id`, `number`, `name`, `ip_address`, `mac_address`
  - `sim_type`: Currently installed game
  - `status`: Offline, Idle, InSession, Error, Disabled
  - `current_driver`: Customer name or ID
  - `current_session_id`: Active billing session UUID
  - `last_seen`: Last heartbeat timestamp
  - `driving_state`: Optional (Active, Idle, Stopped)
  - `billing_session_id`: Linked to billing.rs session
  - `game_state`: Loading, Menu, Paused, Racing, Finished
  - `installed_games`: Vec of SimType available on pod

- **Driver** (struct): Customer profile
  - `id`, `name`, `email`, `phone`
  - `steam_guid`, `iracing_id`: Third-party IDs
  - `total_laps`, `total_time_ms`: Career stats

- **SessionType**: Practice, Qualifying, Race, Hotlap
- **SessionStatus**: Pending, Active, Completed, Cancelled
- **DrivingState**: Active, Idle, Stopped (from wheelbase input detection)
- **GameState**: Loading, Menu, Paused, Racing, Finished (from telemetry)

#### `protocol.rs` - WebSocket Messages
Serializable messages exchanged between rc-core and rc-agent over WebSocket:

**CoreMessage** (server → agent):
- `LaunchGame { pod_id, sim_type, experience_id }`: Start a game
- `StopGame { pod_id }`: Kill active game process
- `SetLockScreen { pod_id, state }`: Display lock screen UI
- `ShowOverlay { pod_id, widget_type, data }`: In-game overlay (telemetry, times)
- `UpdatePodConfig { pod_id, config }`: Wheelbase settings, game paths
- `Ping`: Heartbeat

**AgentMessage** (agent → server):
- `PodStateUpdate { pod_id, state }`: Pod status, driving state, game state
- `TelemetryFrame { pod_id, frame_data }`: Lap telemetry (delta, fuel, temps)
- `Lap { pod_id, lap_data }`: Completed lap (time, sector splits, fuel used)
- `SessionEvent { pod_id, event }`: Session start, end, reset
- `Pong`: Heartbeat response

#### `udp_protocol.rs` - UDP Telemetry Format
Binary protocol for game telemetry via UDP (F1 25 on port 20777, AC on 9996, iRacing on 6789, etc.):

- Frame-based deserialization for each sim's telemetry packet
- Lap detection logic (sector times, session state transitions)
- Fuel/tire/damage parsing for live overlay

---

## rc-core: Central Server (Port 8080)

**Location**: `crates/rc-core/src/`

**Purpose**: RESTful API server + WebSocket hub. Single point of truth for pod state, customer data, billing, cloud sync, and real-time control.

**Entry Point**: `main.rs`
- Loads config from `racecontrol.toml` (venue name, JWT secret, database path, cloud credentials)
- Initializes SQLite database (`initialize_db()`)
- Spawns async background tasks (pod monitor, cloud sync, scheduler)
- Starts Axum HTTP router on `config.server.host:config.server.port` (default 127.0.0.1:8080)
- Configures CORS, tracing, and JWT middleware
- Listens for WebSocket upgrades on `/ws`

### Core Modules

#### `state.rs` - In-Memory Application State
**AppState** (wrapped in Arc<RwLock<>>):
- `pods: HashMap<String, PodInfo>`: Current state of all pods (cached in-memory)
- `db: Arc<Database>`: SQLite connection pool
- `cloud_client: CloudClient`: HTTP client for cloud API sync
- `config: Config`: Runtime configuration
- `active_sessions: HashMap<String, BillingSession>`: In-flight billing sessions
- `action_queue: ActionQueue`: Queued pod operations (start, stop, config)

**Why in-memory?** For sub-100ms WebSocket broadcast latency to connected clients. Database is source of truth for persistence; state is a live cache.

#### `api/routes.rs` - REST Endpoints
Organized by resource (customers, pods, billing, etc.):

**Customer Routes** (`/customer/`):
- `GET /customer/me` — Current user profile + wallet balance
- `GET /customer/sessions` — Booking history with lap data
- `GET /customer/sessions/{id}` — Session detail + telemetry graph
- `GET /customer/sessions/{id}/share` — Shareable report (percentile, consistency, PB)
- `POST /customer/book` — Initiate booking (step through wizard, reserve pod)
- `GET /customer/ac/catalog` — 36 featured AC tracks/cars
- `GET /customer/packages` — Preset packages (Date Night, Birthday, Corporate)
- `GET /customer/membership` — Membership tier, hours tracking
- `POST /customer/membership` — Subscribe to membership

**Pod Routes** (`/pods/`):
- `GET /pods` — List all pods with current state
- `GET /pods/{id}` — Pod detail (status, driver, game, installed games)
- `POST /pods/{id}/launch` — Start game on pod (requires booking JWT)
- `POST /pods/{id}/stop` — Kill game (staff only)
- `POST /pods/{id}/screen` — Per-pod blanking (kiosk lock screen state)
- `GET /pods/{id}/metrics` — Pod uptime, session count, avg play time

**Billing Routes** (`/billing/`):
- `POST /billing/session/start` — Begin charging for drive time
- `POST /billing/session/end` — Stop charging, compute final bill
- `GET /billing/pricing` — Dynamic pricing for current time
- `GET /billing/history` — Customer's past bills

**Terminal Routes** (`/terminal/`):
- `POST /terminal/auth` — Verify 4-digit PIN for remote access
- `POST /terminal/{pod_id}/launch` — Direct game launch (kiosk staff)
- `POST /terminal/{pod_id}/stop` — Direct game kill

**Admin Routes** (`/admin/`):
- `GET /admin/dashboard` — Venue KPIs
- `POST /admin/pricing/rules` — Configure dynamic pricing (multipliers by day/hour)
- `POST /admin/coupons` — Create discount codes
- `GET /admin/audit-log` — Double-entry bookkeeping audit trail

#### `auth/mod.rs` - JWT Authentication
- `JwtClaims` struct: `{ user_id, role (Customer|Staff|Admin), exp, iat }`
- `generate_jwt()`: Create token on login (24h expiry)
- `verify_jwt()` middleware: Validate signature, expiry, role
- `get_current_user()` extractor: Extract user_id from request

**Employee PIN Auth**: `POST /terminal/auth` accepts 4-digit PIN, returns JWT. PIN is computed daily from hash(jwt_secret + date), preventing reverse-engineering.

#### `billing.rs` - Drive-Time Billing
**BillingSession** (struct):
```rust
pub struct BillingSession {
    pub id: String,                    // UUID
    pub pod_id: String,
    pub driver_id: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub drive_time_ms: u64,            // Accumulates while driving
    pub idle_time_ms: u64,             // While stationary
    pub total_paise_charged: i64,      // ₹1 = 100 paise
    pub status: SessionStatus,
}
```

**Drive-Time Logic**:
1. **Start Session**: Customer selects game/track. `billing::start_session()` creates row in `billing_sessions` table, debits wallet at START (not end).
2. **Timer Loop** (100ms tick): Check `DrivingState` from pod telemetry.
   - If Active: accumulate `drive_time_ms`
   - If Idle (>10s): accumulate `idle_time_ms`, don't charge
   - Staff CANNOT launch session if customer wallet balance < session cost estimate
3. **Dynamic Pricing**: `compute_dynamic_price()` applies multipliers from `pricing_rules` table (e.g., +30% on weekends, +20% 5–8 PM).
4. **End Session**: Write actual paise charged to `wallet_debit_paise` field, deduct from `wallet.balance`.

**Post-Session Hooks**:
- Referral rewards (₹100 referrer, ₹50 referee if first session)
- Review nudge scheduling (₹50 credit for Google review)
- Membership hours tracking (Pro/Champion tiers)

#### `ws/mod.rs` - WebSocket Hub
**Connection Lifecycle**:
1. Client (rc-agent or dashboard) connects to `/ws?pod_id={id}`
2. `handle_socket()` spawns a task per connection.
3. Server pushes `PodStateUpdate` messages (pod status, lap data) to all connected clients.
4. Server receives `CoreMessage` commands from authorized clients, forwards to action queue.

**Broadcast Architecture**:
- `broadcast::channel()` (Tokio): Fan-out pod state updates to all dashboards.
- Each pod has a separate channel to avoid congestion.
- Clients subscribe on connect, unsubscribe on disconnect.

#### `cloud_sync.rs` - Venue ↔ Cloud Sync
**Pull (Cloud → Venue)** every 30s:
- Drivers, wallets, pricing_tiers, pricing_rules, kiosk_experiences, kiosk_settings

**Push (Venue → Cloud)** every 30s:
- Laps, track_records, personal_bests, billing_sessions, pods, drivers (venue-owned fields), wallets
- Uses CRDT (Conflict-free Replicated Data Type): MAX() for `updated_at` timestamps to prevent stale overwrites

**Sync Conflict Prevention** (Mar 9 fix):
- `upsert_wallet()` checks `updated_at` before overwriting — prevents stale cloud balance erasing venue debits.
- Long-term: wallet_transactions table to sync individual credit/debit events.

#### `pod_monitor.rs` - Pod Health
- Heartbeat checker: Flags pods offline if no message in 2 minutes.
- Status transitions: Idle → InSession → Idle (or Error on crash).
- Auto-recovery: Attempts WoL (Wake-on-LAN) if offline.

#### `pod_healer.rs` - Auto-Recovery
- Detects crashed games, dead rc-agent processes.
- Sends `StopGame` command to orphaned processes.
- Restarts rc-agent via WoL if needed.

#### `game_launcher.rs` - Game Start Orchestration
1. Receives `POST /pods/{id}/launch` with `{ game: "ac", track, car, ... }`
2. Validates customer wallet balance.
3. Calls `billing::start_session()` (debits wallet immediately).
4. Queues `LaunchGame` message to rc-agent over WebSocket.
5. Waits for `GameState::Racing` telemetry.
6. Returns session ID to customer.

#### `pod_reservation.rs` - Booking Wizard
- 8-step wizard: Sim selection → Track/Car → Duration → Difficulty → Friends → Customization → Review → Checkout.
- Reserves pod for requested time slot.
- Checks pod availability + customer wallet.
- Returns formatted booking summary.

#### `lap_tracker.rs` - Telemetry → Leaderboard
- Consumes `Lap` messages from rc-agent (lap_time, sector_splits, fuel_used, etc.).
- Computes percentile rank vs. same track/car.
- Stores in `personal_bests`, `track_records` tables.
- Exposes `GET /leaderboard/public` (no auth) for public rankings.

#### `catalog.rs` - AC Catalog API
- Hosts 36 featured tracks, 41 featured cars (50 total, 325 total).
- Endpoints: `GET /customer/ac/catalog`, `GET /customer/ac/tracks`, `GET /customer/ac/cars`.

#### `friends.rs` - Social Features
- Friend requests, accept/reject/block.
- Presence (online/offline/in_session).
- Group booking (multiple drivers on same pod, turn-based).

#### `multiplayer.rs` - Multiplayer Session Coordination
- Join multiplayer lobbies (AC server mode).
- Coordinate pod allocation for team events.

#### `action_queue.rs` - Command Queueing
- Prevents race conditions when multiple clients issue commands simultaneously.
- FIFO queue: `LaunchGame`, `StopGame`, `SetLockScreen`, etc.
- Consumer task processes one command at a time, awaits WebSocket ack.

#### `scheduler.rs` - Cron Jobs
- Pricing rule updates (apply multipliers on schedule).
- Referral payout batching.
- Membership hour resets (monthly).

#### `ai.rs` - Claude API Integration
- `POST /ai/coaching` — AI-powered lap coaching (sector deltas, tips).
- `POST /ai/chat` — Customer support chatbot.

#### `accounting.rs` - Journal Entries
- Double-entry bookkeeping for financial transactions.
- Auto-posts on every wallet credit/debit/refund.
- Queries: trial-balance, profit-loss, balance-sheet.

#### `activity_log.rs` - Audit Trail
- Soft-deletes: pricing_tiers, pricing_rules, coupons marked `is_active=0`.
- Auto-logs before/after snapshots on CRUD operations.
- Query via `GET /audit-log`.

#### `db/mod.rs` - SQLite Abstraction
- Connection pool (sqlite with rusqlite, wrapped in Arc<RwLock<>>).
- Schema migrations (create tables if not exist).
- Tables: customers, billing_sessions, personal_bests, track_records, pricing_rules, coupons, audit_log, accounts, journal_entries, wallets, etc.

#### `config.rs` - Configuration
- Reads `racecontrol.toml`:
  - `[venue]`: name, location, timezone
  - `[server]`: host, port
  - `[auth]`: jwt_secret, employee_pin_base
  - `[database]`: path (default /tmp/racecontrol.db)
  - `[cloud]`: api_url, sync_interval_secs
  - `[games]`: paths to game executables on pod
- Fallback defaults if file missing.

#### `remote_terminal.rs` - Staff Terminal
- PIN-authenticated remote launcher for kiosk staff.
- `POST /terminal/{pod_id}/launch` launches game directly (no customer booking needed).

#### `error_aggregator.rs` - Error Tracking
- Collects pod/game errors.
- Alerts admins if error rate spikes.

#### `ac_server.rs`, `ac_camera.rs` - Assetto Corsa Specifics
- AC stracker protocol (UDP server list, live track state).
- Camera control for replay clips.

#### `udp_heartbeat.rs` - UDP Listener
- Listens on multiple ports (9996 AC, 20777 F1, 5300 Forza, 6789 iRacing, 5555 LMU).
- Deserializes telemetry packets, emits Lap/TelemetryFrame events.

#### `wol.rs` - Wake-on-LAN
- Send magic packet to offline pods (MAC address stored in PodInfo).

---

## rc-agent: Pod Client (Gaming PC)

**Location**: `crates/rc-agent/src/`

**Purpose**: Runs on each gaming PC (pod). Listens for commands from rc-core, launches/kills games, captures telemetry, manages UI overlays, handles lock screen.

**Entry Point**: `main.rs`
- Loads `AgentConfig` from TOML (pod ID, core server URL, game paths, wheelbase vendor ID/PID).
- Spawns threads for:
  - Game process manager
  - Lock screen UI
  - Overlay renderer
  - Kiosk UI
  - UDP telemetry listeners (6 ports in parallel)
  - Driving detector (HID wheelbase input)
  - WebSocket sender/receiver
- Connects to rc-core WebSocket: `ws://core:8080/ws?pod_id={id}`

### Core Modules

#### `game_process.rs` - Game Launch & Monitoring
**GameProcessManager**:
- Spawns game EXE (e.g., `C:\Program Files\Assetto Corsa\assettocorsa.exe`).
- Polls process status (running, crashed, exited).
- Sends SIGTERM/SIGKILL on `StopGame` command.
- Tracks game PID, CPU/memory usage.

**Game Launch Flow**:
1. Receive `LaunchGame { pod_id, sim_type, experience_id }` over WebSocket.
2. Look up game path from config: `GamesConfig { assetto_corsa, f1_25, ... }`.
3. Resolve experience (track/car combo) from customer booking.
4. Build command line: `game.exe --track "Monza" --car "Ferrari" --difficulty "Expert"`.
5. Spawn process with redirected stdout/stderr.
6. Poll telemetry (game sends UDP frames on 9996, 20777, etc.).
7. On game exit or `StopGame`, clean up.

#### `game_process.rs` - Telemetry Parsing
- Listens on UDP port for game telemetry (F1 25, AC, iRacing, Forza, LMU).
- Parses binary frames (frame_id, position, velocity, fuel, tire temps, etc.).
- Emits `TelemetryFrame` and `Lap` events to rc-core.

#### `sims/mod.rs` - Sim Adapter Pattern
**SimAdapter** (trait):
- `parse_frame()`: Convert binary UDP to normalized telemetry struct.
- `detect_lap()`: Identify lap completion from session_id + lap counter changes.
- `extract_sectors()`: Compute sector times from waypoint data.

**Implementations**:
- `sims/assetto_corsa.rs`: AC physics data deserialization.
- `sims/f1_25.rs`: F1 25 telemetry (complete as of Mar 5).

#### `driving_detector.rs` - Wheelbase Input Detection
**DrivingDetector**:
- Reads OpenFFBoard HID input (steering angle, throttle, brake, clutch).
- Detects if input is active (steering >5°, throttle >10%, etc.).
- Emits `DrivingState::Active` or `DrivingState::Idle`.
- Used by billing timer to distinguish charged drive time from idle time.

**HID Access**:
- Uses `hidapi` crate (cross-platform USB HID).
- Vendor ID: 0x1209, Product ID: 0xFFB0 (OpenFFBoard).
- Polls every 10ms, logs if input inactive for >10s.

#### `lock_screen.rs` - Lock Screen Manager
- Displays full-screen overlay (customer name, track, timer, fuel gauge).
- Receives `SetLockScreen` messages from rc-core.
- Renders using Windows API (DirectX or GDI on Windows, Xlib on Linux).

#### `overlay.rs` - Overlay Renderer
- In-game overlay (telemetry, delta time, fuel estimate).
- Uses game's native overlay API or injected DLL (game-specific).
- Updates from `TelemetryFrame` events.

#### `kiosk.rs` - Kiosk UI Manager
**KioskManager**:
- Displays staff interface (pod status, active session, quick buttons).
- Staff PIN login (4-digit, computed daily).
- Quick launch buttons for games.
- Sends commands to rc-core: `LaunchGame`, `StopGame`.
- Shows customer name, timer, current fuel, seat position.

#### `ai_debugger.rs` - AI Coaching Integration
- Optionally hooks into lap data.
- Sends coaching queries to rc-core `/ai/coaching` endpoint.
- Displays coaching tips in overlay.

#### `udp_heartbeat.rs` - Heartbeat to rc-core
- Sends `Pong` every 2 minutes to `ws://core:8080/ws`.
- rc-core detects offline pods if no heartbeat for 6 minutes.

#### `debug_server.rs` - Local Debug HTTP
- `http://pod_ip:3000/status` returns PodInfo JSON.
- Used by James for on-site debugging.

---

## Data Flow: Customer Session

### 1. Booking (PWA → rc-core)
```
Customer on PWA /book
  → SELECT game, track, car, duration
  → POST /customer/book { game, track, car, duration, friends: [...] }
  → rc-core: pod_reservation::validate_booking()
    → Check pod availability
    → Check customer wallet balance
    → Reserve pod + session slot in DB
  ← Returns { booking_id, pod_id, cost_estimate, reserved_until }
```

### 2. Game Launch (rc-core → rc-agent → Game)
```
Customer confirms booking, enters pod
  → POST /pods/{pod_id}/launch { booking_id, game, track, car }
  → rc-core validates JWT, calls billing::start_session()
    → Debits wallet immediately
    → Creates billing_sessions row
  → rc-core queues CoreMessage::LaunchGame
  → rc-core sends over WebSocket to rc-agent

  ← rc-agent receives CoreMessage::LaunchGame
    → game_process.rs looks up game EXE path
    → Spawns: C:\AC\assettocorsa.exe --track Monza --car Ferrari
    → Polls UDP 9996 for telemetry frames
    → Emits TelemetryFrame events to rc-core
```

### 3. Live Telemetry (rc-agent → rc-core → Dashboard)
```
Game sends UDP frame (F1 25 on 20777, AC on 9996, etc.)
  ← rc-agent::sims::f1_25::parse_frame()
    → Normalized { speed, position, throttle, fuel, tire_temp, ... }
    → AgentMessage::TelemetryFrame
  → Sent to rc-core over WebSocket

  ← rc-core receives TelemetryFrame
    → Broadcasts to all dashboard clients over WebSocket
    → Updates in-memory pod.driving_state (Active/Idle)
    → Accumulates drive_time_ms (if Active)
```

### 4. Lap Completion (rc-agent → rc-core → Leaderboard)
```
Game detects lap completion (session_id changed, lap_id incremented)
  ← rc-agent::lap_tracker::detect_lap()
    → Extracts lap_time, sector_splits, fuel_used
    → AgentMessage::Lap { pod_id, lap_data }
  → Sent to rc-core

  ← rc-core::lap_tracker.rs
    → Computes percentile rank
    → Stores in personal_bests, track_records
    → Updates leaderboard
```

### 5. Session End (Customer Exits → rc-core)
```
Customer exits game (or timeout)
  ← rc-agent detects game process exit
    → Sends AgentMessage::SessionEvent { event: SessionEnded }
  → rc-core receives
    → Calls billing::end_session(session_id)
    → Computes final paise charged
    → Writes to wallet.balance
    → Schedules post-session hooks (referral, review nudge, membership hours)

  ← PWA shows session summary
    → /sessions/{id} page with telemetry graph, lap times, leaderboard rank
```

---

## State Management

### In-Memory State (AppState)
**Located in**: `state.rs`

**Pods HashMap** (source of truth for real-time):
- `pods: HashMap<String, PodInfo>`
- Populated on startup from DB.
- Updated every 100ms from telemetry/agent messages.
- Broadcast to all WebSocket clients.

**Active Sessions** (transient, not persisted until end):
- `active_sessions: HashMap<String, BillingSession>`
- Track in-flight billing (drive time, idle time, paise charged).
- Written to DB when session ends.

**Action Queue** (commands waiting to send):
- `action_queue: ActionQueue`
- Holds `CoreMessage` items (LaunchGame, StopGame, SetLockScreen).
- Consumer task processes FIFO, awaits WebSocket ack.

### SQLite Persistence
**Location**: Default `/tmp/racecontrol.db` (configurable in `racecontrol.toml`)

**Key Tables**:
- `pods` — Pod metadata (name, IP, MAC, sim_type, installed_games)
- `drivers` — Customer profiles (name, email, phone, steam_guid, iracing_id)
- `billing_sessions` — Session records (pod_id, driver_id, start_time, end_time, paise_charged)
- `personal_bests` — Customer's fastest lap per track/car
- `track_records` — Venue leaderboard (fastest lap ever on each track/car)
- `pricing_rules` — Dynamic pricing multipliers (day_of_week, hour, multiplier)
- `coupons` — Discount codes (percent/flat/free_minutes, is_active)
- `wallets` — Customer credit balances (driver_id, balance_paise, updated_at)
- `journal_entries` — Double-entry bookkeeping (account_id, debit/credit, timestamp)
- `audit_log` — Change history (table, record_id, before/after JSON, timestamp)

### Cloud Sync (Bi-directional)
**Located in**: `cloud_sync.rs`

**Pull (Cloud → Venue)**: Every 30s
- Drivers, wallets, pricing_tiers, pricing_rules, kiosk_experiences

**Push (Venue → Cloud)**: Every 30s
- Laps, track_records, personal_bests, billing_sessions, pods, drivers, wallets

**Conflict Resolution**: MAX(updated_at) — newest write wins. Long-term: event sourcing via `wallet_transactions` table.

---

## Real-Time Features

### WebSocket Architecture
**Endpoint**: `GET /ws?pod_id={id}`

**Message Flow**:
```
Client (rc-agent or Dashboard)
  ↓ (send CoreMessage)
  rc-core WebSocket handler
  ↓ (broadcast AgentMessage)
  All connected clients receive update
```

**Channels**:
- Tokio `broadcast::channel()` per pod (to avoid global congestion).
- Clients subscribe on `/ws` connect, unsubscribe on disconnect.

### Live Dashboard Updates
- Real-time pod status (Online/Offline/Idle/InSession/Error).
- Active session timers (elapsed drive time, estimated total time).
- Telemetry updates (speed, fuel, tire temps) every 10–100ms.
- Lap-time notifications (instant leaderboard rank update).

### Broadcast to Multiple Clients
Example: One customer's session affects multiple dashboards.
```
Pod 5 spawns lap message
  → rc-core receives from rc-agent
  → Broadcasts PodStateUpdate to all clients subscribed to pod_5
  → Dashboard 1 updates live timer
  → Dashboard 2 updates leaderboard
  → Kiosk on Pod 5 updates overlay
```

---

## Authentication & Authorization

### JWT Structure
**Claims**:
```rust
{
  "user_id": "cust_abc123",
  "role": "Customer",  // or "Staff", "Admin"
  "exp": 1700000000,   // 24h expiry
  "iat": 1699913600
}
```

**Roles**:
- **Customer**: Can book, view own sessions, join leaderboard.
- **Staff**: Can launch games directly (PIN auth), view pod status.
- **Admin**: Full access (pricing rules, coupons, audit logs).

### Middleware
`jwt_error_to_401` middleware in `main.rs` converts JWT decode errors to 401 Unauthorized.

### Employee PIN Authentication
**Endpoint**: `POST /terminal/auth { pin }`

**PIN Computation**:
```
pin = hash(jwt_secret + today_date) % 10000
```
- Daily rotation prevents brute-force / reverse-engineering.
- Staff can log in from kiosk using 4-digit PIN.

---

## Error Handling & Resilience

### Pod Failure Recovery
1. **pod_monitor.rs** detects offline (no heartbeat >2min).
2. **pod_healer.rs** attempts WoL (Wake-on-LAN).
3. If game crashes mid-session, billing auto-closes session.
4. Customer refunded if crash occurred.

### Network Disconnection
- rc-agent WebSocket drops: rc-core flags pod as Offline.
- Dashboard reconnect auto: PWA WebSocket reconnect logic.
- Retry queue: Action commands (LaunchGame, StopGame) queued if pod unreachable, retried on reconnect.

### Database Failures
- Fallback to in-memory state if SQLite read fails.
- All writes replicated to cloud (cloud_sync.rs) as backup.
- Soft deletes prevent accidental data loss.

---

## Scalability & Performance

### Multi-Pod Scaling
- **In-memory HashMap** for <100 pods (typical venue has 8–12).
- Each pod has independent UDP telemetry listener (non-blocking).
- WebSocket broadcast fan-out scales with client count (Tokio task-per-connection).

### Telemetry Throughput
- **F1 25**: 60 Hz telemetry (60 frames/sec per pod × 8 pods = 480 frames/sec).
- **UDP parsing**: Non-blocking, scales with core count.
- **WebSocket broadcast**: Tokio async, supports 100s of concurrent clients.

### Database Throughput
- **SQLite**: Write-serialized (one connection). Suitable for venue-level (not global enterprise).
- **Cloud sync**: Batches updates, pushes every 30s (not real-time consistency).

### Optimization Notes
- Pod state cached in-memory (HashMap), not fetched from DB on each request.
- Lap data batched (not inserted individually).
- Pricing rules cached on startup, updated via scheduler (not fetched on every session start).

---

## Key Design Principles

1. **Single Source of Truth**: Database is persistent, memory is cache. Always read from DB on restart.
2. **Loose Coupling**: rc-agent and rc-core communicate via WebSocket + protocol types (rc-common), not shared code.
3. **Async-First**: Tokio for all I/O (database, network, WebSocket). No blocking calls in hot paths.
4. **Protocol Versioning**: protocol.rs enums versioned for backward-compat when agents/servers diverge.
5. **Cloud-Aware**: All writes replicate to cloud. Venue is always degraded-mode capable (works offline).
6. **Soft Deletes**: Audit trail preservation via is_active flag, not hard deletes.
7. **Drive-Time Integrity**: Billing debits at session start (not end) to prevent customers with $0 balance launching games.
8. **Real-Time Broadcasting**: WebSocket broadcast for live dashboards, avoiding long-polling.
