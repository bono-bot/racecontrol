# RaceControl External Integrations

**Date:** March 11, 2026
**Version:** 0.1.0

## Overview
Multi-source data architecture with cloud sync, real-time telemetry from 6 racing sims, AI-driven diagnostics, camera control, billing automation, and venue hardware integration.

---

## Cloud Sync (Bono's VPS)

### Service
- **Host:** app.racingpoint.cloud (72.60.101.58, Bono's VPS)
- **API Base:** `https://app.racingpoint.cloud/api/v1`
- **Protocol:** HTTPS REST

### Endpoints

#### Pull Changes
```
GET /api/v1/sync/changes?since=<timestamp>&tables=drivers,wallets,pricing_tiers,pricing_rules,kiosk_experiences,kiosk_settings
Headers:
  x-terminal-secret: <shared_secret>

Response:
{
  "drivers": [...],
  "wallets": [...],
  "pricing_tiers": [...],
  "pricing_rules": [...],
  "kiosk_experiences": [...],
  "kiosk_settings": {...},
  "synced_at": "2026-03-11T14:30:00Z"
}
```

#### Push Local Data
```
POST /api/v1/sync/push
Headers:
  x-terminal-secret: <shared_secret>
  Content-Type: application/json

Body:
{
  "laps": [...],
  "track_records": [...],
  "personal_bests": [...],
  "billing_sessions": [...],
  "drivers": [...],
  "wallets": [...],
  "pods": [...]
}

Response:
{ "upserted": 123 }
```

### Sync Strategy
- **Interval:** Every 30 seconds (configurable `cloud.sync_interval_secs`)
- **Pull authority:** Cloud authoritative for: drivers, pricing, experiences
- **Local authority:** Venue authoritative for: billing, laps, game state
- **ID resolution:** Driver phone/email match when cloud UUID ≠ local UUID
- **Implementation:** `crates/rc-core/src/cloud_sync.rs`
- **Timeout:** 15s for pull, 30s for push
- **Sync state tracking:** `sync_state` table with `last_synced_at` per table

### Configuration (`racecontrol.toml`)
```toml
[cloud]
enabled = true
api_url = "https://app.racingpoint.cloud/api/v1"
sync_interval_secs = 30
action_poll_interval_secs = 3
terminal_secret = "<shared_secret>"
terminal_pin = "<4_digits>"  # Uday only
```

---

## AI Integrations

### Priority Stack
1. **Claude CLI** (local, on James's machine)
   - Command: `claude -p --output-format text`
   - Timeout: 30s default, configurable `claude_cli_timeout_secs`
   - Input: prompt via stdin
   - Output: plain text response

2. **Ollama** (on-site venue GPU)
   - **Host:** James's RTX 4070 (192.168.31.27)
   - **Port:** 11434 (default Ollama port)
   - **Endpoint:** `http://localhost:11434/api/chat`
   - **Model:** Configurable, default from training/ QLoRA fine-tune
   - **Method:** POST with message array, stream=false
   - **Timeout:** 60s

3. **Anthropic Messages API** (cloud fallback)
   - **Endpoint:** `https://api.anthropic.com/v1/messages`
   - **Auth:** `x-api-key: <ANTHROPIC_API_KEY>`
   - **Model:** Claude 3 Opus (default)
   - **Features:** System prompt separation, temperature 0.7

### Config
```toml
[ai_debugger]
enabled = true
claude_cli_enabled = true
claude_cli_timeout_secs = 30
ollama_url = "http://localhost:11434"
ollama_model = "racecontrol-fine-tuned"
anthropic_api_key = "<key>"
anthropic_model = "claude-3-opus-20250219"
chat_enabled = true
proactive_analysis = true
```

### Use Cases
- **Crash analysis:** Pod state snapshot → AI suggests fixes
- **Edge stacking:** ConspitLink.exe persistence → auto-kill both msedge.exe + msedgewebview2.exe
- **Game issues:** Telemetry gaps, invalid laps → auto-repair suggestions
- **Training pairs:** Claude CLI responses logged to DB for Ollama QLoRA (135+ pairs)
- **Implementation:** `crates/rc-core/src/ai.rs`, `crates/rc-agent/src/ai_debugger.rs`

---

## Game Telemetry (UDP & Shared Memory)

### Racing Simulators

#### Assetto Corsa (AC)
- **Protocol:** Windows shared memory (IPC)
- **Files:** `acpmf_physics`, `acpmf_graphics`, `acpmf_static`
- **Updated:** Every frame (~60Hz) for physics, ~10Hz for graphics
- **Data read:** Speed (km/h), throttle, brake, steering, RPM, gear, lap time, sector times
- **Sector tracking:** From `currentSectorIndex` (0/1/2) + `lastSectorTime` (ms)
- **Lap completion:** Via `completedLaps` counter increment
- **Adapter:** `crates/rc-agent/src/sims/assetto_corsa.rs`
- **Reference:** AC forum shared memory doc (offsets for physics/graphics/static structs)

#### F1 25 (EA Sports F1)
- **Protocol:** UDP multicast (broadcast)
- **Port:** 20777
- **Packet size:** Header (29B) + variable per-car data
- **Packet types:** 1=Session, 2=LapData, 4=Participants, 6=CarTelemetry, 7=CarStatus
- **Per-car data:** 60B (telemetry), 57B (lapdata)
- **Data:** Speed (u16 km/h), throttle, brake, steer, gear (i8), RPM, DRS, ERS, sector times
- **Session:** Track ID, session type, player car index
- **Participants:** Driver name, team ID
- **Adapter:** `crates/rc-agent/src/sims/f1_25.rs`
- **Implementation:** Passive listener, no handshake required

#### iRacing
- **Protocol:** Named pipes / shared memory (Windows)
- **Data:** Telemetry API (iRSDKSharMem.h)
- **Monitored:** Session state, telemetry frames, lap completion
- **Adapter:** Planned (TBD, sysinfo process detection)

#### Le Mans Ultimate (LMU)
- **Protocol:** UDP telemetry (port 5300)
- **Data:** Vehicle state, position, session info
- **Adapter:** Planned

#### Forza Motorsport / Forza Horizon
- **Protocol:** UDP (port 5555)
- **Format:** Binary telemetry packet
- **Adapter:** Planned

#### Assetto Corsa Evo (ACEvo)
- **Protocol:** UDP / shared memory
- **Adapter:** Planned (compatibility layer over AC)

### Telemetry Frame Structure
```rust
pub struct TelemetryFrame {
    pub pod_id: String,
    pub sim_type: SimType,
    pub driver_name: String,
    pub track: String,
    pub car: String,
    pub lap_number: u32,
    pub lap_time_ms: u32,
    pub sector1_ms: Option<u32>,
    pub sector2_ms: Option<u32>,
    pub sector3_ms: Option<u32>,
    pub speed_kmh: f32,
    pub throttle: f32,
    pub brake: f32,
    pub steering: f32,
    pub rpm: u16,
    pub gear: i8,
    pub timestamp: DateTime<Utc>,
}
```

**Implementation:** `crates/rc-common/src/types.rs`, `crates/rc-agent/src/sims/mod.rs`

---

## UDP Heartbeat Protocol

### Purpose
Fast liveness detection between rc-core and rc-agent (alternative to WebSocket which can stall)

### Specs
- **Port:** 9999 (local network, agents → core)
- **Interval:** Agent sends ping every 2 seconds
- **Timeout:** 3 missed pings/pongs = dead (6 seconds total)
- **Packet size:** 12B (ping), 16B (pong)
- **Byte order:** Little-endian

### Ping Packet (Agent → Core)
```
Bytes  0-1:  Magic "RP" (0x52, 0x50)
Byte   2:    Pod number (1-8)
Byte   3:    Type 0x01 (ping)
Bytes  4-7:  Sequence (u32)
Bytes  8-11: Status bitfield (u32):
             bit 0:    ws_connected (1=WebSocket active)
             bit 1:    game_running (1=game.exe running)
             bit 2:    driving_active (1=inputs detected)
             bit 3:    billing_active (1=session in progress)
             bits 4-7: game_id (0=none, 1=AC, 2=F1, 3=iRacing, 4=LMU, 5=Forza, 6=ACEvo)
             bits 8-15: cpu_percent (0-100)
             bits 16-23: gpu_percent (0-100)
             bits 24-31: reserved
```

### Pong Packet (Core → Agent)
```
Bytes 0-1:   Magic "RP"
Byte  2:     Pod number
Byte  3:     Type 0x02 (pong)
Bytes 4-7:   Sequence (echo of ping)
Bytes 8-11:  Server timestamp (u32)
Bytes 12-15: Flags (u32):
             (reserved for future use)
```

**Implementation:** `crates/rc-common/src/udp_protocol.rs`

---

## Pod-Agent Communication

### HTTP API (Pod Internal)
- **Port:** 8090 per pod
- **Host:** pod IP (192.168.31.{88-91}, {28,33,38,86,87,89})
- **Protocol:** JSON REST

#### POST /exec
Execute a command on the pod
```json
{
  "cmd": "tasklist /NH | findstr rc-agent",
  "timeout_secs": 10
}
```

#### POST /write
Write file to pod
```json
{
  "path": "C:\\path\\to\\file.json",
  "content": "..."
}
```

**Implementation:** `crates/rc-core/src/action_queue.rs` (curl calls to pod-agent)

### Deployment Flow
1. Compile new `rc-agent.exe` on James's machine
2. Copy to `C:\Users\bono\racingpoint\deploy-staging\`
3. Start HTTP server: `python3 -m http.server 9998 --directory C:\Users\bono\racingpoint\deploy-staging`
4. Write deployment JSON: `deploy-cmd.json`
5. Execute via pod-agent: `curl -X POST http://pod_ip:8090/exec -d @deploy-cmd.json`
6. Verify: tasklist check or polling `/status`

**Notes:**
- `start` command timeout expected (rc-agent runs indefinitely)
- Always kill old binary before downloading new one
- Verify file size after download
- Never execute pod binaries on James's machine (crashes workstation)

---

## Camera System (13x Dahua 4MP)

### Cameras
| Location | IP | Type | Auth | RTSP |
|----------|----|----|------|------|
| **Entrance** | .8 | Dahua 4MP | admin / Admin@123 | rtsp://ip/stream?subtype=1 |
| **Reception** | .15 | Dahua 4MP | admin / Admin@123 | rtsp://ip/stream?subtype=1 |
| **Reception 2** | .154 | Dahua 4MP | admin / Admin@123 | rtsp://ip/stream?subtype=1 |
| **Pods** | .{pods} | Various | admin / Admin@123 | Per model |
| **NVR** | .18 | Dahua NVR | admin / Admin@123 | DVR storage |

### RTSP Subtype
- `subtype=1` — Main stream (4MP)
- `subtype=2` — Sub stream (lower resolution)
- Default user/pass: `admin` / `Admin@123`

### Integration
- **People Tracker:** Port 8095 (FastAPI + YOLOv8)
- **Cameras:** 3 cameras (entry, reception, pod view)
- **Output:** Entry/exit counting, occupancy detection
- **Future:** Face recognition, incident replay

**Implementation:** Separate service, not in racecontrol repo

---

## Assetto Corsa Server (LAN)

### Server Host
- **IP:** 192.168.31.51 (or drift from .23, Racing-Point-Server)
- **Preset:** RP_OPTIMAL (100% grip)
- **Config:** Content Manager (CM) with CSP gui.ini overrides

### CSP Force Settings
```ini
[FORCE_START]
FORCE_START=1
HIDE_MAIN_MENU=1
```

### Integration
- **Launch URL:** `acmanager://race/online/join?ip=<lan_ip>&httpPort=<port>`
- **HTTP Port:** Configurable (default 8081 per preset)
- **Authentication:** PIN code entry on pod lock screen
- **Session type:** Custom experiences from kiosk catalog
- **Preset ID:** Stored in `kiosk_experiences.ac_preset_id`

**Implementation:** `crates/rc-core/src/ac_server.rs`

---

## Billing & Wallet Integration

### Pricing Tiers
| ID | Name | Duration | Price (INR paise) | Trial |
|----|------|----------|------|-------|
| `tier_30min` | 30 Minutes | 30 min | 70,000 (₹700) | No |
| `tier_60min` | 1 Hour | 60 min | 90,000 (₹900) | No |
| `tier_trial` | Free Trial | 5 min | 0 | Yes |

### Dynamic Pricing Rules
```sql
CREATE TABLE pricing_rules (
  id TEXT PRIMARY KEY,
  rule_name TEXT,
  rule_type TEXT,  -- 'peak', 'off_peak', 'custom'
  day_of_week TEXT,  -- NULL = all days
  hour_start INTEGER,  -- NULL = all hours
  hour_end INTEGER,
  multiplier REAL,  -- e.g., 1.5 for 50% surge
  flat_adjustment_paise INTEGER,
  is_active BOOLEAN
);
```

### Wallet System
- **Currency:** Paise (1 rupee = 100 paise)
- **Balance tracking:** `balance_paise`, `total_credited_paise`, `total_debited_paise`
- **Cloud sync:** Venue debits authoritative, cloud credits override locally
- **Discount support:** Coupon IDs, discount reasons, custom pricing

**Implementation:** `crates/rc-core/src/billing.rs`, `crates/rc-core/src/cloud_sync.rs`

---

## Customer Authentication

### PIN-Based Lock Screen
1. Customer enters 4-digit PIN at reception kiosk
2. PIN validated against driver record in DB
3. QR code generated for mobile confirmation
4. PWA scans QR and confirms identity
5. Rig lock screen disengaged

### QR Code & PWA
- **QR generation:** `qrcode` crate v0.13 on rc-agent
- **PWA scan:** Mobile app at dynamic QR endpoint
- **Session token:** JWT with 24-hour validity
- **Pin expiry:** Configurable `auth.pin_expiry_secs` (default: as set in config)

### Waiver Signing
- **Digital signatures:** Stored in `drivers.signature_data`
- **Waiver tracking:** `waiver_signed` flag + `waiver_signed_at` timestamp
- **Version tracking:** `waiver_version` field for audit

**Implementation:** `crates/rc-agent/src/lock_screen.rs`, `crates/rc-core/src/auth/mod.rs`

---

## Email & SMS Integration

### Evolution API (WhatsApp)
- **Provider:** Evolution API (custom instances)
- **Auth:** API key + instance name
- **Endpoint:** Configurable `auth.evolution_url`
- **Purpose:** OTP delivery, booking confirmations, billing reminders

**Config:**
```toml
[auth]
evolution_url = "https://evolution.api.example.com"
evolution_api_key = "<key>"
evolution_instance = "<instance_name>"
```

### Gmail API (Backend)
- **Scope:** gmail.send, gmail.readonly
- **Auth:** OAuth 2.0 (refresh token in keyring)
- **Purpose:** Transactional emails (receipts, confirmations)
- **Implementation:** Direct googleapis.com calls from rc-core
- **Status:** Working (as of memory notes)

---

## Discord Webhooks

### Webhook Integration
- **Endpoint:** Configurable `integrations.discord.webhook_url`
- **Channel:** `integrations.discord.results_channel` (optional)
- **Events:** Session results, records broken, billing anomalies
- **Format:** Embeds with driver name, track, lap time, comparison to PB

**Config:**
```toml
[integrations.discord]
webhook_url = "https://discord.com/api/webhooks/..."
results_channel = "results"
```

---

## WhatsApp Integration

### Configuration
- **Enabled:** `integrations.whatsapp.enabled` (boolean)
- **Contact:** `integrations.whatsapp.contact` (phone number for notifications)
- **Provider:** Evolution API or Twilio (TBD)

---

## Database Sync Tables

### Cloud Authoritative
- `drivers` — Name, email, phone, avatar, trial status, waiver
- `pricing_tiers` — Base pricing for standard packages
- `pricing_rules` — Dynamic pricing rules (peak, off-peak)
- `kiosk_experiences` — Track/car combinations, presets, difficulty
- `kiosk_settings` — Global flags (demo mode, maintenance, etc.)

### Venue Authoritative
- `billing_sessions` — Local charging decisions, overrides
- `laps` — Telemetry-driven records, sector times
- `personal_bests` — Driver achievements at venue
- `track_records` — Venue records by car/track
- `wallets` — Balance after local debits (pushed to cloud)

### Bidirectional
- `drivers` — Cloud wins on profile, local preserves trial flag
- `wallets` — Cloud credited paise, venue debited paise (venue authoritative for debits)

---

## File Paths & Configs

| Component | Path | Purpose |
|-----------|------|---------|
| **rc-core config** | `racecontrol.toml` | Main server config (venue, server, database, cloud, cloud, AI) |
| **rc-agent config** | `rc-agent-pod{1-8}.toml` | Per-pod config (pod number, sim, core URL, game paths) |
| **Database** | `racecontrol.db` | SQLite (venue authoritative) |
| **Kiosk** | `kiosk/` | Next.js UI (port 3300) |
| **PWA** | `pwa/` | Mobile portal (React) |
| **Deploy kit** | `D:\pod-deploy\` | install.bat, rc-agent.exe, configs |
| **Staging** | `deploy-staging/` | HTTP server root for pod updates |
| **Training data** | `training/` | QLoRA pairs for Ollama fine-tuning (135+ samples) |

---

## Security Notes

- **No secrets in git:** API keys in .toml files (not versioned)
- **Terminal PIN:** Only Uday knows (admin access gate)
- **JWT secret:** Generated per-venue in `auth.jwt_secret`
- **Cloud secret:** Shared secret in `cloud.terminal_secret` (headers)
- **Windows registry:** Used for stored credentials (GitHub PAT, Gmail refresh token)
- **RTSP auth:** Hard-coded in Dahua init (admin / Admin@123) — no .env exposure

---

## Integration Test Points

| Integration | Test | Expected Result |
|-----------|------|-----------------|
| **Cloud sync** | `cargo test cloud_sync` | Pull/push completes without errors |
| **AI fallback** | Claude CLI → Ollama → Anthropic | Chain attempts until success |
| **Telemetry** | Lap completion detection | Sector times + lap_time match sim |
| **Heartbeat** | UDP ping/pong | 6s timeout triggers pod offline |
| **Pod-agent** | curl to /exec | Command executes, return code 0 |
| **Billing** | Session start/end | Duration matches allocated time |
| **Wallet sync** | Cloud credit override | Balance reflects remote + local debits |
| **Camera** | RTSP stream test | 4MP stream plays, no Auth errors |
| **AC Server** | Launch URL | CM opens session, pods join |

---

## Integration Deployment Checklist

- [ ] Cloud API responding (test sync_changes endpoint)
- [ ] Ollama running on James's machine (port 11434)
- [ ] All 8 pods reachable via mDNS + UDP heartbeat
- [ ] AC Server listening on LAN IP + HTTP port
- [ ] Dahua cameras accessible via RTSP (auth verified)
- [ ] Evolution API credentials valid (OTP test)
- [ ] Gmail OAuth refresh token in Windows keyring
- [ ] Discord webhook URL valid (test message)
- [ ] Database initialized with seed data
- [ ] Pod configs deployed to all pods
- [ ] rc-agent binaries current on all pods
- [ ] Watchdog tasks running on all pods (schtasks)

