# Phase 366: Fleet Intelligence - Context

**Gathered:** 2026-04-10
**Status:** Ready for planning
**Mode:** `--auto` (Claude picked recommended defaults for all gray areas; log in 366-DISCUSSION-LOG.md)

<domain>
## Phase Boundary

**What this phase delivers:** Fleet-wide aggregation and anomaly detection on top of the
per-session signals produced by Phases 363 and 364. Specifically:

- **GLD-F-01** Per-pod composite health score (0-100, 7-day rolling), surfaced via new
  `GET /api/v1/fleet/intelligence` endpoint.
- **GLD-F-02** Time-of-day failure pattern analysis per pod (hour-correlated failure rates).
- **GLD-F-03** Content drift detector -- compares each pod's physical inventory vs its
  committed TOML; emits `ContentDriftDetected` WS event on any delta.
- **GLD-F-04** Concurrent session guard -- server-side mutex so a single pod cannot have
  two active billing sessions or two active game launches simultaneously.

**Inputs from upstream phases (already deployed or code-complete):**
- `billing_sessions.lap_count_flag`, `.telemetry_coverage_pct`, `.suspect`,
  `.suspect_reasons`, `.csv_fallback_received_at`, `.lap_reject_grace_until`
  (Phase 363, code-complete; NOT yet deployed as of 2026-04-10).
- `TelemetryGap`, `LapConsistencyAlert`, `StalledSession` WS events (Phase 364,
  planned but not yet coded).
- Existing `FleetHealthStore` fields in `fleet_health.rs`: `crash_recovery`,
  `crashes_last_hour`, `crash_loop`, `idle_health_fail_count`, `in_maintenance`,
  `http_reachable`, `violation_count_24h`, `clock_drift_secs`.
- Existing `metrics_tsdb.rs` TSDB with `METRIC_POD_HEALTH_SCORE` already defined
  (currently a simple binary 1.0/0.0 from `http_reachable`).
- Existing `GET /api/v1/fleet/health` endpoint (binary pod up/down status).
- Pod inventory TOML reader at `GET /api/v1/pods/{id}/inventory` (Phase 361).
- `audit_known_issues` table in `fleet_kb.rs` (Mesh Intelligence Tier 0).

**NOT in scope (hard boundary):**
- Admin UI rendering of `/fleet/intelligence` -- Phase 367 GLD-G-03.
- Suspect lap views or staff action buttons -- Phase 367.
- AI-tier-aware health targets -- Phase 365.
- Historical backfill of health scores for pre-Phase-366 sessions.
- Cloud replication of new `fleet_intelligence` data -- inherits Phase 301 pipeline
  but cloud deploy parity is a deploy step, not a new feature.
- Any rc-agent changes -- server-only phase.

</domain>

<decisions>
## Implementation Decisions

### GLD-F-01: Per-Pod Health Score Aggregator

- **D-01:** Health score is a **weighted composite of four components**, computed on query
  (not pre-materialized), using a 7-day rolling window from `billing_sessions`:
  - Session success rate: 40 pts -- `1 - (suspect_count / total_sessions)` in last 7 days.
  - Telemetry completeness: 30 pts -- average `telemetry_coverage_pct` across last 7 days.
  - Config mismatch rate: 20 pts -- `1 - (config_mismatch_count / total_sessions)` from
    `config_mismatch_detected` WS events stored in a new `session_events` table (see D-05).
  - Crash rate: 10 pts -- `crashes_last_hour` from `FleetHealthStore` (live, not DB).
  
  **Formula:** `score = round((success_rate * 40) + (telemetry_avg/100 * 30) + (mismatch_rate * 20) + (1 - crash_penalty) * 10)`
  where `crash_penalty = min(crashes_last_hour / 5, 1.0)` (5+ crashes in the last hour = 0 pts on crash component).

  **Rationale for compute-on-query:** Avoids a background job that could drift out of sync;
  a single `/fleet/intelligence` call is infrequent (staff dashboard polling); 8-pod fleet
  means each call runs 8 queries over indexed tables -- sub-100ms per call expected.
  _Rejected: pre-materialized score in a new table -- introduces a staleness window and
  a background job that requires watchdog supervision; overkill for 8 pods._

- **D-02:** If a pod has **fewer than 3 sessions in the 7-day window**, the score is marked
  `insufficient_data: true` and a default score of `null` (not 0) is returned. Zero would
  mislead staff into thinking a pod is unhealthy when it has no data. Phase 367 renders
  "No data" badge for `null` scores.

- **D-03:** The endpoint is `GET /api/v1/fleet/intelligence` (new route, separate from
  the existing `/fleet/health`). Response schema:
  ```json
  {
    "generated_at": "<ISO-8601>",
    "pods": [{
      "pod_id": "pod-1",
      "score": 87,
      "insufficient_data": false,
      "components": {
        "session_success_rate": 0.95,
        "telemetry_completeness_avg": 91.2,
        "config_mismatch_rate": 0.02,
        "crashes_last_hour": 0
      },
      "window_days": 7,
      "sessions_in_window": 14
    }]
  }
  ```
  Auth: staff JWT (same as `/fleet/health`).

- **D-04:** The existing `METRIC_POD_HEALTH_SCORE` in `metrics_tsdb.rs` is upgraded: the
  metric producer in `metrics_producers.rs` calls the Phase 366 scoring function and
  emits the 0-100 composite value (not the old binary 0.0/1.0) every 30 seconds. This
  means the TSDB time-series already has the composite score available for historical
  trending without extra infrastructure.
  **Backward compat:** old consumers of the binary metric will see values between 0-100
  instead of 0 or 1. This is acceptable -- no consumer enforces the binary contract.

### GLD-F-02: Time-of-Day Pattern Analysis

- **D-05:** Time-of-day analysis is a **batch SQL query** grouped by hour-of-day (0-23)
  over the last 30 days (wider window than health score -- patterns need volume). Uses
  `billing_sessions.started_at` + the `suspect` flag. Returns a per-pod, per-hour failure
  rate. Added as a sub-section in the `/fleet/intelligence` response under a `time_patterns`
  key:
  ```json
  "time_patterns": [{
    "pod_id": "pod-6",
    "flagged_hours": [{"hour": 14, "failure_rate": 0.45, "sample_count": 11}],
    "threshold_pct": 30
  }]
  ```
  An hour is "flagged" when its failure rate >= 30% AND sample count >= 3. Threshold chosen
  to suppress false positives in low-traffic hours while catching systematic thermal/task failures.
  _Rejected: background job writing to a new table -- same staleness argument as D-01;
  the query is cheap and on-demand is sufficient._

- **D-06:** No ML -- pure SQL aggregation. Phase 365 AI tier is the right place for ML
  enhancement. Phase 366 ships the infrastructure and the threshold-based detector.

### GLD-F-03: Content Drift Detector

- **D-07:** Drift detection runs **on a schedule** (every 60 minutes via a background tokio
  task), NOT on every session start. Rationale: checking a TOML file on disk 8 times per
  session start is wasteful; drift changes slowly (a game gets uninstalled or a TOML is
  updated). A 60-minute poll gives staff a fresh alert within a working hour.

- **D-08:** The comparison is **TOML-as-ground-truth vs rc-agent live disk** (NOT TOML vs
  TOML). Phase 361 built `GET /api/v1/pods/{id}/inventory` which reads the TOML. Phase 362
  built `SessionConfig` verification via shared memory on pods. Phase 366 uses the existing
  `/debug/content-dirs` rc-agent probe (if available) OR the pod inventory TOML as the
  expected state, comparing against the live disk state reported by rc-agent. If rc-agent
  does not expose a content inventory endpoint, Phase 366 falls back to comparing the TOML
  vs the TOML of the PREVIOUS snapshot (catching TOML mutations, not disk mutations).
  **Researcher must verify:** does `rc-agent` expose a live disk content list endpoint?
  If not, fallback is TOML change detection (git diff on TOML files) -- still useful for
  catching TOML mutations that don't match the deployed fleet.

- **D-09:** On drift detected: emit `ContentDriftDetected` WS event to admin clients with
  `{pod_id, expected_game_key, delta_type: "added"|"removed", item: "car_name_or_game_key"}`.
  Also write to a new `content_drift_events` table for audit log and Phase 367 rendering.
  WhatsApp alert fires if severity is `game_removed` (a whole game missing -- P2-10 class);
  car/track drift fires WS only (too noisy for WhatsApp).

- **D-10:** The `content_drift_events` table:
  ```sql
  CREATE TABLE IF NOT EXISTS content_drift_events (
    id TEXT PRIMARY KEY,
    pod_id TEXT NOT NULL,
    detected_at TEXT NOT NULL,
    game_key TEXT NOT NULL,
    delta_type TEXT NOT NULL,  -- 'game_added', 'game_removed', 'car_added', 'car_removed', 'track_added', 'track_removed'
    item TEXT NOT NULL,
    resolved_at TEXT,          -- NULL = unresolved; staff marks resolved via Phase 367 UI
    resolution_note TEXT
  );
  ```
  This is a NEW table (not the `audit_known_issues` table). Rationale: `audit_known_issues`
  is for MI symptom-to-fix knowledge (operational patterns); `content_drift_events` is
  append-only audit log of physical inventory state changes -- different lifecycle,
  different consumers. `audit_known_issues` is NOT reused for content drift.

### GLD-F-04: Concurrent Session Guard

- **D-11:** The guard is a **server-side check at session-start time**, added to the
  `start_billing_session()` path in `billing.rs`. Before inserting a new session, check
  `active_timers` (already in-memory in `BillingManager`) for an existing entry for the
  same `pod_id`. If one exists, return HTTP 409 with body:
  ```json
  {"error": "pod_already_active", "active_session_id": "<id>", "pod_id": "<id>"}
  ```
  _This is the minimal, correct approach._ The `active_timers` map is already the single
  source of truth for active billing state (used by lockdown guard at line 1239).
  Existing guard pattern confirmed at `lockdown_pod()` -- same check, same map.

- **D-12:** The guard applies to **ALL session-start paths** -- kiosk, PWA, admin. All three
  ultimately call `start_billing_session()`. No per-path logic needed; one check in one place.

- **D-13:** The guard also covers **game launches** via `game_launcher.active_games` map
  (already in `AppState`). If a game launch is already active for `pod_id`, return HTTP 409.
  The two checks (billing + game launch) are independent -- either alone triggers the 409.
  Test: a parallel `curl` smoke test firing two simultaneous POST /billing/start requests
  for the same pod must result in one 200 and one 409.

- **D-14:** NO DB query for the concurrent guard. The in-memory `active_timers` and
  `active_games` maps are the source of truth for live state. Adding a DB read-before-insert
  would introduce a TOCTOU race and slow down the hot path. The in-memory check is O(1)
  and consistent with the existing lockdown guard pattern.

### Storage and Architecture

- **D-15:** **No new persistent store** for the health score itself -- it is computed
  on-query from existing `billing_sessions` table (Phase 363 columns) and the live
  `FleetHealthStore` in memory. The score IS written to the existing `metrics_samples`
  TSDB every 30 seconds (via upgraded `spawn_metric_producers`) for historical trending.
  Storage approach: existing SQLite TSDB + compute-on-query.

- **D-16:** The `content_drift_events` table (D-10) is the only new SQLite table in Phase 366.
  Schema is additive (new table, no ALTER on existing tables). Migration goes in the existing
  `db::migrate()` chain (same pattern as all prior phases).

- **D-17:** **Real-time vs batch:** Mixed.
  - Health score: compute-on-query (real-time per request). Emitted to TSDB every 30s.
  - Time-of-day analysis: compute-on-query within the `/fleet/intelligence` response.
  - Content drift: batch poll every 60 minutes via background tokio task.
  - Concurrent session guard: synchronous in-memory check on every session start (hot path).

### MI Table Decision

- **D-18:** Phase 366 does NOT write to `audit_known_issues`. That table is for MI
  symptom-to-fix solutions (e.g., "pod crashes on AC launch → apply fix X"). Fleet
  intelligence health scores and content drift are operational metrics, not knowledge base
  entries. The relevant MI interaction is read-only: the Phase 367 admin UI may cross-
  reference `audit_known_issues` when displaying a pod with a low health score ("known fixes
  for this symptom type"), but that is Phase 367's concern.

### Cloud Sync

- **D-19:** The `content_drift_events` table replicates via the existing Phase 301
  cloud_data_sync_v2 pipeline. The Phase 366 PLAN must update `sync/` to include the new
  table in the upsert payload. Cloud deploy parity required (same migration deployed to
  Bono VPS in same session per deploy parity rule). Health scores are NOT synced to cloud
  (computed from local `billing_sessions`; cloud has its own sessions, computes its own).

### Claude's Discretion
- Component weight tuning (40/30/20/10 split) can be adjusted by researcher if the existing
  `billing_sessions` data distribution suggests different weights would produce more useful
  signal. The researcher should query the current data distribution.
- Content drift poll interval (60 min) can be made configurable via `AppConfig` if the
  researcher finds a config key pattern already used for other background tasks.
- The `time_patterns` flagging threshold (30%, min 3 samples) can be tuned.
- Whether to add a `feature_flag` kill switch for the concurrent session guard (consistent
  with Phase 363's `phase363_session_audit` kill switch pattern).

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase Requirements
- `.planning/milestones/v46.0-REQUIREMENTS.md` -- GLD-F-01..04 acceptance criteria, P2-09/P2-10/P2-12 silent-loss points
- `.planning/milestones/v46.0-ROADMAP.md` -- Phase 366 success criteria (4 items)

### Upstream Phase Decisions (what exists post-363/364)
- `.planning/phases/363-data-recording-verification/363-CONTEXT.md` -- D-01..D-15: all new `billing_sessions` columns, especially `lap_count_flag`, `telemetry_coverage_pct`, `suspect`, `suspect_reasons`, `csv_fallback_received_at`
- `.planning/phases/363-data-recording-verification/363-01-SUMMARY.md` -- what 363-01 actually built

### Existing Fleet Infrastructure (MUST READ before coding)
- `crates/racecontrol/src/fleet_health.rs` -- `FleetHealthStore` fields (crash, idle, maintenance, violation, WS reconnect), `fleet_health_handler`, `/fleet/health` route
- `crates/racecontrol/src/metrics_tsdb.rs` -- TSDB schema, `METRIC_POD_HEALTH_SCORE`, `MetricSample`, `record_sample()`
- `crates/racecontrol/src/metrics_producers.rs` -- existing `spawn_metric_producers()` (upgrade this for composite score)
- `crates/racecontrol/src/fleet_kb.rs` -- `audit_known_issues` table schema (context only -- Phase 366 does NOT write here)
- `crates/racecontrol/src/fleet_alert.rs` -- `post_fleet_alert()` pattern (WhatsApp escalation -- reuse for content drift alerts)
- `crates/racecontrol/src/billing.rs` -- `BillingManager`, `active_timers` map, `start_billing_session()` path (lines ~662+ for BillingManager, ~908+ for session start flow, lockdown guard pattern at ~1237)
- `crates/racecontrol/src/api/routes.rs` -- route registration patterns; `/fleet/health` at line 94; `/games/active` at line 434 (game launcher map)
- `crates/racecontrol/src/api/pods.rs` -- Phase 361 TOML reader and `PodInventory` type (content drift comparison baseline)
- `crates/racecontrol/src/session_audit.rs` -- Phase 363 implementation (suspect flag, coverage metric patterns)

### Deploy Rules
- `CLAUDE.md` -- deploy parity rule, cloud sync parity, migration rules, fleet = ALL targets
- `.planning/milestones/v46.0-REQUIREMENTS.md` §"Out of Scope" -- what NOT to build

</canonical_refs>
