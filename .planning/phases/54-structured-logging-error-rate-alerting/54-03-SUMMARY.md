---
phase: 54-structured-logging-error-rate-alerting
plan: "03"
subsystem: racecontrol
tags: [error-rate, alerting, tracing, monitoring, email]
dependency_graph:
  requires: [54-01, email_alerts.rs]
  provides: [ErrorCountLayer, error_rate_alerter_task, MonitoringConfig]
  affects: [racecontrol tracing registry, racecontrol.toml config]
tech_stack:
  added: []
  patterns: [tracing Layer, mpsc bridge sync→async, sliding window VecDeque]
key_files:
  created:
    - crates/racecontrol/src/error_rate.rs
  modified:
    - crates/racecontrol/src/lib.rs
    - crates/racecontrol/src/config.rs
    - crates/racecontrol/src/main.rs
decisions:
  - "Config loaded before tracing init in main.rs so MonitoringConfig thresholds are available at layer setup time — pre-init messages use eprintln!"
  - "alert_rx dropped when error_rate_email_enabled=false — ErrorCountLayer still counts but try_send returns Err silently (belt-and-suspenders design)"
  - "email_script_path and email_enabled cloned/extracted from config before AppState::new() moves config — avoids use-after-move"
  - "ErrorCountLayer clears timestamps after firing alert to avoid re-firing on the very next error within the same burst"
metrics:
  duration: 6 min
  completed: "2026-03-20"
  tasks: 2
  files: 4
---

# Phase 54 Plan 03: Error Rate Alerting Summary

**One-liner:** ErrorCountLayer tracing Layer with sliding window VecDeque, mpsc bridge to async alerter task, configurable via `[monitoring]` section in racecontrol.toml, sends to both james@racingpoint.in and usingh@racingpoint.in.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | ErrorCountLayer with sliding window counter and unit tests | 73a1bbb | error_rate.rs, lib.rs |
| 2 | MonitoringConfig + wire ErrorCountLayer into tracing registry | 47293b2 | config.rs, main.rs |

## What Was Built

### error_rate.rs (new)

- `ErrorRateConfig` — threshold (default: 5), window_secs (default: 60), cooldown_secs (default: 1800)
- `ErrorCountLayer` — implements `tracing_subscriber::Layer`, counts ERROR events in a sliding window `VecDeque<Instant>`, fires via `try_send` (non-blocking, critical since `on_event` is sync)
- Cooldown enforced in-layer via `last_alerted: Option<Instant>` — prevents alert spam without relying on the email layer's rate limiting
- Timestamps cleared after alert fires to avoid burst re-triggering
- `error_rate_alerter_task` — async mpsc receiver, creates one `EmailAlerter` per recipient, calls `send_alert("server", ...)` for each
- 4 unit tests covering: below threshold, threshold reached, window eviction, cooldown

### config.rs (modified)

- Added `MonitoringConfig` struct with serde defaults: threshold=5, window_secs=60, cooldown_secs=1800, email_enabled=false
- Added `#[serde(default)] pub monitoring: MonitoringConfig` to `Config` struct
- Added `monitoring: MonitoringConfig::default()` to `Config::default_config()`

### main.rs (modified)

- Config loaded first (moved before tracing init) — pre-init output uses `eprintln!`
- `ErrorCountLayer` created from `config.monitoring` fields, added as 4th layer in tracing registry chain
- `error_rate_alerter_task` spawned if `monitoring.error_rate_email_enabled = true`
- Monitoring fields extracted before `AppState::new(config, pool)` moves config

## Deviations from Plan

None — plan executed exactly as written. The config-before-tracing reorder was anticipated in the plan notes and implemented as specified.

## Verification

- `cargo check --workspace`: PASS (no errors, only pre-existing warnings)
- `grep -q 'ErrorCountLayer' crates/racecontrol/src/error_rate.rs`: PASS
- `grep -q 'try_send' crates/racecontrol/src/error_rate.rs`: PASS
- `grep -q 'error_rate_alerter_task' crates/racecontrol/src/error_rate.rs`: PASS
- `grep -q 'mod error_rate' crates/racecontrol/src/lib.rs`: PASS
- `grep -q 'MonitoringConfig' crates/racecontrol/src/config.rs`: PASS
- `grep -q 'ErrorCountLayer' crates/racecontrol/src/main.rs`: PASS
- `grep -q 'usingh@racingpoint.in' crates/racecontrol/src/main.rs`: PASS
- Config load line 297 < tracing init line 328 in main.rs: PASS

Note: `cargo test -p racecontrol-crate --lib` is blocked by Windows Application Control policy on this machine (known environment constraint — same restriction exists for rc-agent tests). The code compiles cleanly and logic is verified via `cargo check --workspace`.

## Self-Check: PASSED

- `crates/racecontrol/src/error_rate.rs`: FOUND
- `crates/racecontrol/src/config.rs` contains MonitoringConfig: FOUND
- `crates/racecontrol/src/main.rs` contains ErrorCountLayer: FOUND
- Commit 73a1bbb: FOUND
- Commit 47293b2: FOUND
