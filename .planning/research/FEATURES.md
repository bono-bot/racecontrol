# Feature Research

**Domain:** Rust HTTP server hardening, large module decomposition, shared library extraction, test patterns
**Researched:** 2026-03-20
**Confidence:** HIGH (primary source: direct codebase analysis + established Rust patterns)

---

## Feature Landscape

### Table Stakes (Users Expect These)

These are baseline behaviors any production operations tool must have. "Users" here are the operations team (James, Bono) and Uday — they expect tools to not hang, not flood memory, and to be testable.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| rc-sentry timeout enforcement | `timeout_ms` is parsed from the request JSON into `_timeout_ms` but silently ignored. The `Command::new("cmd.exe").output()` call has no timeout wrapping at all — a hung command hangs that OS thread forever. | LOW | rc-agent already does this with `tokio::time::timeout`. rc-sentry uses `std::process::Command` (blocking) — use `std::thread::spawn` + `channel.recv_timeout()` pattern since tokio is not available |
| rc-sentry output truncation | rc-agent truncates at 64KB in both remote_ops.rs and handle_ws_exec(). rc-sentry has no truncation — `dir /S C:\` could produce megabytes flooding the response buffer | LOW | Apply the same 65,536 byte threshold after `String::from_utf8_lossy()`. Append `\n... [truncated]` suffix |
| rc-sentry concurrency limit | Current design spawns a new OS thread per request (`std::thread::spawn`). Under fleet health polling at 8 pods, 8+ concurrent blocking threads can run with no cap | MEDIUM | `AtomicUsize` counter with a configured max (e.g., 4) — reject with 429 if over limit. No semaphore available without tokio; use atomic increment/decrement with a RAII guard |
| rc-sentry structured logging | Currently only bare `eprintln!()` with no timestamps, no level, no pod context. Staff cannot diagnose issues from logs | LOW | `eprintln!("[{} rc-sentry INFO] {msg}", chrono::Local::now().format(...))`. No tracing crate (no tokio dep); keep it simple. `chrono` is already a workspace dep |
| rc-agent main.rs decomposition | main.rs is 3,404 lines. State machines, WS handler, config loading, and event loop all share local variables inside one giant `run()` function — unmaintainable and untestable | HIGH | Extract in order of risk: `config.rs` (AgentConfig, validate_config, load_config, defaults), `state_machine.rs` (LaunchState, CrashRecoveryState enums + transition methods), `ws_handler.rs` (WS connect/reconnect outer loop) |
| Unit tests for billing_guard.rs | billing_guard has no tests. It performs state-machine logic (BILL-02 / BILL-03 / SESSION-01) that gates auto-end of billing sessions — wrong behavior costs money or traps sessions | MEDIUM | Inject a `watch::Sender<FailureMonitorState>` in tests. Mock the HTTP call with a flag/channel. Test: stuck session threshold fires at 60s, idle drift fires at 300s, suppressed when `recovery_in_progress = true` |
| Unit tests for failure_monitor.rs | failure_monitor drives the entire autonomous healing loop (game freeze, launch timeout, USB). No tests exist. | MEDIUM | Same watch channel injection pattern. Stub `try_auto_fix` via `#[cfg(test)]` conditional. Test: freeze detection (UDP silence >= 30s), launch timeout (90s), HID disconnect toggle |
| Unit tests for FFB safety | ffb_controller.rs has no tests. The panic-safe HID commands run on `spawn_blocking` — need to verify safety functions are called on correct lifecycle events | MEDIUM | Pure logic tests only (not HID hardware tests). Verify `SESSION_END_IN_PROGRESS` AtomicBool is set/cleared correctly. HID send path left untested (hardware-bound) |
| rc-sentry /health endpoint | rc-agent has `GET /health` returning uptime, exec slots, version. rc-sentry has only `GET /ping`. During a failover when rc-agent is down, operators need to confirm sentry is alive | LOW | Returns JSON: `{"status":"ok","uptime_secs":N,"version":"X.Y.Z","pid":N}`. Static `OnceLock<Instant>` for start time. `env!("CARGO_PKG_VERSION")` for version |
| rc-sentry /version endpoint | rc-agent reports `version` and `build_id` in /health. rc-sentry has no version surface — operators cannot confirm which binary is deployed during incident recovery | LOW | Returns JSON: `{"version":"X.Y.Z","build_id":"git-hash"}`. Needs `build.rs` to embed `GIT_HASH` (same pattern as rc-agent) |
| rc-sentry /files endpoint | rc-agent has `GET /files` listing directory contents. When rc-agent is down, sentry is the fallback — operators need to verify the binary is present before trying to start the agent | MEDIUM | List directory: parse `?path=C:\RacingPoint` query param from request URI (manual string split, no axum). Return JSON array of `{name, size, is_dir, modified}`. Use `std::fs::read_dir()` |
| rc-sentry /processes endpoint | When rc-agent is down you need to know: is rc-agent.exe running? Is the game running? No equivalent endpoint exists anywhere in the stack | MEDIUM | Returns running processes via `tasklist /FO CSV /NH` shell-out. Parse CSV output. Returns JSON array of `{name, pid}`. `tasklist` is always available on Windows 11 — no extra deps |
| rc-sentry endpoint tests | New endpoints need tests verifying correct behavior without a real pod | MEDIUM | Integration tests in `crates/rc-sentry/tests/`. Send raw HTTP via `TcpStream` to a sentry bound on a test port. Verify JSON shape and status codes |

### Differentiators (Competitive Advantage)

These features make the v11.0 outcome meaningfully better than just "it compiles."

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| rc-common exec helper | rc-agent remote_ops.rs and main.rs both implement the same exec pattern (semaphore acquire + timeout + CREATE_NO_WINDOW + output truncate + release). Extracting to `rc-common::exec::run_command()` eliminates drift — one change benefits both callers | MEDIUM | Signature: `pub async fn run_command(cmd: &str, timeout_ms: u64, permit: SemaphorePermit<'_>) -> ExecResult`. Return type: `ExecResult { stdout, stderr, exit_code, timed_out }`. Lives in `rc-common::exec` |
| Characterization tests before decomposition | Standing rule: "characterization tests first, verify green, then refactor. No exceptions." For main.rs this means pinning current behavior of config loading and state machine transitions before touching a line of the decomposition | HIGH | These are the hardest tests to write (code was not written for testability). Minimum viable characterization: unit test `validate_config()` and `load_config()` with temp files. These are pure functions — extracting them to `config.rs` first is the prerequisite |
| Sentry build.rs for GIT_HASH | rc-agent and racecontrol already embed `GIT_HASH` via `build.rs`. rc-sentry has no build.rs. Adding one lets /version report the actual deployed commit — critical for confirming a deploy succeeded | LOW | rc-sentry already has `[target.cfg(windows).build-dependencies] winres`. Add a `build.rs` that outputs `GIT_HASH` from `git rev-parse --short HEAD`. See rc-agent/build.rs |
| 429 response on sentry overload | Rather than silently queueing or panicking, HTTP 429 tells the caller "try later" — allows fleet_health poller to back off rather than pile on | LOW | Only valuable when concurrency limit is implemented. Costs a few lines of code |

### Anti-Features (Commonly Requested, Often Problematic)

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| Add tokio/axum to rc-sentry | Would make timeout and semaphore trivial (both are already in rc-agent) | rc-sentry's value is minimal binary size and zero shared deps — stated in the module doc: "No tokio, no async — pure std::net for minimal binary size and zero shared deps." Adding tokio doubles binary size and compile time. The fallback must compile and run even when workspace has dep issues | Use `std::thread::spawn` + `channel.recv_timeout()` for timeout. Use `AtomicUsize` for concurrency. Pure std. |
| Full test coverage of all rc-agent modules | Testing everything sounds correct | Some modules are hardware-bound (ffb_controller HID writes, driving_detector HID reads, firewall.rs netsh calls). Tests for these require Windows + attached wheelbase + admin rights. Forcing 100% coverage stalls the milestone | Test the pure logic paths only: threshold calculations, state machine transitions, config validation. Mark hardware-dependent code with `#[cfg(not(test))]` where needed |
| Rewrite main.rs event loop | "Let's make it clean while we're in here" | The main select! loop coordinates 10+ concurrent tasks via channels. Refactoring the coordination logic risks breaking timing-sensitive paths like the crash recovery state machine — silent behavioral regressions only surface under live conditions | Extract only what is clearly separable with no behavioral change: config types, pure validation functions, state machine enums + transition methods. Leave the select! dispatch body structurally unchanged |
| Shared HTTP client in rc-common | Tempting to centralize the reqwest client so billing_guard and other modules share it | rc-sentry cannot use reqwest (no tokio dep). rc-common would need feature flags. The reqwest dep would pull tokio into rc-common, affecting compilation for any crate that imports rc-common without wanting async | Keep reqwest in rc-agent only. The billing_guard orphan client stays local with `OnceLock`. rc-common remains sync and dep-light |

---

## Feature Dependencies

```
rc-sentry /version endpoint
    └──requires──> rc-sentry build.rs (GIT_HASH output)

rc-sentry /health endpoint
    └──requires──> start_time OnceLock (trivial inline addition)

rc-sentry /files endpoint
    └──requires──> Manual query string parser (no axum, must split path?query manually)

rc-sentry concurrency limit
    └──must be co-implemented with──> rc-sentry timeout enforcement
         (both require the same threading design: child thread + channel.recv_timeout)

rc-common exec helper
    └──requires──> rc-common gains async via tokio (already transitive via rc-agent)
    └──enhances──> rc-agent remote_ops exec (remove duplication)
    └──enhances──> rc-agent ws_exec handler (remove duplication)
    └──conflicts-with──> rc-sentry (rc-sentry must NOT depend on rc-common — no tokio)

Characterization tests
    └──must precede──> rc-agent main.rs decomposition (standing rule: no exceptions)

rc-agent config.rs extraction
    └──enables──> Unit tests for validate_config() (now a testable pure function)
    └──enables──> Unit tests for load_config() (testable with temp files)
    └──must precede──> state_machine.rs extraction (config types are dependencies)

rc-agent state_machine.rs extraction
    └──enables──> Unit tests for LaunchState transitions
    └──enables──> Unit tests for CrashRecoveryState transitions

billing_guard unit tests
    └──requires──> FailureMonitorState is pub (already pub)
    └──requires──> watch::Sender injection (already designed: spawn() takes watch::Receiver)
    └──requires──> HTTP call mockable (currently hardcoded reqwest — needs conditional or callback)

failure_monitor unit tests
    └──requires──> try_auto_fix mockable (calls ai_debugger -> Ollama)
    └──solution──> #[cfg(test)] stub module for try_auto_fix

rc-sentry endpoint tests
    └──requires──> All new endpoints implemented first
    └──requires──> Test port that does not conflict with production port 8091
```

### Dependency Notes

- **rc-sentry timeout and concurrency must be implemented together.** The current blocking `Command.output()` call cannot have a timeout without threading the execution — a child thread runs the command and sends result via channel, the parent thread calls `recv_timeout`. Both the timeout limit and the concurrency counter depend on this threading change.
- **failure_monitor tests require mocking try_auto_fix.** `try_auto_fix` calls the Ollama endpoint via HTTP. In tests, this must be disabled. Fastest approach: `#[cfg(test)] mod ai_debugger { pub async fn try_auto_fix(...) -> bool { true } }` in the test file to shadow the real module. Cleaner approach: pass a `fix_fn` callback to `failure_monitor::spawn()` — this also improves production testability but requires a signature change.
- **rc-common exec helper must not affect rc-sentry.** rc-sentry's `Cargo.toml` deliberately has no rc-common dependency. The exec helper in rc-common is only for rc-agent. Do not add rc-common to rc-sentry's deps.

---

## MVP Definition

### Phase 1: Sentry Hardening + Critical Tests

Minimum viable for this milestone — makes rc-sentry reliable and proves billing/healing code is correct.

- [ ] rc-sentry timeout enforcement — prevents hung threads from accumulating
- [ ] rc-sentry output truncation — prevents memory exhaustion
- [ ] rc-sentry concurrency limit — prevents thread flood (implement with timeout: same threading change)
- [ ] rc-sentry structured logging — timestamp + level prefix via eprintln
- [ ] rc-sentry /health endpoint — confirms sentry is alive with version
- [ ] rc-sentry /version endpoint — confirms deployed binary (requires build.rs)
- [ ] Unit tests for billing_guard.rs — highest business risk (wrong = lost revenue or stuck sessions)
- [ ] Unit tests for failure_monitor.rs — wrong = undetected game freezes

### Phase 2: Decomposition + Fallback Endpoints

Add after Phase 1 is solid and tests are green.

- [ ] rc-agent config.rs extraction (characterization tests must be green first)
- [ ] rc-agent state_machine.rs extraction
- [ ] rc-sentry /files endpoint — fallback binary verification
- [ ] rc-sentry /processes endpoint — fallback process health check
- [ ] rc-common exec helper extraction — eliminates drift between remote_ops and ws_exec
- [ ] rc-sentry endpoint tests — validate all new endpoints

### Future Consideration (v12.0+)

- [ ] FFB safety unit tests — hardware mock design is non-trivial; defer unless FFB incidents occur
- [ ] rc-agent ws_handler.rs extraction — timing-sensitive; only after simpler extractions prove safe
- [ ] rc-agent event_loop.rs full extraction — highest regression risk; defer to last

---

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| rc-sentry timeout enforcement | HIGH | LOW | P1 |
| rc-sentry output truncation | HIGH | LOW | P1 |
| rc-sentry concurrency limit | HIGH (co-impl with timeout) | MEDIUM | P1 |
| rc-sentry structured logging | MEDIUM | LOW | P1 |
| rc-sentry /health endpoint | HIGH | LOW | P1 |
| rc-sentry /version endpoint | MEDIUM | LOW (+ build.rs) | P1 |
| billing_guard unit tests | HIGH — business-critical | MEDIUM | P1 |
| failure_monitor unit tests | HIGH — drives healing | MEDIUM | P1 |
| rc-agent config.rs extraction | MEDIUM | LOW | P2 |
| rc-agent state_machine.rs extraction | MEDIUM | MEDIUM | P2 |
| rc-sentry /files endpoint | HIGH in failover | MEDIUM | P2 |
| rc-sentry /processes endpoint | HIGH in failover | MEDIUM | P2 |
| rc-common exec helper | MEDIUM — removes drift | MEDIUM | P2 |
| rc-sentry endpoint tests | HIGH | MEDIUM | P2 |
| FFB safety unit tests | LOW (no recent incidents) | HIGH | P3 |
| rc-agent ws_handler.rs extraction | MEDIUM | HIGH (regression risk) | P3 |
| rc-agent event_loop.rs extraction | LOW | HIGH (regression risk) | P3 |

**Priority key:**
- P1: Must have for this milestone
- P2: Should have, add after P1 is solid
- P3: Nice to have, future milestone

---

## Implementation Notes by Feature Area

### HTTP Server Hardening (rc-sentry)

rc-sentry uses `std::net` (no tokio). Timeout and concurrency share a threading design:

The current per-request thread calls `Command::output()` which blocks. To add timeout:
1. Spawn a child thread that runs the command and sends the result via a `std::sync::mpsc::channel`
2. The request thread calls `receiver.recv_timeout(Duration::from_millis(timeout_ms))`
3. On timeout: kill the child process, return `{"error":"timed_out","exit_code":-1}`
4. An `AtomicUsize` slot counter guards concurrency — increment on entry, decrement via Drop guard, reject with 429 if over limit

The `_timeout_ms` variable in the current code already reads the JSON field — it just needs to be wired to `recv_timeout` instead of being discarded.

Output truncation: after `String::from_utf8_lossy(&output.stdout)`, check `.len() > 65_536`. Match rc-agent's exact threshold for consistency.

### Module Decomposition (rc-agent main.rs)

The 3,404-line main.rs has clearly separable sections by risk level:

| Approx Lines | Content | Extract to | Risk |
|--------------|---------|------------|------|
| 1–70 | `mod` declarations | stays in `main.rs` | None |
| 70–200 | AgentConfig and sub-structs | `config.rs` | LOW |
| 200–320 | LaunchState, CrashRecoveryState enums | `state_machine.rs` | LOW |
| 320–380 | handle_ws_exec() function | `ws_handler.rs` or `state_machine.rs` | LOW |
| 380–500 | `main()` setup: logging, panic hook, firewall | stays in `main.rs` | None |
| 500–2800 | `run()` WS connect + reconnect loop | `ws_handler.rs` | MEDIUM |
| 800–2800 | select! dispatch body (inside run()) | `event_loop.rs` | HIGH — defer |
| 2800–3404 | validate_config, load_config, local_ip utilities | `config.rs` | LOW |

Extract in risk order: config types first (pure data), then state machine enums + methods, then the ws_handler outer structure. Stop before touching the select! dispatch body.

### Shared Library Extraction (rc-common)

What belongs in rc-common after this milestone:
- `ExecResult` type: `{ stdout: String, stderr: String, exit_code: i32, timed_out: bool }`
- `run_command(cmd, timeout_ms, permit) -> ExecResult` — async, requires tokio feature

What must NOT go in rc-common:
- reqwest HTTP client — keeps rc-sentry dep-free
- Windows-specific process creation flags (CREATE_NO_WINDOW) — rc-agent concern only
- tracing subscriber setup — application-level, stays in each binary's `main()`

rc-common currently has: types, protocol, udp_protocol, watchdog, ai_names. The exec module fits cleanly alongside these.

### Test Patterns (Rust)

Patterns consistent with what exists in `crates/racecontrol/tests/integration.rs`:

**watch channel injection for billing_guard:**
The `spawn()` function already accepts `watch::Receiver<FailureMonitorState>` — tests can drive state changes via the paired `Sender` without modifying the production code. The HTTP orphan-end call is the only part that needs mocking — pass an injectable base URL and use a mock HTTP server (or `mockito`) to verify calls.

**rc-sentry endpoint tests:**
Send raw HTTP via `std::net::TcpStream` to a sentry instance started on a test port (e.g., 18091). Parse the response string to verify status code and JSON fields. No HTTP client library needed.

**Config unit tests:**
`validate_config()` is a pure function — once extracted to `config.rs` it can be tested with inline struct literals. No I/O, no async, no mocking. These are the fastest tests to write and highest confidence return.

**try_auto_fix stub for failure_monitor tests:**
Use `#[cfg(test)]` to replace the Ollama call with a no-op. In the test file:
```
#[cfg(test)]
mod ai_debugger_stub { ... }
```
This shadows the real module within the test binary without changing production code.

---

## Sources

- Direct codebase analysis: `crates/rc-sentry/src/main.rs` — `_timeout_ms` unused at line 78, no output truncation, no concurrency limit
- Direct codebase analysis: `crates/rc-agent/src/remote_ops.rs` — truncation at 65,536 bytes, Semaphore pattern with 8 slots
- Direct codebase analysis: `crates/rc-agent/src/main.rs` — 3,404 lines confirmed, LaunchState and CrashRecoveryState enums present
- Direct codebase analysis: `crates/rc-agent/src/billing_guard.rs` — BILL-02/03/SESSION-01 logic, no tests present
- Direct codebase analysis: `crates/rc-agent/src/failure_monitor.rs` — CRASH-01/02/USB-01 detection, no tests present
- Direct codebase analysis: `crates/rc-sentry/Cargo.toml` — deps: serde, serde_json, toml only; no tokio
- Existing test reference: `crates/racecontrol/tests/integration.rs` — in-memory SQLite pattern for integration tests
- PROJECT.md v11.0 requirements (active requirements list)
- CLAUDE.md standing rule: "Refactor Second — characterization tests first, verify green, then refactor. No exceptions."

---
*Feature research for: v11.0 Agent & Sentry Hardening*
*Researched: 2026-03-20*
