# Phase 71: rc-common Foundation + rc-sentry Core Hardening - Research

**Researched:** 2026-03-20 IST
**Domain:** Rust stdlib process management, Cargo feature gates, HTTP/TCP request parsing
**Confidence:** HIGH

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**rc-common exec API**
- New module: `crates/rc-common/src/exec.rs`
- `run_cmd_sync(cmd: &str, timeout: Duration, max_output: usize) -> ExecResult` — stdlib-only, uses `std::process::Command` + `wait-timeout` crate for child process timeout enforcement
- `run_cmd_async(cmd: &str, timeout: Duration, max_output: usize) -> ExecResult` — behind `tokio` Cargo feature, uses `tokio::process::Command` + `tokio::time::timeout`
- `ExecResult` struct: `stdout: String, stderr: String, exit_code: i32, timed_out: bool, truncated: bool`
- Output truncation happens inside rc-common (not in caller) — `MAX_OUTPUT` default 64KB matching rc-agent remote_ops
- On timeout: kill child process, return partial output with `timed_out: true`
- On Windows: `CREATE_NO_WINDOW` flag applied in both sync and async variants
- rc-common Cargo.toml: `wait-timeout = "0.2"` as required dep; `tokio` as optional dep behind `[features] tokio = ["dep:tokio"]`
- Verification: `cargo tree -p rc-sentry` must show no tokio in dependency tree

**rc-sentry threading model**
- AtomicUsize counter for concurrency limiting (cap at 4 concurrent execs)
- Increment on exec entry, decrement on exit (in Drop guard for safety)
- Reject with HTTP 429 `{"error":"too many concurrent requests"}` when at capacity
- No thread pool — keep per-connection `thread::spawn` model (simple, matches existing pattern)
- Thread name set to `sentry-handler-{N}` for debuggability

**rc-sentry timeout enforcement**
- Use `wait-timeout` crate via rc-common's `run_cmd_sync`
- `_timeout_ms` field (line 78) becomes active — default 30_000ms if not provided
- On timeout: child process killed via `Child::kill()`, response includes `"timed_out": true`
- Output truncation to 64KB via rc-common

**Logging migration**
- Add `tracing` and `tracing-subscriber` to rc-sentry Cargo.toml (both already in workspace)
- Plain text format on stdout (NOT JSON) — rc-sentry is a lightweight tool, JSON logging is overkill
- No log file rotation — stdout only, matches the tool's minimal philosophy
- Replace all `eprintln!` with `tracing::info!`, `tracing::warn!`, `tracing::error!`
- Init tracing in main() before TcpListener::bind

**Partial TCP read fix**
- Content-Length-aware read loop: parse Content-Length header, read until that many body bytes received
- Max request size: 64KB (existing MAX_BODY constant, already defined)
- No chunked transfer support — rc-sentry is called by curl/reqwest with Content-Length always set
- Read loop with 30s total timeout (existing stream.set_read_timeout)
- If Content-Length missing on POST: read until connection timeout, treat available data as body (backward compat)

**HTTP response helpers**
- Add HTTP 429 status to `send_response` match
- Keep existing response format (no change to /ping or CORS behavior)

### Claude's Discretion
- Exact ordering of rc-common vs rc-sentry changes within plans
- Whether to add build.rs to rc-sentry now (for GIT_HASH embedding) or defer to Phase 72
- Internal naming of the concurrency guard struct (SlotGuard, ExecSlot, etc.)

### Deferred Ideas (OUT OF SCOPE)
None — discussion stayed within phase scope.
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| SHARED-01 | rc-common exposes run_cmd_sync (thread + timeout) for rc-sentry and sync contexts | wait-timeout 0.2.1 provides `child.wait_timeout(duration)` — the only correct stdlib approach on Windows |
| SHARED-02 | rc-common exposes run_cmd_async (tokio, feature-gated) for rc-agent | Cargo optional dep pattern: `[features] tokio = ["dep:tokio"]`; tokio workspace dep already declared |
| SHARED-03 | rc-sentry uses rc-common run_cmd_sync without pulling in tokio (verified via cargo tree) | Feature-gate boundary: rc-sentry Cargo.toml references rc-common with no features = no tokio pulled |
| SHARD-01 | rc-sentry enforces timeout_ms on command execution (kills child process after deadline) | wait-timeout: `child.wait_timeout(dur)` returns `Ok(None)` on timeout; caller must call `child.kill()` |
| SHARD-02 | rc-sentry truncates command output to 64KB (matching rc-agent remote_ops behavior) | Truncation: capture stdout/stderr bytes, truncate to max_output before String::from_utf8_lossy; MAX_BODY=65536 already in rc-sentry |
| SHARD-03 | rc-sentry limits concurrent exec requests to 4 (rejects with HTTP 429 when full) | AtomicUsize + compare_exchange pattern; Drop guard ensures decrement even on panic |
| SHARD-04 | rc-sentry fixes partial TCP read bug (loops until full HTTP body received) | Current code: single `stream.read(&mut buf)` — may return partial data. Fix: parse Content-Length header, loop read until all bytes received |
| SHARD-05 | rc-sentry uses structured logging via tracing (replaces eprintln) | tracing 0.1.44 + tracing-subscriber 0.3.23 already in workspace; `tracing_subscriber::fmt::init()` for plain stdout |
</phase_requirements>

---

## Summary

Phase 71 has two parallel tracks: (1) add a feature-gated exec primitive to rc-common so both rc-sentry (sync) and rc-agent (async) share one implementation, and (2) fix five live correctness defects in rc-sentry. These tracks are tightly coupled — rc-common must be implemented first because rc-sentry's timeout and truncation fixes are delivered by calling `rc_common::exec::run_cmd_sync`.

The primary technical challenge is the Cargo feature gate boundary. rc-sentry must never pull in tokio (it is deliberately stdlib-only for reliability as a fallback admin tool). The `dep:tokio` optional dependency pattern in rc-common's Cargo.toml achieves this: rc-sentry's Cargo.toml references rc-common without features, so tokio never appears in `cargo tree -p rc-sentry`. rc-agent will add `features = ["tokio"]` in a later phase.

All five rc-sentry bugs (no timeout, no truncation, no concurrency cap, partial TCP reads, eprintln logging) are independent of each other and can be fixed in any order once rc-common exec.rs exists. The partial TCP read fix (SHARD-04) is pure HTTP parsing work with no rc-common dependency and can be sequenced first.

**Primary recommendation:** Build rc-common exec.rs first (SHARED-01/02/03), then wire rc-sentry to it for timeout + truncation (SHARD-01/02), then add the AtomicUsize concurrency cap (SHARD-03), then fix the partial read loop (SHARD-04), then migrate eprintln to tracing (SHARD-05).

---

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| wait-timeout | 0.2.1 | Blocking child process timeout for stdlib `std::process::Child` | Only crate that correctly abstracts `WaitForSingleObject` (Windows) + `waitpid` with WNOHANG loop (Unix) — nothing in std does this |
| tracing | 0.1.44 | Structured logging macros (`info!`, `warn!`, `error!`) | Already in workspace; used by all other crates |
| tracing-subscriber | 0.3.23 | Log output formatting and filtering | Already in workspace; `fmt::init()` is one line |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| std::sync::atomic::AtomicUsize | stdlib | Concurrency slot counter | All concurrency counting in rc-sentry (no external dep needed) |
| std::process::Command | stdlib | Sync subprocess spawning | Used in run_cmd_sync |
| tokio::process::Command | tokio (feature-gated) | Async subprocess spawning | Used in run_cmd_async, only compiled when feature = "tokio" |
| std::os::windows::process::CommandExt | stdlib (windows) | `creation_flags(CREATE_NO_WINDOW)` | Applied to all cmd.exe invocations on Windows |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| wait-timeout 0.2 | Hand-roll with thread::sleep + child.try_wait | Racy, poor signal on timeout vs exit vs error |
| AtomicUsize + compare_exchange | Mutex<usize> | AtomicUsize is lock-free, correct for this use case |
| tracing plain fmt | tracing JSON | JSON is overkill for a 155-line stdlib tool; plain fmt is readable in terminal |

**Installation additions to workspace:**
```toml
# crates/rc-common/Cargo.toml — new additions
[dependencies]
wait-timeout = "0.2"

[dependencies.tokio]
workspace = true
optional = true

[features]
tokio = ["dep:tokio"]
```

```toml
# crates/rc-sentry/Cargo.toml — new additions
[dependencies]
rc-common = { path = "../rc-common" }   # no features = sync only, no tokio
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
```

**Version verification:** Confirmed against crates.io 2026-03-20:
- `wait-timeout`: 0.2.1 (latest)
- `tracing`: 0.1.44 (latest)
- `tracing-subscriber`: 0.3.23 (latest)

---

## Architecture Patterns

### Recommended Project Structure

New file: `crates/rc-common/src/exec.rs` — self-contained, no cross-module state.

```
crates/rc-common/src/
├── lib.rs           # add: pub mod exec;
├── exec.rs          # NEW: run_cmd_sync + run_cmd_async + ExecResult
├── types.rs         # existing
├── protocol.rs      # existing
├── udp_protocol.rs  # existing
├── watchdog.rs      # existing
└── ai_names.rs      # existing
```

### Pattern 1: Cargo Optional Feature Gate
**What:** Mark tokio as optional in rc-common; rc-sentry gets rc-common without tokio; rc-agent gets rc-common with tokio via `features = ["tokio"]`.
**When to use:** When a lib crate must serve both sync-only and async callers without contaminating the sync user's dependency tree.
**Example:**
```toml
# crates/rc-common/Cargo.toml
[dependencies]
wait-timeout = "0.2"

[dependencies.tokio]
version = "1"
features = ["full"]
optional = true

[features]
tokio = ["dep:tokio"]
```

```toml
# crates/rc-sentry/Cargo.toml — sync only (no tokio in cargo tree)
rc-common = { path = "../rc-common" }

# crates/rc-agent/Cargo.toml — add features to existing rc-common dep
rc-common = { workspace = true, features = ["tokio"] }
```

Verification command (run after every rc-common change):
```bash
cargo tree -p rc-sentry | grep tokio
# Must produce NO output
```

### Pattern 2: ExecResult Struct + run_cmd_sync Implementation
**What:** Single return type covers all outcomes: success, timeout, truncation, error.
**When to use:** When callers must distinguish timeout from error from large-output without multiple Result types.
**Example:**
```rust
// Source: rc-agent remote_ops.rs reference pattern (adapted for sync)
pub struct ExecResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub timed_out: bool,
    pub truncated: bool,
}

pub fn run_cmd_sync(cmd: &str, timeout: Duration, max_output: usize) -> ExecResult {
    use std::process::Command;
    #[cfg(windows)]
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    let mut child = {
        let mut c = Command::new("cmd.exe");
        c.args(["/C", cmd]);
        #[cfg(windows)]
        c.creation_flags(CREATE_NO_WINDOW);
        match c.spawn() {
            Ok(ch) => ch,
            Err(e) => return ExecResult {
                stdout: String::new(),
                stderr: e.to_string(),
                exit_code: -1,
                timed_out: false,
                truncated: false,
            },
        }
    };

    // wait-timeout: returns Ok(None) on timeout, Ok(Some(status)) on exit
    match child.wait_timeout(timeout).expect("wait_timeout failed") {
        None => {
            // Timed out — kill child, collect whatever output exists
            let _ = child.kill();
            let output = child.wait_with_output().unwrap_or_default();
            let (stdout, stderr, truncated) = truncate_output(output.stdout, output.stderr, max_output);
            ExecResult { stdout, stderr, exit_code: -1, timed_out: true, truncated }
        }
        Some(status) => {
            let output = child.wait_with_output().unwrap_or_default();
            let (stdout, stderr, truncated) = truncate_output(output.stdout, output.stderr, max_output);
            ExecResult {
                stdout,
                stderr,
                exit_code: status.code().unwrap_or(-1),
                timed_out: false,
                truncated,
            }
        }
    }
}

fn truncate_output(mut out: Vec<u8>, mut err: Vec<u8>, max: usize) -> (String, String, bool) {
    let truncated = out.len() + err.len() > max;
    out.truncate(max);
    err.truncate(max.saturating_sub(out.len()));
    (
        String::from_utf8_lossy(&out).into_owned(),
        String::from_utf8_lossy(&err).into_owned(),
        truncated,
    )
}
```

### Pattern 3: AtomicUsize Concurrency Cap with Drop Guard
**What:** Increment on entry with compare_exchange; decrement in Drop impl; reject with 429 when at cap.
**When to use:** Thread-per-connection model without a thread pool — need slot tracking without blocking.
**Example:**
```rust
use std::sync::atomic::{AtomicUsize, Ordering};

const MAX_EXEC_SLOTS: usize = 4;
static EXEC_SLOTS: AtomicUsize = AtomicUsize::new(0);

struct SlotGuard;

impl SlotGuard {
    fn acquire() -> Option<Self> {
        loop {
            let current = EXEC_SLOTS.load(Ordering::Acquire);
            if current >= MAX_EXEC_SLOTS { return None; }
            match EXEC_SLOTS.compare_exchange(current, current + 1, Ordering::AcqRel, Ordering::Acquire) {
                Ok(_) => return Some(SlotGuard),
                Err(_) => continue, // another thread raced; retry
            }
        }
    }
}

impl Drop for SlotGuard {
    fn drop(&mut self) {
        EXEC_SLOTS.fetch_sub(1, Ordering::Release);
    }
}

// In handle_exec:
let _guard = match SlotGuard::acquire() {
    Some(g) => g,
    None => return send_response(stream, 429, r#"{"error":"too many concurrent requests"}"#),
};
// _guard dropped at end of scope — slot decremented even on early return or panic
```

### Pattern 4: Content-Length-Aware TCP Read Loop
**What:** Parse Content-Length header from request, then loop `stream.read()` until `content_length` bytes of body received.
**When to use:** HTTP/1.1 over raw TcpStream — single `read()` may return fewer bytes than the Content-Length header indicates.
**Example:**
```rust
fn read_request(stream: &mut TcpStream) -> Result<String, Box<dyn std::error::Error>> {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 4096];

    // Read until we have the full header (ends with \r\n\r\n)
    let header_end = loop {
        let n = stream.read(&mut tmp)?;
        if n == 0 { return Err("connection closed".into()); }
        buf.extend_from_slice(&tmp[..n]);
        if buf.len() > MAX_BODY { return Err("request too large".into()); }
        if let Some(pos) = find_header_end(&buf) { break pos; }
    };

    let header_str = std::str::from_utf8(&buf[..header_end])?;
    let content_length = parse_content_length(header_str).unwrap_or(0);

    let body_start = header_end + 4; // skip \r\n\r\n
    let body_needed = content_length.saturating_sub(buf.len().saturating_sub(body_start));

    // Read remaining body bytes
    let mut remaining = body_needed.min(MAX_BODY);
    while remaining > 0 {
        let n = stream.read(&mut tmp[..remaining.min(4096)])?;
        if n == 0 { break; }
        buf.extend_from_slice(&tmp[..n]);
        remaining = remaining.saturating_sub(n);
    }

    Ok(String::from_utf8_lossy(&buf).into_owned())
}

fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}

fn parse_content_length(headers: &str) -> Option<usize> {
    headers.lines()
        .find(|l| l.to_ascii_lowercase().starts_with("content-length:"))
        .and_then(|l| l.split(':').nth(1))
        .and_then(|v| v.trim().parse().ok())
}
```

### Pattern 5: tracing Init for Lightweight Binaries
**What:** Single-line init in main() before any I/O; plain fmt format; respects RUST_LOG env var.
**When to use:** Stdlib binary that wants structured logging without JSON overhead.
**Example:**
```rust
// In main(), before TcpListener::bind:
tracing_subscriber::fmt::init();
// Reads RUST_LOG env var; defaults to INFO level
// Output: 2026-03-20T06:15:00.123Z  INFO rc_sentry: listening on :8091

tracing::info!("rc-sentry listening on :{port}");
tracing::warn!("rc-sentry: handler error: {e}");
tracing::error!("rc-sentry: bind :{port} failed: {e}");
```

### Anti-Patterns to Avoid
- **`child.wait_with_output()` for timeout:** This blocks forever. Use wait-timeout's `child.wait_timeout(dur)` first, then kill if it returns `None`.
- **Decrement-without-guard:** Manually calling `EXEC_SLOTS.fetch_sub(1)` after every return path — misses panics and early returns. Use Drop guard.
- **`stream.read()` once for the body:** Returns whatever the kernel has buffered — commonly the headers plus zero body bytes for large POSTs. Always loop on Content-Length.
- **Touching rc-agent Cargo.toml in this phase:** SHARED-02 (rc-common tokio feature) is declared now but rc-agent does not switch to using it until Phase 72+. rc-agent's existing direct dep on tokio is unchanged.
- **`eprintln!` left in after tracing init:** Mixing eprintln + tracing is confusing. Do a complete replacement, not a partial one.
- **`wait-timeout` on `child.wait_with_output()`:** The `wait_timeout` method is only on `Child`, not on the output future. Collect the child handle first with `spawn()`, then call `wait_timeout`.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Blocking child timeout on Windows | Thread sleep + `try_wait` loop | `wait-timeout` 0.2 | WinAPI `WaitForSingleObject` semantics not replicated correctly by polling; race conditions |
| Cargo feature gate between sync/async | Separate crates or cfg flags | Cargo `[features]` + `optional = true` | Standard Rust pattern; cargo tree verifies it; no code duplication |
| Concurrency counter | Mutex<usize> with manual lock/unlock | AtomicUsize + compare_exchange + Drop | Lock-free, correct under concurrent thread spawning, no deadlock risk |

**Key insight:** wait-timeout is the sole correct solution for `std::process::Child` timeout on Windows — there is no stdlib alternative.

---

## Common Pitfalls

### Pitfall 1: tokio Contamination via Transitive Dependency
**What goes wrong:** rc-common declares `tokio` as a required (non-optional) dependency. rc-sentry adds rc-common. `cargo tree -p rc-sentry` now shows tokio. rc-sentry binary grows by ~3MB and pulls in the tokio runtime — which is never initialized, wasting memory.
**Why it happens:** Forgetting `optional = true` in rc-common's Cargo.toml, or omitting the `[features]` block.
**How to avoid:** After every Cargo.toml change in rc-common, run `cargo tree -p rc-sentry | grep tokio` and verify empty output. This is the phase's mandatory verification gate.
**Warning signs:** `cargo build --bin rc-sentry` starts taking noticeably longer; binary size increases to >2MB.

### Pitfall 2: wait-timeout API Misuse
**What goes wrong:** Calling `child.wait_timeout()` then immediately calling `child.wait_with_output()` — the latter consumes the child and may hang because kill() hasn't been called.
**Why it happens:** Misreading the wait-timeout API. `wait_timeout` returns `Ok(None)` on timeout — the child is still running.
**How to avoid:** Pattern: `None` branch → `child.kill()` → `child.wait_with_output()`. The kill + wait pattern is required to prevent zombie processes on Windows.
**Warning signs:** Tests hang when deliberately triggering timeout paths.

### Pitfall 3: AtomicUsize Counter Leak on Panic
**What goes wrong:** Handler thread panics inside the exec path after incrementing the counter but before decrementing it. Counter is now permanently one too high. After 4 panics, all exec requests return 429 forever.
**Why it happens:** Manual `fetch_add`/`fetch_sub` without a Drop guard.
**How to avoid:** Use a `SlotGuard` struct with `Drop` impl that calls `fetch_sub(1, Release)`. The guard is created on slot acquisition and dropped at scope end — even on panic.
**Warning signs:** rc-sentry starts rejecting all exec requests with 429 without ever receiving 4 concurrent requests.

### Pitfall 4: Content-Length Parsing with CRLF
**What goes wrong:** `Content-Length: 123\r\n` — parsing the value without stripping `\r` gives `"123\r"` which fails `parse::<usize>()`.
**Why it happens:** HTTP headers use CRLF line endings; Rust's `str::lines()` splits on `\n` but does NOT strip the trailing `\r` on Windows-style line endings.
**How to avoid:** Use `.trim()` on the header value before parsing: `v.trim().parse::<usize>()`.
**Warning signs:** Content-Length parse fails for all POST requests; body read as 0 bytes; cmd field is empty; returns "missing cmd" 400.

### Pitfall 5: Output Truncation After UTF-8 Conversion
**What goes wrong:** `String::from_utf8_lossy(&output.stdout)` produces a String, then `.truncate(64*1024)` panics because truncate may split a multi-byte UTF-8 character.
**Why it happens:** `String::truncate` requires the index to be on a char boundary.
**How to avoid:** Truncate the `Vec<u8>` BEFORE converting to String. The `truncate_output` helper takes `Vec<u8>` args, truncates the raw bytes, then converts.
**Warning signs:** Runtime panics on commands that produce non-ASCII output (e.g., `dir` on paths with Chinese/Cyrillic characters).

### Pitfall 6: Forgetting 429 in send_response Match
**What goes wrong:** `send_response(stream, 429, ...)` is called but the match arm is missing — falls through to `_ => "Error"` reason phrase. HTTP response is `429 Error` instead of `429 Too Many Requests`.
**Why it happens:** send_response uses a match on status code to get the reason phrase; 429 isn't in the existing list.
**How to avoid:** Add `429 => "Too Many Requests"` to the match in `send_response` before wiring the concurrency cap.
**Warning signs:** curl shows `HTTP/1.1 429 Error` — functionally works but incorrect HTTP.

---

## Code Examples

Verified patterns from source files and official crate docs:

### wait-timeout Child Timeout Pattern
```rust
// Source: wait-timeout 0.2.1 crate API
use wait_timeout::ChildExt;
use std::time::Duration;

let status = child.wait_timeout(Duration::from_millis(timeout_ms))?;
match status {
    Some(status) => { /* exited normally */ }
    None => {
        child.kill()?;
        child.wait()?; // reap the zombie
        // return timed_out: true
    }
}
```

### Feature-Gated tokio in rc-common
```toml
# Source: Cargo reference — optional dependencies
[dependencies.tokio]
version = "1"
features = ["full"]
optional = true

[features]
tokio = ["dep:tokio"]  # dep: prefix avoids name collision with feature name
```

```rust
// Source: Cargo conditional compilation pattern
#[cfg(feature = "tokio")]
pub async fn run_cmd_async(cmd: &str, timeout: Duration, max_output: usize) -> ExecResult {
    // tokio::process::Command is only available when feature is active
    use tokio::process::Command;
    use tokio::time::timeout as tokio_timeout;
    // ...
}
```

### tracing Init in Plain Fmt Mode
```rust
// Source: tracing-subscriber 0.3 docs
fn main() {
    tracing_subscriber::fmt::init(); // reads RUST_LOG, writes to stdout
    tracing::info!("rc-sentry listening on :{port}");
    // ...
}
```

### Existing rc-sentry exec structure (lines to replace)
```rust
// CURRENT (line 78) — timeout_ms is parsed but ignored:
let _timeout_ms = parsed["timeout_ms"].as_u64().unwrap_or(30_000);

// CURRENT (lines 86-88) — no timeout, no truncation, no concurrency cap:
let result = Command::new("cmd.exe")
    .args(["/C", cmd])
    .output(); // blocks forever on hung command
```

### Existing rc-agent semaphore reference (verified pattern)
```rust
// Source: crates/rc-agent/src/remote_ops.rs line 426
let _permit = match EXEC_SEMAPHORE.try_acquire() {
    Ok(permit) => permit,
    Err(_) => return Err((StatusCode::TOO_MANY_REQUESTS, Json(...))),
};
// rc-sentry uses AtomicUsize instead of Semaphore (no tokio available)
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `Command::output()` (blocks forever) | `wait-timeout` crate (timed kill) | Phase 71 | Eliminates hung thread accumulation |
| Single `stream.read()` | Content-Length loop read | Phase 71 | Fixes dropped POST bodies on large JSON payloads |
| Unbounded `thread::spawn` | AtomicUsize slot cap (4) | Phase 71 | Prevents resource exhaustion under load |
| `eprintln!` | `tracing::info!/warn!/error!` | Phase 71 | Structured, filterable log output |
| No output limit | 64KB truncation in rc-common | Phase 71 | Matches rc-agent behavior; prevents memory spike |

**Deprecated/outdated:**
- `let _timeout_ms = ...` (line 78 in rc-sentry main.rs): prefixed underscore signals it was never wired — remove the underscore and wire it through rc-common
- Direct `Command::output()` in rc-sentry handle_exec: replaced by `rc_common::exec::run_cmd_sync`

---

## Open Questions

1. **wait-timeout interaction with partially-read stdout on timeout**
   - What we know: `child.kill()` followed by `child.wait_with_output()` will collect whatever was written to the pipe buffer before kill
   - What's unclear: On Windows, is stdout pipe buffer flushed before kill signal is processed?
   - Recommendation: Accept best-effort partial stdout on timeout — include what's available, set `timed_out: true`. This matches rc-agent's behavior.

2. **Thread naming for debuggability**
   - What we know: CONTEXT.md says "Thread name set to `sentry-handler-{N}`"
   - What's unclear: `std::thread::Builder::name()` takes a static string — a dynamic name like `sentry-handler-{N}` requires a global counter for N
   - Recommendation: Use the thread count at spawn time (EXEC_SLOTS.load()) as N, or use the system thread ID. Either is acceptable; defer exact choice to implementer (Claude's discretion).

3. **run_cmd_async signature in Phase 71 vs Phase 72**
   - What we know: SHARED-02 declares the function; rc-agent doesn't switch to it until Phase 72+
   - What's unclear: Should run_cmd_async be implemented and tested in Phase 71, or just declared?
   - Recommendation: Implement the function body in Phase 71 (otherwise SHARED-02 is not done), but leave rc-agent's Cargo.toml unchanged. The tokio feature gate is wired but unused by rc-agent until Phase 72.

---

## Validation Architecture

nyquist_validation = true in config.json — section included.

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in `cargo test` |
| Config file | none (workspace-level cargo test) |
| Quick run command | `cargo test -p rc-common` |
| Full suite command | `cargo test -p rc-common && cargo test -p rc-agent && cargo build --bin rc-sentry && cargo tree -p rc-sentry \| grep tokio` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| SHARED-01 | run_cmd_sync executes cmd, returns ExecResult with stdout/stderr/exit_code | unit | `cargo test -p rc-common -- exec::tests::test_run_cmd_sync_basic` | ❌ Wave 0 |
| SHARED-01 | run_cmd_sync kills process and returns timed_out=true after deadline | unit | `cargo test -p rc-common -- exec::tests::test_run_cmd_sync_timeout` | ❌ Wave 0 |
| SHARED-02 | run_cmd_async is only compiled when "tokio" feature is active | build | `cargo build -p rc-common` (no features) must succeed without tokio | ❌ Wave 0 |
| SHARED-03 | cargo tree -p rc-sentry shows no tokio | build/smoke | `cargo tree -p rc-sentry \| grep -c tokio` must output `0` | ❌ Wave 0 |
| SHARD-01 | Long-running command killed and returns timeout error after timeout_ms | integration | `cargo test -p rc-common -- exec::tests::test_timeout_kills_child` | ❌ Wave 0 |
| SHARD-02 | Command producing >64KB output is truncated; truncated=true | unit | `cargo test -p rc-common -- exec::tests::test_output_truncation` | ❌ Wave 0 |
| SHARD-03 | 5th concurrent exec request rejected with HTTP 429 | manual | Start rc-sentry, send 5 concurrent curl /exec — 5th returns 429 | N/A |
| SHARD-04 | POST with large JSON body received completely | unit | `cargo test -p rc-common -- exec::tests::test_partial_read` (or rc-sentry integration) | ❌ Wave 0 |
| SHARD-05 | tracing output visible on stdout when rc-sentry starts | smoke | Start rc-sentry, observe "INFO rc_sentry" line in output | manual |

### Sampling Rate
- **Per task commit:** `cargo test -p rc-common`
- **Per wave merge:** `cargo test -p rc-common && cargo build --bin rc-sentry && cargo tree -p rc-sentry | grep tokio`
- **Phase gate:** All of the above green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/rc-common/src/exec.rs` — the new module (all exec tests live here)
- [ ] `crates/rc-common/src/exec/tests.rs` or inline `#[cfg(test)]` block — unit tests for run_cmd_sync, timeout, truncation
- [ ] No test framework install needed — cargo test is built-in

---

## Sources

### Primary (HIGH confidence)
- Direct source read: `crates/rc-sentry/src/main.rs` — 155 lines, all bugs visible
- Direct source read: `crates/rc-agent/src/remote_ops.rs` — semaphore pattern, timeout pattern, CREATE_NO_WINDOW
- Direct source read: `crates/rc-common/src/lib.rs` — existing module structure
- Direct source read: `crates/rc-common/Cargo.toml` + `crates/rc-sentry/Cargo.toml` + workspace `Cargo.toml`
- Direct source read: `crates/rc-agent/Cargo.toml` — confirms rc-common used without features today
- Cargo registry: `cargo search wait-timeout` → 0.2.1 confirmed current
- Cargo registry: `cargo search tracing` → 0.1.44 confirmed current
- Cargo registry: `cargo search tracing-subscriber` → 0.3.23 confirmed current

### Secondary (MEDIUM confidence)
- Cargo reference: optional dependency pattern (`dep:tokio`) — standard documented Rust feature
- wait-timeout 0.2 API: `ChildExt::wait_timeout` returning `Option<ExitStatus>` — confirmed via crate description

### Tertiary (LOW confidence)
- Windows stdout pipe flush behavior on kill() — not empirically tested in this codebase; behavior assumed from documentation

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all crates verified against live registry; source files read directly
- Architecture: HIGH — all patterns derived from existing codebase (remote_ops.rs is the reference implementation)
- Pitfalls: HIGH — all pitfalls derived from direct code inspection (line 78 timeout never wired, single read, no guard)

**Research date:** 2026-03-20 IST
**Valid until:** 2026-04-20 (stable crates; wait-timeout 0.2.x is mature and unlikely to change)
