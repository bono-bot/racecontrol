# Requirements: RaceControl Reliability & Connection Hardening

**Defined:** 2026-03-13
**Core Value:** Pods self-heal, deployments work reliably, customers never see system internals

## v1.0 Requirements

Requirements for milestone v1.0. Each maps to roadmap phases.

### Watchdog & Supervision

- [x] **WD-01**: Pod restart uses escalating backoff (30s→2m→10m→30m) instead of fixed cooldown
- [x] **WD-02**: pod_monitor and pod_healer share backoff state via AppState — no concurrent restarts
- [x] **WD-03**: Post-restart verification confirms process running + WebSocket connected + lock screen responsive (60s window)
- [x] **WD-04**: Backoff resets to base on confirmed full recovery

### Connection Resilience

- [x] **CONN-01**: WebSocket ping/pong keepalive prevents drops during game launch CPU spikes
- [x] **CONN-02**: Kiosk debounces disconnect events — only shows "Disconnected" after 15s+ confirmed absence
- [x] **CONN-03**: rc-agent reconnects automatically with short backoff on WebSocket drop

### Deployment & Config

- [x] **DEPLOY-01**: rc-agent validates all required config fields at startup, exits non-zero on invalid config
- [x] **DEPLOY-02**: Deploy sequence enforces kill→wait→verify-dead→download→size-check→start→verify-reconnect
- [x] **DEPLOY-03**: pod-agent /exec returns clear success/failure status (not HTTP 200 for everything)
- [x] **DEPLOY-04**: Deploy wipes old config files from pods before writing new config — no stale config remnants
- [x] **DEPLOY-05**: Future binary and config updates deploy without disrupting active sessions or requiring manual pod-by-pod fixups — rolling update with backward-compatible transitions

### Alerting

- [x] **ALERT-01**: Email alert fires when post-restart verification fails or max escalation reached
- [x] **ALERT-02**: Rate-limited: max 1 email per pod per 30min, 1 venue-wide per 5min

### Blanking Screen Protocol

- [x] **SCREEN-01**: Clean branded lock screen visible before session starts and after session ends — no Windows desktop exposed
- [x] **SCREEN-02**: All error popups suppressed on pod screens (WerFault, application errors, "Cannot find" dialogs, ConspitLink messages)
- [x] **SCREEN-03**: No file path errors or system dialogs leak through to the customer-facing display

### Performance & Latency

- [x] **PERF-01**: Game launch completes within target time from kiosk "Start" to game visible on pod
- [x] **PERF-02**: Lock screen responds to PIN entry within 1-2 seconds
- [x] **PERF-03**: WebSocket command round-trip (rc-core → rc-agent → response) stays under low threshold
- [x] **PERF-04**: Kiosk UI interactions (page loads, button responses, state updates) feel instant to staff

### Authentication

- [x] **AUTH-01**: PIN authentication works identically on pod lock screen, customer PWA, and customer kiosk — same validation, same flow, same response time

## Future Requirements

Deferred to v1.x or v2. Tracked but not in current roadmap.

### Alerting Enhancements

- **ALERT-03**: Aggregated multi-pod email alerts — single email when 2+ pods fail simultaneously
- **ALERT-04**: Partial recovery classification (Session 0 vs full failure) — smarter alert severity

### Deployment Enhancements

- **DEPLOY-06**: Deployment dry-run mode — validate config and binary compatibility without changing anything

### Observability

- **OBS-01**: Activity log persistence across rc-agent restarts — survives reboot for richer debugging context
- **OBS-02**: Configurable cooldown step tuning in racecontrol.toml without code change

## Out of Scope

| Feature | Reason |
|---------|--------|
| HUD overlay features | Deferred to next project (archived in .planning/archive/hud-safety/) |
| FFB safety | Deferred (archived research available) |
| New game integrations | Current games only — reliability first |
| Cloud sync changes | cloud_sync.rs is stable |
| Customer-facing PWA redesign | Only auth consistency is in scope |
| SNMP/Prometheus metrics | Over-engineered for 8-pod venue |
| Auto binary rollback | Config validation + Pod 8 first is sufficient |
| Sub-second process polling | UDP heartbeat at 6s is already fast enough |
| Restart on every WebSocket drop | Creates restart storms; keepalive is the fix |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| WD-01 | Phase 2 | Complete |
| WD-02 | Phase 1 | Complete |
| WD-03 | Phase 2 | Complete |
| WD-04 | Phase 2 | Complete |
| CONN-01 | Phase 3 | Complete |
| CONN-02 | Phase 3 | Complete |
| CONN-03 | Phase 3 | Complete |
| DEPLOY-01 | Phase 1 | Complete |
| DEPLOY-02 | Phase 4 | Complete |
| DEPLOY-03 | Phase 1 | Complete |
| DEPLOY-04 | Phase 1 | Complete |
| DEPLOY-05 | Phase 4 | Complete |
| ALERT-01 | Phase 2 | Complete |
| ALERT-02 | Phase 2 | Complete |
| SCREEN-01 | Phase 5 | Complete |
| SCREEN-02 | Phase 5 | Complete |
| SCREEN-03 | Phase 5 | Complete |
| PERF-01 | Phase 4 | Complete |
| PERF-02 | Phase 5 | Complete |
| PERF-03 | Phase 3 | Complete |
| PERF-04 | Phase 3 | Complete |
| AUTH-01 | Phase 5 | Complete |

**Coverage:**
- v1.0 requirements: 22 total
- Mapped to phases: 22
- Unmapped: 0

---
*Requirements defined: 2026-03-13*
*Last updated: 2026-03-13 — traceability populated after roadmap creation*
