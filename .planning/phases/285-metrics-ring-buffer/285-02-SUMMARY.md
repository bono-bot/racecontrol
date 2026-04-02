---
phase: 285-metrics-ring-buffer
plan: 02
subsystem: database
tags: [sqlite, tsdb, mpsc, tokio, async, metrics]

requires:
  - phase: 285-01
    provides: MetricSample struct, record_sample, record_samples_batch, compute_hourly_rollups, compute_daily_rollups, DB schema
provides:
  - MetricsSender mpsc channel type for non-blocking metric ingestion
  - spawn_metrics_ingestion background task (batch=64, flush=5s)
  - purge_old_samples (7-day retention) and purge_old_rollups (90-day retention)
  - spawn_rollup_and_purge scheduler (60min interval)
  - Background tasks wired in main.rs
affects: [286-metrics-producers, 287-metrics-api, 288-metrics-dashboard]

tech-stack:
  added: [tokio::sync::mpsc]
  patterns: [mpsc-channel-batched-writes, scheduled-background-purge]

key-files:
  created: []
  modified:
    - crates/racecontrol/src/metrics_tsdb.rs
    - crates/racecontrol/src/main.rs

key-decisions:
  - "512-buffer mpsc channel with 64-sample batch size and 5s flush interval"
  - "_metrics_tx binding keeps channel alive for server lifetime; producers wired in future plans"

patterns-established:
  - "mpsc batched write: collect samples in Vec, flush on size threshold or timer"
  - "Scheduled purge: 2min startup delay, then 60min interval for rollup+purge"

requirements-completed: [TSDB-02, TSDB-06, TSDB-07]

duration: 3min
completed: 2026-04-01
---

# Phase 285 Plan 02: Async Ingestion Pipeline + Purge Summary

**Async mpsc-based metric ingestion (batch=64, flush=5s), 7-day raw purge, 90-day rollup purge, all wired as background tasks in main.rs**

## Performance

- **Duration:** 3 min
- **Started:** 2026-04-01T10:21:13Z
- **Completed:** 2026-04-01T10:28:09Z
- **Tasks:** 2/2
- **Files modified:** 2

## Accomplishments

### Task 1: Async ingestion pipeline and purge functions
- Added `MetricsSender` type alias (`mpsc::Sender<MetricSample>`)
- `spawn_metrics_ingestion()`: spawns tokio task that batches up to 64 samples or flushes every 5s via `tokio::select!`
- `purge_old_samples()`: DELETE FROM metrics_samples WHERE recorded_at < 7 days ago
- `purge_old_rollups()`: DELETE FROM metrics_rollups WHERE period_start < 90 days ago
- `spawn_rollup_and_purge()`: 2min startup delay, then hourly tick running rollups + purge
- **Commit:** `24f546c4`

### Task 2: Wire into main.rs
- Added `spawn_metrics_ingestion` and `spawn_rollup_and_purge` calls after `spawn_alert_checker`
- `_metrics_tx` binding keeps the channel alive; producers will be wired in phase 286+
- **Commit:** `b415c865`

## Deviations from Plan

None - plan executed exactly as written.

## Known Stubs

- `_metrics_tx` is created but no producers send to it yet (intentional -- wired in phase 286+)

## Verification

- `cargo check -p racecontrol-crate`: no errors in metrics_tsdb.rs or main.rs (pre-existing BillingTimer errors unrelated)
- Zero `.unwrap()` calls in metrics_tsdb.rs
- All 5 new public functions/types confirmed via grep
- Purge thresholds: 7 days (raw samples), 90 days (rollups)
