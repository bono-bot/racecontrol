# Phase 82: Billing and Session Lifecycle - Research

**Researched:** 2026-03-21
**Domain:** Rust billing engine (billing.rs, billing_guard.rs, event_loop.rs) + Next.js admin UI (web/billing/pricing)
**Confidence:** HIGH — all findings from direct codebase inspection of canonical files

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**PlayableSignal Design**
- Hybrid approach: use telemetry signal if adapter exists, fall back to process detection + 90s delay if no adapter
- AC: existing `AcStatus::Live` from shared memory (unchanged)
- F1 25: wait for first UDP session packet on port 20777 — no billing during shader compilation (3-5 min first launch)
- iRacing, LMU, EVO, WRC: process-based fallback (90s after exe detected) until their telemetry adapters are built in Phases 83-87
- Once a telemetry adapter exists for a game, it overrides the process-based fallback automatically
- Generalize `handle_game_status_update()` from AC-only `AcStatus::Live` to accept a generic "playable" signal per sim type

**Per-Game Billing Rates**
- Add `sim_type` column to `billing_rates` table — each tier has a different rate per game
- Example: Standard tier F1 25 = 5 credits/min, Standard tier iRacing = 7 credits/min
- Admin UI: add game column to existing Per-Minute Rates table in admin dashboard
- `BillingManager` rate cache must be extended to hold per-game rates
- `compute_session_cost()` must accept sim_type parameter to look up correct rate
- `billing_rates` already in `SYNC_TABLES` for cloud replication — sim_type column syncs automatically

**Session Lifecycle States**
- Show Loading state in kiosk — pod card shows "Loading F1 25..." with timer counting up; staff can see billing hasn't started yet
- This requires a new state or sub-state visible in kiosk (distinct from "Launching")
- 30s grace period on exit — when game process exits, wait 30s before ending billing session; avoids fragmenting sessions on accidental exit or quick crash recovery relaunch
- If game relaunches within grace period (crash recovery), billing continues seamlessly
- Full lifecycle observable in logs and kiosk: launch > loading > playable (billing starts) > gameplay > exit (30s grace) > cleanup

**Edge Cases**
- Alt-Tab / idle: if no input for 5+ minutes, pause billing and alert staff; uses existing DrivingDetector idle detection
- Telemetry drops: use existing DrivingDetector idle thresholds to handle brief telemetry gaps without pausing billing
- Shader compilation: F1 25 UDP signal handles this; other games use 90s process fallback

### Claude's Discretion
- Exact implementation of the generic PlayableSignal interface (trait vs enum vs callback)
- How to surface "Loading" vs "Launching" in the kiosk (new GameState variant vs sub-state on existing)
- DrivingDetector threshold for telemetry drop tolerance
- DB migration strategy for adding sim_type to billing_rates (default value for existing rows)
- Admin UI layout for per-game rate editing (inline per row vs separate per-game view)

### Deferred Ideas (OUT OF SCOPE)

None — discussion stayed within phase scope
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| BILL-01 | Billing starts when game is playable (PlayableSignal), not at process launch | `defer_billing_start()` + `handle_game_status_update()` pattern already exists; needs generalization beyond AC-only `AcStatus::Live` |
| BILL-02 | Per-game PlayableSignal: F1 25 (UDP session type), iRacing/LMU/EVO/WRC (process-based 90s fallback) | `event_loop.rs` already reads AC shared memory for `AcStatus::Live`; need per-sim dispatch branch in telemetry interval + new UDP listener for F1 25 port 20777 |
| BILL-03 | Per-game billing rates configurable in billing_rates table | DB schema needs `sim_type TEXT` column with default `'all'`; `BillingManager.rate_tiers` cache and `refresh_rate_tiers()` need per-game keying; `compute_session_cost()` needs `sim_type` param |
| BILL-04 | Billing auto-stops on game exit, crash, or session end | `AcStatus::Off` path in `handle_game_status_update()` already ends sessions; needs 30s grace period timer before calling `end_billing_session()` |
| BILL-05 | Session lifecycle: launch -> loading -> playable (billing starts) -> gameplay -> exit (billing stops) -> cleanup | Need new `Loading` state in `GameState` enum (or sub-state); need kiosk pod card display; lifecycle observable in structured logs |
</phase_requirements>

---

## Summary

Phase 82 builds on a rich, already-working billing engine. The core `defer_billing_start()` + `handle_game_status_update(AcStatus::Live)` pattern works perfectly for AC but is AC-specific throughout. The primary task is generalizing this pattern to accept a per-sim "PlayableSignal" — replacing the hardcoded `AcStatus::Live` check in `event_loop.rs` (line ~194) with a per-sim dispatch that emits the same WS `GameStatusUpdate` to the server, which then calls the same `handle_game_status_update()` function.

The billing rate table (`billing_rates`) currently has no `sim_type` column. Adding it as a nullable TEXT with SQLite `ALTER TABLE ... ADD COLUMN` (safe, no rebuild) and updating `refresh_rate_tiers()` and `compute_session_cost()` is the DB work. The admin UI for billing rates lives in `web/src/app/billing/pricing/page.tsx` and already has inline row editing — the game column extends that pattern.

The `GameState` enum in `rc-common/src/types.rs` currently has `Idle | Launching | Running | Stopping | Error`. Adding a `Loading` variant (between Launching and Running) is the cleanest way to surface the "billing hasn't started yet" state to the kiosk pod card. The 30s grace period on exit needs a new timer in `CrashRecoveryState` or a separate grace-period timer in `ConnectionState`.

**Primary recommendation:** Implement as 3 plans: (1) PlayableSignal generalization in rc-agent/rc-common + F1 25 UDP listener, (2) per-game billing rates DB migration + BillingManager cache + admin UI, (3) session lifecycle states (Loading GameState, 30s grace, kiosk display).

---

## Standard Stack

### Core (all already in use — no new dependencies)

| Component | Location | Version/Notes | Purpose |
|-----------|----------|--------------|---------|
| SQLite via sqlx | `crates/racecontrol/Cargo.toml` | SQLx 0.7.x | `ALTER TABLE billing_rates ADD COLUMN sim_type` — safe incremental migration |
| tokio::time::Sleep | `crates/rc-agent/src/event_loop.rs` | already used | 30s grace timer, 90s process fallback timer |
| tokio::net::UdpSocket | Standard tokio | already in project | F1 25 UDP listener on port 20777 for PlayableSignal |
| rc-common types | `crates/rc-common/src/types.rs` | project-internal | Add `Loading` to GameState enum, add PlayableSignal enum |
| Next.js admin UI | `web/src/app/billing/pricing/page.tsx` | existing | Extend inline edit table to include sim_type column |

### No New Dependencies Required

The entire phase uses existing crate dependencies. No new crates need to be added to any `Cargo.toml`.

---

## Architecture Patterns

### Recommended Change Map

```
rc-common/src/types.rs
├── GameState::Loading (new variant, between Launching and Running)
├── PlayableSignal enum (new: TelemetryLive { sim_type }, ProcessFallback { sim_type })
└── AgentMessage::GameStatusUpdate — keep existing, add sim_type field (Option<SimType>)

rc-agent/src/event_loop.rs
├── ConnectionState: add loading_timer (30s process fallback), grace_timer (30s on exit)
├── telemetry interval: per-sim dispatch (AC unchanged, F1 25 checks UDP listener result)
├── game_check interval: process-based fallback counter — when exe found + 90s elapsed → emit PlayableSignal
└── AcStatus::Off path: start grace_timer instead of immediate end

crates/racecontrol/src/billing.rs
├── BillingRateTier: add sim_type: Option<SimType> field
├── BillingManager.rate_tiers: HashMap<Option<SimType>, Vec<BillingRateTier>> (None = universal)
├── refresh_rate_tiers(): SELECT including sim_type column, group by sim_type
└── compute_session_cost(): accepts sim_type, looks up sim-specific tiers or falls back to universal

crates/racecontrol/src/db/mod.rs
└── Migration: ALTER TABLE billing_rates ADD COLUMN sim_type TEXT (nullable, None = applies to all games)

crates/racecontrol/src/api/routes.rs
└── list/create/update_billing_rate: include sim_type in SELECT/INSERT/UPDATE

web/src/app/billing/pricing/page.tsx
└── Add "Game" column to Per-Minute Rates table — inline select from SimType enum values

kiosk/src/components/KioskPodCard.tsx
└── Handle game_state === "loading" — show "Loading {game}..." with elapsed timer badge
```

### Pattern 1: PlayableSignal Generalization

**What:** Replace the AC-only `AcStatus::Live` check in `event_loop.rs` (telemetry interval, ~line 194) with a per-sim dispatch. AC continues using shared memory. F1 25 uses UDP port 20777. Others use process-based 90s fallback.

**When to use:** Every non-AC sim launch from Phase 81 onward.

**The key insight:** The server-side `handle_game_status_update()` only cares that it receives `AcStatus::Live` (or an equivalent "now billing" signal). The agent controls WHEN this signal is emitted. So the generalization lives entirely in the agent — the server API is unchanged.

```rust
// event_loop.rs — conceptual per-sim playable dispatch
// (within game_check interval or telemetry interval)
match current_sim {
    SimType::AssettoCorsa => {
        // Unchanged: AcStatus::Live from shared memory triggers GameStatusUpdate
        // (existing code in telemetry_interval already handles this)
    }
    SimType::F125 => {
        // UDP listener on port 20777 detected first session packet
        if state.f1_udp_ready {
            emit_playable_signal(&mut ws_tx, &state.pod_id, SimType::F125).await;
            conn.launch_state = LaunchState::Live;
        }
    }
    SimType::IRacing | SimType::LeMansUltimate | SimType::AssettoCorsaEvo | SimType::AssettoCorsaRally => {
        // Process-based fallback: exe detected + 90s elapsed
        if let LaunchState::WaitingForLive { launched_at, .. } = &conn.launch_state {
            if launched_at.elapsed() >= Duration::from_secs(90) {
                emit_playable_signal(&mut ws_tx, &state.pod_id, sim_type).await;
                conn.launch_state = LaunchState::Live;
            }
        }
    }
}
```

### Pattern 2: DB Migration for sim_type Column

**What:** SQLite `ALTER TABLE ... ADD COLUMN` is safe on a live table — adds nullable column, existing rows get NULL (which means "applies to all games").

```sql
-- Safe: SQLite supports ADD COLUMN without table rebuild
ALTER TABLE billing_rates ADD COLUMN sim_type TEXT;

-- Seed per-game rates if needed (INSERT OR IGNORE, idempotent):
INSERT OR IGNORE INTO billing_rates (id, tier_order, tier_name, threshold_minutes, rate_per_min_paise, sim_type)
VALUES ('rate_f125_standard', 1, 'Standard', 30, 2500, 'f1_25');
```

**Default value strategy:** `sim_type = NULL` means "universal rate, applies when no game-specific rate exists." `refresh_rate_tiers()` loads by sim_type; `compute_session_cost()` tries game-specific tiers first, falls back to universal (NULL) tiers.

### Pattern 3: 30s Grace Period Timer

**What:** When game process exits, start a 30s sleep before calling `end_billing_session`. If game relaunches within grace period (crash recovery already handles relaunch), cancel the timer.

**Implementation:** Add `grace_timer: Option<Pin<Box<Sleep>>>` to `ConnectionState`. Set it on `AcStatus::Off` / process exit. Clear it when crash recovery relaunches successfully. If timer fires without relaunch, emit `AcStatus::Off` to the server.

**Interaction with existing crash recovery:** `CrashRecoveryState::PausedWaitingRelaunch` already pauses billing with `billing_paused = true` and uses a 60s timer per attempt. The 30s grace sits BEFORE crash recovery engages — it gives 30s for a quick crash recovery relaunch before escalating to `CrashRecoveryState`.

### Pattern 4: Loading GameState

**What:** Add `Loading` variant to `GameState` enum in `rc-common/src/types.rs`. Agent emits `GameState::Loading` immediately when process is detected (exe running) but PlayableSignal not yet fired. Kiosk shows "Loading F1 25..." with count-up timer.

**Serde:** `GameState` uses `#[serde(rename_all = "snake_case")]` — new variant serializes as `"loading"`. Backward-compatible (old agents without this variant will use unknown string, kiosk falls back to existing badge).

```rust
// rc-common/src/types.rs
pub enum GameState {
    Idle,
    Launching,  // process not yet detected
    Loading,    // process detected, waiting for PlayableSignal (NEW)
    Running,    // PlayableSignal received — billing active
    Stopping,
    Error,
}
```

### Anti-Patterns to Avoid

- **Billing timer on process launch:** Never call `defer_billing_start()` at game exe spawn — only on PlayableSignal. Phase 81 does NOT call defer_billing_start; that call stays in the auth/session-start flow. The PlayableSignal just unlocks the already-deferred entry.
- **Changing AgentMessage::GameStatusUpdate protocol:** The server already handles this message. Do NOT change its shape — if sim_type context is needed server-side, add it as an optional field with `#[serde(default)]` backward compat.
- **Separate rate_tiers cache per sim_type:** Use a flat `Vec<BillingRateTier>` with a `sim_type: Option<SimType>` field, not a HashMap keyed by SimType — the compute function already handles iteration and can filter by sim_type in a single pass.
- **Running UDP socket in main select! loop:** Spawn the F1 25 UDP listener as a separate tokio task (like telemetry adapters), communicate result via channel or AppState flag — do NOT add a UDP socket directly to the select! arms.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| 30s grace timer | Custom timer struct | `tokio::time::sleep()` wrapped in `Box::pin()` — same as existing `blank_timer` and `CrashRecoveryState` timers | Already the project pattern in `ConnectionState` |
| F1 25 UDP "playable" detection | Custom packet parser | Bind `UdpSocket` on port 20777, receive first packet — any packet = session started (Phase 83 handles full parsing) | Port 20777 is ONLY active during live F1 25 session, not menus/shaders |
| Per-game rate lookup | HashMap<SimType, Vec<tier>> | Flat Vec with `sim_type: Option<SimType>`, filter in `compute_session_cost()` | Simpler, matches existing BillingRateTier struct shape |
| DB migration | sqlx migrate files | Inline `ALTER TABLE ... ADD COLUMN` in `init_db()` wrapped in `IF NOT EXISTS` check | SQLite supports this; project currently uses inline DDL in `db/mod.rs` |

---

## Common Pitfalls

### Pitfall 1: AcStatus::Off Fires on Normal Pause-Menu Navigation

**What goes wrong:** Some sims send process exit on error + immediate relaunch. Without the 30s grace period, billing ends and the customer is charged for a partial session, then a new session starts.

**Why it happens:** The existing `AcStatus::Off` handler in `handle_game_status_update()` calls `end_billing_session()` immediately. Phase 81 crash recovery already handles the relaunch, but the billing end happens before relaunch is detected.

**How to avoid:** The 30s grace timer in the agent delays the `AcStatus::Off` signal to the server. `billing_paused = true` (set by crash recovery) suppresses anomaly detection during this window. Grace timer is ONLY in the agent; server side is unchanged.

**Warning signs:** Sessions in DB with `driving_seconds < 10` followed immediately by a new session for the same driver on the same pod.

### Pitfall 2: 90s Process Fallback Starts Before User Has Control

**What goes wrong:** The 90s clock starts when the exe is detected in the process list. For iRacing, the exe may load 20s before the user has the wheel in hand. This means 70s of "loading" time is billed.

**Why it happens:** Process detection is a weak signal — it doesn't know if the game is showing a loading splash or the actual game.

**How to avoid:** 90s is deliberately conservative. CONTEXT.md explicitly accepts this tradeoff: "better to miss 90s of billing than to charge during loading screens." The billing rate means a customer pays ~90 × rate_per_min_paise / 60 extra in worst case. This will be replaced per-game in Phases 83-87 when telemetry adapters ship.

**Warning signs:** If 90s proves too short (game still loading at 90s), the config should allow per-game override. Consider adding `playable_delay_secs` to game profile config.

### Pitfall 3: sim_type NULL in billing_rates Breaks compute_session_cost Lookup

**What goes wrong:** After adding `sim_type` column, existing rates have `sim_type = NULL`. If `compute_session_cost()` only looks up by exact sim_type match, existing rates never match any game and billing returns 0.

**Why it happens:** SQL NULL != NULL, so `WHERE sim_type = ?` with NULL param matches nothing.

**How to avoid:** The lookup logic must be: first try `WHERE sim_type = ?` for the specific game; if no rows, fall back to `WHERE sim_type IS NULL`. Alternatively, load all rates into memory and filter in Rust code — already the pattern since `rate_tiers` is an in-memory Vec.

**Warning signs:** Sessions showing `cost_paise = 0` or `rate_per_min_paise = 0` in BillingTick messages after migration.

### Pitfall 4: F1 25 UDP Port Already Bound by Telemetry Adapter (Phase 83)

**What goes wrong:** Phase 82 binds port 20777 for PlayableSignal detection. Phase 83 will also need port 20777 for full telemetry. Two tasks binding the same port will conflict.

**Why it happens:** UDP `SO_REUSEPORT` is needed for multiple consumers on the same port, but this is complex and platform-specific.

**How to avoid:** Design the Phase 82 F1 25 listener so it is REPLACED (not added to) by the Phase 83 telemetry adapter. When the F1 25 telemetry adapter initializes, it takes ownership of port 20777 and emits the PlayableSignal itself. Phase 82 listener is the temporary implementation until Phase 83 exists. Use a `f1_playable_rx: Option<watch::Receiver<bool>>` in AppState — Phase 82 sets this up with a simple UDP task, Phase 83 replaces the task with the full adapter.

**Warning signs:** `Address already in use` error when loading Phase 83 telemetry adapter.

### Pitfall 5: GameState::Loading Not Backward-Compatible with Kiosk

**What goes wrong:** Old kiosk JavaScript code uses `if (gameInfo?.game_state === "running")` etc. A new `"loading"` value falls through all conditions and the pod card shows wrong state.

**Why it happens:** TypeScript/JS string comparison — unknown enum variant just doesn't match any case.

**How to avoid:** Add explicit `"loading"` handling to `KioskPodCard.tsx` in the same plan as the Rust enum change. The `getPodDisplayState()` function in `KioskPodCard.tsx` (line 87) must handle `"loading"` before rolling out to production. The state badge mapping at line 785 must also include `"loading"`.

---

## Code Examples

Verified patterns from codebase inspection:

### Existing WaitingForGame Pattern (billing.rs)

```rust
// Source: crates/racecontrol/src/billing.rs
// This pattern is REUSED — generalize only the trigger condition

// Step 1: On session auth (already done in Phase 81's auth flow)
defer_billing_start(state, pod_id, driver_id, pricing_tier_id, ...).await;
// → adds WaitingForGameEntry to billing.waiting_for_game

// Step 2: On PlayableSignal (currently AC-only, Phase 82 generalizes this)
handle_game_status_update(state, pod_id, AcStatus::Live, cmd_tx).await;
// → removes from waiting_for_game, calls start_billing_session()
```

### Existing Grace Timer Pattern (event_loop.rs ConnectionState)

```rust
// Source: crates/rc-agent/src/event_loop.rs — blank_timer as reference
// Same Box::pin(tokio::time::sleep(...)) pattern for grace period:
pub(crate) struct ConnectionState {
    // ... existing fields ...
    pub(crate) blank_timer: std::pin::Pin<Box<tokio::time::Sleep>>,
    pub(crate) blank_timer_armed: bool,
    // NEW for Phase 82:
    pub(crate) exit_grace_timer: std::pin::Pin<Box<tokio::time::Sleep>>,
    pub(crate) exit_grace_armed: bool,
}
// In select! loop, add arm:
// _ = &mut conn.exit_grace_timer, if conn.exit_grace_armed => { ... emit AcStatus::Off to server }
```

### Existing Billing Rate Refresh (billing.rs)

```rust
// Source: crates/racecontrol/src/billing.rs
pub async fn refresh_rate_tiers(state: &Arc<AppState>) {
    let rows = sqlx::query_as::<_, (i64, String, i64, i64)>(
        "SELECT tier_order, tier_name, threshold_minutes, rate_per_min_paise
         FROM billing_rates WHERE is_active = 1 ORDER BY tier_order ASC",
    )
    // Phase 82: extend to (i64, String, i64, i64, Option<String>) — add sim_type
    // SELECT tier_order, tier_name, threshold_minutes, rate_per_min_paise, sim_type
}
```

### Existing GameState Display in KioskPodCard.tsx

```typescript
// Source: kiosk/src/components/KioskPodCard.tsx line 87-99
// Phase 82: add "loading" before "running" check
if (gameInfo?.game_state === "loading") return "loading";      // NEW
if (gameInfo?.game_state === "running") return "on_track";
if (gameInfo?.game_state === "launching") return "selecting";
// ...
// And in the status badge map (line 785):
// loading: "Loading...",   // NEW
```

### DB Migration Pattern (db/mod.rs style)

```sql
-- Safe SQLite incremental migration — add to init_db() after existing CREATE TABLE
-- SQLite ADD COLUMN cannot have NOT NULL without DEFAULT, so use nullable (correct here)
ALTER TABLE billing_rates ADD COLUMN sim_type TEXT;
-- No data migration needed: NULL = universal rate (apply to all games)
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Billing starts on game process spawn | Billing deferred until `AcStatus::Live` | Phase 03 | WaitingForGame pattern in billing.rs |
| AC-only billing trigger | Per-sim PlayableSignal | Phase 82 (this phase) | Generalizes `handle_game_status_update()` |
| Universal billing rate | Per-game billing rate | Phase 82 (this phase) | `sim_type` column on `billing_rates` |
| No "Loading" state | `GameState::Loading` (between Launching and Running) | Phase 82 (this phase) | Kiosk can show "Loading F1 25..." |

**Deprecated/outdated:**
- Hardcoded `AcStatus::Live` check in `event_loop.rs` line ~194: after Phase 82, this becomes `SimType::AssettoCorsa` branch of a per-sim dispatch

---

## Open Questions

1. **Where does defer_billing_start() get called for non-AC games?**
   - What we know: For AC, it's called in the auth/session-start flow (Phase 03). Phase 81 added non-AC game launch but the billing deferral is already in place before launch.
   - What's unclear: Does Phase 81's `LaunchGame` handler call `defer_billing_start()`? Need to verify the non-AC launch path calls it.
   - Recommendation: Verify that `defer_billing_start()` is called for all sim types in the launch/auth flow. If not, the WaitingForGame entry won't exist when PlayableSignal fires, and billing silently won't start.

2. **F1 25 port 20777 — agent receives on that port or does the server receive?**
   - What we know: CLAUDE.md states UDP telemetry ports include 20777 (F1). The DrivingDetector already monitors this port.
   - What's unclear: The `driving_detector.rs` `DetectorConfig` lists port 20777 in `telemetry_ports` — meaning the agent already listens on 20777. A PlayableSignal-only listener can reuse this UDP socket or hook into the existing detector.
   - Recommendation: Check if `driving_detector.rs` already binds port 20777 on the agent. If yes, PlayableSignal for F1 25 can be derived from the first `UdpActive` signal from the detector rather than a separate socket bind.

3. **Should `compute_session_cost()` signature change or add a new overload?**
   - What we know: `compute_session_cost(elapsed_seconds, tiers)` is called in ~7 places in billing.rs. Adding sim_type would require updating all call sites.
   - What's unclear: Whether existing call sites know the sim_type or if it needs to flow through from BillingTimer.
   - Recommendation: Add `sim_type: Option<SimType>` to `BillingTimer` when `start_billing_session()` is called. The compute function signature stays the same — callers pass already-filtered tiers. Rate lookup happens in `billing_tick()` which reads from BillingManager.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` + `#[tokio::test]` via tokio |
| Config file | None — inline `#[cfg(test)]` modules in each .rs file |
| Quick run command | `cargo test -p rc-agent -p racecontrol billing -- --nocapture` |
| Full suite command | `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p racecontrol` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| BILL-01 | PlayableSignal fires billing start (not process launch) | unit | `cargo test -p racecontrol billing::tests::bill01` | ❌ Wave 0 |
| BILL-02 | Per-sim PlayableSignal: F1 25 UDP, others 90s fallback | unit | `cargo test -p rc-agent playable_signal` | ❌ Wave 0 |
| BILL-03 | Per-game billing rate lookup from DB cache | unit | `cargo test -p racecontrol billing::tests::per_game_rate` | ❌ Wave 0 |
| BILL-04 | Billing ends on exit (with 30s grace) | unit | `cargo test -p racecontrol billing::tests::bill04_grace` | ❌ Wave 0 |
| BILL-05 | Lifecycle state transitions observable in logs | integration | `cargo test -p racecontrol billing::tests::bill05_lifecycle` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p racecontrol billing -- --nocapture`
- **Per wave merge:** `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p racecontrol`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] Characterization tests for new `compute_session_cost()` with `sim_type` parameter — covers BILL-03
- [ ] Unit tests for 30s grace period timer in ConnectionState — covers BILL-04
- [ ] Unit test: `GameState::Loading` serialization roundtrip — covers BILL-05
- [ ] Test: `PlayableSignal::ProcessFallback` after 90s elapsed — covers BILL-02

---

## Sources

### Primary (HIGH confidence)
- `crates/racecontrol/src/billing.rs` — full billing engine, WaitingForGame pattern, compute_session_cost, DB schema, rate tiers
- `crates/rc-agent/src/billing_guard.rs` — anomaly detection, billing_paused suppression pattern
- `crates/rc-agent/src/driving_detector.rs` — DetectorSignal, idle thresholds, port 20777 already monitored
- `crates/rc-agent/src/event_loop.rs` — ConnectionState, LaunchState, CrashRecoveryState, blank_timer pattern
- `crates/rc-common/src/types.rs` — GameState enum, AcStatus, SimType, BillingSessionStatus
- `crates/rc-agent/src/ws_handler.rs` — BillingStarted handler, billing state in agent
- `crates/racecontrol/src/db/mod.rs` — billing_rates CREATE TABLE, existing schema
- `crates/racecontrol/src/api/routes.rs` — list/create/update_billing_rate endpoints
- `web/src/app/billing/pricing/page.tsx` — existing Per-Minute Rates admin UI (inline edit pattern)
- `web/src/lib/api.ts` — BillingRate TypeScript interface
- `kiosk/src/components/KioskPodCard.tsx` — GameState display, getPodDisplayState(), badge map
- `crates/racecontrol/src/cloud_sync.rs` — SYNC_TABLES confirms billing_rates auto-syncs

### Secondary (MEDIUM confidence)
- `.planning/phases/82-billing-and-session-lifecycle/82-CONTEXT.md` — locked decisions from user discussion

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all existing crates, no new dependencies
- Architecture: HIGH — direct codebase inspection of all canonical files
- Pitfalls: HIGH — port conflict and NULL lookup pitfalls verified against actual code

**Research date:** 2026-03-21 IST
**Valid until:** 2026-04-20 (stable codebase, no fast-moving external dependencies)
