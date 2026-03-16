# Deferred Items — Phase 08

## Pre-existing Issues (Out-of-scope, discovered during 08-01 execution)

### racecontrol non-exhaustive match in ws/mod.rs

**Found during:** Task 2 (racecontrol test run)
**Issue:** `crates/racecontrol/src/ws/mod.rs:117` has non-exhaustive match on `AgentMessage` — missing arms for `AssistChanged`, `FfbGainChanged`, `AssistState` variants added in rc-common in a previous phase.
**Why deferred:** Pre-existing before Plan 08-01. Not caused by any 08-01 changes. Lock screen changes (lock_screen.rs, main.rs) do not touch racecontrol or the WebSocket handler.
**Impact:** `cargo test -p racecontrol-crate` fails. rc-agent and rc-common tests pass cleanly.
**Suggested fix:** Add match arms in `crates/racecontrol/src/ws/mod.rs` around line 117 for the three new AgentMessage variants (or use `_ => {}` wildcard if unhandled is acceptable).
