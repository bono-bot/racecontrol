---
phase: 71-rc-common-foundation-rc-sentry-core-hardening
plan: 02
subsystem: infra
tags: [rust, rc-sentry, tracing, atomics, tcp, exec, hardening]

# Dependency graph
requires:
  - 71-01 (rc-common exec module with run_cmd_sync)
provides:
  - rc-sentry hardened with timeout enforcement via rc_common::exec::run_cmd_sync
  - SlotGuard concurrency cap (AtomicUsize + Drop) -- max 4 concurrent execs, HTTP 429 on overflow
  - Content-Length-aware read_request() -- fixes partial TCP body read bug
  - Structured tracing logging (replaces all eprintln!)
  - Named handler threads (sentry-handler-{N} via thread::Builder)
affects: [72, 74]

# Tech tracking
tech-stack:
  added:
    - tracing = { workspace = true } in rc-sentry Cargo.toml
    - tracing-subscriber = { workspace = true } in rc-sentry Cargo.toml
  patterns:
    - SlotGuard pattern: AtomicUsize + compare_exchange + Drop impl -- lock-free concurrency cap without panics
    - Content-Length-aware read loop: parse header, loop stream.read() until body_received >= content_length
    - Named thread spawning: thread::Builder::new().name(format!("sentry-handler-{}", n)).spawn(...)
    - tracing_subscriber::fmt::init() as first line in main() -- respects RUST_LOG, plain stdout

key-files:
  modified:
    - crates/rc-sentry/src/main.rs
    - crates/rc-sentry/Cargo.toml

key-decisions:
  - "SlotGuard Drop impl ensures EXEC_SLOTS decremented even on early return or panic -- prevents 429 lockout"
  - "read_request() parses Content-Length header with .trim() to handle CRLF line endings correctly"
  - "THREAD_COUNTER AtomicUsize provides monotonic thread IDs for sentry-handler-{N} naming"
  - "No tokio added to rc-sentry -- all five hardening fixes use stdlib + rc-common + tracing only"

patterns-established:
  - "SlotGuard pattern: acquire() with compare_exchange loop + Drop decrement -- zero-overhead concurrency cap"
  - "Content-Length read loop: header parse -> body_received calc -> loop until remaining == 0"

requirements-completed: [SHARD-01, SHARD-02, SHARD-03, SHARD-04, SHARD-05]

# Metrics
duration: 5min
completed: 2026-03-20
---

# Phase 71 Plan 02: rc-sentry Core Hardening Summary

**Fully hardened rc-sentry: timeout via rc_common::exec::run_cmd_sync, 64KB output truncation, concurrency cap at 4 with HTTP 429, Content-Length TCP read loop, and tracing structured logging replacing all eprintln!**

## Performance

- **Duration:** ~5 min
- **Started:** 2026-03-20T12:14:16Z
- **Completed:** 2026-03-20T12:14:59Z
- **Tasks:** 1
- **Files modified:** 2

## Accomplishments

- Replaced `Command::new("cmd.exe").output()` (no timeout, blocks forever) with `rc_common::exec::run_cmd_sync(cmd, Duration::from_millis(timeout_ms), MAX_BODY)` -- SHARD-01, SHARD-02
- Added `SlotGuard` struct with `AtomicUsize` + `compare_exchange` + `Drop` impl -- caps concurrent execs at 4, rejects 5th with HTTP 429 `{"error":"too many concurrent requests"}` -- SHARD-03
- Added `read_request()` function with Content-Length-aware loop -- fixes partial TCP body read on large POSTs -- SHARD-04
- Replaced all `eprintln!` with `tracing::info!`, `tracing::warn!`, `tracing::error!` -- added `tracing_subscriber::fmt::init()` in main() -- SHARD-05
- Added `tracing = { workspace = true }` and `tracing-subscriber = { workspace = true }` to rc-sentry Cargo.toml
- Spawned handler threads with `thread::Builder::new().name(format!("sentry-handler-{}", n))` for debuggability
- Added `429 => "Too Many Requests"` match arm to `send_response()`
- Response JSON includes `timed_out` and `truncated` fields from `ExecResult`
- Verified: `cargo build --bin rc-sentry` exits 0, `cargo tree -p rc-sentry | grep tokio` outputs 0 (no tokio), all 5 rc-common exec tests pass

## Task Commits

Each task was committed atomically:

1. **Task 1: All five hardening fixes in rc-sentry** - `b8a43b9` (feat)

## Files Created/Modified

- `crates/rc-sentry/src/main.rs` - Complete rewrite: SlotGuard, read_request(), handle_exec wired to run_cmd_sync, tracing logging, named threads
- `crates/rc-sentry/Cargo.toml` - Added tracing and tracing-subscriber workspace deps

## Decisions Made

- Used `THREAD_COUNTER: AtomicUsize` (separate from `EXEC_SLOTS`) for monotonic thread IDs -- EXEC_SLOTS tracks live connections, THREAD_COUNTER tracks total spawned
- `read_request()` uses `.trim()` on Content-Length value to handle CRLF line endings (pitfall 4 from RESEARCH.md avoided)
- Truncation happens inside `rc_common::exec::run_cmd_sync` via `truncate_output(Vec<u8>)` -- before UTF-8 conversion, no char boundary panics

## Deviations from Plan

None - plan executed exactly as written. The rc-sentry/src/main.rs already had the correct structure from the Plan 01 wire-up step; Plan 02 completed the full hardening by committing the complete rewrite with all five SHARD fixes applied.

## Issues Encountered

None. Files were already in the correct hardened state (modified but not committed from Plan 01 execution). Build passed immediately. All 5 exec tests green.

## User Setup Required

None.

## Next Phase Readiness

- All 5 SHARD requirements complete (SHARD-01..05) plus SHARED-01..03 from Phase 71 Plan 01
- Phase 71 is complete -- ready for Phase 72 (rc-sentry Endpoint Expansion + Integration Tests)
- rc-sentry now correctly enforces timeout, truncation, concurrency cap, full TCP reads, and structured logging
- `cargo tree -p rc-sentry` shows zero tokio references -- stdlib isolation maintained

---
*Phase: 71-rc-common-foundation-rc-sentry-core-hardening*
*Completed: 2026-03-20*
