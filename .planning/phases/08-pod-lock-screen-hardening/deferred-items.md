# Deferred Items — Phase 08

## Pre-existing Issues (Out-of-scope, discovered during 08-01 execution)

### rc-core non-exhaustive match in ws/mod.rs

**Found during:** Task 2 (rc-core test run)
**Issue:** `crates/rc-core/src/ws/mod.rs:117` has non-exhaustive match on `AgentMessage` — missing arms for `AssistChanged`, `FfbGainChanged`, `AssistState` variants added in rc-common in a previous phase.
**Why deferred:** Pre-existing before Plan 08-01. Not caused by any 08-01 changes. Lock screen changes (lock_screen.rs, main.rs) do not touch rc-core or the WebSocket handler.
**Impact:** `cargo test -p rc-core` fails. rc-agent and rc-common tests pass cleanly.
**Suggested fix:** Add match arms in `crates/rc-core/src/ws/mod.rs` around line 117 for the three new AgentMessage variants (or use `_ => {}` wildcard if unhandled is acceptable).
