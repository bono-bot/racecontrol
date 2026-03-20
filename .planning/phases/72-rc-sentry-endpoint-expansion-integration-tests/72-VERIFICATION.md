---
phase: 72-rc-sentry-endpoint-expansion-integration-tests
verified: 2026-03-20T13:15:00+05:30
status: passed
score: 10/10 must-haves verified
re_verification: false
human_verification:
  - test: "Ctrl+C graceful shutdown"
    expected: "rc-sentry.exe 9191 started, press Ctrl+C, logs show 'shutdown requested -- draining N active connections' then 'rc-sentry shutdown complete', process exits cleanly with code 0"
    why_human: "OS signal (CTRL_C_EVENT) cannot be injected programmatically from cargo test; SetConsoleCtrlHandler and AtomicBool path is code-verified but the end-to-end OS interaction requires a manual run"
---

# Phase 72: rc-sentry Endpoint Expansion and Integration Tests — Verification Report

**Phase Goal:** rc-sentry becomes a complete fallback operations tool with process visibility, file inspection, and health confirmation -- all endpoints covered by integration tests running against an ephemeral port
**Verified:** 2026-03-20T13:15:00 IST
**Status:** passed
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|---------|
| 1 | GET /health returns JSON with status, version, build_id, uptime_secs, exec_slots_available, exec_slots_total, hostname | VERIFIED | `handle_health` in main.rs lines 246-259; all 7 fields present in serde_json::json! call; `test_health_fields` asserts each field and passes |
| 2 | GET /version returns JSON with version and git_hash fields | VERIFIED | `handle_version` in main.rs lines 261-264; `test_version_fields` asserts both fields and passes |
| 3 | GET /files?path=C%3A%5C returns JSON array of directory entries with name, is_dir, size, modified | VERIFIED | `handle_files` in main.rs lines 266-326; `test_files_directory` asserts name and is_dir fields, HTTP 200, no 500 — passes |
| 4 | GET /processes returns JSON array with pid, name, memory_kb for running processes | VERIFIED | `handle_processes` in main.rs lines 328-343; sysinfo::System::new_all() + refresh_all(); `test_processes_fields` asserts all 3 fields and passes |
| 5 | Ctrl+C sets SHUTDOWN_REQUESTED, accept loop exits, active connections drain, process exits cleanly | VERIFIED (code) / NEEDS HUMAN (runtime) | `ctrl_handler` (lines 58-67) stores `true` on CTRL_C_EVENT (0) or CTRL_CLOSE_EVENT (2); main loop checks `SHUTDOWN_REQUESTED.load` and breaks; `for h in handles { let _ = h.join(); }` drains; `SetConsoleCtrlHandler` registered at line 91 — all code paths present. OS signal injection not testable via cargo test. |
| 6 | rc-sentry still has zero tokio in cargo tree after all additions | VERIFIED | `cargo tree -p rc-sentry \| grep -i tokio` returns nothing (exit code 1 = no matches) |
| 7 | cargo test -p rc-sentry runs 7 integration tests and all pass | VERIFIED | Live run: `test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.30s` |
| 8 | Each test gets its own ephemeral port (no port conflicts in parallel execution) | VERIFIED | `start_test_server` binds `"127.0.0.1:0"` — OS assigns port; `listener.local_addr().unwrap().port()` retrieves it; all 7 tests call `start_test_server(1)` independently |
| 9 | Tests cover all 6 endpoints: /ping, /exec, /health, /version, /files, /processes plus 404 | VERIFIED | test_ping, test_exec_echo, test_health_fields, test_version_fields, test_files_directory, test_processes_fields, test_404_unknown_path — all 7 present in main.rs lines 446-514 |
| 10 | Test server threads exit cleanly via incoming().take(N) | VERIFIED | `listener.incoming().take(requests).flatten()` at line 414; `start_test_server(1)` passes 1 — each test thread handles exactly 1 request then exits cleanly |

**Score:** 10/10 truths verified (1 truth has a runtime-only component requiring human confirmation)

---

### Required Artifacts

#### Plan 72-01 Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/rc-sentry/build.rs` | GIT_HASH embedded at build time via cargo:rustc-env | VERIFIED | File exists, 18 lines; contains `cargo:rustc-env=GIT_HASH={hash}` (line 13), `rerun-if-changed=.git/HEAD` (line 16), `rerun-if-changed=.git/refs/heads` (line 17) |
| `crates/rc-sentry/Cargo.toml` | sysinfo and winapi dependencies | VERIFIED | `sysinfo = "0.33"` at line 19; `winapi = { version = "0.3", features = ["consoleapi"] }` at line 22; both sections present |
| `crates/rc-sentry/src/main.rs` | 6 endpoint handlers + graceful shutdown | VERIFIED | All 4 new handlers present (handle_health L246, handle_version L261, handle_files L266, handle_processes L328); graceful shutdown via ctrl_handler L58, SHUTDOWN_REQUESTED L28, set_nonblocking L87, handles.retain L98 |

#### Plan 72-02 Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/rc-sentry/src/main.rs` | Inline #[cfg(test)] module with 7 integration tests | VERIFIED | `#[cfg(test)]` at line 405, `mod tests` at line 406, `start_test_server` L409, `http_get` L422, `http_post` L432, all 7 test functions L446-514 |

---

### Key Link Verification

#### Plan 72-01 Key Links

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `crates/rc-sentry/src/main.rs` | `crates/rc-sentry/build.rs` | `env!("GIT_HASH")` reads build.rs rustc-env output | WIRED | `const BUILD_ID: &str = env!("GIT_HASH");` at line 23; build.rs emits `cargo:rustc-env=GIT_HASH` at line 13 |
| `crates/rc-sentry/src/main.rs` | sysinfo crate | `System::new_all()` for /processes and `System::host_name()` for /health | WIRED | `sysinfo::System::host_name()` at line 256 in handle_health; `sysinfo::System::new_all()` at line 329 in handle_processes; `sys.refresh_all()` at line 330 |
| `crates/rc-sentry/src/main.rs` | winapi crate | `SetConsoleCtrlHandler` for Ctrl+C graceful shutdown | WIRED | `winapi::um::consoleapi::SetConsoleCtrlHandler(Some(ctrl_handler), 1)` at line 91; `#[cfg(windows)]` guard correctly applied |

#### Plan 72-02 Key Links

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `#[cfg(test)] mod tests` | `handle()` function | `start_test_server` spawns thread calling `handle()` via `incoming().take(N)` | WIRED | `for stream in listener.incoming().take(requests).flatten() { let _ = handle(stream); }` at lines 414-416 |
| `#[cfg(test)] mod tests` | `TcpStream on 127.0.0.1:port` | `http_get`/`http_post` helpers send raw HTTP requests | WIRED | `std::net::TcpStream::connect(format!("127.0.0.1:{port}"))` at lines 424 and 435 |

---

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|---------|
| SEXP-01 | 72-01 | rc-sentry exposes GET /health returning uptime, version, concurrent exec slots, hostname | SATISFIED | `handle_health` returns all 7 required fields; `test_health_fields` asserts each; marked [x] in REQUIREMENTS.md |
| SEXP-02 | 72-01 | rc-sentry exposes GET /version returning binary version and git commit hash | SATISFIED | `handle_version` returns `version` and `git_hash`; BUILD_ID via `env!("GIT_HASH")` from build.rs; `test_version_fields` passes |
| SEXP-03 | 72-01 | rc-sentry exposes GET /files?path=... returning directory listing or file contents | SATISFIED | `handle_files` parses query param, percent-decodes path, reads dir with name/is_dir/size/modified fields; `test_files_directory` passes against C:\ |
| SEXP-04 | 72-01 | rc-sentry exposes GET /processes returning list of running processes with PID, name, memory | SATISFIED | `handle_processes` uses sysinfo to collect pid/name/memory_kb; `test_processes_fields` passes |
| SHARD-06 | 72-01 | rc-sentry handles graceful shutdown on SIGTERM/Ctrl+C (drains active connections) | SATISFIED (code) | `ctrl_handler` sets `SHUTDOWN_REQUESTED`; main loop drains handles before exit; `set_nonblocking(true)` enables polling; runtime confirmation needs human test |
| TEST-04 | 72-02 | rc-sentry endpoint integration tests (/ping, /exec, /health, /version, /files, /processes) | SATISFIED | All 7 tests pass live: `test result: ok. 7 passed; 0 failed` in 0.30s; all 6 endpoints + 404 covered |

**Orphaned requirements check:** REQUIREMENTS.md traceability table maps SHARD-06, SEXP-01, SEXP-02, SEXP-03, SEXP-04, TEST-04 to Phase 72. All 6 appear in plan frontmatter (SHARD-06, SEXP-01–04 in 72-01; TEST-04 in 72-02). No orphaned requirements.

---

### Anti-Patterns Found

Scanned `crates/rc-sentry/src/main.rs` (516 lines), `crates/rc-sentry/build.rs` (18 lines), `crates/rc-sentry/Cargo.toml` (26 lines).

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| main.rs | 229 | `rc_common::exec::run_cmd_sync` used in `handle_exec` | Info | Expected — rc-common dependency is correct design; not an anti-pattern |

No TODO/FIXME/placeholder comments found. No `return null` / `return {}` stubs. No empty handler bodies. No console.log-only implementations. No tokio in dependency tree.

---

### Human Verification Required

#### 1. Ctrl+C Graceful Shutdown (SHARD-06 runtime path)

**Test:** Build release binary with `cargo build --release --bin rc-sentry`. Run `.\target\release\rc-sentry.exe 9191`. Wait for "listening on :9191" log. Send a long-running exec request (e.g., `{"cmd":"timeout /t 10","timeout_ms":15000}`), then immediately press Ctrl+C.
**Expected:** Log shows `shutdown requested -- draining 1 active connections`. The in-flight exec request completes (or times out) before the log shows `rc-sentry shutdown complete`. Process exits with code 0 and no hung threads.
**Why human:** `SetConsoleCtrlHandler` fires on OS-level console events. Cargo test runs in a subprocess with a different console context; CTRL_C_EVENT cannot be delivered programmatically to the handler without OS-level signal injection which is not available in the test harness.

---

### Gaps Summary

No gaps found. All automated checks pass. The only item requiring human attention is the runtime confirmation of Ctrl+C graceful shutdown — the code path is fully implemented and verified at the source level, but the OS signal interaction cannot be exercised by cargo test.

---

## Summary

Phase 72 achieved its goal. rc-sentry is a complete fallback operations tool:

- **6 endpoints** routed in `handle()`: /ping, /exec, /health, /version, /files, /processes
- **Graceful shutdown** via AtomicBool + SetConsoleCtrlHandler + non-blocking accept loop with connection draining
- **GIT_HASH** embedded at build time via build.rs (copied from rc-agent pattern)
- **sysinfo 0.33** for process enumeration and hostname; **winapi 0.3** for console control handler
- **Zero tokio contamination** confirmed via cargo tree
- **7 integration tests** covering all endpoints + 404, each on its own ephemeral port, all passing in 0.30s
- All 6 requirements (SEXP-01, SEXP-02, SEXP-03, SEXP-04, SHARD-06, TEST-04) satisfied and marked [x] in REQUIREMENTS.md

---

_Verified: 2026-03-20T13:15:00 IST_
_Verifier: Claude (gsd-verifier)_
