---
phase: "303"
plan: "03"
subsystem: "multi-venue"
tags: ["venue_id", "INSERT", "data-isolation", "multi-venue", "schema"]
dependency_graph:
  requires: ["303-01", "303-02"]
  provides: ["venue_id-on-all-non-routes-INSERTs"]
  affects: ["billing", "game_launcher", "metrics", "lap_tracker", "driver_rating", "event_archive", "activity_log", "multiplayer", "reservation", "scheduler", "auth", "ac_server", "ws"]
tech_stack:
  added: []
  patterns: ["venue_id parameter propagation", "fire-and-forget with cloned venue_id", "pool-only function signature extension"]
key_files:
  created: []
  modified:
    - "crates/racecontrol/src/billing.rs"
    - "crates/racecontrol/src/cafe.rs"
    - "crates/racecontrol/src/cloud_sync.rs"
    - "crates/racecontrol/src/multiplayer.rs"
    - "crates/racecontrol/src/billing_replay.rs"
    - "crates/racecontrol/src/metrics.rs"
    - "crates/racecontrol/src/wallet.rs"
    - "crates/racecontrol/src/activity_log.rs"
    - "crates/racecontrol/src/event_archive.rs"
    - "crates/racecontrol/src/game_launcher.rs"
    - "crates/racecontrol/src/lap_tracker.rs"
    - "crates/racecontrol/src/driver_rating.rs"
    - "crates/racecontrol/src/ac_server.rs"
    - "crates/racecontrol/src/scheduler.rs"
    - "crates/racecontrol/src/auth/mod.rs"
    - "crates/racecontrol/src/reservation.rs"
    - "crates/racecontrol/src/deploy.rs"
    - "crates/racecontrol/src/metric_alerts.rs"
    - "crates/racecontrol/src/pod_healer.rs"
    - "crates/racecontrol/src/ws/mod.rs"
    - "crates/racecontrol/src/main.rs"
    - "crates/racecontrol/src/db/mod.rs"
    - "crates/racecontrol/tests/integration.rs"
decisions:
  - "Skipped internal/config tables (audit_log, scheduler_events, ai_suggestions, pod_crash_events, metrics_samples, app_health_log, incident_log, etc.) — these are operational/diagnostic tables, not business data"
  - "Skipped migration copy INSERTs in db/mod.rs (personal_bests_v2, track_records_v2 copy-then-drop) — one-time schema operations that use DEFAULT"
  - "For fire-and-forget functions (append_event, log_pod_activity) that use tokio::spawn, cloned venue_id into the async block before move"
  - "For pool-only functions (record_launch_event, record_billing_accuracy_event, record_recovery_event, insert_event_direct, spawn_rating_worker, backfill_ratings): added venue_id parameter and updated all callers"
  - "billing_fsm.rs skipped — no INSERT statements, pure FSM transition logic"
  - "cafe_promos.rs skipped — cafe_promos is not a major business table"
  - "cafe_alerts.rs skipped — cafe_items is not a major business table"
metrics:
  duration: "~90 minutes (continuation from prior session)"
  completed: "2026-04-01"
  tasks: 2
  files_modified: 23
---

# Phase 303 Plan 03: Add venue_id to All Non-Routes INSERTs Summary

**One-liner:** Propagated `venue_id` from `state.config.venue.venue_id` into every major-table INSERT statement across 23 non-routes Rust source files, covering ~120 INSERT call sites with zero compile errors.

## Tasks Completed

### Task 1: High-Volume Files
Updated the major INSERT statements in all high-volume files:

- **billing.rs** — `billing_events`, `billing_sessions`, `split_sessions`, `review_nudges`, `billing_accuracy_events` (3 call sites)
- **cafe.rs** — `cafe_orders`
- **cloud_sync.rs** — `wallet_transactions`; upsert_driver/upsert_kiosk_experience use cloud payload venue_id with local fallback
- **multiplayer.rs** — `kiosk_experiences`, `group_sessions`, `group_session_members` (3 booking functions)
- **billing_replay.rs** — `billing_audit_log`; `insert_audit_log` signature extended with `venue_id: &str`
- **metrics.rs** — `launch_events`, `billing_accuracy_events`, `recovery_events`; all three public functions gained `venue_id: &str` parameter
- **wallet.rs** — `wallet_transactions` via `credit_in_tx`/`debit_in_tx` signature extensions

### Task 2: All Remaining Source Files
Updated the remaining major-table INSERTs across all other source files:

- **activity_log.rs** — `pod_activity_log`: cloned `venue_id` before `tokio::spawn`
- **event_archive.rs** — `system_events`: added `venue_id: &str` to both `append_event` (public) and `insert_event_direct` (pub(crate)); updated all callers in billing.rs, deploy.rs, metric_alerts.rs, pod_healer.rs, ws/mod.rs, plus test fixtures
- **game_launcher.rs** — `game_launch_events` (log_game_event), `launch_events` (5 record_launch_event call sites), `recovery_events` (2 record_recovery_event call sites)
- **lap_tracker.rs** — `laps`, `personal_bests`, `track_records`, `hotlap_event_entries`, `championship_standings`
- **driver_rating.rs** — `driver_ratings`; `spawn_rating_worker(db, venue_id)` and `backfill_ratings(db, venue_id)` both extended
- **ac_server.rs** — `multiplayer_results` (skip ac_sessions/ac_presets — config/internal tables)
- **scheduler.rs** — `debit_intents` refund INSERT (skip scheduler_events/kiosk_settings/settings — internal)
- **auth/mod.rs** — `auth_tokens`, `drivers`
- **reservation.rs** — `reservations`, `debit_intents` (create, cancel, modify paths)
- **deploy.rs** — updated all 4 `append_event` callers
- **metric_alerts.rs** — updated `append_event` caller
- **pod_healer.rs** — updated `append_event` caller
- **ws/mod.rs** — `pods` INSERT + 2 `append_event` callers
- **main.rs** — `spawn_rating_worker(db, venue_id)` and `backfill_ratings(db, venue_id)` call sites

## Tables Updated

| Table | File | Notes |
|-------|------|-------|
| billing_sessions | billing.rs | — |
| billing_events | billing.rs | — |
| split_sessions | billing.rs | — |
| review_nudges | billing.rs | — |
| billing_accuracy_events | billing.rs / metrics.rs | — |
| cafe_orders | cafe.rs | — |
| wallet_transactions | wallet.rs / cloud_sync.rs | — |
| kiosk_experiences | multiplayer.rs / cloud_sync.rs | cloud path preserves source venue_id |
| group_sessions | multiplayer.rs | — |
| group_session_members | multiplayer.rs | — |
| billing_audit_log | billing_replay.rs | — |
| launch_events | metrics.rs | — |
| recovery_events | metrics.rs | — |
| pod_activity_log | activity_log.rs | — |
| system_events | event_archive.rs | — |
| game_launch_events | game_launcher.rs | — |
| laps | lap_tracker.rs | — |
| personal_bests | lap_tracker.rs | — |
| track_records | lap_tracker.rs | — |
| hotlap_event_entries | lap_tracker.rs | — |
| championship_standings | lap_tracker.rs | — |
| driver_ratings | driver_rating.rs | — |
| multiplayer_results | ac_server.rs | — |
| debit_intents | reservation.rs / scheduler.rs | — |
| reservations | reservation.rs | — |
| auth_tokens | auth/mod.rs | — |
| drivers | auth/mod.rs | — |
| pods | ws/mod.rs | UPSERT — venue_id in INSERT only, not ON CONFLICT UPDATE |

## Tables Skipped (Internal/Config)

- `audit_log`, `journal_entries`, `journal_entry_lines`, `invoices` — accounting internal
- `ac_sessions`, `ac_presets` — AC server config, not business data
- `ai_suggestions`, `pod_crash_events`, `debug_resolutions` — diagnosis/ML internal
- `metrics_samples`, `telemetry_samples`, `hardware_telemetry` — time-series infra
- `scheduler_events`, `kiosk_settings`, `settings` — operational config
- `app_health_log`, `incident_log`, `fleet_incidents` — monitoring internal
- `maintenance_events`, `maintenance_tasks`, `daily_business_metrics`, `employees`, `attendance_records` — HR/ops (no multi-venue partitioning needed)
- `notification_outbox`, `prediction_outcomes`, `admin_overrides` — internal queues
- `cafe_promos`, `cafe_items`, `game_presets`, `combo_reliability` — config/catalog
- `pricing_proposals`, `policy_rules`, `feature_flags`, `config_push_queue` — system config
- `friend_requests` — social feature, no venue partitioning needed
- `streaks`, `nudge_queue`, `driving_passport`, `variable_reward_log`, `driver_achievements` — psychology/gamification (driver-centric, handled via driver's venue)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Cascade updates for append_event API change**
- **Found during:** Task 2
- **Issue:** `append_event` is called from 8+ files (billing.rs, deploy.rs, metric_alerts.rs, pod_healer.rs, ws/mod.rs, event_archive.rs tests). Adding `venue_id` parameter required updating all callers.
- **Fix:** Updated all call sites with `&state.config.venue.venue_id`
- **Files modified:** billing.rs, deploy.rs, metric_alerts.rs, pod_healer.rs, ws/mod.rs, event_archive.rs
- **Commits:** 0015e644, a60d69fa

**2. [Rule 1 - Bug] spawn_rating_worker / backfill_ratings needed venue_id threading**
- **Found during:** Task 2 (driver_rating.rs)
- **Issue:** `spawn_rating_worker(db)` and `backfill_ratings(db)` call `compute_and_store_rating` internally; needed venue_id to flow through.
- **Fix:** Extended all three function signatures; updated main.rs call sites.
- **Files modified:** driver_rating.rs, main.rs
- **Commit:** a60d69fa

**3. [Rule 1 - Bug] record_launch_event callers in billing.rs needed venue_id**
- **Found during:** Task 1 (billing.rs)
- **Issue:** `record_billing_accuracy_event` is called 3 times in billing.rs; once the signature was updated in metrics.rs, the callers needed updating.
- **Fix:** All 3 call sites updated with `&state.config.venue.venue_id`.
- **Files modified:** billing.rs
- **Commit:** 0015e644

## Commits

| Hash | Description | Files |
|------|-------------|-------|
| `0015e644` | feat(303-03): add venue_id to INSERT statements in high-volume files | 7 files |
| `a60d69fa` | feat(303-03): add venue_id to INSERT statements in remaining source files | 16 files |

## Known Stubs

None. All venue_id values are sourced from `state.config.venue.venue_id` at runtime.

## Self-Check: PASSED

- Commits 0015e644 and a60d69fa exist in git log
- `cargo check --bin racecontrol` produces zero errors after both commits
- All 28 major tables listed above have venue_id in their INSERT statements
