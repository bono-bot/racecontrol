# Requirements: Racing Point Operations — v11.0 Agent & Sentry Hardening

**Defined:** 2026-03-20
**Core Value:** Customers see their lap times, compete on leaderboards, and compare telemetry

## v11.0 Requirements

Requirements for rc-sentry hardening, rc-agent decomposition, shared extraction, and test coverage.

### Sentry Hardening

- [ ] **SHARD-01**: rc-sentry enforces timeout_ms on command execution (kills child process after deadline)
- [ ] **SHARD-02**: rc-sentry truncates command output to 64KB (matching rc-agent remote_ops behavior)
- [ ] **SHARD-03**: rc-sentry limits concurrent exec requests to 4 (rejects with HTTP 429 when full)
- [ ] **SHARD-04**: rc-sentry fixes partial TCP read bug (loops until full HTTP body received)
- [ ] **SHARD-05**: rc-sentry uses structured logging via tracing (replaces eprintln)
- [ ] **SHARD-06**: rc-sentry handles graceful shutdown on SIGTERM/Ctrl+C (drains active connections)

### Sentry Expansion

- [ ] **SEXP-01**: rc-sentry exposes GET /health returning uptime, version, concurrent exec slots, hostname
- [ ] **SEXP-02**: rc-sentry exposes GET /version returning binary version and git commit hash
- [ ] **SEXP-03**: rc-sentry exposes GET /files?path=... returning directory listing or file contents
- [ ] **SEXP-04**: rc-sentry exposes GET /processes returning list of running processes with PID, name, memory

### Agent Decomposition

- [ ] **DECOMP-01**: rc-agent config types extracted from main.rs to config.rs (<500 lines)
- [ ] **DECOMP-02**: rc-agent AppState struct and shared state extracted to app_state.rs
- [ ] **DECOMP-03**: rc-agent WebSocket message handler extracted to ws_handler.rs
- [ ] **DECOMP-04**: rc-agent event loop select! body extracted to event_loop.rs using ConnectionState struct pattern

### Shared Extraction

- [ ] **SHARED-01**: rc-common exposes run_cmd_sync (thread + timeout) for rc-sentry and sync contexts
- [ ] **SHARED-02**: rc-common exposes run_cmd_async (tokio, feature-gated) for rc-agent
- [ ] **SHARED-03**: rc-sentry uses rc-common run_cmd_sync without pulling in tokio (verified via cargo tree)

### Testing

- [ ] **TEST-01**: billing_guard unit tests cover stuck session detection (BILL-02) and idle drift (BILL-03)
- [ ] **TEST-02**: failure_monitor unit tests cover game freeze (CRASH-01) and launch timeout (CRASH-02)
- [ ] **TEST-03**: ffb_controller tests via FfbBackend trait seam (no real HID access in tests)
- [ ] **TEST-04**: rc-sentry endpoint integration tests (/ping, /exec, /health, /version, /files, /processes)

## v12.0 Requirements

Deferred to future release. Tracked but not in current roadmap.

### Agent Decomposition (Advanced)

- **DECOMP-05**: Extract select! dispatch body into sub-handlers per message type (requires ConnectionState/ReconnectState split design)
- **DECOMP-06**: Extract lock_screen state machine into standalone module with unit tests

### Testing (Extended)

- **TEST-05**: Integration tests for rc-agent ↔ racecontrol WebSocket protocol
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
| SHARD-01 | — | Pending |
| SHARD-02 | — | Pending |
| SHARD-03 | — | Pending |
| SHARD-04 | — | Pending |
| SHARD-05 | — | Pending |
| SHARD-06 | — | Pending |
| SEXP-01 | — | Pending |
| SEXP-02 | — | Pending |
| SEXP-03 | — | Pending |
| SEXP-04 | — | Pending |
| DECOMP-01 | — | Pending |
| DECOMP-02 | — | Pending |
| DECOMP-03 | — | Pending |
| DECOMP-04 | — | Pending |
| SHARED-01 | — | Pending |
| SHARED-02 | — | Pending |
| SHARED-03 | — | Pending |
| TEST-01 | — | Pending |
| TEST-02 | — | Pending |
| TEST-03 | — | Pending |
| TEST-04 | — | Pending |

**Coverage:**
- v11.0 requirements: 21 total
- Mapped to phases: 0
- Unmapped: 21

---
*Requirements defined: 2026-03-20*
*Last updated: 2026-03-20 after initial definition*
