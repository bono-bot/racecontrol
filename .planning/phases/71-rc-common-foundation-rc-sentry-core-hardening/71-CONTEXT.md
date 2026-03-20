# Phase 71: rc-common Foundation + rc-sentry Core Hardening - Context

**Gathered:** 2026-03-20
**Status:** Ready for planning

<domain>
## Phase Boundary

Fix rc-sentry's three live correctness failures (timeout not enforced, no output truncation, unbounded thread spawning) and add structured logging. Establish a feature-gated exec primitive in rc-common that rc-sentry uses for sync exec and rc-agent can use for async exec. rc-sentry MUST remain stdlib-only (no tokio dependency).

</domain>

<decisions>
## Implementation Decisions

### rc-common exec API
- New module: `crates/rc-common/src/exec.rs`
- `run_cmd_sync(cmd: &str, timeout: Duration, max_output: usize) -> ExecResult` — stdlib-only, uses `std::process::Command` + `wait-timeout` crate for child process timeout enforcement
- `run_cmd_async(cmd: &str, timeout: Duration, max_output: usize) -> ExecResult` — behind `tokio` Cargo feature, uses `tokio::process::Command` + `tokio::time::timeout`
- `ExecResult` struct: `stdout: String, stderr: String, exit_code: i32, timed_out: bool, truncated: bool`
- Output truncation happens inside rc-common (not in caller) — `MAX_OUTPUT` default 64KB matching rc-agent remote_ops
- On timeout: kill child process, return partial output with `timed_out: true`
- On Windows: `CREATE_NO_WINDOW` flag applied in both sync and async variants
- rc-common Cargo.toml: `wait-timeout = "0.2"` as required dep; `tokio` as optional dep behind `[features] tokio = ["dep:tokio"]`
- Verification: `cargo tree -p rc-sentry` must show no tokio in dependency tree

### rc-sentry threading model
- AtomicUsize counter for concurrency limiting (cap at 4 concurrent execs)
- Increment on exec entry, decrement on exit (in Drop guard for safety)
- Reject with HTTP 429 `{"error":"too many concurrent requests"}` when at capacity
- No thread pool — keep per-connection `thread::spawn` model (simple, matches existing pattern)
- Thread name set to `sentry-handler-{N}` for debuggability

### rc-sentry timeout enforcement
- Use `wait-timeout` crate via rc-common's `run_cmd_sync`
- `_timeout_ms` field (line 78) becomes active — default 30_000ms if not provided
- On timeout: child process killed via `Child::kill()`, response includes `"timed_out": true`
- Output truncation to 64KB via rc-common

### Logging migration
- Add `tracing` and `tracing-subscriber` to rc-sentry Cargo.toml (both already in workspace)
- Plain text format on stdout (NOT JSON) — rc-sentry is a lightweight tool, JSON logging is overkill
- No log file rotation — stdout only, matches the tool's minimal philosophy
- Replace all `eprintln!` with `tracing::info!`, `tracing::warn!`, `tracing::error!`
- Init tracing in main() before TcpListener::bind

### Partial TCP read fix
- Content-Length-aware read loop: parse Content-Length header, read until that many body bytes received
- Max request size: 64KB (existing MAX_BODY constant, already defined)
- No chunked transfer support — rc-sentry is called by curl/reqwest with Content-Length always set
- Read loop with 30s total timeout (existing stream.set_read_timeout)
- If Content-Length missing on POST: read until connection timeout, treat available data as body (backward compat)

### HTTP response helpers
- Add HTTP 429 status to `send_response` match
- Keep existing response format (no change to /ping or CORS behavior)

### Claude's Discretion
- Exact ordering of rc-common vs rc-sentry changes within plans
- Whether to add build.rs to rc-sentry now (for GIT_HASH embedding) or defer to Phase 72
- Internal naming of the concurrency guard struct (SlotGuard, ExecSlot, etc.)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### rc-sentry source (modify)
- `crates/rc-sentry/src/main.rs` — Current 155-line implementation; all changes happen here
- `crates/rc-sentry/Cargo.toml` — Must add tracing, tracing-subscriber, rc-common dependency

### rc-common source (extend)
- `crates/rc-common/src/lib.rs` — Add `pub mod exec;` declaration
- `crates/rc-common/Cargo.toml` — Add wait-timeout, optional tokio

### rc-agent reference patterns (read-only)
- `crates/rc-agent/src/remote_ops.rs` — Reference implementation: semaphore pattern (line 51), exec with timeout (line 49), output truncation, CREATE_NO_WINDOW

### Research
- `.planning/research/PITFALLS.md` — 10 pitfalls including partial TCP read (Pitfall 6), tokio contamination (Pitfall 4), unbounded threads (Pitfall 2)
- `.planning/research/ARCHITECTURE.md` — Build order and integration points
- `.planning/research/STACK.md` — wait-timeout 0.2 recommendation, AtomicUsize pattern

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `remote_ops.rs` exec pattern: semaphore + timeout + truncation + CREATE_NO_WINDOW — the reference for rc-common's exec API
- `rc-agent build.rs`: embeds GIT_HASH — can be copied to rc-sentry if needed for /version (Phase 72)
- `MAX_BODY` const already defined in rc-sentry (64KB) — reuse for output truncation cap

### Established Patterns
- Cargo workspace features: rc-common already uses workspace-level dependency declarations
- `Connection: close` header: both rc-agent and rc-sentry already set this (prevents CLOSE_WAIT)
- Static CRT: `.cargo/config.toml` applies to all workspace members including rc-sentry

### Integration Points
- rc-sentry Cargo.toml: add `rc-common = { path = "../rc-common" }` (no features = sync-only exec)
- rc-agent Cargo.toml: add `features = ["tokio"]` to existing rc-common dependency (when Phase 72+ migrates)
- rc-common lib.rs: new `pub mod exec;` alongside existing protocol, types, watchdog modules

</code_context>

<specifics>
## Specific Ideas

No specific requirements — user delegated all decisions to Claude's discretion. Decisions above reflect the research recommendations and existing codebase patterns.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 71-rc-common-foundation-rc-sentry-core-hardening*
*Context gathered: 2026-03-20*
