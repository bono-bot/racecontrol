# Roadmap: v36.0 Config Management & Policy Engine

## Overview

Centralize configuration so every pod runs from server-pushed config instead of local TOML files that drift. Phases build from schema foundation through push infrastructure, editor UI, preset library, and an automated policy rules engine.

## Milestones

- 🚧 **v34.0 Time-Series Metrics & Operational Dashboards** - Phases 285-291 (gap closure in progress)
- 📋 **v36.0 Config Management & Policy Engine** - Phases 295-299 (planned)

## Phases

### 🚧 v34.0 Time-Series Metrics & Operational Dashboards (Gap Closure)

### Phase 285: Metrics Ring Buffer
**Goal**: Server efficiently stores and retains time-series metric data from all pods
**Depends on**: Nothing (first phase)
**Requirements**: TSDB-01, TSDB-02, TSDB-03, TSDB-04, TSDB-05
**Success Criteria** (what must be TRUE):
  1. Metrics from all pods are stored in SQLite with timestamps and pod identifiers
  2. Raw samples are retained for 7 days, hourly rollups for 90 days
  3. Hourly and daily rollups (min/max/avg/count) exist and are retained for 90 days
  4. Metric ingestion does not introduce observable latency on the main server event loop (async/batched writes)
  5. Storage is bounded -- disk usage does not grow indefinitely regardless of uptime duration
**Plans:** 2/2 plans complete

Plans:
- [x] 285-01-PLAN.md -- SQLite TSDB schema (metrics_samples + metrics_rollups tables), types, record_sample, rollup functions
- [x] 285-02-PLAN.md -- Async mpsc ingestion pipeline, purge tasks (7d raw, 90d rollups), wired in main.rs

### Phase 286: Metrics Query API
**Goal**: Operators can retrieve historical and current metric data via REST API
**Depends on**: Phase 285
**Requirements**: QAPI-01, QAPI-02, QAPI-03, QAPI-04, QAPI-05
**Success Criteria** (what must be TRUE):
  1. GET /api/v1/metrics/query returns time-series data filtered by metric name and time range
  2. GET /api/v1/metrics/names returns the complete list of known metric names
  3. GET /api/v1/metrics/snapshot returns the latest value for every metric in one call
  4. Queries accept a pod filter parameter and return only that pod's data
  5. API auto-selects resolution (raw < 24h, hourly < 7d, daily > 7d) without caller needing to specify
**Plans:** 1/1 plans complete

Plans:
- [x] 286-01-PLAN.md -- Three REST endpoints (query, names, snapshot) with auto-resolution and pod filtering

### Phase 287: Metrics Dashboard
**Goal**: Staff can visually monitor venue health trends through a browser dashboard
**Depends on**: Phase 286
**Requirements**: DASH-01, DASH-02, DASH-03, DASH-04, DASH-05
**Success Criteria** (what must be TRUE):
  1. Admin app has a /metrics page displaying sparkline charts for selected metrics
  2. Staff can filter charts by individual pod using a pod selector
  3. Staff can change time range (1h, 6h, 24h, 7d, 30d, custom) and charts update accordingly
  4. Dashboard auto-refreshes every 30 seconds without manual reload
  5. Current snapshot values appear as headline numbers above the charts
**Plans:** 1/1 plans complete
**UI hint**: yes

Plans:
- [x] 287-01-PLAN.md -- Stub API client + metrics page with sparkline charts, pod selector, time range picker, headline numbers, 30s auto-refresh

### Phase 288: Prometheus Export
**Goal**: Metrics are available in Prometheus exposition format for future monitoring tool compatibility
**Depends on**: Phase 286
**Requirements**: PROM-01, PROM-02
**Success Criteria** (what must be TRUE):
  1. GET /api/v1/metrics/prometheus returns all current metrics in valid Prometheus exposition format
  2. Endpoint works without any Prometheus server deployed -- zero additional infrastructure required
**Plans:** 1/1 plans complete

Plans:
- [x] 288-01-PLAN.md -- Prometheus exposition format handler + public route registration (completed 2026-04-01)

### Phase 289: Metric Alert Thresholds
**Goal**: Operators receive WhatsApp alerts when metrics cross configured thresholds
**Depends on**: Phase 285
**Requirements**: ALRT-01, ALRT-02, ALRT-03, ALRT-04, ALRT-05
**Success Criteria** (what must be TRUE):
  1. Alert rules are defined in racecontrol.toml under [alert_rules] and parsed at startup
  2. Alert engine evaluates rules every 60 seconds against live TSDB data
  3. Triggered alerts fire to WhatsApp via the existing Bono VPS Evolution API alerter
  4. Same alert is suppressed for 30 minutes after first fire (deduplication)
  5. Rules support threshold conditions (>, <, ==) on any metric name
**Plans:** 2/2 plans complete

Plans:
- [x] 289-01-PLAN.md -- Config structs, evaluation engine with dedup, WhatsApp firing, unit tests
- [x] 289-02-PLAN.md -- Wire metric_alert_task into main.rs startup

### Phase 290: Wire Metric Producers
**Goal**: Real metric data flows into the TSDB so all downstream phases (query, dashboard, alerts, Prometheus) return live venue data instead of empty results
**Depends on**: Phase 285
**Requirements**: TSDB-03, TSDB-05
**Gap Closure**: Closes P1 gap from v34.0 audit — MetricsSender channel has no producers
**Success Criteria** (what must be TRUE):
  1. MetricsSender channel is cloned and used by at least one producer loop in main.rs
  2. metrics_samples table contains rows within 2 minutes of server startup
  3. GET /api/v1/metrics/snapshot returns at least one metric with a non-zero value
  4. GET /api/v1/metrics/names returns at least 3 metric names
**Plans:** 1/1 plans complete

Plans:
- [x] 290-01-PLAN.md -- Metric producer loops (ws_connections, game_sessions, pod_health, billing_revenue) + main.rs wiring

### Phase 291: Dashboard API Wiring
**Goal**: Metrics dashboard displays real TSDB data by calling the Phase 286 API instead of stub functions
**Depends on**: Phase 286, Phase 290
**Requirements**: DASH-01
**Gap Closure**: Closes P1 gap from v34.0 audit — dashboard stubs not replaced, API contract mismatches
**Success Criteria** (what must be TRUE):
  1. No TODO markers remain in web/src/lib/api/tsdb.ts
  2. Dashboard page issues HTTP requests to /api/v1/metrics/names, /query, /snapshot (visible in network tab)
  3. TypeScript interfaces match Rust response structs (name not metric_name, pod not pod_id, updated_at as number)
  4. Dashboard displays real data when server has metrics_samples rows
**Plans:** 1/1 plans complete

Plans:
- [x] 291-01-PLAN.md — TSDB API client + metrics dashboard page + sidebar nav link

### 📋 v36.0 Config Management & Policy Engine (Planned)

**Milestone Goal:** Every pod runs from server-pushed config. No local TOML drift. Staff can edit, push, and automate config changes via admin UI and policy rules.

- [x] **Phase 295: Config Schema & Validation** - Typed AgentConfig struct shared across rc-agent and racecontrol via rc-common, with serde validation and schema versioning (completed 2026-04-01)
- [ ] **Phase 296: Server-Pushed Config** - SQLite pod_configs table, WS push on connect, hot/cold reload semantics, local fallback, and hash-based deduplication
- [ ] **Phase 297: Config Editor UI** - Admin /config page with per-pod form editor, diff view, single-pod and bulk push, and audit trail
- [ ] **Phase 298: Game Preset Library** - SQLite preset store, push via config channel, reliability scoring, and flagging of unreliable presets in UI
- [ ] **Phase 299: Policy Rules Engine** - IF/THEN rules stored in SQLite, evaluated periodically against live metrics, with staff CRUD UI and evaluation log

## Phase Details

### Phase 295: Config Schema & Validation
**Goal**: A typed, versioned AgentConfig struct is the single source of truth for all pod-level configuration
**Depends on**: Nothing (first phase of milestone)
**Requirements**: SCHEMA-01, SCHEMA-02, SCHEMA-03, SCHEMA-04
**Success Criteria** (what must be TRUE):
  1. rc-agent and racecontrol both import AgentConfig from rc-common with no duplication
  2. A config with an unknown field still loads with defaults and emits a warning log -- it does not crash the agent
  3. A config with a mismatched type on a known field falls back to the field default and logs a warning
  4. AgentConfig carries a schema_version field that old agents ignore when encountering a newer version
**Plans:** 1/1 plans complete

Plans:
- [x] 295-01-PLAN.md -- Move AgentConfig to rc-common with schema_version, lenient parsing with warnings

### Phase 296: Server-Pushed Config
**Goal**: The server is the authoritative source of pod config; pods receive and persist config over WebSocket
**Depends on**: Phase 295
**Requirements**: PUSH-01, PUSH-02, PUSH-03, PUSH-04, PUSH-05, PUSH-06
**Success Criteria** (what must be TRUE):
  1. On WebSocket connect, every pod receives its current config from the server within 5 seconds
  2. Changing a hot-reload field (threshold, flag, budget limit) on the server takes effect on the pod within one WS round-trip -- no agent restart needed
  3. Changing a cold field (port, path, binary location) is marked as pending-restart and applied on next agent startup
  4. If the server is unreachable at pod boot, the pod loads its last-received local config and operates normally
  5. If the pushed config hash matches the pod's current config hash, the pod skips processing and logs "config unchanged"
**Plans**: TBD

### Phase 297: Config Editor UI
**Goal**: Staff can view, edit, and push pod configuration from the admin app without touching files
**Depends on**: Phase 296
**Requirements**: EDITOR-01, EDITOR-02, EDITOR-03, EDITOR-04, EDITOR-05, EDITOR-06
**Success Criteria** (what must be TRUE):
  1. Admin app /config page lists all pods with their current config status (in-sync, pending-restart, unknown)
  2. Staff can open a form editor for any pod, change fields, and see a diff of old vs new values before pushing
  3. Staff can push the updated config to a single pod with one click and the pod's status updates within 10 seconds
  4. Staff can push the current config to all pods simultaneously with one bulk-push action
  5. Every config change is recorded in the audit log with staff identity, timestamp, and the changed fields
**Plans**: TBD
**UI hint**: yes

### Phase 298: Game Preset Library
**Goal**: Game presets are server-managed, pushed to pods, and flagged when their launch reliability is poor
**Depends on**: Phase 296
**Requirements**: PRESET-01, PRESET-02, PRESET-03, PRESET-04
**Success Criteria** (what must be TRUE):
  1. Staff can create and store named car/track/session presets in the admin app tied to a specific game
  2. On pod connect, all presets are delivered via the config channel alongside pod config
  3. Each preset displays a reliability score in the kiosk and admin UI based on historical launch success/failure data
  4. Presets with a reliability score below the configured threshold are visually flagged as unreliable before staff selects them
**Plans**: TBD

### Phase 299: Policy Rules Engine
**Goal**: Staff can define automated IF/THEN rules that respond to live metrics without manual intervention
**Depends on**: Phase 296
**Requirements**: POLICY-01, POLICY-02, POLICY-03, POLICY-04, POLICY-05
**Success Criteria** (what must be TRUE):
  1. Staff can create a rule in the admin UI with a metric condition (e.g., gpu_temp > 85) and an action (alert, config change, feature flag toggle, budget adjust)
  2. The rule engine evaluates all active rules periodically against live metrics and triggers matching actions automatically
  3. Staff can view a log of rule evaluations showing which rules fired, when, and what action was taken
  4. Staff can edit or delete any rule from the admin UI and the change takes effect on the next evaluation cycle
  5. Rules that have never fired are distinguishable from rules that fired recently in the evaluation log
**Plans**: TBD
**UI hint**: yes

## Progress

**Execution Order:**
295 -> 296 -> 297
296 -> 298
296 -> 299

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 285. Metrics Ring Buffer | 2/2 | Complete | 2026-04-01 |
| 286. Metrics Query API | 1/1 | Complete | 2026-04-01 |
| 287. Metrics Dashboard | 1/1 | Complete | 2026-04-01 |
| 288. Prometheus Export | 1/1 | Complete | 2026-04-01 |
| 289. Metric Alert Thresholds | 2/2 | Complete | 2026-04-01 |
| 290. Wire Metric Producers | 1/1 | Complete    | 2026-04-01 |
| 291. Dashboard API Wiring | 1/1 | Complete   | 2026-04-01 |
| 295. Config Schema & Validation | 1/1 | Complete    | 2026-04-01 |
| 296. Server-Pushed Config | 0/TBD | Not started | - |
| 297. Config Editor UI | 0/TBD | Not started | - |
| 298. Game Preset Library | 0/TBD | Not started | - |
| 299. Policy Rules Engine | 0/TBD | Not started | - |
