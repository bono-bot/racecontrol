# RaceControl Directory Structure

## Root Repository Layout

```
racecontrol/
├── Cargo.toml                           # Workspace definition (rc-common, rc-core, rc-agent)
├── Cargo.lock                           # Dependency lock file
├── racecontrol.toml                     # Server config (venue, database, cloud, auth)
├── racecontrol.docker.toml              # Docker override config
├── rc-agent.example.toml                # Example pod agent config
│
├── crates/                              # Rust workspace
│   ├── rc-common/                       # Shared types & protocol library
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs                   # Module declarations
│   │       ├── types.rs                 # Core domain types (1,613 LOC total)
│   │       ├── protocol.rs              # WebSocket message enums
│   │       └── udp_protocol.rs          # UDP telemetry parsers
│   │
│   ├── rc-core/                         # Central server (Axum + SQLite)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs                  # Server initialization (port 8080)
│   │       ├── state.rs                 # AppState: pods, db, WebSocket multiplexer
│   │       ├── config.rs                # TOML config loader (VenueConfig, etc)
│   │       │
│   │       ├── db/
│   │       │   └── mod.rs               # SQLite schema & migrations
│   │       │
│   │       ├── api/                     # HTTP REST endpoints
│   │       │   ├── mod.rs               # Router setup
│   │       │   └── routes.rs            # All REST routes (100+ endpoints)
│   │       │
│   │       ├── ws/                      # WebSocket handlers
│   │       │   └── mod.rs               # agent_ws, dashboard_ws, message routing
│   │       │
│   │       ├── pod_*.rs                 # Pod lifecycle
│   │       │   ├── pod_monitor.rs       # Heartbeat monitoring
│   │       │   ├── pod_healer.rs        # Automatic recovery & WoL
│   │       │   ├── pod_reservation.rs   # Pod availability & allocation
│   │       │   └── wol.rs               # Wake-on-LAN magic packet
│   │       │
│   │       ├── billing.rs               # Billing engine (1,434 LOC)
│   │       ├── accounting.rs            # Accounting & ledger
│   │       ├── wallet.rs                # Customer wallet management
│   │       │
│   │       ├── game_*.rs                # Game lifecycle
│   │       │   ├── game_launcher.rs     # Launch/stop games
│   │       │   ├── ac_server.rs         # Assetto Corsa server management
│   │       │   ├── ac_camera.rs         # Camera control (Dahua RTSP)
│   │       │   └── catalog.rs           # AC track/car metadata
│   │       │
│   │       ├── session_*.rs             # Session & lap tracking
│   │       │   ├── lap_tracker.rs       # Lap completion detection
│   │       │   └── multiplayer.rs       # Group sessions & scoring
│   │       │
│   │       ├── auth/                    # Authentication
│   │       │   └── mod.rs               # JWT, PIN, OTP validation
│   │       │
│   │       ├── ai.rs                    # AI debugging integration
│   │       ├── error_aggregator.rs      # API error tracking & escalation
│   │       ├── remote_terminal.rs       # SSH-like command execution
│   │       ├── cloud_sync.rs            # Cloud ↔ local sync (828 LOC)
│   │       ├── scheduler.rs             # Periodic tasks (healer, sync, watchdog)
│   │       ├── friends.rs               # Friend list & grouping
│   │       ├── activity_log.rs          # Audit trail
│   │       └── udp_heartbeat.rs         # UDP keep-alive
│   │
│   └── rc-agent/                        # Pod client agent
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs                  # Entry point, config load, task spawning
│           │
│           ├── game_*.rs                # Game lifecycle
│           │   ├── game_process.rs      # Process state tracking & restart
│           │   ├── ac_launcher.rs       # AC launch via Content Manager
│           │   └── sims/
│           │       ├── mod.rs           # SimAdapter trait
│           │       ├── assetto_corsa.rs # AC UDP telemetry parser
│           │       └── f1_25.rs         # F1 25 UDP telemetry parser
│           │
│           ├── driving_detector.rs      # USB HID wheelbase monitoring
│           ├── udp_heartbeat.rs         # Pod ↔ core heartbeat
│           │
│           ├── lock_screen.rs           # Fullscreen lock screen UI
│           ├── kiosk.rs                 # Fullscreen kiosk mode
│           ├── overlay.rs               # In-game HUD overlay
│           │
│           ├── ai_debugger.rs           # Crash analysis & auto-fix
│           └── debug_server.rs          # HTTP debug endpoint (port 8090)
│
├── kiosk/                               # In-venue reception Next.js app (port 3300)
│   ├── package.json                     # Next.js 16 + React 19
│   ├── package-lock.json
│   ├── tsconfig.json
│   ├── next.config.ts
│   ├── postcss.config.mjs
│   ├── .next/                           # Build output (ignored in git)
│   │
│   └── src/
│       ├── app/                         # Next.js app directory
│       │   ├── layout.tsx               # Root layout
│       │   ├── page.tsx                 # Home/splash screen
│       │   ├── book/
│       │   │   └── page.tsx             # Experience booking UI
│       │   ├── pod/
│       │   │   └── [number]/
│       │   │       └── page.tsx         # Individual pod control
│       │   ├── control/
│       │   │   └── page.tsx             # Master pod array control
│       │   ├── debug/
│       │   │   └── page.tsx             # Staff debugging interface
│       │   ├── settings/
│       │   │   └── page.tsx             # Venue configuration
│       │   ├── spectator/
│       │   │   └── page.tsx             # Live spectating feeds
│       │   └── staff/
│       │       └── page.tsx             # Staff login & management
│       │
│       ├── components/                  # React components (TailwindCSS)
│       │   ├── KioskHeader.tsx
│       │   ├── KioskPodCard.tsx         # Pod status card
│       │   ├── ExperienceSelector.tsx   # Track/car/difficulty picker
│       │   ├── DriverRegistration.tsx   # QR + PIN entry
│       │   ├── GameConfigurator.tsx     # Game preset manager
│       │   ├── LiveTelemetry.tsx        # Real-time speed/gear display
│       │   ├── LiveLapTicker.tsx        # Lap time feed
│       │   ├── SessionTimer.tsx         # Billing countdown
│       │   ├── F1Speedometer.tsx        # RPM gauge
│       │   ├── LiveSessionPanel.tsx     # Current session info
│       │   ├── SidePanel.tsx            # Navigation sidebar
│       │   ├── StaffLoginScreen.tsx     # Staff authentication
│       │   ├── WalletTopup.tsx          # Credit recharge UI
│       │   ├── WalletTopupPanel.tsx     # Panel variant
│       │   ├── AssistanceAlert.tsx      # Error/alert display
│       │   ├── PodKioskView.tsx         # Pod detail view
│       │   └── SetupWizard.tsx          # Multi-step setup flow
│       │
│       ├── hooks/
│       │   ├── useKioskSocket.ts        # WebSocket to rc-core
│       │   └── useSetupWizard.ts        # Setup state machine
│       │
│       ├── lib/
│       │   ├── api.ts                   # HTTP client for REST endpoints
│       │   └── types.ts                 # TypeScript type definitions
│       │
│       └── public/                      # Static assets
│           └── ...
│
├── pwa/                                 # Mobile customer PWA (port 3100)
│   ├── package.json                     # Next.js 16 + React 19 + Recharts
│   ├── package-lock.json
│   ├── tsconfig.json
│   ├── next.config.ts
│   ├── postcss.config.mjs
│   ├── .next/                           # Build output (ignored in git)
│   │
│   └── src/
│       ├── app/                         # Next.js app directory
│       │   ├── layout.tsx               # Root layout + navigation
│       │   ├── page.tsx                 # Dashboard home
│       │   ├── login/
│       │   │   └── page.tsx             # QR + OTP login
│       │   ├── register/
│       │   │   └── page.tsx             # New driver signup
│       │   ├── scan/
│       │   │   └── page.tsx             # QR scanner for pod pairing
│       │   ├── book/
│       │   │   ├── page.tsx             # Booking interface
│       │   │   ├── active/
│       │   │   │   └── page.tsx         # Active sessions
│       │   │   └── group/
│       │   │       └── page.tsx         # Group/tournament booking
│       │   ├── sessions/
│       │   │   ├── page.tsx             # Session history
│       │   │   └── [id]/
│       │   │       └── page.tsx         # Session detail + replay
│       │   ├── leaderboard/
│       │   │   ├── page.tsx             # Global rankings
│       │   │   └── public/
│       │   │       └── page.tsx         # Public leaderboard
│       │   ├── stats/
│       │   │   └── page.tsx             # Personal statistics
│       │   ├── telemetry/
│       │   │   └── page.tsx             # Live telemetry visualization
│       │   ├── coaching/
│       │   │   └── page.tsx             # AI coaching insights
│       │   ├── friends/
│       │   │   └── page.tsx             # Friend list management
│       │   ├── profile/
│       │   │   └── page.tsx             # User profile & settings
│       │   ├── tournaments/
│       │   │   └── page.tsx             # Event/tournament browser
│       │   ├── ai/
│       │   │   └── page.tsx             # AI debugging chat
│       │   ├── terminal/
│       │   │   └── page.tsx             # Web terminal (Uday staff only)
│       │   └── dashboard/
│       │       ├── layout.tsx           # Dashboard layout wrapper
│       │       └── page.tsx             # Main dashboard
│       │
│       ├── components/
│       │   ├── BottomNav.tsx            # Mobile bottom navigation
│       │   ├── SessionCard.tsx          # Session summary card
│       │   └── TelemetryChart.tsx       # Lap visualization (Recharts)
│       │
│       └── lib/
│           └── api.ts                   # HTTP client for REST endpoints
│
├── web/                                 # Admin web dashboard (port 3200, legacy)
│   ├── package.json
│   ├── tsconfig.json
│   ├── next.config.ts
│   └── src/
│       └── ...
│
├── data/                                # Runtime data (SQLite, AC server configs)
│   ├── racecontrol.db                   # SQLite database
│   ├── racecontrol.db-shm              # SQLite WAL temporary file
│   ├── racecontrol.db-wal              # SQLite WAL write-ahead log
│   └── ac_servers/                      # AC server presets & configs
│       └── RP_OPTIMAL/                  # Example AC preset
│           ├── setup.ini
│           ├── weather.ini
│           └── ...
│
├── deploy/                              # Deployment & CI/CD
│   ├── Dockerfile                       # Docker image for cloud rc-core
│   └── ...
│
├── docs/                                # Documentation
│   └── debugging-playbook.md            # Pod troubleshooting guide
│
├── training/                            # QLoRA fine-tuning data
│   ├── training_pairs.json              # 135 pairs for pod AI model
│   ├── Modelfile                        # Ollama model definition
│   ├── training_script.py               # Unsloth training
│   └── convert_to_gguf.py               # GGUF conversion
│
├── scripts/                             # Utility scripts
│   ├── pod_deploy_and_test.py
│   ├── pod_diagnostic.py
│   ├── pod_fix.py
│   ├── pod_investigate.py
│   ├── pod_verify.py
│   └── ...
│
├── .planning/                           # Planning & architecture docs
│   └── codebase/
│       ├── ARCHITECTURE.md              # System design & data flow
│       └── STRUCTURE.md                 # This file
│
├── pod-scripts/                         # Pod-specific scripts
│   └── ...
│
├── assets/                              # Branding & images
│   └── ...
│
├── target/                              # Cargo build output (ignored in git)
│   ├── debug/
│   ├── release/
│   └── ...
│
├── .git/                                # Git repository
├── .gitignore                           # Ignore: target/, .next/, node_modules/
├── .dockerignore                        # Docker: ignore git, target, node_modules
│
├── README.md                            # Project overview
└── Dockerfile                           # Docker image definition (cloud)
```

---

## Key File Paths (Absolute)

### Rust Workspace
- **Workspace root:** `C:\Users\bono\racingpoint\racecontrol\Cargo.toml`
- **rc-common lib:** `C:\Users\bono\racingpoint\racecontrol\crates\rc-common\src\lib.rs`
- **rc-core main:** `C:\Users\bono\racingpoint\racecontrol\crates\rc-core\src\main.rs`
- **rc-agent main:** `C:\Users\bono\racingpoint\racecontrol\crates\rc-agent\src\main.rs`

### Core Modules
- **Shared types:** `C:\Users\bono\racingpoint\racecontrol\crates\rc-common\src\types.rs`
- **Protocol:** `C:\Users\bono\racingpoint\racecontrol\crates\rc-common\src\protocol.rs`
- **Server state:** `C:\Users\bono\racingpoint\racecontrol\crates\rc-core\src\state.rs`
- **API routes:** `C:\Users\bono\racingpoint\racecontrol\crates\rc-core\src\api\routes.rs`
- **WebSocket:** `C:\Users\bono\racingpoint\racecontrol\crates\rc-core\src\ws\mod.rs`
- **Billing:** `C:\Users\bono\racingpoint\racecontrol\crates\rc-core\src\billing.rs`
- **Pod healer:** `C:\Users\bono\racingpoint\racecontrol\crates\rc-core\src\pod_healer.rs`
- **Cloud sync:** `C:\Users\bono\racingpoint\racecontrol\crates\rc-core\src\cloud_sync.rs`

### Pod Agent Modules
- **Driving detector:** `C:\Users\bono\racingpoint\racecontrol\crates\rc-agent\src\driving_detector.rs`
- **AI debugger:** `C:\Users\bono\racingpoint\racecontrol\crates\rc-agent\src\ai_debugger.rs`
- **AC launcher:** `C:\Users\bono\racingpoint\racecontrol\crates\rc-agent\src\ac_launcher.rs`
- **Lock screen:** `C:\Users\bono\racingpoint\racecontrol\crates\rc-agent\src\lock_screen.rs`
- **Sim adapters:** `C:\Users\bono\racingpoint\racecontrol\crates\rc-agent\src\sims\mod.rs`

### Frontend Apps
- **Kiosk source:** `C:\Users\bono\racingpoint\racecontrol\kiosk\src\app\`
- **Kiosk components:** `C:\Users\bono\racingpoint\racecontrol\kiosk\src\components\`
- **PWA source:** `C:\Users\bono\racingpoint\racecontrol\pwa\src\app\`
- **PWA components:** `C:\Users\bono\racingpoint\racecontrol\pwa\src\components\`

### Configuration
- **Server config:** `C:\Users\bono\racingpoint\racecontrol\racecontrol.toml`
- **Example agent config:** `C:\Users\bono\racingpoint\racecontrol\rc-agent.example.toml`
- **Kiosk package.json:** `C:\Users\bono\racingpoint\racecontrol\kiosk\package.json`
- **PWA package.json:** `C:\Users\bono\racingpoint\racecontrol\pwa\package.json`

### Data & Deployment
- **SQLite database:** `C:\Users\bono\racingpoint\racecontrol\data\racecontrol.db`
- **AC server presets:** `C:\Users\bono\racingpoint\racecontrol\data\ac_servers\`
- **Deploy kit (pendrive):** `D:\pod-deploy\`
- **Deploy staging:** `C:\Users\bono\racingpoint\deploy-staging\`

### Documentation
- **Architecture guide:** `C:\Users\bono\racingpoint\racecontrol\.planning\codebase\ARCHITECTURE.md`
- **Structure guide:** `C:\Users\bono\racingpoint\racecontrol\.planning\codebase\STRUCTURE.md`
- **Debugging playbook:** `C:\Users\bono\racingpoint\racecontrol\docs\debugging-playbook.md`

---

## Naming Conventions

### Rust Files
- **Module files:** Snake case (`ai_debugger.rs`, `pod_healer.rs`, `game_launcher.rs`)
- **Structs:** PascalCase (`AppState`, `BillingManager`, `PodInfo`)
- **Enums:** PascalCase (`SimType`, `PodStatus`, `DrivingState`)
- **Functions:** Snake case (`handle_agent`, `start_billing_session`)
- **Constants:** UPPER_SNAKE (`PROTECTED_PROCESSES`, `DEFAULT_PORT`)

### TypeScript/Next.js Files
- **Pages:** Snake case inside `app/` directory, PascalCase component exports (`page.tsx` → exports Page component)
- **Components:** PascalCase (`KioskPodCard.tsx`, `ExperienceSelector.tsx`)
- **Hooks:** camelCase with `use` prefix (`useKioskSocket.ts`, `useSetupWizard.ts`)
- **Utilities:** camelCase (`api.ts`, `types.ts`)

### Database Tables
- Snake case: `drivers`, `pods`, `sessions`, `laps`, `billing_sessions`, `wallets`

### Configuration Sections
- Snake case: `[venue]`, `[server]`, `[database]`, `[cloud]`, `[pods]`, `[ai_debugger]`

---

## Module Organization Principles

### rc-common (Types Library)
- **Compact:** Single responsibility per type
- **Serializable:** All types implement Serialize/Deserialize
- **No dependencies:** Only serde, chrono, uuid from workspace
- **Protocol enums:** Tag-based for JSON (e.g., `AgentMessage::Register(...)`)

### rc-core (Server)
- **Layered:** DB layer → business logic → API routes → WebSocket
- **Manager pattern:** BillingManager, GameManager, AcServerManager (stateful)
- **Module per domain:** `billing.rs`, `pod_healer.rs`, `cloud_sync.rs`
- **Central state:** AppState holds all shared resources (db, http_client, broadcast channels)

### rc-agent (Pod Client)
- **Task-based:** Main spawns independent tasks (driving_detector, game_process, etc)
- **Trait abstraction:** SimAdapter for multi-sim support
- **Direct USB/UDP:** HID and UDP I/O on pod hardware

### Frontends (Next.js)
- **App router:** Each page is a route (app/book/page.tsx → /book)
- **Components:** Reusable React components in components/ folder
- **Hooks:** Custom hooks for stateful logic (WebSocket, form state)
- **Lib:** API client and type definitions

---

## Build & Compilation

### Rust Build
```bash
# From C:\Users\bono\racingpoint\racecontrol\
cargo build                              # Debug build
cargo build --release                    # Optimized build
cargo test                               # Run all tests
cargo build -p rc-core --release         # Single crate
cargo build -p rc-agent --release
```

### Outputs
- **rc-core binary:** `target/release/racecontrol.exe` (port 8080)
- **rc-agent binary:** `target/release/rc-agent.exe` (pod client)

### Frontend Build
```bash
# Kiosk (port 3300)
cd kiosk
npm install
npm run dev                              # Development
npm run build && npm start               # Production

# PWA (port 3100)
cd pwa
npm install
npm run dev
npm run build && npm start
```

### Docker
```bash
# From root
docker build -t racingpoint-core:latest .
docker run -p 8080:8080 racingpoint-core:latest
```

---

## Database Schema (SQLite)

**Tables:**
- `drivers` - Driver profiles (id, name, email, phone, steam_guid, iracing_id, total_laps, total_time_ms, created_at)
- `pods` - Pod inventory (id, number, name, ip_address, sim_type, status, current_driver_id, current_session_id)
- `sessions` - Racing sessions (id, pod_id, driver_id, started_at, ended_at, lap_count, best_lap_ms)
- `laps` - Individual lap records (id, session_id, lap_number, time_ms, track_id, car_id, sector_times)
- `billing_sessions` - Billing records (id, driver_id, pod_id, started_at, ended_at, duration_ms, cost_credits, status)
- `pricing_tiers` - Pricing configs (id, name, duration_minutes, rate_per_minute, type)
- `wallets` - Customer credit (driver_id, balance_credits, updated_at)
- `friends` - Friend relationships (id, driver_id_a, driver_id_b, status)
- `tournaments` - Events (id, name, track_id, car_id, start_at, prize_pool)

---

## Environment & Configuration Priority

1. Environment variables (highest priority)
   - `OLLAMA_URL`, `OLLAMA_MODEL`, `ANTHROPIC_API_KEY`
2. `racecontrol.toml` in current directory
3. `/etc/racecontrol/racecontrol.toml` (Linux)
4. Built-in defaults (lowest priority)

---

## Testing Structure

**Unit tests:** Located in same module with `#[cfg(test)]`
- `driving_detector.rs::tests` - Input parsing, idle detection
- `billing.rs::tests` - Cost calculation, refunds
- `protocol.rs::tests` - Serialization roundtrips

**Integration tests:** `tests/` directory (if added)

**Test runner:**
```bash
cargo test -p rc-common
cargo test -p rc-core
cargo test -p rc-agent
```

---

## Important Notes

### File Encoding
- All Rust files: UTF-8
- All TypeScript files: UTF-8
- TOML configs: UTF-8

### Line Endings
- `.gitconfig` set to `core.autocrlf=input` (LF on Linux, CRLF normalized on Windows)

### Dependencies
- **Rust:** rustc 1.93.1 (stable-x86_64-pc-windows-msvc)
- **Node.js:** Latest (for npm/Next.js)
- **Cargo:** 1.93.1

### Deployment Artifacts
- Pod binary: `rc-agent.exe` → `D:\pod-deploy\` → copied to each pod
- Core binary: `racecontrol.exe` → runs on James's machine or cloud (Bono's VPS)
- Frontend builds: `.next/standalone` → deployed to Vercel or on-premise

### Special Files (Not in Git)
- `target/` - Cargo build artifacts
- `.next/` - Next.js build output
- `node_modules/` - npm dependencies
- `*.db`, `*.db-shm`, `*.db-wal` - SQLite runtime files (exception: tracked for example schema)
