# Requirements: Racing Point Operations -- v11.0 Agent & Sentry Hardening

**Defined:** 2026-03-20
**Core Value:** Customers see their lap times, compete on leaderboards, and compare telemetry

## v10.0 Requirements

Requirements for connectivity, config sync, and failover redundancy.

### Config Sync

- [x] **SYNC-01**: racecontrol.toml changes on server .23 are detected via SHA-256 hash comparison and pushed to Bono via comms-link sync_push within 60s
- [x] **SYNC-02**: Config payload is sanitized before push -- allowlist-only (venue/pods/branding), no credentials (jwt_secret, terminal_secret, relay_secret), no local Windows paths
- [ ] **SYNC-03**: Cloud racecontrol receives config_snapshot in /sync/push and stores venue/pods/branding in AppState.venue_config (TOML-based config only -- billing rates and game catalog are DB-based and already synced via cloud_sync.rs SYNC_TABLES)

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

- [ ] **TEST-01**: billing_guard unit tests cover stuck session detection (BILL-02) and idle drift (BILL-03)
- [ ] **TEST-02**: failure_monitor unit tests cover game freeze (CRASH-01) and launch timeout (CRASH-02)
- [ ] **TEST-03**: ffb_controller tests via FfbBackend trait seam (no real HID access in tests)
- [x] **TEST-04**: rc-sentry endpoint integration tests (/ping, /exec, /health, /version, /files, /processes)

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
| New sim adapter implementations | Separate milestone concern (iRacing, LMU, Forza telemetry) |
| rc-sentry TLS/HTTPS | Internal LAN only; no external exposure |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| SYNC-01 | Phase 67 | Complete |
| SYNC-02 | Phase 67 | Complete |
| SYNC-03 | Phase 67 | Pending |
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
| TEST-01 | Phase 73 | Pending |
| TEST-02 | Phase 73 | Pending |
| TEST-03 | Phase 73 | Pending |
| TEST-04 | Phase 72 | Complete |

**Coverage:**
- v10.0 requirements: 3 total
- v10.0 mapped to phases: 3
- v11.0 requirements: 21 total
- v11.0 mapped to phases: 21
- Unmapped: 0

---
*Requirements defined: 2026-03-20*
*Last updated: 2026-03-20 -- added SYNC-01/02/03 for Phase 67 config sync*
