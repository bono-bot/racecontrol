---
phase: 71-rc-common-foundation-rc-sentry-core-hardening
verified: 2026-03-20T12:45:00+05:30
status: passed
score: 10/10 must-haves verified
re_verification: false
gaps: []
human_verification: []
---

# Phase 71: rc-common Foundation + rc-sentry Core Hardening Verification Report

**Phase Goal:** rc-sentry's three live correctness failures are fixed and rc-common gains the feature-gated exec primitive that both callers will share -- with the tokio contamination boundary verified before any code migrates
**Verified:** 2026-03-20T12:45:00 IST
**Status:** passed
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | run_cmd_sync executes a shell command and returns ExecResult with stdout, stderr, exit_code | VERIFIED | `exec.rs:38` — `pub fn run_cmd_sync(cmd: &str, timeout: Duration, max_output: usize) -> ExecResult`; test_run_cmd_sync_basic passes |
| 2 | run_cmd_sync kills child process and returns timed_out=true after timeout duration | VERIFIED | `exec.rs:70-93` — Ok(None) branch: `child.kill()` then `child.wait()` then sets `timed_out: true`; test_run_cmd_sync_timeout passes |
| 3 | run_cmd_sync truncates output to max_output bytes and sets truncated=true | VERIFIED | `exec.rs:125-144` — `truncate_output()` operates on `Vec<u8>` before UTF-8 conversion; test_output_truncation and test_truncate_output_fn pass |
| 4 | run_cmd_async compiles only when tokio feature is enabled | VERIFIED | `exec.rs:150` — `#[cfg(feature = "tokio")] pub async fn run_cmd_async` guards the function |
| 5 | cargo tree -p rc-sentry shows zero tokio references | VERIFIED | `cargo tree -p rc-sentry \| grep -c tokio` outputs 0; rc-sentry Cargo.toml has no `features = ["tokio"]` on rc-common dep |
| 6 | Long-running command sent to rc-sentry is killed after timeout_ms and returns timed_out:true | VERIFIED | `main.rs:178-182` — `rc_common::exec::run_cmd_sync(cmd, Duration::from_millis(timeout_ms), MAX_BODY)` delegates timeout enforcement |
| 7 | Command output exceeding 64KB is truncated before response is sent | VERIFIED | `main.rs:184-190` — response JSON includes `"timed_out": result.timed_out, "truncated": result.truncated`; truncation enforced inside run_cmd_sync with MAX_BODY=65536 |
| 8 | 5th concurrent exec request is rejected with HTTP 429 Too Many Requests | VERIFIED | `main.rs:25-51` — `SlotGuard` with `AtomicUsize EXEC_SLOTS`, `MAX_EXEC_SLOTS=4`; `main.rs:153-158` — acquire() returns None → `send_response(stream, 429, ...)` |
| 9 | Large POST body is fully received before parsing (partial TCP read fixed) | VERIFIED | `main.rs:85-128` — `read_request()` loops on `stream.read()` until Content-Length bytes accumulated; header parsed for `content-length:` (case-insensitive, trimmed) |
| 10 | rc-sentry startup and handler log lines use tracing format with timestamps and levels | VERIFIED | `main.rs:54` — `tracing_subscriber::fmt::init();` as first line; zero `eprintln!` in file; `tracing::info!`, `tracing::warn!`, `tracing::error!` throughout |

**Score:** 10/10 truths verified

---

### Required Artifacts

#### Plan 71-01 Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/rc-common/src/exec.rs` | ExecResult struct + run_cmd_sync + run_cmd_async + truncate_output + unit tests | VERIFIED | 273 lines; all exports present; 5 tests in `mod tests` |
| `crates/rc-common/Cargo.toml` | wait-timeout dep + optional tokio dep + tokio feature gate | VERIFIED | `wait-timeout = "0.2"`, `[dependencies.tokio] optional = true`, `[features] tokio = ["dep:tokio"]` |
| `crates/rc-common/src/lib.rs` | pub mod exec declaration | VERIFIED | Line 6: `pub mod exec;` |

#### Plan 71-02 Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/rc-sentry/src/main.rs` | Hardened rc-sentry with timeout, truncation, concurrency cap, TCP read fix, tracing | VERIFIED | 253 lines; SlotGuard, read_request(), handle_exec with run_cmd_sync, all tracing macros, no eprintln! |
| `crates/rc-sentry/Cargo.toml` | tracing and tracing-subscriber dependencies | VERIFIED | Both `tracing = { workspace = true }` and `tracing-subscriber = { workspace = true }` present |

---

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `crates/rc-common/src/exec.rs` | wait-timeout crate | `ChildExt::wait_timeout` | WIRED | `exec.rs:12` — `use wait_timeout::ChildExt;`; called at line 60 `child.wait_timeout(timeout)` |
| `crates/rc-common/Cargo.toml` | tokio | optional dependency with feature gate | WIRED | `[dependencies.tokio] workspace = true; optional = true` + `[features] tokio = ["dep:tokio"]` |
| `crates/rc-sentry/src/main.rs` | `rc_common::exec::run_cmd_sync` | handle_exec function | WIRED | `main.rs:178` — `rc_common::exec::run_cmd_sync(cmd, Duration::from_millis(timeout_ms), MAX_BODY)` |
| `crates/rc-sentry/src/main.rs` | `AtomicUsize EXEC_SLOTS` | SlotGuard::acquire in handle_exec | WIRED | `main.rs:153` — `let _guard = match SlotGuard::acquire()` in handle_exec |
| `crates/rc-sentry/src/main.rs` | tracing macros | all log lines | WIRED | `tracing::error!` (lines 64, 80), `tracing::info!` (lines 68, 176), `tracing::warn!` (lines 76, 156) |

---

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| SHARED-01 | 71-01 | rc-common exposes run_cmd_sync (thread + timeout) for rc-sentry and sync contexts | SATISFIED | `exec.rs:38` — public function with wait-timeout; 5/5 unit tests green |
| SHARED-02 | 71-01 | rc-common exposes run_cmd_async (tokio, feature-gated) for rc-agent | SATISFIED | `exec.rs:150-208` — `#[cfg(feature = "tokio")] pub async fn run_cmd_async` |
| SHARED-03 | 71-01 | rc-sentry uses rc-common run_cmd_sync without pulling in tokio (verified via cargo tree) | SATISFIED | `cargo tree -p rc-sentry \| grep -c tokio` = 0; rc-sentry Cargo.toml has no tokio feature on rc-common |
| SHARD-01 | 71-02 | rc-sentry enforces timeout_ms on command execution (kills child process after deadline) | SATISFIED | `main.rs:170,178-182` — timeout_ms parsed from JSON and passed to run_cmd_sync |
| SHARD-02 | 71-02 | rc-sentry truncates command output to 64KB (matching rc-agent remote_ops behavior) | SATISFIED | `main.rs:18` — `const MAX_BODY: usize = 64 * 1024`; passed as max_output to run_cmd_sync |
| SHARD-03 | 71-02 | rc-sentry limits concurrent exec requests to 4 (rejects with HTTP 429 when full) | SATISFIED | `main.rs:19,25,27-51,153-158` — SlotGuard + MAX_EXEC_SLOTS=4 + 429 response |
| SHARD-04 | 71-02 | rc-sentry fixes partial TCP read bug (loops until full HTTP body received) | SATISFIED | `main.rs:85-128` — read_request() with Content-Length-aware loop |
| SHARD-05 | 71-02 | rc-sentry uses structured logging via tracing (replaces eprintln) | SATISFIED | Zero eprintln! in file; tracing_subscriber::fmt::init() + all log calls use tracing macros |

**Orphaned requirements check:** SHARD-06 (graceful shutdown) is mapped to Phase 72 in REQUIREMENTS.md — not orphaned for Phase 71.

---

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| None | — | — | — | — |

No TODO/FIXME/PLACEHOLDER comments found. No empty implementations (`return null`, `return {}`, `=> {}`). No console.log/eprintln stubs. All implementations are substantive.

---

### Human Verification Required

None. All must-haves are verifiable via static analysis and build outputs. The following were verified programmatically:

- `cargo build --bin rc-sentry` exits 0 (Finished dev profile in 0.26s)
- `cargo test -p rc-common -- exec::tests` — 5/5 tests pass (including timeout test with 9s ping killed after 1s)
- `cargo tree -p rc-sentry | grep -c tokio` = 0
- `cargo build -p rc-common` (no features) exits 0

---

### Gaps Summary

No gaps. All 10 observable truths verified. All 5 artifacts exist, are substantive, and are correctly wired. All 8 requirement IDs (SHARED-01/02/03, SHARD-01/02/03/04/05) are satisfied by concrete implementation evidence. The tokio contamination boundary is enforced at the Cargo feature level and confirmed by cargo tree at zero references.

---

_Verified: 2026-03-20T12:45:00 IST_
_Verifier: Claude (gsd-verifier)_
