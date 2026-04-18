# Phase 414: Continuous Billing Session — Research

**Researched:** 2026-04-18
**Domain:** Rust backend FSM extension + tokio per-second tick loop + Next.js kiosk UI extension. Single-binary monorepo (`racecontrol` server crate + `rc-common` shared protocol crate + `kiosk` Next.js app).
**Confidence:** HIGH — every claim below was verified by direct file read or grep against the local checkout (commit `67e580bb` per memory).

## Summary

Phase 414 decouples a billing session's lifetime from any individual game's lifetime. The user's contract (in CONTEXT.md) is fully locked, including the central design choice: **reuse the existing `BillingSessionStatus::WaitingForGame` variant for the between-games idle state instead of adding a new variant.** The chief existential risk for that choice is whether existing consumers of `WaitingForGame` implicitly assume `elapsed_seconds == 0` (i.e., that `WaitingForGame` only ever means "never-driven yet"). I performed a complete consumer audit (Step 2.5–style) and the verdict is: **the reuse decision is safe with three known surface fixes** — see "WaitingForGame Consumer Audit" below.

The remaining work is a pure FSM extension (3 new transitions, 1 new event, no new variants), a tick-loop branch on `WaitingForGame` to advance an in-memory idle counter, a single new `DashboardEvent::IdleWarning` variant (server→dashboard only — backwards-compatible protocol change), and frontend code in `kiosk/src/app/staff/page.tsx` plus a status pill rewording in `KioskPodCard.tsx`/`StatusBadge.tsx` so "Game Loading" no longer shows when the session is mid-stream between games.

**Primary recommendation:** Plan as 6 narrow waves: (1) FSM table extension + unit tests, (2) BillingTimer field + tick branch + unit tests, (3) `BillingSessionInfo.between_games_idle_seconds` optional field + protocol round-trip test, (4) `billing_game_status::handle_game_off` rewrite to fire `GameStopped` instead of `end_billing_session` for single-player + 3 surface-fix grep audit (StatusBadge label, billing.rs/page.tsx label, stop_billing branch), (5) kiosk staff page UI (Continue / End buttons + IdleWarning modal + paused-meter visual), (6) integration test (25min Active → GameStopped → 7min wait → GameLive → 5min Active → assert ₹700 cumulative). Wave 0 = test scaffolding (already in place — `billing_fsm.rs::tests` has 33 transition tests as template; `billing_tests.rs` has the snap-pricing pattern).

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Status Machine Strategy** — Reuse existing `WaitingForGame` status. Do NOT add a new status variant. Avoids `rc-common` protocol churn that would require fleet rc-agent redeploy.

**FSM Transitions** — Add to `billing_fsm.rs::TRANSITION_TABLE`:
- `Active + GameStopped → WaitingForGame` (NEW event `BillingEvent::GameStopped`)
- `WaitingForGame + End → Completed` (NEW)
- `WaitingForGame + EndEarly → EndedEarly` (NEW)
- (Existing) `WaitingForGame + GameLive → Active` reused for resume

**Meter Pause Trigger** — Meter pauses on `Active → WaitingForGame`. Trigger: in `billing_game_status.rs`, when game state goes `Running → Stopped` OR `Running → Crashed` AND billing.status is `Active`, fire `BillingEvent::GameStopped`. Crash recovery still uses existing `PausedCrashRecovery` (10-min); only clean game-end goes through the new path.

**Idle Auto-End Timing**
- 600s (10 min): broadcast `DashboardEvent::IdleWarning { pod_id, session_id, balance_paise, seconds_remaining: 300, can_continue }` to kiosk
- 900s (15 min): auto-end via `BillingEvent::End` → `Completed` status
- Counter resets to 0 on any `WaitingForGame → Active` transition.

**Balance Gate at Warning** — At the 10-min warning, kiosk displays customer's current wallet balance. If `balance < rate_paise_per_minute` (i.e. < ₹25), modal shows "Insufficient balance — please top up at POS to continue." Auto-end fires regardless at 15 min.

**Snap Pricing Across Swaps** — No code change required for snap-across-swap. `BillingTimer::elapsed_seconds` already accumulates because the BillingTimer outlives individual games. `snap_debit_amount()` reads cumulative `elapsed_seconds` regardless of game identity. **MUST add an integration test** proving 25min AC + 5min F1 25 → cumulative cost == ₹700, not ₹750.

**Manual End Session** — Staff hitting End Session mid-game still stops the game AND ends billing (current `stop_game` + `end_billing_session_public` chain unchanged). Only the AUTO-END-on-game-stop path is removed (game stop no longer ends billing).

**Activity Log Granularity** — Game-stop → WaitingForGame and WaitingForGame → Active transitions both log to `pod_activity` (existing pattern). Use distinct `kind` values to differentiate from initial wait.

**Cloud Sync** — `between_games_idle_seconds` is in-memory only. NOT persisted. Server restart mid-WaitingForGame resets the counter to 0 — customer-favourable.

**Frontend Behaviour** — When kiosk receives `BillingSessionInfo` with `status == WaitingForGame` AND `elapsed_seconds > 0`:
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

### Deferred Ideas (OUT OF SCOPE)
- **Per-game/per-car/per-track snap incentives** — beyond per-minute snap. Not in scope. v2.
- **Customer-facing UI** — the staff kiosk gets the new buttons; customer-facing kiosk overlay (if any) is separate work.
- **Notifications to staff WhatsApp on auto-end** — could be useful but adds notification system coupling. v2.
- **Configurable idle threshold** — hardcoded to 15 min for now. v2 could expose in `pricing_rules` table.
- **Multi-driver swap inside one billing session** — if pod has split sessions, swap rules get more complex. NOT in scope.
- **Mid-session balance top-up while in WaitingForGame** — would require POS↔kiosk live sync. The "Insufficient balance" branch tells customer to top up; doesn't auto-detect topup. Could be v2.
</user_constraints>

## Project Constraints (from CLAUDE.md)

These are non-negotiable and must be honored by the plan. Each is verbatim or near-verbatim from the standing rules at the top of `CLAUDE.md`:

| Rule | Source | Implication for Phase 414 |
|------|--------|---------------------------|
| **Financial flow E2E: trace actual currency values through complete flows** before shipping billing/wallet changes. | CLAUDE.md > Code Quality | Mandatory. Plan must include an end-to-end trace: customer with X balance → continuous session with game-stop → wait → game-start → end → assert wallet delta == cumulative snap cost. Not optional. |
| **Never hold a lock across `.await`** | CLAUDE.md > Code Quality | Existing `tick_all_timers` already uses `try_write` + tight `{}` blocks. New idle-counter increment lives inside the existing per-tick write lock — no new `.await` inside the lock. |
| **DEPLOY PARITY (UNIVERSAL — NO EXCEPTIONS)** | CLAUDE.md > Deploy | Server `.23` AND Bono VPS racecontrol both need rebuild + restart after this lands. Kiosk frontend build must be deployed to BOTH locations (currently the cloud kiosk and venue kiosk both serve the staff page). |
| **No `.unwrap()` in production Rust** | CLAUDE.md > Code Quality | All new code paths use `?`, `.ok()`, or `match`. |
| **No `any` in TypeScript** | CLAUDE.md > Code Quality | Kiosk modal + button code must be fully typed against the existing `BillingSessionInfo` shape from `packages/shared-types/src/billing.ts`. |
| **MMA audit MANDATORY for cross-system bridges** | CLAUDE.md > Standing Rules | Debatable. No rc-agent change in this phase, but billing FSM + a new cross-boundary protocol field warrant an MMA audit per the "any new feature that creates a data flow across 2+ system boundaries" criterion. Plan should include a Wave that runs `node scripts/multi-model-audit.js` before deploy. |
| **Pre-commit hooks block** `cargo test` failures on `rc-common` + `racecontrol-crate --lib` | spawn prompt | All new tests must compile and pass before commit. Plan tasks must finish each wave with a green local `cargo test -p rc-common && cargo test -p racecontrol-crate --lib` run. |
| **Cascade updates (RECURSIVE)** | CLAUDE.md > Code Quality | Adding the optional `between_games_idle_seconds` field on `BillingSessionInfo` is a cascade trigger — `packages/shared-types/src/billing.ts`, `web/src/lib/api.ts`, and `packages/contract-tests/src/billing.contract.test.ts` MUST also be updated in the same commit set. |
| **First-run verification after enabling any guard/filter** | CLAUDE.md > Process | After deploy, verify the FIRST WaitingForGame mid-stream session shows the new UI on kiosk. The 10-min warning + 15-min auto-end need a real-clock observation, not just unit tests. |

## Phase Requirements

(No formal REQ-IDs — this phase was added 2026-04-18 outside the v49.0 requirements doc; CONTEXT.md decisions are the contract.)

| Pseudo-ID | Description | Research Support |
|----|-------------|------------------|
| 414-FSM-01 | `BillingEvent::GameStopped` added to enum | `billing_fsm.rs:41-64` enum location verified |
| 414-FSM-02 | `Active + GameStopped → WaitingForGame` transition | `TRANSITION_TABLE` at `billing_fsm.rs:68-101` |
| 414-FSM-03 | `WaitingForGame + End → Completed` transition | Currently rejected — see `api/billing_session.rs:258-259` workaround proves this |
| 414-FSM-04 | `WaitingForGame + EndEarly → EndedEarly` transition | Same — workaround comment confirms |
| 414-TIMER-01 | `BillingTimer.between_games_idle_seconds` field added | `billing.rs:27-104` — struct definition with all current fields |
| 414-TIMER-02 | `tick()` increments idle counter when status == WaitingForGame | `billing.rs:241-265` — current `match self.status` block has the slot at line 262 (currently `WaitingForGame => false` no-op) |
| 414-TIMER-03 | At 600s, broadcast `IdleWarning`. At 900s, auto-end via `BillingEvent::End` | `billing_timer.rs:241-271` (expired branch) is the template — same pattern as `expired` in tick |
| 414-PROTOCOL-01 | `DashboardEvent::IdleWarning { pod_id, session_id, balance_paise, seconds_remaining, can_continue }` added to enum | `protocol.rs:1175` location verified — additive variant safe per existing serde tagged union |
| 414-PROTOCOL-02 | `BillingSessionInfo.between_games_idle_seconds: Option<u32>` added | `types.rs:404` definition verified; existing pattern (e.g. `recovery_pause_seconds: Option<u32>`) shows additive optional field is backwards-compat |
| 414-GAME-01 | `billing_game_status::handle_game_off` no longer ends billing in single-player path; instead fires `BillingEvent::GameStopped` | `billing_game_status.rs:296-358` — `handle_game_off` currently calls `end_billing_session(... EndedEarly)` at line 329 — this is the EXACT line to change |
| 414-FRONTEND-01 | Kiosk staff page shows Continue/End buttons when WaitingForGame + elapsed_seconds > 0 | `kiosk/src/components/PodKioskView.tsx:402-403` and `LiveSessionPanel.tsx:115-118` are the existing handlers |
| 414-FRONTEND-02 | IdleWarning modal with countdown + balance + Continue / End CTAs | New component required — no existing modal handles a server-pushed countdown |
| 414-FRONTEND-03 | StatusBadge label for between-games WaitingForGame differs from first-time WaitingForGame | `web/src/components/StatusBadge.tsx:5` currently labels `waiting_for_game: "Loading..."` — needs conditional label based on elapsed_seconds |
| 414-TEST-01 | Integration test: 25min Active → GameStopped → 7min wait → GameLive → 5min Active → cumulative cost == ₹700 | `billing_tests.rs:373-399` is the snap-pricing test pattern; the integration test needs the same `BillingTimer` construction style |

## Standard Stack

### Core (already in workspace, no new dependencies)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `tokio` | workspace pin | Async runtime, RwLock, mpsc channels | Already powers entire racecontrol; standard rule "never hold lock across .await" applies |
| `sqlx` | workspace pin | SQLite query interface | All billing DB writes go through sqlx; no migration required for this phase (in-memory field only) |
| `serde` + `serde_json` | workspace pin | Protocol enum (de)serialization | `DashboardEvent` is already `#[serde(tag = "event", content = "data")]` — adding a variant is a one-line additive change |
| `chrono` | workspace pin | UTC timestamps for `started_at`, `lap_reject_grace_until` | Existing pattern; nothing new |
| `tracing` | workspace pin | All logs | All warn/info/error already emitted via `tracing::*!` macros |
| `uuid` | workspace pin | New event UUIDs (if any are persisted) | Existing pattern in `billing_session_lifecycle.rs` |

### Frontend (already in kiosk app, no new dependencies)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| Next.js | 15.x (basePath `/kiosk`) | Staff page hosting | Existing — `kiosk/src/app/staff/page.tsx` is the file to edit |
| React 19 | workspace | Component model | Existing |
| `useKioskSocket` hook | local | WS event reception | Existing — `kiosk/src/hooks/useKioskSocket.ts` is where `IdleWarning` handler lands |
| Tailwind | local | Styling | Existing — use `bg-rp-surface`, `border-rp-border`, `text-rp-grey` per kiosk components |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Reuse `WaitingForGame` | Add new `BillingSessionStatus::BetweenGames` | LOCKED rejected — would force `rc-common` minor bump → fleet rc-agent redeploy. Reuse is the smaller change. Confirmed safe by consumer audit. |
| Server-side idle counter (chosen) | Client-side idle counter that polls server when expired | Server-side is the only correct option — multiple kiosks may share state, server restart shouldn't lose customer-favourable state, and the 15-min timer must run regardless of kiosk activity. |
| New `BillingEvent::GameStopped` (chosen) | Reuse `BillingEvent::EndEarly` from `Active` and special-case it | Reuse is wrong — `EndEarly` from Active goes to `EndedEarly` (terminal). We need a non-terminal pause-equivalent. New event is the right choice and matches the FSM table style. |
| `between_games_idle_seconds` as field on `BillingTimer` (chosen) | Hold idle timer in a separate `HashMap<String, IdleState>` on `BillingManager` | Field on `BillingTimer` is colocated with all other per-pod billing state, so the existing tick lock covers it for free. Separate HashMap would require a second lock and more careful ordering. |

**Installation:** No new dependencies needed. All work uses existing crates.

**Version verification:** Skipped — no new dependencies added in this phase. Existing workspace pins remain authoritative.

## Architecture Patterns

### Recommended Code Layout

```
crates/racecontrol/src/
├── billing_fsm.rs               # Add BillingEvent::GameStopped + 3 transitions to TRANSITION_TABLE
├── billing.rs                   # Add between_games_idle_seconds: u32 field to BillingTimer
├── billing_timer.rs             # Add WaitingForGame branch in tick() that increments idle counter, fires IdleWarning at 600s, auto-ends at 900s
├── billing_game_status.rs       # In handle_game_off (single-player path, line 329), replace end_billing_session(...EndedEarly) with FSM transition Active→WaitingForGame via GameStopped event
├── billing_session_lifecycle.rs # No code change — already routes End/Cancel through end_billing_session which now (per FSM-03/04) accepts WaitingForGame as source
├── api/billing_session.rs       # Remove the workaround at line 258-342 that detects waiting_for_game and forces CancelledNoPlayable — keep logic ONLY for elapsed_seconds == 0 case (initial wait)
└── billing_tests.rs             # Add 4 new tests (FSM transitions × 3 + cumulative snap integration)

crates/rc-common/src/
├── types.rs                     # Add `pub between_games_idle_seconds: Option<u32>` to BillingSessionInfo struct (line 404)
└── protocol.rs                  # Add DashboardEvent::IdleWarning { pod_id, session_id, balance_paise, seconds_remaining, can_continue } variant (after line 1175)

packages/shared-types/src/
└── billing.ts                   # Mirror the `between_games_idle_seconds?: number` field on BillingSession TS type (cascade rule)

packages/contract-tests/src/
└── billing.contract.test.ts     # Add round-trip test for the new field
└── ws-dashboard.contract.test.ts# Add IdleWarning fixture + parsing test

kiosk/src/
├── app/staff/page.tsx                       # Show Continue/End buttons + paused-meter when status=WaitingForGame AND elapsed_seconds>0
├── components/IdleWarningModal.tsx          # NEW — countdown timer + balance display + Continue/End CTAs
├── components/PodKioskView.tsx              # Update isWaitingForGame branch to differentiate first-wait vs between-games (line 402-403)
├── components/KioskPodCard.tsx              # Update statusLabel() to differentiate (line 118)
└── hooks/useKioskSocket.ts                  # Add IdleWarning event handler that opens IdleWarningModal

web/src/
├── lib/api.ts                               # Update BillingSessionInfo type (line 957-980)
└── components/StatusBadge.tsx               # Conditional label override based on elapsed_seconds (line 5)
```

### Pattern 1: Adding an FSM Transition (HIGH confidence — 33 existing tests prove the pattern)

**What:** New transitions are a one-line append to `TRANSITION_TABLE` plus a unit test that calls `validate_transition`.

**When to use:** Every new billing state change.

**Example (verified pattern from `billing_fsm.rs:228-232`):**
```rust
// Source: crates/racecontrol/src/billing_fsm.rs (existing tests, verified)
#[test]
fn test_active_game_stopped_to_waiting() {
    let result = validate_transition(BillingSessionStatus::Active, BillingEvent::GameStopped);
    assert_eq!(result, Ok(BillingSessionStatus::WaitingForGame));
}

#[test]
fn test_waiting_for_game_end_to_completed() {
    // NEW transition: was rejected before this phase
    let result = validate_transition(BillingSessionStatus::WaitingForGame, BillingEvent::End);
    assert_eq!(result, Ok(BillingSessionStatus::Completed));
}
```

### Pattern 2: Tick-Loop Branch Extension (HIGH confidence)

**What:** The tick loop (`billing_timer.rs::tick_all_timers`) takes a single `try_write` on `active_timers`, iterates all pods, mutates `BillingTimer` fields directly, collects events into `Vec`s, then drops the lock and emits the events. New status handling is added inside the existing `for (pod_id, timer) in timers.iter_mut()` block.

**When to use:** All per-second time-based logic.

**Example (mirroring existing PausedDisconnect handler at `billing_timer.rs:82-108`):**
```rust
// Source: crates/racecontrol/src/billing_timer.rs (existing PausedDisconnect handler, verified)
// Phase 414: WaitingForGame mid-stream handler (new)
if timer.status == BillingSessionStatus::WaitingForGame && timer.elapsed_seconds > 0 {
    timer.between_games_idle_seconds += 1;

    // 10-min warning (one-shot)
    if timer.between_games_idle_seconds == 600 {
        // Look up wallet balance separately AFTER lock drop — collect for post-lock work
        idle_warnings_to_emit.push((
            pod_id.clone(),
            timer.session_id.clone(),
            timer.wallet_owner_id.clone(),
            timer.rate_paise_per_minute,
        ));
    }

    // 15-min auto-end
    if timer.between_games_idle_seconds >= 900 {
        sessions_to_auto_end.push((
            pod_id.clone(),
            timer.session_id.clone(),
            "Idle 15 minutes between games".to_string(),
        ));
    }

    // Broadcast paused-meter tick so kiosk shows Cumulative Time + Cost so far
    events_to_broadcast.push(DashboardEvent::BillingTick(timer.to_info(&rate_tiers)));
    continue;
}
```

### Pattern 3: Additive `DashboardEvent` Variant (HIGH confidence)

**What:** `DashboardEvent` is `#[serde(tag = "event", content = "data")]` (verified at `protocol.rs:1173-1175`). Adding a new variant is forward-compatible — older clients silently ignore unknown event tags. No protocol bump needed.

**When to use:** Any new server→client push event.

**Example (mirroring existing `DashboardEvent::SessionPaused` test at `protocol.rs:1993-2010`):**
```rust
// Source: crates/rc-common/src/protocol.rs (existing variants, verified)
DashboardEvent::IdleWarning {
    pod_id: String,
    session_id: String,
    balance_paise: u64,
    seconds_remaining: u32,
    can_continue: bool,
}
```

### Pattern 4: Frontend Event Reception (HIGH confidence — existing handler hook)

**What:** `kiosk/src/hooks/useKioskSocket.ts` (per CONTEXT.md `canonical_refs`) is the established WS event landing point. New event handlers are registered alongside existing ones; UI side effects (e.g., opening a modal) propagate via React state updates and existing component tree.

### Anti-Patterns to Avoid

- **Don't add a new `BillingSessionStatus` variant** — explicitly rejected per CONTEXT.md. Forces fleet rc-agent redeploy.
- **Don't try to persist `between_games_idle_seconds` to DB** — explicitly rejected per CONTEXT.md ("in-memory only, customer-favourable on restart"). Stay disciplined: do NOT add a column or sync field.
- **Don't reuse `BillingEvent::Pause` for game-stop** — semantically wrong. `Pause` is for AC `STATUS=Pause` (driver hit ESC). Game-stop is a different signal and goes through `AcStatus::Off → handle_game_off`.
- **Don't fire `IdleWarning` more than once per session** — use a `idle_warning_sent: bool` flag on the timer, mirroring the existing `warning_5min_sent: bool` pattern at `billing.rs:33-34`.
- **Don't broadcast every `BillingTick` while idle if the kiosk would interpret it as "active driving"** — verify the kiosk LiveSessionPanel.tsx existing branch at line 116 (`status === "waiting_for_game"`) gates the meter display correctly.
- **Don't read `wallet.balance` inside the tick lock** — DB query. Collect `(pod_id, session_id, wallet_owner_id, rate)` tuples inside the lock, drop the lock, then `SELECT balance` and emit `IdleWarning`. Same pattern as the existing per-minute debit at `billing_timer.rs:323-396`.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| FSM transition validation | Manual `if status == X && event == Y` chains | `validate_transition()` at `billing_fsm.rs:109` | Single source of truth, 33 tests cover correctness. Bypass = silent inconsistency. |
| Per-second iteration over pods | New tokio interval loop | Add a branch to existing `tick_all_timers` | One global tick already runs every 1s. Adding another would double the lock contention and risk races. |
| Idle counter reset on resume | Hand-coded reset on every WaitingForGame→Active path | Reset inside `handle_live_resume` at `billing_game_status.rs:240-263` (existing function) | Already the central handler for the resume FSM transition. Adding `timer.between_games_idle_seconds = 0; timer.idle_warning_sent = false;` there covers all callers. |
| Wallet balance lookup | New DB query helper | `sqlx::query_as::<_, (i64,)>("SELECT balance_paise FROM wallets WHERE driver_id = ?")` per existing pattern at `billing_timer.rs:377-380` | Same query, same column. Don't introduce a 2nd helper. |
| Snap pricing across game swaps | New aggregator | Already works — `BillingTimer::snap_debit_amount()` at `billing.rs:210` reads cumulative `elapsed_seconds` regardless of game identity | The whole point of CONTEXT.md's "Snap Pricing Across Swaps" being LOCKED with "no code change required." Verified by reading `snap_debit_amount` body. |
| Frontend countdown display | New timer component | Mirror `LaunchTimerBanner` at `kiosk/src/components/LiveSessionPanel.tsx:117` (existing component for the `waiting_for_game` first-wait countdown) | Same shape: server pushes `seconds_remaining`, client interpolates 1s ticks locally between WS messages. |

**Key insight:** The largest existential risk for this phase (per the spawn prompt's open risk #1) was the WaitingForGame consumer audit. The audit (next section) shows that 95% of consumers are SAFE under the reuse decision because they treat `waiting_for_game` as a UI display state not as a temporal "is this the first wait" state. The 3 unsafe consumers are surface fixes that touch existing label and gate logic, not deep model changes. **There is no need to invent a new abstraction here** — the reuse decision was correct.

## WaitingForGame Consumer Audit (Open Risk #1 — DEFINITIVE)

This is the highest-priority research finding. Every consumer of `BillingSessionStatus::WaitingForGame` was located via `grep -r WaitingForGame|waiting_for_game crates/ kiosk/ web/ packages/`. For each, I report whether it implicitly assumes `elapsed_seconds == 0`.

**Verdict:** **Safe to reuse, with 3 surface fixes.** No consumer requires architectural redesign.

### Backend (Rust)

| File:Line | Consumer | Assumes `elapsed_seconds == 0`? | Action Required |
|-----------|----------|-------------------------------|-----------------|
| `crates/racecontrol/src/billing_fsm.rs:71-73` | `TRANSITION_TABLE` rows for `WaitingForGame + GameLive → Active`, `+ Cancel → CancelledNoPlayable`, `+ CancelNoPlayable → CancelledNoPlayable` | NO assumption. Pure state-machine table. | **No change to existing rows.** ADD: `+ End → Completed`, `+ EndEarly → EndedEarly`, `Active + GameStopped → WaitingForGame`. |
| `crates/racecontrol/src/billing_fsm.rs:171` | `authoritative_end_session` CAS list of pre-terminal states | NO assumption. Just lists which DB statuses can be terminated. | **No change.** `'waiting_for_game'` is already in the CAS list. End from WaitingForGame will work as soon as FSM transition allows it. |
| `crates/racecontrol/src/billing.rs:262` | `BillingTimer::tick()` — `BillingSessionStatus::WaitingForGame => false` (no-op) | NO assumption. Currently a no-op because pre-414 there is no `Active → WaitingForGame` transition that creates a timer in this state. | **REPLACE the no-op** with the idle-counter increment + warning + auto-end logic per Pattern 2. |
| `crates/racecontrol/src/billing_timer.rs:412-437` | `tick_all_timers` "BILL-05 broadcast" — iterates `state.billing.waiting_for_game` map (NOT active_timers) and emits `BillingTick` with `elapsed_seconds: Some(entry.waiting_since.elapsed())` | YES — this is the FIRST-WAIT broadcast path. Reads from a *different* HashMap (`waiting_for_game: HashMap<String, WaitingForGameEntry>`) than `active_timers`. | **No change.** The first-wait path stays as-is. The new mid-stream WaitingForGame lives in `active_timers` (because the timer was already there from the original Active session) — completely separate code path. NO interference. |
| `crates/racecontrol/src/billing_session_end.rs:131` | CAS UPDATE WHERE status IN (...incl. `'waiting_for_game'`) | NO assumption. CAS list. | **No change.** End now legal from this state. |
| `crates/racecontrol/src/billing_session_end.rs:357` | Orphan recovery query — same status list | NO assumption. | **No change.** |
| `crates/racecontrol/src/api/billing_session.rs:258-342` | `stop_billing` HTTP handler — special-case: if DB status is `waiting_for_game`, route to `CancelledNoPlayable` + full refund | YES — assumes `waiting_for_game` means "first wait, never drove, full refund." | **SURFACE FIX 1 (CRITICAL).** Branch on `elapsed_seconds`: if `0` → keep existing CancelledNoPlayable + refund path. If `> 0` → route to `EndedEarly` (now allowed by new FSM transition `WaitingForGame + EndEarly → EndedEarly`) and bill cumulative cost. The "Refunded {}p for staff-cancelled waiting_for_game session" log line currently always fires — that becomes wrong for between-games sessions where customer DID drive. Plan must update this handler. |
| `crates/racecontrol/src/api/billing_start.rs:48-54, 273-278` | Pod conflict check + INSERT with status='waiting_for_game' | NO assumption (creates the FIRST-WAIT entry; no overlap with mid-stream). | **No change.** |
| `crates/racecontrol/src/billing_timer_stale.rs:21-22, 69-83` | LBILL stale-session cleanup — auto-cancels DB sessions in `'pending'` or `'waiting_for_game'` for >5 min | YES — assumes `waiting_for_game` is the first-wait state and that "5 min stale" means the customer never connected. The stale check uses `created_at < datetime('now', '-5 minutes')` which is the *session creation* time, not the waiting-since time. | **NO immediate fix required, but plan must verify carefully.** Mid-stream WaitingForGame sessions have `created_at` from the original session start, which could be hours old by the time they enter the between-games state. The query would catch them. **Action**: add an `AND elapsed_seconds = 0` (or equivalent) filter to the stale query, OR change the time anchor to the new field. Recommend the simpler fix: `WHERE driving_seconds = 0` added to the stale query. The plan needs an explicit task for this. |
| `crates/racecontrol/src/billing_timer_stale.rs:142` | Removes from `state.billing.waiting_for_game` (the in-memory map for first-wait) | NO assumption (separate map; mid-stream WaitingForGame lives in `active_timers`). | **No change.** |
| `crates/racecontrol/src/billing_timer_expiry_timeout.rs:247, 288, 327` | Launch timeout handler — operates on the `waiting_for_game` HashMap (not active_timers) | NO assumption (separate map). | **No change.** Launch timeouts are first-wait only, by construction. |
| `crates/racecontrol/src/billing_recovery.rs:33, 90` | `venue_shutdown` recovery query — lists `'waiting_for_game'` in pre-terminal status set | NO assumption. | **No change.** Mid-stream sessions correctly recover. |
| `crates/racecontrol/src/billing_jobs.rs:69` | Coupon reservation cleanup query | NO assumption (pre-terminal status set). | **No change.** |
| `crates/racecontrol/src/visits.rs:72, 149-150` | "Active session count" query (pre-terminal status set) | NO assumption. | **No change.** Both first-wait and mid-stream WaitingForGame correctly count as active. |
| `crates/racecontrol/src/auth/token_consume.rs:176, 311, 431` | "Update pod state to WaitingForGame" — 3 sites that update the Pod state, not the billing session | NO assumption (Pod state, not billing). | **No change.** |
| `crates/racecontrol/src/auth/token_manage.rs` | One match found | NO assumption (similar Pod state update). | **No change.** Verified by grep — only mentions the term, doesn't gate on it. |
| `crates/racecontrol/src/billing_game_status.rs:296-358` | `handle_game_off` — currently calls `end_billing_session(... EndedEarly)` for single-player path | THIS IS THE CHANGE TARGET. | **CORE CHANGE.** Replace `end_billing_session` call at line 329 with `validate_transition(timer.status, BillingEvent::GameStopped)` mutation + reset `between_games_idle_seconds = 0` + `idle_warning_sent = false`. Multiplayer path (line 314-320) keeps existing pause_multiplayer_group behavior — multi-player crash recovery is out-of-scope per CONTEXT.md "Multi-driver swap inside one billing session — NOT in scope." |
| `crates/racecontrol/src/billing_game_status.rs:240-263` | `handle_live_resume` — fires `BillingEvent::Resume` to come back to Active | NO assumption per se, BUT this is also the natural reset point for the idle counter. | **CHANGE: Add `timer.between_games_idle_seconds = 0; timer.idle_warning_sent = false;` inside the success branch** (after `Ok(new_status)` at line 246). This is the single-source-of-truth reset. |
| `crates/rc-agent/src/billing_guard.rs:120, 343-346` | `PodFailureReason::SessionStuckWaitingForGame` — agent-side detection of "billing active but no game_pid" | NO assumption — uses a different mechanism (agent-side game_pid presence). Won't fire during between-games WaitingForGame because the billing status will be `WaitingForGame`, not `Active`, on the kiosk side. | **VERIFY but no change expected.** Plan should add an explicit unit test that confirms the agent's billing_guard does NOT fire `SessionStuckWaitingForGame` while the server-side session is in mid-stream WaitingForGame. The test would verify the agent receives status updates and skips the "active but no game" check. |
| `crates/racecontrol/src/billing_tests.rs:643-859` | Existing tests for `WaitingForGameEntry` | NO assumption — tests the `WaitingForGameEntry` struct (first-wait HashMap entry). | **No change.** Add 3-5 NEW tests for mid-stream behavior. |
| `crates/racecontrol/src/db/migrate_billing.rs` | Status string list in DB migration | NO assumption (just CHECK constraint). | **VERIFY no change needed** — `'waiting_for_game'` already in the CHECK constraint. Mid-stream uses the same string. |
| `crates/racecontrol/src/billing_game_status_defer.rs`, `billing_game_status_mp.rs` | Deferred and multiplayer billing helpers — both create `WaitingForGameEntry` (first-wait HashMap) | NO assumption. | **No change.** Multiplayer between-games is out-of-scope. |
| `crates/racecontrol/src/game_launcher_ops.rs:99, 251` | Billing gate at LaunchGame — accepts pods in active_timers OR waiting_for_game map | NO assumption (gate is permissive). | **No change.** Mid-stream WaitingForGame is in active_timers, so a re-launch from the kiosk Continue button correctly passes this gate. |
| `crates/racecontrol/src/api/billing_start_validate.rs`, `billing_start_postcommit.rs` | Helpers for the FIRST-WAIT path | NO assumption (creates first-wait state). | **No change.** |
| `crates/racecontrol/src/game_launcher_tests.rs`, `crates/racecontrol/src/billing_tests.rs` | Existing tests | NO assumption. | **Add new tests, don't change existing.** |

### Frontend (Kiosk + Web Dashboard + Contract Tests)

| File:Line | Consumer | Assumes `elapsed_seconds == 0`? | Action Required |
|-----------|----------|-------------------------------|-----------------|
| `kiosk/src/components/PodKioskView.tsx:42-48` | Routes `billing.status === "waiting_for_game"` to "launching" view state | YES (implicitly — shows the LaunchTimerBanner with `elapsedSeconds={billing.elapsed_seconds ?? 0}`, which assumes 0 means launch elapsed time). | **CHANGE.** Branch on `billing.elapsed_seconds`: if 0 → "launching" view (existing). If > 0 → new "between_games" view with paused meter + Continue/End. |
| `kiosk/src/components/PodKioskView.tsx:402-475` | `LaunchTimerBanner` component — shows spinner + "Game Loading..." + 180s countdown | YES — assumes the elapsed_seconds is "time spent waiting for game to load," not "cumulative driving time so far." | **CHANGE: Bypass this component when `elapsed_seconds > 0`.** Show the new paused-meter UI instead. |
| `kiosk/src/components/SessionTimer.tsx:40` | Status pill: `waiting_for_game ? "Game Loading"` | YES — wrong label for between-games. | **SURFACE FIX 2.** Conditional: `elapsed_seconds == 0 ? "Game Loading" : "Between Games"` (or similar). |
| `kiosk/src/components/KioskPodCard.tsx:87, 118, 171` | KioskPodCard view-state mapping + status label + countdown control | YES at line 118 — label is "Waiting for Game" (which is correct for first-wait, wrong for between-games). | **SURFACE FIX 3.** Add elapsed_seconds branch to `statusLabel()` switch. Lines 87 and 171 are gate-only (loading state, don't decrement remaining) — those gates are still correct (don't decrement remaining when paused) and need NO change. |
| `kiosk/src/components/LiveSessionPanel.tsx:54, 115-118, 177` | "Game Loading" label + LaunchTimerBanner display + meter pause | YES — same issue as SessionTimer. | **CHANGE.** Mirror SessionTimer fix. Display new paused-meter UI for between-games (cumulative time + cost). |
| `kiosk/src/app/debug/page.tsx:1128` | Debug telemetry coverage filter — includes `"active"` and `"waiting_for_game"` sessions if `driving_seconds >= 120` | NO ASSUMPTION — filter is permissive and gates on `driving_seconds`, not status. | **No change.** Mid-stream sessions correctly included if they've driven > 2min. |
| `web/src/lib/api.ts:962, 957-980` | TypeScript type definition for `BillingSession` | NO assumption (just enum string list). | **CHANGE: Add `between_games_idle_seconds?: number` to the type.** Cascade rule. |
| `web/src/components/StatusBadge.tsx:5, 20, 28, 107` | Status pill label + color routing | YES at line 5 — `"waiting_for_game": "Loading..."` is the override. | **SURFACE FIX 3 (admin/web side).** Label needs to be conditional on elapsed_seconds — but admin dashboard may not always have access to elapsed_seconds in the badge context. Recommend: keep the static label as "Loading..." for first-wait, and broadcast a different status display name via the BillingSessionInfo (e.g., add a UI hint field) — OR — accept "Loading..." as a coarse label for both states on the admin dashboard. **Decision deferred to planner.** |
| `web/src/app/billing/page.tsx:140-212` | Admin billing page — gates pause/resume controls based on `isWaitingForGame` (hides pause/resume during waiting_for_game). | YES — assumes hide-controls is correct for both first-wait AND between-games. Actually this IS correct for both — you can't pause if no game is running, and the kiosk Continue/End buttons replace the pause/resume affordance. | **No change.** Coincidentally correct. |
| `packages/contract-tests/src/ws-dashboard.contract.test.ts:44-53` | Asserts `billing_tick_waiting.elapsed_seconds === 0` | YES — fixture explicitly asserts `elapsed_seconds == 0` for the WaitingForGame fixture. | **CHANGE: Fixture is for first-wait — keep as-is. ADD a NEW fixture `billing_tick_between_games` with `elapsed_seconds: 1500` (25min) and a contract test asserting it deserializes correctly.** |
| `packages/contract-tests/src/billing.contract.test.ts:7, 53, 56-57` | TS enum membership tests | NO assumption. | **No change** for the enum. **Add test for the new field.** |
| `packages/shared-types/src/billing.ts:11` | TS BillingSessionStatus enum | NO assumption (string union). | **No change** to the enum. **Add `between_games_idle_seconds?: number` to the BillingSession type if defined here** (verify in plan). |
| `crates/rc-agent/src/overlay.rs:66, 274, 842-879, 1549-1593` | Pod overlay — has its own `waiting_for_game` boolean (set by activate_v2() and reset when elapsed_seconds > 0 OR paused) | YES at line 878-880: `else if elapsed_seconds > 0 { data.waiting_for_game = false; data.game_live = true; }` | **VERIFY but likely no change.** The overlay already correctly switches off `waiting_for_game` once `elapsed_seconds > 0`. Mid-stream the overlay would NOT show the spinner — it would show the count-up meter. **However**: between games the overlay would show the meter still ticking (because `paused = false` from server's perspective if we just send `BillingTick` with status=WaitingForGame). The plan needs an explicit task: in the WaitingForGame mid-stream tick, send `BillingTick` to the agent with `paused: Some(true)` so the overlay shows PAUSED badge, not actively ticking. |

### Summary of Surface Fixes Required

1. **`api/billing_session.rs::stop_billing` (line 258-342)** — Branch on elapsed_seconds for `waiting_for_game` end path.
2. **`billing_timer_stale.rs` cleanup query (line 21-22)** — Add `AND driving_seconds = 0` so mid-stream WaitingForGame is not auto-cancelled by the 5-min stale rule.
3. **3 frontend label sites** — `SessionTimer.tsx:40`, `KioskPodCard.tsx:118`, `LiveSessionPanel.tsx:177` (kiosk-side; web `StatusBadge.tsx` is a deferred decision).
4. **Overlay paused state** — Send `paused: Some(true)` in the per-tick BillingTick to agent during mid-stream WaitingForGame, so the overlay shows PAUSED.

**Confidence: HIGH** — every line cited above was opened in the file and verified.

## Open Risk Resolutions (from spawn prompt risks 2-9)

### Risk 2 — Cancel/CancelNoPlayable from WaitingForGame
**Question:** Should we keep the existing `Cancel`/`CancelNoPlayable` transitions from WaitingForGame or differentiate first-wait cancel from between-games auto-end?

**Resolution:** **Keep both.** The differentiation happens at the *consumer* layer (api/billing_session.rs branches on `elapsed_seconds`), not the FSM layer. The FSM correctly says "from WaitingForGame, you can go to CancelledNoPlayable via Cancel (existing) OR Completed via End (new)." The runtime caller picks the right end status based on whether the customer drove. This is the cleanest separation and matches the existing pattern (e.g., `Active + End → Completed` AND `Active + Cancel → Cancelled` both exist; the API layer picks one).

### Risk 3 — Where exactly does Game::Stopped get observed?
**Question:** Where in `billing_game_status.rs` does game-state Running→Stopped get observed?

**Resolution:** **`billing_game_status.rs:296-358 — handle_game_off()`** is the function. It is called from `handle_game_status_update` (line 96-98) when `AcStatus::Off` arrives. The single-player branch is at line 322-331 and currently calls `end_billing_session(... EndedEarly)`. **This is the exact line to change.** Replace it with:

```rust
// Phase 414: Single-player game stopped — pause meter, enter mid-stream WaitingForGame
let mut timers = state.billing.active_timers.write().await;
if let Some(timer) = timers.get_mut(pod_id) {
    match crate::billing_fsm::validate_transition(timer.status, crate::billing_fsm::BillingEvent::GameStopped) {
        Ok(new_status) => {
            timer.status = new_status; // → WaitingForGame
            timer.between_games_idle_seconds = 0;
            timer.idle_warning_sent = false;
            tracing::info!("Phase 414: Game stopped on pod {}, billing paused (mid-stream WaitingForGame)", pod_id);
            // Persist status change to DB
            // Log pod_activity with kind="between_games_start"
            // Broadcast BillingSessionChanged
        }
        Err(e) => tracing::warn!("BILLING: GameStopped rejected for pod {}: {}", pod_id, e),
    }
}
```

(The exact shape will be determined by the plan; this snippet is illustrative.)

### Risk 4 — Tick loop interaction with FSM transitions
**Question:** Is it safe to add Active→WaitingForGame mid-tick?

**Resolution:** **YES, safe.** The transition is initiated from `handle_game_off` (a separate code path triggered by an inbound WS message from rc-agent), NOT from inside the tick loop. The tick loop reads `timer.status` and dispatches based on it. The locking model: the tick loop uses `try_write` on `active_timers` — if `handle_game_off` is currently holding the write lock for the FSM transition, the tick is skipped (one cycle, harmless). Once the lock is released, the next tick sees `WaitingForGame` and increments the idle counter. **No race, no deadlock.** Verified pattern: existing `handle_game_status_update::AcStatus::Pause` (line 77-95) does the exact same thing (write-lock + FSM transition + drop lock) and the tick loop tolerates it via `try_write`.

### Risk 5 — `BillingSessionInfo` serialization backward-compat
**Question:** Does adding `between_games_idle_seconds: Option<u32>` break old clients?

**Resolution:** **NO.** Verified by reading the existing `recovery_pause_seconds: Option<u32>` field at `billing.rs:194-199`. It's already in the struct and serialized as `Option<u32>` with the same shape. Old clients receive `null` for unknown fields; new clients receive `Some(N)`. The `billing_session_info_without_optional_fields_backward_compat` test at `types.rs:1615-1640` proves the round-trip works. **Add the new field with `#[serde(default, skip_serializing_if = "Option::is_none")]`** (or simply rely on serde's default behavior — verify the existing pattern).

### Risk 6 — Existing pattern for broadcasting `DashboardEvent::*` from a tick handler
**Question:** Find an example.

**Resolution:** **`DashboardEvent::BillingTick`** at `billing_timer.rs:105, 135, 183, 243, 361, 435` is the canonical example. The pattern:
1. Inside the per-tick mutation loop, push `DashboardEvent::Foo {...}` into `events_to_broadcast: Vec<DashboardEvent>`.
2. After the lock drops (line 293), `for event in events_to_broadcast { let _ = state.dashboard_tx.send(event); }` (line 448-450).

For `IdleWarning`, the wallet balance lookup must happen *after* the lock drop. Pattern:
1. Inside the lock, when idle_seconds == 600 and !idle_warning_sent: push `(pod_id, session_id, wallet_owner_id, rate)` into `idle_warnings_to_emit: Vec<...>`. Set `timer.idle_warning_sent = true`.
2. After lock drop: for each entry, query wallet balance, compute `can_continue = balance >= rate`, push `DashboardEvent::IdleWarning {...}` into `events_to_broadcast`. Then send.

This mirrors the per-minute debit pattern at `billing_timer.rs:323-396` exactly.

### Risk 7 — Existing test pattern for FSM transitions
**Question:** Find one.

**Resolution:** **`billing_fsm.rs::tests` mod (line 221-418)** is the canonical pattern. 33 tests, each a 4-line function:
```rust
#[test]
fn test_active_pause() {
    let result = validate_transition(BillingSessionStatus::Active, BillingEvent::Pause);
    assert_eq!(result, Ok(BillingSessionStatus::PausedGamePause));
}
```

Plan should add 3 new tests in this style:
1. `test_active_game_stopped_to_waiting`
2. `test_waiting_end_to_completed`
3. `test_waiting_end_early_to_ended_early`

Plus 2 negative tests:
4. `test_completed_game_stopped_rejected` (terminal state)
5. `test_pending_game_stopped_rejected` (illegal transition)

### Risk 8 — Existing test pattern for end-to-end billing flow tests
**Question:** Find one.

**Resolution:** **`billing_tests.rs`** has the snap-pricing test pattern at lines 373-399. The integration test for cumulative snap should be:

```rust
#[test]
fn cumulative_snap_25min_ac_then_5min_f1_yields_pkg_30() {
    let mut timer = BillingTimer {
        elapsed_seconds: 0,
        total_debited_paise: 0,
        ..BillingTimer::default()
    };

    // Phase 1: 25 minutes of Active driving in AC
    for _ in 0..(25 * 60) {
        timer.tick(); // increments elapsed_seconds
    }
    assert_eq!(timer.elapsed_seconds, 1500);
    let cost_at_25 = timer.snap_debit_amount() + timer.total_debited_paise as i32;
    assert_eq!(cost_at_25, 62500); // 25 × ₹25 = ₹625 (per-minute, below 30-min snap)

    // Phase 2: GameStopped → WaitingForGame, no ticks elapse on driving counter
    timer.status = BillingSessionStatus::WaitingForGame;
    for _ in 0..(7 * 60) {
        timer.tick(); // no-op for elapsed_seconds; idle counter would tick (verify if added)
    }
    assert_eq!(timer.elapsed_seconds, 1500); // unchanged

    // Phase 3: GameLive → Active, 5 more minutes
    timer.status = BillingSessionStatus::Active;
    for _ in 0..(5 * 60) {
        timer.tick();
    }
    assert_eq!(timer.elapsed_seconds, 1800); // 30 min total
    let final_cost = crate::billing_pricing::snap_cost_for_minutes(30, 2500, 70000, 90000);
    assert_eq!(final_cost, 70000); // ₹700 — snap to 30-min package, NOT 30 × ₹25 = ₹750
}
```

**Note:** This is a unit-style integration test (no DB, no async runtime). For a true E2E with DB + tx commit, the existing `BillingManager::new()` test fixture at `billing_tests.rs:761` is the template (in-memory state with test data).

### Risk 9 — PausedDisconnect from WaitingForGame — does this transition need to exist?
**Question:** What if the pod goes offline during WaitingForGame?

**Resolution:** **NO new transition needed.** Reasoning:
- The existing tick loop at `billing_timer.rs:153-208` checks `pod_is_offline` for **Active timers only** (line 148: `if timer.status != BillingSessionStatus::Active { continue; }`).
- For WaitingForGame timers (mid-stream), the tick loop currently does nothing (after the new idle-counter increment, it does that and continues). The disconnect path is not entered.
- This is **correct behavior**: the meter is already paused. There's no value in a separate "PausedDisconnect from WaitingForGame" state because the customer-favourable invariant (no charge during pod offline) is already satisfied by virtue of being in WaitingForGame.
- If the pod is offline for the full 15 minutes, the auto-end at 900s fires and ends the session as Completed. Customer is charged the cumulative cost from before they walked away — which is correct.
- If the customer comes back and the pod is still offline, their game launch will fail, the FSM stays in WaitingForGame, and the auto-end fires normally.

**No transition needed. No code change for the disconnect path.** Plan should add a single test that verifies "pod offline + WaitingForGame mid-stream + 16 min → session auto-ends as Completed with cumulative cost."

## Code Examples

Verified patterns from local source (no external sources needed — all internal):

### Adding a new BillingEvent

```rust
// crates/racecontrol/src/billing_fsm.rs (existing enum)
pub enum BillingEvent {
    StartWaiting,
    GameLive,
    Pause,
    Disconnect,
    PauseManual,
    Resume,
    End,
    EndEarly,
    Cancel,
    CancelNoPlayable,
    CrashPause,
    // Phase 414: Add this variant
    GameStopped,
}
```

### Adding rows to the transition table

```rust
// crates/racecontrol/src/billing_fsm.rs::TRANSITION_TABLE (append)
const TRANSITION_TABLE: &[(BillingSessionStatus, BillingEvent, BillingSessionStatus)] = &[
    // ... existing 23 rows ...
    // Phase 414: Active → WaitingForGame on game stop (mid-stream)
    (BillingSessionStatus::Active, BillingEvent::GameStopped, BillingSessionStatus::WaitingForGame),
    // Phase 414: End from mid-stream WaitingForGame
    (BillingSessionStatus::WaitingForGame, BillingEvent::End, BillingSessionStatus::Completed),
    (BillingSessionStatus::WaitingForGame, BillingEvent::EndEarly, BillingSessionStatus::EndedEarly),
];
```

### Adding the BillingTimer field

```rust
// crates/racecontrol/src/billing.rs (BillingTimer struct, append)
pub struct BillingTimer {
    // ... existing fields ...

    /// Phase 414: Seconds elapsed in mid-stream WaitingForGame (between games).
    /// Resets to 0 on every WaitingForGame → Active transition.
    /// In-memory only — NOT persisted (lost on server restart, customer-favourable).
    pub between_games_idle_seconds: u32,

    /// Phase 414: Whether the 10-min IdleWarning has been broadcast for this between-games wait.
    /// Resets to false on every WaitingForGame → Active transition.
    pub idle_warning_sent: bool,
}
```

### IdleWarning DashboardEvent variant

```rust
// crates/rc-common/src/protocol.rs (DashboardEvent enum, append)
pub enum DashboardEvent {
    // ... existing variants ...

    /// Phase 414: Mid-session idle warning at 10-min mark (5 min before auto-end).
    /// Kiosk shows modal with countdown + balance check.
    IdleWarning {
        pod_id: String,
        session_id: String,
        balance_paise: u64,
        seconds_remaining: u32, // 300 at 10-min mark
        can_continue: bool,     // false if balance < rate_paise_per_minute
    },
}
```

### BillingSessionInfo additive field

```rust
// crates/rc-common/src/types.rs (BillingSessionInfo struct, append)
pub struct BillingSessionInfo {
    // ... existing fields ...

    /// Phase 414: Mid-stream idle counter (Some only when status == WaitingForGame AND elapsed_seconds > 0).
    /// Used by kiosk to display the auto-end countdown after the 10-min warning.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub between_games_idle_seconds: Option<u32>,
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Game stop → end billing session immediately | (Phase 414) Game stop → pause meter, enter WaitingForGame | This phase | Customers can swap games without losing snap pricing, no double-charge |
| `WaitingForGame` only used for first-time wait | (Phase 414) Reused for both first-wait AND between-games | This phase | Saves a `rc-common` protocol bump and fleet rc-agent redeploy |
| `BillingTimer.tick()` for `WaitingForGame` is a no-op | (Phase 414) Increments idle counter, fires warning, auto-ends | This phase | Active server-side enforcement of 15-min idle |
| 24hr session safety cap (`max_session_seconds = 86400`, set in `e3d05cea`) | UNCHANGED — still applies | n/a | Protects against runaway sessions even with continuous play |
| Snap pricing computed from cumulative `elapsed_seconds` | UNCHANGED — already correct, just needs an integration test to prove it works across game swaps | n/a | The whole reason this phase is cheap to implement |

**Deprecated/outdated:**
- The single-player `end_billing_session(... EndedEarly)` call in `handle_game_off` (line 329) — replaced with FSM transition.
- The "Refunded {}p for staff-cancelled waiting_for_game session" log at `api/billing_session.rs:295` — needs a branch for between-games.

## Environment Availability

This phase is purely backend Rust + frontend TypeScript code change. No external tools, services, runtimes, or CLI utilities introduced.

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust toolchain | All Rust changes | ✓ (verified per memory: 1.93.1 stable) | 1.93.1 | — |
| `cargo` | Build + test | ✓ | workspace pin | — |
| Node.js | Kiosk Next.js build | ✓ (James .27: v22.22.0; Server .23: v24.14.0) | 22.x / 24.x | — |
| sqlite (server runtime) | Existing billing_sessions table writes | ✓ | bundled with sqlx | — |

**Missing dependencies with no fallback:** None.
**Missing dependencies with fallback:** None.

## Validation Architecture (Nyquist enabled per `.planning/config.json`)

### Test Framework

| Property | Value |
|----------|-------|
| Framework (Rust) | `cargo test` (per-crate). Workspace packages: `rc-common`, `racecontrol-crate`, `rc-agent-crate`. |
| Framework (TS) | `vitest` (per `packages/contract-tests/`); `jest` for kiosk component tests if any |
| Config file (Rust) | `Cargo.toml` workspace; per-crate `[[test]]` blocks; built-in `#[cfg(test)]` mods |
| Config file (TS) | `packages/contract-tests/vitest.config.ts` (existing) |
| Quick run command | `cargo test -p racecontrol-crate --lib billing_fsm` (FSM tests only, ~3s) |
| Quick run command (broader) | `cargo test -p racecontrol-crate --lib billing` (all billing tests, ~30s) |
| Full suite command | `cargo test -p rc-common && cargo test -p racecontrol-crate && cargo test -p rc-agent-crate` |
| Pre-commit hook (already enforced) | runs `cargo test -p rc-common && cargo test -p racecontrol-crate --lib` per spawn prompt |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| 414-FSM-01 | `BillingEvent::GameStopped` enum variant exists | unit (compile-time) | `cargo build -p racecontrol-crate` | ✅ existing module |
| 414-FSM-02 | `Active + GameStopped → WaitingForGame` | unit | `cargo test -p racecontrol-crate --lib billing_fsm::tests::test_active_game_stopped_to_waiting` | ❌ Wave 0 — add to `billing_fsm.rs::tests` |
| 414-FSM-03 | `WaitingForGame + End → Completed` | unit | `cargo test ... test_waiting_end_to_completed` | ❌ Wave 0 |
| 414-FSM-04 | `WaitingForGame + EndEarly → EndedEarly` | unit | `cargo test ... test_waiting_end_early_to_ended_early` | ❌ Wave 0 |
| 414-FSM-05 | `Completed + GameStopped` rejected | unit | `cargo test ... test_completed_game_stopped_rejected` | ❌ Wave 0 |
| 414-TIMER-01 | `BillingTimer.between_games_idle_seconds` increments only when status == WaitingForGame | unit | `cargo test ... timer_idle_counter_advances_only_in_waiting` | ❌ Wave 0 |
| 414-TIMER-02 | Idle counter resets to 0 on WaitingForGame → Active | unit | `cargo test ... timer_idle_counter_resets_on_resume` | ❌ Wave 0 |
| 414-TIMER-03 | At 600s, `idle_warning_sent` becomes true exactly once | unit | `cargo test ... idle_warning_fires_at_600s_once` | ❌ Wave 0 |
| 414-TIMER-04 | At 900s, session auto-ends as Completed | unit (no DB) | `cargo test ... idle_auto_ends_at_900s_completed` | ❌ Wave 0 |
| 414-PROTOCOL-01 | `DashboardEvent::IdleWarning` round-trips through serde | unit | `cargo test -p rc-common --lib test_idle_warning_serde_roundtrip` | ❌ Wave 0 — add to `protocol.rs::tests` |
| 414-PROTOCOL-02 | `BillingSessionInfo.between_games_idle_seconds` round-trips (Some + None) | unit | `cargo test -p rc-common --lib test_billing_info_idle_seconds_roundtrip` | ❌ Wave 0 |
| 414-INTEGRATION-01 | 25min Active → GameStopped → 7min wait → GameLive → 5min Active → cumulative snap == ₹700 | integration (in-memory) | `cargo test -p racecontrol-crate --lib cumulative_snap_25_5_yields_pkg_30` | ❌ Wave 0 — add to `billing_tests.rs` |
| 414-INTEGRATION-02 | 16-min idle from mid-stream WaitingForGame triggers auto-end as Completed with correct cumulative cost | integration | `cargo test ... idle_auto_end_completes_with_cumulative_cost` | ❌ Wave 0 |
| 414-INTEGRATION-03 | Pod offline during WaitingForGame mid-stream + 16 min → auto-end as Completed | integration | `cargo test ... pod_offline_in_waiting_auto_ends_completed` | ❌ Wave 0 |
| 414-INTEGRATION-04 | `stop_billing` HTTP endpoint with elapsed_seconds==0 → CancelledNoPlayable + refund (existing); with elapsed_seconds>0 → EndedEarly + bill cumulative | integration with DB fixture | `cargo test --test billing_session_e2e stop_billing_branches_on_elapsed` | ❌ Wave 0 — needs new e2e test file |
| 414-CONTRACT-01 | TS `BillingSession.between_games_idle_seconds` type matches Rust shape | unit (TS) | `cd packages/contract-tests && npx vitest run billing.contract.test.ts` | ✅ existing — add new assertion |
| 414-CONTRACT-02 | `IdleWarning` event TS shape matches Rust shape | unit (TS) | `cd packages/contract-tests && npx vitest run ws-dashboard.contract.test.ts` | ✅ existing — add new fixture + test |
| 414-FRONTEND-01 | Kiosk staff page renders Continue/End buttons when status=WaitingForGame AND elapsed_seconds>0 | component test (visual + Playwright) | Manual + `e2e-regression` if added | ❌ Wave 0 — Playwright spec optional |
| 414-FRONTEND-02 | IdleWarningModal renders + countdown decrements + Continue resets via game launch | component test | Manual verification at venue | ❌ Manual-only (acceptable per existing kiosk pattern) |
| 414-FINANCIAL-E2E | At venue: customer with ₹X balance plays AC for 10min, swaps to F1 25 for 10min, ends — wallet shows correct ₹400 debit (snap to 20min × ₹25=₹500 vs cumulative=₹500; or whatever the math says) | manual at venue | n/a | **CLAUDE.md MANDATE** — must run before deploy. |

### Sampling Rate

- **Per task commit:** `cargo test -p rc-common && cargo test -p racecontrol-crate --lib billing` (covers FSM, timer, billing core; ~30s).
- **Per wave merge:** `cargo test -p rc-common && cargo test -p racecontrol-crate && cargo test -p rc-agent-crate` (full Rust suite, ~5 min). Plus `cd packages/contract-tests && npx vitest run` (~10s).
- **Phase gate (before `/gsd:verify-work`):** Full Rust suite + TS contract tests + manual venue financial E2E + MMA audit.

### Wave 0 Gaps

- [ ] **`billing_fsm.rs::tests`** — add 5 new unit tests (FSM-02..05). Existing module is the template; just append.
- [ ] **`billing_tests.rs`** — add 6 new tests (TIMER-01..04, INTEGRATION-01..03). Existing module has 100+ tests; follow the snap_debit_normal_at_15 pattern for the cumulative snap test.
- [ ] **`crates/racecontrol/tests/billing_session_e2e.rs`** — NEW file for INTEGRATION-04. Existing `tests/` directory has integration-test patterns to mirror (e.g., from billing_atomicity Phase 314).
- [ ] **`protocol.rs::tests`** — add 2 round-trip tests (PROTOCOL-01, PROTOCOL-02). Mirror existing `test_dashboard_event_session_paused`.
- [ ] **`packages/contract-tests/src/fixtures/ws-dashboard.json`** — add `idle_warning` and `billing_tick_between_games` fixtures.
- [ ] **`packages/contract-tests/src/ws-dashboard.contract.test.ts`** — add 2 tests for the new fixtures.
- [ ] **`packages/contract-tests/src/billing.contract.test.ts`** — add `between_games_idle_seconds` field assertion.
- [ ] No new framework install needed — `cargo test` and `vitest` already configured.

## Sources

### Primary (HIGH confidence — direct file reads)

- `crates/racecontrol/src/billing_fsm.rs` (411 lines) — TRANSITION_TABLE, validate_transition, authoritative_end_session, 33 existing tests
- `crates/racecontrol/src/billing.rs` (409 lines) — BillingTimer struct, snap_debit_amount, current_cost, tick(), BillingManager
- `crates/racecontrol/src/billing_timer.rs` (508 lines) — tick_all_timers, lock pattern, BillingTick broadcast, BILL-05 first-wait broadcast
- `crates/racecontrol/src/billing_game_status.rs` (466 lines) — handle_game_status_update, handle_game_off, handle_live_resume, handle_precommitted_live, handle_single_player_live, handle_crashed_waiting_entry, handle_game_error
- `crates/racecontrol/src/billing_session_lifecycle.rs` (517 lines) — handle_dashboard_command, set_billing_status, finalize_billing_start, resume_billing_from_disconnect, BillingStartData
- `crates/racecontrol/src/billing_session_end.rs` — CAS list including `'waiting_for_game'`, orphan recovery
- `crates/racecontrol/src/api/billing_session.rs` — stop_billing handler with FSM workaround at line 258-342
- `crates/racecontrol/src/api/billing_start.rs` — initial waiting_for_game INSERT
- `crates/racecontrol/src/billing_timer_stale.rs` — LBILL stale-session cleanup (the one that needs the `driving_seconds = 0` filter)
- `crates/racecontrol/src/billing_timer_expiry_timeout.rs` — launch timeout handler
- `crates/racecontrol/src/billing_recovery.rs` — venue shutdown recovery
- `crates/racecontrol/src/billing_jobs.rs` — coupon reservation cleanup
- `crates/racecontrol/src/visits.rs` — visit/session active count
- `crates/racecontrol/src/bot_coordinator.rs`, `bot_coordinator_recovery.rs` — SessionStuckWaitingForGame anomaly routing
- `crates/racecontrol/src/auth/token_consume.rs` — Pod state update to WaitingForGame
- `crates/racecontrol/src/billing_pricing.rs` — snap_cost_for_minutes
- `crates/racecontrol/src/billing_tests.rs` — existing 100+ tests, snap-pricing pattern at line 373
- `crates/racecontrol/src/db/migrate_billing.rs` — DB schema constraints (verified `'waiting_for_game'` is in CHECK)
- `crates/rc-common/src/types.rs` — BillingSessionStatus enum (line 364), BillingSessionInfo struct (line 404), backward-compat tests
- `crates/rc-common/src/protocol.rs` — DashboardEvent enum (line 1175), CoreToAgentMessage::BillingTick (line 863), tagged-union serde
- `crates/rc-agent/src/billing_guard.rs` — agent-side stuck-session detection
- `crates/rc-agent/src/overlay.rs` — pod overlay state (waiting_for_game boolean already separate from elapsed_seconds)
- `kiosk/src/components/PodKioskView.tsx` — viewState routing
- `kiosk/src/components/SessionTimer.tsx` — status pill labels
- `kiosk/src/components/KioskPodCard.tsx` — pod card UI
- `kiosk/src/components/LiveSessionPanel.tsx` — live session panel + LaunchTimerBanner
- `kiosk/src/app/debug/page.tsx` — debug telemetry page
- `web/src/lib/api.ts` — TypeScript BillingSession type
- `web/src/components/StatusBadge.tsx` — admin badge labels
- `web/src/app/billing/page.tsx` — admin billing controls
- `packages/shared-types/src/billing.ts` — TS BillingSessionStatus type
- `packages/contract-tests/src/billing.contract.test.ts` — billing contract test
- `packages/contract-tests/src/ws-dashboard.contract.test.ts` — WS contract test (existing fixture asserts elapsed_seconds == 0 for waiting_for_game)
- `packages/contract-tests/src/fixtures/ws-dashboard.json` — billing_tick_waiting fixture
- `.planning/phases/414-continuous-billing-session/414-CONTEXT.md` — locked design contract from Uday session
- `.planning/REQUIREMENTS.md` — v49.0 requirements
- `.planning/STATE.md` — current phase status, recent commits
- `.planning/config.json` — nyquist_validation: true confirmed
- `CLAUDE.md` (via system reminder) — all standing rules

### Secondary (MEDIUM confidence)
None — every critical claim is from direct file read.

### Tertiary (LOW confidence)
None.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — no new dependencies, all existing crates in workspace
- Architecture: HIGH — every code location verified by direct file open or grep
- Pitfalls: HIGH — drawn from CLAUDE.md standing rules + 100+ existing billing tests
- WaitingForGame consumer audit: HIGH — exhaustive grep across `crates/`, `kiosk/`, `web/`, `packages/`; every match opened and classified
- Validation: HIGH — Wave 0 gaps map to existing test files with established patterns

**Research date:** 2026-04-18
**Valid until:** 2026-05-18 (30 days — billing FSM is stable; only invalidates if a parallel phase changes BillingTimer struct shape or DashboardEvent enum)
