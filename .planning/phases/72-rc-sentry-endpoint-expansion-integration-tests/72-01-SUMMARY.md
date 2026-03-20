---
phase: 72-rc-sentry-endpoint-expansion-integration-tests
plan: "01"
subsystem: rc-sentry
tags: [rc-sentry, endpoints, graceful-shutdown, sysinfo, winapi, build-rs]
dependency_graph:
  requires: [71-02]
  provides: [SEXP-01, SEXP-02, SEXP-03, SEXP-04, SHARD-06]
  affects: [rc-sentry binary, fleet health endpoint consumers]
tech_stack:
  added: [sysinfo 0.33, winapi 0.3]
  patterns: [non-blocking accept loop, AtomicBool shutdown, OnceLock uptime, SetConsoleCtrlHandler, build.rs GIT_HASH embedding]
key_files:
  created: [crates/rc-sentry/build.rs]
  modified: [crates/rc-sentry/Cargo.toml, crates/rc-sentry/src/main.rs]
decisions:
  - build.rs copied verbatim from rc-agent (same git hash pattern, same rerun triggers)
  - winapi 0.3 consoleapi feature only -- minimal surface for SetConsoleCtrlHandler
  - Non-blocking accept loop with 10ms sleep on WouldBlock -- avoids busy loop, enables shutdown polling
  - SHUTDOWN_REQUESTED checked after handles.retain() -- existing connections drain before exit
  - 403 Forbidden added to send_response reason match (minor correctness fix)
metrics:
  duration_min: 15
  completed_date: "2026-03-20T12:57:59Z"
  tasks_completed: 2
  tasks_total: 2
  files_changed: 3
---

# Phase 72 Plan 01: rc-sentry Endpoint Expansion Summary

**One-liner:** rc-sentry gains /health, /version, /files, /processes endpoints + Ctrl+C graceful shutdown via AtomicBool + SetConsoleCtrlHandler, zero tokio contamination maintained.

## What Was Built

rc-sentry transformed from a 2-endpoint exec tool (/ping + /exec) into a complete fallback operations tool with 6 endpoints. Graceful shutdown allows active connections to drain before process exit.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Add build.rs and Cargo.toml dependencies | 0d0baf6 | crates/rc-sentry/build.rs (created), crates/rc-sentry/Cargo.toml |
| 2 | Add 4 new endpoints and graceful shutdown | 185eb7d | crates/rc-sentry/src/main.rs |

## Endpoints Added

| Endpoint | Response | Uses |
|----------|----------|------|
| GET /health | `{status, version, build_id, uptime_secs, exec_slots_available, exec_slots_total, hostname}` | sysinfo::System::host_name(), OnceLock<Instant> |
| GET /version | `{version, git_hash}` | env!("CARGO_PKG_VERSION"), env!("GIT_HASH") |
| GET /files?path=... | `[{name, is_dir, size, modified}]` | std::fs::read_dir, SystemTime |
| GET /processes | `[{pid, name, memory_kb}]` | sysinfo::System::new_all() |

## Graceful Shutdown

- `SHUTDOWN_REQUESTED: AtomicBool` set by `ctrl_handler` on CTRL_C_EVENT (0) or CTRL_CLOSE_EVENT (2)
- `SetConsoleCtrlHandler` registered immediately after `listener.set_nonblocking(true)`
- Accept loop polls `SHUTDOWN_REQUESTED` every iteration; on true, breaks and joins all active handler threads
- WouldBlock error sleeps 10ms to avoid busy loop

## Verification Results

- `cargo build --bin rc-sentry` exits 0
- `cargo tree -p rc-sentry | grep tokio` outputs nothing (zero tokio contamination)
- `crates/rc-sentry/build.rs` contains `cargo:rustc-env=GIT_HASH`
- All 4 handler functions present in main.rs
- `SHUTDOWN_REQUESTED`, `SetConsoleCtrlHandler`, `set_nonblocking`, `handles.retain` all confirmed

## Deviations from Plan

None — plan executed exactly as written.

## Self-Check: PASSED

- `crates/rc-sentry/build.rs` exists: FOUND
- `crates/rc-sentry/Cargo.toml` contains sysinfo: FOUND
- `crates/rc-sentry/src/main.rs` contains all 4 handlers: FOUND
- Commits 0d0baf6 and 185eb7d: FOUND
- cargo build exits 0: PASS
- No tokio in cargo tree: PASS
