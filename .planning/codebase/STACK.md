# RaceControl Technology Stack

## Overview
RaceControl is a distributed sim racing venue management system built on Rust backend services (Axum) and Next.js/React frontends, with cross-platform Windows/Linux compatibility.

## Backend Stack

### Language & Runtime
- **Rust 1.93.1+** (2024 edition)
- **Tokio async runtime** (full features)
- **Cross-platform**: Compiles for Windows (x86_64) and Linux

### Workspace & Build System
- **Cargo workspaces** with 3 crates: `rc-common`, `rc-core`, `rc-agent`
- **Workspace dependencies** centralized in root `/root/racecontrol/Cargo.toml`
- **Build output**: Target binaries:
  - `racecontrol` (rc-core server, port 8080)
  - `rc-agent` (gaming PC agent, port 18923 lock screen)

#### Workspace Configuration
- Location: `/root/racecontrol/Cargo.toml`
- Members: `crates/rc-common`, `crates/rc-core`, `crates/rc-agent`
- Common version: 0.1.0
- License: MIT
- Repository: https://github.com/racingpoint/racecontrol

### Core Dependencies (Workspace-shared)

| Dependency | Version | Purpose |
|-----------|---------|---------|
| `serde` | 1.x | Serialization/deserialization (derive) |
| `serde_json` | 1.x | JSON encoding |
| `chrono` | 0.4 | Date/time with serde support |
| `uuid` | 1.x | UUID generation (v4, serde) |
| `tracing` | 0.1 | Structured logging |
| `tracing-subscriber` | 0.3 | Logging with env-filter |
| `tokio` | 1.x | Async runtime (full features) |
| `anyhow` | 1.x | Error handling |
| `thiserror` | 2.x | Error definitions |
| `toml` | 0.8 | TOML config parsing |
| `jsonwebtoken` | 9.x | JWT auth |
| `rand` | 0.8 | Random number generation |

### rc-common Crate
**Location**: `/root/racecontrol/crates/rc-common/Cargo.toml`

Shared library with common types and protocols:
- Serialization types (serde)
- UDP telemetry protocol definitions
- Shared data structures

**Dependencies**:
- `serde`, `serde_json`, `chrono`, `uuid` (workspace)

### rc-core Crate (Main Server)
**Location**: `/root/racecontrol/crates/rc-core/Cargo.toml`
**Binary**: `racecontrol` → `/root/racecontrol/crates/rc-core/src/main.rs`
**Port**: 8080 (configurable via racecontrol.toml)

#### Web Framework
- **Axum 0.8**: Async web framework with:
  - WebSocket support (`ws` feature)
  - Macros for routing
- **Tower 0.5**: Middleware composition
- **Tower-HTTP 0.6**:
  - CORS middleware
  - Static file serving (`fs` feature)
  - Request tracing (`trace` feature)

#### Database
- **SQLx 0.8**: Async SQL toolkit
  - Tokio runtime integration
  - SQLite compile-time query verification
  - Database: `/root/racecontrol/data/racecontrol.db`

#### Networking & Communication
- **mDNS-SD 0.12**: Service discovery (pod discovery on LAN)
- **Tokio-Tungstenite 0.26**: WebSocket client/server
- **Futures-util 0.3**: Async combinator utilities
- **Reqwest 0.12**: HTTP client (JSON support, AI/Claude calls)

#### Auth & Security
- **jsonwebtoken 9.x**: JWT encoding/decoding
- **rand 0.8**: Secure random generation

#### Utilities
- **urlencoding 2.x**: URL encoding (Evolution API instance names)

#### Key Source Files
- `/root/racecontrol/crates/rc-core/src/main.rs` → Server entry point
- `/root/racecontrol/crates/rc-core/src/api/` → REST API routes
- `/root/racecontrol/crates/rc-core/src/ws/` → WebSocket handlers
- `/root/racecontrol/crates/rc-core/src/db/` → Database layer
- `/root/racecontrol/crates/rc-core/src/auth/` → JWT & terminal auth
- `/root/racecontrol/crates/rc-core/src/billing.rs` → Billing engine
- `/root/racecontrol/crates/rc-core/src/cloud_sync.rs` → Venue ↔ Cloud sync
- `/root/racecontrol/crates/rc-core/src/udp_heartbeat.rs` → UDP telemetry listener
- `/root/racecontrol/crates/rc-core/src/pod_monitor.rs` → Pod health/status
- `/root/racecontrol/crates/rc-core/src/game_launcher.rs` → Game launch orchestration
- `/root/racecontrol/crates/rc-core/src/catalog.rs` → AC cars/tracks (36 featured cars, 41 featured)
- `/root/racecontrol/crates/rc-core/src/lap_tracker.rs` → Lap/telemetry aggregation
- `/root/racecontrol/crates/rc-core/src/ac_server.rs` → AC dedicated server config
- `/root/racecontrol/crates/rc-core/src/accounting.rs` → Double-entry bookkeeping
- `/root/racecontrol/crates/rc-core/src/ai.rs` → Claude/Ollama AI integration
- `/root/racecontrol/crates/rc-core/src/friends.rs` → Multiplayer presence
- `/root/racecontrol/crates/rc-core/src/multiplayer.rs` → Group booking
- `/root/racecontrol/crates/rc-core/src/pod_healer.rs` → Pod recovery (Steam, Conspit cleanup)
- `/root/racecontrol/crates/rc-core/src/remote_terminal.rs` → PIN-protected terminal
- `/root/racecontrol/crates/rc-core/src/wallet.rs` → Credit system
- `/root/racecontrol/crates/rc-core/src/scheduler.rs` → Cron jobs (review nudges, tournaments)
- `/root/racecontrol/crates/rc-core/src/action_queue.rs` → Async action processing
- `/root/racecontrol/crates/rc-core/src/error_aggregator.rs` → Pod error tracking
- `/root/racecontrol/crates/rc-core/src/activity_log.rs` → Activity audit trail
- `/root/racecontrol/crates/rc-core/src/wol.rs` → Wake-on-LAN for pods

### rc-agent Crate (Gaming PC Agent)
**Location**: `/root/racecontrol/crates/rc-agent/Cargo.toml`
**Binary**: `rc-agent` → `/root/racecontrol/crates/rc-agent/src/main.rs`
**Port**: 18923 (lock screen via WebSocket)
**Deployment**: One instance per gaming pod (Pod 1-8 at 192.168.31.x)

#### Core Dependencies
**Workspace shared**: tokio, serde, chrono, uuid, etc. (same as rc-core)

#### Networking & Game Control
- **Tokio-Tungstenite 0.26**: WebSocket client (with native-tls)
- **Futures-util 0.3**: Async utilities
- **mDNS-SD 0.12**: Pod discovery on LAN

#### Hardware Integration
- **hidapi 2.x**: USB HID library for wheelbase detection
  - Monitors Conspit Ares 8Nm/10Nm/12Nm wheelbase (Vendor ID: 0x1209, Product ID: 0xFFB0)
  - Detects steering wheel input for driving state

#### System Monitoring
- **sysinfo 0.33**: Cross-platform process & system info
  - Game process monitoring (AC, iRacing, F1 25, Forza, LMU)
  - CPU/memory tracking

#### Game & UI
- **qrcode 0.13**: QR code generation for lock screen (PIN-based remote access)
- **dirs-next 2.x**: Cross-platform directory paths (Documents, AppData)

#### AI Debugging
- **reqwest 0.12**: HTTP client for Ollama/Claude API calls

#### Windows-Specific
**Target-gated features** (`[target.'cfg(windows)'.dependencies]`):
- **winapi 0.3**: Windows API bindings with features:
  - `processthreadsapi`: Process creation/termination
  - `winnt`: Windows NT types
  - `handleapi`: Handle management
  - `winuser`: Window/message APIs
  - `memoryapi`: Memory management
  - `basetsd`: Base types
  - `synchapi`: Synchronization primitives
  - `errhandlingapi`: Windows error handling
  - `winerror`: Error codes
  - `wingdi`: Graphics Device Interface
  - `libloaderapi`: DLL loading

#### Key Source Files
- `/root/racecontrol/crates/rc-agent/src/main.rs` → Agent entry point
- `/root/racecontrol/crates/rc-agent/src/game_process.rs` → Game launch/kill
- `/root/racecontrol/crates/rc-agent/src/ac_launcher.rs` → Assetto Corsa launch (Steam app ID 244210)
- `/root/racecontrol/crates/rc-agent/src/kiosk.rs` → Kiosk UI (staff PIN login, game selection)
- `/root/racecontrol/crates/rc-agent/src/lock_screen.rs` → Lock screen with PIN/QR code
- `/root/racecontrol/crates/rc-agent/src/overlay.rs` → In-game overlay (telemetry, timers)
- `/root/racecontrol/crates/rc-agent/src/driving_detector.rs` → HID + UDP active detection
- `/root/racecontrol/crates/rc-agent/src/debug_server.rs` → Debug CLI interface
- `/root/racecontrol/crates/rc-agent/src/ai_debugger.rs` → Ollama/Claude integration (Qwen2.5-coder:14b preferred)
- `/root/racecontrol/crates/rc-agent/src/udp_heartbeat.rs` → UDP telemetry listener
- `/root/racecontrol/crates/rc-agent/src/sims/` → Game-specific modules

## Frontend Stack

### PWA (Customer-facing)
**Location**: `/root/racecontrol/pwa/`
**Port**: 3100 (dev), configurable in production
**Package**: `/root/racecontrol/pwa/package.json`

| Dependency | Version | Purpose |
|-----------|---------|---------|
| **Next.js** | 16.1.6 | React framework, SSR/SSG |
| **React** | 19.2.3 | UI library |
| **React-DOM** | 19.2.3 | DOM rendering |
| **recharts** | 3.8.0 | Charts/graphs (telemetry visualization) |
| **html5-qrcode** | 2.3.8 | QR code scanning (customer scan-to-book) |
| **Tailwind CSS** | 4.x | Utility-first styling |
| **TypeScript** | 5.x | Type safety |

**Pages**: `/`, `/login`, `/register`, `/dashboard`, `/book`, `/book/active`, `/book/group`, `/sessions`, `/sessions/[id]`, `/stats`, `/leaderboard`, `/leaderboard/public`, `/telemetry`, `/friends`, `/tournaments`, `/coaching`, `/ai`, `/profile`, `/scan`, `/terminal`

**Key routes**:
- `/book` - Custom booking wizard (8-step flow)
- `/sessions/[id]` - Session details + shareable report
- `/leaderboard/public` - Public leaderboard (no auth)
- `/coaching` - Sector analysis + lap comparison
- `/tournaments` - Tournament bracket display

### Kiosk (On-venue staff UI)
**Location**: `/root/racecontrol/kiosk/`
**Port**: 3300
**Package**: `/root/racecontrol/kiosk/package.json`

| Dependency | Version | Purpose |
|-----------|---------|---------|
| **Next.js** | 16.1.6 | React framework |
| **React** | 19.2.3 | UI library |
| **React-DOM** | 19.2.3 | DOM rendering |
| **Tailwind CSS** | 4.x | Styling |
| **TypeScript** | 5.9.3 | Type safety |

**Key features**:
- Staff PIN login
- Game configurator (car selection, track, etc.)
- Direct launch flow (no customer PIN required)
- Pod status grid

### Web (Legacy/Admin UI)
**Location**: `/root/racecontrol/web/`
**Port**: 3000 (dev)
**Package**: `/root/racecontrol/web/package.json`

| Dependency | Version | Purpose |
|-----------|---------|---------|
| **Next.js** | 16.1.6 | React framework |
| **React** | 19.2.3 | UI library |
| **React-DOM** | 19.2.3 | DOM rendering |
| **socket.io-client** | 4.8.3 | Real-time WebSocket events |
| **Tailwind CSS** | 4.x | Styling |
| **TypeScript** | 5.x | Type safety |

## Configuration Management

### rc-core Configuration
**File**: `/root/racecontrol/racecontrol.toml`

**Sections**:
- `[venue]` - Venue name, location, timezone
- `[server]` - Host, port (8080)
- `[database]` - SQLite path (`./data/racecontrol.db`)
- `[cloud]` - Cloud API URL, sync interval (30s), terminal auth
- `[watchdog]` - Pod health check settings
- `[pods]` - Count (8), discovery, healer settings
- `[branding]` - Primary color, theme
- `[ai_debugger]` - Claude CLI timeout (30s), Ollama settings, chat enabled
- `[ac_server]` - AC server path, data directory
- `[auth]` - JWT secret, Evolution API credentials
- `[integrations.discord]` - Discord channel names
- `[integrations.whatsapp]` - WhatsApp contact

**Docker variant**: `/root/racecontrol/racecontrol.docker.toml`

### rc-agent Configuration
**File**: `/root/racecontrol/rc-agent.example.toml` (template)
**Deployed to**: Each pod (e.g., Pod 1: `C:\racecontrol\rc-agent.toml`)

**Sections**:
- `[agent]` - Pod name, pod number, server WebSocket URL
- `[games.*]` - Per-game config (steam_app_id or exe_path)
  - assetto_corsa: 244210
  - f1_25: 2488620
  - le_mans_ultimate: 1564310
  - forza: 2440510
  - iracing: Direct exe path
- `[ai_debugger]` - Ollama URL, model (qwen2.5-coder:14b), API key fallback

## Database

**Type**: SQLite 3
**Location**: `/root/racecontrol/data/racecontrol.db`
**Access Layer**: SQLx with compile-time query checking

**Key tables**:
- `drivers` - Customer accounts (phone, steam_guid, iracing_id)
- `pods` - Pod configuration & status
- `billing_sessions` - Drive sessions + payment
- `wallet_transactions` - Credit ledger (debit/credit/refund)
- `laps` - Lap telemetry aggregates (track, car, time, etc.)
- `pricing_rules` - Dynamic pricing multipliers by day/hour
- `coupons` - Discount codes (percent/flat/free_minutes)
- `packages` - 5 seeded: Date Night, Squad, Birthday, Corporate, Student
- `memberships` - 3 tiers: Rookie/Pro/Champion
- `tournaments` - Full bracket system with auto-advance
- `time_trials` - Weekly leaderboards
- `referrals` - Referral codes + auto-credit
- `friends` - Multiplayer friend requests
- `journal_entries` - Double-entry accounting
- `audit_log` - Soft-delete audit trail (pricing_rules, coupons)
- `review_nudges` - Post-session review requests
- `kiosk_experiences` - Per-pod game configs
- `heartbeats` - Cloud sync health checks

## Build & Deployment

### Rust Build
```bash
cd /root/racecontrol
cargo build --release
```

**Binaries**:
- `target/release/racecontrol` (rc-core, ~10-15MB)
- `target/release/rc-agent` (rc-agent, ~8-12MB)

### Next.js Build
```bash
cd /root/racecontrol/pwa
npm run build
npm run start -p 3100
```

### Docker
**Dockerfile**: `/root/racecontrol/Dockerfile`
- Multi-stage build (Rust + Node)
- Runs rc-core on port 8080
- Configured via environment variables

### PM2 Deployment (Cloud)
- `racecontrol` → rc-core on port 8080
- `racingpoint-dashboard` → Next.js dashboard on port 3400
- `racingpoint-admin` → Admin UI on port 3200

### Windows Deployment (Venues)
- **PowerShell deployment scripts**:
  - `/root/racecontrol/deploy_pod8.ps1`
  - `/root/racecontrol/deploy_pod8_v2.ps1`
  - `/root/racecontrol/deploy_watchdogs.ps1`
- **Python helper scripts**:
  - `/root/racecontrol/fix_conspit.py` - Wheelbase cleanup
  - `/root/racecontrol/launch_ac.py` - AC launch wrapper
  - `/root/racecontrol/pod_ac_launch.py` - Pod-specific AC launcher

## Third-Party Integrations

- **Claude AI**: Via Anthropic SDK (rc-core `ai.rs`) or Claude CLI (rc-agent `ai_debugger.rs`)
- **Ollama**: Local LLM on venue James machine (llama3.1:8b or qwen2.5-coder:14b)
- **Evolution API**: WhatsApp gateway (http://localhost:53622)
- **Steam**: Game launch via steam app IDs + DirectX 11
- **mDNS**: Pod discovery protocol
- **WebSocket**: Agent ↔ Core real-time communication

## Summary Statistics
- **Rust crates**: 3
- **Next.js apps**: 3 (pwa, kiosk, web)
- **Database tables**: 30+
- **REST API endpoints**: 50+
- **WebSocket handlers**: 20+
- **Lines of Rust code**: ~15,000
- **Lines of TypeScript/React**: ~8,000
