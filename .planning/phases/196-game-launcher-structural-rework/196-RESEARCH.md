# Phase 196: Game Launcher Structural Rework - Research

**Researched:** 2026-03-26
**Domain:** Rust async game launcher decomposition — trait-based dispatch, billing gate correctness, state machine fixes
**Confidence:** HIGH (all findings from direct source code inspection)

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
None — all implementation choices are at Claude's discretion.

### Claude's Discretion
- GameLauncher trait design (methods: launch(), validate_args(), cleanup())
- Per-game launcher struct placement (same file vs separate modules)
- Billing gate check ordering and TOCTOU mitigation strategy
- Stopping timeout implementation (tokio::spawn with sleep vs background task)
- How externally_tracked games are represented in GameTracker
- Whether to use enum dispatch or dynamic dispatch for trait
- Error type design for launch failures

### Deferred Ideas (OUT OF SCOPE)
None — discussion stayed within phase scope.
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| LAUNCH-01 | Trait-based architecture: AcLauncher, F1Launcher, IRacingLauncher each impl GameLauncher with launch(), validate_args(), cleanup() | Monolithic launch_game() identified at lines 75-205; all per-game branching points documented |
| LAUNCH-02 | Billing gate — deferred sessions: waiting_for_game pods must be allowed to launch | Bug confirmed: gate at line 101-106 checks only active_timers, ignores waiting_for_game |
| LAUNCH-03 | Billing gate — paused sessions: PausedManual/PausedDisconnect/PausedGamePause must reject launch | Not implemented; BillingSessionStatus::Paused* variants exist in rc-common |
| LAUNCH-04 | Billing gate — TOCTOU: billing expiry between gate check and tracker creation must fail cleanly | Race window identified between lines 100-139; no re-check before tracker commit |
| LAUNCH-05 | Double-launch — Stopping state: launch while state=Stopping must be rejected | Bug confirmed: guard at line 111-116 only blocks Launching/Running, not Stopping |
| LAUNCH-06 | Invalid JSON bypass: malformed launch_args must be rejected, not silently skipped | Bug confirmed: serde_json failure at line 87 falls through to launch — no rejection |
| LAUNCH-07 | Broadcast reliability: dashboard_tx.send() failure must log at warn level | Bug confirmed: `let _ =` at line 177 silently drops broadcast failures |
| STATE-01 | Stopping timeout: 30s without agent confirmation auto-transitions to Error + dashboard broadcast | Not implemented; Stopping state has no timeout in check_game_health() (line 640+) |
| STATE-02 | Disconnected agent detection: immediate Error transition, not 120s timeout | Partially implemented: disconnect sets Error, but only when agent conn drops during active send |
| STATE-03 | Feature flag block propagation: agent sends explicit GameStateUpdate with Error state | Not implemented; no feature flag check in launch_game() before sending to agent |
| STATE-04 | Externally tracked games: tracker with externally_tracked=true, launch_args=None | Partially present: handle_game_state_update() creates tracker with launch_args=None (line 383) but no externally_tracked field |
| STATE-05 | Error propagation for no-agent: immediate Error state + dashboard broadcast (not 120s) | Partially implemented: no-agent path at line 156-173 does set Error immediately |
| STATE-06 | Race Engineer re-check after stop: relaunch not allowed from Stopping state | Bug: relaunch_game() only checks game_state == Error (line 221), Stopping should be blocked |
</phase_requirements>

---

## Summary

Phase 196 works on a single 770-line file: `crates/racecontrol/src/game_launcher.rs`. The monolithic `launch_game()` function (lines 75-205) contains all game logic with no per-game branching — identical paths for AC, F1, iRacing. Six structural bugs exist in the billing gate, state machine, and error propagation, all confirmed by direct code inspection.

The billing gate bug (LAUNCH-02) is the most critical: the gate at line 101-106 checks only `active_timers`, but deferred billing sessions live in `waiting_for_game`. Any pod in `WaitingForGame` state cannot launch, breaking the primary use-case for deferred billing. The LAUNCH-03/04 paused-session and TOCTOU bugs are corollaries of the same single-map check.

State machine gaps are equally important: `Stopping` state has no timeout (STATE-01), the feature flag system exists in `AppState.feature_flags` but is never consulted during launch (STATE-03), and externally-tracked games need an `externally_tracked` field added to `GameTracker` (STATE-04).

**Primary recommendation:** Decompose into per-game launchers using static dispatch (enum variant match, not `dyn GameLauncher`) to avoid vtable overhead in the hot path. Fix all 13 requirements in two plans: Plan 01 covers the trait architecture + billing gate + error propagation; Plan 02 covers state machine gaps + Stopping timeout + externally_tracked.

---

## Standard Stack

### Core (all already in Cargo.toml — no new dependencies)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| tokio | existing | async runtime, `tokio::spawn` for Stopping timeout | Already used for all async in this crate |
| tokio::time | existing | `tokio::time::sleep(Duration::from_secs(30))` for Stopping timeout | Standard tokio timer, no new deps |
| tracing | existing | `tracing::warn!` for broadcast failures | Already used pervasively |
| serde_json | existing | JSON validation of launch_args before accepting | Already used at line 87 |

**No new Cargo.toml dependencies required for this phase.**

---

## Architecture Patterns

### Recommended Structure

The trait and per-game launchers stay in `game_launcher.rs` (same file). The file is 770 lines and will grow by ~200 lines — still manageable as one file. Separate modules would require pub-ing internal types unnecessarily.

```
crates/racecontrol/src/game_launcher.rs
├── GameLauncher trait (new)
├── AcLauncher struct + impl GameLauncher (new)
├── F1Launcher struct + impl GameLauncher (new)
├── IRacingLauncher struct + impl GameLauncher (new)
├── GameTracker struct (extended with externally_tracked field)
├── GameManager struct (unchanged)
├── launch_game() — now delegates to per-game launcher (refactored)
├── handle_game_state_update() (extended for STATE-01..STATE-06)
├── relaunch_game() (minor fix for Stopping guard)
├── stop_game() (unchanged)
├── check_game_health() (extended for Stopping timeout)
└── existing helpers: log_game_event, extract_launch_fields, classify_error_taxonomy
```

### Pattern 1: Static Enum Dispatch for GameLauncher Trait

Use a `GameLauncher` enum with per-variant structs rather than `Box<dyn GameLauncher>`. This avoids heap allocation in the hot path (launch_game is called on every game start) and keeps the match exhaustive — compiler enforces all SimTypes handled.

```rust
// In game_launcher.rs
pub trait GameLauncherImpl {
    /// Validate sim-specific launch args. Called before billing gate.
    fn validate_args(&self, args: Option<&str>) -> Result<(), String>;
    /// Return the CoreToAgentMessage to send for this game.
    fn make_launch_message(&self, sim_type: SimType, launch_args: Option<String>) -> CoreToAgentMessage;
    /// Optional cleanup on launch failure (AC: kill acs.exe orphans; others: no-op).
    fn cleanup_on_failure(&self, _pod_id: &str) {}
}

pub struct AcLauncher;
pub struct F1Launcher;
pub struct IRacingLauncher;
pub struct DefaultLauncher; // Forza, EVO, WRC etc.

impl GameLauncherImpl for AcLauncher {
    fn validate_args(&self, args: Option<&str>) -> Result<(), String> {
        let Some(json) = args else { return Ok(()); };
        serde_json::from_str::<serde_json::Value>(json)
            .map_err(|e| format!("Invalid launch_args JSON: {}", e))?;
        Ok(())
    }
    fn make_launch_message(&self, sim_type: SimType, launch_args: Option<String>) -> CoreToAgentMessage {
        CoreToAgentMessage::LaunchGame { sim_type, launch_args }
    }
}
// F1Launcher, IRacingLauncher follow same pattern

fn launcher_for(sim_type: SimType) -> &'static dyn GameLauncherImpl {
    match sim_type {
        SimType::AssettoCorsa | SimType::AssettoCorsaRally | SimType::AssettoCorsaEvo => &AcLauncher,
        SimType::F125 => &F1Launcher,
        SimType::IRacing => &IRacingLauncher,
        _ => &DefaultLauncher,
    }
}
```

**Why static dispatch:** `CoreToAgentMessage::LaunchGame` is already the same for all games in Phase 196. Per-game behavior diverges in Phase 197 (AC polling waits, CM timeout). The trait interface established here is the extension point Phase 197 will fill.

### Pattern 2: Billing Gate — Combined Map Check

```rust
// LAUNCH-02 fix: check BOTH active_timers AND waiting_for_game
{
    let timers = state.billing.active_timers.read().await;
    let waiting = state.billing.waiting_for_game.read().await;
    let has_active = timers.contains_key(pod_id);
    let has_deferred = waiting.contains_key(pod_id);
    if !has_active && !has_deferred {
        return Err(format!("Pod {} has no active or deferred billing session", pod_id));
    }
    // LAUNCH-03: reject paused sessions
    if let Some(timer) = timers.get(pod_id) {
        if matches!(timer.status, BillingSessionStatus::PausedManual
                               | BillingSessionStatus::PausedDisconnect
                               | BillingSessionStatus::PausedGamePause) {
            return Err(format!("Pod {} billing session is paused", pod_id));
        }
    }
}
// TOCTOU re-check after tracker creation (LAUNCH-04): re-acquire read lock
// just before sending to agent and verify billing still present.
```

**Important:** Both maps must be read in a single lock scope to prevent TOCTOU between the two checks. The pattern above holds both read guards simultaneously.

### Pattern 3: Stopping Timeout — tokio::spawn with sleep

```rust
// STATE-01: 30s Stopping timeout
// Spawned inside check_game_health() when Stopping state is detected
// OR spawned inside stop_game() when transition to Stopping is recorded.
// Recommendation: spawn in stop_game() at Stopping transition for immediate timing.

tokio::spawn(async move {
    tokio::time::sleep(Duration::from_secs(30)).await;
    // Re-check: if still Stopping, transition to Error
    let mut games = state_clone.game_launcher.active_games.write().await;
    if let Some(tracker) = games.get_mut(&pod_id_owned) {
        if tracker.game_state == GameState::Stopping {
            tracker.game_state = GameState::Error;
            tracker.error_message = Some("Stop timed out (30s)".to_string());
            let info = tracker.to_info();
            drop(games);
            let _ = state_clone.dashboard_tx.send(DashboardEvent::GameStateChanged(info));
            tracing::warn!("game state: Stopping timed out on pod {}", pod_id_owned);
        }
    }
});
```

**Why spawn in stop_game(), not check_game_health():** `check_game_health()` runs on a periodic tick (interval unknown but likely 10-30s). Spawning a precise 30s timer at the moment of Stopping transition is more accurate. `check_game_health()` remains for catching edge cases where stop_game() was not called (e.g. external stop).

### Pattern 4: Feature Flag Block — Server-Side Check

The feature flag system is in `AppState.feature_flags: RwLock<HashMap<String, FeatureFlagRow>>`. The flags are per-flag-name, not per-pod. The agent receives flags via `FlagSyncPayload` on connection.

The required behavior (STATE-03): server checks `game_launch` flag before sending to agent. If disabled, server records the failure immediately rather than waiting for the agent to send back an error. This is the correct approach — avoids a round-trip.

```rust
// Check feature flag before sending LaunchGame to agent
{
    let flags = state.feature_flags.read().await;
    let game_launch_enabled = flags.get("game_launch")
        .map(|f| f.enabled)
        .unwrap_or(true); // default enabled if flag not configured
    if !game_launch_enabled {
        // Update tracker to Error, broadcast, return
        let mut games = state.game_launcher.active_games.write().await;
        if let Some(tracker) = games.get_mut(pod_id) {
            tracker.game_state = GameState::Error;
            tracker.error_message = Some("game_launch feature disabled".to_string());
            let info = tracker.to_info();
            let _ = state.dashboard_tx.send(DashboardEvent::GameStateChanged(info));
        }
        return Err("game_launch feature flag is disabled".to_string());
    }
}
```

**Note:** The CONTEXT.md says "agent sends explicit GameStateUpdate with Error state" — this means the agent ALSO enforces the flag on its side, but the server pre-empts this by setting Error state before sending. Both sides enforce it; the server is authoritative.

### Pattern 5: externally_tracked Field on GameTracker

```rust
pub struct GameTracker {
    pub pod_id: String,
    pub sim_type: SimType,
    pub game_state: GameState,
    pub pid: Option<u32>,
    pub launched_at: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
    pub launch_args: Option<String>,
    pub auto_relaunch_count: u32,
    /// True when the server learned about this game from an agent report
    /// rather than initiating the launch itself. auto-relaunch is prohibited.
    pub externally_tracked: bool,   // NEW field
}
```

The creation path in `handle_game_state_update()` at line 382-395 already creates trackers with `launch_args: None` when no tracker exists. Add `externally_tracked: true` to that branch; all other creation paths use `externally_tracked: false`.

### Pattern 6: Broadcast Reliability — log warn on failure

Current code: `let _ = state.dashboard_tx.send(DashboardEvent::GameStateChanged(info));`

Fix:
```rust
if let Err(e) = state.dashboard_tx.send(DashboardEvent::GameStateChanged(info)) {
    tracing::warn!("dashboard broadcast failed for pod {}: {}", pod_id, e);
}
```

Apply this pattern to ALL `dashboard_tx.send()` calls in game_launcher.rs (currently 4 call sites: launch_game line 179, no-agent branch line 171, handle_game_state_update line 463, check_game_health line 715).

### Anti-Patterns to Avoid

- **`dyn GameLauncher` boxed trait objects:** Heap alloc on every launch. Use static dispatch via match or `&'static dyn` references (structs are zero-size, no heap needed).
- **Holding write lock across await:** The billing gate acquires read locks on both maps simultaneously. Never hold a write lock while awaiting — deadlock risk with the billing tick task.
- **Two separate lock acquisitions for TOCTOU re-check:** Must be atomic. Acquire the write lock for tracker creation ONLY after confirming billing. Never: check billing (drop lock) → do work → create tracker.
- **Adding feature flag check inside the agent-send branch:** Feature flag check must come BEFORE tracker creation. Tracker in Launching state with a disabled feature flag creates confusing state.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Stopping timeout timer | Custom interval poll in check_game_health() | `tokio::time::sleep` in `tokio::spawn` | Periodic poll has jitter; sleep + spawn is precise |
| JSON validation | Manual char-by-char parser | `serde_json::from_str::<serde_json::Value>` | Already in Cargo.toml, handles all edge cases |
| Feature flag lookup | Custom DB query in launch path | `state.feature_flags.read().await.get("game_launch")` | In-memory cache already populated at startup |
| Billing map dual check | Re-implement BillingManager lookup | Direct `state.billing.active_timers` + `state.billing.waiting_for_game` read | Maps are public on BillingManager |

---

## Common Pitfalls

### Pitfall 1: TOCTOU Window Between Billing Check and Tracker Creation
**What goes wrong:** Billing session ends between `active_timers.contains_key()` (line 101) and tracker insert (line 139). Game tracker created, LaunchGame sent to agent, no billing session to charge.
**Why it happens:** Two separate lock acquisitions with async work between them.
**How to avoid:** Re-check billing immediately before inserting tracker. Pattern: acquire write lock on `active_games` → inside the lock, do a final read of `active_timers` → only then insert. Since active_games write lock and active_timers are different RwLock instances, this is still a separate acquire but the window narrows to microseconds.
**Warning signs:** Game tracker exists with `launched_at` set but no corresponding billing session.

### Pitfall 2: Stopping State Double-Launch Guard
**What goes wrong:** Current guard at line 111-116 only blocks `Launching | Running`. A pod in `Stopping` state accepts a new launch request, creating two game trackers for the same pod.
**Why it happens:** `Stopping` was not in the original guard when it was written.
**How to avoid:** Extend the double-launch guard to include `Stopping`:
```rust
if matches!(tracker.game_state, GameState::Launching | GameState::Running | GameState::Stopping) {
    return Err(format!("game still stopping on pod {}", pod_id));
}
```

### Pitfall 3: Invalid JSON Silently Bypasses Content Validation
**What goes wrong:** `serde_json::from_str()` at line 87 fails → `if let Ok(args)` skips the block → `validate_launch_combo()` never called → launch proceeds with malformed args.
**Why it happens:** The `if let Ok(args)` pattern converts parse failure into "no validation" rather than "reject".
**How to avoid:** In `validate_args()`, explicitly reject invalid JSON:
```rust
fn validate_args(&self, args: Option<&str>) -> Result<(), String> {
    let Some(json) = args else { return Ok(()); };
    serde_json::from_str::<serde_json::Value>(json)
        .map_err(|e| format!("Invalid launch_args JSON: {e}"))?;
    Ok(())
}
```
Return `Err` before the launch proceeds if JSON is malformed.

### Pitfall 4: Stopping Timeout Fires After Game Already Recovered
**What goes wrong:** Stopping timeout spawned → agent confirms stop (Idle) → tracker removed → 30s later, timeout fires → `games.get_mut()` returns `None` (already removed). If not guarded, will panic or insert stale data.
**Why it happens:** Spawned future holds a reference to the pod_id but the tracker is removed on Idle.
**How to avoid:** The `if tracker.game_state == GameState::Stopping` re-check inside the timeout handles this — if tracker was removed, `get_mut()` returns `None` and the block is skipped. Always guard with `if let Some(tracker) = ... && tracker.game_state == Stopping`.

### Pitfall 5: Race Engineer Relaunches a Stopping Pod
**What goes wrong:** `relaunch_game()` checks `game_state != Error` and returns early (line 221). But the check is `!=` not `==` — Stopping state would fail with "Pod game is Stopping, not Error — cannot relaunch". This is actually correct behavior, but the error message is confusing. Log it clearly.
**Why it happens:** Not a bug, but the error message misleads staff.
**How to avoid:** Message is fine as-is; document that Stopping != Error is intentional.

### Pitfall 6: Feature Flag Default — Missing Flag = Enabled
**What goes wrong:** If `game_launch` flag is not configured in the DB, `flags.get("game_launch")` returns `None`. Default should be `true` (enabled), not `false` (disabled). Getting this wrong disables game launching venue-wide silently.
**Why it happens:** `unwrap_or(false)` vs `unwrap_or(true)`.
**How to avoid:** Always use `.unwrap_or(true)` for feature flags that gate core functionality.

---

## Code Examples

### Current Billing Gate (the bug)
```rust
// Source: crates/racecontrol/src/game_launcher.rs lines 99-106
// LIFE-02: Reject launch if no active billing session
{
    let timers = state.billing.active_timers.read().await;
    if !timers.contains_key(pod_id) {
        tracing::warn!("Launch rejected for pod {}: no active billing session", pod_id);
        return Err(format!("Pod {} has no active billing session", pod_id));
    }
}
// BUG: Does NOT check state.billing.waiting_for_game
// Deferred billing pods (WaitingForGame) are in waiting_for_game, not active_timers
```

### Current Double-Launch Guard (missing Stopping)
```rust
// Source: crates/racecontrol/src/game_launcher.rs lines 108-116
// LIFE-04: Check if a game is currently launching or running (avoid double-launch)
{
    let games = state.game_launcher.active_games.read().await;
    if let Some(tracker) = games.get(pod_id) {
        if matches!(tracker.game_state, GameState::Launching | GameState::Running) {
            return Err(format!("Pod {} already has a game active", pod_id));
        }
        // BUG: GameState::Stopping is missing from this guard
    }
}
```

### Current Broadcast (silent drop)
```rust
// Source: crates/racecontrol/src/game_launcher.rs line 177-179
// Broadcast to dashboards (only reached if agent IS connected)
let _ = state
    .dashboard_tx
    .send(DashboardEvent::GameStateChanged(info));
// BUG: `let _ =` silently discards broadcast::SendError
```

### BillingManager — Both Maps (the fix pattern)
```rust
// Source: crates/racecontrol/src/billing.rs line 370-372
pub active_timers: RwLock<HashMap<String, BillingTimer>>,
pub waiting_for_game: RwLock<HashMap<String, WaitingForGameEntry>>,
// Both are pub fields — directly accessible from launch_game()
```

### handle_game_state_update — Externally Tracked Creation Path
```rust
// Source: crates/racecontrol/src/game_launcher.rs lines 381-395
} else {
    // Agent reported state for a game we don't have tracked — create tracker
    games.insert(
        pod_id.to_string(),
        GameTracker {
            pod_id: pod_id.to_string(),
            sim_type: info.sim_type,
            game_state: info.game_state,
            pid: info.pid,
            launched_at: info.launched_at,
            error_message: info.error_message.clone(),
            launch_args: None,          // No args — externally started
            auto_relaunch_count: 0,
            // ADD: externally_tracked: true,
        },
    );
}
```

### Existing Test Pattern for Billing Gate
```rust
// Source: crates/racecontrol/src/game_launcher.rs lines 872-915
// Tests already exist for: no billing → reject, with billing → pass
// Phase 196 must ADD tests for: waiting_for_game → pass, paused → reject, TOCTOU → reject
```

---

## State of the Art

| Old Approach | Current Approach | Phase 196 Change |
|--------------|------------------|-----------------|
| Single `launch_game()` for all games | Same | Decompose into `GameLauncherImpl` trait per-game |
| Check `active_timers` only | Same | Check `active_timers` + `waiting_for_game` |
| No paused-session check | Same | Reject `Paused*` status sessions |
| Silent JSON bypass | Same | Explicit reject on parse failure |
| Silent broadcast drop | Same | `tracing::warn!` on send failure |
| No Stopping guard in double-launch | Same | Add Stopping to guard |
| No Stopping timeout | Same | 30s tokio::spawn timeout |
| No feature flag check in launch | Same | Check `game_launch` flag before send |
| No `externally_tracked` field | Same | Add field to GameTracker |

---

## Open Questions

1. **Feature flag per-pod vs global**
   - What we know: `AppState.feature_flags` is a global map (`HashMap<String, FeatureFlagRow>`). `FeatureFlagRow.overrides` is a JSON field for per-pod overrides but its structure is not confirmed.
   - What's unclear: Does CONTEXT.md's "Disable game_launch flag on pod-8" mean the override system is per-pod? Or is it a global flag toggle?
   - Recommendation: Implement as global flag check for Phase 196. The success criterion says "Disable game_launch flag on pod-8" — interpret as: disable the global `game_launch` flag and verify it blocks pod-8. Per-pod override support is Phase 197+ scope.

2. **Stopping timeout spawn location**
   - What we know: Both `stop_game()` and `check_game_health()` are candidates for spawning the 30s timer.
   - What's unclear: If stop_game() is called externally without going through the normal path, will the timer still spawn?
   - Recommendation: Spawn in `stop_game()` at the point of Stopping transition. Also add detection in `check_game_health()` for Stopping state without a pending timeout (edge case: server restart while game was Stopping).

3. **TOCTOU mitigation depth**
   - What we know: The success criterion says "launch fails cleanly with 'billing session expired'". This implies a re-check, not a transactional lock.
   - What's unclear: How tight must the re-check window be? Full correctness would require a single write lock over both billing check + tracker creation — but active_timers and active_games are separate RwLock instances.
   - Recommendation: Re-check billing inside the active_games write lock as the second guard. This narrows the TOCTOU window to the time of lock acquisition, which is acceptable for this use case (billing sessions don't expire in microseconds).

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[tokio::test]` + in-memory SQLite |
| Config file | No config file — `make_state()` helper builds AppState |
| Quick run command | `cargo test --package racecontrol-crate game_launcher 2>&1` |
| Full suite command | `cargo test --package racecontrol-crate 2>&1` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| LAUNCH-01 | `grep -rn "impl GameLauncherImpl for" crates/racecontrol/` shows 3 structs | structural grep | `grep -rn "impl GameLauncherImpl for" crates/racecontrol/src/game_launcher.rs` | ❌ Wave 0 |
| LAUNCH-02 | Pod in waiting_for_game → launch succeeds | unit | `cargo test --package racecontrol-crate test_launch_allowed_with_deferred_billing` | ❌ Wave 0 |
| LAUNCH-03 | Paused billing session → launch rejected with HTTP 400 | unit | `cargo test --package racecontrol-crate test_launch_rejected_paused_billing` | ❌ Wave 0 |
| LAUNCH-04 | TOCTOU: billing expires during launch → clean error | unit | `cargo test --package racecontrol-crate test_launch_toctou_billing_expired` | ❌ Wave 0 |
| LAUNCH-05 | Stopping state → launch rejected | unit | `cargo test --package racecontrol-crate test_double_launch_blocked_stopping` | ❌ Wave 0 |
| LAUNCH-06 | Malformed JSON → launch rejected | unit | `cargo test --package racecontrol-crate test_launch_rejected_invalid_json` | ❌ Wave 0 |
| LAUNCH-07 | dashboard_tx.send() failure → warn log | unit | `cargo test --package racecontrol-crate test_broadcast_failure_logged` | ❌ Wave 0 |
| STATE-01 | Stopping timeout 30s → Error + broadcast | unit | `cargo test --package racecontrol-crate test_stopping_timeout_transitions_to_error` | ❌ Wave 0 |
| STATE-02 | Disconnected agent → immediate Error | unit | existing `test_launch_allowed_with_billing` covers no-agent Error path | ✅ |
| STATE-03 | Feature flag disabled → Error state + no LaunchGame sent | unit | `cargo test --package racecontrol-crate test_feature_flag_disabled_rejects_launch` | ❌ Wave 0 |
| STATE-04 | Agent-reported game → externally_tracked=true, launch_args=None | unit | `cargo test --package racecontrol-crate test_game_state_update_creates_external_tracker` | ❌ Wave 0 |
| STATE-05 | No agent → Error immediately (not 120s) | unit | existing `test_launch_allowed_with_billing` verifies error path timing | ✅ |
| STATE-06 | Stopping state → relaunch_game() rejected | unit | `cargo test --package racecontrol-crate test_relaunch_rejected_stopping_state` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test --package racecontrol-crate game_launcher 2>&1`
- **Per wave merge:** `cargo test --package racecontrol-crate 2>&1`
- **Phase gate:** Full suite green + all new tests pass before marking phase shipped

### Wave 0 Gaps
- [ ] 11 new unit tests required — all in `crates/racecontrol/src/game_launcher.rs` `#[cfg(test)] mod tests`
- [ ] `make_state()` helper needs `launch_events` and `billing_events` tables (from Phase 195) — add to test setup
- [ ] `WaitingForGameEntry` needs to be constructible in test — add minimal constructor or use direct struct literal
- No new test files needed — all tests go into existing `mod tests` block in game_launcher.rs

---

## Sources

### Primary (HIGH confidence)
- Direct inspection: `crates/racecontrol/src/game_launcher.rs` (770 lines, fully read)
- Direct inspection: `crates/racecontrol/src/billing.rs` (4000+ lines, relevant sections read)
- Direct inspection: `crates/rc-common/src/types.rs` — GameState, SimType, BillingSessionStatus enums confirmed
- Direct inspection: `crates/racecontrol/src/state.rs` — AppState.feature_flags, agent_senders confirmed
- Cargo test run: `cargo test --package racecontrol-crate` → 66 tests pass (baseline confirmed)

### Secondary (MEDIUM confidence)
- ROADMAP.md Phase 196 success criteria (lines 3108-3124) — used to derive requirement interpretations
- CONTEXT.md implementation decisions — all discretion choices confirmed consistent with codebase patterns

### Tertiary (LOW confidence)
- None — all findings from authoritative source inspection

---

## Metadata

**Confidence breakdown:**
- Existing bugs: HIGH — all 6 bug locations confirmed with exact line numbers from source
- Trait design: HIGH — static dispatch pattern matches existing Rust idioms in codebase
- Billing gate fix: HIGH — both maps confirmed as `pub` fields on BillingManager
- Stopping timeout: HIGH — tokio::spawn + sleep is the standard pattern used in same file (Race Engineer)
- Feature flag: MEDIUM — `overrides` field semantics not confirmed (JSON column structure not inspected)
- Test gaps: HIGH — 11 new tests identified, existing 2 tests cover STATE-02/STATE-05 already

**Research date:** 2026-03-26
**Valid until:** 2026-04-25 (stable codebase, no external library changes needed)
