# Phase 253: State Machine Hardening - Context

**Gathered:** 2026-03-29
**Status:** Ready for planning
**Mode:** Auto-generated (infrastructure phase, discuss skipped)

<domain>
## Phase Boundary

Billing and game states are always consistent — phantom billing and free gaming are structurally impossible. This phase adds a formal FSM transition table, cross-FSM invariant guards, crash recovery atomicity, and split session modeling.

Requirements: FSM-01 (transition table), FSM-02 (billing=active requires game≠Idle), FSM-03 (game=Running requires billing≠cancelled), FSM-04 (crash recovery pauses billing atomically), FSM-05 (StopGame handled in every recovery state), FSM-06 (single authoritative end-session trigger), FSM-07 (split session modeling), FSM-08 (split transition persisted before launch)

Depends on: Phase 252 (atomicity layer must exist before state guards are added)

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — infrastructure phase. Key guidance from MMA audit:

- FSM-01: Create a `BillingTransitionTable` with explicit `(current_state, event) → new_state | REJECT` mappings. Validate in `end_billing_session`, `pause_billing`, `resume_billing`. Log rejected transitions at WARN.
- FSM-02: In the billing tick loop (or health check), detect billing=active + game=Idle for >30s → auto-pause billing + ERROR log. Don't auto-end — just pause and alert.
- FSM-03: In game_launcher before sending LaunchGame, verify billing session exists and is active. If not → reject launch with error.
- FSM-04: The crash handler in ws_handler.rs must set billing to paused_game_pause BEFORE any relaunch attempt. Currently this happens but may not be atomic with the crash detection.
- FSM-05: Every CrashRecoveryState variant must handle StopGame. In PausedWaitingRelaunch → cancel relaunch, transition to Idle. In AutoEndPending → same.
- FSM-06: Both billing pause timeout (10 min) and recovery auto-end must converge on a single `authoritative_end_session()` that acquires the CAS lock from Phase 252.
- FSM-07: Add `split_sessions` table: parent_session_id, split_number, allocated_seconds, status. Each split is an immutable child entitlement.
- FSM-08: Before any split-2 launch, the split record must be committed to DB. No launch without persisted state.

</decisions>

<code_context>
## Existing Code Insights

### Key Files
- `crates/racecontrol/src/billing.rs` — BillingTimer, end_billing_session (now with CAS from Phase 252), billing states
- `crates/racecontrol/src/game_launcher.rs` — GameManager, GameTracker, game state transitions
- `crates/rc-agent/src/ws_handler.rs` — Agent-side crash recovery FSM (CrashRecoveryState)
- `crates/rc-agent/src/event_loop.rs` — CrashRecoveryState machine definition
- `crates/rc-common/src/types.rs` — BillingSessionStatus enum, GameState enum
- `crates/racecontrol/src/api/routes.rs` — billing endpoints (start, stop, pause, resume)

### Established Patterns
- CAS guard on session finalization (Phase 252: UPDATE WHERE status='active')
- Snapshot under lock + drop before await
- compute_refund() for unified refund calculation

</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase.

</specifics>

<deferred>
## Deferred Ideas

None.

</deferred>
