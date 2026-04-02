# Phase 288: Prometheus Export - Context

**Gathered:** 2026-04-01
**Status:** Ready for planning
**Mode:** Auto-generated (infrastructure phase — discuss skipped)

<domain>
## Phase Boundary

GET /api/v1/metrics/prometheus returns all current metrics in valid Prometheus exposition format. Zero additional infrastructure — no Prometheus server deployed. This is a passive compatibility endpoint for future monitoring tool integration.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase. Key patterns:
- Add handler to existing api/routes.rs (or a new metrics_query.rs if 286's module exists)
- Read latest values from metrics_tsdb (use snapshot query pattern from Phase 285's MetricsTsdb)
- Return text/plain with Prometheus exposition format (TYPE, HELP, metric lines)
- Metric names: snake_case with `racecontrol_` prefix (e.g., `racecontrol_cpu_usage`)
- Labels: `pod="pod-3"` for per-pod metrics
- No auth required on this endpoint (public, read-only metrics)

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/racecontrol/src/metrics_tsdb.rs` — MetricsTsdb with snapshot-like queries
- `crates/racecontrol/src/api/routes.rs` — Route registration pattern
- `crates/racecontrol/src/db/mod.rs` — SqlitePool access via AppState

### Integration Points
- New route added to api/routes.rs public_routes section
- Reads from metrics_samples table (latest value per metric+pod)

</code_context>

<specifics>
## Specific Ideas

Prometheus exposition format example:
```
# HELP racecontrol_cpu_usage CPU usage percentage
# TYPE racecontrol_cpu_usage gauge
racecontrol_cpu_usage{pod="pod-1"} 45.2
racecontrol_cpu_usage{pod="pod-3"} 67.8
```

</specifics>

<deferred>
## Deferred Ideas

None.

</deferred>
