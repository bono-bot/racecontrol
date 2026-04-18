# Phase 414: Continuous Billing Session — Context

**Gathered:** 2026-04-18
**Status:** Ready for planning
**Source:** Design contract from `~/.claude/projects/C--Users-bono/memory/decision_billing_continuous_session_design.md` + Uday session 2026-04-18

<domain>
## Phase Boundary

Decouple the billing-session lifetime from individual game lifetimes. Today, ending a game effectively ends the billing session (no UI to "swap game inside session"), so customers playing 15min AC + 15min F1 25 pay ₹375 + ₹375 = ₹750 instead of the ₹700 snap that one continuous 30-min session would have cost. The fix:

1. One billing session can span multiple games/cars/tracks.
2. The meter ticks ONLY while a game is `Running` AND the driver is `Active` (arcade coin-op model).
3. Between games, billing status moves to `WaitingForGame` (existing status, reused) — meter pauses, idle counter starts.
4. After 15 minutes of no game running, the session auto-ends with a 10-minute warning showing the customer's wallet balance.
5. Snap pricing accumulates across game swaps — 25min AC + 5min F1 25 hits the 30-min snap (₹700), not two separate per-minute charges.

This phase changes BACKEND (`billing_fsm.rs`, `billing_timer.rs`, `billing_game_status.rs`, `billing_session_lifecycle.rs`, `rc-common/src/protocol.rs`) AND FRONTEND (`kiosk/src/app/staff/page.tsx` — new "Continue with another game" / "End session" buttons + IdleWarning modal). It does NOT change rc-agent code (game-state events already flow up).

</domain>

<decisions>
## Implementation Decisions

### Status Machine Strategy (LOCKED)
- **Reuse existing `WaitingForGame` status** — do NOT add a new status variant. Avoids `rc-common` protocol churn that would require fleet rc-agent redeploy. `WaitingForGame` semantically already means "no game running, billing waiting." Extending it from "first-time wait" to "between-games wait" is the smallest change.

### FSM Transitions (LOCKED)
Add to `billing_fsm.rs::TRANSITION_TABLE`:
- `Active + GameStopped → WaitingForGame` (NEW event `BillingEvent::GameStopped`)
- `WaitingForGame + End → Completed` (NEW — currently FSM rejects this; see `api/billing_session.rs:259`)
- `WaitingForGame + EndEarly → EndedEarly` (NEW — same rejection issue)
- (Existing) `WaitingForGame + GameLive → Active` reused for resume

### Meter Pause Trigger (LOCKED)
Meter pauses on transition `Active → WaitingForGame`. Trigger: in `billing_game_status.rs`, when game state transitions `Running → Stopped` OR `Running → Crashed` AND billing.status is currently `Active`, fire `BillingEvent::GameStopped`. Crash recovery still uses the existing `PausedCrashRecovery` path (10-min) — only clean game-end goes through the new path.

### Idle Auto-End Timing (LOCKED)
- 600s (10 min): broadcast `DashboardEvent::IdleWarning { pod_id, session_id, balance_paise, seconds_remaining: 300 }` to kiosk
- 900s (15 min): auto-end via `BillingEvent::End` → `Completed` status

Counter resets to 0 on any `WaitingForGame → Active` transition.

### Balance Gate at Warning (LOCKED)
At the 10-min warning, kiosk displays customer's current wallet balance. If balance < `rate_paise_per_minute` (i.e. < ₹25 = can't afford even 1 more minute), modal shows "Insufficient balance — please top up at POS to continue." Auto-end fires regardless at 15 min.

### Snap Pricing Across Swaps (LOCKED)
No code change required for snap-across-swap. `BillingTimer::elapsed_seconds` already accumulates across game-stop/game-start pairs because the BillingTimer outlives individual games. `snap_debit_amount()` (billing.rs:210) reads cumulative `elapsed_seconds` regardless of game identity. **MUST add an integration test** that proves this works: 25min Active → GameStopped → 5min after GameLive → cumulative cost == ₹700.

### Manual End Session (LOCKED, current behaviour preserved)
Staff hitting End Session mid-game still stops the game AND ends billing (current `stop_game` + `end_billing_session_public` chain unchanged). Only the AUTO-END-on-game-stop path changes (it's removed entirely — game stop no longer ends billing).

### Activity Log Granularity (LOCKED)
Game-stop → WaitingForGame and WaitingForGame → Active transitions both log to pod_activity (existing pattern). Use distinct `kind` values to differentiate from initial wait.

### Cloud Sync (LOCKED)
`between_games_idle_seconds` is in-memory only. NOT persisted. Server restart mid-WaitingForGame resets the counter to 0 — customer-favourable.

### Frontend Behaviour (LOCKED)
When kiosk receives a `BillingSessionInfo` with `status == WaitingForGame` AND `elapsed_seconds > 0`:
- Show paused-meter UI with cumulative driving time + cost so far
- Show "Continue with another game" button → opens existing game-select panel
- Show "End session" button → existing end_billing flow (now allowed from WaitingForGame per new FSM transition)

When kiosk receives `DashboardEvent::IdleWarning`:
- Modal overlay with countdown timer + balance display
- "Tap to continue" button → reactivates by opening game-select (any game launch will fire `GameLive` → status returns to Active, idle counter resets)
- Branch: if balance < min, show "Insufficient balance" message + only "End session" CTA

### Claude's Discretion
- Internal naming of new field on BillingTimer (suggest `between_games_idle_seconds` for clarity, but `idle_seconds` or `pause_idle_seconds` also acceptable)
- Specific UI styling (use existing kiosk component library — buttons, modals, typography per `kiosk/src/components/`)
- Whether the IdleWarning event is broadcast to admin dashboard too or only kiosk (suggest both — admin should see active idle countdowns)
- Test file naming (existing convention: `billing_*_tests.rs` modules in racecontrol-crate)
- Whether to also cover "all 3 paused statuses including WaitingForGame should reject `Pause` event" in the new transition tests

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Billing FSM + Timer (existing — must not break)
- `crates/racecontrol/src/billing_fsm.rs` — TRANSITION_TABLE (single source of truth for status changes)
- `crates/racecontrol/src/billing.rs` — BillingTimer struct, snap_debit_amount, current_cost
- `crates/racecontrol/src/billing_timer.rs` — tick() function, per-second loop logic
- `crates/racecontrol/src/billing_session_lifecycle.rs` — extend_billing_session, status mutations
- `crates/racecontrol/src/billing_session_end.rs` — end_billing_session (status → Completed/EndedEarly)
- `crates/racecontrol/src/billing_session_start.rs` — initial WaitingForGame creation, max_session_seconds=86400 (24hr cap from `e3d05cea`)
- `crates/rc-common/src/types.rs` — BillingSessionStatus enum (DO NOT add variant), BillingSessionInfo (may add optional field)
- `crates/rc-common/src/protocol.rs` — DashboardEvent enum (add IdleWarning variant — server→dashboard only, safe protocol change)
- `crates/racecontrol/src/billing_game_status.rs` — game-state→billing-status coupling

### Game-stop trigger source
- `crates/racecontrol/src/game_launcher_ops_stop.rs` — stop_game function (already does NOT call end_billing — confirmed in pre-planning grep)
- `crates/racecontrol/src/api/game_ac.rs` + `crates/racecontrol/src/api/game_launch.rs` — game-state update handlers

### Kiosk frontend
- `kiosk/src/app/staff/page.tsx` — staff session UI (line 252-255 has current End Session handler)
- `kiosk/src/hooks/useKioskSocket.ts` — WS event reception (where IdleWarning handler will land)
- `kiosk/src/components/` — button/modal components for the new UI

### Standing rules + protocols
- `CLAUDE.md` — project-global standing rules. Specifically:
  - "Financial flow E2E: trace actual currency values through complete flows before shipping billing/wallet changes" — mandatory before deploy
  - "Never hold a lock across .await" — billing module already complies but new code must too
  - "DEPLOY PARITY (UNIVERSAL — NO EXCEPTIONS)" — server .23 + Bono VPS racecontrol both need rebuild after this lands
  - "MMA audit MANDATORY for cross-system bridges" — debatable here (no rc-agent change) but billing FSM changes warrant a verification audit
- `~/.claude/projects/C--Users-bono/memory/decision_billing_continuous_session_design.md` — full design contract

### Existing tests to keep green
- `crates/racecontrol/src/billing_tests.rs` — 102 billing tests must continue to pass
- `crates/racecontrol/src/billing_fsm.rs::tests` — FSM transition tests (existing transitions unchanged, new ones added)

### Recently-merged related work (2026-04-17/18)
- `e3d05cea` — End Session error broadcast + 24hr cap (already in deployed binary `45d03bd5`)
- `45d03bd5` — Trial auto-end fallback to package mode (current server build)
- `319d8fab` — Pause/Resume FSM error broadcast

</canonical_refs>

<specifics>
## Specific Ideas

### Cumulative snap example (must be in tests)
- Customer plays 25 min in AC (cost so far: 25 × ₹25 = ₹625)
- Stops AC cleanly → status=WaitingForGame, between_games_idle_seconds=0
- Waits 7 min between games (no charge — meter paused)
- Starts F1 25 → status=Active, idle counter cleared
- Plays 5 more min → cumulative elapsed_seconds=30 min → snap fires → cost=₹700 (NOT ₹625 + 5×₹25=₹750)
- Verifies: total wallet debit = ₹700

### IdleWarning event payload (locked)
```rust
DashboardEvent::IdleWarning {
    pod_id: String,
    session_id: String,
    balance_paise: u64,           // current wallet balance for customer
    seconds_remaining: u32,       // 300 at 10-min mark, lower if event re-fires
    can_continue: bool,           // false if balance < rate_paise_per_minute
}
```

### Kiosk paused-meter UI requirement
The existing meter UI (cost ticking up per second) must visually freeze when status=WaitingForGame, but display "Paused — between games" subtle indicator below the cost. Cumulative cost stays visible.

### Edge case: customer stops game then immediately ends session
Sequence: game stops → status=WaitingForGame (idle=0) → staff hits End Session within 5 sec → FSM transition WaitingForGame+End → Completed. Final bill = whatever cost was at game-stop (snap pricing applies normally to final elapsed_seconds).

### Edge case: WaitingForGame but customer NEVER drove
Sequence: book session → status=WaitingForGame → 15 min pass with no game ever launched → existing "no playable game" behaviour should still apply (CancelledNoPlayable). Need to differentiate: if `elapsed_seconds == 0`, treat as initial wait (existing CancelledNoPlayable path); if `elapsed_seconds > 0`, treat as between-games (new auto-end path → Completed). Idle threshold should be 15 min for both? Or keep existing CancelledNoPlayable threshold? **DECISION DEFERRED to planner with research.**

### Edge case: server restarts mid-WaitingForGame
between_games_idle_seconds resets to 0 on restart. Customer-favourable. Document this in code comment.

### Edge case: pod offline during WaitingForGame
PausedDisconnect path takes priority via existing transition `Active + Disconnect → PausedDisconnect`. From WaitingForGame, what does Disconnect do? Currently no transition exists. Decision: add `WaitingForGame + Disconnect → PausedDisconnect` to keep behaviour consistent (or just leave WaitingForGame state through disconnect since meter is already paused).

</specifics>

<deferred>
## Deferred Ideas

- **Per-game/per-car/per-track snap incentives** — beyond per-minute snap. Not in scope. v2.
- **Customer-facing UI** — the staff kiosk gets the new buttons; customer-facing kiosk overlay (if any) is separate work.
- **Notifications to staff WhatsApp on auto-end** — could be useful but adds notification system coupling. v2.
- **Configurable idle threshold** — hardcoded to 15 min for now. v2 could expose in `pricing_rules` table.
- **Multi-driver swap inside one billing session** — if pod has split sessions, swap rules get more complex. NOT in scope.
- **Mid-session balance top-up while in WaitingForGame** — would require POS↔kiosk live sync. The "Insufficient balance" branch tells customer to top up; doesn't auto-detect topup. Could be v2.

</deferred>

---

*Phase: 414-continuous-billing-session*
*Context gathered: 2026-04-18 from design memory + Uday session decisions*
