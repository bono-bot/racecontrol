# Phase 198: On-Track Billing - Research

**Researched:** 2026-03-26
**Domain:** Rust billing state machine + per-game telemetry detection (AC shared memory, F1 25 UDP, iRacing shared memory)
**Confidence:** HIGH — based on direct codebase read of billing.rs, game_launcher.rs, event_loop.rs, and all sim adapters

## Summary

Phase 198 reworks billing to start ONLY when a customer car is on-track and controllable. The core infrastructure is already partially present: `WaitingForGame` billing status exists, `defer_billing_start()` is wired into the auth flow, and `handle_game_status_update()` already converts `AcStatus::Live` to billing start. The primary gap is that the current Live trigger is not accurate enough — AC uses raw STATUS=2 which fires during replays and menus, F1 25 uses first UDP packet (correct but needs speed gate), and iRacing IsOnTrack already works.

The key changes are: (1) adding a speed+steering gate for AC's False-Live check (BILL-02), (2) wiring the existing `PlayableSignal` enum into server-side billing start, (3) adding `cancelled_no_playable` session status, (4) adding a `BillingConfig` TOML section for all timeouts currently hardcoded, and (5) broadcasting `WaitingForGame` status to the kiosk WebSocket tick so the kiosk timer shows "Loading..." instead of countdown.

**Primary recommendation:** Extend existing `handle_game_status_update()` and `WaitingForGameEntry` rather than rebuilding. The server-side billing state machine is solid. The biggest new work is the AC False-Live guard (5s window) and configurable timeouts extraction.

## Standard Stack

### Core (no new dependencies needed)
| Component | Current State | Phase 198 Role |
|-----------|--------------|----------------|
| `billing.rs` `handle_game_status_update()` | Converts AcStatus::Live → billing start | Add False-Live guard, PlayableSignal variant routing |
| `BillingTimer::tick()` | Already handles WaitingForGame (no-op) | No change needed |
| `WaitingForGameEntry` | Already tracks `attempt`, `waiting_since`, `group_session_id` | Add `playable_signal_at` timestamp for metrics |
| `check_launch_timeouts()` / `check_launch_timeouts_from_manager()` | Hardcoded 180s | Replace with configurable `BillingConfig::launch_timeout_per_attempt_secs` |
| `BillingSessionStatus::WaitingForGame` | Already in enum | Already ticked as no-op — no change |
| `PlayableSignal` enum | Defined in `rc-common/src/types.rs` | Not yet routed through server billing |
| `LaunchState::WaitingForLive` (agent) | Tracks per-game PlayableSignal dispatch | AC False-Live guard adds speed+steer check |
| `AssettoCorsaAdapter::read_ac_status()` | Returns raw STATUS field | Feeds False-Live guard via telemetry frame speed |
| `F125Adapter` (agent) | First UDP packet fires `UdpActive` → PlayableSignal | Already correct — just needs speed gate on server |
| `IracingAdapter::read_is_on_track()` | Already returns `is_on_track` bool | Already dispatched in event_loop — works for BILL-04 |

### Supporting
| Component | Purpose |
|-----------|---------|
| `DashboardEvent::BillingTick` | Already broadcasts BillingSessionInfo (with status field) — kiosk receives it |
| `BillingSessionInfo.status` | Already serialized as snake_case — kiosk parses it |
| `metrics.rs` `BillingAccuracyEvent` | Already records `playable_signal_at`, `billing_start_at`, `delta_ms` |
| `racecontrol.toml` `[billing]` section | DOES NOT EXIST yet — must be added |
| `Config` struct in `config.rs` | Must add `billing: BillingConfig` field |

**Installation:** No new crates needed.

## Architecture Patterns

### Existing Flow (what works today)
```
kiosk start session
  → defer_billing_start() → waiting_for_game HashMap (WaitingForGameEntry)
  → BillingTimer::tick() → WaitingForGame status (no elapsed increment)

agent: game launched
  → LaunchState::WaitingForLive
  → telemetry_interval tick: read_ac_status() == Live (1s stability window)
  → AgentMessage::GameStatusUpdate { ac_status: AcStatus::Live }

server: handle_game_status_update(Live)
  → removes from waiting_for_game
  → calls start_billing_session() immediately
  → BillingTimer.status = Active
  → BillingTick broadcasts show elapsed time
```

### Phase 198 Target Flow
```
kiosk start session
  → defer_billing_start() → waiting_for_game
  → BillingTick broadcasts status=waiting_for_game → kiosk shows "Loading..."

agent: game process detected (Launching → Running/Loading)
  → GameState::Loading emitted (already implemented: loading_emitted flag)
  → per-game PlayableSignal dispatch runs every 2s (game_check_interval)

AC path:
  → read_ac_status() == Live (1s stability window) — already implemented
  → NEW: check telemetry frame: speed > 0 OR |steer| > 0.02 within 5s window
  → if False-Live (speed==0 AND steer==0 for 5s): suppress Live signal
  → on True Live: send AcStatus::Live with sim_type=AssettoCorsa

F1 25 path:
  → UdpActive (first UDP packet) detected by DrivingDetector → f1_udp_playable_received=true
  → game_check_interval: f1_udp_playable_received → send AcStatus::Live
  → BILL-03: ALREADY CORRECT (F1 UDP means active session data)

iRacing path:
  → adapter.read_is_on_track() == true → send AcStatus::Live
  → BILL-04: ALREADY IMPLEMENTED in event_loop.rs game_check block

EVO/WRC/Forza (process fallback):
  → 90s elapsed since WaitingForLive → send AcStatus::Live (ProcessFallback)
  → BILL-08: Guard — if game.is_running() == false at fallback time → send Error not Live

server: handle_game_status_update(Live)
  → EXISTING: removes from waiting_for_game, calls start_billing_session()
  → NEW: record playable_signal_at timestamp in billing_accuracy_events

crash during WaitingForGame:
  → agent: GameCrashed { billing_active: false }
  → server: AcStatus::Off removes from waiting_for_game (already done!)
  → NEW: insert billing_sessions record with status='cancelled_no_playable'
  → NEW: send WhatsApp alert to staff (BILL-06)

crash during Active billing:
  → EXISTING: GameCrashed → BillingPaused → PausedGamePause
  → EXISTING: relaunch → AcStatus::Live → resume (status == Active, pause_seconds reset)
  → NEW: record total_paused_seconds in billing_sessions (BILL-07) — already tracked!

timeout with no PlayableSignal (BILL-06):
  → check_launch_timeouts(): attempt 1 at 180s → reset to attempt 2
  → attempt 2 at 360s → cancel with no charge
  → NEW: status = 'cancelled_no_playable' in DB insert for tracking
  → Staff alert already partially in place (end session path)
```

### Recommended Project Structure (changes only)
```
crates/racecontrol/src/
├── billing.rs           # Add False-Live guard, BillingConfig usage, cancelled_no_playable
├── config.rs            # Add BillingConfig struct + billing field on Config

crates/rc-agent/src/
├── event_loop.rs        # Add False-Live 5s speed+steer window for AC

C:\RacingPoint\
└── racecontrol.toml     # Add [billing] section with configurable timeouts
```

### Anti-Patterns to Avoid
- **Rebuilding PlayableSignal from scratch:** The enum exists in rc-common/types.rs, the dispatch exists in event_loop.rs — extend, don't replace.
- **Adding a new WS message type for PlayableSignal:** The existing `AgentMessage::GameStatusUpdate { ac_status: AcStatus::Live }` IS the PlayableSignal to the server. Reuse it.
- **Putting False-Live logic on server side:** The server only receives AcStatus::Live once per second. The False-Live speed gate must run on the AGENT side where it has access to live telemetry frames (100ms cadence).
- **Using `tokio::time::pause()` for timeout tests:** Phase 196-02 lesson — breaks SQLite pool. Use `Instant`-based elapsed checks with injectable `check_launch_timeouts_from_manager()` which already takes `&BillingManager` directly.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| AC on-track detection | Custom UDP parser on port 9996 | Existing `AssettoCorsaAdapter` shared memory (acpmf_physics speed + steer fields) | AC already reads these at offsets `physics::SPEED_KMH` (28) and `physics::STEER_ANGLE` (24) |
| F1 25 on-track detection | New UDP port parser | Existing `F125Adapter` + `DrivingDetector::UdpActive` signal | Already wired: `f1_udp_playable_received` flag + `LaunchState::WaitingForLive` dispatch |
| iRacing on-track detection | Shared memory re-implementation | Existing `IracingAdapter::read_is_on_track()` | Already dispatches in event_loop game_check block |
| Billing pause on crash | New crash handler | Existing `CrashRecoveryState::PausedWaitingRelaunch` + `BillingPaused` message | Already sets PausedGamePause on server side via GameCrashed |
| Billing resume on relaunch | New resume path | Existing `handle_game_status_update(Live)` with `PausedGamePause` check | Line 629: `if timer.status == PausedGamePause { timer.status = Active }` |
| WaitingForGame status broadcast | New WS event | Existing `DashboardEvent::BillingTick(timer.to_info())` | Status field already in BillingSessionInfo — kiosk already receives it |
| Multiplayer WaitingForGame eviction | New cleanup job | Existing `multiplayer_billing_timeout()` + `check_launch_timeouts()` | Both already clean up non-connecting pods |

## Common Pitfalls

### Pitfall 1: AC False-Live — STATUS=2 during replay/menu
**What goes wrong:** AC reports `STATUS=2 (LIVE)` during replay start and briefly during menu. Speed=0, steer=0. Billing would start for 0 driving time.
**Why it happens:** AC's STATUS field reflects rendering state not car controllability. On fresh session start, there is a brief LIVE window before the car is actually handed over.
**How to avoid:** 5-second window after LIVE detected — require `speed_kmh > 0 OR |steer_angle| > 0.02` at least once within that window before emitting `AcStatus::Live` to server. Track `ac_live_since: Option<Instant>` and `ac_live_has_input: bool` in ConnectionState.
**Warning signs:** BILL-02 success criteria: "speed stays 0 and no steering input for 5s → billing does NOT start"

### Pitfall 2: Process Fallback fires for crashed game (BILL-08)
**What goes wrong:** EVO/WRC/Forza game crashes at 88s. At 90s, `WaitingForLive` elapsed check fires and emits AcStatus::Live. Billing starts for dead game.
**Why it happens:** The 90s fallback (event_loop.rs line 702) checks time elapsed but not whether `game.is_running()`.
**How to avoid:** Gate the fallback: `if launched_at.elapsed() >= 90s AND game.is_running() { emit Live } else if !game.is_running() { emit Error }`. The Error path already ends the WaitingForGame entry (AcStatus::Off handler removes from waiting_for_game).
**Warning signs:** Check `game.is_running()` returns false before the fallback timer fires.

### Pitfall 3: WaitingForGame status not reaching kiosk
**What goes wrong:** Kiosk shows countdown from 0 during game loading instead of "Loading..." because it receives no BillingTick with WaitingForGame status.
**Why it happens:** `tick_all_timers()` already handles WaitingForGame (returns false, no elapsed increment) BUT may not broadcast a tick event for WaitingForGame sessions. Check whether `BillingSessionStatus::WaitingForGame` timers receive a BillingTick event each second.
**How to avoid:** In `tick_all_timers()`, add a branch: `if timer.status == WaitingForGame { events_to_broadcast.push(BillingTick(timer.to_info())); continue; }` — the status field will be `waiting_for_game` which the kiosk can render as "Loading...".
**Warning signs:** BILL-05: "kiosk WebSocket receives BillingSessionStatus::WaitingForGame" — verify the BillingTick message includes this status.

### Pitfall 4: cancelled_no_playable status not in BillingSessionStatus enum
**What goes wrong:** BILL-06 requires `status='cancelled_no_playable'` in DB. If the enum doesn't have this variant, the billing_session INSERT won't write it, and the SELECT will return nothing.
**Why it happens:** `BillingSessionStatus` currently has: Pending, WaitingForGame, Active, PausedManual, PausedDisconnect, PausedGamePause, Completed, EndedEarly, Cancelled. No `CancelledNoPlayable`.
**How to avoid:** Add `CancelledNoPlayable` variant to `BillingSessionStatus` in rc-common/types.rs. Then create a minimal billing_sessions DB record in the timeout handler (currently it only clears the WaitingForGameEntry without creating a DB record).
**Warning signs:** This also needs a TypeScript type update in `packages/shared-types/` for Phase 201 compatibility.

### Pitfall 5: Hardcoded 180s timeout in check_launch_timeouts
**What goes wrong:** BILL-12 requires configurable timeouts — currently hardcoded at `180` in `check_launch_timeouts_from_manager()` and `60` for multiplayer in `tokio::spawn`.
**Why it happens:** No `BillingConfig` in config.rs — all timeouts are magic numbers.
**How to avoid:** Add `BillingConfig` struct to config.rs, add `[billing]` section to racecontrol.toml with: `multiplayer_wait_timeout_secs = 60`, `pause_auto_end_timeout_secs = 600`, `launch_timeout_per_attempt_secs = 180`, `idle_drift_threshold_secs = 300`, `offline_grace_secs = 300`. Thread `Arc<Config>` or clone the billing config into relevant call sites.
**Warning signs:** The test in BILL-12: "Change multiplayer_wait to 90 → restart server → multiplayer wait is 90s"

### Pitfall 6: Multiplayer group_session_members query failure (BILL-10)
**What goes wrong:** `sqlx::query_scalar` for `group_session_members` silently falls back to empty set → treats multiplayer as single-player → billing starts immediately on first Live.
**Why it happens:** Current code at billing.rs line 492 does `.unwrap_or_default()` — silent empty set on DB error.
**How to avoid:** Change to `match .fetch_all().await { Ok(ids) => ..., Err(e) => { tracing::error!(...); return; } }` — billing start REJECTED with logged error on DB failure.

### Pitfall 7: AC timer sync — two Utc::now() calls (BILL-09)
**What goes wrong:** If two `Utc::now()` calls bracket an await point, the `playable_signal_at` and `billing_start_at` timestamps diverge by up to milliseconds. Not a customer-facing bug but a metrics accuracy issue.
**Why it happens:** Two separate timestamp captures in the billing accuracy event recording.
**How to avoid:** Capture `let now = Utc::now()` once, use it for both `playable_signal_at` and `billing_start_at` fields.

## Code Examples

### False-Live Guard for AC (agent-side, event_loop.rs)
```rust
// In ConnectionState — add these two fields:
pub(crate) ac_live_since: Option<std::time::Instant>,
pub(crate) ac_live_has_input: bool,

// In telemetry_interval tick, after read_ac_status() returns Live:
if status == AcStatus::Live {
    // Start 5s False-Live window if not already in it
    if conn.ac_live_since.is_none() {
        conn.ac_live_since = Some(std::time::Instant::now());
        conn.ac_live_has_input = false;
    }
    // Read speed + steer from telemetry (already available in same tick)
    if let Ok(Some(ref frame)) = adapter.read_telemetry() {
        if frame.speed_kmh > 0.0 || frame.steering.abs() > 0.02 {
            conn.ac_live_has_input = true;
        }
    }
    // Fire billing only if input confirmed OR 5s window expired with input seen
    let elapsed = conn.ac_live_since.map(|t| t.elapsed().as_secs()).unwrap_or(0);
    if conn.ac_live_has_input {
        // True Live — emit
        conn.ac_live_since = None;
        // ... existing emit code ...
        conn.launch_state = LaunchState::Live;
    } else if elapsed >= 5 {
        // False-Live: suppress, reset window
        conn.ac_live_since = None;
        tracing::info!(target: LOG_TARGET, "AC False-Live suppressed (5s, speed=0, steer=0)");
    }
}
// On AcStatus::Off — clear window
if status == AcStatus::Off {
    conn.ac_live_since = None;
    conn.ac_live_has_input = false;
}
```

### BillingConfig in config.rs
```rust
#[derive(Debug, Clone, Deserialize)]
pub struct BillingConfig {
    /// How long to wait for multiplayer pods to all reach LIVE before evicting non-connectors (seconds).
    #[serde(default = "default_multiplayer_wait_timeout")]
    pub multiplayer_wait_timeout_secs: u64,
    /// How long a game-pause can last before billing session auto-ends (seconds).
    #[serde(default = "default_pause_auto_end_timeout")]
    pub pause_auto_end_timeout_secs: u32,
    /// Per-attempt timeout waiting for PlayableSignal (seconds). 2 attempts = 2x this.
    #[serde(default = "default_launch_timeout_per_attempt")]
    pub launch_timeout_per_attempt_secs: u64,
    /// Seconds of no driving input before billing anomaly flagged (idle drift).
    #[serde(default = "default_idle_drift_threshold")]
    pub idle_drift_threshold_secs: u64,
    /// Grace period before auto-ending session when pod goes offline (seconds).
    #[serde(default = "default_offline_grace")]
    pub offline_grace_secs: u64,
}

fn default_multiplayer_wait_timeout() -> u64 { 60 }
fn default_pause_auto_end_timeout() -> u32 { 600 }
fn default_launch_timeout_per_attempt() -> u64 { 180 }
fn default_idle_drift_threshold() -> u64 { 300 }
fn default_offline_grace() -> u64 { 300 }

impl Default for BillingConfig {
    fn default() -> Self {
        Self {
            multiplayer_wait_timeout_secs: default_multiplayer_wait_timeout(),
            pause_auto_end_timeout_secs: default_pause_auto_end_timeout(),
            launch_timeout_per_attempt_secs: default_launch_timeout_per_attempt(),
            idle_drift_threshold_secs: default_idle_drift_threshold(),
            offline_grace_secs: default_offline_grace(),
        }
    }
}
```

### WaitingForGame tick broadcast (billing.rs — tick_all_timers gap)
```rust
// In tick_all_timers(), before the PausedDisconnect block:
if timer.status == BillingSessionStatus::WaitingForGame {
    // Broadcast WaitingForGame status each tick so kiosk shows "Loading..."
    events_to_broadcast.push(DashboardEvent::BillingTick(timer.to_info(&rate_tiers)));
    continue; // No elapsed increment — matches existing tick() behavior
}
```

### cancelled_no_playable DB record (billing.rs — launch timeout handler)
```rust
// In tick_all_timers() launch timeout (attempt 2) handler, after removing from waiting_for_game:
if let Some(entry) = entry {
    // Insert billing_sessions record so SELECT works for audit
    let _ = sqlx::query(
        "INSERT INTO billing_sessions (id, pod_id, driver_id, status, created_at, ended_at)
         VALUES (?, ?, ?, 'cancelled_no_playable', datetime('now'), datetime('now'))"
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(&entry.pod_id)
    .bind(&entry.driver_id)
    .execute(&state.db)
    .await;
    tracing::warn!("Session cancelled_no_playable: pod={} driver={}", entry.pod_id, entry.driver_id);
    // Staff alert (BILL-06)
    // ... WhatsApp send ...
}
```

### Process fallback crash guard (event_loop.rs)
```rust
// In the `Some(sim_type)` fallback arm (line 699-716):
Some(sim_type) => {
    if let LaunchState::WaitingForLive { launched_at, .. } = &conn.launch_state {
        if launched_at.elapsed() >= Duration::from_secs(90) {
            // BILL-08: Gate — only emit Live if game is still running
            let game_alive = state.game_process.as_ref()
                .map(|g| g.is_running())
                .unwrap_or(false);
            if game_alive {
                tracing::info!(target: LOG_TARGET, "{:?} process fallback (90s) — game alive, emitting Live", sim_type);
                // ... existing emit code ...
            } else {
                tracing::warn!(target: LOG_TARGET, "{:?} process fallback (90s) — game DEAD, emitting Error not Live", sim_type);
                let info = GameLaunchInfo { game_state: GameState::Error, ... };
                // ... emit Error ...
                conn.launch_state = LaunchState::Idle;
            }
        }
    }
}
```

## State of the Art

| Old Approach | Current Approach (post-Phase 196/197) | Phase 198 Change |
|--------------|--------------------------------------|-----------------|
| Start billing immediately on game launch | Defer billing until AC STATUS=2 (Live) | Require car controllability signal (speed>0 OR steer≠0) before billing |
| 180s hardcoded timeout | 180s hardcoded timeout | Configurable via `[billing]` TOML section |
| No PlayableSignal enum | PlayableSignal enum defined, partially wired | Fully wire PlayableSignal through server billing start |
| Multiplayer: start billing on first LIVE | Multiplayer: wait for all group members | Keep + make timeout configurable |
| No WaitingForGame kiosk broadcast | WaitingForGame status exists but may not broadcast | Add BillingTick broadcast for WaitingForGame status |
| No cancelled_no_playable record | Session just removed from waiting_for_game map | Insert DB record + staff alert |

**Deprecated/outdated:**
- Magic number `180` in `check_launch_timeouts_from_manager()` — replace with `config.billing.launch_timeout_per_attempt_secs`
- Magic number `60` in `tokio::spawn(async move { sleep(60s) })` for multiplayer timeout — replace with config
- Magic number `600` in `BillingTimer::tick()` PausedGamePause branch — replace with config

## Open Questions

1. **Does `BillingTimer::tick()` need to read from config?**
   - What we know: `tick()` uses hardcoded `600` for PausedGamePause timeout. `AppState` is not passed to `tick()` — it only takes `&mut self`.
   - What's unclear: How to thread `pause_auto_end_timeout_secs` into tick() without changing its signature.
   - Recommendation: Move the timeout check OUT of tick() into `tick_all_timers()` where `AppState` is available. Then `tick()` just increments `pause_seconds`, and `tick_all_timers()` checks `timer.pause_seconds >= config.billing.pause_auto_end_timeout_secs`.

2. **F1 25 speed gate — is first UDP packet sufficient?**
   - What we know: First UDP packet from F1 25 on port 20777 fires `UdpActive` which fires `AcStatus::Live`. This is currently correct behavior per BILL-03 success criteria ("After UDP telemetry on port 20777 shows m_sessionType > 0 AND m_speed > 0 → billing starts").
   - What's unclear: The current F1 code triggers on ANY UDP packet — packet 1 (Session header) may arrive before the car is on track.
   - Recommendation: The BILL-03 criterion says "m_sessionType > 0 AND m_speed > 0". The `F125Adapter::session_type` field is already tracked. Gate on `session_type > 0` (already means active race/qualifying, not lobby) — this is likely sufficient without a speed check.

3. **racecontrol.toml [billing] section — does AppState need to pass config to check_launch_timeouts?**
   - What we know: `check_launch_timeouts(state)` takes `&Arc<AppState>`. Adding `state.config.billing.launch_timeout_per_attempt_secs` to the timeout check is straightforward.
   - What's unclear: `check_launch_timeouts_from_manager()` is used in tests and takes only `&BillingManager`. Tests pass hardcoded 180s currently.
   - Recommendation: Add a `timeout_secs: u64` parameter to `check_launch_timeouts_from_manager()` — test pass `180`, production pass config value.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust `cargo test` (tokio::test for async, plain test for sync) |
| Config file | Cargo.toml (existing) |
| Quick run command | `cargo test -p racecontrol billing 2>&1 \| tail -20` |
| Full suite command | `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p racecontrol` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| BILL-01 | AC billing starts only after car on-track (speed>0 or steer≠0) | unit | `cargo test -p racecontrol ac_false_live -x` | ❌ Wave 0 |
| BILL-02 | AC False-Live suppressed (speed=0, steer=0 for 5s) | unit | `cargo test -p racecontrol ac_false_live_suppressed` | ❌ Wave 0 |
| BILL-03 | F1 25 billing starts on first UDP packet (session active) | unit (existing DrivingDetector) | `cargo test -p rc-agent f1_udp_playable` | ❌ Wave 0 |
| BILL-04 | iRacing billing starts when IsOnTrack=true | unit | `cargo test -p racecontrol iracing_on_track` | ❌ Wave 0 |
| BILL-05 | Kiosk receives WaitingForGame status via BillingTick | unit | `cargo test -p racecontrol waiting_for_game_tick_broadcasts` | ❌ Wave 0 |
| BILL-06 | Kill before PlayableSignal → cancelled_no_playable, staff alert | unit | `cargo test -p racecontrol cancelled_no_playable` | ❌ Wave 0 |
| BILL-07 | Crash pause/resume tracks total_paused_seconds | unit (existing nearby) | `cargo test -p racecontrol billing_pause_resume_tracks_paused_seconds` | ❌ Wave 0 |
| BILL-08 | Process fallback crash guard — dead game → Error not Live | unit | `cargo test -p racecontrol process_fallback_crash_guard` | ❌ Wave 0 |
| BILL-09 | AC timer sync: single Utc::now(), dynamic threshold | unit | `cargo test -p racecontrol ac_timer_sync_single_timestamp` | ❌ Wave 0 |
| BILL-10 | Multiplayer DB query failure → reject billing, log error | unit (async) | `cargo test -p racecontrol multiplayer_db_query_failure` | ❌ Wave 0 |
| BILL-11 | Orphan cleanup: evicted WaitingForGame entries removed | unit (existing) | `cargo test -p racecontrol timeout_evicts_non_connecting_pod_billing_starts_for_connected` | ✅ (billing.rs line 3890) |
| BILL-12 | Configurable timeouts: change multiplayer_wait → runtime change | integration | `cargo test -p racecontrol configurable_billing_timeouts` | ❌ Wave 0 |

**Note on BILL-07:** `billing_pause_disconnect_freezes_driving_seconds` (line 2984) and `game_status_live_on_paused_game_pause_resumes_billing` (line 3389) cover pause/resume. A new test specifically verifying `total_paused_seconds` accumulation during `PausedGamePause` is needed.

**Note on BILL-11:** `timeout_evicts_non_connecting_pod_billing_starts_for_connected` at line 3890 already covers BILL-11 — but verify it checks `waiting_for_game.len()` decreases.

### Sampling Rate
- **Per task commit:** `cargo test -p racecontrol billing 2>&1 | grep -E "test result|FAILED"`
- **Per wave merge:** `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p racecontrol`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/racecontrol/src/billing.rs` — 11 new test functions (BILL-01 through BILL-12 minus BILL-11)
- [ ] `BillingConfig` struct added to `config.rs` — needed before configurable timeout tests compile
- [ ] `BillingSessionStatus::CancelledNoPlayable` variant added to `rc-common/types.rs` — needed before BILL-06 test
- [ ] `ConnectionState::ac_live_since` + `ac_live_has_input` fields — needed before BILL-01/BILL-02 tests
- [ ] Framework: already installed (cargo test), no new setup needed

## Phase Requirements

<phase_requirements>

| ID | Description | Research Support |
|----|-------------|-----------------|
| BILL-01 | AC billing starts only when car on-track and controllable (speed>0 OR steer≠0 within 5s of AcStatus::Live) | AC shared memory `acpmf_physics` speed at offset 28 + steer at offset 24 already read by `AssettoCorsaAdapter`. Need 5s window guard in `ConnectionState`. |
| BILL-02 | AC False-Live: if STATUS=Live but speed=0 and no steer for 5s → billing does NOT start | New `ac_live_since: Option<Instant>` + `ac_live_has_input: bool` in ConnectionState. Reset on AcStatus::Off. |
| BILL-03 | F1 25 billing starts on UDP port 20777 session data (m_sessionType>0 AND speed>0) | `F125Adapter` already sends `UdpActive` signal → `f1_udp_playable_received` flag. Gate on session_type field already tracked. |
| BILL-04 | iRacing billing starts when IsOnTrack=true AND IsOnTrackCar=true | `IracingAdapter::read_is_on_track()` already implemented. Event_loop already dispatches. |
| BILL-05 | Kiosk shows "Loading..." during WaitingForGame state via BillingTick broadcast | Add WaitingForGame branch to `tick_all_timers()` to broadcast `DashboardEvent::BillingTick` each second. |
| BILL-06 | Kill before PlayableSignal → billing NEVER starts, `cancelled_no_playable` DB record, staff alert, ₹0 charge | Extend launch timeout handler (attempt 2) to INSERT billing_sessions record + WhatsApp alert. |
| BILL-07 | Crash pause/resume: total_paused_seconds tracks exact crash recovery duration | Existing `PausedGamePause` → `Active` transition already resets `pause_seconds`. Need `total_paused_seconds` persisted to DB in sync_timers_to_db(). |
| BILL-08 | EVO/WRC/Forza 90s fallback guard: game crashed before 90s → emit Error not Live | Add `game.is_running()` check before process fallback emit in event_loop.rs `Some(sim_type)` arm. |
| BILL-09 | AC timer sync: dynamic threshold from historical data, single Utc::now() call, correct pod ID | Replace `billing_alt_id` with canonical pod_id; capture single `let now = Utc::now()` for both timestamp fields in billing_accuracy_event. |
| BILL-10 | Multiplayer DB query failure → billing rejected with error, not silently treated as single-player | Replace `.unwrap_or_default()` on group_session_members query with explicit error return. |
| BILL-11 | Multiplayer 60s timeout evicts non-connected pods; late-arriving pod does NOT start billing | Existing `multiplayer_billing_timeout()` + `evicted_pod_late_live_does_not_start_billing` test covers this. Verify coverage, make timeout configurable. |
| BILL-12 | All billing timeouts configurable via racecontrol.toml [billing] section | Add `BillingConfig` to config.rs. Thread into `check_launch_timeouts()`, `multiplayer_billing_timeout()` spawn, `tick_all_timers()` pause check. |

</phase_requirements>

## Sources

### Primary (HIGH confidence)
- Direct codebase read — `crates/racecontrol/src/billing.rs` (4300+ lines) — WaitingForGame flow, handle_game_status_update, tick_all_timers, check_launch_timeouts
- Direct codebase read — `crates/rc-agent/src/event_loop.rs` — LaunchState machine, per-sim PlayableSignal dispatch, False-Live AC handling, F1/iRacing/LMU dispatch
- Direct codebase read — `crates/rc-agent/src/sims/assetto_corsa.rs` — physics offsets, SPEED_KMH=28, STEER_ANGLE=24
- Direct codebase read — `crates/rc-agent/src/sims/f1_25.rs` — UDP port 20777 parsing, session_type tracking
- Direct codebase read — `crates/rc-agent/src/sims/iracing.rs` — is_on_track shared memory variable
- Direct codebase read — `crates/rc-common/src/types.rs` — BillingSessionStatus enum, PlayableSignal enum, GameState enum, AcStatus enum
- Direct codebase read — `crates/racecontrol/src/config.rs` — Config struct (no BillingConfig section exists)
- ROADMAP.md success criteria (lines 3156-3173) — BILL-01 through BILL-12 exact test scenarios
- CONTEXT.md — all implementation decisions at Claude's discretion

### Secondary (MEDIUM confidence)
- STATE.md accumulated context — Phase 196-02 lesson: `tokio::time::pause()` breaks SQLite pool
- STATE.md — Phase 197 lesson: exit_code priority, atomic Race Engineer

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — direct codebase read, no guessing
- Architecture: HIGH — existing flow mapped line-by-line, gaps identified precisely
- Pitfalls: HIGH — 7 specific pitfalls with line references and exact reproduction conditions

**Research date:** 2026-03-26
**Valid until:** 2026-04-25 (stable billing.rs — changes only if Phase 199/200 modifies billing.rs)
