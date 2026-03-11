# RaceControl Integrations & External APIs

## Overview
RaceControl integrates with multiple external systems for sim racing telemetry, game launchers, authentication, cloud sync, and AI services. All integrations are configurable via TOML and support graceful degradation.

---

## Database Integration

### Primary Database: SQLite
**Type**: SQLite 3 (local file-based)
**Location**: `/root/racecontrol/data/racecontrol.db`
**Access Layer**: SQLx (async, compile-time query checking)
**Connection**: Tokio-backed async SQLite driver

#### Key Tables & Schema

**Drivers (Customers)**
- `id` (UUID)
- `customer_id` (external ref)
- `phone` (unique)
- `name`, `email`
- `steam_guid`, `iracing_id` - Game account linking
- `avatar_url`
- `wallet_balance` (in paise ₹0.01)
- `has_used_trial`, `total_laps`
- Cloud sync: `updated_at` (CRDT merge)

**Pods**
- `pod_number` (1-8)
- `pod_name` (display name)
- `status` (online/offline/error)
- `current_game` (AC/iRacing/F1/Forza/LMU)
- `ip_address` (192.168.31.x)
- `last_heartbeat` (timestamp)
- `error_log` (latest issue)

**Billing Sessions**
- `id` (UUID)
- `driver_id`, `pod_number`
- `game`, `track`, `car` (AC metadata)
- `start_time`, `end_time`, `duration_secs`
- `pricing_tier_id` (dynamic pricing applied)
- `amount_paise` (debited from wallet at start)
- `wallet_debit_paise` (actual amount debited)
- `status` (active/completed/refunded)
- Cloud sync: `updated_at`

**Wallet Transactions**
- `id` (UUID)
- `driver_id`
- `type` (debit/credit/refund)
- `amount_paise` (positive value)
- `reason` (session_debit, referral_credit, review_nudge, top_up, etc.)
- `session_id` (nullable)
- `created_at`

**Pricing Rules**
- `id`
- `day_of_week` (0-6), `hour_start`, `hour_end`
- `session_type` (30min/60min/trial)
- `multiplier` (e.g., 1.2 for peak hours)
- `is_active` (soft-delete support)
- Auto-applied at session start via `compute_dynamic_price()`

**Coupons**
- `code` (unique)
- `coupon_type` (percent/flat/free_minutes)
- `value` (discount amount or % or minutes)
- `max_uses`, `uses_count`
- `expiry_date`
- `is_active` (soft-delete)

**Memberships**
- 3 tiers: Rookie, Pro, Champion
- `tier_id`, `driver_id`
- `subscribed_at`, `expires_at`
- `hours_included`, `hours_used`

**Tournaments**
- `id`, `name`, `status` (pending/active/completed)
- `bracket_type` (single_elimination)
- Auto-generates matches + advances winners

**Time Trials**
- `id`, `track`, `car`
- `week_start`, `week_end`
- Public leaderboard filtered by week

**Referrals**
- `referrer_code` (unique)
- `referrer_id`, `referee_id`
- Auto-credit: ₹100 referrer, ₹50 referee on first session

**Laps**
- `id`, `session_id`, `driver_id`
- `lap_number`, `lap_time_ms`, `sector_times`
- `track`, `car`, `setup`
- `consistency_score` (σ deviation)
- `fuel`, `tires`, `tire_wear`
- `created_at`

**Friends**
- `requester_id`, `recipient_id`
- `status` (pending/accepted/blocked)

**Journal Entries (Double-entry Bookkeeping)**
- `id`, `created_at`
- `entry_type` (invoice, payment, adjustment)
- `description`

**Journal Entry Lines**
- `id`, `entry_id`
- `account_id`, `debit_paise`, `credit_paise`

**Audit Log**
- `id`, `table_name`, `record_id`
- `action` (create/update/delete)
- `before`, `after` (JSON snapshots)
- `changed_by`, `changed_at`
- Soft-delete audit trail for pricing_rules, coupons

**Heartbeats**
- `id`, `source` (bono/james)
- `last_beat` (timestamp)
- Used for failsafe system (6 min alert threshold, 3 missed beats)

---

## Telemetry Integration

### UDP Telemetry Protocol
**Module**: `/root/racecontrol/crates/rc-agent/src/udp_heartbeat.rs`
**Protocol**: Custom binary UDP packets
**Listener**: Both rc-core and rc-agent

#### Monitored UDP Ports

| Sim | Port | Data | Handler |
|-----|------|------|---------|
| **Assetto Corsa** | 9996 | Realtime telemetry (speed, throttle, brake, steering) | AC physics plugin (CSP) |
| **F1 25** | 20777 | F1 telemetry (session, lap, damage, tire data) | F1 telemetry API |
| **Forza Motorsport** | 5300 | Forza telemetry (accel, vel, rotation, tire temp) | Forza API |
| **iRacing** | 6789 | iRacing telemetry (session, driver, telemetry vars) | iRacing telemetry API |
| **Le Mans Ultimate (LMU)** | 5555 | LMU telemetry | LMU physics plugin |

**Packet Structure**:
- Header: Frame ID, timestamp, session ID
- Payload: Game-specific telemetry (speed m/s, throttle 0-100, brake 0-100, steering -1 to +1)
- Footer: CRC checksum

**Aggregation**:
- `lap_tracker.rs` rolls up UDP packets into lap records
- Sector detection: 3-sector splits per lap
- Consistency: Standard deviation of lap times
- PB tracking: Personal best per track/car combination

**Heartbeat interval**: 16ms (60 Hz update frequency)

---

## WebSocket Communication

### Agent ↔ Core Bidirectional Protocol
**Server port**: 8080 (rc-core)
**Client port**: 18923 (rc-agent lock screen)
**Protocol**: `ws://[core_ip]:8080/ws/agent`
**Modules**:
- rc-core: `/root/racecontrol/crates/rc-core/src/ws/`
- rc-agent: uses tokio-tungstenite client

#### Message Types (Bidirectional)

**Core → Agent** (Commands):
- `LaunchGame { game: String, track: String, car: String }`
- `StopGame { pod_id: u32 }`
- `ScreenBlank { pod_id: u32, duration_secs: u32 }`
- `LockScreen { pin_required: bool }`
- `UpdateKiosk { config: KioskConfig }`
- `PingHeartbeat { timestamp: u64 }`

**Agent → Core** (Events):
- `AgentOnline { pod_id: u32, pod_name: String }`
- `GameStateChanged { game: String, state: (running/stopped/error) }`
- `DrivingDetected { is_driving: bool, hid_active: bool, udp_active: bool }`
- `Error { pod_id: u32, message: String }`
- `Heartbeat { pod_id: u32, timestamp: u64 }`

**Reconnection**: Automatic with exponential backoff (1s → 30s max)
**Timeout**: 30s inactivity → reconnect attempt

---

## Cloud Sync Integration

### Venue ↔ Cloud Sync System
**Endpoint**: `https://app.racingpoint.cloud/api/v1`
**Interval**: 30 seconds (configurable in racecontrol.toml `[cloud].sync_interval_secs`)
**Module**: `/root/racecontrol/crates/rc-core/src/cloud_sync.rs`
**Status**: LIVE (Mar 7, 2026)

#### Pull Direction (Cloud → Venue)
**Synced data**:
- `drivers` - Customer profiles
- `wallets` - Account balances (with `updated_at` CRDT merge)
- `pricing_tiers` - Billing configuration
- `pricing_rules` - Dynamic pricing rules
- `kiosk_experiences` - Per-pod game configs
- `kiosk_settings` - Venue-wide settings

**Conflict resolution**: MAX() CRDT on `updated_at` (latest wins)

#### Push Direction (Venue → Cloud)
**Synced data**:
- `laps` - Lap records + telemetry
- `track_records` - Session PBs
- `personal_bests` - Driver PBs per track/car
- `billing_sessions` - Session records
- `pods` - Pod status & config
- `drivers` - Venue-specific fields (phone verified, trial used, total laps)
- `wallets` - Wallet debits + balance updates

**Push logic**:
- Debits: Immediately pushed on session start
- Wallet updates: `upsert_wallet` checks `updated_at` before overwriting (prevents stale cloud data)

**Error handling**: Failed syncs queue for retry; no blocking of local operations

#### Configuration
```toml
[cloud]
enabled = true
api_url = "https://app.racingpoint.cloud/api/v1"
sync_interval_secs = 30
terminal_secret = "rp-terminal-2026"
terminal_pin = "261121"
```

---

## Evolution API Integration (WhatsApp)

### WhatsApp Gateway
**Provider**: Evolution API
**Instance Name**: "Racing Point Reception"
**URL**: `http://localhost:53622` (local Evolution server)
**API Key**: `zNAKEHsXudyqL3dFngyBJAZWw9W4hWN0`
**Configuration File**: `/root/racecontrol/racecontrol.toml [auth]`

**Endpoints**:
- `POST /message/sendText` - Send WhatsApp text
- `POST /message/sendMedia` - Send images/videos
- `POST /webhook` - Incoming message webhook

**Modules**:
- `/root/racecontrol/crates/rc-core/src/api/evolution.rs` (integration layer)

**Use cases**:
- OTP delivery (PIN for terminal access)
- Session notifications (booking confirmation, session end)
- AI responses (Claude chat via WhatsApp bot)
- Review nudges (post-session review request with incentive)

**Integration notes**:
- URLencoded instance names (dependency: `urlencoding 2.x`)
- Rate limiting: 1 msg/sec per instance
- Message queuing: Action queue handles delivery

---

## Game Launch Integration

### Steam Integration
**Module**: `/root/racecontrol/crates/rc-agent/src/game_process.rs`
**Steam API**: Direct Steam app ID launch via Windows registry
**Supported games**:

| Game | Steam App ID | Launch Method | Notes |
|------|-------------|---------------|-------|
| **Assetto Corsa** | 244210 | Steam app launch | CSP plugins for UDP telemetry on port 9996 |
| **F1 25** | 2488620 | Steam app launch | Native F1 telemetry API on port 20777 |
| **Forza Motorsport** | 2440510 | Steam app launch | Telemetry on port 5300 |
| **iRacing** | Direct exe | `C:\Program Files (x86)\iRacing\iRacingSim64DX11.exe` | Telemetry on port 6789 |
| **Le Mans Ultimate** | 1564310 | Steam app launch | Telemetry on port 5555 |

**Launch flow**:
1. Staff selects game + track + car on kiosk
2. rc-agent receives `LaunchGame` command via WebSocket
3. `game_process.rs` spawns process (Steam or direct exe)
4. `driving_detector.rs` monitors HID + UDP for active driving
5. On driving detected: lock screen hides, overlay shows
6. On driving idle (>10s): overlay minimized, lock screen re-enabled

**Kill flow**:
1. Session end time reached OR customer requests stop
2. `StopGame` command via WebSocket
3. `game_process.rs` terminates process (WaitForInputIdle → WM_QUIT → TerminateProcess)
4. Lock screen re-enabled, session recorded

**PowerShell integration** (Windows):
- `/root/racecontrol/deploy_pod8.ps1` - Deploy rc-agent to pod
- Get-Process, Start-Process, Stop-Process commands
- Steam path detection: `C:\Program Files (x86)\Steam\steamapps\common\[game]`

---

## Hardware Integration

### USB HID (Wheelbase Detection)
**Module**: `/root/racecontrol/crates/rc-agent/src/driving_detector.rs`
**Library**: `hidapi 2.x` (cross-platform USB HID)
**Hardware**: Conspit Ares 8Nm, 10Nm, 12Nm wheelbases

**Vendor/Product IDs**:
- VID: `0x1209` (OpenFFBoard VID)
- PID: `0xFFB0` (OpenFFBoard generic)

**Detection logic**:
- Poll HID devices every 100ms
- Read steering axis (analog input -1 to +1)
- Threshold: >0.05 magnitude = active steering
- State transitions: connected → active → idle → disconnected

**Driving state machine**:
```
HID connected (steering present)
  └─ Steering active (>0.05 threshold)
     └─ [Driving detected] Lock screen hidden, overlay visible

HID idle (steering <0.05 for >2s)
  └─ [Idle detected] Lock screen visible again

HID disconnected (USB unplugged)
  └─ Fall back to UDP-only detection
```

**Fallback**: If HID unavailable, UDP packet timing used (>20Hz = driving)

### Process Monitoring
**Library**: `sysinfo 0.33` (cross-platform process tracking)
**Monitored processes**:
- Game process (AC.exe, iRacing, F1_25.exe, etc.)
- Steam processes (steam.exe, steamwebhelper.exe)
- Conspit wheel software (ConspitLink2.0.exe)
- System processes (SystemSettings, ApplicationFrameHost)

**Pod healer** (`pod_healer.rs`):
- Monitors Steam/Conspit zombies
- Auto-kills stale processes
- Triggers on pod error detection

---

## AI Integration

### Claude Integration (rc-core)
**Module**: `/root/racecontrol/crates/rc-core/src/ai.rs`
**Method**: Anthropic SDK + Claude Haiku (for customers), Sonnet (for admin)
**HTTP client**: `reqwest 0.12`

**Use cases**:
- AI chat (`/api/v1/customer/chat`)
- Session analysis (lap quality feedback)
- Telemetry interpretation
- Anomaly detection (error logs)

**Configuration**:
```toml
[ai_debugger]
enabled = true
claude_cli_enabled = true  # Preferred: Free tier via Claude CLI
claude_cli_timeout_secs = 30
anthropic_api_key = "sk-ant-..."  # Fallback API key
chat_enabled = true
proactive_analysis = true
```

### Ollama Integration (rc-agent Local LLM)
**Module**: `/root/racecontrol/crates/rc-agent/src/ai_debugger.rs`
**Primary model**: `qwen2.5-coder:14b` (venue James machine RTX 4070)
**Fallback**: Anthropic API if Ollama unavailable
**HTTP client**: `reqwest 0.12`

**Configuration** (`rc-agent.toml`):
```toml
[ai_debugger]
enabled = true
ollama_url = "http://192.168.31.100:11434"  # Venue James machine
ollama_model = "qwen2.5-coder:14b"
# anthropic_api_key = "sk-ant-..."  # Optional fallback
```

**Use cases**:
- Lock screen debug overlay (Claude code suggestions)
- Game crash analysis
- Performance tuning tips
- Live performance coaching

---

## mDNS Service Discovery

### Pod Discovery
**Library**: `mdns-sd 0.12` (both rc-core and rc-agent)
**Protocol**: Multicast DNS on port 5353
**Service type**: `_racecontrol._tcp.local`

**Discovery flow**:
1. rc-agent announces `Pod-N._racecontrol._tcp.local` with IP + port 18923
2. rc-core scans LAN, discovers all pods
3. Pods register in `pods` table with IP address
4. WebSocket connection established on discovery

**Fallback**: Manual pod IP configuration in `racecontrol.toml` or pod config files

---

## Authentication & Security

### JWT Authentication
**Library**: `jsonwebtoken 9.x`
**Secret**: Stored in `racecontrol.toml` `[auth].jwt_secret`
**Token lifetime**: 24 hours (default)
**Algorithms**: HS256

**Endpoints requiring auth**:
- All `/api/v1/customer/*` endpoints
- `/api/v1/employee/*` endpoints
- WebSocket connections from agents

**Terminal PIN Auth**
**Module**: `/root/racecontrol/crates/rc-core/src/remote_terminal.rs`
**Endpoint**: `POST /terminal/auth`
- PIN sent via WhatsApp (Evolution API)
- 5-minute validity window
- Used for remote pod access (restart, logs, debug)

**Employee Debug PIN**
**Generation**: `hash(jwt_secret + date)` daily rotating
**Use**: Staff access to debug pages (no login required)

---

## Scheduling & Jobs

### Cron-like Scheduler
**Module**: `/root/racecontrol/crates/rc-core/src/scheduler.rs`
**Runtime**: Tokio interval tasks

**Scheduled jobs**:
- Review nudge scheduling (2 hours post-session)
- Referral auto-credit (on first session completion)
- Tournament auto-advance (weekly)
- Cloud sync (every 30s)
- Pod health check (every 60s)
- Wallet reconciliation (daily 2 AM IST)

---

## Activity & Audit Logging

### Structured Logging
**Library**: `tracing 0.1` + `tracing-subscriber 0.3`
**Output**: Structured JSON logs to stdout
**Filter**: Environment variable `RUST_LOG` (default: info)

**Modules**:
- Pod discovery: `INFO Pod discovered: 192.168.31.89`
- Game launch: `DEBUG Game launched: AC on Pod 1`
- Billing: `WARN Wallet insufficient: driver_id=123, balance=0`
- Cloud sync: `ERROR Sync failed, retrying...`

### Activity Audit Log
**Table**: `activity_log`
**Fields**: `driver_id`, `action`, `metadata`, `created_at`
**Actions tracked**:
- Session start/end
- Wallet top-up
- Coupon redemption
- Friend request
- Tournament registration

**Query**: `GET /api/v1/audit-log`

---

## AC Dedicated Server Integration

### Assetto Corsa Server Control
**Module**: `/root/racecontrol/crates/rc-core/src/ac_server.rs`
**Configuration**:
```toml
[ac_server]
acserver_path = "C:\Program Files (x86)\Steam\steamapps\common\assettocorsa\server\acServer.exe"
data_dir = "./data/ac_servers"
```

**Functionality**:
- Server config generation (track, cars, fuel, tire wear)
- Launch/stop AC server instances
- Session result parsing (CSP telemetry logs)
- Multiplayer race orchestration

**Data dir**: Stores config files, results, replay files

---

## Payment & Wallet System

### Credit Wallet Integration
**Module**: `/root/racecontrol/crates/rc-core/src/wallet.rs`
**Currency**: Paise (₹0.01), stored as integers
**Pricing**:
- 30 min session: ₹700 (70,000 paise)
- 60 min session: ₹900 (90,000 paise)
- 5 min free trial

**Workflow**:
1. Customer tops up wallet (₹100 = 10,000 paise)
2. At session start: wallet debited (amount from dynamic pricing)
3. `wallet_debit_paise` records actual debit amount
4. Referral/coupon/review nudge credits added asynchronously

**Database schema**:
- `drivers.wallet_balance` - Current balance (paise)
- `wallet_transactions` - Full ledger (debit/credit/refund)

---

## Configuration Files Summary

| File | Purpose | Location |
|------|---------|----------|
| **racecontrol.toml** | rc-core main config | `/root/racecontrol/` |
| **racecontrol.docker.toml** | Docker variant | `/root/racecontrol/` |
| **rc-agent.example.toml** | rc-agent template | `/root/racecontrol/` |
| **.env** | PM2/environment | `/root/` (cloud) |

---

## Error Handling & Resilience

### Pod Error Aggregator
**Module**: `/root/racecontrol/crates/rc-core/src/error_aggregator.rs`
**Tracking**: Pod errors stored in `pods.error_log` (latest issue)
**Retry logic**: Exponential backoff (1s → 30s) on connection failures
**Recovery**: Pod healer auto-restarts game + cleans up processes

### Failsafe System
**Cloud**: `/root/bono-failsafe.py` (PM2 process)
**Venue**: `/root/james-failsafe.py` (deployed to James machine)
**Heartbeat API**: `POST/GET /api/comms/heartbeat` on gateway
**Interval**: 2 min heartbeats, 6 min alert threshold (3 missed beats)

---

## Third-Party API Limits & Quotas

| Service | Limit | Notes |
|---------|-------|-------|
| **Claude API** | Varies | Token-based billing |
| **Evolution API (WhatsApp)** | 1 msg/sec per instance | Rate limiting per instance |
| **mDNS discovery** | No explicit limit | LAN-based, no external quota |
| **SQLite** | File-based | No user/connection limit |

---

## Summary of Integration Points

**External systems**: 8
- Claude AI (via SDK + CLI)
- Ollama (local LLM)
- Evolution API (WhatsApp)
- Steam (game launcher)
- SQLite (database)
- Cloud API (sync)
- mDNS (discovery)
- UDP telemetry (games)

**Data flows**: 5
- Telemetry ingestion (UDP)
- Game control (WebSocket)
- Cloud sync (HTTPS REST)
- WhatsApp messages (Evolution)
- AI queries (HTTP)

**Protocols**: 6
- UDP (telemetry)
- WebSocket (agent ↔ core)
- HTTPS (cloud sync, AI)
- mDNS (discovery)
- SQLite (local queries)
- Windows APIs (process control)
