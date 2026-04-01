# Roadmap: v34.0 Time-Series Metrics & Operational Dashboards

## Milestone Goal

Make autonomous action loops observable and queryable with time-series depth -- answer "what happened last Tuesday at 8pm" without grepping JSONL logs. SQLite TSDB with rollups, query API, Next.js dashboard, Prometheus export, and WhatsApp threshold alerts.

## Phases

**Phase Numbering:** Continues from v33.0 (ended at Phase 281). Start at 285 (per backlog allocation).

**Parallelism Map:**
- Phase 285 (Ring Buffer) runs FIRST (foundation)
- Phase 286 (Query API) depends on 285
- Phase 287 (Dashboard) depends on 286
- Phase 288 (Prometheus) depends on 286 (can parallel with 287)
- Phase 289 (Alerts) depends on 285 (can parallel with 286)

```
285 ──> 286 ──┬──> 287 (Dashboard)
              └──> 288 (Prometheus)
285 ──> 289 (Alerts)
```

- [ ] **Phase 285: Metrics Ring Buffer** - SQLite TSDB with 1-min samples, hourly/daily rollups, bounded storage, async ingestion
- [ ] **Phase 286: Metrics Query API** - REST endpoints for time-series queries, metric names, snapshots, per-pod filtering, auto-resolution
- [ ] **Phase 287: Metrics Dashboard** - Next.js /metrics page with sparkline charts, pod selector, time range picker, auto-refresh
- [ ] **Phase 288: Prometheus Export** - Prometheus exposition format endpoint for future compatibility
- [ ] **Phase 289: Metric Alert Thresholds** - TOML-configured alert rules evaluated against TSDB, firing to WhatsApp with dedup

## Phase Details

### Phase 285: Metrics Ring Buffer
**Goal**: Server persistently stores time-series metric data with automatic rollups and bounded storage
**Depends on**: Nothing (first phase)
**Requirements**: TSDB-01, TSDB-02, TSDB-03, TSDB-04, TSDB-05, TSDB-06, TSDB-07
**Success Criteria** (what must be TRUE):
  1. Server records CPU, GPU temp, FPS, billing revenue, WS connections, pod health score, and game session count at 1-minute resolution into SQLite
  2. Raw samples older than 7 days are automatically purged without manual intervention
  3. Hourly and daily rollups (min/max/avg/count) exist and are retained for 90 days
  4. Metric ingestion does not introduce observable latency on the main server event loop (async/batched writes)
  5. Storage is bounded -- disk usage does not grow indefinitely regardless of uptime duration
**Plans:** 1/2 plans executed

Plans:
- [x] 285-01-PLAN.md -- SQLite TSDB schema (metrics_samples + metrics_rollups tables), types, record_sample, rollup functions
- [ ] 285-02-PLAN.md -- Async mpsc ingestion pipeline, purge tasks (7d raw, 90d rollups), wired in main.rs

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
**Plans:** 1 plan

Plans:
- [ ] 286-01-PLAN.md — Three REST endpoints (query, names, snapshot) with auto-resolution and pod filtering

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
**Plans**: TBD
**UI hint**: yes

Plans:
- [ ] 287-01: Next.js /metrics page with charts, selectors, and auto-refresh

### Phase 288: Prometheus Export
**Goal**: Metrics are available in Prometheus exposition format for future monitoring tool compatibility
**Depends on**: Phase 286
**Requirements**: PROM-01, PROM-02
**Success Criteria** (what must be TRUE):
  1. GET /api/v1/metrics/prometheus returns all current metrics in valid Prometheus exposition format
  2. Endpoint works without any Prometheus server deployed -- zero additional infrastructure required
**Plans**: TBD

Plans:
- [ ] 288-01: Prometheus exposition format endpoint

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
**Plans**: TBD

Plans:
- [ ] 289-01: TOML alert rule config and evaluation engine
- [ ] 289-02: WhatsApp integration and deduplication

## Progress

**Execution Order:**
285 -> 286 + 289 (parallel) -> 287 + 288 (parallel)

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 285. Metrics Ring Buffer | 1/2 | In Progress|  |
| 286. Metrics Query API | 0/1 | Not started | - |
| 287. Metrics Dashboard | 0/1 | Not started | - |
| 288. Prometheus Export | 0/1 | Not started | - |
| 289. Metric Alert Thresholds | 0/2 | Not started | - |
