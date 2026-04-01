# Roadmap: Racing Point eSports Operations

## Overview

Operational data durability and multi-venue readiness — ensuring all venue data survives hardware failure, syncs reliably to cloud, is structured for query and archival, and that the schema and deploy pipeline are ready for a second venue.

## Milestones

- ✅ **v34.0 Time-Series Metrics & Operational Dashboards** - Phases 285-291, SHIPPED 2026-04-01 ([archive](milestones/v34.0-ROADMAP.md))
- 🚧 **v36.0 Config Management & Policy Engine** - Phases 295-299 (in progress)
- 📋 **v37.0 Data Durability & Multi-Venue Readiness** - Phases 300-304 (planned)

## Phases

<details>
<summary>✅ v34.0 Time-Series Metrics & Operational Dashboards (SHIPPED 2026-04-01)</summary>

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

</details>

### 🚧 v36.0 Config Management & Policy Engine (In Progress)

**Milestone Goal:** Every pod runs from server-pushed config. No local TOML drift. Staff can edit, push, and automate config changes via admin UI and policy rules.

- [x] **Phase 295: Config Schema & Validation** - Typed AgentConfig struct shared across rc-agent and racecontrol via rc-common, with serde validation and schema versioning (completed 2026-04-01)
- [x] **Phase 296: Server-Pushed Config** - SQLite pod_configs table, WS push on connect, hot/cold reload semantics, local fallback, and hash-based deduplication (completed 2026-04-01)
- [x] **Phase 297: Config Editor UI** - Admin /config page with per-pod form editor, diff view, single-pod and bulk push, and audit trail (completed 2026-04-01)
- [x] **Phase 298: Game Preset Library** - SQLite preset store, push via config channel, reliability scoring, and flagging of unreliable presets in UI (completed 2026-04-01)
- [ ] **Phase 299: Policy Rules Engine** - IF/THEN rules stored in SQLite, evaluated periodically against live metrics, with staff CRUD UI and evaluation log

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
**Plans:** 2/2 plans complete

Plans:
- [x] 296-01-PLAN.md -- pod_configs SQLite table, FullConfigPush WS message, store/retrieve/push functions, REST API, wired into Register handler
- [x] 296-02-PLAN.md -- Agent FullConfigPush handler: hash dedup, hot/cold field categorization, local persistence, server-config boot fallback

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
**Plans:** 2/2 plans complete

Plans:
- [x] 297-01-PLAN.md -- Config API client and TypeScript types (configApi, AgentConfig, PodConfigResponse, AuditLogEntry, HOT_RELOAD_FIELDS)
- [x] 297-02-PLAN.md -- Config page (pod grid + status badges + bulk push + audit log), ConfigEditorModal (form + diff view + single push), nav link

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
**Plans**: 2 plans

Plans:
- [x] 298-01-PLAN.md -- GamePreset types, game_presets SQLite table, PresetsConfig, PresetPush WS message, push on connect, REST CRUD API with reliability scoring
- [x] 298-02-PLAN.md -- Admin /presets page with reliability badges, create/delete form, Sidebar nav link

**UI hint**: yes

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
**Plans**: 3 plans

Plans:
- [ ] 299-01-PLAN.md -- SQLite schema (policy_rules + policy_eval_log tables), PolicyRule/PolicyAction types, DB helpers, five REST endpoints (CRUD + eval-log)
- [ ] 299-02-PLAN.md -- policy_engine_task evaluation loop (60s cadence, 30-min cooldown, 4 action types: alert/config_change/flag_toggle/budget_adjust), wired into main.rs
- [ ] 299-03-PLAN.md -- Admin /policy page (rule list, create/edit/delete form, eval log with fired/not-fired badges), Sidebar nav link

**UI hint**: yes

---

### 📋 v37.0 Data Durability & Multi-Venue Readiness (Planned)

**Milestone Goal:** Operational data survives hardware failure. Cloud sync is extended to new tables. All events are structured and archived. Schema is ready for a second venue. Fleet deploys are automated with canary, health verify, auto-rollout, and auto-rollback.

- [x] **Phase 300: SQLite Backup Pipeline** - Hourly WAL-safe backup, 7-daily + 4-weekly rotation, nightly SCP to Bono VPS, staleness WhatsApp alert, admin visibility (completed 2026-04-01)
- [x] **Phase 301: Cloud Data Sync v2** - cloud_sync.rs extended with fleet_solutions, model_evaluations, metrics_rollups; cross-venue authority model; conflict handling; admin status (completed 2026-04-01)
- [ ] **Phase 302: Structured Event Archive** - SQLite events table with structured schema, daily JSONL export, 90-day SQLite retention, nightly SCP to VPS, REST query API
- [ ] **Phase 303: Multi-Venue Schema Prep** - venue_id column on all major tables, backward-compatible migration, INSERT/UPDATE coverage, design doc for venue 2 trigger
- [ ] **Phase 304: Fleet Deploy Automation** - POST /fleet/deploy endpoint with canary (Pod 8), health verify, auto-rollout, auto-rollback, billing drain, deploy status endpoint

## Phase Details

### Phase 300: SQLite Backup Pipeline
**Goal**: Operational databases are continuously backed up and staff can see backup health at a glance
**Depends on**: Nothing (first phase of v37.0)
**Requirements**: BACKUP-01, BACKUP-02, BACKUP-03, BACKUP-04, BACKUP-05
**Success Criteria** (what must be TRUE):
  1. Server runs an hourly backup of all SQLite databases using the WAL-safe .backup API -- backup files appear in the local backup directory within 60 minutes of server start
  2. Local backup directory contains at most 7 daily + 4 weekly snapshots; older files are automatically deleted without manual intervention
  3. After a nightly backup completes, the backup file appears on Bono VPS and a SHA256 checksum comparison confirms the file is intact
  4. If no backup has succeeded within 2 hours, a WhatsApp alert fires to the staff number -- the alert does not re-fire until the next 2-hour staleness window
  5. Admin dashboard backup panel shows last backup time, file size, and whether the Bono VPS destination is reachable
**Plans:** 2/2 plans complete

Plans:
- [x] 300-01-PLAN.md -- BackupConfig, backup_pipeline.rs (VACUUM INTO, rotation, staleness alert), wired into main.rs
- [x] 300-02-PLAN.md -- Nightly SCP to Bono VPS with SHA256, GET /api/v1/backup/status, admin Backup Status card

### Phase 301: Cloud Data Sync v2
**Goal**: Key intelligence tables are synced to Bono VPS and the system is ready for cross-venue data flows
**Depends on**: Phase 300
**Requirements**: SYNC-01, SYNC-02, SYNC-03, SYNC-04, SYNC-05, SYNC-06
**Success Criteria** (what must be TRUE):
  1. fleet_solutions, model_evaluations, and metrics_rollups rows written at the venue appear in the Bono VPS database within the next sync cycle (server-authoritative direction)
  2. A row written with a future venue_id on Bono VPS flows back to the venue database on the next sync (cloud-authoritative direction established)
  3. When two writes target the same row, the row with the later updated_at timestamp wins; if timestamps are equal, the row with the lexicographically smaller venue_id wins
  4. Admin dashboard sync panel shows last sync timestamp, number of tables synced, and running conflict count
**Plans:** 2/2 plans complete

Plans:
- [x] 301-01-PLAN.md -- DB migrations + cloud_sync.rs push/receive/pull for fleet_solutions, model_evaluations, metrics_rollups with LWW conflict resolution
- [x] 301-02-PLAN.md -- Admin settings Sync Status panel (syncHealth API client + SyncStatusPanel component)

### Phase 302: Structured Event Archive
**Goal**: Every significant system event is captured, queryable, and permanently archived off-server
**Depends on**: Phase 300
**Requirements**: EVENT-01, EVENT-02, EVENT-03, EVENT-04, EVENT-05
**Success Criteria** (what must be TRUE):
  1. After any significant system action (session start/end, deploy, alert fire, pod recovery), a row appears in the events table with type, source, pod, timestamp, and JSON payload populated
  2. A JSONL file for the previous day's events exists in the archive directory by 01:00 IST each morning
  3. Events in SQLite older than 90 days are purged by the daily maintenance task; the corresponding JSONL files remain untouched
  4. The nightly JSONL file for the previous day appears on Bono VPS after the archive task runs
  5. GET /api/v1/events returns a filtered list of events when given type, pod, or date range query parameters
**Plans**: TBD

### Phase 303: Multi-Venue Schema Prep
**Goal**: The database schema supports a second venue without data model changes -- only a config value changes
**Depends on**: Phase 301, Phase 302
**Requirements**: VENUE-01, VENUE-02, VENUE-03, VENUE-04
**Success Criteria** (what must be TRUE):
  1. Every major table has a venue_id column; existing rows all have venue_id = 'racingpoint-hyd-001' and the application behaves identically to before the migration
  2. The migration runs on an existing production database without data loss -- no manual intervention required, no functional behavior change for current single-venue operation
  3. All INSERT and UPDATE queries in racecontrol pass venue_id explicitly -- no row is written without a venue_id value
  4. MULTI-VENUE-ARCHITECTURE.md exists and documents the trigger conditions, schema strategy, sync model, and breaking points for a second venue
**Plans**: TBD

### Phase 304: Fleet Deploy Automation
**Goal**: Staff can deploy a new binary to the entire fleet in one API call with automatic safety gates
**Depends on**: Phase 303
**Requirements**: DEPLOY-01, DEPLOY-02, DEPLOY-03, DEPLOY-04, DEPLOY-05, DEPLOY-06
**Success Criteria** (what must be TRUE):
  1. POST /api/v1/fleet/deploy with a binary hash and scope (all/canary/specific pods) initiates a deployment and returns a deploy_id immediately
  2. The deploy goes to Pod 8 first; the next wave does not start until Pod 8 passes its health check
  3. After canary passes, remaining pods receive the binary in waves with a configurable inter-wave delay; the full fleet is updated without additional manual action
  4. If Pod 8 or any subsequent wave pod fails its post-deploy health check, all affected pods are automatically reverted to the previous binary
  5. GET /api/v1/fleet/deploy/status shows current wave, each pod's status (pending/deploying/healthy/rolled-back), and a log of rollback events
  6. No pod swaps its binary while it has an active billing session; the swap is deferred until the session ends naturally
**Plans**: TBD

## Progress

**Execution Order:**
295 -> 296 -> 297
296 -> 298
296 -> 299
300 -> 301
300 -> 302
301 + 302 -> 303
303 -> 304

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 285. Metrics Ring Buffer | 2/2 | Complete | 2026-04-01 |
| 286. Metrics Query API | 1/1 | Complete | 2026-04-01 |
| 287. Metrics Dashboard | 1/1 | Complete | 2026-04-01 |
| 288. Prometheus Export | 1/1 | Complete | 2026-04-01 |
| 289. Metric Alert Thresholds | 2/2 | Complete | 2026-04-01 |
| 290. Wire Metric Producers | 1/1 | Complete | 2026-04-01 |
| 291. Dashboard API Wiring | 1/1 | Complete | 2026-04-01 |
| 295. Config Schema & Validation | 1/1 | Complete | 2026-04-01 |
| 296. Server-Pushed Config | 2/2 | Complete    | 2026-04-01 |
| 297. Config Editor UI | 2/2 | Complete    | 2026-04-01 |
| 298. Game Preset Library | 2/2 | Complete    | 2026-04-01 |
| 299. Policy Rules Engine | 0/3 | Not started | - |
| 300. SQLite Backup Pipeline | 2/2 | Complete    | 2026-04-01 |
| 301. Cloud Data Sync v2 | 2/2 | Complete    | 2026-04-01 |
| 302. Structured Event Archive | 0/TBD | Not started | - |
| 303. Multi-Venue Schema Prep | 0/TBD | Not started | - |
| 304. Fleet Deploy Automation | 0/TBD | Not started | - |
