# RaceControl Technology Stack

**Date:** March 11, 2026
**Version:** 0.1.0
**Repository:** https://github.com/racingpoint/racecontrol

## Overview
Rust + Next.js monorepo for Racing Point eSports venue management system. Multi-tier architecture with cloud sync, real-time telemetry, AI-driven diagnostics, and billing automation.

---

## Languages & Runtimes

| Component | Language | Version | Notes |
|-----------|----------|---------|-------|
| **rc-core** (backend) | Rust | 1.93.1 (stable-x86_64-pc-windows-msvc) | Axum web server on port 8080 |
| **rc-agent** (pod client) | Rust | 1.93.1 | Gaming pod agent on port 8090 |
| **rc-common** (shared) | Rust | 1.93.1 | Protocol + types, compiled for all crates |
| **Kiosk** (frontend) | TypeScript / Next.js | 16.1.6 / React 19.2.3 | Port 3300 |
| **PWA** | TypeScript / Next.js | 16.1.6 / React 19.2.3 | Mobile customer interface |
| **Admin Web** | TypeScript / React | 19.2.3 | Bono's VPS admin portal |
| **Python (ops)** | Python 3.11+ | (3.8+) | Deployment scripts, web terminal |

---

## Build Configuration

### Rust (Cargo)

**Workspace:** `C:\Users\bono\racingpoint\racecontrol`

```toml
[workspace]
resolver = "2"
members = ["crates/rc-common", "crates/rc-core", "crates/rc-agent"]

[workspace.package]
version = "0.1.0"
edition = "2024"
```

**Build Flags:**
- **Static CRT linking:** `.cargo/config.toml` sets `rustflags = ["-C", "target-feature=+crt-static"]` to eliminate `vcruntime140.dll` dependency on pods
- **Target:** `x86_64-pc-windows-msvc`
- **Cargo PATH:** `$PATH:/c/Users/bono/.cargo/bin` must be exported in bash before `cargo` commands

### Node.js & Next.js

**Kiosk (`kiosk/package.json`):**
```json
{
  "dependencies": {
    "next": "16.1.6",
    "react": "19.2.3",
    "react-dom": "19.2.3"
  },
  "devDependencies": {
    "@tailwindcss/postcss": "^4",
    "tailwindcss": "^4",
    "typescript": "5.9.3"
  },
  "scripts": {
    "dev": "next dev -p 3300 -H 0.0.0.0 --webpack",
    "build": "next build",
    "start": "next start -p 3300"
  }
}
```

---

## Rust Crates & Dependencies

### rc-core (Central Backend Server)

**Binary:** `racecontrol` | **Port:** 8080

#### Web Framework
- **axum** 0.8 (features: ws, macros) — async HTTP server with WebSocket support
- **tower** 0.5 — middleware and service composition
- **tower-http** 0.6 (features: cors, fs, trace) — HTTP utilities
- **tokio-tungstenite** 0.26 — WebSocket client (cloud agent comms)

#### Database
- **sqlx** 0.8 (features: runtime-tokio, sqlite) — SQLite query builder + pool
- **SQLite pragma:** `journal_mode=WAL`, `foreign_keys=ON`

#### Network & Discovery
- **mdns-sd** 0.12 — mDNS discovery for pod auto-discovery
- **reqwest** 0.12 (features: json) — HTTP client (cloud sync, AI, Ollama)
- **urlencoding** 2 — URL encoding for Evolution API instance names

#### Serialization & Time
- **serde** 1 (features: derive) — JSON serialization
- **serde_json** 1 — JSON parsing
- **chrono** 0.4 (features: serde) — Datetime handling
- **uuid** 1 (features: v4, serde) — UUID generation

#### Auth & Crypto
- **jsonwebtoken** 9 — JWT token generation/validation
- **rand** 0.8 — Cryptographic randomness

#### Error Handling & Config
- **anyhow** 1 — Error propagation
- **thiserror** 2 — Custom error types
- **toml** 0.8 — Configuration file parsing

#### Tracing & Logging
- **tracing** 0.1 — Structured logging facade
- **tracing-subscriber** 0.3 (features: env-filter) — Log backend with environment filtering

---

### rc-agent (Pod Gaming Client)

**Binary:** `rc-agent` | **Port:** 8090 (pod-agent), UDP heartbeat 9999

#### WebSocket & Network
- **tokio-tungstenite** 0.26 (features: native-tls) — WebSocket client to rc-core
- **mdns-sd** 0.12 — Pod discovery
- **reqwest** 0.12 (features: json) — HTTP client (AI debugger)

#### Hardware & System Monitoring
- **hidapi** 2 — USB HID access (Conspit Ares 8Nm wheelbase VID:0x1209 PID:0xFFB0)
- **sysinfo** 0.33 — Process list + CPU/GPU metrics
- **winapi** 0.3 (features: processthreadsapi, winnt, handleapi, winuser, memoryapi, basetsd, synchapi, errhandlingapi, winerror, wingdi, libloaderapi) — Windows process management + GUI
- **dirs-next** 2 — Cross-platform directory paths (Documents, AppData)

#### UI & Code Generation
- **qrcode** 0.13 — QR code generation (lock screen authentication)
- **futures-util** 0.3 — Async stream helpers

#### Shared Types
- **rc-common** (workspace) — Protocol enums, types, UDP heartbeat format

---

### rc-common (Shared Protocol Library)

#### Types
- **serde** 1 (features: derive) — JSON serialization for message types
- **serde_json** 1 — JSON utilities
- **chrono** 0.4 (features: serde) — DateTime serialization
- **uuid** 1 (features: v4, serde) — Driver/pod/session IDs

**Key types:**
- `TelemetryFrame` — UDP telemetry from sims (speed, throttle, brake, steering, RPM, gear)
- `SimType` — enum: AssettoCorsaAdapter, F125Adapter (F1 25), iRacing, LMU, Forza, ACEvo
- `DrivingState` — detector: Active, Idle, Loading, Paused, Invalid
- `BillingSessionInfo` — session state: allocated_seconds, driving_seconds, status
- `HeartbeatPing/Pong` — binary UDP packets (12/16 bytes, magic "RP" 0x52 0x50)

**Module:** `crates/rc-common/src/`
- `protocol.rs` — message types (CoreToAgentMessage, AgentToCoreMessage, DashboardEvent, DashboardCommand)
- `udp_protocol.rs` — binary heartbeat format
- `types.rs` — shared data structures

---

## Key Dependencies Summary

| Crate | Purpose | Version | Status |
|-------|---------|---------|--------|
| **axum** | Web framework | 0.8 | Stable, production |
| **sqlx** | Database ORM | 0.8 | Stable, SQLite only |
| **tokio** | Async runtime | 1.x | Full features enabled |
| **serde/serde_json** | JSON serialization | 1 / 1 | Standard |
| **chrono** | DateTime | 0.4 | With serde |
| **uuid** | ID generation | 1 | v4 + serde |
| **reqwest** | HTTP client | 0.12 | JSON support |
| **jsonwebtoken** | JWT auth | 9 | Simple secrets |
| **tracing** | Structured logging | 0.1 | env-filter enabled |
| **hidapi** | USB HID (wheelbase) | 2 | Windows only, critical |
| **sysinfo** | Process monitoring | 0.33 | Pod health checks |
| **winapi** | Windows API | 0.3 | Process mgmt + GUI |
| **qrcode** | QR generation | 0.13 | Lock screen |
| **mdns-sd** | mDNS discovery | 0.12 | Pod auto-discovery |
| **tokio-tungstenite** | WebSocket | 0.26 | Both client & server |

---

## Database

### Type
SQLite (embedded, file-based)

### Path (Configurable)
Default: `C:\Users\bono\racingpoint\racecontrol\racecontrol.db`
Config key: `database.path` in `racecontrol.toml`

### Initialization
- Auto-created on first run by `rc-core/src/db/mod.rs::init_pool()`
- Migrations run on every startup (CREATE TABLE IF NOT EXISTS)
- **Pragma:** WAL mode, foreign keys enabled
- **Pool:** Max 5 connections (sqlx::SqlitePoolOptions)

### Core Tables
- **drivers** — user profiles, trial status, waiver signing
- **billing_sessions** — time-based pricing, session duration, discounts
- **pricing_tiers** — 30min/₹700, 60min/₹900, 5min trial (free)
- **pricing_rules** — dynamic multipliers (peak/off-peak, day-of-week)
- **laps** — completed laps with sector times, validity tracking
- **personal_bests** — per-driver per-track-per-car records
- **track_records** — venue-wide records by track + car
- **pods** — pod registration, online status, current driver
- **kiosk_experiences** — Experience catalog (AC tracks/cars with presets)
- **kiosk_settings** — global settings synced from cloud
- **billing_events** — start/idle/end events for debugging
- **game_launch_events** — error logs + AI suggestions
- **ac_sessions** — AC server LAN session state
- **ac_presets** — AC server configuration templates
- **sync_state** — cloud sync timestamps (pull last_synced_at per table)
- **wallets** — balance_paise, total_credited, total_debited, updated_at

---

## Frontend Stack

### Kiosk (Venue Display & Customer Booking)
- **Framework:** Next.js 16.1.6 (React 19.2.3)
- **Styling:** Tailwind CSS 4 (PostCSS)
- **Port:** 3300
- **Build:** `next build` → standalone server
- **Dev:** `next dev -p 3300 -H 0.0.0.0`

### PWA (Mobile Customer Portal)
- **Framework:** Next.js 16.1.6 (React 19.2.3)
- **Purpose:** QR scan → confirmation → billing
- **Build:** Standalone deployment

### Admin Web (Bono's VPS)
- **Framework:** React 19.2.3 + TypeScript
- **Purpose:** Dashboard, driver management, live pod status
- **Host:** app.racingpoint.cloud (72.60.101.58)

---

## Development Tools

### Rust
- **rustc:** 1.93.1 (stable)
- **cargo:** 1.93.1
- **rust-analyzer:** LSP-enabled (settings.json)
- **Test command:** `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p rc-core`
- **Total tests:** 47 across 3 crates (protocol, driving detector, billing, AI, auto-fix)

### Node.js
- **Version:** 18+ (inferred from Next.js 16.1.6 compatibility)
- **Package manager:** npm (inferred from package.json)
- **TypeScript:** 5.9.3

### Python (Deployment & Ops)
- **Version:** 3.8+
- **Key scripts:**
  - `webterm.py` — web terminal at port 9999 (for Uday's phone)
  - Deployment helpers for HTTP server (`python3 -m http.server`)
  - Health check scripts

---

## Cloud Infrastructure

### Bono's VPS
- **Host:** app.racingpoint.cloud (72.60.101.58)
- **Role:** Cloud rc-core, admin portal, webhook targets
- **API:** `GET /api/v1/sync/changes`, `POST /api/v1/sync/push`
- **Auth:** `x-terminal-secret` header (shared secret in `cloud.terminal_secret`)

### DNS & Service Discovery
- **mDNS:** Pod discovery via `mdns-sd` (broadcast on local network)
- **Pod discovery interval:** Configurable, default 30s

---

## File Locations

| Component | Path |
|-----------|------|
| **Workspace root** | `C:\Users\bono\racingpoint\racecontrol` |
| **rc-core** | `crates/rc-core/src/main.rs` → `racecontrol` binary |
| **rc-agent** | `crates/rc-agent/src/main.rs` → `rc-agent` binary |
| **rc-common** | `crates/rc-common/src/lib.rs` |
| **Kiosk** | `kiosk/` (Next.js project) |
| **Database schema** | `crates/rc-core/src/db/mod.rs` (inline migrations) |
| **Config example** | `racecontrol.toml` (venue root) |
| **Pod config** | `rc-agent-pod{1-8}.toml` (each pod) |
| **Cargo.lock** | `Cargo.lock` (pinned deps) |
| **Deployment kit** | `D:\pod-deploy\` (pendrive: install.bat, binaries, configs) |

---

## Deployment Artifacts

### Binary Sizes (approx, static CRT build)
- **rc-core.exe** — ~15MB (Axum server with full tower stack)
- **rc-agent.exe** — ~12MB (pod client with wheelbbase USB, process monitoring)
- **pod-agent.exe** — ~2MB (lightweight pod command executor)

### Static Assets
- **Kiosk build:** Next.js standalone in `kiosk/.next/standalone/`
- **PWA build:** Similar structure
- **No external CDN required** — all assets bundled

---

## Build & Release Process

1. **Unit tests:** `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p rc-core`
2. **Binary build:** `cargo build --release -p rc-core -p rc-agent`
3. **Static CRT:** Automatically applied via `.cargo/config.toml`
4. **Verification:** Size check, smoke test on Pod 8, connect to core
5. **Deploy:** Copy binaries to `deploy-staging/`, HTTP serve, curl to pod-agent `/exec` endpoint
6. **Cleanup:** Kill old binaries, remove stale files before download

---

## Version Pinning

- **Rust edition:** 2024 (workspace default)
- **Next.js:** Pinned to 16.1.6 (specific version in package.json)
- **React:** Pinned to 19.2.3
- **TypeScript:** Pinned to 5.9.3
- **Tailwind CSS:** v4 (postcss)
- **Cargo.lock:** Committed (for reproducible builds)

---

## Notable Omissions & Constraints

- **No ORM except sqlx:** Raw SQL queries for custom performance
- **No GraphQL:** REST API only
- **No TypeScript in Rust:** Separate type definitions (no code generation)
- **No async file I/O on pods:** Uses sync I/O to avoid cross-thread complexity on Windows
- **No vcruntime dependency:** Static CRT linking avoids runtime DLL installation
- **No Docker build artifacts:** Only cross-platform Rust binaries
