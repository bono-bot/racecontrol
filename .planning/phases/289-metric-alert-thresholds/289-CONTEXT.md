# Phase 289: Metric Alert Thresholds - Context

**Gathered:** 2026-04-01
**Status:** Ready for planning
**Mode:** Auto-generated (--auto mode, infrastructure phase)

<domain>
## Phase Boundary

TOML-configured alert rules evaluated every 60s against TSDB data from Phase 285. Triggered alerts fire to WhatsApp via existing Bono VPS Evolution API. Deduplication suppresses same alert for 30 minutes. Rules support threshold conditions (>, <, ==) on any metric name. Five requirements: ALRT-01 through ALRT-05.

</domain>

<decisions>
## Implementation Decisions

### Alert Rule Configuration
- **D-01:** Alert rules defined in `racecontrol.toml` under `[[alert_rules]]` array-of-tables — each rule has `name`, `metric`, `condition` (gt/lt/eq), `threshold` (f64), `severity` (info/warn/critical), `message_template` (String with `{value}` and `{threshold}` placeholders)
- **D-02:** Rules parsed at startup via existing `Config` struct in `config.rs` — invalid rules logged as WARN and skipped, not fatal

### Evaluation Engine
- **D-03:** Background tokio task evaluates all rules every 60 seconds — queries TSDB `metrics_samples` for latest value per metric, compares against thresholds
- **D-04:** Evaluation uses the snapshot query pattern from Phase 286's `metrics_query.rs` to get latest values efficiently

### WhatsApp Integration
- **D-05:** Triggered alerts sent via existing `whatsapp_alerter.rs` send mechanism (Bono VPS Evolution API) — reuse the existing WhatsApp channel, don't create a new one
- **D-06:** Alert message format: `"[{severity}] {name}: {message}"` — same style as existing P0 alerts

### Deduplication
- **D-07:** In-memory HashMap<String, Instant> tracks last fire time per rule name. Suppress if last fire < 30 minutes ago. Cleared on restart (acceptable — restart resets the 30-min window).

### Claude's Discretion
- Whether to create a new `metric_alerts.rs` module or extend `alert_engine.rs`
- Exact TOML structure details beyond the fields specified
- Whether to add a per-pod dimension to alert rules (if metrics are per-pod)
- Test strategy for the evaluation loop

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Existing Alert Infrastructure
- `crates/racecontrol/src/alert_engine.rs` — Existing business alert engine with hardcoded KPI thresholds
- `crates/racecontrol/src/whatsapp_alerter.rs` — P0 alert sender via Evolution API (Bono VPS)
- `crates/racecontrol/src/whatsapp_escalation.rs` — WhatsApp escalation patterns

### Phase 285 Foundation
- `.planning/phases/285-metrics-ring-buffer/285-CONTEXT.md` — TSDB schema decisions
- `crates/racecontrol/src/api/metrics_query.rs` — Snapshot query pattern (Phase 286) reusable for latest values

### Configuration
- `crates/racecontrol/src/config.rs` — Config struct, TOML parsing patterns

### Requirements
- `.planning/REQUIREMENTS.md` — ALRT-01 through ALRT-05 definitions

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `alert_engine.rs` — BusinessAlert struct, AlertChannel enum, check_business_alerts() pattern
- `whatsapp_alerter.rs` — P0State with dedup tracking, send_whatsapp_alert() via Evolution API
- `config.rs` — Config struct with serde Deserialize, TOML parsing
- `api/metrics_query.rs` — snapshot_handler inner query for latest metric values

### Established Patterns
- Background tasks via `tokio::spawn` with `loop { tokio::time::sleep(); ... }`
- Config loaded at startup, available via `Arc<AppState>` 
- WhatsApp alerts go through Bono VPS relay (not direct)
- Dedup via HashMap<key, Instant> with duration check

### Integration Points
- New `[[alert_rules]]` section in racecontrol.toml config
- Config struct in config.rs extended with `alert_rules: Vec<MetricAlertRule>`
- Background task spawned from main.rs alongside existing alert_engine
- Queries TSDB tables created by Phase 285

</code_context>

<specifics>
## Specific Ideas

No specific requirements beyond ALRT-01 through ALRT-05.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 289-metric-alert-thresholds*
*Context gathered: 2026-04-01*
