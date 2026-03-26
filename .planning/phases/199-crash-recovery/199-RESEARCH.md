# Phase 199: Crash Recovery - Research

**Researched:** 2026-03-26
**Domain:** Rust crash recovery — server-side Race Engineer (game_launcher.rs) + agent-side CrashRecoveryState (event_loop.rs) + metrics (metrics.rs)
**Confidence:** HIGH

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
None specified — all implementation choices are at Claude's discretion.

### Claude's Discretion
All implementation choices:
- How to detect crash vs intentional stop (exit code analysis, process monitoring)
- Clean state reset sequence (kill processes, clear files, reset adapters)
- Recovery action selection from historical data
- Grace timer implementation for relaunch attempts
- Safe mode interaction with game crashes vs pod health crashes
- Auto-relaunch flow with preserved launch_args
- Staff alerting after exhausted retries

### Deferred Ideas (OUT OF SCOPE)
None — discussion stayed within phase scope.
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| RECOVER-01 | Crash detection to full clean state (<10s) | clean_state_reset() already kills 13 exe names + clears game.pid + removes adapter lock. Already logs "clean_state_reset complete". Needs triggering from handle_game_state_update() immediately on Error. |
| RECOVER-02 | Full cycle crash→cleanup→relaunch→game spawned (<60s total) | Race Engineer 5s grace + agent CrashRecoveryState 60s per attempt = 65s theoretical max. Grace timer must be tuned. recovery_events.duration_ms measures this. |
| RECOVER-03 | Auto-relaunch uses exact same car/track/session (preserved args) | GameTracker.launch_args already stored. relaunch_game() already reads it. CrashRecoveryState.last_launch_args also stored in agent. Both paths exist — need wiring. |
| RECOVER-04 | Null args guard: externally tracked crash → skip auto-relaunch + log + staff alert | Race Engineer already guards tracker.externally_tracked || tracker.launch_args.is_none(). Needs "manual relaunch required" dashboard message + WS notification path. |
| RECOVER-05 | History-informed recovery: query recovery_events success rates, choose best action | recovery_events table exists with failure_mode, recovery_action_tried, outcome. Need query function + decision logic selecting Tier 1 action by success rate. Currently hardcoded to "game_crash" + "auto_relaunch_attempt_N". |
| RECOVER-06 | After 2 failed retries: billing paused + DashboardEvent::BillingPaused + WhatsApp alert with exit codes + suggested alternative | Billing pause on exhaustion already works (PausedGamePause). send_staff_launch_alert() exists. Gaps: BillingPaused event variant, exit codes in alert, suggested alternative from recovery_events history. |
| RECOVER-07 | Exit grace timer NOT armed during recovery (crash_recovery != Idle) | Already guarded: `if !matches!(conn.crash_recovery, CrashRecoveryState::PausedWaitingRelaunch{..})` in AcStatus::Off path AND in game process exit path. Needs safe mode persistence test. |
</phase_requirements>

---

## Summary

Phase 199 builds on a substantial foundation already laid in Phases 196-198. The server-side Race Engineer (game_launcher.rs) already has atomic auto-relaunch logic, null-args guard, billing pause on exhaustion, and WhatsApp staff alerts. The agent-side CrashRecoveryState FSM already has PausedWaitingRelaunch (60s per attempt), exit grace suppression during recovery, and AutoEndPending. The metrics infrastructure already has recovery_events table and record_recovery_event().

The **primary gap** is that these two subsystems (server Race Engineer and agent CrashRecoveryState) currently operate independently without the full clean_state_reset() being triggered as part of the recovery loop. The agent FSM handles the 60s relaunch timers, but clean_state_reset() must fire before each relaunch attempt to ensure orphan processes are killed. The second gap is history-informed action selection — currently the failure_mode is always hardcoded to "game_crash" and the recovery_action is hardcoded to "auto_relaunch_attempt_N" without querying historical success rates.

The third gap is measurement: RECOVER-02 requires the full cycle be under 60s total, but the current Race Engineer adds a 5s delay before sending LaunchGame, meaning the 60s SLA needs clean_state_reset to run within that 5s window. The agent-side timer is 60s per attempt — total worst case is 5s + 60s = 65s, which violates RECOVER-02. The grace delay should be eliminated or the agent timer reduced.

**Primary recommendation:** Wire clean_state_reset() into the server Race Engineer's pre-relaunch 5s window (via agent exec or by routing through the agent's LaunchGame handler which calls pre_launch_checks() which includes orphan detection). Eliminate the 5s server-side grace; instead the 5s is consumed by the clean state reset itself on the agent. Add recovery action history query to choose the best action. Slim the kiosk notification path for recovery state.

---

## What Is Already Built (HIGH confidence)

### Server: game_launcher.rs — Race Engineer

| Component | Status | Location |
|-----------|--------|----------|
| Atomic auto-relaunch (max 2, single write lock) | DONE | handle_game_state_update() lines 692-712 |
| Null args guard (externally_tracked OR launch_args=None) | DONE | lines 696-700 |
| 5s grace delay before relaunch send | DONE | tokio::time::sleep(5s) line 733 |
| Billing still-active re-check before relaunch | DONE | lines 735-750 |
| Billing pause on exhaustion (PausedGamePause) | DONE | lines 811-820 |
| WhatsApp staff alert on exhaustion | DONE | send_staff_launch_alert() line 825 |
| recovery_event recording (attempt + exhausted) | DONE | lines 770-782, 832-847 |
| GameTracker.launch_args storage | DONE | struct field, set in launch_game() |
| GameTracker.auto_relaunch_count atomic increment | DONE | single write lock, LAUNCH-17 |
| Timeout routing through handle_game_state_update | DONE | check_game_health() line 1001 |

### Agent: event_loop.rs — CrashRecoveryState FSM

| Component | Status | Location |
|-----------|--------|----------|
| CrashRecoveryState enum (Idle/PausedWaitingRelaunch/AutoEndPending) | DONE | lines 44-56 |
| CrashRecoveryState.last_launch_args field | DONE | struct field in PausedWaitingRelaunch |
| 60s per-attempt timer | DONE | Duration::from_secs(60) |
| Exit grace suppression during recovery | DONE | !matches!(conn.crash_recovery, PausedWaitingRelaunch{..}) |
| Safe mode suppression of process guard scans | DONE | safe_mode_active AtomicBool check |
| Overlay toast on crash ("Game crashed — relaunching...") | DONE | line 636 |
| Billing resume on successful relaunch (PID detected) | DONE | lines 1167-1181 |
| Auto-end billing after 2 failed attempts | DONE | AutoEndPending + SessionAutoEnded |

### Agent: game_process.rs — Clean State Reset

| Component | Status | Location |
|-----------|--------|----------|
| clean_state_reset() | DONE | kills 13 exe names, clears game.pid, removes adapter lock |
| all_game_process_names() — 13 exe names | DONE | static list |
| clear_persisted_pid() | DONE |  |
| pre_launch_checks() — orphan detection | DONE | returns Err if orphan found |

### Metrics: metrics.rs

| Component | Status | Location |
|-----------|--------|----------|
| RecoveryEvent struct | DONE | failure_mode, recovery_action_tried, outcome, duration_ms |
| record_recovery_event() | DONE | SQLite + error logging |
| recovery_events SQLite table | DONE | columns: pod_id, sim_type, car, track, failure_mode, recovery_action_tried, recovery_outcome, recovery_duration_ms |

---

## Architecture Patterns

### Recovery Flow (as-built vs. what Phase 199 adds)

**Current flow (Phases 196-198):**
```
Game crash detected (GameState::Error)
  └─ handle_game_state_update() [server]
       ├─ Race Engineer: increment counter, check <= 2
       ├─ sleep 5s
       └─ send LaunchGame to agent (NO clean_state_reset)

Agent receives LaunchGame
  └─ pre_launch_checks() → FAILS if orphan processes still running
       └─ returns Err, game stays in Error
```

**Phase 199 target flow:**
```
Game crash detected (GameState::Error)
  └─ handle_game_state_update() [server]
       ├─ Race Engineer: increment counter, query history → choose action
       ├─ Send CleanStateReset command to agent (new or reuse existing path)
       └─ After clean (ack or 5s window) → send LaunchGame

Agent receives CleanStateReset (or LaunchGame with clean flag)
  └─ clean_state_reset() → kills orphans, clears PID, clears adapter lock
  └─ pre_launch_checks() → now passes (no orphans)
  └─ game process spawned

Recovery duration tracked: crash_detected_at → game PID appears
```

### History-Informed Action Selection Pattern

The recovery_events table already exists. The query pattern mirrors query_dynamic_timeout():

```rust
// Query top recovery action for this failure_mode+sim_type on this pod
// Returns: ("kill_clean_relaunch", success_rate, sample_count)
async fn query_best_recovery_action(
    db: &SqlitePool,
    pod_id: &str,
    sim_type: &str,
    failure_mode: &str,
) -> (String, f64) {
    // SELECT recovery_action_tried, COUNT(*) as total,
    //        SUM(CASE WHEN recovery_outcome='"Success"' THEN 1 ELSE 0 END) as successes
    // FROM recovery_events
    // WHERE pod_id=? AND sim_type=? AND failure_mode=?
    //   AND created_at > datetime('now','-30 days')
    // GROUP BY recovery_action_tried
    // ORDER BY (successes * 1.0 / total) DESC LIMIT 1
}
```

Minimum 3 samples required (same policy as query_dynamic_timeout). Fall back to default "kill_clean_relaunch" action if insufficient history.

### RECOVER-02: 60s SLA Analysis

Current timing breakdown:
- Crash detection: near-instant (game_check_interval polls every ~2s)
- Server Race Engineer processes: < 1s
- Server 5s grace delay: 5s
- Agent receives LaunchGame: < 1s
- clean_state_reset() on agent: 1-3s (sysinfo scan + taskkill)
- Game spawn: < 1s
- **Total to game spawned: ~8-10s** — well within 60s

The agent-side CrashRecoveryState timer is 60s per ATTEMPT (waiting for game to reach Running state, not just spawn). The 60s measures the full attempt window. Game spawn itself happens within ~10s of crash. RECOVER-02 says "game process spawned" — this is the spawn event, not Running state. So 60s SLA is achievable.

**Key insight:** The 5s server-side grace must cover the clean state reset round-trip. Option A: send CleanStateReset command first, then LaunchGame after ack. Option B: Include clean_state_reset in agent's LaunchGame handler (already does via pre_launch_checks() orphan detection — but that REJECTS if orphan found, doesn't clean). Option C: Add a "force_clean" flag to LaunchGame that triggers clean_state_reset() before pre_launch_checks(). Option C is the minimal-change approach.

### Safe Mode Persistence During Recovery (RECOVER-07 + RECOVER-08)

Safe mode is activated by WMI game detection (game EXE starts). During recovery:
- Game crashes → safe mode starts cooldown (30s timer)
- Race Engineer triggers relaunch within 5s
- WMI detects new game EXE → safe mode re-entered before cooldown expires

**Gap:** If clean_state_reset kills the game process, safe mode cooldown fires. Then when relaunch starts, WMI re-activates safe mode. This is correct behavior. But the 30s safe mode cooldown must NOT deactivate safe mode between the crash and the relaunch. The fix: in CrashRecoveryState::PausedWaitingRelaunch, suppress safe mode cooldown timer from firing.

The CONTEXT.md requirement: "game crash counter SEPARATE from pod health counter" — this is already true. MAINTENANCE_MODE (pod health) uses the rc-agent self-restart counter. CrashRecoveryState FSM is entirely separate. No counter sharing exists.

### RECOVER-06: BillingPaused Dashboard Event

Currently on exhaustion: billing status is set to PausedGamePause (correct), but there is no DashboardEvent::BillingPaused broadcast separate from the billing tick. The kiosk needs to receive a specific message to show "Session paused — staff notified". Phase 201 (KIOSK-05) mentions this state. For Phase 199, the requirement is to broadcast BillingPaused and ensure WhatsApp includes exit codes + suggested alternative.

**Current send_staff_launch_alert signature:**
```rust
async fn send_staff_launch_alert(state, pod_id, sim_type, error_taxonomy)
```
Needs to add: `exit_codes: &[Option<i32>]` (stored in GameTracker or passed from the Error events), and `suggested_alternative: Option<String>` (from query_best_recovery_action or a simple heuristic).

Exit codes: The GameTracker does NOT currently store exit codes. The `info.exit_code` field is in GameLaunchInfo. Phase 199 needs to add `last_exit_codes: Vec<Option<i32>>` to GameTracker (or pass the exit code from the Error event into the alert call).

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead |
|---------|-------------|-------------|
| Process kill | Custom kill loop | existing kill_process() + all_game_process_names() in game_process.rs |
| PID file cleanup | Custom file ops | clear_persisted_pid() in game_process.rs |
| Recovery event storage | Custom DB code | record_recovery_event() in metrics.rs |
| Timeout computation | Custom math | query_dynamic_timeout() pattern in metrics.rs |
| WhatsApp alert | New HTTP client | send_staff_launch_alert() in game_launcher.rs |
| Atomic counter | Mutex + int | existing auto_relaunch_count in GameTracker under write lock |
| Safe mode check | AtomicBool | existing safe_mode_active AtomicBool in AppState |

---

## Common Pitfalls

### Pitfall 1: Double-Triggering Race Engineer from rapid duplicate Error events
**What goes wrong:** Two Error events in <100ms both increment auto_relaunch_count and spawn two relaunches.
**Why it happens:** handle_game_state_update() called concurrently from WS receive tasks.
**How to avoid:** Already fixed with single write lock (LAUNCH-17). Do NOT change the locking pattern.
**Warning signs:** Server log shows two "attempt 1/2" entries for same pod.

### Pitfall 2: MAINTENANCE_MODE triggered by game crash counter
**What goes wrong:** RC-agent interprets game crash as pod health failure, writes MAINTENANCE_MODE sentinel, blocks all future relaunches.
**Why it happens:** Confusing rc-agent self-restart counter with game crash counter.
**How to avoid:** Game crash is handled by server-side Race Engineer counter (auto_relaunch_count in GameTracker). RC-agent self-restart counter is in rc-agent main.rs. They are separate. Never increment the rc-agent restart counter from game crash events.
**Warning signs:** MAINTENANCE_MODE file appears after game crash.

### Pitfall 3: Exit grace timer fires during recovery (premature AcStatus::Off)
**What goes wrong:** AC game exits (crash), exit grace timer arms (30s), fires before relaunch completes, sends AcStatus::Off to server → server treats as session end → billing stops.
**Why it happens:** Exit grace arms on AcStatus::Off (AC shared memory path) or game process exit (non-AC path).
**How to avoid:** Already guarded with `!matches!(conn.crash_recovery, PausedWaitingRelaunch{..})`. Verify this check covers ALL paths where exit_grace_armed could be set.
**Warning signs:** "exit grace armed" in agent log during recovery window.

### Pitfall 4: clean_state_reset() called on server instead of agent
**What goes wrong:** Server calls clean_state_reset() which is an rc-agent function living in crates/rc-agent/src/game_process.rs. Server cannot call it directly.
**Why it happens:** Confusion about crate boundaries.
**How to avoid:** clean_state_reset() MUST be called on the agent. Options: (a) send a new CoreToAgentMessage::CleanStateReset variant, (b) add force_clean: bool to LaunchGame message, (c) rely on pre_launch_checks() orphan detection + auto-retry. Option (b) is cleanest — single message, agent handles clean then launch.

### Pitfall 5: recovery_events car/track NULL when launch_args has them
**What goes wrong:** Recovery event recorded with car=None, track=None even though launch_args JSON has car/track. History-informed selection can't distinguish per-combo success rates.
**Why it happens:** recovery_event populated before extracting car/track from launch_args.
**How to avoid:** Use extract_launch_fields(&launch_args) before building RecoveryEvent, same pattern as in launch_game() and relaunch_game().

### Pitfall 6: Safe mode cooldown fires between crash and relaunch
**What goes wrong:** Game crashes at T=0. Safe mode starts cooldown (30s). At T=30s, safe mode deactivates. Relaunch happens at T=5-10s but WMI may not re-detect fast enough. Process guard scans resume during recovery window.
**Why it happens:** Safe mode cooldown timer not suppressed during CrashRecoveryState::PausedWaitingRelaunch.
**How to avoid:** In safe mode cooldown timer select! branch, check `!matches!(conn.crash_recovery, PausedWaitingRelaunch{..})` before deactivating safe mode. This keeps process guard suppressed for the duration of recovery.

---

## Code Examples

### Pattern 1: Adding force_clean to LaunchGame (minimal-change approach)

```rust
// In rc-common/src/protocol.rs
pub enum CoreToAgentMessage {
    LaunchGame {
        sim_type: SimType,
        launch_args: Option<String>,
        #[serde(default)]
        force_clean: bool,  // Phase 199: true = run clean_state_reset() before launch
    },
    // ...
}
```

```rust
// In rc-agent/src/event_loop.rs — LaunchGame handler
CoreToAgentMessage::LaunchGame { sim_type, launch_args, force_clean } => {
    if force_clean {
        let killed = game_process::clean_state_reset();
        tracing::info!(target: LOG_TARGET, "clean state reset complete: {} processes killed", killed);
    }
    // existing pre_launch_checks() + spawn logic
}
```

### Pattern 2: History-informed recovery action query

```rust
// In racecontrol/src/metrics.rs
pub async fn query_best_recovery_action(
    db: &SqlitePool,
    pod_id: &str,
    sim_type: &str,
    failure_mode: &str,
) -> (String, f64) {
    let rows: Vec<(String, i64, i64)> = sqlx::query_as(
        "SELECT recovery_action_tried,
                COUNT(*) as total,
                SUM(CASE WHEN recovery_outcome='\"Success\"' THEN 1 ELSE 0 END) as successes
         FROM recovery_events
         WHERE pod_id = ? AND sim_type = ? AND failure_mode = ?
           AND created_at > datetime('now', '-30 days')
         GROUP BY recovery_action_tried
         ORDER BY (successes * 1.0 / total) DESC
         LIMIT 1"
    )
    .bind(pod_id).bind(sim_type).bind(failure_mode)
    .fetch_all(db).await.unwrap_or_default();

    if rows.is_empty() || rows[0].1 < 3 {
        return ("kill_clean_relaunch".to_string(), 0.0);  // default
    }
    let (action, total, successes) = &rows[0];
    let rate = *successes as f64 / *total as f64;
    (action.clone(), rate)
}
```

### Pattern 3: RecoveryEvent with car/track populated from launch_args

```rust
// In handle_game_state_update(), Race Engineer branch — replace existing RecoveryEvent build
let (rec_car, rec_track, _, _) = extract_launch_fields(&tracker.launch_args);
let recovery_event = metrics::RecoveryEvent {
    id: uuid::Uuid::new_v4().to_string(),
    pod_id: pod_id_owned.clone(),
    sim_type: Some(sim_name.clone()),
    car: rec_car,    // populated from launch_args
    track: rec_track, // populated from launch_args
    failure_mode: format!("{:?}", error_taxonomy),  // use actual ErrorTaxonomy, not "game_crash"
    recovery_action_tried: format!("kill_clean_relaunch_attempt_{}", attempt),
    recovery_outcome: metrics::RecoveryOutcome::Success,
    recovery_duration_ms: Some(crash_detected_at.elapsed().as_millis() as i64),
    error_details: Some(format!("exit_code: {:?}", exit_code)),
};
```

### Pattern 4: Safe mode cooldown suppression during recovery

```rust
// In event_loop.rs — safe mode cooldown timer branch
_ = &mut state.safe_mode_cooldown_timer, if state.safe_mode_cooldown_armed => {
    // RECOVER-08: Do NOT deactivate safe mode during crash recovery window
    if matches!(conn.crash_recovery, CrashRecoveryState::PausedWaitingRelaunch { .. }) {
        tracing::info!(target: LOG_TARGET, "Safe mode cooldown suppressed — crash recovery in progress");
        // Re-arm cooldown for another 30s
        state.safe_mode_cooldown_timer.as_mut().reset(
            tokio::time::Instant::now() + std::time::Duration::from_secs(30)
        );
    } else {
        state.safe_mode_cooldown_armed = false;
        state.safe_mode.exit();
        state.safe_mode_active.store(false, std::sync::atomic::Ordering::Relaxed);
        tracing::info!(target: LOG_TARGET, "Safe mode cooldown expired — safe mode DEACTIVATED");
    }
}
```

### Pattern 5: exit_codes storage in GameTracker

```rust
// In GameTracker struct (game_launcher.rs)
pub struct GameTracker {
    // ... existing fields ...
    /// Last 2 exit codes from auto-relaunch attempts (for staff alert)
    pub exit_codes: Vec<Option<i32>>,
}
```

---

## Gap Analysis: RECOVER-01 through RECOVER-07

| Req | What's Built | What's Missing |
|-----|-------------|----------------|
| RECOVER-01 (cleanup <10s) | clean_state_reset() exists, logs "clean_state_reset complete" | Not triggered from Race Engineer path. Needs force_clean=true on LaunchGame OR new CleanStateReset message sent before LaunchGame. |
| RECOVER-02 (full cycle <60s) | Race Engineer 5s grace + agent 60s timer | Timing is fine (game spawns in ~10s). But clean_state_reset must happen within the 5s window. With force_clean on LaunchGame, agent cleans then spawns — all within 10s. |
| RECOVER-03 (preserved args) | GameTracker.launch_args stored, used by Race Engineer | Already works — Race Engineer passes stored launch_args to the relaunch LaunchGame. CrashRecoveryState.last_launch_args also stored on agent. No gap. |
| RECOVER-04 (null args guard) | externally_tracked/None guard exists, logs warning | Missing: no DashboardEvent sent for "manual relaunch required". Staff sees nothing on dashboard. Add DashboardEvent::GameStateChanged with error_message="Cannot auto-relaunch: no launch args. Please relaunch from kiosk." |
| RECOVER-05 (history-informed) | recovery_events table exists, failure_mode hardcoded | Missing: query_best_recovery_action() function. Failure_mode should use actual ErrorTaxonomy not hardcoded "game_crash". Add server-side query + log "recovery action selected: X (Y% historical success)". |
| RECOVER-06 (billing pause notification) | Billing paused (PausedGamePause), WhatsApp sent | Missing: exit_codes not in alert. suggested_alternative not in alert. DashboardEvent::BillingPaused not a separate event (billing tick broadcasts the status change). Need exit_codes in GameTracker + structured alert message. |
| RECOVER-07 (exit grace guard) | Guard exists in AcStatus::Off path AND game process exit path | Need to verify guard also covers safe mode cooldown not deactivating during recovery. Pattern 4 above. |

---

## Standard Stack

No new dependencies required. All infrastructure is in-crate:

| Component | Version | Purpose |
|-----------|---------|---------|
| sqlx | existing | recovery_events query |
| serde_json | existing | ErrorTaxonomy serialization for failure_mode |
| tokio::time::sleep | existing | Race Engineer grace timer |
| uuid | existing | recovery event IDs |
| tracing | existing | "clean state reset complete" + "recovery action selected" logs |

---

## Architecture Patterns

### Recommended Plan Structure

Based on the gap analysis, Phase 199 fits naturally into 2 plans:

**Plan 199-01 — Server-side recovery hardening:**
- Add exit_codes field to GameTracker
- Use actual ErrorTaxonomy for failure_mode (not hardcoded "game_crash")
- Populate car/track in RecoveryEvent from launch_args
- Add query_best_recovery_action() to metrics.rs
- Log "recovery action selected: X (Y% historical success)"
- Send force_clean=true on Race Engineer LaunchGame (adds force_clean to CoreToAgentMessage)
- Structured staff alert with exit codes + suggested alternative
- Add DashboardEvent for null-args guard (RECOVER-04)

**Plan 199-02 — Agent-side recovery hardening + tests:**
- Handle force_clean in LaunchGame handler (call clean_state_reset() before pre_launch_checks)
- Log "clean state reset complete" within LaunchGame handler path
- Suppress safe mode cooldown during CrashRecoveryState::PausedWaitingRelaunch (Pattern 4)
- Tests: clean state reset triggers, exit grace suppressed during recovery, safe mode persists, concurrent crash protection, null args rejection

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust cargo test (built-in) |
| Config file | No separate config — standard `cargo test` |
| Quick run command | `cargo test -p rc-agent -- crash_recovery` |
| Full suite command | `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p racecontrol` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| RECOVER-01 | clean_state_reset() called on game crash | unit | `cargo test -p rc-agent -- test_clean_state_reset_on_crash` | ❌ Wave 0 |
| RECOVER-02 | Full cycle <60s measured in recovery_events | unit | `cargo test -p racecontrol -- test_recovery_duration` | ❌ Wave 0 |
| RECOVER-03 | Relaunch uses same launch_args | unit | `cargo test -p racecontrol -- test_relaunch_preserves_args` | ❌ Wave 0 |
| RECOVER-04 | Null args → skip relaunch + dashboard msg | unit | `cargo test -p racecontrol -- test_null_args_guard` | ❌ Wave 0 |
| RECOVER-05 | query_best_recovery_action returns history | unit | `cargo test -p racecontrol -- test_query_best_recovery_action` | ❌ Wave 0 |
| RECOVER-06 | Exit codes in staff alert | unit | `cargo test -p racecontrol -- test_staff_alert_with_exit_codes` | ❌ Wave 0 |
| RECOVER-07 | Exit grace NOT armed during CrashRecovery != Idle | unit | `cargo test -p rc-agent -- test_exit_grace_suppressed_during_recovery` | ❌ existing partial: crash_recovery_state_starts_idle ✅ |
| RECOVER-08 | Safe mode persists during recovery | unit | `cargo test -p rc-agent -- test_safe_mode_cooldown_suppressed_during_recovery` | ❌ Wave 0 |
| RECOVER-09 | Concurrent crashes: only 1 recovery sequence | unit | `cargo test -p racecontrol -- test_atomic_relaunch_count` | ❌ Wave 0 (LAUNCH-17 has related test structure) |

### Sampling Rate
- **Per task commit:** `cargo test -p rc-agent -- crash_recovery`
- **Per wave merge:** `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p racecontrol`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] Tests for RECOVER-01 through RECOVER-09 (all in rc-agent and racecontrol crates)
- [ ] `query_best_recovery_action()` unit test with mock DB rows
- [ ] CrashRecoveryState safe mode cooldown suppression test
- [ ] Existing test `crash_recovery_state_starts_idle` in event_loop.rs line 1599 can be extended

---

## Open Questions

1. **force_clean on LaunchGame vs separate CleanStateReset message**
   - What we know: CoreToAgentMessage::LaunchGame already goes through the agent's main WS handler. Adding a bool field is backward-compatible via `#[serde(default)]`.
   - What's unclear: Whether a separate CleanStateReset message is cleaner architecturally (separation of concerns).
   - Recommendation: Use force_clean=true on LaunchGame (simpler, one round-trip, backward compatible). Add separate CleanStateReset only if clean confirmation ack is needed.

2. **suggested_alternative in staff alert**
   - What we know: The RECOVER-06 success criterion says "suggested action: try different car" if history shows this car fails often. query_best_recovery_action() can provide this.
   - What's unclear: Whether to build a full alternative recommendation (requires combo_reliability table from Phase 200) or use a simple heuristic (e.g., "try different car" for ProcessCrash taxonomy).
   - Recommendation: Use simple heuristic for Phase 199. Full alternatives come from Phase 200 (INTEL-05). Phrase as "suggested action: try different car/track combo" without specific alternative.

3. **DashboardEvent::BillingPaused vs relying on billing tick**
   - What we know: Phase 201 (KIOSK-05) explicitly handles `paused_game_pause` WebSocket message on kiosk. The billing tick already broadcasts PausedGamePause status.
   - What's unclear: Whether Phase 199 needs a dedicated BillingPaused event or if the billing status change in the tick is sufficient.
   - Recommendation: Do NOT add a new DashboardEvent variant in Phase 199 — that's Phase 201's job. Phase 199 only needs to ensure PausedGamePause is set (already done) and WhatsApp is sent (already done, just needs exit codes + better message).

---

## Sources

### Primary (HIGH confidence)
- `crates/racecontrol/src/game_launcher.rs` — Full Race Engineer implementation, handle_game_state_update(), relaunch_game(), send_staff_launch_alert(), check_game_health()
- `crates/rc-agent/src/game_process.rs` — clean_state_reset(), pre_launch_checks(), all_game_process_names(), GameProcess
- `crates/rc-agent/src/event_loop.rs` — CrashRecoveryState FSM, exit_grace logic, safe_mode suppression, ConnectionState
- `crates/racecontrol/src/metrics.rs` — RecoveryEvent, record_recovery_event(), query_dynamic_timeout() (pattern for history query)
- `crates/rc-agent/src/app_state.rs` — safe_mode, safe_mode_active AtomicBool, safe_mode_cooldown_timer
- `.planning/ROADMAP.md` lines 3179-3193 — Phase 199 success criteria (9 items mapping to RECOVER-01 through RECOVER-09)
- `.planning/phases/199-crash-recovery/199-CONTEXT.md` — locked decisions, code context, integration points

### Secondary (MEDIUM confidence)
- `.planning/STATE.md` — Phase 196-198 accumulated context (billing gates, atomic Race Engineer decisions, clean_state_reset details)

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all in-crate, no new deps
- Architecture: HIGH — code read directly, gaps identified precisely
- Pitfalls: HIGH — derived from existing code paths and standing rules (MAINTENANCE_MODE, safe mode, exit grace)

**Research date:** 2026-03-26 IST
**Valid until:** 2026-04-25 (stable domain — changes only if Phases 197/198 are modified)
