# Stack Research

**Domain:** Rust daemon hardening -- rc-sentry timeout/concurrency, rc-agent decomposition, rc-common extraction, test infrastructure
**Researched:** 2026-03-20
**Confidence:** HIGH (all additions verified against crates.io, docs.rs, or already present in Cargo.lock)

---

## What Is Already Present (Do Not Re-Add)

Confirmed in the workspace Cargo.toml and crate Cargo.toml files:

| Crate | Already Available In |
|-------|---------------------|
| serde / serde_json | workspace (all crates) |
| tokio (full features) | workspace (rc-agent) |
| tracing + tracing-subscriber + tracing-appender | workspace (rc-agent) |
| anyhow / thiserror | workspace (rc-agent) |
| reqwest 0.12 | rc-agent Cargo.toml |
| axum 0.8 | rc-agent Cargo.toml |
| sysinfo 0.33 | rc-agent Cargo.toml |
| tokio::sync::Semaphore | available via tokio (rc-agent) |
| tokio::time::timeout | available via tokio (rc-agent) |
| serial_test 3 | rc-agent dev-dependencies |
| tempfile 3 | rc-agent dev-dependencies |
| tower 0.5 | rc-agent dev-dependencies |
| http-body-util 0.1 | rc-agent dev-dependencies |

---

## Recommended Stack -- New Additions Only

### rc-sentry: Timeout Enforcement

**Problem:** handle_exec() calls Command::new("cmd.exe").output() with no timeout. The _timeout_ms field is parsed but ignored (line 78 in main.rs). A hung command blocks the thread indefinitely.

**Solution:** wait-timeout 0.2 -- adds ChildExt::wait_timeout(Duration) to std::process::Child. On Windows this calls WaitForSingleObject with a timeout, then kills the child if it has not exited. Pure stdlib, zero async dependency, fits rc-sentry no-tokio design.

| Library | Version | Add To | Purpose |
|---------|---------|--------|---------|
| wait-timeout | 0.2 | rc-sentry dependencies | Enforce timeout_ms in /exec -- kill hung cmd.exe after deadline |

**Why not tokio::time::timeout?** rc-sentry deliberately avoids tokio. Adding tokio would triple the binary size and contradict the design intent. wait-timeout is the correct std-compatible solution.

**Why not set_read_timeout on the stream?** output() waits for the child to exit -- set_read_timeout only affects socket reads, not child process waits.



---

### rc-sentry: Concurrency Limiting

**Problem:** rc-sentry spawns a new std::thread per connection with no cap. A flood of /exec requests spawns unbounded threads.

**Solution:** std::sync::atomic::AtomicUsize -- stdlib only, no new dependency. Pattern: fetch_add before running the command, check against MAX_CONCURRENT_EXECS, return HTTP 429 if over limit, fetch_sub in all exit paths. This is a cap-and-reject pattern -- appropriate for a daemon that must remain responsive.

No new crate needed. AtomicUsize is in std::sync::atomic. A tokio Semaphore would require the async runtime which rc-sentry intentionally avoids.

---

### rc-sentry: Structured Logging

**Problem:** rc-sentry uses eprintln!() -- unstructured, no timestamps, no log levels.

**Solution:** tracing + tracing-subscriber -- both already in the workspace. rc-sentry declares them as dependencies and calls tracing_subscriber::fmt::init() at startup. No new crates required. tracing does not require tokio and works with std::thread via set_global_default().



---

### rc-agent: Module Decomposition

No new crates needed. This is a Rust module system refactor. The modern pattern (Rust 2018+) is src/module_name.rs + mod module_name; in main.rs.

Suggested extraction targets from the 3,400-line main.rs:

| New Module File | Extracts | Lines (approx) |
|----------------|----------|----------------|
| src/app_state.rs | AppState struct + shared Arc fields | ~150 |
| src/config.rs | AgentConfig + all config sub-structs | ~200 |
| src/ws_handler.rs | WebSocket connect loop, message dispatch, reconnect backoff | ~400 |
| src/event_loop.rs | Main select! loop, timer ticks, signal handling | ~350 |
| src/session.rs | Session state machine, orphan detection, billing sync | ~300 |

No new tooling or crates are required. pub use re-exports handle visibility at module boundaries.

---

### rc-common: Shared Exec Pattern Extraction

**Problem:** remote_ops.rs (rc-agent) and rc-sentry both implement exec + timeout + output truncation. They diverge -- rc-agent uses async tokio::process::Command with tokio::time::timeout; rc-sentry uses sync std::process::Command with no timeout enforcement at all.

**Solution:** Extract a synchronous exec_cmd(cmd, timeout_ms, max_output_bytes) -> ExecResult function into rc-common. rc-agent keeps its own tokio-based implementation. rc-sentry and any other sync caller use the shared function.

rc-sentry links rc-common and calls rc_common::exec::exec_cmd(). This eliminates the duplicated pattern while preserving rc-sentry no-tokio constraint.



The shared type lives at rc-common/src/exec.rs:



---

### Test Infrastructure

**Existing dev-dependencies already in rc-agent (no additions needed):**

| Library | Version | Purpose |
|---------|---------|----------|
| tempfile | 3 | Temp files/dirs for file system tests |
| serial_test | 3 | Serialize tests that share global state (port binds, static singletons) |
| tower | 0.5 | tower::ServiceExt::oneshot() for axum handler unit tests |
| http-body-util | 0.1 | Read response bodies in axum handler tests |
| tokio::test | (tokio feature) | via tokio -- the test attribute macro |

**New test-only addition for rc-agent:**

The billing_guard, failure_monitor, and FFB safety tests need to exercise code paths that call reqwest::Client::post() (orphan session end). Mocking requires trait abstraction.

| Library | Version | Add To | Purpose |
|---------|---------|--------|---------|
| mockall | 0.13 | rc-agent dev-dependencies | Mock traits for billing_guard / failure_monitor HTTP isolation |

**Why mockall?** The billing guard calls reqwest::Client::post() directly. To unit-test the guard logic without a live HTTP server, wrap the client behind a trait and use mockall automock. mockall 0.13.1: 84M downloads, supports async methods, MSRV 1.77 (project uses 1.93).

**Why not wiremock?** wiremock spins up a real HTTP server -- appropriate for integration tests, too heavy for billing_guard unit tests.

**rc-sentry endpoint tests:** No new dev-dependencies needed. Use std::net::TcpListener::bind with port 0 (OS assigns port) in test setup, connect with std::net::TcpStream, send raw HTTP, parse response. Avoids port collisions with the real sentry on :8091.

---

## Complete Dependency Delta

Changes relative to current state -- minimal by design.

**crates/rc-sentry/Cargo.toml** -- add tracing, tracing-subscriber, wait-timeout to dependencies. No dev-dependencies needed.

**crates/rc-common/Cargo.toml** -- add wait-timeout to dependencies.

**crates/rc-agent/Cargo.toml** -- add mockall to dev-dependencies only.

**Total new crates: 2** -- wait-timeout (used in both rc-sentry and rc-common), mockall (dev-only in rc-agent). Everything else is already present.

---

## Alternatives Considered

| Recommended | Alternative | Why Not |
|-------------|-------------|---------|
| wait-timeout 0.2 | tokio + tokio::time::timeout | Pulls async runtime into rc-sentry, contradicts no-async design |
| wait-timeout 0.2 | set_read_timeout on TcpStream | Does not terminate the child process |
| AtomicUsize stdlib | External semaphore crate | Overkill for a cap-and-reject pattern; adds a dependency for ~10 lines of code |
| tracing from workspace | Custom eprintln formatting | Workspace already pays the compile cost; rc-sentry gets timestamps + levels for free |
| mockall 0.13 | wiremock | wiremock runs a real HTTP server -- too heavy for billing_guard unit tests |
| mockall 0.13 | Manual mock structs | Manual mocks require ongoing boilerplate maintenance |
| File-level mod decomposition | New workspace crate per subsystem | Cross-crate imports add complexity with no benefit at this scale |

---

## What NOT to Add

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| async-std | rc-sentry is sync by design; mixing runtimes is undefined behavior | wait-timeout + AtomicUsize |
| actix-web or hyper | rc-sentry uses pure std::net intentionally | Keep std::net::TcpListener, extend dispatch by hand |
| log crate | Project committed to tracing; mixing creates double initialization | tracing macros everywhere |
| env_logger | Redundant -- tracing-subscriber::fmt::init() covers the same use case | tracing-subscriber from workspace |
| criterion | Not needed for correctness testing of exec/billing paths | cargo test with assertions |
| proptest / quickcheck | Property testing suits parsers; exec and billing need deterministic scenarios | tokio::test + serial_test |
| New workspace member for rc-agent subsystems | Module decomposition inside one crate is the right scope | File-level mod in rc-agent src/ |

---

## Stack Patterns by Context

**If rc-sentry /processes endpoint needs process listing:**
Add sysinfo = "0.33" to rc-sentry Cargo.toml dependencies. Use the same version as rc-agent to share Cargo.lock resolution.

**If billing_guard tests need async HTTP mocking:**
1. Wrap reqwest::Client behind a trait (e.g. HttpClient)
2. Add mockall automock attribute to the trait
3. Inject the mock in tests via the trait bound
4. Production code gets the real reqwest client via the concrete impl

**If rc-sentry concurrency tests need to verify the 429 path:**
Spawn N std::threads each connecting a TcpStream. Assert that threads N+1 and above receive 429 in the response status line. No additional crate needed.

---

## Version Compatibility

| Package | Compatible With | Notes |
|---------|-----------------|-------|
| wait-timeout 0.2 | rustc 1.93.1 (project) | Pure stdlib bindings, no MSRV issues |
| tracing 0.1 workspace | tracing-subscriber 0.3 workspace | Same tracing ecosystem, already validated in rc-agent |
| mockall 0.13 | rustc >= 1.77 | Project at 1.93.1, well above MSRV |
| sysinfo 0.33 | Already in Cargo.lock via rc-agent | Adding to rc-sentry resolves same locked version |
| serial_test 3 | tokio 1.x | Already in rc-agent dev-deps, compatible |

---

## Sources

- crates.io/crates/wait-timeout -- version 0.2, Windows impl via WaitForSingleObject; confirmed at lib.rs (February 2025)
- crates.io/crates/mockall -- version 0.13.1, MSRV 1.77, 84M downloads; confirmed at generalistprogrammer.com (2025 guide)
- crates.io/crates/tempfile -- version 3.23.0, 379M downloads; confirmed current
- crates.io/crates/serial_test -- version 3.2.0, 75.5M downloads; confirmed current
- docs.rs/tracing -- tracing does not require tokio; set_global_default() covers all std::thread threads
- std::sync::atomic documentation -- AtomicUsize::fetch_add/fetch_sub is the standard cap-and-reject pattern
- rc-agent Cargo.toml inspected -- confirmed: sysinfo 0.33, axum 0.8, reqwest 0.12, serial_test 3, tempfile 3, tower 0.5
- rc-sentry src/main.rs inspected -- 155 LOC, pure std, timeout_ms parsed but ignored line 78, unbounded thread spawn

---

*Stack research for: v11.0 Agent and Sentry Hardening*
*Researched: 2026-03-20 IST*
