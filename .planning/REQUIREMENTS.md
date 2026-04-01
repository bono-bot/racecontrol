# Requirements: v34.0 Time-Series Metrics & Operational Dashboards

**Defined:** 2026-04-01
**Core Value:** Make autonomous action loops observable and queryable with time-series depth -- answer "what happened last Tuesday at 8pm" without grepping JSONL logs

## Metrics Storage (TSDB)

- [x] **TSDB-01**: Server records metric samples at 1-minute resolution into SQLite metrics_tsdb
- [x] **TSDB-02**: Raw samples retained for 7 days, then purged automatically
- [x] **TSDB-03**: Hourly rollups (min/max/avg/count) computed and retained for 90 days
- [x] **TSDB-04**: Daily rollups computed and retained for 90 days
- [x] **TSDB-05**: Metrics captured include: CPU usage, GPU temperature, FPS, billing revenue, WS connection count, pod health score, game session count
- [x] **TSDB-06**: Metric ingestion does not block the main server event loop (async insert, batch if needed)
- [x] **TSDB-07**: Ring buffer behavior -- storage is bounded, old data purged by cron/background task

## Metrics Query API (QAPI)

- [ ] **QAPI-01**: GET /api/v1/metrics/query returns time-series data filtered by metric name and time range
- [ ] **QAPI-02**: GET /api/v1/metrics/names returns list of all known metric names
- [ ] **QAPI-03**: GET /api/v1/metrics/snapshot returns current (latest) value for all metrics
- [ ] **QAPI-04**: Query API supports per-pod filtering (e.g., ?pod=3)
- [ ] **QAPI-05**: Query API auto-selects resolution (raw for <24h, hourly for <7d, daily for >7d)

## Metrics Dashboard (DASH)

- [ ] **DASH-01**: Admin app (/metrics page) displays sparkline charts for selected metrics
- [ ] **DASH-02**: Dashboard has pod selector to filter metrics by pod
- [ ] **DASH-03**: Dashboard has time range picker (1h, 6h, 24h, 7d, 30d, custom)
- [ ] **DASH-04**: Dashboard auto-refreshes every 30 seconds
- [ ] **DASH-05**: Dashboard shows current snapshot values as headline numbers above charts

## Prometheus Export (PROM)

- [ ] **PROM-01**: GET /api/v1/metrics/prometheus returns metrics in Prometheus exposition format
- [ ] **PROM-02**: Endpoint is zero-cost -- no Prometheus server required, just the format for future compatibility

## Alert Thresholds (ALRT)

- [ ] **ALRT-01**: Alert rules are defined in racecontrol.toml under [alert_rules] section
- [ ] **ALRT-02**: Alert engine evaluates rules every 60 seconds against TSDB data
- [ ] **ALRT-03**: Triggered alerts fire to existing WhatsApp alerter (Bono VPS Evolution API)
- [ ] **ALRT-04**: Alert deduplication -- same alert suppressed for 30 minutes after first fire
- [ ] **ALRT-05**: Alert rules support threshold conditions (>, <, ==) on any metric name

## v2 Requirements

Deferred to future milestones. Tracked but not in current roadmap.

### Advanced Analytics

- **ANLYT-01**: Anomaly detection on metric trends (standard deviation based)
- **ANLYT-02**: Correlation analysis between metrics (e.g., GPU temp vs FPS drop)
- **ANLYT-03**: Capacity planning reports based on metric trends

### Dashboard Enhancements

- **DASH-v2-01**: Customizable dashboard layouts (drag-and-drop widget placement)
- **DASH-v2-02**: Metric annotations (mark events like deploys, incidents)
- **DASH-v2-03**: Dashboard sharing via URL with time range preserved

## Out of Scope

| Feature | Reason |
|---------|--------|
| Prometheus server deployment | Venue-scale doesn't need it -- SQLite TSDB + custom dashboard is sufficient |
| Grafana | Custom Next.js dashboard is more maintainable and branded |
| Real-time streaming (WebSocket metrics) | 30s polling is sufficient for dashboards; real-time is v32.0's domain |
| Pod-side metric collection agent | Server already receives all data via WS -- no agent needed |
| External TSDB (InfluxDB, TimescaleDB) | SQLite handles 8-pod venue scale; upgrade trigger is >100 metrics at >1Hz |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| TSDB-01 | Phase 285 | Complete |
| TSDB-02 | Phase 285 | Complete |
| TSDB-03 | Phase 285 | Complete |
| TSDB-04 | Phase 285 | Complete |
| TSDB-05 | Phase 285 | Complete |
| TSDB-06 | Phase 285 | Complete |
| TSDB-07 | Phase 285 | Complete |
| QAPI-01 | Phase 286 | Pending |
| QAPI-02 | Phase 286 | Pending |
| QAPI-03 | Phase 286 | Pending |
| QAPI-04 | Phase 286 | Pending |
| QAPI-05 | Phase 286 | Pending |
| DASH-01 | Phase 287 | Pending |
| DASH-02 | Phase 287 | Pending |
| DASH-03 | Phase 287 | Pending |
| DASH-04 | Phase 287 | Pending |
| DASH-05 | Phase 287 | Pending |
| PROM-01 | Phase 288 | Pending |
| PROM-02 | Phase 288 | Pending |
| ALRT-01 | Phase 289 | Pending |
| ALRT-02 | Phase 289 | Pending |
| ALRT-03 | Phase 289 | Pending |
| ALRT-04 | Phase 289 | Pending |
| ALRT-05 | Phase 289 | Pending |

**Coverage:**
- v1 requirements: 24 total
- Mapped to phases: 24
- Unmapped: 0

---
*Requirements defined: 2026-04-01*
*Last updated: 2026-04-01 after roadmap creation*
