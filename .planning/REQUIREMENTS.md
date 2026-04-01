# Requirements: v36.0 Config Management & Policy Engine

**Defined:** 2026-04-01
**Core Value:** Centralize configuration so every pod runs from server-pushed config, not local TOML files that drift

## Config Schema & Validation (SCHEMA)

- [x] **SCHEMA-01**: Typed Rust `AgentConfig` struct defines all pod-level configuration fields with serde validation
- [x] **SCHEMA-02**: Invalid config fields fall back to defaults with a warning log (never crash)
- [x] **SCHEMA-03**: Config has a schema version field for forward compatibility (old agent ignores unknown fields)
- [x] **SCHEMA-04**: AgentConfig is shared between rc-agent and racecontrol via rc-common

## Server-Pushed Config (PUSH)

- [ ] **PUSH-01**: SQLite `pod_configs` table stores per-pod configuration with last-modified timestamp
- [ ] **PUSH-02**: Server pushes config to pod via WS on initial connection
- [ ] **PUSH-03**: Hot-reload fields (thresholds, flags, budget limits) apply immediately without agent restart
- [ ] **PUSH-04**: Cold fields (ports, paths, binary locations) require agent restart to take effect
- [ ] **PUSH-05**: Pod persists received config locally as fallback for server-down boot scenarios
- [ ] **PUSH-06**: Config push includes a hash so pod can skip processing if config unchanged

## Config Editor UI (EDITOR)

- [ ] **EDITOR-01**: Admin app has /config page listing all pods with their current config status
- [ ] **EDITOR-02**: Staff can edit config for a specific pod with a form-based editor
- [ ] **EDITOR-03**: Editor shows diff view before pushing changes (old vs new values)
- [ ] **EDITOR-04**: Staff can push config to a single pod with one click
- [ ] **EDITOR-05**: Staff can bulk-push config to all pods at once
- [ ] **EDITOR-06**: All config changes are logged in an audit trail (who, when, what changed)

## Game Preset Library (PRESET)

- [ ] **PRESET-01**: Server stores car/track/session presets in SQLite with name, game, and parameters
- [ ] **PRESET-02**: Presets are pushed to pods via the config channel on connect
- [ ] **PRESET-03**: Each preset has a historical reliability score based on launch success/failure data
- [ ] **PRESET-04**: Unreliable presets (score < threshold) are flagged in the kiosk/admin UI

## Policy Rules Engine (POLICY)

- [ ] **POLICY-01**: Policy rules are defined as IF metric_condition THEN action (e.g., IF gpu_temp > 85 THEN alert)
- [ ] **POLICY-02**: Rules are stored in SQLite and evaluated periodically against live metrics
- [ ] **POLICY-03**: Supported actions include: change config value, send alert, toggle feature flag, adjust budget
- [ ] **POLICY-04**: Staff can create, edit, and delete rules via the admin UI
- [ ] **POLICY-05**: Rule evaluation logs are visible in admin for debugging

## Out of Scope

| Feature | Reason |
|---------|--------|
| etcd / Consul | Venue-scale (8 pods) — SQLite + WS is sufficient |
| Multi-server config sync | Single server architecture — defer to v37+ multi-venue |
| Config encryption at rest | Internal network only, no sensitive data in pod config |
| Real-time config conflict resolution | Single admin editor, no concurrent edit scenario at venue scale |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| SCHEMA-01 | Phase 295 | Complete |
| SCHEMA-02 | Phase 295 | Complete |
| SCHEMA-03 | Phase 295 | Complete |
| SCHEMA-04 | Phase 295 | Complete |
| PUSH-01 | Phase 296 | Pending |
| PUSH-02 | Phase 296 | Pending |
| PUSH-03 | Phase 296 | Pending |
| PUSH-04 | Phase 296 | Pending |
| PUSH-05 | Phase 296 | Pending |
| PUSH-06 | Phase 296 | Pending |
| EDITOR-01 | Phase 297 | Pending |
| EDITOR-02 | Phase 297 | Pending |
| EDITOR-03 | Phase 297 | Pending |
| EDITOR-04 | Phase 297 | Pending |
| EDITOR-05 | Phase 297 | Pending |
| EDITOR-06 | Phase 297 | Pending |
| PRESET-01 | Phase 298 | Pending |
| PRESET-02 | Phase 298 | Pending |
| PRESET-03 | Phase 298 | Pending |
| PRESET-04 | Phase 298 | Pending |
| POLICY-01 | Phase 299 | Pending |
| POLICY-02 | Phase 299 | Pending |
| POLICY-03 | Phase 299 | Pending |
| POLICY-04 | Phase 299 | Pending |
| POLICY-05 | Phase 299 | Pending |

**Coverage:**
- v1 requirements: 25 total
- Mapped to phases: 25
- Unmapped: 0

---
*Requirements defined: 2026-04-01*
*Last updated: 2026-04-01 after initial definition*
