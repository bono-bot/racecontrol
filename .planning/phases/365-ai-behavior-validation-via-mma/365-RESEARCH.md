# Phase 365: AI Behavior Validation via MMA - Research

## RESEARCH COMPLETE

**Phase:** 365 — AI Behavior Validation via MMA
**Requirements:** GLD-E-01, GLD-E-02, GLD-E-03, GLD-E-04
**Research date:** 2026-04-10

---

## 1. AI Lap Data Source (GLD-E-01)

### Finding: AC Result Files Are the Primary Source for Multiplayer Sessions

`crates/racecontrol/src/ac_server.rs` — `parse_ac_results()` at line 1448 already reads JSON files
from `{server_dir}/results/` directory. The `AcResultFile` structure contains ALL cars — human AND
AI — in the `Result` array. Key fields:

- `DriverName: String` — AI cars get auto-generated names (from `rc_common::ai_names::pick_ai_names()`)
- `DriverGuid: String` — Human cars have a GUID; AI cars have `""` (empty string). This is the
  discriminator: **`entry.driver_guid.is_empty()` = AI car**.
- `BestLap: i64` — best lap time in milliseconds
- `CarModel: String` — car model identifier
- `LapCount: u32` — number of completed laps

The `AcResultEntry` struct is at lines 1428-1445. It already has `best_lap` and `lap_count`.

**For GLD-E-01:** After session end (when `collect_and_persist_ac_results()` is called in
`stop_ac_server()`), filter `AcResultEntry` rows where `driver_guid.is_empty()`, verify
`lap_count >= 3`, and compute median best_lap from AI entries to store in `ai_behavior_samples`.

### Finding: Solo Session AI Lap Data Requires SHM Extension

For solo (single-pod) sessions, AI cars run in the AC game process. The AC SharedMemory layout
exposes multi-car data but `rc-agent/src/sims/assetto_corsa.rs` currently only reads index 0
(the player's car) for lap completion events. Specifically:
- `num_cars` is read at offset 64 of the statics SHM page (line 892)
- Physics and graphics SHM have per-car arrays but the current adapter only reads index 0

**Research conclusion:** For Phase 365 v1.0, use **AC result files as the data source** (server-side,
end-of-session). This applies to BOTH group sessions (ac_server.rs) and solo sessions IF AC writes
a results file on solo session end. Phase 365 does NOT require modifying rc-agent SHM code.

**Risk:** Solo AC sessions may not write result files to the same `results/` path. This needs
verification during implementation. If solo sessions don't produce result files, the collector falls
back to reading from the ac_camera.rs per-car tracking that exists in the AC server context only.
Research recommendation: start with group sessions (confirmed data source) and add solo session
support if result files are confirmed available.

---

## 2. AI Difficulty Tier (GLD-E-01 key dimension)

### Finding: `DifficultyTier` enum already defined in rc-agent

`crates/rc-agent/src/ac_launcher.rs` lines 31-80:
- `DifficultyTier` enum: Rookie(70-79), Amateur(80-84), SemiPro(85-89), Pro(90-95), Alien(96-100)
- `tier_for_level(ai_level: u32) -> Option<DifficultyTier>` — reverse-maps raw AI_LEVEL to tier
- `.midpoint()` — returns representative level per tier
- `.name()` — returns "Rookie", "Amateur", "Semi-Pro", "Pro", "Alien"

**For GLD-E-01 keying:** The `ai_level` is stored in the session's `config_json` (set when
`start_ac_server()` or `multiplayer.rs` launches the session). Extract `ai_level` from
`sessions.config_json` at sample collection time, then call `tier_for_level()` to derive the tier.

**Note:** `DifficultyTier` is defined in `rc-agent` crate. `racecontrol` crate needs to re-derive
or share it. Recommended: move `DifficultyTier` to `rc-common` crate (where protocol.rs lives) to
avoid duplication. Alternatively, define a parallel `AiDifficultyTier` in racecontrol's domain and
keep them in sync via a conversion.

---

## 3. WS Event Architecture (GLD-E-04)

### Finding: `DashboardEvent` enum is the correct target, not `AgentMessage`

The `AgentMessage` enum in `rc-common/src/protocol.rs` (line 87) is for pod-to-server messages.
`DashboardEvent` enum (line 1088) is for server-to-dashboard WebSocket broadcasts. Existing
examples:
- `DashboardEvent::AcServerUpdate(AcServerInfo)` — AC server state
- `DashboardEvent::LapCompleted(LapData)` — new human lap
- `DashboardEvent::BillingTick(...)` — billing timer

**`AiBehaviorAnomaly` must be added to `DashboardEvent`**, NOT to `AgentMessage`. Pattern:
```rust
AiBehaviorAnomaly {
    pod_id: String,
    session_id: String,
    car: String,
    track: String,
    difficulty_tier: String,
    expected_p10_ms: i64,
    expected_p90_ms: i64,
    observed_laps: Vec<i64>,
    direction: String,   // "too_slow" | "too_fast" | "unknown"
    timestamp: String,   // RFC3339
},
```

Broadcast via `state.dashboard_tx.send(DashboardEvent::AiBehaviorAnomaly { ... })`.

---

## 4. Background Task Pattern (GLD-E-02 scheduling)

### Finding: `spawn_data_retention_job` is the canonical template

`crates/racecontrol/src/api/routes.rs` line 21539:
```rust
pub async fn spawn_data_retention_job(state: Arc<AppState>) {
    tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;  // 1h initial delay
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(86400)); // daily
    loop {
        interval.tick().await;
        run_pii_anonymization_cycle(state.clone()).await;
    }
}
```

Spawned from `main.rs` line 951: `api::routes::spawn_data_retention_job(retention_state).await`

**For Phase 365:** New `pub async fn spawn_ai_behavior_batch(state: Arc<AppState>)` in a new
`crates/racecontrol/src/ai_behavior_batch.rs` module. Interval: `604800s` (7 days).
Add spawn call to `main.rs` alongside other background tasks.

---

## 5. OpenRouter API Access (GLD-E-02)

### Finding: Key access pattern established in server_diagnostics.rs

`crates/racecontrol/src/server_diagnostics.rs` lines 464-497:
```rust
let key = match std::env::var("OPENROUTER_KEY") {
    Ok(k) if !k.is_empty() => k,
    _ => {
        let saved = std::path::Path::new("data/openrouter-mma-key.txt");
        // fallback to reading file
    }
};
client.get("https://openrouter.ai/api/v1/auth/key").bearer_auth(&key)...
```

**For GLD-E-02 MMA batch:** Replicate this key-access pattern. Use `reqwest::Client` (already a
dep). API endpoint for chat completions: `https://openrouter.ai/api/v1/chat/completions` with
`Authorization: Bearer {key}`.

**Models to use (3/5 consensus from CONTEXT.md D-07):**
- `anthropic/claude-3.5-sonnet`
- `openai/gpt-4o`
- `google/gemini-1.5-pro`
- `mistralai/mistral-large`
- `deepseek/deepseek-chat` (DeepSeek V3)

Cost guard: each batch call is 1 completion per model × 5 models × N (car, track, tier) tuples.
With the minimum 10-sample threshold (CONTEXT.md D-08), this only runs when data exists. Add a
`MAX_TUPLES_PER_BATCH=20` limit to cap cost per weekly run.

---

## 6. KB File Format (GLD-E-03)

### Finding: `.planning/kb/` directory does not exist yet

`ls .planning/` output: no `kb/` directory. Phase 365 must create `.planning/kb/ai-behavior/` on
first batch run.

**TOML file writing:** Use `std::fs::write()` with a formatted TOML string (no new crate needed —
toml files for this are simple enough to generate with format strings). No need to add `toml` crate
for writing; reading later can use the `toml` crate that may already be a dep.

```bash
grep "toml" crates/racecontrol/Cargo.toml | head -5
```

Check whether `toml` crate is already available as a dependency.

---

## 7. DB Schema Integration (GLD-E-01 storage)

### Finding: New table goes in `db/mod.rs` after billing_accuracy_events

`crates/racecontrol/src/db/mod.rs` line ~460+ is where `billing_accuracy_events` ends. Phase 365
adds `ai_behavior_samples` table as a new `CREATE TABLE IF NOT EXISTS` block. Indices needed:
- `idx_ai_behavior_samples_combo ON ai_behavior_samples(car, track, difficulty_tier)` — batch query
- `idx_ai_behavior_samples_sampled ON ai_behavior_samples(sampled_at)` — recency filter

### Finding: No cloud sync needed (CONTEXT.md D-17 confirmed)

`cloud_sync.rs` does NOT need updating for this phase. AI samples are venue-specific.

---

## 8. Live Anomaly Detector Integration Point (GLD-E-04)

### Finding: Anomaly detection runs as a session-end hook, not a live hook

The live anomaly detection requirement ("fire on >3 consecutive laps") needs a point where per-lap
AI data is available. For group sessions, AC result files are written at session end. For a "live"
check, we'd need the AC HTTP API (`GET http://127.0.0.1:{http_port}/BEST_LAPS` or similar).

**Research conclusion:** For Phase 365 v1.0, implement the anomaly check at session end (same hook
that calls `parse_ac_results()`). Check if AI laps deviate from KB band. This is technically
"post-session" but satisfies GLD-E-04's spirit — the `AiBehaviorAnomaly` event fires before the
session is archived.

True live (mid-session) detection requires polling the AC server API during the session; mark as
follow-up in deferred. The session-end approach fires reliably and avoids polling overhead.

---

## 9. Feature Flags

### Finding: Feature flags stored in `feature_flags` table, read from AppState

Existing pattern in `session_audit.rs` (Phase 363): `phase363_session_audit` flag in `feature_flags`
table. Phase 365 adds:
- `phase365_mma_batch` (default: true)
- `phase365_anomaly_detection` (default: true)

Read via `state.get_feature_flag("phase365_mma_batch").await` (or equivalent existing accessor).

---

## 10. Validation Architecture

### Key verification points for Nyquist:

1. **DB insert** — `ai_behavior_samples` row count increases after a >3-lap AI session ends
2. **KB file write** — `.planning/kb/ai-behavior/{car}-{track}.toml` exists after first batch run
3. **WS event fire** — `AiBehaviorAnomaly` DashboardEvent serializes correctly (unit test)
4. **Anomaly check** — median AI lap 20% outside KB band triggers the event
5. **Feature flag kill-switch** — disabling `phase365_mma_batch` stops KB updates
6. **3/5 consensus** — only 3+ model agreement updates KB (unit test with mocked responses)

---

## Validation Architecture

The following test types are recommended for this phase:

### Unit Tests (no DB, no network)
- `ai_behavior_samples` schema: CREATE TABLE IF NOT EXISTS executes without error
- `tier_for_level()` mapping: test all 5 tiers map correctly
- KB TOML serialization: verify written TOML matches expected format
- Consensus logic: test 3/5 agree → update, 2/5 agree → no update
- Anomaly check: median outside p10-p90 band → direction "too_slow"/"too_fast"

### Integration Tests (with test DB)
- Collector: insert a test session with AI cars in results, verify `ai_behavior_samples` row inserted
- KB path: mock OpenRouter responses (3 agree), verify TOML file written
- Feature flag: disable `phase365_anomaly_detection`, verify event not fired

### Excluded (too expensive/complex for Phase 365)
- Live OpenRouter API calls in tests (cost + flakiness)
- Real AC session end-to-end test (requires running AC server)
