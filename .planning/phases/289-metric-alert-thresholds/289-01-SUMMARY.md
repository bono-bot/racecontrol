---
phase: 289-metric-alert-thresholds
plan: "01"
subsystem: metric-alerts
tags: [alerts, whatsapp, metrics, config, background-task]
dependency_graph:
  requires: [285-metrics-tsdb, 286-metrics-query-api, 288-prometheus-export]
  provides: [metric-alert-task, alert-config-types]
  affects: [config.rs, lib.rs, whatsapp_alerter]
tech_stack:
  added: []
  patterns: [background-tokio-task, hashmap-dedup-cooldown, serde-default-vec]
key_files:
  created:
    - crates/racecontrol/src/metric_alerts.rs
  modified:
    - crates/racecontrol/src/config.rs
    - crates/racecontrol/src/lib.rs
decisions:
  - "check_condition() extracted as free function so tests can call it directly without spinning up AppState"
  - "alert_rules field placed after mma in Config struct to match struct declaration order"
  - "default_config() fixture updated with alert_rules: Vec::new() to satisfy exhaustive struct init"
metrics:
  duration: 15min
  completed: "2026-04-01"
  tasks: 1
  files: 3
---

# Phase 289 Plan 01: Metric Alert System Summary

**One-liner:** TOML-configurable metric alert rules with gt/lt/eq conditions, 30-min dedup, and WhatsApp firing via existing send_whatsapp.

## Tasks Completed

| # | Task | Commit | Files |
|---|------|--------|-------|
| 1 | Config structs + metric_alerts module with evaluation engine, dedup, WhatsApp | `4e31df4f` | config.rs, metric_alerts.rs, lib.rs |

## What Was Built

**config.rs additions:**
- `AlertCondition` enum with `Gt`, `Lt`, `Eq` variants (serde rename_all = snake_case for TOML)
- `MetricAlertRule` struct: name, metric, condition, threshold, severity (default "warn"), message_template
- `alert_rules: Vec<MetricAlertRule>` field on `Config` with `#[serde(default)]`
- `default_config()` fixture updated with `alert_rules: Vec::new()`

**metric_alerts.rs (new module):**
- `metric_alert_task(state: Arc<AppState>)` — 60s interval loop
- `query_snapshot(&state.db, None)` integration to get latest per-metric/pod values
- Any-pod threshold check: fires if ANY pod's value crosses condition
- 30-minute per-rule dedup using `HashMap<String, Instant>`
- Template substitution: `{value}` and `{threshold}` replaced with formatted f64
- Message format: `[SEVERITY] rule_name: body`
- `send_whatsapp(&state.config, &message)` — reuses existing WhatsApp alerter, no new HTTP client

**lib.rs:**
- `pub mod metric_alerts;` added alongside existing module declarations

## Test Results

8 tests, all passing:
- `metric_alert_gt_fires_above_threshold` — boundary checks (>, ==, <)
- `metric_alert_lt_fires_below_threshold` — boundary checks
- `metric_alert_eq_fires_on_exact_match` — equality checks
- `metric_alert_eq_uses_epsilon_comparison` — f64::EPSILON boundary
- `metric_alert_dedup_suppresses_within_cooldown` — second fire within 30min suppressed
- `metric_alert_dedup_fires_after_cooldown_expires` — fires again after cooldown
- `metric_alert_toml_with_rules_deserializes` — full TOML round-trip with 2 rules
- `metric_alert_toml_without_rules_deserializes` — missing [[alert_rules]] gives empty vec

## Verification

- `grep "pub alert_rules" config.rs` → 1 match
- `grep "pub async fn metric_alert_task" metric_alerts.rs` → 1 match
- `grep "mod metric_alerts" lib.rs` → 1 match
- `grep "send_whatsapp" metric_alerts.rs` → 1 match
- `grep "AlertCondition" config.rs` → 2 matches
- `cargo test -p racecontrol-crate --lib metric_alert` → 8 passed
- `cargo build --release -p racecontrol-crate` → Finished (no errors)
- `grep "deny_unknown_fields" config.rs` → still present (not removed)
- `grep 'serde(default)' config.rs | grep alert_rules` → found

## Deviations from Plan

**1. [Rule 1 - Bug] Missing alert_rules in default_config() test fixture**
- **Found during:** Task 1 compilation
- **Issue:** Config struct has exhaustive field init in `default_config()`; adding `alert_rules` without updating the fixture caused compile error E0063
- **Fix:** Added `alert_rules: Vec::new()` to `default_config()` in config.rs
- **Files modified:** crates/racecontrol/src/config.rs
- **Commit:** `4e31df4f`

**2. [Rule 3 - Blocking] Integration tests had pre-existing BillingTimer nonce errors**
- **Found during:** `cargo test` with integration tests included
- **Issue:** crates/racecontrol/tests/integration.rs has 4 BillingTimer struct inits missing `nonce` field — pre-existing, unrelated to this plan
- **Fix:** Used `--lib` flag to run only lib tests, bypassing integration test compile failures
- **Scope:** Out of scope (pre-existing). Deferred to deferred-items.md.

## Known Stubs

None. The metric_alert_task is complete and wired to real query_snapshot and send_whatsapp. Note: the task is not yet spawned from main.rs — that wiring is deferred to plan 289-02 (spawn + TOML example).

## Self-Check: PASSED

- `crates/racecontrol/src/metric_alerts.rs` — FOUND
- `crates/racecontrol/src/config.rs` — FOUND (modified)
- `crates/racecontrol/src/lib.rs` — FOUND (modified)
- Commit `4e31df4f` — FOUND
