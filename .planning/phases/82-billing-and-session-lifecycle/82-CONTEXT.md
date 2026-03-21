# Phase 82: Billing and Session Lifecycle - Context

**Gathered:** 2026-03-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Customers are charged only for actual gameplay time. Billing starts when the game reports a playable state (PlayableSignal), not during loading or shader compilation. Per-game billing rates are configurable. Billing auto-stops on exit/crash with a 30s grace period. The full session lifecycle (launch > loading > playable > gameplay > exit > cleanup) is visible in the kiosk.

</domain>

<decisions>
## Implementation Decisions

### PlayableSignal Design
- **Hybrid approach:** Use telemetry signal if adapter exists, fall back to process detection + 90s delay if no adapter
- AC: existing `AcStatus::Live` from shared memory (unchanged)
- F1 25: wait for first UDP session packet on port 20777 — no billing during shader compilation (3-5 min first launch)
- iRacing, LMU, EVO, WRC: process-based fallback (90s after exe detected) until their telemetry adapters are built in Phases 83-87
- Once a telemetry adapter exists for a game, it overrides the process-based fallback automatically
- Generalize `handle_game_status_update()` from AC-only `AcStatus::Live` to accept a generic "playable" signal per sim type

### Per-Game Billing Rates
- Add `sim_type` column to `billing_rates` table — each tier has a different rate per game
- Example: Standard tier F1 25 = 5 credits/min, Standard tier iRacing = 7 credits/min
- Admin UI: add game column to existing Per-Minute Rates table in admin dashboard
- `BillingManager` rate cache must be extended to hold per-game rates
- `compute_session_cost()` must accept sim_type parameter to look up correct rate
- `billing_rates` already in `SYNC_TABLES` for cloud replication — sim_type column syncs automatically

### Session Lifecycle States
- **Show Loading state in kiosk** — pod card shows "Loading F1 25..." with timer counting up. Staff can see billing hasn't started yet
- This requires a new state or sub-state visible in kiosk (distinct from "Launching")
- **30s grace period on exit** — when game process exits, wait 30s before ending billing session. Avoids fragmenting sessions on accidental exit or quick crash recovery relaunch
- If game relaunches within grace period (crash recovery), billing continues seamlessly
- Full lifecycle observable in logs and kiosk: launch > loading > playable (billing starts) > gameplay > exit (30s grace) > cleanup

### Edge Cases
- **Alt-Tab / idle:** If no input for 5+ minutes, pause billing and alert staff. Uses existing DrivingDetector idle detection
- **Telemetry drops:** Claude's Discretion — use existing DrivingDetector idle thresholds to handle brief telemetry gaps without pausing billing
- **Shader compilation:** F1 25 UDP signal handles this. Other games use 90s process fallback which covers most load times

### Claude's Discretion
- Exact implementation of the generic PlayableSignal interface (trait vs enum vs callback)
- How to surface "Loading" vs "Launching" in the kiosk (new GameState variant vs sub-state on existing)
- DrivingDetector threshold for telemetry drop tolerance
- DB migration strategy for adding sim_type to billing_rates (default value for existing rows)
- Admin UI layout for per-game rate editing (inline per row vs separate per-game view)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Billing Infrastructure
- `crates/racecontrol/src/billing.rs` — BillingManager, compute_session_cost(), defer_billing_start(), handle_game_status_update() (currently AC-only via AcStatus::Live), billing_rates DB table, WaitingForGameEntry
- `crates/rc-agent/src/billing_guard.rs` — Billing anomaly detection, orphan auto-end, idle drift detection
- `crates/rc-agent/src/driving_detector.rs` — DrivingDetector with DetectorSignal (HidActive, UdpActive, HidIdle, UdpIdle, HidDisconnected), idle threshold, DrivingState enum

### Session Lifecycle
- `crates/rc-agent/src/event_loop.rs` — Game state machine, crash recovery, PlayableSignal consumption point
- `crates/rc-agent/src/ws_handler.rs` — BillingStarted message handler (line 134)
- `crates/racecontrol/src/game_launcher.rs` — launch_game(), handle_game_state_update()

### Shared Types
- `crates/rc-common/src/types.rs` — AcStatus enum (used for billing trigger), GameState enum, SimType enum, DrivingState enum

### Admin UI
- `kiosk/src/app/staff/page.tsx` — Staff dashboard (billing state display)
- `kiosk/src/components/KioskPodCard.tsx` — Pod card with game state badge

### Research (from game-launcher project)
- `../game-launcher/.planning/research/PITFALLS.md` — Billing during loading screens pitfall, telemetry-as-billing-signal approach
- `../game-launcher/.planning/research/ARCHITECTURE.md` — PlayableSignal as critical missing piece

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `defer_billing_start()`: Already queues pods in `waiting_for_game` — billing only starts when `handle_game_status_update(Live)` fires. This pattern generalizes cleanly
- `DrivingDetector`: Has idle detection with configurable threshold — can be used for 5-minute idle billing pause
- `billing_rates` table + `BillingManager` cache: Already handles tiered rates with in-memory refresh every 60s
- `compute_session_cost()`: Already accepts billing tier — just needs sim_type parameter added

### Established Patterns
- Billing trigger: `AcStatus::Live` in `handle_game_status_update()` — needs generalization from AC-only to per-sim PlayableSignal
- Rate lookup: `BillingManager::get_rates()` returns cached Vec<BillingRate> — needs game-aware lookup
- Admin rates UI: Existing Per-Minute Rates table with inline editing in admin dashboard

### Integration Points
- `event_loop.rs` crash recovery (Phase 81): sends game state updates that feed into billing. PlayableSignal must integrate here
- `ws_handler.rs`: `BillingStarted` message already carries session info to agent — no change needed
- `billing_rates` cloud sync: Column addition propagates automatically via SYNC_TABLES

</code_context>

<specifics>
## Specific Ideas

- "Loading F1 25..." with timer on pod card — staff can see billing hasn't started during shader compilation
- 30s grace period on game exit prevents session fragmentation during crash recovery
- 90s process-based fallback is deliberately conservative — better to miss 90s of billing than to charge during loading screens
- Per-game rates with admin UI — same pattern as existing Per-Minute Rates table, just with a game column

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 82-billing-and-session-lifecycle*
*Context gathered: 2026-03-21*
