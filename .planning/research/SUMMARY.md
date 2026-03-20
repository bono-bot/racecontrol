# Project Research Summary

**Project:** v11.0 Agent and Sentry Hardening
**Domain:** Rust daemon hardening — rc-sentry timeout/concurrency, rc-agent decomposition, rc-common extraction, test infrastructure
**Researched:** 2026-03-20
**Confidence:** HIGH

## Executive Summary

This milestone hardens two production daemons running across an 8-pod sim-racing fleet. rc-sentry (155-line stdlib HTTP server on port 8091) has three silent correctness failures: `timeout_ms` is parsed but never enforced, output is never truncated, and threads are spawned without a concurrency cap. These are not theoretical — a single long-running `dir /s` command will block the sentry thread indefinitely and a concurrent fleet-wide health poll can spawn unbounded OS threads. rc-agent (port 8090) is architecturally sound but its 3,400-line `main.rs` is approaching an unmaintainable mass; its two highest-risk subsystems (billing_guard, failure_monitor) have no unit tests despite controlling revenue and autonomous healing behavior.

The recommended approach is a strict two-phase sequence: harden rc-sentry and write critical tests first, then decompose rc-agent. rc-common gains two shared primitives (exec helper, HTTP utils) that both callers benefit from — but must be feature-gated to preserve rc-sentry's stdlib-only constraint. The key architectural invariant throughout is that rc-sentry must never take a tokio dependency: its value as a fallback tool derives entirely from being independent of rc-agent's failure modes. Any temptation to "just add axum" to simplify timeout logic must be resisted.

The top risk is the rc-agent main.rs decomposition breaking the `select!` event loop, which coordinates 10+ concurrent tasks with shared mutable state across tightly coupled arms. The standing rule applies without exception: characterization tests first, verify green, then refactor one module at a time in compilation order. The recovery path for a bad decomposition (git revert + pendrive deploy) is expensive; the prevention (incremental extraction with tests) is not.

---

## Key Findings

### Recommended Stack

The dependency delta for this milestone is intentionally minimal: 2 new crates total. `wait-timeout 0.2` adds `ChildExt::wait_timeout(Duration)` for rc-sentry's blocking process execution — it calls `WaitForSingleObject` under the hood, requires no async runtime, and is the only correct stdlib-compatible solution for the timeout problem. `mockall 0.13` goes in rc-agent dev-dependencies only, enabling trait-based mocking of the reqwest HTTP client in billing_guard tests without spinning up a real server.

Everything else is already in the workspace: tracing/tracing-subscriber (rc-sentry structured logging), tokio (rc-agent async exec), sysinfo 0.33 (rc-sentry /processes endpoint), serial_test 3, tempfile 3, tower 0.5, and http-body-util 0.1 for test infrastructure. The workspace already pays the compile cost for these — adding them to rc-sentry's Cargo.toml resolves the same locked versions at zero additional cost.

**Core technologies:**
- `wait-timeout 0.2`: child process timeout for rc-sentry — only stdlib-compatible solution on Windows
- `AtomicUsize` (stdlib): concurrency cap for rc-sentry — no external crate needed for a cap-and-reject pattern
- `tracing` + `tracing-subscriber` (workspace): structured logging for rc-sentry — already paid for
- `mockall 0.13` (dev-only): HTTP trait mocking for billing_guard tests — MSRV 1.77, project at 1.93.1
- `sysinfo 0.33` (workspace version): process listing for rc-sentry /processes — same locked version as rc-agent

### Expected Features

**Must have (table stakes — Phase 1):**
- rc-sentry timeout enforcement — `_timeout_ms` is silently ignored; hung threads accumulate
- rc-sentry output truncation — no cap; `dir /s C:\` floods the response buffer
- rc-sentry concurrency limit — unbounded `thread::spawn` per connection; must be co-implemented with timeout (same threading design)
- rc-sentry structured logging — `eprintln!()` only; no timestamps, no levels, no pod context
- rc-sentry /health endpoint — operators need to confirm sentry is alive when rc-agent is down
- rc-sentry /version endpoint — operators need to confirm which binary is deployed; requires build.rs for GIT_HASH
- Unit tests for billing_guard.rs — controls BILL-02/03/SESSION-01; wrong = lost revenue or trapped sessions
- Unit tests for failure_monitor.rs — drives autonomous healing; wrong = undetected game freezes

**Should have (Phase 2):**
- rc-agent config.rs extraction — pure data structs + validation functions; lowest refactor risk, enables clean unit tests
- rc-agent state_machine.rs extraction — LaunchState/CrashRecoveryState enums; depends on config.rs first
- rc-sentry /files endpoint — fallback binary verification when rc-agent is down
- rc-sentry /processes endpoint — fallback process health check when rc-agent is down
- rc-common exec helper — eliminates drift between remote_ops.rs and ws_exec handler
- rc-sentry endpoint tests — TcpStream-based integration tests on ephemeral port

**Defer (v12.0+):**
- FFB safety unit tests — hardware mock design non-trivial; defer unless FFB incidents occur
- rc-agent ws_handler.rs extraction — timing-sensitive WS reconnect loop; only after simpler extractions prove safe
- rc-agent event_loop.rs full extraction — highest regression risk; select! dispatch must not be touched until everything else is proven stable

**Anti-features (do not build):**
- tokio/axum in rc-sentry — doubles binary size, eliminates independent-failure-mode property
- Shared reqwest client in rc-common — pulls tokio into rc-common, contaminates rc-sentry
- Big-bang main.rs rewrite — no regression safety net, behavioral regressions only surface under live conditions
- Full hardware test coverage — ffb, HID, firewall paths are hardware-bound; 100% coverage goal stalls milestone

### Architecture Approach

The workspace has a clean three-tier structure: rc-common (shared types, lib-only), rc-agent (pod daemon, tokio/axum, port 8090), rc-sentry (pod fallback, stdlib, port 8091). v11.0 adds two shared primitives to rc-common — `exec.rs` with sync and async exec paths feature-gated behind `async-exec`, and `http_util.rs` for raw HTTP response formatting. The feature gate is the critical design decision: rc-sentry imports rc-common without `async-exec`, preserving its zero-async-runtime constraint. rc-agent imports rc-common with `features = ["async-exec"]` to get the tokio-based path. This boundary must be established before any code moves into rc-common.

**Major components and changes:**
1. `rc-common/src/exec.rs` (new) — `ExecRequest`, `ExecResult`, `run_cmd_sync` (always compiled), `run_cmd_async` (feature-gated); both callers converge on the same truncation threshold and timeout logic
2. `rc-common/src/http_util.rs` (new) — `json_response()`, `plain_response()`, `cors_ok()` for rc-sentry's raw TCP HTTP responses
3. `rc-sentry/src/main.rs` (hardened) — `OnceLock<Instant>` for start time, `AtomicUsize` for active connections, `recv_timeout` for exec timeout, 4 new endpoints, structured log per request
4. `rc-agent/src/config.rs` (extracted) — all `*Config` structs + `load_config()` + `validate_config()`; pure data, testable in isolation
5. `rc-agent/src/app_state.rs` (extracted) — `AppState`, `LaunchState`, `CrashRecoveryState`; depends on config.rs
6. `rc-agent/src/ws_handler.rs` (extracted) — WS connect/reconnect outer loop; depends on config + app_state
7. `rc-agent/tests/` (new) — billing_guard, failure_monitor tests; use existing watch channel injection pattern

Build order is leaves-to-roots: rc-common first, then rc-sentry (independent of rc-agent), then rc-agent decomposition (highest risk, done last).

### Critical Pitfalls

1. **timeout_ms parsed but never enforced in rc-sentry** — `_` prefix suppresses the warning; a hung `xcopy` blocks the thread indefinitely. Fix: spawn child thread + `channel.recv_timeout(Duration::from_millis(timeout_ms))`. Must NOT use tokio.

2. **rc-sentry thread-per-connection with no concurrency cap** — fleet health poller + deploy script + manual curl = unbounded threads, each consuming ~1MB stack on Windows. Fix: `AtomicUsize` semaphore before spawn, reject with HTTP 429 at configured limit. Must be co-implemented with timeout (same threading change).

3. **rc-agent select! loop decomposition breaking borrow checker** — 10+ select! arms share ~14 mutable local variables (`game_process`, `launch_state`, `crash_recovery`, etc.). Naive function extraction fails E0502/E0505. Fix: extract ALL shared state into `ConnectionState` struct, pass `&mut ConnectionState` to extracted handlers; never use `Arc<Mutex<T>>` for variables that were previously local — introduces lock contention in async hot path.

4. **rc-common extraction contaminating rc-sentry with tokio** — adding `async fn` or `tokio` to rc-common without feature gating causes rc-sentry's linker to pull in the async runtime. Fix: define `[features] async-exec = ["dep:tokio"]` in rc-common BEFORE moving any code; always run `cargo build --bin rc-sentry` after every rc-common change.

5. **Tests triggering real HID/FFB hardware** — `FfbController::new(0x1209, 0xFFB0)` in a test on James's workstation (no wheelbase connected) fails with USB error; on a pod it silently sends a zero-torque command to live hardware. Fix: `FfbBackend` trait with `HidBackend` (production) and `NullBackend` (tests), or `#[cfg(test)]` early-return guards; never call `FfbController` methods directly in tests without this seam.

---

## Implications for Roadmap

Based on combined research, the natural phase structure follows the build-order dependency graph and risk gradient.

### Phase 1: rc-sentry Hardening + Critical Business Tests

**Rationale:** rc-sentry has three live correctness failures (timeout, truncation, unbounded threads) that can manifest at any time during fleet operations. The billing_guard and failure_monitor have no tests despite controlling revenue and autonomous healing — they are the highest business risk in the codebase. Both are achievable with no refactoring risk. Phase 1 should be completable and deployed before any refactoring work begins.

**Delivers:** A hardened fallback daemon with verified behavior under load; test coverage on the two highest-risk subsystems; deployable to Pod 8 for canary verification.

**Addresses:**
- rc-sentry timeout enforcement (P1)
- rc-sentry output truncation (P1)
- rc-sentry concurrency limit (P1, co-implemented with timeout)
- rc-sentry structured logging (P1)
- rc-sentry /health endpoint (P1)
- rc-sentry /version endpoint + build.rs (P1)
- Unit tests for billing_guard.rs (P1)
- Unit tests for failure_monitor.rs (P1)

**Avoids:** Do not add tokio to rc-sentry (destroys independent-failure-mode property). Do not add /files or /processes before timeout + concurrency cap is in place (new endpoints expose longer-running commands). Do not start rc-agent decomposition until Phase 1 is green.

---

### Phase 2: rc-common Extraction + rc-sentry Endpoint Expansion

**Rationale:** Once rc-sentry is hardened and tests are green, rc-common gains the shared exec primitive and HTTP utilities. This phase is safe to do before rc-agent decomposition because rc-common changes compile independently. The new rc-sentry endpoints (/files, /processes) complete the fallback tool's capabilities for incident response.

**Delivers:** Single source of truth for exec timeout/truncation logic; rc-sentry becomes a capable fallback with process and file visibility; rc-common feature gating established for all future shared code.

**Uses:**
- `wait-timeout 0.2` in rc-common (sync exec path)
- `sysinfo 0.33` in rc-sentry (process listing)
- Cargo feature gate `async-exec` in rc-common

**Implements:** rc-common exec.rs + http_util.rs; rc-sentry /files + /processes + endpoint integration tests

**Critical gate:** Run `cargo build --bin rc-sentry` explicitly after every rc-common change. The `cargo test -p rc-common` command does not catch rc-sentry contamination.

---

### Phase 3: rc-agent Decomposition

**Rationale:** Highest regression risk in the milestone. Protected by characterization tests written in this phase before any structural change. Extraction proceeds in strict risk order: config (pure data, no runtime effects) → app_state (pure construction, no channels) → ws_handler (WS lifecycle) → event_loop (select! dispatch, deferred). The panic hook and startup sequence stay in main.rs regardless.

**Delivers:** rc-agent main.rs reduced from ~3,400 lines to ~150 lines; four new testable modules; clean module boundaries for future feature development.

**Uses:**
- `mockall 0.13` in rc-agent dev-dependencies (billing_guard HTTP isolation)
- rc-common exec helper (async path) to remove duplication in remote_ops.rs

**Implements:** config.rs, app_state.rs, ws_handler.rs extraction (event_loop.rs deferred to v12.0)

**Avoids:** No `Arc<Mutex<T>>` for previously-local select! variables. No big-bang rewrite. event_loop.rs extraction explicitly deferred — the select! dispatch body is not touched in this phase.

---

### Phase Ordering Rationale

- Phase 1 before Phase 2: rc-sentry hardening is a prerequisite for safe endpoint expansion. A /files endpoint on an un-rate-limited, no-timeout sentry is a reliability liability.
- Phase 2 before Phase 3: rc-common extraction is independent of rc-agent. Establishing the feature gate boundary and validating `cargo build --bin rc-sentry` clean before any rc-agent code moves is the correct order.
- Phase 3 last: rc-agent decomposition carries the highest regression risk. It must be protected by Phase 1's test infrastructure before it starts. A bad extraction is recovered by `git revert` + pendrive deploy — expensive. Prevention via incremental extraction is not.
- rc-sentry and rc-agent development can proceed in parallel within Phase 3 (they share only rc-common, not each other).

### Research Flags

Phases with standard patterns (research-phase not needed):
- **Phase 1 — rc-sentry hardening:** well-documented stdlib patterns; thread + channel timeout, AtomicUsize semaphore, OnceLock uptime are all established std patterns with no ambiguity.
- **Phase 1 — billing_guard tests:** watch channel injection already designed; mockall automock is well-documented; the test seam exists in the current spawn() signature.
- **Phase 2 — rc-common extraction:** Cargo feature gating for conditional tokio dependency is documented; the exec API design is fully specified in ARCHITECTURE.md.

Phases likely needing targeted research during planning:
- **Phase 3 — select! decomposition:** the EventLoopArgs struct boundary needs careful design before extraction. The 14 mutable shared variables and their ownership across 10+ select! arms need a concrete mapping before a line of code changes. Recommend a planning step that enumerates all captured variables and assigns each to `ConnectionState` (inner loop) or `ReconnectState` (outer loop) before implementation begins.
- **Phase 3 — FFB backend trait:** if FFB safety tests are pulled into this phase, the `FfbBackend` trait design needs upfront agreement. The `#[cfg(test)]` stub approach is simpler but produces fragile tests; the trait approach is cleaner but requires a signature change to `FfbController`. Decide before writing any FFB tests.

---

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | All recommendations verified against crates.io, docs.rs, and direct Cargo.lock inspection. Dependency delta is minimal (2 new crates). No speculative additions. |
| Features | HIGH | Based on direct codebase analysis of all affected files. `_timeout_ms` unused variable confirmed at line 78. No tests for billing_guard/failure_monitor confirmed by grep. Feature dependencies are code-verified. |
| Architecture | HIGH | Build order, module boundaries, channel ownership, and data flow all derived from direct source inspection of 155-line rc-sentry and 3,400-line rc-agent. Feature-gate design for rc-common is technically sound. |
| Pitfalls | HIGH | Every pitfall is grounded in observed code structure. No speculation. Recovery strategies are operationally realistic (pendrive deploy path confirmed in CLAUDE.md). |

**Overall confidence:** HIGH

### Gaps to Address

- **billing_guard HTTP mock seam:** The orphan session end call (`attempt_orphan_end`) currently calls `reqwest::Client::post()` directly without an injectable seam. Before writing billing_guard tests, decide between: (a) wrap reqwest behind a trait + mockall automock, or (b) extract `attempt_orphan_end` as a callback parameter to `billing_guard::spawn()`. Option (b) is simpler and avoids trait boilerplate. This decision should be made during Phase 1 planning, not mid-implementation.

- **failure_monitor try_auto_fix stub approach:** `try_auto_fix` calls the Ollama endpoint. The `#[cfg(test)] mod ai_debugger_stub` approach works but shadows the real module in tests. If `failure_monitor::spawn()` is later refactored to accept a `fix_fn` callback, the test approach improves. Acceptable to start with the cfg(test) stub in Phase 1 and refactor if needed.

- **rc-sentry partial TCP read:** PITFALLS.md documents that the single `stream.read(&mut buf)` call can return partial data for bodies >~1KB. This is a correctness issue distinct from the timeout/truncation work. The fix (~30 lines) should be included in Phase 1 hardening, not deferred. It is not currently listed as a P1 feature in FEATURES.md — roadmapper should flag it.

- **event_loop.rs deferred scope:** The select! dispatch body (lines ~800-2800 in main.rs) is explicitly deferred to v12.0. The roadmap should make this boundary explicit — Phase 3 extractions stop at the ws_handler outer structure and do not enter the select! arms.

---

## Sources

### Primary (HIGH confidence — direct source inspection)
- `crates/rc-sentry/src/main.rs` (full file, 155 lines) — timeout_ms unused at line 78, unbounded thread spawn, no output truncation confirmed
- `crates/rc-agent/src/main.rs` (lines 1-1165) — 3,400 lines confirmed, LaunchState/CrashRecoveryState present, panic hook at lines 396-438
- `crates/rc-agent/src/remote_ops.rs` — truncation at 65,536 bytes, Semaphore 8-slot pattern
- `crates/rc-agent/src/billing_guard.rs` — BILL-02/03/SESSION-01 state machine, no tests present
- `crates/rc-agent/src/failure_monitor.rs` — CRASH-01/02/USB-01 detection, no tests present
- `crates/rc-agent/src/ffb_controller.rs` — VID:0x1209 PID:0xFFB0, no FfbBackend trait, no test seam
- `crates/rc-sentry/Cargo.toml` — deps: serde, serde_json, toml only; no tokio confirmed
- `crates/rc-agent/Cargo.toml` — sysinfo 0.33, axum 0.8, reqwest 0.12, serial_test 3, tempfile 3, tower 0.5 confirmed
- `crates/rc-common/src/lib.rs` — current modules: types, protocol, udp_protocol, watchdog, ai_names
- `.planning/PROJECT.md` — v11.0 requirements list

### Secondary (HIGH confidence — external verification)
- crates.io/crates/wait-timeout — version 0.2, Windows WaitForSingleObject impl confirmed
- crates.io/crates/mockall — version 0.13.1, MSRV 1.77, 84M downloads confirmed
- docs.rs/tracing — tracing does not require tokio; set_global_default() covers std::thread threads
- std::sync::atomic documentation — AtomicUsize cap-and-reject is the standard stdlib pattern

### Tertiary (context)
- CLAUDE.md standing rules — "Refactor Second" rule, deployment rules, pendrive recovery path
- PROJECT.md v11.0 — active requirements confirmed to match research scope

---
*Research completed: 2026-03-20 IST*
*Ready for roadmap: yes*
