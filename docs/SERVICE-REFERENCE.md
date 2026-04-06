# Racing Point — Service Reference

Per-binary deep dive for debugging. Each section covers: purpose, modules, config, ports, log locations, common failures, and recovery.

---

## 1. rc-agent (Pod Agent)

**Binary:** `rc-agent.exe`
**Runs on:** Pods 1-8 (:8090), POS .20 (:8090)
**Session:** MUST be Session 1 (interactive desktop) — Session 0 cannot launch games/GUI
**Config:** `C:\RacingPoint\rc-agent.toml` (searches: exe dir -> CWD)
**Start:** `start-rcagent.bat` via HKLM Run key; RCWatchdog restarts on crash

### Ports

| Port | Server | Purpose | Auth |
|------|--------|---------|------|
| 8090 | remote_ops | Main control: /exec, /health, /screenshot, /files, /write | X-Service-Key |
| 18923 | lock_screen | Customer UI: PIN entry, QR, timer, session summary | None (LAN only) |
| 18924 | debug_server | Diagnostics: /status, /page, /screenshot | IP allowlist (.23, .27, localhost) |

### Key Modules (78 total)

**Core Infrastructure:**
- `app_state` — Shared state: WS sender, config, pod metadata, feature flags
- `config` — TOML loading with validation + game detection via Steam appmanifest
- `ws_handler` — WS client to server (heartbeat 60s, exponential backoff reconnect, failover URL)
- `event_loop` — Main async loop: billing, game lifecycle, session management
- `remote_ops` — HTTP server :8090 (SO_REUSEADDR, Connection: close to prevent CLOSE_WAIT)
- `firewall` — Auto-configures Windows Firewall (ICMP + TCP 8090) at startup

**Game Launching (6 sims):**
- `ac_launcher` — Assetto Corsa via Content Manager, ini generation
- `sims/assetto_corsa` — AC shared memory telemetry
- `sims/f1_25` — F1 25 UDP telemetry (:20777)
- `sims/iracing` — iRacing shared memory (variable offset lookup, 4096 var limit)
- `sims/assetto_corsa_evo` — AC Evo shared memory + UDP (:9996)
- `sims/lmu` — Le Mans Ultimate (rF2) shared memory (:5555)

**Self-Healing (5-tier):**
- `self_heal` — Tier 1: config/bat/registry repair, Defender exclusions
- `knowledge_base` — Tier 2: SQLite KB at `C:\RacingPoint\knowledge-base.db`
- `ai_debugger` — Tier 3: Ollama on James .27:11434 (qwen2.5-coder:14b)
- `openrouter` — Tier 4: OpenRouter API wrapper (mostly stub)
- `diagnostic_engine` — 5-tier decision tree, event stream, KB lookup

**Monitoring:**
- `self_monitor` — 5-min CLOSE_WAIT socket detection on :8090
- `self_test` — 13-point startup probe (ports, processes, files)
- `predictive_maintenance` — CLOSE_WAIT flood detection before port exhaustion
- `inactivity_monitor` — 600s (10min) idle threshold, one-shot alert
- `failure_monitor` — Pod-wide failure tracking

**Resilience:**
- `startup_cleanup` — Boot-time orphan/sentinel cleanup
- `sentinel_watcher` — ReadDirectoryChangesW on C:\RacingPoint
- `safe_mode` — Graceful degradation when server unreachable
- `process_guard` — Allowlist enforcement (5-min re-fetch cycle)
- `feature_flags` — Disk cache + WS sync + 5-min HTTP re-fetch
- `billing_guard` — Gate deploys during active billing

**Advanced:**
- `dxgi_capture` — DXGI Desktop Duplication for D3D game screenshots (FULLY IMPLEMENTED)
- `overlay` — On-screen HUD (steering force, RPM bar)
- `mesh_gossip` — Pod-to-pod learning (skeleton, incomplete fleet consensus)
- `content_scanner` — Game inventory scan (5-min rescan)
- `experience_collector` — Telemetry collection during sessions
- `night_ops` — Off-hours maintenance (morning CLOSE_WAIT check, reboot if >20)

### Config Structure (rc-agent.toml)

```toml
[pod]
number = 1           # 1-99
name = "Pod 01"
sim = "assetto_corsa"
sim_ip = "127.0.0.1"
sim_port = 9996

[core]
url = "ws://192.168.31.23:8080/ws/agent"
failover_url = "wss://app.racingpoint.cloud/ws/agent"  # optional

[games.assetto_corsa]
steam_app_id = 244210
use_steam = false

[games.f1_25]
steam_app_id = 3059520
use_steam = true

# Also: iracing, assetto_corsa_evo, assetto_corsa_rally, le_mans_ultimate

[ai_debugger]
enabled = true
ollama_url = "http://192.168.31.27:11434"
ollama_model = "qwen2.5-coder:14b"
```

### Log Files

| File | Content | Rotation |
|------|---------|----------|
| `C:\RacingPoint\rc-agent-{date}.jsonl` | Main structured logs (tracing) | 100MB or 24h, 30-day retention |
| `C:\RacingPoint\rc-bot-events.log` | Panic events (sync write in panic hook) | None |
| `C:\RacingPoint\crash-seh.log` | Windows SEH exceptions (0xC0000005 etc.) | None |
| `C:\RacingPoint\termination.log` | Process termination (Ctrl+C, taskkill, logoff) | None |
| `C:\RacingPoint\process-guard.log` | Process guard violations | 512KB rotation |
| `C:\RacingPoint\startup.log` | Boot phase tracking | Per-boot |
| `C:\RacingPoint\flags-cache.json` | Feature flag cache (survives restart) | Atomic write |
| `C:\RacingPoint\sentry-flags.json` | Flags for rc-sentry consumption | Atomic write |
| `C:\RacingPoint\knowledge-base.db` | Tier 2 KB solutions SQLite | Persistent |

### Sentinel Files

| File | Purpose | Auto-Clear |
|------|---------|------------|
| `MAINTENANCE_MODE` | Blocks restarts after 3 crashes in 10min | NO (manual only via SSH/rc-sentry) |
| `OTA_DEPLOYING` | Written during OTA; cleared on complete/rollback | Yes (>10min stale auto-clear) |
| `GAME_LAUNCHING` | Prevents concurrent launches | Cleared on external termination |
| `INTERRUPTED_SESSION_{id}.json` | Billing recovery after graceful shutdown | Consumed on next boot |

### Common Failures

| Symptom | Root Cause | Fix |
|---------|-----------|-----|
| Blank screen during session | rc-agent restarted, WS lost BillingStarted | Server re-sends on Register (273db1c) |
| Edge not launching | Session 0 (no GUI context) | Verify `tasklist /V` shows Console |
| Port 18923 in use | Stale rc-agent process | `taskkill /F /IM rc-agent.exe` (mutex since 305638b) |
| Game launches but wrong config | Serde silent field drop | Verify AcLaunchParams fields match kiosk buildLaunchArgs() |
| MAINTENANCE_MODE stuck | 3 crashes triggered sentinel | `del C:\RacingPoint\MAINTENANCE_MODE` via rc-sentry |
| Process guard blocking everything | Empty allowlist (server was down at boot) | Wait 5min for re-fetch, or restart rc-agent |
| CLOSE_WAIT accumulation | HTTP health polls without Connection: close | Fixed in remote_ops (SO_REUSEADDR + close header) |

### Debug Checklist

```bash
# 1. Is rc-agent running in Session 1?
tasklist /V /FO CSV | findstr rc-agent    # Session must show "Console"

# 2. Is lock screen working?
curl -s http://127.0.0.1:18924/status     # Check edge_process_count > 0

# 3. Active billing session?
curl -s http://localhost:8080/api/v1/billing/active

# 4. Lock screen page content?
curl -s http://127.0.0.1:18923/           # What page is Edge showing?

# 5. Sentinel files?
dir C:\RacingPoint\MAINTENANCE_MODE C:\RacingPoint\OTA_DEPLOYING 2>nul

# 6. WS connected to server?
curl -s http://127.0.0.1:8090/health      # Check ws_connected field
```

### Implementation Status

| Component | Status |
|-----------|--------|
| HTTP servers (8090, 18923, 18924) | FULLY IMPLEMENTED |
| WebSocket client (TLS, failover, heartbeat) | FULLY IMPLEMENTED |
| 6 game adapters (AC, F1, iRacing, ACE, ACR, LMU) | FULLY IMPLEMENTED |
| Lock screen UI + state machine | FULLY IMPLEMENTED |
| DXGI game screenshot capture | FULLY IMPLEMENTED |
| Feature flags (cache + WS + HTTP re-fetch) | FULLY IMPLEMENTED |
| Process guard (allowlist + 5min re-fetch) | FULLY IMPLEMENTED |
| Self-heal Tier 1 (deterministic) | FULLY IMPLEMENTED |
| Self-heal Tier 2 (KB lookup) | FULLY IMPLEMENTED |
| Self-heal Tier 3 (Ollama) | FULLY IMPLEMENTED |
| Self-heal Tier 4 (Cloud AI) | STUB — OpenRouter wrapper only |
| Self-heal Tier 5 (Escalation) | STUB — notification skeleton |
| Mesh gossip (cross-pod learning) | INCOMPLETE — skeleton, no consensus |
| Inactivity monitor (10min) | FULLY IMPLEMENTED |

---

## 2. rc-sentry (Pod Exec Daemon)

**Binary:** `rc-sentry.exe`
**Runs on:** Pods 1-8 (:8091), POS .20 (:8091)
**Session:** Session 0 (Windows service) — designed to survive rc-agent crashes
**Config:** `C:\RacingPoint\rc-sentry.toml`
**Start:** `start-rcsentry.bat` via HKLM Run

### Ports

| Port | Purpose | Auth |
|------|---------|------|
| 8091 | HTTP exec daemon | X-Service-Key (public: /ping, /health, /version) |

### Endpoints

| Method | Path | Auth | Purpose |
|--------|------|------|---------|
| GET | `/ping` | None | "pong" |
| GET | `/health` | None | Uptime, thread count, exec slots |
| GET | `/version` | None | Version + build_id |
| POST | `/exec` | Service key | Run cmd.exe command (JSON payload) |
| GET | `/flags` | Service key | Feature flags |
| GET | `/files/*` | Service key | Browse directory tree |
| GET | `/processes` | Service key | List running processes |

### Key Behavior

- **Health polling:** Checks rc-agent@:8090/health every 5s (hysteresis: 3 failures = crash)
- **Crash handling:** Extracts panic message + exit code from rc-agent logs
- **Session 1 restart:** Uses `WTSQueryUserToken + CreateProcessAsUser` (NOT schtasks)
- **Recovery logging:** `C:\RacingPoint\recovery-pod.jsonl` (JSONL audit trail)
- **Max concurrent execs:** 32 connections
- **Anti-cheat safe:** TCP/HTTP only — zero WinAPI process inspection

### Log Files

| File | Content |
|------|---------|
| `C:\RacingPoint\watchdog.log` | Rolling daily (service mode) |
| `C:\RacingPoint\recovery-pod.jsonl` | All restart decisions |

### Common Failures

| Symptom | Fix |
|---------|-----|
| rc-sentry can't restart rc-agent | Verify RCWatchdog service registered (`sc query RCWatchdog`) |
| /exec returns 401 | Service key mismatch — check server racecontrol.toml vs pod rc-sentry.toml |
| rc-agent stuck in Session 0 after restart | Use kill -> RCWatchdog auto-restart (not schtasks) |

### Implementation Status: FULLY IMPLEMENTED (0 TODOs, 0 production unwraps)

---

## 3. rc-watchdog (Fleet Healer)

**Binary:** `rc-watchdog.exe`
**Runs on:** James .27 (daemon mode) OR Pods (Windows service mode)
**Modes:**
- `--service` — Pod watchdog (Windows service, port 8091)
- No flag — James monitor (persistent daemon, 2-min cycle)

### Services Monitored (James Mode — 9 services)

| Service | Check | Healthy When |
|---------|-------|-------------|
| Ollama | HTTP /api/tags | Models loaded (not just 200) |
| comms-link | HTTP /relay/health | "connected" field present |
| rc-sentry-ai | HTTP /health | Detection stats returned |
| claude-code | Process check | "claude" process exists |
| racecontrol (.23) | HTTP /api/v1/health | status=ok |
| kiosk (.23) | HTTP /kiosk/api/health | healthy:true |
| dashboard (.23) | HTTP /api/health | healthy:true |
| go2rtc (.27) | HTTP /api/frame.jpeg | >1KB response (catches auth failures) |
| tailscale-bono | HTTP to cloud + SSH | build_id check |

### Healing Escalation

```
Failure 1: WARN log + read service logs for context
Failure 2: Attempt restart (if .bat script available) + verify spawn (3x poll)
Failure 3: Query Ollama qwen2.5:3b for AI diagnosis
Failure 4+: Alert Bono via comms-link WS + WhatsApp fallback
Recovery: Reset counter, record pattern
```

### OpenRouter MMA Integration

- **Trigger:** Restart loop detected (3+ restarts in 10min)
- **Model:** deepseek/deepseek-chat-v3-0324:free
- **Budget:** $0.05/day default
- **Sentinel:** `C:\RacingPoint\MMA_DIAGNOSING` (120s TTL)
- **Output:** `C:\RacingPoint\mma-diagnosis.json`

### MAINTENANCE_MODE Behavior

- **Trigger:** 3 restart depth levels exhausted
- **Auto-clear:** 30 minutes (SW-07 fix) OR manual via JSON timestamp check
- **Alert:** WhatsApp when triggered

### Log Files

| File | Content |
|------|---------|
| `C:\Users\bono\.claude\rc-watchdog.log` | James mode |
| `C:\RacingPoint\watchdog.log` | Pod service mode (rolling daily) |
| `C:\RacingPoint\mma-diagnosis.json` | MMA diagnosis results |

### Implementation Status: FULLY IMPLEMENTED (0 TODOs)

---

## 4. rc-guardian (External Monitor)

**Binary:** `rc-guardian.exe`
**Runs on:** Bono VPS (Linux)
**Purpose:** Layer 3 — monitors server from outside the venue

### Behavior

- **Health polling:** Every 60s to racecontrol health endpoint
- **Dead-man detection:** 3 consecutive failures = server dead
- **Billing safety:** Checks for active billing before any restart
- **Graduated restart:** Soft (schtasks) -> Hard (taskkill + start) -> Report-only (alert)
- **Restarts via:** Tailscale SSH to server (100.125.108.37)
- **Coordination:** GUARDIAN_ACTING mutex prevents concurrent actions
- **Heartbeat:** Status sent to comms-link WS every interval

### Status Labels

- **Healthy** — responding normally
- **Busy** — slow but responding
- **Dead** — connection refused
- **Unreachable** — timeout

### Implementation Status: FULLY IMPLEMENTED

---

## 5. rc-sentry-ai (Face Detection)

**Binary:** `rc-sentry-ai.exe`
**Runs on:** James .27
**Purpose:** Real-time face detection + recognition + attendance

### Architecture

- **Detection model:** SCRFD-10GF (ONNX, CUDA GPU acceleration) — NOT YOLO as previously documented
- **Recognition model:** GLintR100 (ArcFace embedding, ONNX)
- **Gallery:** SQLite database with pre-enrolled face embeddings
- **Matching:** Cosine similarity (threshold 0.7)
- **Cameras:** Entrance (cam2) detection+recognition, Entrance alt (cam9) attendance, Reception (cam15/154) NVR proxy
- **RTSP source:** go2rtc relay at localhost:1984

### API Endpoints

| Method | Path | Purpose |
|--------|------|---------|
| GET | `/health` | Detection stats (cameras, fps, detections/min) |
| POST | `/api/v1/enrollment/person` | Create person record |
| PUT | `/api/v1/enrollment/person/{id}` | Update face enrollment |
| GET | `/api/v1/attendance/report` | Attendance report |
| GET | `/api/v1/privacy/audit` | GDPR audit trail |
| WS | `/ws/alerts` | Live alert streaming |
| GET | `/api/v1/mjpeg/{camera_id}` | MJPEG live stream |
| GET | `/api/v1/playback/{timestamp}` | NVR playback proxy |

### Features

- Face detection (SCRFD inference on 3 cameras)
- Face recognition (gallery-based matching)
- Attendance (shift-based check-in/out, SQLite)
- Alerts (Windows toast + WhatsApp for unknown persons)
- Privacy (face crop saving 112x112, audit logging, retention purge)
- MJPEG streaming + NVR playback proxy

### Implementation Status: FULLY IMPLEMENTED

---

## 6. rc-process-guard (Standalone)

**Binary:** `rc-process-guard.exe`
**Runs on:** James .27 (standalone, not embedded in rc-agent)
**Purpose:** James workstation process enforcement

### Behavior

- Fetches whitelist from `/api/v1/guard/whitelist/james` (5-min refresh)
- 4 audit types every 5min: process scan, registry Run keys, scheduled tasks, port listeners
- Grace: 2 warnings before kill (unless critical)
- Safety valve: >80% violations = force report_only (corrupted whitelist protection)
- Violation types: Process, WrongMachineBinary, AutoStart, Port

### Log File

`C:\Users\bono\racingpoint\process-guard-james.log` (512KB rotation)

### Implementation Status: FULLY IMPLEMENTED

---

## 7. racecontrol (Server)

**Binary:** `racecontrol.exe`
**Runs on:** Server .23 (:8080), Bono VPS (:8080)
**Config:** `C:\RacingPoint\racecontrol.toml`
**DB:** SQLite WAL at `./data/racecontrol.db`

### Key Server Modules

| Module | Purpose | Status |
|--------|---------|--------|
| billing_fsm | 11-state FSM (Pending->Active->Completed) with CAS protection | FULLY IMPLEMENTED |
| billing_replay | Nonce replay protection for billing mutations | FULLY IMPLEMENTED |
| cloud_sync | Dual-mode: relay (2s) + HTTP fallback (30s) with circuit breaker | FULLY IMPLEMENTED |
| game_launcher | Launch flow with dynamic timeout (120s AC, 90s others, 180s cap) | FULLY IMPLEMENTED |
| pod_healer | Graduated recovery (6 steps) with cascade guard | FULLY IMPLEMENTED |
| fleet_health | Crash loop detection (>3 in 5min) + WhatsApp alert | FULLY IMPLEMENTED |
| config_push | Server-pushed config to pods via WS | FULLY IMPLEMENTED |
| flags | Feature flags (runtime toggles, FF-01+) | FULLY IMPLEMENTED |

### Log Configuration

- **Format:** JSONL structured logs (tracing subscriber)
- **IMPORTANT:** Log timestamps are UTC, operations are IST. Always convert: `UTC + 5:30 = IST`
- **Console vs file:** Different tracing filters! Process guard violations flood console but may not appear in JSONL

### Billing FSM States

```
Pending -> WaitingForGame -> Active
Active -> PausedGamePause | PausedDisconnect | PausedManual | PausedCrashRecovery
All paused -> End | EndEarly | Cancel
Terminal: Completed, EndedEarly, Cancelled, CancelledNoPlayable
```

### Cloud Sync Details

- **Relay mode (2s):** Through comms-link, push-only (other side pushes independently)
- **HTTP fallback (30s):** Direct to cloud when relay down
- **Hysteresis:** 3 failures -> down, 2 successes -> up
- **Backoff:** 5s -> 10s -> 20s -> ... -> 300s cap
- **15 tables synced:** drivers, wallets, pricing_tiers, pricing_rules, billing_rates, kiosk_experiences, kiosk_settings, auth_tokens, reservations, debit_intents, staff_members, driver_ratings, fleet_solutions, model_evaluations, metrics_rollups
