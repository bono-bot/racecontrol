# Phase 3: Billing Synchronization - Research

**Researched:** 2026-03-13
**Domain:** Billing lifecycle, AC shared memory integration, overlay rendering, per-minute pricing
**Confidence:** HIGH

## Summary

Phase 3 transforms the billing system from fixed-duration countdown billing (30/60 min pre-paid) to open-ended per-minute billing with retroactive two-tier pricing. The billing trigger must shift from "PIN validated" to "AC shared memory STATUS=2 (LIVE)" so customers are never charged for loading screens, DirectX initialization, or game startup failures.

The existing codebase provides approximately 70% of the required infrastructure. The AC shared memory adapter (`AssettoCorsaAdapter`) already reads the STATUS field at `graphics::STATUS` (offset 4), but this value is not currently exposed through the `SimAdapter` trait or used for billing control. The `BillingTimer` in rc-core counts down from `allocated_seconds` and deducts from wallet upfront -- both concepts must be replaced with an elapsed-time model where cost is calculated retroactively at session end. The overlay (`overlay.rs`) currently shows a countdown timer (`remaining_seconds`) and detects `game_live` via a telemetry heuristic (speed/RPM > 0) rather than the authoritative STATUS field.

The primary architectural challenge is coordinating state across three components: (1) rc-agent reads AC STATUS and reports it to rc-core, (2) rc-core owns the billing timer and pricing calculation, (3) rc-agent renders the overlay with elapsed time and running cost. The existing WebSocket protocol between agent and core provides the communication channel, and the existing `BillingTick` message already flows every second -- it just needs different payload fields.

**Primary recommendation:** Add a `read_ac_status()` method to `SimAdapter`, have rc-agent poll it and send a new `GameStatusUpdate` message to rc-core, refactor `BillingTimer` to count UP (elapsed) instead of DOWN (remaining), compute per-minute cost with retroactive tier logic in a pure function, and update the overlay to display elapsed time + running cost (taxi meter).

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **Billing Model -- Open-Ended Per-Minute:** No pre-booked duration. Per-minute billing with two-tier retroactive pricing: under 30 min = Rs.23.3/min, 30+ min = Rs.15/min (cheaper rate applies to ENTIRE session retroactively). Hard max cap (e.g. 3 hours).
- **Billing Trigger:** Starts ONLY when AC shared memory STATUS=2 (LIVE). No billable time during startup, loading, or DirectX init. Timer counts UP (elapsed), not down.
- **Pause Behavior:** Billing pauses when AC STATUS=PAUSE (ESC/pause menu). Does NOT pause for pit lane. After 10 minutes continuously paused, session auto-ends. Overlay shows PAUSED badge, elapsed timer freezes.
- **Session End Triggers:** Customer via PWA, staff via kiosk, next booking forces end (5-min warning), hard max cap, pause timeout. Game killed cleanly (FFB zeroing is Phase 4 scope).
- **Overlay -- Taxi Meter Display:** Shows elapsed time + running cost (e.g. "15:23 -- Rs.350"). At ~25 min, shows rate upgrade prompt. At 30 min, brief celebration that rate dropped. If next booking approaching, shows warning.
- **Loading Screen Experience:** Overlay appears at game launch with "0:00 -- Rs.0" frozen + "WAITING FOR GAME" text. Switches to live when STATUS=LIVE.
- **Game Launch Failure:** 3-minute timeout if AC never reaches STATUS=LIVE. Auto-retry once. If second attempt fails, cancel session (no charge).
- **Session Extension:** No explicit extension -- session just keeps running. Only limit is hard max cap or upcoming booking.

### Claude's Discretion
- Exact mechanism for agent-to-core billing trigger communication (WebSocket message, HTTP call, etc.)
- How to track pause duration (timer in agent vs polling STATUS)
- Whether auto-retry reuses the same billing session ID or creates a new one
- Internal state machine transitions (Pending -> Live -> Paused -> Live -> Ended)
- How the per-minute cost calculation integrates with existing pricing_tiers table
- Exact hard max cap duration (suggest 3 hours)
- Rate upgrade prompt timing and display duration

### Deferred Ideas (OUT OF SCOPE)
- Booking/scheduling system (15-min slots, advance booking, pod blocking, booking fee, schedule on lock screen)
- Mid-session crash recovery (AC crashes mid-drive -- billing pause? reconnect?)
- Multi-pod synchronized billing (Phase 9)
- Time-of-day rate multipliers (peak hours pricing -- pricing_rules table exists but not wired to per-minute model)
- Loyalty/subscription pricing
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| BILL-01 | Billing timer starts when AC shared memory STATUS=LIVE (on-track), not at game process launch | SimAdapter needs `read_ac_status()` method; agent sends GameStatusUpdate to core; core creates billing session only on STATUS=LIVE; 3-min timeout with auto-retry on failure |
| BILL-02 | DirectX initialization delay does not count as billable time | Same mechanism as BILL-01 -- billing does not start until STATUS=LIVE, which only happens after DirectX init, content loading, and car placement are complete |
| BILL-06 | Session time remaining displayed as overlay during gameplay | Overlay refactored from countdown to taxi meter (elapsed time + running cost); WAITING FOR GAME state during loading; PAUSED badge when STATUS=PAUSE; rate upgrade prompt near 25 min |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| rc-common (protocol.rs) | local | Agent-Core WebSocket protocol messages | Established pattern -- all billing events flow through here |
| rc-common (types.rs) | local | BillingSessionInfo, BillingSessionStatus, PricingTier types | Shared types across crates |
| rc-core (billing.rs) | local | BillingTimer, BillingManager, tick loop, session lifecycle | All billing logic lives here -- refactor in place |
| rc-agent (overlay.rs) | local | Native Win32 HUD overlay with GDI rendering | Existing overlay -- extend OverlayData and paint routine |
| rc-agent (assetto_corsa.rs) | local | AC shared memory reader for STATUS field | Already reads graphics::STATUS at offset 4 |
| winapi | 0.3.x | Windows shared memory (MapViewOfFile) + Win32 GDI | Already in use for AC adapter and overlay |
| sqlx | 0.7.x | SQLite async queries for billing_sessions, pricing_tiers | Existing DB layer in rc-core |
| chrono | 0.4.x | Timestamp handling for billing events | Already used throughout |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| serde/serde_json | 1.x | Protocol message serialization | All WebSocket messages |
| uuid | 1.x | Billing session ID generation | New session creation |
| tokio | 1.x | Async runtime, timers, channels | Tick loop, timeout handling |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| WebSocket `GameStatusUpdate` msg | HTTP POST from agent to core | WebSocket already established; HTTP adds latency and a second connection to manage |
| STATUS polling in agent main loop | Dedicated STATUS watcher thread | Main loop already polls telemetry at ~200ms; adding STATUS check there avoids thread overhead |
| Pure function for cost calc | DB-backed pricing lookup per tick | Pure function is testable, fast; DB lookup only needed at session end for final bill |

## Architecture Patterns

### Current Architecture (What Exists)

```
AUTH FLOW (current):
  PIN validated -> billing::start_billing_session() -> BillingTimer created (countdown)
                -> CoreToAgentMessage::BillingStarted -> overlay.activate(driver, allocated_seconds)
                -> CoreToAgentMessage::LaunchGame -> AC starts

BILLING TICK (current, every 1s):
  BillingTimer.tick() -> driving_seconds++ -> remaining = allocated - driving
  -> CoreToAgentMessage::BillingTick { remaining_seconds, allocated_seconds }
  -> overlay.update_billing(remaining_seconds)
```

### Target Architecture (What We Build)

```
AUTH FLOW (new):
  PIN validated -> CoreToAgentMessage::LaunchGame -> AC starts
              -> overlay shows "0:00 -- Rs.0 WAITING FOR GAME"
              -> agent polls AC STATUS every 200ms
              -> STATUS transitions 0 -> 2 (LIVE)
              -> AgentMessage::GameStatusUpdate { pod_id, status: AcLive }
              -> billing::start_billing_session() creates timer (count-UP, no allocated_seconds)
              -> CoreToAgentMessage::BillingTick { elapsed_seconds, cost_paise, rate_per_min_paise }
              -> overlay shows "15:23 -- Rs.350"

BILLING TICK (new, every 1s):
  BillingTimer.tick() -> elapsed_seconds++
  -> compute_session_cost(elapsed_seconds) -> cost_paise
  -> CoreToAgentMessage::BillingTick { elapsed_seconds, cost_paise, rate_per_min_paise, ... }
  -> overlay.update_billing(elapsed, cost, rate, paused, ...)

PAUSE (new):
  Agent polls STATUS -> STATUS transitions 2 -> 3 (PAUSE)
  -> AgentMessage::GameStatusUpdate { status: AcPause }
  -> BillingTimer.status = PausedGamePause (new status)
  -> elapsed_seconds FROZEN, pause_seconds ticking
  -> overlay shows PAUSED badge, frozen timer
  -> If pause_seconds >= 600 -> auto-end session
  -> STATUS transitions 3 -> 2 (LIVE) -> resume billing

GAME LAUNCH FAILURE:
  AC launched -> agent polls STATUS
  -> 3 minutes pass, STATUS never reaches 2 (LIVE)
  -> Agent sends GameStatusUpdate { status: LaunchTimeout }
  -> Core kills AC, retries once (same billing session ID, not yet created)
  -> If second timeout -> cancel, no charge, agent shows error
```

### Recommended Project Structure (changes only)
```
rc-common/src/
  protocol.rs          # Add: AgentMessage::GameStatusUpdate, update BillingTick fields
  types.rs             # Add: BillingSessionStatus::PausedGamePause, AcStatus enum, update BillingSessionInfo

rc-core/src/
  billing.rs           # Refactor: BillingTimer count-up, compute_session_cost(), new tick payload
  auth/mod.rs          # Decouple: validate_pin no longer calls start_billing_session
  db/mod.rs            # Migration: billing_sessions schema changes

rc-agent/src/
  sims/mod.rs          # Add: SimAdapter::read_ac_status() -> Option<AcStatus>
  sims/assetto_corsa.rs # Implement: read_ac_status() reads graphics::STATUS
  main.rs              # Add: STATUS polling, GameStatusUpdate sending, launch timeout logic
  overlay.rs           # Refactor: OverlayData elapsed model, taxi meter rendering, PAUSED badge
```

### Pattern 1: Agent STATUS Polling
**What:** rc-agent reads AC STATUS from shared memory every telemetry tick (~200ms) and reports transitions to rc-core via WebSocket.
**When to use:** Every telemetry poll cycle in the main loop.
**Why this way:** STATUS is only available via Windows shared memory on the pod. rc-core runs on the server and cannot read it directly. Agent already has the shared memory handles open.

```rust
// In SimAdapter trait (sims/mod.rs):
fn read_ac_status(&self) -> Option<AcStatus> { None } // default for non-AC sims

// In AssettoCorsaAdapter:
fn read_ac_status(&self) -> Option<AcStatus> {
    let graphics = self.graphics_handle.as_ref()?;
    let raw = Self::read_i32(graphics, graphics::STATUS);
    Some(match raw {
        0 => AcStatus::Off,
        1 => AcStatus::Replay,
        2 => AcStatus::Live,
        3 => AcStatus::Pause,
        _ => AcStatus::Off,
    })
}
```

### Pattern 2: Per-Minute Cost Calculation (Pure Function)
**What:** Retroactive two-tier pricing computed as a pure function of elapsed minutes.
**When to use:** Every billing tick (1s) for overlay display, and at session end for final charge.
**Why this way:** Pure function is trivially testable, no DB dependency per tick.

```rust
// In billing.rs:
pub struct SessionCost {
    pub total_paise: i64,
    pub rate_per_min_paise: i64,  // current rate (2330 or 1500)
    pub tier_name: &'static str,  // "standard" or "value"
    pub minutes_to_next_tier: Option<u32>, // None if already on value tier
}

/// Compute session cost from elapsed seconds.
/// Two-tier retroactive: <30min = Rs.23.3/min, >=30min = Rs.15/min (entire session).
pub fn compute_session_cost(elapsed_seconds: u32) -> SessionCost {
    let elapsed_minutes = elapsed_seconds as f64 / 60.0;

    if elapsed_seconds >= 1800 { // 30+ minutes
        let cost = (elapsed_minutes * 1500.0).round() as i64; // Rs.15/min in paise
        SessionCost {
            total_paise: cost,
            rate_per_min_paise: 1500,
            tier_name: "value",
            minutes_to_next_tier: None,
        }
    } else {
        let cost = (elapsed_minutes * 2330.0).round() as i64; // Rs.23.3/min in paise
        let minutes_to_value = 30 - (elapsed_seconds / 60);
        SessionCost {
            total_paise: cost,
            rate_per_min_paise: 2330,
            tier_name: "standard",
            minutes_to_next_tier: Some(minutes_to_value),
        }
    }
}
```

### Pattern 3: State Machine for Billing Session
**What:** Explicit state machine governing billing lifecycle on core side.
**States:** `WaitingForGame -> Live -> Paused -> Live -> Ended`

```
WaitingForGame: Game launched, waiting for STATUS=LIVE. No billing timer running.
  -> GameStatusUpdate(AcLive)  => transition to Live, start billing timer
  -> LaunchTimeout (3 min)     => kill AC, retry once or cancel
  -> SessionEndRequest         => cancel (no charge)

Live: Billing timer running, elapsed counting up.
  -> GameStatusUpdate(AcPause) => transition to Paused
  -> SessionEndRequest         => transition to Ended
  -> HardMaxCap reached        => transition to Ended
  -> GameCrashed               => (deferred to mid-session crash recovery)

Paused: Billing frozen, pause timer running.
  -> GameStatusUpdate(AcLive)  => transition to Live, resume billing
  -> PauseTimeout (10 min)     => transition to Ended (auto-end)
  -> SessionEndRequest         => transition to Ended

Ended: Final cost calculated, wallet charged, game killed.
  -> Terminal state
```

### Anti-Patterns to Avoid
- **Starting billing on PIN validation:** Current code in `auth/mod.rs:validate_pin()` calls `billing::start_billing_session()` immediately. This MUST be decoupled -- billing starts on STATUS=LIVE, not on auth.
- **Countdown timer mental model:** The current `allocated_seconds` / `remaining_seconds()` pattern assumes pre-paid fixed duration. Do not try to shoehorn per-minute billing into this model (e.g. setting allocated_seconds to MAX). Use a count-up `elapsed_seconds` field instead.
- **Wallet debit at session start:** Current flow debits wallet when billing starts. With open-ended billing, the charge amount is unknown at start. Debit must happen at session end (or use an authorization/hold pattern if wallet balance is a concern).
- **Treating overlay `game_live` as billing trigger:** The current `game_live` flag uses a telemetry heuristic (speed/RPM/lap_time > 0). This is wrong for billing -- STATUS=LIVE is the authoritative signal. A car can be LIVE but stationary (just placed on track). Use `read_ac_status()` for billing, keep telemetry heuristic as secondary overlay signal.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| AC shared memory reading | Custom FFI bindings | Existing `AssettoCorsaAdapter` with `read_i32(graphics, graphics::STATUS)` | Already handles memory mapping, pointer arithmetic, and cleanup safely |
| Win32 overlay rendering | Browser-based overlay or new framework | Existing `overlay.rs` native Win32 GDI approach | Already working, topmost, 200ms repaint, no browser dependency |
| Agent-Core communication | HTTP polling or new protocol | Existing WebSocket + protocol.rs message enum | Adding a new variant to `AgentMessage` is trivial and type-safe |
| Timer persistence | Custom file-based persistence | Existing `sync_timers_to_db()` (5s interval) + `recover_active_sessions()` | Already handles crash recovery, server restart |
| Per-minute pricing | Complex pricing engine | Pure function `compute_session_cost(elapsed_seconds)` | Two tiers with one threshold is dead simple -- a function, not an engine |

**Key insight:** This phase is a refactor of existing billing infrastructure, not a greenfield build. The shared memory reader, overlay, protocol, and billing timer all exist. The work is changing the billing direction (countdown to count-up) and adding STATUS-awareness.

## Common Pitfalls

### Pitfall 1: AC STATUS Stale Data on Shared Memory Disconnect
**What goes wrong:** When AC crashes or exits, the shared memory mapping may still be accessible but contain stale data (last STATUS before crash).
**Why it happens:** Windows does not zero shared memory on process exit if other processes hold the mapping open.
**How to avoid:** Treat STATUS reading as unreliable if the AC process is not running. Check `game_process.pid` is alive before trusting STATUS value. If game PID is gone, STATUS should be treated as Off regardless of what shared memory says.
**Warning signs:** Billing timer starts or continues after AC has already crashed.

### Pitfall 2: Race Condition Between BillingStarted and LaunchGame
**What goes wrong:** Currently, `BillingStarted` is sent before `LaunchGame`. If we decouple billing from auth, we need to ensure the overlay shows the "WAITING FOR GAME" state correctly.
**Why it happens:** The overlay `activate()` is called on `BillingStarted`. If billing no longer starts at auth, the overlay activation must happen at game launch instead.
**How to avoid:** Send a new message (or repurpose `BillingStarted`) at game launch time to activate the overlay in "waiting" mode. Only switch to "live" mode when STATUS=LIVE is reported.

### Pitfall 3: Pause Flapping (STATUS oscillates between LIVE and PAUSE)
**What goes wrong:** Customer rapidly pressing ESC causes rapid LIVE->PAUSE->LIVE transitions, generating excessive billing events and state changes.
**Why it happens:** AC STATUS updates at ~10Hz. A quick ESC press can appear as multiple transitions.
**How to avoid:** Apply debounce/hysteresis to STATUS transitions. Require STATUS=PAUSE to persist for at least 1-2 seconds before pausing billing. The existing `DrivingDetector` uses a similar hysteresis pattern (10s threshold).
**Warning signs:** Billing events table fills with rapid pause/resume entries.

### Pitfall 4: Wallet Insufficient Balance for Open-Ended Session
**What goes wrong:** Customer starts an open-ended session, drives for 2 hours, and their wallet can't cover the final charge.
**Why it happens:** No upfront debit means no balance check at start.
**How to avoid:** Option A: Check minimum balance at session start (e.g. enough for 5 minutes). Option B: Debit incrementally every N minutes. Option C: Allow negative balance and settle later. **Recommendation:** Check minimum balance (Rs.100 = 10000 paise) at start, then debit at session end. If wallet goes negative, flag for staff resolution. This is simplest and handles 99% of cases since most customers have pre-loaded wallets.

### Pitfall 5: BillingTick Message Protocol Break
**What goes wrong:** Changing `BillingTick` fields breaks communication between old agents and new core (or vice versa during rolling deploy).
**Why it happens:** rc-agent and rc-core are deployed separately. During a rolling update, mixed versions may be running.
**How to avoid:** Add new fields as `Option<T>` with `#[serde(default)]` to maintain backward compatibility. Old agents receiving new fields will ignore unknown fields (serde default behavior). New agents receiving old-format ticks can fall back to remaining_seconds.

### Pitfall 6: Overlay Cost Display Precision
**What goes wrong:** Displaying cost with sub-rupee precision looks unprofessional (e.g. "Rs.349.83").
**Why it happens:** Rs.23.3/min does not divide evenly into whole rupees for most durations.
**How to avoid:** Display in whole rupees rounded down (customer-friendly). Internal tracking stays in paise for precision. Show "Rs.350" not "Rs.349.83". Round up only at session end for final charge.

## Code Examples

### Example 1: AcStatus Enum and SimAdapter Extension
```rust
// rc-common/src/types.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcStatus {
    Off,     // 0 - AC not running or in menu
    Replay,  // 1 - Watching replay
    Live,    // 2 - Car is on track, driving
    Pause,   // 3 - Game paused (ESC menu)
}

// rc-common/src/protocol.rs - new AgentMessage variant
AgentMessage::GameStatusUpdate {
    pod_id: String,
    ac_status: AcStatus,
}
```

### Example 2: Refactored BillingTimer (Count-Up)
```rust
// rc-core/src/billing.rs
pub struct BillingTimer {
    pub session_id: String,
    pub driver_id: String,
    pub driver_name: String,
    pub pod_id: String,
    pub elapsed_seconds: u32,       // Counts UP from 0
    pub pause_seconds: u32,         // Time spent paused (not billed)
    pub status: BillingSessionStatus,
    pub started_at: Option<DateTime<Utc>>,
    pub max_session_seconds: u32,   // Hard cap (e.g. 10800 = 3 hours)
    // ... other fields
}

impl BillingTimer {
    /// Tick the timer by 1 second. Returns true if hard max cap reached.
    pub fn tick(&mut self) -> bool {
        match self.status {
            BillingSessionStatus::Active => {
                self.elapsed_seconds += 1;
                self.elapsed_seconds >= self.max_session_seconds
            }
            BillingSessionStatus::PausedGamePause => {
                self.pause_seconds += 1;
                self.pause_seconds >= 600 // 10-min pause timeout
            }
            _ => false,
        }
    }

    pub fn current_cost(&self) -> SessionCost {
        compute_session_cost(self.elapsed_seconds)
    }
}
```

### Example 3: Updated BillingTick Message
```rust
// rc-common/src/protocol.rs - updated CoreToAgentMessage variant
CoreToAgentMessage::BillingTick {
    // Legacy fields (keep for backward compat during rolling deploy)
    remaining_seconds: u32,
    allocated_seconds: u32,
    driver_name: String,
    // New fields (Option for backward compat)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    elapsed_seconds: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cost_paise: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    rate_per_min_paise: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    paused: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    minutes_to_value_tier: Option<u32>,
}
```

### Example 4: Overlay Taxi Meter Rendering
```rust
// In overlay.rs OverlayData:
struct OverlayData {
    active: bool,
    driver_name: String,
    elapsed_seconds: u32,        // Was: remaining_seconds
    cost_paise: i64,             // Running cost
    rate_per_min_paise: i64,     // Current rate tier
    game_live: bool,             // STATUS=LIVE
    paused: bool,                // STATUS=PAUSE
    waiting_for_game: bool,      // Pre-LIVE state
    minutes_to_value_tier: Option<u32>, // Rate upgrade countdown
    // ... existing telemetry fields unchanged
}

// Display format: "15:23 -- Rs.350"  or  "0:00 -- Rs.0 WAITING FOR GAME"  or  "15:23 -- Rs.350 PAUSED"
```

### Example 5: Launch Timeout State Machine (Agent Side)
```rust
// In rc-agent main.rs, alongside game_process state:
enum LaunchState {
    Idle,
    WaitingForLive {
        launched_at: std::time::Instant,
        attempt: u8, // 1 or 2
    },
    Live,
}

// In main loop:
if let LaunchState::WaitingForLive { launched_at, attempt } = &launch_state {
    if launched_at.elapsed() > Duration::from_secs(180) {
        // 3-minute timeout
        if *attempt < 2 {
            // Kill AC, retry
            // ... kill game, relaunch
            launch_state = LaunchState::WaitingForLive {
                launched_at: std::time::Instant::now(),
                attempt: attempt + 1,
            };
        } else {
            // Both attempts failed - cancel
            // Send GameStatusUpdate { status: LaunchFailed } to core
            launch_state = LaunchState::Idle;
        }
    }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Billing starts at PIN validation | Billing starts at STATUS=LIVE | This phase | Customers not charged for loading time |
| Fixed-duration countdown (30/60 min) | Open-ended per-minute count-up | This phase | No pre-booking, taxi meter UX |
| Flat rate per tier | Retroactive two-tier pricing | This phase | Rewards longer sessions, better economics |
| Wallet debited upfront | Wallet debited at session end | This phase | Supports open-ended billing |
| Overlay shows remaining time | Overlay shows elapsed time + cost | This phase | Taxi meter experience |
| game_live from telemetry heuristic | game_live from STATUS=LIVE | This phase | Authoritative signal, not heuristic |

**Deprecated/outdated after this phase:**
- `allocated_seconds` concept in BillingTimer (replaced by `max_session_seconds` hard cap)
- `remaining_seconds()` method on BillingTimer (replaced by `elapsed_seconds`)
- Upfront wallet debit in `start_billing_session()` (moved to session end)
- `overlay.update_billing(remaining_seconds)` signature (replaced with structured update)

## Open Questions

1. **Wallet balance enforcement strategy**
   - What we know: Current system debits wallet upfront. Open-ended billing has unknown final cost.
   - What's unclear: Should we require a minimum balance at session start? Allow negative balance?
   - Recommendation: Require minimum balance of Rs.100 (10000 paise) at session start. Debit actual amount at session end. If customer lacks minimum, staff can override. This is the simplest approach that prevents abuse without being restrictive.

2. **BillingTick backward compatibility during rolling deploy**
   - What we know: Agent and core are deployed separately. Mixed versions will exist temporarily.
   - What's unclear: How long will the transition period last?
   - Recommendation: Use `Option<T>` with `#[serde(default)]` for all new fields. Keep legacy fields (`remaining_seconds`, `allocated_seconds`) populated with sensible values (remaining_seconds = max_cap - elapsed, allocated_seconds = max_cap). Remove legacy fields in a future cleanup phase.

3. **Dashboard/PWA impact**
   - What we know: `DashboardEvent::BillingTick(BillingSessionInfo)` broadcasts to web dashboards. `BillingSessionInfo` struct needs updating.
   - What's unclear: How much dashboard JS/PWA code depends on `remaining_seconds` and `allocated_seconds` semantics?
   - Recommendation: Update `BillingSessionInfo` to include elapsed, cost, and rate fields. Keep remaining_seconds populated for backward compat. Dashboard updates are Phase 8 scope but the data layer changes here.

4. **Rate upgrade prompt dismissal**
   - What we know: At ~25 min, overlay shows "Drive 5 more minutes to unlock Rs.15/min rate!"
   - What's unclear: How long should the prompt display? Should it auto-dismiss? What if customer is in a tight racing moment?
   - Recommendation: Show for 10 seconds, then auto-dismiss. Re-show briefly at 27 and 29 minutes if still under 30. Keep it non-intrusive (small text below the main timer, not a modal).

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` + cargo test |
| Config file | Cargo.toml per crate |
| Quick run command | `cargo test -p rc-common && cargo test -p rc-core -- billing && cargo test -p rc-agent -- overlay` |
| Full suite command | `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p rc-core` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| BILL-01 | Billing starts only on STATUS=LIVE, not at game launch | unit | `cargo test -p rc-core -- billing::tests::billing_starts_on_live_only -x` | Wave 0 |
| BILL-01 | 3-min launch timeout triggers retry | unit | `cargo test -p rc-core -- billing::tests::launch_timeout_triggers_retry -x` | Wave 0 |
| BILL-01 | Second launch failure cancels session (no charge) | unit | `cargo test -p rc-core -- billing::tests::double_failure_cancels_no_charge -x` | Wave 0 |
| BILL-02 | Elapsed stays 0 during WaitingForGame state | unit | `cargo test -p rc-core -- billing::tests::no_billing_during_waiting -x` | Wave 0 |
| BILL-06 | Overlay shows elapsed + cost in taxi meter format | unit | `cargo test -p rc-agent -- overlay::tests::taxi_meter_display -x` | Wave 0 |
| BILL-06 | Overlay shows PAUSED badge when paused | unit | `cargo test -p rc-agent -- overlay::tests::paused_badge_display -x` | Wave 0 |
| BILL-06 | Overlay shows WAITING FOR GAME during loading | unit | `cargo test -p rc-agent -- overlay::tests::waiting_for_game_display -x` | Wave 0 |
| ALL | compute_session_cost returns correct amounts for both tiers | unit | `cargo test -p rc-core -- billing::tests::cost_calculation -x` | Wave 0 |
| ALL | compute_session_cost retroactive tier crossing | unit | `cargo test -p rc-core -- billing::tests::retroactive_tier_crossing -x` | Wave 0 |
| ALL | BillingTimer count-up tick increments elapsed | unit | `cargo test -p rc-core -- billing::tests::timer_counts_up -x` | Wave 0 |
| ALL | Pause freezes elapsed, increments pause_seconds | unit | `cargo test -p rc-core -- billing::tests::pause_freezes_elapsed -x` | Wave 0 |
| ALL | 10-min pause timeout auto-ends session | unit | `cargo test -p rc-core -- billing::tests::pause_timeout_auto_end -x` | Wave 0 |
| ALL | Hard max cap (3h) auto-ends session | unit | `cargo test -p rc-core -- billing::tests::hard_max_cap_auto_end -x` | Wave 0 |
| ALL | AcStatus read from shared memory | unit | `cargo test -p rc-agent -- assetto_corsa::tests::ac_status_read -x` | Wave 0 |
| ALL | Protocol roundtrip: GameStatusUpdate | unit | `cargo test -p rc-common -- protocol::tests::game_status_update_roundtrip -x` | Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p rc-common && cargo test -p rc-core -- billing && cargo test -p rc-agent -- overlay`
- **Per wave merge:** `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p rc-core`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `billing::tests::billing_starts_on_live_only` -- new test for STATUS-triggered billing
- [ ] `billing::tests::cost_calculation` -- per-minute cost calculation tests (both tiers + boundary)
- [ ] `billing::tests::retroactive_tier_crossing` -- verify rate drops at 30-min threshold
- [ ] `billing::tests::timer_counts_up` -- verify elapsed increments instead of remaining decrements
- [ ] `billing::tests::pause_freezes_elapsed` -- game pause behavior
- [ ] `billing::tests::pause_timeout_auto_end` -- 10-min pause timeout
- [ ] `billing::tests::hard_max_cap_auto_end` -- 3-hour cap
- [ ] `billing::tests::launch_timeout_triggers_retry` -- launch failure handling
- [ ] `billing::tests::double_failure_cancels_no_charge` -- double failure = no charge
- [ ] `billing::tests::no_billing_during_waiting` -- WaitingForGame state
- [ ] `protocol::tests::game_status_update_roundtrip` -- new message roundtrip
- [ ] `overlay::tests::taxi_meter_display` -- overlay rendering with elapsed + cost
- [ ] `overlay::tests::paused_badge_display` -- PAUSED overlay state
- [ ] `overlay::tests::waiting_for_game_display` -- WAITING overlay state
- [ ] `assetto_corsa::tests::ac_status_read` -- STATUS field reading (cfg(not(windows)) stub)

Existing tests that must still pass (regression):
- 6 existing billing tests in `rc-core/src/billing.rs` (will need updating for count-up model)
- 5 existing overlay tests in `rc-agent/src/overlay.rs`
- All protocol roundtrip tests in `rc-common/src/protocol.rs`

## Sources

### Primary (HIGH confidence)
- Codebase inspection: `rc-core/src/billing.rs` (1782 lines) -- full billing lifecycle, BillingTimer, tick loop, tests
- Codebase inspection: `rc-agent/src/overlay.rs` (~800 lines) -- native Win32 overlay, OverlayData, rendering
- Codebase inspection: `rc-agent/src/sims/assetto_corsa.rs` (431 lines) -- AC shared memory reader with STATUS offset
- Codebase inspection: `rc-common/src/protocol.rs` (943 lines) -- all protocol messages
- Codebase inspection: `rc-common/src/types.rs` (914 lines) -- BillingSessionInfo, PricingTier, etc.
- Codebase inspection: `rc-core/src/auth/mod.rs` -- PIN validation flow, billing start trigger
- Codebase inspection: `rc-core/src/db/mod.rs` -- billing_sessions, pricing_tiers schema
- AC Shared Memory docs reference (in assetto_corsa.rs comments): https://www.assettocorsa.net/forum/index.php?threads/shared-memory-reference.3352/

### Secondary (MEDIUM confidence)
- CONTEXT.md user decisions (2026-03-13) -- locked billing model, pricing tiers, UX decisions
- REQUIREMENTS.md (2026-03-13) -- BILL-01, BILL-02, BILL-06 requirement text
- STATE.md (2026-03-13) -- project history, accumulated decisions from Phases 1-2

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all libraries and patterns are already in the codebase; this is a refactor
- Architecture: HIGH -- clear separation of concerns (agent reads STATUS, core owns billing, agent renders overlay)
- Pitfalls: HIGH -- identified from direct codebase analysis (stale shared memory, race conditions, wallet balance)
- Pricing calculation: HIGH -- two-tier retroactive pricing is simple arithmetic, verified with examples from CONTEXT.md
- Protocol compatibility: MEDIUM -- rolling deploy backward compat strategy is sound but untested

**Research date:** 2026-03-13
**Valid until:** 2026-04-13 (stable -- internal codebase, no external dependency changes expected)
