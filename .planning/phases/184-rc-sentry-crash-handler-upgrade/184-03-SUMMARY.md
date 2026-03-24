---
phase: 184-rc-sentry-crash-handler-upgrade
plan: "03"
subsystem: rc-sentry
tags: [session1-spawn, winapi, windows, crash-handler, GUI-process, SPAWN-03]
one_liner: "Session 1 spawn (WTSQueryUserToken + CreateProcessAsUser) added to rc-sentry as primary restart path, with schtasks as fallback — GUI process launches now bridge SYSTEM Session 0 to interactive Session 1"

dependency_graph:
  requires: [184-01, 184-02]
  provides: [SPAWN-03]
  affects: [tier1_fixes.restart_service, rc-sentry binary]

tech_stack:
  added:
    - winapi 0.3 features: wtsapi32, errhandlingapi, handleapi, processthreadsapi, securitybaseapi, userenv, winbase, winnt
    - chrono (workspace dep, resolves pre-existing E0433)
  patterns:
    - Session 0 -> Session 1 bridge via WTSGetActiveConsoleSessionId + WTSQueryUserToken + DuplicateTokenEx + CreateProcessAsUser
    - Result<(), String> error handling (no anyhow — pure std rc-sentry constraint)
    - try-primary-fallback-to-schtasks pattern in restart_service()

key_files:
  created:
    - crates/rc-sentry/src/session1_spawn.rs
  modified:
    - crates/rc-sentry/Cargo.toml
    - crates/rc-sentry/src/main.rs
    - crates/rc-sentry/src/tier1_fixes.rs

decisions:
  - "Used CREATE_UNICODE_ENVIRONMENT | CREATE_NEW_CONSOLE flags (not CREATE_NO_WINDOW) so rc-agent's kiosk window appears on the interactive desktop"
  - "Fallback path preserves the full schtasks /Create + /Run sequence — not just /Run — so the task is guaranteed idempotent at fallback time"
  - "spawn_in_session1 takes bat_path directly (not exe_dir) unlike rc-watchdog — callers already have cfg.start_script from sentry_config"
  - "chrono added as workspace dependency to fix pre-existing E0433 from 184-02 handle_crash refactor"
  - "main.rs CrashHandlerResult tuple destructuring fixed to struct field access (pre-existing E0308 from 184-02)"

metrics:
  duration_seconds: 665
  tasks_completed: 2
  files_modified: 4
  files_created: 1
  completed_date: "2026-03-25"
---

# Phase 184 Plan 03: Session 1 Spawn Path Summary

Session 1 spawn (WTSQueryUserToken + CreateProcessAsUser) added to rc-sentry as primary restart path, with schtasks as fallback — GUI process launches now bridge SYSTEM Session 0 to interactive Session 1.

## Tasks Completed

| Task | Name | Commit | Status |
|------|------|--------|--------|
| 1 | Create session1_spawn.rs module | `1e1ffbb2` | Done |
| 2 | Wire Session 1 spawn into restart_service | `885dfe3d` | Done |
| fix | Resolve pre-existing build errors (184-02) | `503fbe77` | Done |

## What Was Built

### Task 1: session1_spawn.rs

New module `crates/rc-sentry/src/session1_spawn.rs` provides `spawn_in_session1(bat_path: &Path) -> Result<(), String>`:

1. `WTSGetActiveConsoleSessionId()` — get active console session (returns 0xFFFFFFFF if none)
2. `WTSQueryUserToken(session_id, &mut user_token)` — get user token for that session
3. `DuplicateTokenEx(...)` — duplicate as primary token
4. `CreateEnvironmentBlock(...)` — create user environment block (warn on failure, proceed)
5. `CreateProcessAsUserW(...)` with `CREATE_UNICODE_ENVIRONMENT | CREATE_NEW_CONSOLE` flags
6. Returns `Err(String)` if no active session exists — caller falls back to schtasks

No anyhow dependency. No tokio. Pure std.

### Task 2: restart_service() wiring

`tier1_fixes::restart_service()` now:
- **Primary**: calls `crate::session1_spawn::spawn_in_session1(bat_path)`
- **On Ok**: logs "Session 1 spawn succeeded" and proceeds to verification
- **On Err**: logs the reason, falls back to schtasks /Create + /Run via run_cmd_sync
- **Always**: calls `verify_service_started()` to poll :8090/health and confirm rc-agent started

`std::process::Command` is NOT used for interactive process launches — only in `fix_kill_zombies()` for taskkill (which is a background process, not a GUI process).

### Pre-existing Build Errors Fixed (Rule 1 — Auto-fix)

Two errors from 184-02 plan blocked release build:
- `E0433: chrono unresolved` — `handle_crash` in tier1_fixes used `chrono::Utc::now()` but Cargo.toml lacked the dep
- `E0308: mismatched types` — main.rs destructured `handle_crash` return as tuple `(Vec, bool)` but 184-02 changed it to `CrashHandlerResult` struct

Fixed: added `chrono = { workspace = true }` to Cargo.toml and updated main.rs to use struct field access.

## Verification Results

1. `cargo test -p rc-sentry` — **58 passed, 0 failed** (after clean rebuild to resolve Windows file lock)
2. `cargo build -p rc-sentry --release` — **clean** (3 pre-existing dead code warnings, not new)
3. `grep -rn "spawn_in_session1" crates/rc-sentry/src/` — referenced in session1_spawn.rs (definition) and tier1_fixes.rs (call site)
4. `grep -rn "WTSQueryUserToken" crates/rc-sentry/src/` — Session 1 API present
5. `grep -rn "std::process::Command" crates/rc-sentry/src/tier1_fixes.rs` — only in fix_kill_zombies (taskkill), NOT in restart_service

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed pre-existing E0433: chrono unresolved in tier1_fixes.rs**
- **Found during:** Task 2 release build verification
- **Issue:** `tier1_fixes.rs` line 708 used `chrono::Utc::now()` in `post_recovery_event()` but `chrono` was not in rc-sentry's Cargo.toml (added by 184-02 plan without the dep)
- **Fix:** Added `chrono = { workspace = true }` to `[dependencies]` in Cargo.toml
- **Files modified:** `crates/rc-sentry/Cargo.toml`
- **Commit:** `503fbe77`

**2. [Rule 1 - Bug] Fixed pre-existing E0308: CrashHandlerResult tuple destructuring in main.rs**
- **Found during:** Task 2 release build verification
- **Issue:** `main.rs` line 235 destructured `handle_crash()` as `let (results, restarted) = ...` but 184-02 changed the return type from `(Vec<CrashDiagResult>, bool)` to `CrashHandlerResult` struct
- **Fix:** Updated main.rs to use struct field access (`result.fix_results`, `result.restarted`)
- **Files modified:** `crates/rc-sentry/src/main.rs`
- **Commit:** `503fbe77`

## Self-Check: PASSED

| Check | Result |
|-------|--------|
| `crates/rc-sentry/src/session1_spawn.rs` exists | FOUND |
| `crates/rc-sentry/src/tier1_fixes.rs` exists | FOUND |
| `184-03-SUMMARY.md` exists | FOUND |
| Commit `1e1ffbb2` (Task 1) | FOUND |
| Commit `885dfe3d` (Task 2) | FOUND |
| Commit `503fbe77` (build fixes) | FOUND |
| 58 tests pass | PASS |
| Release build clean | PASS |
