---
phase: 368-live-launch-status-with-autonomous-debug
plan: 01
subsystem: api
tags: [rust, launch-state-machine, websocket, serde, phase-62-contracts, tokio]

# Dependency graph
requires: []
provides:
  - LaunchState enum (5 variants) with Phase 62 snake_case serialization contract
  - LaunchOrigin enum (4 variants)
  - LaunchStatusCard and LaunchNoteEvent structs in rc-common/protocol.rs
  - DashboardEvent::LaunchStatusChanged and DashboardEvent::LaunchNoteAdded variants
  - AgentMessage::LaunchStatusUpdate variant
  - CoreToAgentMessage::LaunchGame.launch_id field (Option<String>, serde(default) for backward compat)
  - LaunchStateMachine in-memory store (RwLock<HashMap>, monotonic + terminal-state invariants)
  - AppState.launch_state_machine field (Arc<LaunchStateMachine>)
  - launch_id minted once at top of launch_game(), threaded through tracker and LaunchGame message
  - LaunchStarted card emitted on launch accept, NeedsManualIntervention on billing reject, IssueFixed at playable_at
  - D-15 sanitization: billing-reject detail always "Launch blocked — billing not ready", no customer PII
  - 5-min auto-dismiss tokio::spawn after IssueFixed
affects:
  - 368-02 (rc-agent tier_engine LaunchStatusUpdate emissions use these types)
  - 368-03 (REST endpoints + DB persistence use LaunchStateMachine.get_active/dismiss)
  - 368-04 (kiosk TypeScript literal union must match Phase 62 JSON values)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "LaunchStateMachine lock-and-drop: every RwLock guard in its own { } block dropped before any .await"
    - "Phase 62 cross-boundary value contract tests: assert_eq!(serde_json::to_string(&Enum::Variant).unwrap(), '\"exact_string\"')"
    - "Billing-rejection D-15 pattern: emit card before return Err, hardcoded sanitized detail string only"
    - "playable_id capture pattern: capture id inside write lock, emit IssueFixed after lock drops"

key-files:
  created:
    - crates/rc-common/tests/launch_status_serde.rs
    - crates/rc-common/tests/launch_status_value_contract.rs
    - crates/racecontrol/src/launch_state.rs
    - crates/racecontrol/tests/launch_state_machine.rs
    - crates/racecontrol/tests/launch_id_threading.rs
  modified:
    - crates/rc-common/src/protocol.rs (new types + enum variants)
    - crates/rc-common/src/types.rs (cascade: launch_id: None in test fixture)
    - crates/racecontrol/src/lib.rs (pub mod launch_state)
    - crates/racecontrol/src/state.rs (launch_state_machine field)
    - crates/racecontrol/src/game_launcher.rs (launch_id threading + card emissions)
    - crates/racecontrol/src/ac_server.rs (cascade: launch_id: None)
    - crates/racecontrol/src/api/routes.rs (cascade: launch_id: None)
    - crates/racecontrol/src/auth/mod.rs (cascade: launch_id: None)
    - crates/racecontrol/src/billing.rs (cascade: launch_id: None)
    - crates/racecontrol/src/multiplayer.rs (cascade: launch_id: None)

key-decisions:
  - "D-15 hardcoded literal: billing-reject detail is ALWAYS 'Launch blocked — billing not ready' regardless of specific billing error subtype to prevent customer PII in card"
  - "LaunchStarted emitted before tracker write lock acquired, TOCTOU path emits NeedsManualIntervention after dropping all locks"
  - "IssueFixed uses happy-path skip (LaunchStarted -> IssueFixed direct) because no AI fix was needed for a clean launch"
  - "playable_launch_id captured inside games write lock, emitted after lock drops to comply with CLAUDE.md no-lock-across-await rule"
  - "5-min auto-dismiss via tokio::spawn(sleep(300s)) per D-11 — no separate background task needed at this scale"
  - "make_launch_message trait signature extended to accept launch_id: String so rc-agent receives the exact same UUID stored in LaunchStateMachine"

patterns-established:
  - "Phase 62 cross-boundary serde value contract: separate test file asserts EXACT JSON strings for every enum variant"
  - "Lock-and-drop before .await: capture data inside lock scope, process after scope closes"
  - "D-15 sanitization: billing-error detail is a hardcoded invariant, never interpolated from billing state"

requirements-completed: [LLS-01, LLS-02, LLS-03, LLS-10]

# Metrics
duration: 165min
completed: 2026-04-11
---

# Phase 368 Plan 01: Protocol Types + LaunchStateMachine + launch_id Threading Summary

**Typed LaunchState/LaunchOrigin enums with Phase 62 contract tests, in-memory LaunchStateMachine (monotonic + terminal-state guards), and launch_id threaded end-to-end from server mint to rc-agent message with billing-reject and playable-transition card emissions.**

## Performance

- **Duration:** ~165 min (split across two sessions)
- **Started:** 2026-04-11 ~17:30 IST
- **Completed:** 2026-04-11 ~18:55 IST
- **Tasks:** 3/3
- **Files modified:** 14

## Accomplishments

- LaunchState (5 variants) and LaunchOrigin (4 variants) enums serialize to exact snake_case JSON strings required by Plan 04 kiosk TypeScript literal union. Phase 62 value-contract tests enforce these strings will never silently drift.
- LaunchStateMachine in-memory store with monotonic invariant (only forward transitions), terminal-state guard (IssueFixed/NeedsManualIntervention block further transitions), prune cap (100 cards, 10-min stale eviction), and P2-05 idempotency proof (second IssueFixed returns None).
- launch_id minted once at top of launch_game(), used in GameTracker, threaded through make_launch_message trait to CoreToAgentMessage::LaunchGame — rc-agent receives the same UUID stored in LaunchStateMachine.
- Billing-reject paths (FSM-03, paused, TOCTOU) emit NeedsManualIntervention card with D-15 hardcoded detail "Launch blocked — billing not ready" — zero customer PII, locks dropped before all .await calls.
- IssueFixed emitted at playable_at transition with 5-min auto-dismiss (D-11). Terminal-state guard prevents duplicate emission if rc-agent also emits IssueFixed first (P2-05 race safety).
- 13 new tests: 5 Phase 62 contract/serde tests (rc-common), 5 LaunchStateMachine unit tests (racecontrol), 4 threading integration tests (racecontrol).

## Task Commits

Each task was committed atomically:

1. **Task 1: Define protocol contracts in rc-common and write contract tests** - `367b949c` (feat)
2. **Task 2: Implement LaunchStateMachine module with TDD unit tests** - `0b334d67` (feat)
3. **Task 3: Thread launch_id through launch pipeline + emit launch_started/issue_fixed/billing-reject card** - `71618d7e` (feat)

## Files Created/Modified

**Created:**
- `crates/rc-common/tests/launch_status_serde.rs` — 4 DashboardEvent/AgentMessage roundtrip tests including backward-compat (JSON without launch_id decodes to None)
- `crates/rc-common/tests/launch_status_value_contract.rs` — Phase 62 contract: asserts exact JSON strings for all 5 LaunchState + 4 LaunchOrigin variants
- `crates/racecontrol/src/launch_state.rs` — LaunchStateMachine: HashMap<String, LaunchStatusCard> under RwLock, lock-and-drop pattern throughout, monotonic + terminal invariants, prune
- `crates/racecontrol/tests/launch_state_machine.rs` — 5 unit tests: transition/dismiss/concurrent-no-deadlock/prune-cap/P2-05-idempotency
- `crates/racecontrol/tests/launch_id_threading.rs` — 4 integration tests: threaded/unified/D-15-sanitized/issue-fixed-on-playable

**Modified:**
- `crates/rc-common/src/protocol.rs` — New types block (LaunchState, LaunchOrigin, LaunchStatusCard, LaunchNoteEvent), DashboardEvent::LaunchStatusChanged + LaunchNoteAdded variants, AgentMessage::LaunchStatusUpdate variant, CoreToAgentMessage::LaunchGame.launch_id field
- `crates/racecontrol/src/game_launcher.rs` — launch_id mint moved before billing gate, make_launch_message trait updated to accept launch_id:String, card emissions at billing-reject/launch-accept/playable-transition paths
- `crates/racecontrol/src/launch_state.rs` — new module
- `crates/racecontrol/src/lib.rs` — pub mod launch_state
- `crates/racecontrol/src/state.rs` — launch_state_machine: Arc<LaunchStateMachine> field + init
- Cascade updates (launch_id: None): `crates/rc-common/src/types.rs`, `crates/racecontrol/src/ac_server.rs`, `crates/racecontrol/src/api/routes.rs`, `crates/racecontrol/src/auth/mod.rs`, `crates/racecontrol/src/billing.rs`, `crates/racecontrol/src/multiplayer.rs`

## Decisions Made

- **D-15 hardcoded literal**: billing-reject detail is ALWAYS the exact string "Launch blocked — billing not ready" regardless of which billing check failed (no active session, paused, TOCTOU expired). This prevents customer PII from ever appearing in the card.
- **LaunchStarted emitted before tracker write lock**: In Task 3 Step B, the start_launch() call happens before acquiring the active_games write lock. This avoids holding the lock across the start_launch .await (which acquires its own internal RwLock).
- **Happy-path skip (LaunchStarted → IssueFixed)**: When a game launches cleanly with no AI analysis or retry, the card transitions directly from LaunchStarted to IssueFixed. This is an explicitly allowed transition in is_valid_transition().
- **make_launch_message trait accepts launch_id: String**: The trait takes ownership (not reference) since it needs to move the value into the CoreToAgentMessage struct. All 4 launcher impls updated.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Cascade: CoreToAgentMessage::LaunchGame struct literal mismatches**
- **Found during:** Task 1 (cargo check after adding launch_id field to protocol.rs)
- **Issue:** 12 E0063 errors across ac_server.rs, api/routes.rs, auth/mod.rs, billing.rs, game_launcher.rs, multiplayer.rs — all struct literal constructions of LaunchGame missing launch_id field
- **Fix:** Added `launch_id: None` to each call site (non-billing-gate paths kept None; billing gate updated in Task 3)
- **Files modified:** 6 files
- **Committed in:** `367b949c` (part of Task 1 commit)

**2. [Rule 1 - Bug] Wrong SimType variant in types.rs test fixture**
- **Found during:** Task 1 (cargo test failure)
- **Issue:** Test used `SimType::AssettoCorsaCompetizione` which doesn't exist; correct variant is `SimType::AssettoCorsa`
- **Fix:** Changed to `SimType::AssettoCorsa` in types.rs test
- **Committed in:** `367b949c`

**3. [Rule 1 - Bug] Backward-compat test used wrong JSON format**
- **Found during:** Task 1 (test failure)
- **Issue:** Test deserialized bare `{"sim_type":...}` but CoreToAgentMessage uses tagged serde `#[serde(tag = "type", content = "data")]`
- **Fix:** Fixed to `{"type":"launch_game","data":{...without launch_id...}}`
- **Committed in:** `367b949c`

**4. [Rule 1 - Bug] Comment contained .unwrap() triggering production grep check**
- **Found during:** Task 2 (grep -c '\.unwrap()' check)
- **Issue:** Comment line `// NO .unwrap() in production code` matched the grep pattern
- **Fix:** Changed comment to `// No unwrap() in production code`
- **Committed in:** `0b334d67`

---

**Total deviations:** 4 auto-fixed (3x Rule 3/1 blocking, 1x Rule 1 bug)
**Impact on plan:** All auto-fixes required for correctness. No scope creep. Plan truths fully satisfied.

## Issues Encountered

- **TOCTOU path lock analysis**: The TOCTOU billing check is inside a `let info = { ... }` block that holds `state.game_launcher.active_games.write().await`. The billing reject needed to call `state.launch_state_machine.transition().await` inside that block. Resolution: the TOCTOU code already calls `drop(waiting); drop(timers); drop(games)` before the return — so the transition `.await` happens after all locks are dropped. CLAUDE.md rule satisfied.
- **start_launch split line broke acceptance grep**: The Step B `state.launch_state_machine.start_launch` call was split across two lines preventing `grep -c 'launch_state_machine.start_launch'` from returning 2. Fixed by moving to single line.

## Deploy Manifest

```
rust_binary: [racecontrol]
frontend_rebuild: none
config_change: none
db_migration: none
infrastructure: none
data_files: none
bat_file: none
cloud_parity: [binary — racecontrol server + Bono VPS]
targets: [server .23, cloud VPS]
```

**Note:** This plan is feature-gated (no user-visible behavior). The binary must be deployed to activate the new WebSocket events. Plans 02, 03, 04 must complete before the feature is customer-visible.

## Known Stubs

None — no data flows to UI in this plan. All card emissions are server-side only; no kiosk reads them until Plan 04.

## Next Phase Readiness

- Plan 02 (rc-agent tier_engine) can now use AgentMessage::LaunchStatusUpdate with matching LaunchState/LaunchOrigin types
- Plan 03 (REST endpoints + DB) can use LaunchStateMachine.get_active(), .dismiss(), .transition() from AppState
- Plan 04 (kiosk TypeScript) has exact JSON string values from Phase 62 value-contract tests

## Self-Check: PASSED

- `crates/racecontrol/src/launch_state.rs` — FOUND
- `crates/racecontrol/tests/launch_state_machine.rs` — FOUND
- `crates/racecontrol/tests/launch_id_threading.rs` — FOUND
- `crates/rc-common/tests/launch_status_value_contract.rs` — FOUND
- `crates/rc-common/tests/launch_status_serde.rs` — FOUND
- commit `367b949c` — FOUND
- commit `0b334d67` — FOUND
- commit `71618d7e` — FOUND

---
*Phase: 368-live-launch-status-with-autonomous-debug*
*Completed: 2026-04-11*
