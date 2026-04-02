# Phase 304: Fleet Deploy Automation - Research

**Researched:** 2026-04-02
**Domain:** Rust/Axum deploy orchestration — extending existing OTA pipeline with a new API surface
**Confidence:** HIGH (all findings verified from codebase, no web search required)

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Extends existing OTA pipeline from v22.0 (ota_pipeline.rs) — no rewrite
- Uses existing fleet exec infrastructure for binary download + swap
- Canary = Pod 8 (standing rule)
- Billing drain uses existing has_active_billing_session() check
- OTA_DEPLOYING sentinel file protocol (standing rule)
- Previous binary preserved for 72h rollback window (standing rule)
- Deploy status stored in AppState (in-memory, not SQLite)

### Claude's Discretion
All implementation choices — pure infrastructure phase.

### Deferred Ideas (OUT OF SCOPE)
None listed.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| DEPLOY-01 | POST /api/v1/fleet/deploy endpoint accepts binary hash + target scope (all/canary/specific pods) | New route in staff_routes (superadmin tier), delegates to enhanced deploy_rolling() |
| DEPLOY-02 | Canary deploy to Pod 8 first, health verify before fleet rollout | deploy_rolling() already implements this; needs deploy_id + wave-gate logic |
| DEPLOY-03 | Auto-rollout to remaining pods after canary passes (configurable delay between waves) | deploy_rolling() does waves; needs configurable inter-wave delay + deploy_id tracking |
| DEPLOY-04 | Auto-rollback on failure — if canary or any wave fails health check, revert to previous binary | rollback_wave() + do-rollback.bat already exist; needs integration into new endpoint |
| DEPLOY-05 | GET /api/v1/fleet/deploy/status shows progress, wave status, rollback events | New endpoint over in-memory state (AppState field); extends pod_deploy_states |
| DEPLOY-06 | Active billing sessions drain before binary swap on each pod | check_and_trigger_pending_deploy() + WaitingSession already implemented; wire to new endpoint |
</phase_requirements>

---

## Summary

Phase 304 adds a single well-defined API surface (`POST /api/v1/fleet/deploy` + `GET /api/v1/fleet/deploy/status`) on top of a mature deploy infrastructure that already implements almost everything required. The existing `deploy_rolling()` in `deploy.rs` already handles canary-first ordering, billing session deferral, and retries. The existing `rollback_wave()` in `ota_pipeline.rs` handles the rollback script pattern. The missing pieces are: (1) a `deploy_id` to correlate status across requests, (2) wave-aware status tracking (current wave number, per-wave rollback events), (3) the two new API routes, and (4) a new `AppState` field for the fleet deploy session.

The critical structural insight is that `deploy_rolling()` runs synchronously in a `tokio::spawn`'d background task and returns a `Result<(), String>`. The new endpoint must return a `deploy_id` immediately (202 Accepted) while the background task updates in-memory state. The status endpoint reads that in-memory state. This is the same pattern already used by `deploy_rolling_handler()` at `POST /api/deploy/rolling` — Phase 304 extends it with scope scoping, deploy_id tracking, and a richer status response.

**Primary recommendation:** Add a `fleet_deploy_session: RwLock<Option<FleetDeploySession>>` field to `AppState`. Wire the new endpoint to call an enhanced version of `deploy_rolling()` that accepts binary hash + scope and writes wave progress into this field. The `GET /api/v1/fleet/deploy/status` handler reads from this field.

---

## Standard Stack

### Core (all already in Cargo.toml)
| Library | Purpose | How Used |
|---------|---------|----------|
| `axum` | HTTP routing | Route handlers, State extractor, Json |
| `tokio` | Async runtime | `tokio::spawn` for background deploy task |
| `serde` / `serde_json` | Serialization | Request/response JSON, status state |
| `chrono` | Timestamps | IST-aware deploy window check, rollback event timestamps |
| `reqwest` | HTTP client | `http_exec_on_pod()` — download + exec commands |
| `sha2` | SHA256 | `compute_sha256_file()` — binary verification already in ota_pipeline.rs |

**No new dependencies needed.** All required crates are already in workspace Cargo.toml.

---

## Architecture Patterns

### Existing Deploy Infrastructure (what Phase 304 builds on)

#### `crates/racecontrol/src/deploy.rs`
- `deploy_pod(state, pod_id, pod_ip, binary_url)` — deploys one pod via self-swap bat. Infallible (returns `()`). Global 300s timeout. Updates `pod_deploy_states` and broadcasts `DashboardEvent::DeployProgress`.
- `deploy_rolling(state, binary_url, force, actor)` — canary-first rolling deploy. Synchronous (await). Pod 8 first; if canary fails, returns `Err()` immediately; no other pods touched. Remaining pods respect billing session via `WaitingSession` state.
- `check_and_trigger_pending_deploy(state, pod_id)` — called from billing.rs when session ends; fires deferred deploy for pods that were in `WaitingSession`.
- `deploy_status(state)` — returns `HashMap<String, DeployState>` from `pod_deploy_states`.
- `is_deploy_window_locked(force, actor)` — weekend peak hour (18:00–22:59 IST Sat/Sun) gate.
- `exec_on_pod(state, pod_id, pod_ip, cmd, timeout_ms)` — HTTP first, WS fallback. Used throughout.

#### `crates/racecontrol/src/ota_pipeline.rs`
- `WAVE_1 = &[8]`, `WAVE_2 = &[1,2,3,4]`, `WAVE_3 = &[5,6,7]` — canonical wave layout.
- `rollback_wave(http_client, pod_ips, sentry_service_key)` — writes `do-rollback.bat` via rc-agent `/write`, executes via rc-sentry `:8091/exec` (NOT rc-agent — avoids killing exec endpoint). Waits 30s for completion.
- `has_active_billing_session(billing_session_id: &Option<String>) -> bool` — checks `is_some()`. NOTE: in `deploy_rolling()`, billing is checked via `state.billing.active_timers.read().await.contains_key(pod_id)` (not this helper).
- `set_ota_sentinel(http_client, pod_ips)` — writes `C:\RacingPoint\ota-in-progress.flag` via rc-agent `/write`.
- `clear_ota_sentinel(http_client, pod_ips)` — removes sentinel via rc-agent `/exec del`.
- `set_kill_switch(http_client, pod_ips, active)` — writes `sentry-flags.json` to suppress watchdog restarts during OTA.
- `health_check_pod(...)` — verifies ws_connected, http_reachable, binary_sha256, violation_count, scan_failure_count.
- `PipelineState` enum — `Idle / Building / Staging / Canary / StagedRollout / HealthChecking / Completed / RollingBack / Paused`.

#### Existing Route (superadmin tier)
```
POST /api/v1/deploy/rolling  → deploy_rolling_handler (routes.rs:17437)
GET  /api/v1/deploy/status   → deploy_status (routes.rs:17417)
POST /api/v1/ota/deploy      → ota_deploy_handler
GET  /api/v1/ota/status      → ota_status_handler
```
These routes are registered inside the superadmin tier (require_role_superadmin) at routes.rs:600–614.

#### Billing Session Check (actual pattern in deploy_rolling)
```rust
// From deploy.rs:928–946 — the REAL billing check:
let has_active_session = {
    let timers = state.billing.active_timers.read().await;
    timers.contains_key(pod_id)
};
```
`has_active_billing_session()` in ota_pipeline.rs takes `&Option<String>` — it's a helper for context where only the session_id is known. In deploy.rs the live check hits `billing.active_timers` directly.

### AppState Fields Relevant to This Phase
```rust
// From state.rs — existing fields Phase 304 reads/writes:
pub pod_deploy_states: RwLock<HashMap<String, DeployState>>  // per-pod state
pub pending_deploys: RwLock<HashMap<String, String>>         // pod_id -> binary_url (WaitingSession)
pub pod_fleet_health: RwLock<HashMap<String, FleetHealthStore>>  // for health verification
pub billing: BillingManager  // billing.active_timers for session check
```

New field to add:
```rust
// New: tracks the active fleet deploy session (deploy_id, waves, rollback events)
pub fleet_deploy_session: RwLock<Option<FleetDeploySession>>
```

### New Data Structures (to implement)

```rust
// Request body for POST /api/v1/fleet/deploy
#[derive(Deserialize)]
pub struct FleetDeployRequest {
    pub binary_hash: String,        // SHA256 of the binary
    pub binary_url: String,         // staging HTTP URL
    pub scope: DeployScope,         // "all" | "canary" | specific pods
    #[serde(default)]
    pub wave_delay_secs: Option<u64>, // configurable inter-wave delay (default: 5s)
    #[serde(default)]
    pub force: bool,                // override weekend peak hour lock
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum DeployScope {
    All,
    Canary,
    Pods(Vec<u32>),  // specific pod numbers
}

// In-memory session tracking (stored in AppState::fleet_deploy_session)
#[derive(Serialize, Clone)]
pub struct FleetDeploySession {
    pub deploy_id: String,          // UUID or timestamp-based unique ID
    pub binary_hash: String,
    pub binary_url: String,
    pub scope: DeployScope,
    pub wave_delay_secs: u64,
    pub initiated_by: String,       // claims.sub (staff username)
    pub initiated_at: String,       // RFC3339 IST
    pub current_wave: u8,           // 0=not started, 1=canary, 2=wave2, 3=wave3
    pub overall_status: DeployOverallStatus,  // pending/running/completed/failed/rolling_back
    pub waves: Vec<WaveStatus>,
    pub rollback_events: Vec<RollbackEvent>,
}

#[derive(Serialize, Clone)]
pub struct WaveStatus {
    pub wave_number: u8,
    pub pods: Vec<String>,          // e.g. ["pod_8"] or ["pod_1","pod_2","pod_3","pod_4"]
    pub status: WaveDeployStatus,   // pending/running/passed/failed
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub pod_results: Vec<PodDeployResult>,
}

#[derive(Serialize, Clone)]
pub struct PodDeployResult {
    pub pod_id: String,
    pub status: String,             // complete/failed/waiting_session/rolled_back
    pub detail: Option<String>,     // failure reason or rollback reason
}

#[derive(Serialize, Clone)]
pub struct RollbackEvent {
    pub wave: u8,
    pub pod_id: String,
    pub reason: String,
    pub rolled_back_at: String,     // RFC3339 IST
    pub outcome: String,            // "success" | "failed"
}
```

### API Routes to Add

Both in the superadmin tier (same router block as existing `/deploy/rolling`):
```
POST /api/v1/fleet/deploy          → fleet_deploy_handler
GET  /api/v1/fleet/deploy/status   → fleet_deploy_status_handler
```

**Route uniqueness:** These are new paths. `/fleet/deploy` does NOT conflict with existing `/deploy/rolling` or `/fleet/health`. Verify with the uniqueness test: `route_uniqueness_tests::no_duplicate_route_registrations` after adding.

### Deploy Flow (DEPLOY-01 through DEPLOY-06)

```
POST /api/v1/fleet/deploy
  → validate binary_url HEAD reachable
  → check is_deploy_window_locked
  → reject if fleet_deploy_session is Some and status=Running (409 Conflict)
  → generate deploy_id (uuid or format!("{}-{}", binary_hash[..8], unix_ts))
  → write FleetDeploySession { overall_status: Running, current_wave: 0 } to AppState
  → tokio::spawn(run_fleet_deploy(state, session))
  → return 202 { deploy_id, status: "started", canary: "pod_8" }

async fn run_fleet_deploy(state, session):
  → set_ota_sentinel + set_kill_switch on all target pods
  → Wave 1 (canary = pod_8):
      → update session.current_wave = 1, wave[0].status = Running
      → check billing: if active → mark WaitingSession, store pending, skip (DEPLOY-06)
      → deploy_pod(state, "pod_8", ip, binary_url).await
      → health_check_pod OR check pod_deploy_states == Complete
      → if FAIL: rollback_wave, update rollback_events, set overall_status=Failed, STOP
      → if PASS: wave[0].status = Passed, proceed
  → sleep wave_delay_secs
  → Wave 2 (pods 1-4):
      → update session.current_wave = 2, wave[1].status = Running
      → for each pod: billing check → deploy_pod (or WaitingSession)
      → health check per pod after deploy
      → if any FAIL: rollback that pod, record rollback_event (non-fatal, continue)
  → sleep wave_delay_secs
  → Wave 3 (pods 5-7): same pattern as Wave 2
  → clear_ota_sentinel + clear_kill_switch
  → set overall_status = Completed

GET /api/v1/fleet/deploy/status
  → read fleet_deploy_session from AppState
  → return JSON of FleetDeploySession (or { status: "idle" } if None)
```

### Rollback Behavior (DEPLOY-04)

The existing `rollback_wave()` in ota_pipeline.rs uses rc-sentry `:8091/exec` to run `do-rollback.bat`. This is the correct path — never use rc-agent for rollback (standing rule: "NEVER use taskkill /F /IM rc-agent.exe followed by start in the same exec chain"). Phase 304 calls this for:

- **Canary failure**: call `rollback_wave()` for `&[("pod_8", ip)]`. Halt entire deploy.
- **Wave 2/3 pod failure**: call `rollback_wave()` for the specific failed pod. Log to `rollback_events`. Continue to next pod (non-fatal for non-canary pods).

Rollback verification: the 30s sleep in `rollback_wave()` is followed by checking if `pod_deploy_states` returns to `Idle` or `Complete` (post-rollback the pod restarts, sends StartupReport, reconnects WS). A separate `verify_rollback()` helper can check `is_ws_connected()` + `is_process_alive()` with a short timeout.

### Key Implementation Caution: Lock Across Await

Standing rule: "Never hold a lock across `.await`". The session must be updated via the snapshot-clone pattern:
```rust
// CORRECT:
{
    let mut session_guard = state.fleet_deploy_session.write().await;
    if let Some(ref mut s) = *session_guard {
        s.current_wave = wave_num;
        s.waves[wave_idx].status = WaveDeployStatus::Running;
    }
} // guard dropped before any .await
deploy_pod(state.clone(), ...).await; // .await outside lock
```

### Billing Session Hook (DEPLOY-06)

`check_and_trigger_pending_deploy()` is already called from billing.rs when a session ends. The new fleet deploy must use the same `pending_deploys` map to register deferred pods — these will auto-fire when the session ends. This is transparent to the new endpoint — it just sets `WaitingSession` state and stores in `pending_deploys`, same as `deploy_rolling()` does today.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Binary download + exec on pod | Custom HTTP logic | `exec_on_pod()` in deploy.rs | Already handles HTTP→WS fallback |
| Binary swap sequence | New bat generation | `SWAP_SCRIPT_CONTENT` + `ROLLBACK_SCRIPT_CONTENT` in deploy.rs | MMA-audited, AV retry loop, correct kill sequence |
| Rollback execution | New script | `rollback_wave()` in ota_pipeline.rs | Uses rc-sentry (avoids killing exec endpoint) |
| OTA sentinel write/clear | New file writes | `set_ota_sentinel()` / `clear_ota_sentinel()` | Already handles pod_ips iteration |
| Watchdog kill switch | New signal mechanism | `set_kill_switch()` | Already writes sentry-flags.json |
| Deploy window check | New time logic | `is_deploy_window_locked()` in deploy.rs | IST-aware, force override tested |
| SHA256 verification | New hash code | `compute_sha256_file()` in ota_pipeline.rs | Streaming, no OOM on 15MB binary |
| Health check per pod | New probe logic | `health_check_pod()` in ota_pipeline.rs | WS + HTTP + SHA256 + error spike all in one |
| Billing session check | New billing query | `billing.active_timers.contains_key(pod_id)` | Direct map lookup, same as deploy_rolling() |

**Key insight:** The codebase has a complete deploy substrate. Phase 304 is a thin orchestration layer + API surface on top, not a new deploy engine.

---

## Common Pitfalls

### Pitfall 1: Duplicate Route Registration
**What goes wrong:** Adding `/fleet/deploy` when a route with the same METHOD+PATH exists elsewhere causes Axum to panic at startup (runtime-only, not compile-time).
**Why it happens:** routes.rs is 16K+ lines with multiple router blocks.
**How to avoid:** After adding routes, run `cargo test -p racecontrol-crate --lib -- route_uniqueness` which runs `no_duplicate_route_registrations`.
**Warning signs:** Server panics on startup with "route conflict" message.

### Pitfall 2: Executing Rollback via rc-agent (Wrong Port)
**What goes wrong:** Calling `rc-agent :8090/exec` to kill rc-agent kills the process handling the exec command — the subsequent restart command never runs.
**Why it happens:** rc-agent self-exec is self-destructive.
**How to avoid:** ALWAYS use rc-sentry `:8091/exec` for rollback (standing rule + `rollback_wave()` already does this).
**Warning signs:** Pod goes offline during rollback with no recovery.

### Pitfall 3: Lock Held Across `.await`
**What goes wrong:** Holding `fleet_deploy_session.write()` across a `deploy_pod(...).await` call causes deadlock — the WS handler needs `pod_deploy_states.write()` which is a different lock, but it also tries to send on `dashboard_tx` which may require calling code that reads `fleet_deploy_session`.
**Why it happens:** Async Rust doesn't enforce lock ordering at compile time.
**How to avoid:** Always clone/snapshot state, drop guard in `{}` block, then `.await`.

### Pitfall 4: double-trigger (Two Concurrent Fleet Deploys)
**What goes wrong:** Two POST /api/v1/fleet/deploy calls race; both check `fleet_deploy_session.is_none()` before either writes.
**Why it happens:** Async read-check-write is not atomic.
**How to avoid:** Use `fleet_deploy_session.write().await` for the entire check-and-set, not two separate lock acquisitions. Return 409 if already Some and status=Running.

### Pitfall 5: Canary State Check After async deploy_pod
**What goes wrong:** `deploy_pod()` is infallible (returns `()`). After it completes, the only way to know success vs failure is to read `pod_deploy_states`. But states may reset to `Idle` after a brief delay (deploy.rs resets state after success). Read the state immediately after `deploy_pod()` returns.
**Why it happens:** The existing `deploy_rolling_handler()` has the same issue (routes.rs:906–923). The state is `Complete` immediately after, then `Idle` after ~10s.
**How to avoid:** Read `pod_deploy_states` synchronously after `deploy_pod()` returns, before any sleep.

### Pitfall 6: weekend lock in Integration Tests
**What goes wrong:** Tests that call `deploy_rolling()` without `force=true` may fail on Saturdays/Sundays between 18:00–22:59 IST.
**Why it happens:** `is_deploy_window_locked()` checks real system time.
**How to avoid:** Pass `force: true` in test scenarios, or pass an explicit `force` flag in the `FleetDeployRequest`.

---

## Code Examples

### Pattern: 202 Accepted + Background Spawn (from existing deploy_rolling_handler)
```rust
// Source: crates/racecontrol/src/api/routes.rs:17437
async fn deploy_rolling_handler(...) -> (StatusCode, Json<Value>) {
    // guard: check active deploys
    // ...
    let state_clone = Arc::clone(&state);
    tokio::spawn(async move {
        if let Err(e) = crate::deploy::deploy_rolling(state_clone, binary_url, force, &actor).await {
            tracing::error!("Rolling deploy failed: {}", e);
        }
    });
    (StatusCode::ACCEPTED, Json(json!({ "status": "rolling_deploy_started" })))
}
```

### Pattern: Billing Session Check (from deploy_rolling)
```rust
// Source: crates/racecontrol/src/deploy.rs:928
let has_active_session = {
    let timers = state.billing.active_timers.read().await;
    timers.contains_key(pod_id)
};
if has_active_session {
    set_deploy_state(&state, pod_id, DeployState::WaitingSession).await;
    let mut pending = state.pending_deploys.write().await;
    pending.insert(pod_id.clone(), binary_url.clone());
    continue;
}
```

### Pattern: Rollback via rc-sentry (from rollback_wave)
```rust
// Source: crates/racecontrol/src/ota_pipeline.rs:490
// Step 1: write rollback script via rc-agent /write (agent still alive)
let write_url = format!("http://{ip}:8090/write");
// Step 2: execute via rc-SENTRY :8091/exec (NOT rc-agent :8090)
let exec_url = format!("http://{ip}:8091/exec");
req.json(&json!({ "cmd": r#"start /min cmd /c C:\RacingPoint\do-rollback.bat"# }))
```

### Pattern: Health Check After Deploy
```rust
// Source: crates/racecontrol/src/ota_pipeline.rs:353
pub fn health_check_pod(
    _pod_id: &str,
    ws_connected: bool,
    http_reachable: bool,
    binary_sha256: Option<&str>,
    expected_sha256: &str,
    violation_count_24h: u32,
    scan_failure_count: u32,
) -> Result<(), String>
// For Phase 304: sha256 check uses binary_hash from the request, not manifest.
// Read pod_fleet_health store for ws_connected, http_reachable after deploy_pod completes.
```

### Pattern: State Update Without Lock Across Await
```rust
// Correct pattern for updating fleet_deploy_session:
{
    let mut lock = state.fleet_deploy_session.write().await;
    if let Some(ref mut sess) = *lock {
        sess.current_wave = 1;
        sess.overall_status = DeployOverallStatus::Running;
    }
} // lock dropped here
deploy_pod(state.clone(), pod_id, pod_ip, binary_url).await; // safe
```

---

## Environment Availability

Step 2.6: SKIPPED — Phase 304 is a code-only change. All infrastructure (rc-agent HTTP /exec on :8090, rc-sentry HTTP on :8091, pod LAN IPs, staging HTTP server on :18889) is pre-existing operational infrastructure. No new tools or services are required.

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in test + custom helpers |
| Config file | Cargo.toml workspace |
| Quick run command | `cargo test -p racecontrol-crate --lib -- deploy` |
| Full suite command | `cargo test -p racecontrol-crate --lib` |

**Current baseline:** 781 tests passing (racecontrol-crate). 28 tests in deploy.rs, 36 tests in ota_pipeline.rs.

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| DEPLOY-01 | POST /fleet/deploy returns 202 with deploy_id | unit | `cargo test -p racecontrol-crate --lib -- fleet_deploy::tests` | ❌ Wave 0 |
| DEPLOY-01 | Scope=canary only deploys pod_8 | unit | same | ❌ Wave 0 |
| DEPLOY-01 | Binary hash passed through to deploy | unit | same | ❌ Wave 0 |
| DEPLOY-02 | deploy_id generation is unique per call | unit | same | ❌ Wave 0 |
| DEPLOY-02 | Canary failure halts before wave 2 | unit (mock) | same | ❌ Wave 0 |
| DEPLOY-03 | Wave delay is configurable (default 5s, override accepted) | unit | same | ❌ Wave 0 |
| DEPLOY-04 | Rollback event logged when wave pod fails health | unit | same | ❌ Wave 0 |
| DEPLOY-04 | Canary rollback sets overall_status=Failed | unit | same | ❌ Wave 0 |
| DEPLOY-05 | GET /fleet/deploy/status returns idle when no deploy | unit | same | ❌ Wave 0 |
| DEPLOY-05 | Status shows current_wave and per-pod results | unit | same | ❌ Wave 0 |
| DEPLOY-05 | Rollback events appear in status response | unit | same | ❌ Wave 0 |
| DEPLOY-06 | Pod with active session gets WaitingSession, not deployed | unit | same | ❌ Wave 0 |
| DEPLOY-06 | deploy_id in scope=all round-trips through status | unit | same | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p racecontrol-crate --lib -- fleet_deploy deploy ota_pipeline`
- **Per wave merge:** `cargo test -p racecontrol-crate --lib`
- **Phase gate:** Full suite green (781+ tests) before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/racecontrol/src/fleet_deploy.rs` — new module with `FleetDeploySession`, `run_fleet_deploy()`, route handlers, and unit tests
  - Tests can mock `deploy_pod()` behavior by pre-populating `pod_deploy_states` in AppState test helpers
  - Use `state::tests::create_initial_deploy_states_has_8_entries_all_idle` pattern for test setup

*(No new test config or framework install needed — follows existing `#[cfg(test)]` module pattern)*

---

## Open Questions

1. **Does Phase 304 replace or supplement the existing `/api/deploy/rolling`?**
   - What we know: `/api/deploy/rolling` exists and is used. Phase 304 adds `/api/v1/fleet/deploy`.
   - What's unclear: Should the old endpoint be deprecated? Are any existing callers (scripts, admin UI)?
   - Recommendation: Keep both. New endpoint is additive. Old endpoint can be deprecated separately.

2. **Binary hash verification at the endpoint — validate against staging server or trust caller?**
   - What we know: `compute_sha256_file()` exists in ota_pipeline.rs and is used in manifest-based OTA.
   - What's unclear: Does the caller provide `binary_hash` as a checksum to verify after download, or as a lookup key?
   - Recommendation: Treat `binary_hash` as the expected SHA256. After download on each pod, verify with `dir` + size check (existing pattern in deploy.rs). Full SHA256 re-computation on server-side is optional.

3. **Should `fleet_deploy_session` persist across server restarts?**
   - What we know: CONTEXT.md says "stored in AppState (in-memory, not SQLite)".
   - Decision locked: in-memory only. A restarted server loses deploy session state. This is acceptable for Phase 304.

---

## Sources

### Primary (HIGH confidence — all verified from codebase)
- `crates/racecontrol/src/deploy.rs` — deploy_pod, deploy_rolling, exec_on_pod, billing session check, WaitingSession pattern, deploy window lock
- `crates/racecontrol/src/ota_pipeline.rs` — wave constants, rollback_wave, OTA sentinel, kill switch, health_check_pod, has_active_billing_session
- `crates/racecontrol/src/state.rs:127` — AppState struct, pod_deploy_states, pending_deploys, fleet_health fields
- `crates/racecontrol/src/api/routes.rs:600–619` — superadmin route tier, existing deploy route registrations
- `crates/racecontrol/src/api/routes.rs:17417–17486` — deploy_status handler, deploy_rolling_handler (reference patterns)
- `crates/racecontrol/src/fleet_health.rs` — FleetHealthStore, PodFleetStatus shapes
- `crates/rc-common/src/types.rs:774` — DeployState enum, WaitingSession variant, is_active()
- `CLAUDE.md` — OTA sentinel protocol, rollback window, billing drain, deploy standing rules

### Secondary (MEDIUM confidence)
- `crates/racecontrol/src/deploy_awareness.rs` — FleetDeployStatus, DeployManifest shapes (reference for status response design)

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all libraries in Cargo.toml, no new deps
- Architecture: HIGH — all key functions verified by reading actual source code
- Pitfalls: HIGH — all sourced from standing rules and code inspection, not training data
- Test patterns: HIGH — existing test module structure confirmed with cargo test run

**Research date:** 2026-04-02
**Valid until:** Stable — Rust codebase, not subject to fast ecosystem churn. Valid until next major codebase refactor (60+ days).

## Project Constraints (from CLAUDE.md)

The following directives from CLAUDE.md apply to Phase 304 implementation:

| Constraint | Source | Impact |
|------------|--------|--------|
| No `.unwrap()` in production Rust — use `?`, `.ok()`, or match | CLAUDE.md | All new Rust code |
| Never hold lock across `.await` — snapshot and drop before async | CLAUDE.md | `fleet_deploy_session` update pattern |
| Route uniqueness: move = delete old in same commit; run uniqueness test | CLAUDE.md | After adding `/fleet/deploy` routes |
| ROLLBACK via rc-sentry `:8091/exec`, NEVER rc-agent for self-kill | CLAUDE.md | rollback_wave() integration |
| OTA sentinel protocol: write at start, clear on complete or rollback | CLAUDE.md | set_ota_sentinel / clear_ota_sentinel |
| Previous binary preserved 72h minimum (rc-agent-prev.exe, not deleted) | CLAUDE.md | deploy_pod already does this; don't add cleanup |
| Billing sessions drain before binary swap — existing has_active_billing_session | CLAUDE.md | WaitingSession + pending_deploys pattern |
| Deploy window lock: no deploys Sat/Sun 18:00–22:59 IST unless force=true | CLAUDE.md | is_deploy_window_locked() in handler |
| touch build.rs before release builds after new commits | CLAUDE.md | Build step, not code |
| LOGBOOK.md entry on every commit | CLAUDE.md | Post-deploy |
| Auto-push + notify after commit | CLAUDE.md | Post-deploy |
| Canary = Pod 8 always | CLAUDE.md | WAVE_1 = &[8] constant |
| Config push NEVER goes through fleet exec endpoint | CLAUDE.md | N/A — this phase is binary deploy only |
| Single-binary-tier policy: all pods run same binary | CLAUDE.md | scope=all deploys identical binary to all pods |
