# Phase 289: Metric Alert Thresholds - Research

**Researched:** 2026-04-01
**Domain:** Rust background task, TOML config extension, TSDB query, WhatsApp alerter integration
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** Alert rules in `racecontrol.toml` under `[[alert_rules]]` array-of-tables — each rule: `name`, `metric`, `condition` (gt/lt/eq), `threshold` (f64), `severity` (info/warn/critical), `message_template` (String with `{value}` and `{threshold}` placeholders)
- **D-02:** Rules parsed at startup via existing `Config` struct in `config.rs` — invalid rules logged as WARN and skipped, not fatal
- **D-03:** Background tokio task evaluates all rules every 60 seconds — queries TSDB `metrics_samples` for latest value per metric, compares against thresholds
- **D-04:** Evaluation uses the snapshot query pattern from Phase 286's `metrics_query.rs` to get latest values efficiently
- **D-05:** Triggered alerts sent via existing `whatsapp_alerter.rs` send mechanism (Bono VPS Evolution API) — reuse, don't create new channel
- **D-06:** Alert message format: `"[{severity}] {name}: {message}"` — same style as existing P0 alerts
- **D-07:** In-memory `HashMap<String, Instant>` tracks last fire time per rule name. Suppress if last fire < 30 minutes ago. Cleared on restart (acceptable).

### Claude's Discretion
- Whether to create a new `metric_alerts.rs` module or extend `alert_engine.rs`
- Exact TOML structure details beyond the fields specified
- Whether to add a per-pod dimension to alert rules (if metrics are per-pod)
- Test strategy for the evaluation loop

### Deferred Ideas (OUT OF SCOPE)
None — discussion stayed within phase scope.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| ALRT-01 | Alert rules defined in racecontrol.toml under [alert_rules] section | Config struct extension pattern verified — must add `alert_rules: Vec<MetricAlertRule>` with `#[serde(default)]` due to `deny_unknown_fields` on Config |
| ALRT-02 | Alert engine evaluates rules every 60 seconds against TSDB data | `tokio::spawn` loop + `tokio::time::sleep(Duration::from_secs(60))` — established pattern in main.rs |
| ALRT-03 | Triggered alerts fire to existing WhatsApp alerter (Bono VPS Evolution API) | `send_whatsapp()` function in `whatsapp_alerter.rs` is public and reusable directly |
| ALRT-04 | Alert deduplication — same alert suppressed for 30 minutes after first fire | `HashMap<String, Instant>` pattern — same as `P0State` in `whatsapp_alerter.rs` |
| ALRT-05 | Alert rules support threshold conditions (>, <, ==) on any metric name | Enum `AlertCondition { Gt, Lt, Eq }` with f64 comparison — straightforward |
</phase_requirements>

## Summary

Phase 289 adds a configurable metric alert system on top of the TSDB infrastructure from Phase 285/286. The work is entirely server-side Rust: extend `config.rs` with a `MetricAlertRule` struct and `Vec<MetricAlertRule>` field, create a new `metric_alerts.rs` module with the evaluation loop, and wire it into `main.rs`.

The key constraint is that `Config` has `#[serde(deny_unknown_fields)]` — adding `alert_rules` to `racecontrol.toml` without adding the field to the Rust struct will cause parse failure at startup. The field must be added with `#[serde(default)]` so existing configs without the section still parse cleanly.

All integration points are already proven: `send_whatsapp()` in `whatsapp_alerter.rs` is a `pub(crate)` async function that takes `&Config` and a message string — it can be called directly from the alert evaluation loop without going through any channel. The snapshot query pattern in `metrics_query.rs` (`query_snapshot()`) fetches latest values per metric across all pods and returns `Vec<SnapshotEntry>` — this is exactly what the evaluation loop needs.

**Primary recommendation:** Create a new `metric_alerts.rs` module (not extending `alert_engine.rs` which handles business KPIs). Spawn the task from `main.rs` conditionally when `config.alert_rules` is non-empty.

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| tokio | (workspace) | Async runtime, `time::sleep` for the eval loop | Already in project |
| sqlx | (workspace) | Dynamic query to metrics_samples | Already used in metrics_query.rs |
| serde + serde_json | (workspace) | TOML deserialization of alert_rules | Already in config.rs |
| std::collections::HashMap | std | Deduplication state (last fire times) | No external dep needed |
| std::time::Instant | std | Duration comparison for 30-min cooldown | Already used in P0State |

No new dependencies required.

## Architecture Patterns

### Recommended Project Structure
```
crates/racecontrol/src/
├── metric_alerts.rs      # NEW: MetricAlertRule, AlertCondition, eval loop
├── config.rs             # MODIFY: add alert_rules: Vec<MetricAlertRule>
├── main.rs               # MODIFY: spawn metric_alert_task
└── api/metrics_query.rs  # READ-ONLY: reuse query_snapshot()
```

### Pattern 1: Config Struct Extension

**What:** Add `MetricAlertRule` struct and `alert_rules` field to `Config`.
**Critical constraint:** `Config` has `#[serde(deny_unknown_fields)]` — the field MUST be in the struct or TOML parse fails.

```rust
// In config.rs

#[derive(Debug, Clone, Deserialize)]
pub struct MetricAlertRule {
    pub name: String,
    pub metric: String,
    pub condition: AlertCondition,
    pub threshold: f64,
    #[serde(default = "default_severity")]
    pub severity: String,
    pub message_template: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertCondition {
    Gt,
    Lt,
    Eq,
}

fn default_severity() -> String { "warn".to_string() }

// In Config struct:
#[serde(default)]
pub alert_rules: Vec<MetricAlertRule>,
```

**TOML example:**
```toml
[[alert_rules]]
name = "high_gpu_temp"
metric = "gpu_temperature"
condition = "gt"
threshold = 85.0
severity = "critical"
message_template = "GPU temp {value}°C exceeds {threshold}°C"
```

### Pattern 2: Background Evaluation Loop

**What:** tokio::spawn with 60-second sleep interval, iterates all rules, queries latest metric values, fires deduped alerts.
**When to use:** Standard pattern for all periodic background tasks in this codebase.

```rust
// In metric_alerts.rs

pub async fn metric_alert_task(state: Arc<AppState>) {
    let mut last_fired: HashMap<String, Instant> = HashMap::new();
    let cooldown = Duration::from_secs(30 * 60); // 30 minutes

    loop {
        tokio::time::sleep(Duration::from_secs(60)).await;

        // Get snapshot of ALL latest metric values (no pod filter = all pods)
        let snapshot = crate::api::metrics_query::query_snapshot(&state.db, None).await;

        // Build lookup: metric_name -> latest value
        // Note: if same metric appears for multiple pods, this takes the last occurrence.
        // For phase scope, per-pod alerting is Claude's discretion (D-07 is per rule-name only).
        let latest: HashMap<String, f64> = snapshot
            .into_iter()
            .map(|e| (e.name, e.value))
            .collect();

        for rule in &state.config.alert_rules {
            let Some(&value) = latest.get(&rule.metric) else {
                tracing::debug!(target: LOG_TARGET, "metric '{}' not in snapshot, skipping rule '{}'", rule.metric, rule.name);
                continue;
            };

            let triggered = match rule.condition {
                AlertCondition::Gt => value > rule.threshold,
                AlertCondition::Lt => value < rule.threshold,
                AlertCondition::Eq => (value - rule.threshold).abs() < f64::EPSILON,
            };

            if !triggered {
                continue;
            }

            // Deduplication: suppress if fired within last 30 minutes
            let now = Instant::now();
            if let Some(&last) = last_fired.get(&rule.name) {
                if now.duration_since(last) < cooldown {
                    continue;
                }
            }

            last_fired.insert(rule.name.clone(), now);

            let msg_body = rule.message_template
                .replace("{value}", &format!("{:.2}", value))
                .replace("{threshold}", &format!("{:.2}", rule.threshold));
            let message = format!("[{}] {}: {}", rule.severity.to_uppercase(), rule.name, msg_body);

            tracing::warn!(target: LOG_TARGET, "metric alert fired: {}", message);
            crate::whatsapp_alerter::send_whatsapp(&state.config, &message).await;
        }
    }
}
```

### Pattern 3: Spawning from main.rs

```rust
// In main.rs — spawn only when rules exist
if !state.config.alert_rules.is_empty() {
    let alert_state = state.clone();
    tokio::spawn(crate::metric_alerts::metric_alert_task(alert_state));
    tracing::info!(target: "startup", "metric alert task spawned ({} rules)", state.config.alert_rules.len());
}
```

### Anti-Patterns to Avoid
- **Holding a lock across `.await`:** Do NOT hold any RwLock across the `query_snapshot()` await call. The config is an `Arc<Config>` cloned at AppState creation — no lock needed.
- **Using `.unwrap()` anywhere:** Use `unwrap_or_default()` on sqlx query results (already the pattern in metrics_query.rs).
- **Persisting dedup state to DB:** The in-memory HashMap is the correct approach (D-07). DB persistence is out of scope and adds complexity.
- **Sending alerts synchronously inside lock:** `send_whatsapp()` is async and makes HTTP calls — always call it after releasing any locks and outside of tight critical sections.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| WhatsApp delivery | Custom HTTP client | `send_whatsapp()` in whatsapp_alerter.rs | Already handles Evolution API, error handling, config validation |
| Latest metric lookup | Custom SQL | `query_snapshot()` in metrics_query.rs | Self-join pattern already correct (handles NULL pod_id via COALESCE) |
| Config parsing | Custom TOML reader | serde + existing Config struct | `deny_unknown_fields` is in place — wrong struct = panic at startup |
| Cooldown tracking | DB table | `HashMap<String, Instant>` | In-memory is correct per D-07; DB adds latency and migration |

## Common Pitfalls

### Pitfall 1: `deny_unknown_fields` on Config
**What goes wrong:** Adding `[[alert_rules]]` to `racecontrol.toml` without declaring `alert_rules` in the `Config` struct causes TOML parse to fail at startup with a cryptic "unknown field" error. The server falls back to `load_or_default()` (empty config) and the alert task never starts.
**Why it happens:** `#[serde(deny_unknown_fields)]` on the top-level `Config` struct rejects any TOML key not mapped to a struct field.
**How to avoid:** Add `#[serde(default)] pub alert_rules: Vec<MetricAlertRule>` to `Config` in the SAME commit as the TOML changes. Verify with a parse test.
**Warning signs:** Server starts but `config.alert_rules` is empty even though `racecontrol.toml` has entries.

### Pitfall 2: send_whatsapp visibility
**What goes wrong:** `send_whatsapp()` is `pub(crate)` — accessible within the racecontrol crate but must be called via full path `crate::whatsapp_alerter::send_whatsapp()`.
**Why it happens:** It was only designed for intra-crate use.
**How to avoid:** Use the full `crate::whatsapp_alerter::send_whatsapp(&state.config, &message).await` call path. No visibility change needed.

### Pitfall 3: Snapshot query aggregates across pods
**What goes wrong:** `query_snapshot(db, None)` returns one entry per (metric_name, pod_id) pair. If `cpu_usage` exists for pod-1 through pod-8, the `HashMap` built from the iterator will only keep the LAST entry for each metric name (HashMap overwrites on duplicate key). An alert on `cpu_usage` will only fire based on the last pod's value in the result set ordering.
**Why it happens:** The snapshot query returns per-pod rows; the HashMap collapses them.
**How to avoid:** If per-pod alerting is desired (Claude's discretion item), use `query_snapshot(db, Some(pod_id))` in a loop or aggregate the snapshot into `HashMap<String, Vec<f64>>` and check if ANY pod exceeds the threshold. For simplest implementation, document the aggregation behavior in a comment.

### Pitfall 4: Cooldown HashMap grows unbounded
**What goes wrong:** `last_fired` HashMap accumulates one entry per rule name that ever fires. With many rules over a long uptime, this grows.
**Why it happens:** Nothing evicts old entries.
**How to avoid:** At venue scale (< 20 rules) this is not a practical concern. Add a comment to that effect. If rules > 50, implement periodic cleanup: `last_fired.retain(|_, t| t.elapsed() < Duration::from_secs(3600))`.

### Pitfall 5: Task silently dies
**What goes wrong:** If `query_snapshot` panics (it shouldn't — it uses `unwrap_or_default()`), the tokio task drops silently.
**How to avoid:** Per standing rule "Long-Lived Tasks Must Log Lifecycle" — log at task start, log first eval cycle, log on exit.

## Code Examples

### Full MetricAlertRule deserialize round-trip
```rust
// Source: config.rs pattern (AlertingConfig, MmaConfig)
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertCondition { Gt, Lt, Eq }

#[derive(Debug, Clone, Deserialize)]
pub struct MetricAlertRule {
    pub name: String,
    pub metric: String,
    pub condition: AlertCondition,
    pub threshold: f64,
    #[serde(default = "default_alert_severity")]
    pub severity: String,
    pub message_template: String,
}

fn default_alert_severity() -> String { "warn".to_string() }
```

### Snapshot query (existing — no changes needed)
```rust
// Source: crates/racecontrol/src/api/metrics_query.rs — query_snapshot()
// Returns Vec<SnapshotEntry> where each entry has: name, pod (Option<u32>), value (f64), updated_at (i64)
let snapshot = crate::api::metrics_query::query_snapshot(&state.db, None).await;
```

### Deduplication pattern (from P0State)
```rust
// Source: crates/racecontrol/src/whatsapp_alerter.rs — P0State fields
// Replicates: last_all_pods_alert: Option<Instant> + duration check
let now = Instant::now();
if last_fired.get(&rule.name).map(|t| now.duration_since(*t) < cooldown).unwrap_or(false) {
    continue; // suppressed
}
last_fired.insert(rule.name.clone(), now);
```

## Environment Availability

Step 2.6: SKIPPED — this phase is pure Rust code changes with no external dependencies beyond what is already in the project (sqlx, reqwest, tokio all present).

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (built-in) |
| Config file | none — inline test modules |
| Quick run command | `cargo test -p racecontrol metric_alert` |
| Full suite command | `cargo test -p racecontrol && cargo test -p rc-common` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| ALRT-01 | `MetricAlertRule` deserializes from TOML correctly | unit | `cargo test -p racecontrol test_alert_rule_toml_parse` | ❌ Wave 0 |
| ALRT-02 | Evaluation loop fires when threshold exceeded | unit (mock snapshot) | `cargo test -p racecontrol test_alert_eval_triggered` | ❌ Wave 0 |
| ALRT-03 | Alert fires to WhatsApp (integration — requires Evolution API config) | manual | n/a | manual-only |
| ALRT-04 | Dedup suppresses second alert within 30 minutes | unit | `cargo test -p racecontrol test_alert_dedup_suppression` | ❌ Wave 0 |
| ALRT-05 | All three conditions (gt/lt/eq) evaluate correctly | unit | `cargo test -p racecontrol test_alert_conditions` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p racecontrol metric_alert`
- **Per wave merge:** `cargo test -p racecontrol && cargo test -p rc-common`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/racecontrol/src/metric_alerts.rs` — covers ALRT-01 through ALRT-05 (inline `#[cfg(test)]` module)
- Tests for ALRT-02 and ALRT-04 should use `tokio::time::pause()` for deterministic time control (avoids real sleep)

## Sources

### Primary (HIGH confidence)
- `crates/racecontrol/src/config.rs` — Config struct, `deny_unknown_fields`, `AlertingConfig` pattern
- `crates/racecontrol/src/whatsapp_alerter.rs` — `send_whatsapp()` signature, `P0State` dedup pattern
- `crates/racecontrol/src/api/metrics_query.rs` — `query_snapshot()` return type, SQL patterns
- `crates/racecontrol/src/alert_engine.rs` — `BusinessAlert` struct, existing alert style
- `crates/racecontrol/src/main.rs` line 725-732 — whatsapp alerter spawn pattern

### Secondary (MEDIUM confidence)
- Phase 285/286 CONTEXT.md decisions — TSDB schema and snapshot query decisions
- CONTEXT.md D-01 through D-07 — locked implementation decisions

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all libraries already in project, no new dependencies
- Architecture: HIGH — all patterns directly verified from existing source files
- Pitfalls: HIGH — `deny_unknown_fields` and pod aggregation confirmed by reading source

**Research date:** 2026-04-01
**Valid until:** 2026-05-01 (stable codebase, low churn expected)
