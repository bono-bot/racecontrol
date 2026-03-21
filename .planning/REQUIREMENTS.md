# Requirements: Racing Point Operations -- v11.0 Agent & Sentry Hardening

**Defined:** 2026-03-20
**Core Value:** Customers see their lap times, compete on leaderboards, and compare telemetry

## v10.0 Requirements

Requirements for connectivity, config sync, and failover redundancy.

### Config Sync

- [x] **SYNC-01**: racecontrol.toml changes on server .23 are detected via SHA-256 hash comparison and pushed to Bono via comms-link sync_push within 60s
- [x] **SYNC-02**: Config payload is sanitized before push -- allowlist-only (venue/pods/branding), no credentials (jwt_secret, terminal_secret, relay_secret), no local Windows paths
- [x] **SYNC-03**: Cloud racecontrol receives config_snapshot in /sync/push and stores venue/pods/branding in AppState.venue_config (TOML-based config only -- billing rates and game catalog are DB-based and already synced via cloud_sync.rs SYNC_TABLES)

## v11.0 Requirements

Requirements for rc-sentry hardening, rc-agent decomposition, shared extraction, and test coverage.

### Sentry Hardening

- [x] **SHARD-01**: rc-sentry enforces timeout_ms on command execution (kills child process after deadline)
- [x] **SHARD-02**: rc-sentry truncates command output to 64KB (matching rc-agent remote_ops behavior)
- [x] **SHARD-03**: rc-sentry limits concurrent exec requests to 4 (rejects with HTTP 429 when full)
- [x] **SHARD-04**: rc-sentry fixes partial TCP read bug (loops until full HTTP body received)
- [x] **SHARD-05**: rc-sentry uses structured logging via tracing (replaces eprintln)
- [x] **SHARD-06**: rc-sentry handles graceful shutdown on SIGTERM/Ctrl+C (drains active connections)

### Sentry Expansion

- [x] **SEXP-01**: rc-sentry exposes GET /health returning uptime, version, concurrent exec slots, hostname
- [x] **SEXP-02**: rc-sentry exposes GET /version returning binary version and git commit hash
- [x] **SEXP-03**: rc-sentry exposes GET /files?path=... returning directory listing or file contents
- [x] **SEXP-04**: rc-sentry exposes GET /processes returning list of running processes with PID, name, memory

### Agent Decomposition

- [ ] **DECOMP-01**: rc-agent config types extracted from main.rs to config.rs (<500 lines)
- [ ] **DECOMP-02**: rc-agent AppState struct and shared state extracted to app_state.rs
- [ ] **DECOMP-03**: rc-agent WebSocket message handler extracted to ws_handler.rs
- [ ] **DECOMP-04**: rc-agent event loop select! body extracted to event_loop.rs using ConnectionState struct pattern

### Shared Extraction

- [x] **SHARED-01**: rc-common exposes run_cmd_sync (thread + timeout) for rc-sentry and sync contexts
- [x] **SHARED-02**: rc-common exposes run_cmd_async (tokio, feature-gated) for rc-agent
- [x] **SHARED-03**: rc-sentry uses rc-common run_cmd_sync without pulling in tokio (verified via cargo tree)

### Testing

- [x] **TEST-01**: billing_guard unit tests cover stuck session detection (BILL-02) and idle drift (BILL-03)
- [x] **TEST-02**: failure_monitor unit tests cover game freeze (CRASH-01) and launch timeout (CRASH-02)
- [x] **TEST-03**: ffb_controller tests via FfbBackend trait seam (no real HID access in tests)
- [x] **TEST-04**: rc-sentry endpoint integration tests (/ping, /exec, /health, /version, /files, /processes)

## v13.0 Requirements

Requirements for multi-game launch, billing, telemetry, and leaderboard integration.

### Launch

- [ ] **LAUNCH-01**: Staff can select F1 25, iRacing, AC EVO, EA WRC, or LMU from kiosk and launch on any pod with safe defaults
- [ ] **LAUNCH-02**: Customer can request a game launch from PWA/QR, staff confirms via kiosk
- [ ] **LAUNCH-03**: Game launch profiles define exe path, launch args, and safe defaults per game (TOML config)
- [ ] **LAUNCH-04**: Game process monitored -- detect crash/hang, auto-cleanup stale processes
- [ ] **LAUNCH-05**: Crash recovery auto-restarts game or alerts staff with option to relaunch
- [ ] **LAUNCH-06**: Which game is running on which pod visible in kiosk and fleet health dashboard

### Billing

- [ ] **BILL-01**: Billing starts when game is playable (PlayableSignal), not at process launch
- [ ] **BILL-02**: Per-game PlayableSignal: F1 25 (UDP session type), iRacing (IsOnTrack flag), AC EVO (non-zero physics), WRC (first stage packet), LMU (rF2 driving flag)
- [ ] **BILL-03**: Per-game billing rates configurable in billing_rates table
- [ ] **BILL-04**: Billing auto-stops on game exit, crash, or session end
- [ ] **BILL-05**: Session lifecycle: launch -> loading -> playable (billing starts) -> gameplay -> exit (billing stops) -> cleanup

### Telemetry -- F1 25

- [ ] **TEL-F1-01**: F1 25 UDP telemetry captured on port 20777
- [ ] **TEL-F1-02**: Lap times and sector splits extracted from F1 25 telemetry packets
- [ ] **TEL-F1-03**: Lap data emitted as AgentMessage::LapCompleted with sim_type = F1_25

### Telemetry -- iRacing

- [ ] **TEL-IR-01**: iRacing shared memory reader using winapi OpenFileMappingA
- [ ] **TEL-IR-02**: Handle session transitions -- re-open shared memory handle between races
- [ ] **TEL-IR-03**: Lap times and sector splits extracted from iRacing telemetry
- [ ] **TEL-IR-04**: Pre-flight check: verify irsdkEnableMem=1 in app.ini

### Telemetry -- LMU

- [ ] **TEL-LMU-01**: LMU shared memory reader using rFactor 2 shared memory plugin
- [ ] **TEL-LMU-02**: Lap times and sector splits extracted from rF2 scoring data
- [ ] **TEL-LMU-03**: Lap data emitted with sim_type = LMU

### Telemetry -- AC EVO

- [ ] **TEL-EVO-01**: AC EVO shared memory reader using ACC-format struct layout (best-effort, feature-flagged)
- [ ] **TEL-EVO-02**: Graceful degradation -- if telemetry fields are unpopulated, log warning and continue
- [ ] **TEL-EVO-03**: Lap data emitted when available, with sim_type = AC_EVO

### Telemetry -- EA WRC

- [ ] **TEL-WRC-01**: EA WRC UDP telemetry via JSON-configured packets (port 20432)
- [ ] **TEL-WRC-02**: Stage times captured and mapped to laps schema
- [ ] **TEL-WRC-03**: Rally adapter is best-effort -- launch works without telemetry

### Leaderboard

- [ ] **LB-01**: Lap/stage times from all games stored in existing laps table with sim_type field
- [ ] **LB-02**: Track name normalization mapping table
- [ ] **LB-03**: Existing leaderboard endpoints serve multi-game data with sim_type filtering

## v12.0 Requirements

Deferred to future release. Tracked but not in current roadmap.

### Agent Decomposition (Advanced)

- **DECOMP-05**: Extract select! dispatch body into sub-handlers per message type (requires ConnectionState/ReconnectState split design)
- **DECOMP-06**: Extract lock_screen state machine into standalone module with unit tests

### Testing (Extended)

- **TEST-05**: Integration tests for rc-agent <-> racecontrol WebSocket protocol
- **TEST-06**: End-to-end deploy verification tests using rc-sentry as health probe

## Out of Scope

| Feature | Reason |
|---------|--------|
| rc-sentry authentication | Network-scoped by design (192.168.31.x subnet); adding auth increases complexity without security benefit |
| rc-sentry async migration (tokio) | Deliberately stdlib-only for reliability as fallback; tokio would defeat the purpose |
| rc-agent main.rs complete rewrite | Incremental extraction is safer; full rewrite risks regressions in safety-critical paths |
| New sim adapter implementations | Now covered by v13.0 Multi-Game Launcher milestone |
| rc-sentry TLS/HTTPS | Internal LAN only; no external exposure |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| SYNC-01 | Phase 67 | Complete |
| SYNC-02 | Phase 67 | Complete |
| SYNC-03 | Phase 67 | Complete |
| SHARD-01 | Phase 71 | Complete |
| SHARD-02 | Phase 71 | Complete |
| SHARD-03 | Phase 71 | Complete |
| SHARD-04 | Phase 71 | Complete |
| SHARD-05 | Phase 71 | Complete |
| SHARD-06 | Phase 72 | Complete |
| SEXP-01 | Phase 72 | Complete |
| SEXP-02 | Phase 72 | Complete |
| SEXP-03 | Phase 72 | Complete |
| SEXP-04 | Phase 72 | Complete |
| DECOMP-01 | Phase 74 | Pending |
| DECOMP-02 | Phase 74 | Pending |
| DECOMP-03 | Phase 74 | Pending |
| DECOMP-04 | Phase 74 | Pending |
| SHARED-01 | Phase 71 | Complete |
| SHARED-02 | Phase 71 | Complete |
| SHARED-03 | Phase 71 | Complete |
| TEST-01 | Phase 73 | Complete |
| TEST-02 | Phase 73 | Complete |
| TEST-03 | Phase 73 | Complete |
| TEST-04 | Phase 72 | Complete |

| LAUNCH-01 | Phase 81 | Pending |
| LAUNCH-02 | Phase 81 | Pending |
| LAUNCH-03 | Phase 81 | Pending |
| LAUNCH-04 | Phase 81 | Pending |
| LAUNCH-05 | Phase 81 | Pending |
| LAUNCH-06 | Phase 81 | Pending |
| BILL-01 | Phase 82 | Pending |
| BILL-02 | Phase 82 | Pending |
| BILL-03 | Phase 82 | Pending |
| BILL-04 | Phase 82 | Pending |
| BILL-05 | Phase 82 | Pending |
| TEL-F1-01 | Phase 83 | Pending |
| TEL-F1-02 | Phase 83 | Pending |
| TEL-F1-03 | Phase 83 | Pending |
| TEL-IR-01 | Phase 84 | Pending |
| TEL-IR-02 | Phase 84 | Pending |
| TEL-IR-03 | Phase 84 | Pending |
| TEL-IR-04 | Phase 84 | Pending |
| TEL-LMU-01 | Phase 85 | Pending |
| TEL-LMU-02 | Phase 85 | Pending |
| TEL-LMU-03 | Phase 85 | Pending |
| TEL-EVO-01 | Phase 86 | Pending |
| TEL-EVO-02 | Phase 86 | Pending |
| TEL-EVO-03 | Phase 86 | Pending |
| TEL-WRC-01 | Phase 87 | Pending |
| TEL-WRC-02 | Phase 87 | Pending |
| TEL-WRC-03 | Phase 87 | Pending |
| LB-01 | Phase 88 | Pending |
| LB-02 | Phase 88 | Pending |
| LB-03 | Phase 88 | Pending |

**Coverage:**
- v10.0 requirements: 3 total
- v10.0 mapped to phases: 3
- v11.0 requirements: 21 total
- v11.0 mapped to phases: 21
- v13.0 requirements: 30 total
- v13.0 mapped to phases: 30
- Unmapped: 0

---
*Requirements defined: 2026-03-20*
*Last updated: 2026-03-21 -- added v13.0 Multi-Game Launcher (30 requirements, Phases 81-88)*
