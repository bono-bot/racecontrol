---
phase: 318-launch-intelligence
plan: "01"
subsystem: api
tags: [rust, websocket, diagnostic, game-launch, tier-engine]

# Dependency graph
requires:
  - phase: 315-shared-types-foundation
    provides: LaunchTimeoutConfig, SimType types in rc-common
  - phase: 317-server-inventory-fleet-intelligence
    provides: check_game_health, agent_senders pattern

provides:
  - CoreToAgentMessage::LaunchTimedOut{sim_type, elapsed_secs} in protocol.rs
  - DiagnosticTrigger::GameLaunchTimeout{elapsed_secs} in diagnostic_engine.rs
  - AgentConfig::launch_timeout: LaunchTimeoutConfig (default 90s) in config_schema.rs
  - LaunchTimedOut handler in ws_handler.rs feeding tier engine
  - LaunchTimedOut emission in game_launcher.rs check_game_health after timeout

affects:
  - 318-02 (dynamic timeout from p95 data)
  - tier_engine (new GameLaunchTimeout trigger path)
  - knowledge_base (new dedup key game_launch_timeout)
  - mma_engine (classify_domain handles GameLaunchTimeout)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Snapshot agent sender before .await to avoid lock across await"
    - "emit_external_event pattern for external WS-triggered diagnostic events"
    - "TDD RED/GREEN cycle for protocol types + config fields"

key-files:
  created: []
  modified:
    - crates/rc-common/src/protocol.rs
    - crates/rc-agent/src/diagnostic_engine.rs
    - crates/rc-common/src/config_schema.rs
    - crates/rc-agent/src/ws_handler.rs
    - crates/rc-agent/src/cognitive_gate.rs
    - crates/rc-agent/src/tier_engine.rs
    - crates/rc-agent/src/knowledge_base.rs
    - crates/rc-agent/src/mma_engine.rs
    - crates/racecontrol/src/game_launcher.rs
    - crates/racecontrol/src/fleet_health.rs
    - crates/rc-agent/src/config.rs
    - crates/rc-agent/src/content_scanner.rs

key-decisions:
  - "GameLaunchTimeout uses same domain/domain classification as GameLaunchFail (rust_backend)"
  - "No lock held across .await in sender snapshot — senders.get().cloned() before tx.send().await"
  - "LaunchTimedOut send is fire-and-forget (no receiver in no-agent state is acceptable)"
  - "GameLaunchTimeout dedup key is stable static string (not including elapsed_secs)"

patterns-established:
  - "Phase 318 sender snapshot pattern: let sender_opt = { lock.read().await; .cloned() }; drop before .await"

requirements-completed: [LAUNCH-01]

# Metrics
duration: 20min
completed: 2026-04-03
---

# Phase 318 Plan 01: Launch Intelligence Summary

**LaunchTimedOut WS message from server to agent, GameLaunchTimeout DiagnosticTrigger feeding tier engine via emit_external_event, and AgentConfig.launch_timeout field (default 90s) pushed server->agent via FullConfigPush**

## Performance

- **Duration:** 20 min
- **Started:** 2026-04-03T07:21:47Z
- **Completed:** 2026-04-03T13:12:25+05:30
- **Tasks:** 2
- **Files modified:** 12

## Accomplishments
- Added `CoreToAgentMessage::LaunchTimedOut{sim_type, elapsed_secs}` — server can now notify agents of launch timeouts
- Added `DiagnosticTrigger::GameLaunchTimeout{elapsed_secs}` and wired it into all exhaustive match arms (tier_engine, knowledge_base, mma_engine, cognitive_gate)
- Added `AgentConfig::launch_timeout: LaunchTimeoutConfig` with `#[serde(default)]` for backward compatibility
- Server `check_game_health` now sends `LaunchTimedOut` to agent after timeout fires, using lock-snapshot-before-await pattern
- Agent `ws_handler` handles `LaunchTimedOut` by calling `emit_external_event` with `GameLaunchTimeout`
- 6 tests pass (3 rc-common round-trip/default, 1 racecontrol check_game_health, 2 cognitive_gate)

## Task Commits

Each task was committed atomically:

1. **Task 1: Add LaunchTimedOut + GameLaunchTimeout + launch_timeout_config** - `cc9236c1` (feat)
2. **Task 2: Wire LaunchTimedOut into ws_handler + emit from check_game_health** - `97083b5a` (feat)

## Files Created/Modified
- `crates/rc-common/src/protocol.rs` - Added `LaunchTimedOut` CoreToAgentMessage variant + test
- `crates/rc-agent/src/diagnostic_engine.rs` - Added `GameLaunchTimeout` DiagnosticTrigger variant
- `crates/rc-common/src/config_schema.rs` - Added `launch_timeout: LaunchTimeoutConfig` to AgentConfig + tests
- `crates/rc-agent/src/ws_handler.rs` - Handle `LaunchTimedOut` → `emit_external_event(GameLaunchTimeout)`
- `crates/rc-agent/src/cognitive_gate.rs` - `trigger_to_problem` + domain classification for `GameLaunchTimeout`
- `crates/rc-agent/src/tier_engine.rs` - `check_trigger_resolved`, `make_dedup_key`, Tier 1 action match arms
- `crates/rc-agent/src/knowledge_base.rs` - `normalize_problem_key` for `GameLaunchTimeout`
- `crates/rc-agent/src/mma_engine.rs` - `classify_domain` for `GameLaunchTimeout`
- `crates/racecontrol/src/game_launcher.rs` - Emit `LaunchTimedOut` in `check_game_health` timeout loop + test
- `crates/racecontrol/src/fleet_health.rs` - Bug fix: missing `windows_session_id` in None branch
- `crates/rc-agent/src/config.rs` - Bug fix: missing `launch_timeout` field in test AgentConfig init
- `crates/rc-agent/src/content_scanner.rs` - Bug fix: missing `fleet_validity` field in test struct init

## Decisions Made
- `GameLaunchTimeout` classified as `rust_backend` domain (same as `GameLaunchFail`) — both are game launch failures
- No lock held across `.await` in sender snapshot: `senders.get(&pod_id).cloned()` before `tx.send().await`
- `LaunchTimedOut` send is fire-and-forget (error logged at WARN if channel closed — pod is offline)
- `GameLaunchTimeout` dedup key is `"GameLaunchTimeout"` (stable, not including volatile elapsed_secs)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Missing `windows_session_id` field in PodFleetStatus None branch**
- **Found during:** Task 2 (attempting to run racecontrol tests)
- **Issue:** `fleet_health.rs` line 1119 struct init missing `windows_session_id` — compile error blocking all racecontrol tests
- **Fix:** Added `windows_session_id: None` to the None-branch `PodFleetStatus` initializer
- **Files modified:** `crates/racecontrol/src/fleet_health.rs`
- **Verification:** `cargo check --bin racecontrol` passes, tests run
- **Committed in:** `97083b5a` (Task 2 commit)

**2. [Rule 1 - Bug] Missing `launch_timeout` field in test `AgentConfig` initializer in config.rs**
- **Found during:** Task 2 (after adding `launch_timeout` field to AgentConfig struct)
- **Issue:** `crates/rc-agent/src/config.rs` test helper `valid_config()` had exhaustive struct init missing the new field
- **Fix:** Added `launch_timeout: rc_common::types::LaunchTimeoutConfig::default()`
- **Files modified:** `crates/rc-agent/src/config.rs`
- **Verification:** `cargo check --bin rc-agent` passes
- **Committed in:** `97083b5a` (Task 2 commit)

**3. [Rule 1 - Bug] Missing `fleet_validity` field in test `GamePresetWithReliability` initializer in content_scanner.rs**
- **Found during:** Task 2 (running rc-agent tests)
- **Issue:** `crates/rc-agent/src/content_scanner.rs` test helper had exhaustive struct init missing `fleet_validity`
- **Fix:** Added `fleet_validity: "unknown".to_string()`
- **Files modified:** `crates/rc-agent/src/content_scanner.rs`
- **Verification:** `cargo test -p rc-agent-crate cognitive_gate` passes
- **Committed in:** `97083b5a` (Task 2 commit)

---

**Total deviations:** 3 auto-fixed (all Rule 1 - pre-existing bugs in struct initializers unmasked by new struct field addition)
**Impact on plan:** All auto-fixes were cascade effects of adding the `launch_timeout` field to `AgentConfig`. No scope creep.

## Issues Encountered
None beyond the three auto-fixed struct initializer bugs above.

## Next Phase Readiness
- Phase 318-02 can now use `AgentConfig::launch_timeout` for dynamic per-combo timeout tuning
- The `LaunchTimedOut` → `GameLaunchTimeout` → tier engine path is complete and tested
- `DiagnosticTrigger::GameLaunchTimeout` is wired into all exhaustive match arms — no further cascade needed

---
*Phase: 318-launch-intelligence*
*Completed: 2026-04-03*

## Self-Check: PASSED

Verified files exist and commits are present:
- `crates/rc-common/src/protocol.rs` ✓ (LaunchTimedOut variant at line 930)
- `crates/rc-agent/src/diagnostic_engine.rs` ✓ (GameLaunchTimeout at line 112)
- `crates/rc-common/src/config_schema.rs` ✓ (launch_timeout field at line 378)
- `crates/rc-agent/src/ws_handler.rs` ✓ (handler at line 1839)
- Commit cc9236c1 ✓
- Commit 97083b5a ✓
