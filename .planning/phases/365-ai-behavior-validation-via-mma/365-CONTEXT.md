# Phase 365: AI Behavior Validation via MMA - Context

**Gathered:** 2026-04-10 (assumptions mode, --auto)
**Status:** Ready for planning
**Mode:** `--auto` (Claude picked recommended defaults for all gray areas; log in DISCUSSION-LOG.md)

<domain>
## Phase Boundary

**What this phase delivers:** A knowledge base of expected AI lap times per (car, track, difficulty_tier)
and live anomaly detection that fires `AiBehaviorAnomaly` when running AI deviates from KB.

Specifically:
- `ai_behavior_samples` DB table: new table collecting AI median lap times per session
- Weekly MMA batch job: uses OpenRouter (5-model consensus) to derive expected bands from samples
- KB TOML files: `.planning/kb/ai-behavior/{car}-{track}.toml` — per-tier expected bands
- Live anomaly detector: in-session check that fires `AiBehaviorAnomaly` WS event

**Requirements (from `v46.0-REQUIREMENTS.md`):**
- **GLD-E-01**: AI lap time collector — after lap 3, median AI lap times recorded to `ai_behavior_samples` keyed by `(car, track, ai_level, ai_aggression)`.
- **GLD-E-02**: MMA difficulty-tier comparison — weekly batch detects tier misalignment, 3/5 consensus.
- **GLD-E-03**: AI behavior KB in `.planning/kb/ai-behavior/{car}-{track}.toml`.
- **GLD-E-04**: Anomaly alerts — `AiBehaviorAnomaly` WS event fires on >3 consecutive laps outside KB band.

**Closes silent-loss points:** P2-07 (no AI behavior validation), P2-11 (difficulty tier contract untested).

**NOT in scope (hard boundary):**
- Admin UI for AI performance trends — Phase 367 GLD-G-?? or shared with admin dashboard. Phase 365
  writes the data; display is Phase 367's job. (Note: ROADMAP.md SC-4 mentions "Admin dashboard surfaces
  per-car-track AI performance trend (shared UI with Phase 367)" — Phase 365 provides the data endpoint
  only, not the frontend.)
- Refining Phase 363's `expected_laps` heuristic — that is a separate decision point; Phase 365's KB
  DOES NOT feed back into Phase 363's formula. Phase 363 uses its own floor heuristic for session audit.
  Phase 365 is a standalone AI validation layer.
- Real MMA API calls during planning or testing — batch job makes live OpenRouter calls only when
  running in production with a valid key. Tests use mocked responses.
- Multi-sim AI lap collection (iRacing, F1 25, LMU) — Phase 365 targets Assetto Corsa AI sessions
  only for v1.0 (AC is the only sim with defined DifficultyTier mapping in codebase). Others are
  deferred.

</domain>

<decisions>
## Implementation Decisions

### AI Lap Collection (GLD-E-01)

- **D-01:** AI lap times are collected into a NEW `ai_behavior_samples` table, NOT by adding `is_ai`
  to the existing `laps` table. Rationale: existing `laps` is a human-driver leaderboard table
  (driver_id FK, personal_bests, cloud sync). Mixing AI records would pollute leaderboards and the
  cloud sync payload. `ai_behavior_samples` is a dedicated analytics table with no FK to `drivers`.
  - _Rejected: (a) Add `is_ai BOOLEAN` to laps + filter in queries — pollutes leaderboards and
    cloud sync; every existing query would need `WHERE NOT is_ai` guards. (b) Separate DB file —
    unnecessary complexity; SQLite handles multiple tables cleanly._
  - **Auto-pick rationale:** Clean separation between competitive (human) and analytical (AI) data.

- **D-02:** `ai_behavior_samples` table schema:
  ```sql
  CREATE TABLE IF NOT EXISTS ai_behavior_samples (
      id TEXT PRIMARY KEY,
      session_id TEXT NOT NULL,          -- references sessions(id) loosely (no FK to avoid cascade issues)
      pod_id TEXT NOT NULL,
      sim_type TEXT NOT NULL DEFAULT 'assettocorsa',
      car TEXT NOT NULL,
      track TEXT NOT NULL,
      ai_level INTEGER NOT NULL,          -- 0-100 raw value (same as race.ini AI_LEVEL)
      difficulty_tier TEXT NOT NULL,      -- 'rookie'/'amateur'/'semi_pro'/'pro'/'alien' from tier_for_level()
      lap_count INTEGER NOT NULL,         -- number of AI laps used to compute median
      median_lap_ms INTEGER NOT NULL,     -- median AI lap time in milliseconds
      p25_lap_ms INTEGER,                 -- 25th percentile (optional, for band width)
      p75_lap_ms INTEGER,                 -- 75th percentile (optional, for band width)
      sampled_at TEXT NOT NULL DEFAULT (datetime('now')),
      kb_batch_id TEXT                    -- set when this sample is incorporated into a KB update
  )
  ```
  - Index on `(car, track, difficulty_tier)` for KB batch queries.
  - Index on `(sampled_at)` for recency queries.

- **D-03:** The collector fires ONCE per session at "lap 3 completed" threshold (GLD-E-01: "after
  lap 3"). It queries all AI car lap times from the **AC server's result data** for that session.
  Specifically: after the 3rd completed lap of any AI driver, compute the median of all valid AI
  lap times so far and INSERT one row into `ai_behavior_samples`. The session's `ai_level` is read
  from `sessions.config_json` (already stored there when the session is launched via
  `start_ac_server()` / multiplayer.rs).
  - **AI lap source for AC:** The AC server writes per-car lap times to `results/` JSON on session
    end AND exposes `/api/lap` endpoint via `ac_server.rs`. For live collection (not just end-of-
    session), the AC server's live API is the source. Research must confirm which endpoint exposes
    per-car (including AI car) lap times.
  - **AI lap source for solo sessions:** In solo trackday sessions (kiosk → single pod), AI cars
    run in the same AC instance. SharedMemory `SPageFilePhysics` has per-car data for up to 60 cars
    (indices 1..N are AI). The rc-agent assetto_corsa.rs adapter currently only reads car index 0
    (player). Phase 365 requires reading indices 1..ai_count for AI lap times.
  - _Confidence: Likely — needs research to confirm whether rc-agent or racecontrol-side server
    query is the cleaner collection path._

- **D-04:** The collector is a server-side hook, NOT a pod-side rc-agent change (for multiplayer/
  group sessions where ac_server.rs runs on the server). For solo sessions, a thin pod-side hook
  in rc-agent is required to read AI lap data from SharedMemory.
  - **Auto-pick rationale:** Follow the Phase 362/363 pattern — server-side where possible, thin
    pod hook only when the data is unavailable server-side.

### MMA Batch Job (GLD-E-02)

- **D-05:** "Via MMA" means a **scheduled analytics batch job** using OpenRouter, NOT the Unified
  MMA Protocol v3.0 Q1-Q4 incident-response gate. The Unified MMA Protocol is for runtime fault
  diagnosis. Phase 365's MMA is a weekly offline consensus: "Given these AI lap samples, what are
  the expected bands per tier?" This is a distinct use case with different triggers, models, and
  output format.
  - **Auto-pick rationale:** Q1-Q4 gate is incident-driven; difficulty-tier band computation is
    period-driven. Conflating them would require surfacing a "problem" that doesn't exist yet.

- **D-06:** Batch job scheduling follows the `spawn_data_retention_job` pattern in routes.rs:
  a `tokio::spawn` background task with `tokio::time::interval(604800)` (7 days), initial delay
  3600s (1 hour after boot to avoid boot congestion). Lives in a new `ai_behavior_batch.rs` module.

- **D-07:** MMA model roster for the batch (3/5 majority = consensus):
  5 models via OpenRouter — use the existing OpenRouter key (`OPENROUTER_KEY` env var or
  `data/openrouter-mma-key.txt`, same path as `check_openrouter_key()` in server_diagnostics.rs).
  Model selection: Claude 3.5 Sonnet, GPT-4o, Gemini 1.5 Pro, Mistral Large, DeepSeek V3
  (same 5 used in prior MMA audits per `reference_multimodel_audit.md`).
  - Each model receives: car, track, difficulty_tier, list of median_lap_ms samples from the last
    30 days, and the question: "What are the expected p10-p90 lap time bands for this tier?"
  - Consensus: 3/5 agree within 5% → write KB TOML. Disagreement → no update (stale KB retained).

- **D-08:** The batch only runs if `ai_behavior_samples` has >= 10 rows for a (car, track, tier)
  tuple. Fewer than 10 samples = statistically insufficient; batch skips that tuple.

- **D-09:** Batch is **skippable via feature flag** `feature_flags.phase365_mma_batch` (default
  true). Kill-switch for cost control or key rotation. When disabled, KB is not updated but
  anomaly detection still uses the last written KB (stale is acceptable).

### KB File Format (GLD-E-03)

- **D-10:** KB files are stored as TOML in `.planning/kb/ai-behavior/{car_slug}-{track_slug}.toml`
  (directory created on first batch run). One file per (car, track) pair; all tiers within.
  Format:
  ```toml
  # Auto-generated by Phase 365 MMA batch. Do not edit manually.
  # Last updated: 2026-04-10T12:00:00Z  batch_id = "abc123"
  [rookie]
  p10_ms = 95000
  p50_ms = 102000
  p90_ms = 115000
  consensus_models = 3
  samples_used = 42

  [semi_pro]
  p10_ms = 85000
  p50_ms = 90000
  p90_ms = 97000
  consensus_models = 4
  samples_used = 67
  ```
  - Slugs: lowercase, spaces → dashes, special chars stripped (same as `generate-slug` in gsd-tools).
  - Missing tier entry = no KB for that tier; anomaly detector skips anomaly check for that tier.

- **D-11:** KB files are committed to the repo by the batch job (via `git commit` on the server)
  so that KB state is version-controlled and auditable. The batch job shells out to git only if
  running on the venue server (not cloud) to avoid dual-commit races.
  - **Alternative considered and rejected:** Store KB in a DB table. Git is better here: KB is
    planning-layer data (lives under `.planning/`), benefits from history/diff visibility, and
    matches the TOML file pattern already referenced in REQUIREMENTS.md GLD-E-03.

### Live Anomaly Detector (GLD-E-04)

- **D-12:** The anomaly detector runs **server-side**, checking AI lap times reported via the AC
  server's live API or WebSocket events. It maintains a per-session rolling window of the last 3
  AI lap times. If all 3 consecutive AI lap times fall outside the KB band (< p10 or > p90), fire
  `AiBehaviorAnomaly` WS event.

- **D-13:** `AiBehaviorAnomaly` WS event schema (added to `rc-common/protocol.rs`):
  ```json
  {
    "type": "AiBehaviorAnomaly",
    "pod_id": "pod-1",
    "session_id": "sess-xyz",
    "car": "tatuusfa1",
    "track": "magione",
    "difficulty_tier": "pro",
    "expected_p10_ms": 85000,
    "expected_p90_ms": 97000,
    "observed_laps": [110000, 112000, 108000],
    "direction": "too_slow",
    "timestamp": "2026-04-10T12:00:00Z"
  }
  ```
  - `direction`: `"too_slow"` | `"too_fast"` | `"unknown"` (unknown = KB has no data for this tier).

- **D-14:** If no KB entry exists for the (car, track, tier) combination, the anomaly detector
  does NOT fire — it silently skips. This prevents false positives in the first weeks before the
  KB is populated. Log a `tracing::debug!` for observability.

- **D-15:** Feature flag `feature_flags.phase365_anomaly_detection` (default true). Kill-switch
  independent of the batch flag — you can disable anomaly firing without stopping KB updates.

### DB Schema

- **D-16:** All Phase 365 DB changes are NEW tables only (no ALTER TABLE on existing tables):
  - `ai_behavior_samples` (D-02 above)
  - No other schema changes. KB is file-based (D-10).
  - Migration: new `CREATE TABLE IF NOT EXISTS` statement in the existing migration sequence in
    `crates/racecontrol/src/db/mod.rs` (follow pattern after `billing_accuracy_events`).

- **D-17:** Cloud sync: `ai_behavior_samples` is NOT included in the Phase 301 cloud sync payload.
  Rationale: AI behavior analytics are venue-specific (samples depend on local AC installation,
  difficulty tuning, and track conditions). Cloud doesn't need per-venue AI lap data.
  - _Rejected: Sync samples to cloud — would double the sync payload size and cloud has no use for
    venue-specific AI lap statistics._

### Claude's Discretion

- Exact car and track slug normalization (reuse existing slug generation in gsd-tools or implement
  a simple `to_ascii_lowercase().replace(' ', "-")` inline in Rust).
- Whether the MMA HTTP calls use `reqwest` (already a dep in racecontrol) or a new crate — use
  `reqwest` as it's already in scope per server_diagnostics.rs pattern.
- Exact OpenRouter model IDs and API payload format (research must confirm current IDs).
- Exact `ai_behavior_batch.rs` file location (alongside `session_audit.rs` in racecontrol/src/).
- Whether to re-use `spawn_data_retention_job`'s interval pattern verbatim or extract a
  `spawn_weekly_job(name, f)` helper — planner decides.
- Exact test strategy for the MMA batch (mock HTTP responses via `mockito` or `wiremock`).
- Whether `kb_batch_id` in `ai_behavior_samples` is a UUID or a timestamp string.

### Folded Todos

_No todos matched Phase 365 scope (gsd-tools reported todo_count=0)._

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Requirements & Scope
- `.planning/milestones/v46.0-REQUIREMENTS.md` §"Phase 365 — AI Behavior Validation via MMA (Phase E)" — authoritative REQ-IDs GLD-E-01..E-04 and silent-loss point mapping.
- `.planning/milestones/v46.0-ROADMAP.md` §"Phase 365" — success criteria and plan list.
- `.planning/ROADMAP.md` §"Phase 365: AI Behavior Validation via MMA" — main roadmap entry.

### Phase 363 Upstream (heuristic boundary + session_audit.rs)
- `.planning/phases/363-data-recording-verification/363-CONTEXT.md` D-01 — explicit note that Phase 365 is the refinement layer. Phase 363 uses a floor heuristic; Phase 365 provides AI-tier-aware data. They do NOT share code paths.
- `crates/racecontrol/src/session_audit.rs` — Phase 363's `expected_laps()` function. Phase 365 does NOT modify this. Read to understand the upstream audit boundary.

### Existing AI-Related Code
- `crates/rc-agent/src/ac_launcher.rs` — `DifficultyTier` enum + `tier_for_level(ai_level: u32)` (lines ~31-80). MUST reuse this type for keying `ai_behavior_samples.difficulty_tier`. Do not redefine.
- `crates/racecontrol/src/multiplayer.rs` lines 1082-1140 — difficulty_tier → ai_level mapping table. Shows how tiers map to AC AI_LEVEL values. Phase 365 collector must handle the same mapping in reverse (ai_level → tier).
- `crates/racecontrol/src/ac_server.rs` — `start_ac_server()` and `generate_extra_cfg_yml()`. Research must identify which endpoint/hook exposes per-AI-car lap data during a live AC session.
- `crates/racecontrol/src/ac_camera.rs` — already tracks `lap_time_ms` per car index. Shows SHM structure for multi-car lap tracking. Phase 365 can follow this pattern for AI lap collection.
- `crates/racecontrol/src/server_diagnostics.rs` lines 464-497 — `check_openrouter_key()` shows exact key location (`OPENROUTER_KEY` env OR `data/openrouter-mma-key.txt`). MMA batch must use same access pattern.

### DB Schema Context
- `crates/racecontrol/src/db/mod.rs` lines 88-127 — `sessions` and `laps` tables. New `ai_behavior_samples` table goes after `billing_accuracy_events` (line ~460+). Follow CREATE TABLE IF NOT EXISTS + index pattern.
- `crates/racecontrol/src/lap_tracker.rs` lines 239-270 — shows full laps INSERT (no is_ai column). Confirms AI laps are NOT in the laps table today.

### Scheduling Pattern
- `crates/racecontrol/src/api/routes.rs` function `spawn_data_retention_job` (line ~21539) — template for weekly background task pattern. Phase 365 batch follows the same `tokio::time::interval` approach.

### Protocol Types
- `crates/rc-common/src/protocol.rs` — WS event enum. `AiBehaviorAnomaly` variant must be added here. Follow existing event struct patterns.

### MMA Protocol Reference
- `.planning/specs/UNIFIED-MMA-PROTOCOL.md` — Full MMA Protocol v3.0. Phase 365 uses OpenRouter calls for analytics (NOT the Q1-Q4 incident gate). Read §"Part 1" to understand what Phase 365 is NOT doing, then design Phase 365's batch as a simpler "analytics consensus" pattern.
- `crates/racecontrol/src/server_diagnostics.rs` — Existing `reqwest` OpenRouter HTTP call pattern (reference, not reuse directly).

### KB Output Location
- `.planning/kb/ai-behavior/` — create this directory. TOML files written here by batch job.
- CLAUDE.md §"Deploy Manifest Protocol (DMP)" — Phase 365 PLAN MUST include deploy: section.
- CLAUDE.md §"Never hold a lock across .await" — batch job will make async HTTP calls; don't hold DB locks.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`DifficultyTier` enum in rc-agent/src/ac_launcher.rs** — 5 tiers (Rookie/Amateur/SemiPro/Pro/Alien),
  `tier_for_level(u32) -> Option<DifficultyTier>`, midpoints defined. Zero new enum needed.
- **`check_openrouter_key()` in server_diagnostics.rs** — exact pattern for reading the OpenRouter
  API key from env/file. MMA batch replicates this access pattern.
- **`spawn_data_retention_job()` in routes.rs** — template for weekly background task.
  `tokio::time::interval(86400s)` → Phase 365 uses `604800s` (7 days).
- **`reqwest` HTTP client** — already a dependency; no new crates for OpenRouter calls.
- **`ac_camera.rs` multi-car tracking** — already reads `lap_time_ms` per car via SHM. Shows the
  data is accessible per-car from the existing shared memory reader.
- **`session_audit.rs` from Phase 363** — companion module pattern. `ai_behavior_batch.rs` follows
  same structure: pure functions + async DB functions + feature flag guard.

### Established Patterns
- **New table per new data domain** — Phase 363 added `lap_rejections` as a new table rather than
  modifying `laps`. Phase 365 follows the same: `ai_behavior_samples` is standalone.
- **Feature flags for new runtime behaviors** — `phase363_session_audit` precedent. Phase 365 adds
  two flags: `phase365_mma_batch` and `phase365_anomaly_detection`.
- **TOML config files for game-specific data** — already used in `deploy/configs/rc-agent-pod{N}.toml`.
  KB TOML files follow same hand-editable + machine-written duality.
- **Background tasks spawned from main** — `spawn_data_retention_job`, `spawn_alert_checker`,
  `action_queue::spawn` all follow same pattern in `main.rs` startup sequence.

### Integration Points
- **New `ai_behavior_samples` table** — INSERT from a new `ai_behavior_collector` function called
  from the AC session event loop (server-side for group sessions; pod-side thin hook for solo sessions).
- **New `AiBehaviorAnomaly` variant in rc-common/protocol.rs** — WS broadcast to admin clients.
- **New `ai_behavior_batch.rs` module** — weekly tokio background task, OpenRouter HTTP calls,
  KB TOML writer. Spawned from `main.rs` alongside other background tasks.
- **AC server live API or SHM** — research must confirm the exact hook point for per-AI-car lap
  events in AC solo sessions (rc-agent side) vs AC server multiplayer sessions (racecontrol side).

</code_context>

<specifics>
## Specific Ideas

- **"Via MMA" is an analytics MMA, not incident-response MMA.** The phase name refers to using
  multi-model AI consensus for a periodic analytics task (determining expected tier bands) — NOT
  the Unified MMA Protocol's Q1-Q4 incident gate. The batch job is a domain-specific OpenRouter
  call, simpler and cheaper than a full 10-model convergence run.
- **DifficultyTier reuse is mandatory.** rc-agent already has the tier enum and `tier_for_level()`.
  racecontrol must reference this (or re-export from rc-common). The tier slug in `ai_behavior_samples`
  must match the exact strings used in multiplayer.rs ("rookie", "semi_pro", etc.).
- **AI lap collection is the hardest part.** The existing codebase records only human driver laps
  (lap_tracker.rs). Phase 365 requires determining where in the AC data flow AI car lap times are
  available and adding a minimal hook. The research agent must focus significant effort here.
- **Phase 363 boundary is preserved.** Phase 365's `ai_behavior_samples` data does NOT feed back
  into Phase 363's `expected_laps()` heuristic. They are parallel systems.
- **KB TOML is source-controlled.** Committing `.planning/kb/ai-behavior/*.toml` to git on the
  venue server is intentional — it gives history, diffs, and auditability for how AI bands change
  over time as more session data accumulates.

</specifics>

<deferred>
## Deferred Ideas

### Admin UI for AI Performance Trends (Phase 367 or later)
Phase 365 ROADMAP success criterion 4 says "Admin dashboard surfaces per-car-track AI performance
trend (shared UI with Phase 367)." Phase 365 provides the data endpoint; the frontend rendering is
Phase 367's scope. Deferred per hard phase boundary.

### Multi-Sim AI Lap Collection (iRacing, F1 25, LMU)
Phase 365 v1.0 targets AC only (only sim with DifficultyTier enum + tier_for_level() defined).
Other sims can be added in a follow-up when their AI level APIs are understood.

### Per-Driver AI Tier Calibration
Comparing a specific driver's laps against AI bands (e.g., "you lapped faster than Pro AI") is a
future analytics feature, not Phase 365 scope.

### Real-Time KB Updates (per-lap streaming consensus)
The MMA batch is weekly. Real-time KB updates after each session would require much more infrastructure
and cost. Deferred post-venue-open if needed.

### KB Serving via API Endpoint
Phase 365 writes TOML files. A `/api/v1/ai-behavior-kb/{car}/{track}` endpoint for external consumption
is not required by GLD-E-01..E-04. Defer to Phase 367 or a future analytics milestone.

### Reviewed Todos (not folded)
_No todos were reviewed — gsd-tools reported todo_count=0._

</deferred>

---

*Phase: 365-ai-behavior-validation-via-mma*
*Context gathered: 2026-04-10 (--auto mode)*
*Decisions: 17 | Canonical refs: 15 | Deferred items: 5*
