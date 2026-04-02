---
phase: 290-wire-metric-producers
plan: 01
subsystem: metrics-tsdb
tags: [metrics, tsdb, producers, rust]
dependency_graph:
  requires: []
  provides: [live-metrics-in-tsdb]
  affects: [metrics-query-api, alerts, prometheus-exporter]
tech_stack:
  added: []
  patterns: [snapshot-drop-lock, try_send-non-blocking, 30s-interval-producer]
key_files:
  created:
    - crates/racecontrol/src/metrics_producers.rs
  modified:
    - crates/racecontrol/src/lib.rs
    - crates/racecontrol/src/main.rs
decisions:
  - Used try_send().ok() for non-blocking metric emission (drops on full channel rather than blocking)
  - Binary health indicator for pod_health_score (1.0=reachable, 0.0=not) since FleetHealthStore has no explicit score field
  - Single tokio task with 30s interval (not per-metric tasks) to minimize resource use
metrics:
  duration: "~15 minutes"
  completed_date: "2026-04-01"
  tasks_completed: 2
  tasks_total: 2
  files_created: 1
  files_modified: 2
---

# Phase 290 Plan 01: Wire Metric Producers — Summary

**One-liner:** Wired 4 live venue metric producers (ws_connections, game_session_count, pod_health_score, billing_revenue) into MetricsSender channel via 30-second interval loop, closing the P1 gap where TSDB was empty.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Create metrics_producers module with producer loops | d6c41c97 | metrics_producers.rs, lib.rs |
| 2 | Wire metrics_tx into main.rs and run cargo test | bb3f1ecc | main.rs |

## What Was Built

`crates/racecontrol/src/metrics_producers.rs` — new module with `spawn_metric_producers(state, metrics_tx)` that spawns one tokio task running a 30-second interval loop. Each tick emits:

1. **ws_connections** — count of entries in `state.agent_senders` (pod_id: None)
2. **game_session_count** — count of entries in `state.game_launcher.active_games` (pod_id: None)
3. **pod_health_score** — 1.0/0.0 per pod based on `FleetHealthStore::http_reachable` (pod_id: Some)
4. **billing_revenue** — `SUM(total_amount_paise)` for today from DB, converted to rupees (pod_id: None)

In `main.rs`: renamed `_metrics_tx` to `metrics_tx`, then passed it to `spawn_metric_producers`. MetricsSender channel is now actively consumed.

## Verification Results

- `cargo check --bin racecontrol` — PASS
- `cargo test --bin racecontrol` — 4/4 tests PASS
- `cargo build --release --bin racecontrol` — PASS (Finished in ~3 min)
- `grep -c "spawn_metric_producers" crates/racecontrol/src/metrics_producers.rs` — 1
- `grep -c "try_send" crates/racecontrol/src/metrics_producers.rs` — 4 (one per metric type)
- `grep -c "unwrap()" crates/racecontrol/src/metrics_producers.rs` — 0
- `grep -c "_metrics_tx" crates/racecontrol/src/main.rs` — 0

## Decisions Made

- **try_send vs send:** Non-blocking `try_send().ok()` used to prevent backpressure from slowing the producer loop. If the TSDB ingestion channel is full, samples are dropped silently (acceptable — 30s cadence means next tick recovers).
- **Binary health score:** `FleetHealthStore` has no numeric health_score field. Used `http_reachable` bool as binary 1.0/0.0. Future improvement: compute composite score from uptime, crash_loop, in_maintenance flags.
- **Single task:** One shared tokio task for all 4 metrics keeps lifecycle logging simple and resource use minimal.

## Deviations from Plan

None — plan executed exactly as written.

## Known Stubs

None — all 4 metric producers read real live data (agent senders map, game tracker map, fleet health store, billing_sessions DB table).

## Self-Check: PASSED

- `d6c41c97` exists in git log
- `bb3f1ecc` exists in git log
- `crates/racecontrol/src/metrics_producers.rs` exists
- Release binary built successfully
