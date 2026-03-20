# Phase 72: rc-sentry Endpoint Expansion + Integration Tests - Research

**Researched:** 2026-03-20
**Domain:** Rust stdlib-only HTTP server, sysinfo, build.rs, Windows graceful shutdown, integration testing without tokio
**Confidence:** HIGH

## Summary

Phase 72 extends the hardened rc-sentry (Phase 71) from a 2-endpoint fallback tool into a complete operations tool with 4 new endpoints (/health, /version, /files, /processes) and integration tests that verify all 6 endpoints via ephemeral TcpStream connections.

The entire implementation stays stdlib-only (no tokio). The reference implementation in rc-agent/src/remote_ops.rs provides exact JSON schemas for every endpoint. The build.rs pattern for GIT_HASH is already in rc-agent and is a direct copy. The sysinfo 0.33 crate is already in the workspace and provides everything needed for /processes (PID, name, memory).

Graceful shutdown on Windows uses SetConsoleCtrlHandler (winapi) for Ctrl+C. SIGTERM does not exist on Windows -- the requirement is satisfied by Ctrl+C handling alone. Integration tests bind to port 0 (OS-assigned ephemeral) and perform raw HTTP via TcpStream -- no test harness framework beyond cargo test and stdlib.

**Primary recommendation:** Copy the exact JSON response shapes from rc-agent/remote_ops.rs, add sysinfo to rc-sentry Cargo.toml, copy build.rs from rc-agent, implement graceful shutdown via a shared AtomicBool + SetConsoleCtrlHandler, and write stdlib-only integration tests that spawn a server thread on port 0.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| SEXP-01 | rc-sentry exposes GET /health returning uptime, version, concurrent exec slots, hostname | rc-agent health endpoint schema confirmed; START_TIME OnceLock pattern direct copy |
| SEXP-02 | rc-sentry exposes GET /version returning binary version and git commit hash | rc-agent build.rs (GIT_HASH) is a direct copy; CARGO_PKG_VERSION is env! macro |
| SEXP-03 | rc-sentry exposes GET /files?path=... returning directory listing | rc-agent list_files() lines 345-376 is the exact reference; stdlib fs::read_dir replaces axum Query |
| SEXP-04 | rc-sentry exposes GET /processes returning running processes with PID, name, memory | sysinfo 0.33 System::processes() confirmed in workspace (rc-agent, racecontrol) |
| SHARD-06 | rc-sentry handles graceful shutdown on SIGTERM/Ctrl+C | Windows: SetConsoleCtrlHandler via winapi; AtomicBool SHUTDOWN_REQUESTED + non-blocking accept |
| TEST-04 | rc-sentry endpoint integration tests covering all 6 endpoints | stdlib TcpStream on port 0; incoming().take(N) pattern; no tokio |
</phase_requirements>

## Standard Stack

### Core (already in rc-sentry Cargo.toml)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| serde_json | 1 | JSON serialization | Already in rc-sentry Cargo.toml |
| tracing / tracing-subscriber | 0.1 / 0.3 | Structured logging | Already in rc-sentry Cargo.toml |
| rc-common | workspace path | run_cmd_sync for /exec | Already wired, tokio-free |

### To Add
| Library | Version | Purpose | Why |
|---------|---------|---------|-----|
| sysinfo | 0.33 | Process enumeration for /processes | Already in workspace (rc-agent, racecontrol); not yet in rc-sentry |
| winapi | 0.3 | SetConsoleCtrlHandler for graceful shutdown | Already used by rc-agent; direct dep needed for rc-sentry |

**Verification:** sysinfo 0.33 confirmed from crates/rc-agent/Cargo.toml line 40 and crates/racecontrol/Cargo.toml line 58.

**Additions to crates/rc-sentry/Cargo.toml:**
```toml
[dependencies]
sysinfo = "0.33"

[target.'cfg(windows)'.dependencies]
winapi = { version = "0.3", features = ["consoleapi"] }
```

**build.rs:** Copy verbatim from crates/rc-agent/build.rs (18 lines: embeds GIT_HASH via cargo:rustc-env, re-runs on .git/HEAD change).

## Architecture Patterns

### Current rc-sentry Request Routing (Phase 71 final state)

The accept loop in main() is currently blocking via listener.incoming(). Phase 72 converts it to non-blocking + poll to support SHARD-06 graceful shutdown.

**New match arms to add in handle()** -- guard form required because query string is part of the raw path string:

```rust
("GET", "/health")                        => handle_health(&mut stream),
("GET", "/version")                       => handle_version(&mut stream),
("GET", p) if p.starts_with("/files")     => handle_files(&mut stream, p),
("GET", "/processes")                     => handle_processes(&mut stream),
```

### Pattern 1: /health Endpoint

**What:** JSON with status, version, build_id, uptime_secs, exec_slots_available, exec_slots_total, hostname
**Reference:** rc-agent remote_ops.rs health() lines 276-287

New statics and constants at top of main.rs:

```rust
const VERSION: &str = env!("CARGO_PKG_VERSION");
const BUILD_ID: &str = env!("GIT_HASH");
static START_TIME: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
```

Initialize in main() as very first line: `START_TIME.get_or_init(std::time::Instant::now);`

exec_slots_available = MAX_EXEC_SLOTS - EXEC_SLOTS.load(Ordering::Acquire)

hostname: `sysinfo::System::host_name().unwrap_or_default()` (already imported for /processes)

### Pattern 2: /version Endpoint

**What:** Binary version and git hash as JSON
**Reference:** rc-agent remote_ops.rs lines 46-47 + build.rs (18-line direct copy)

```rust
fn handle_version(stream: &mut TcpStream) -> Result<(), Box<dyn std::error::Error>> {
    let resp = serde_json::json!({ "version": VERSION, "git_hash": BUILD_ID });
    send_response(stream, 200, &resp.to_string())
}
```

### Pattern 3: /files?path=... Endpoint

**What:** JSON array of FileEntry {name, is_dir, size, modified (Unix secs)}
**Reference:** rc-agent list_files() lines 345-376; FileEntry struct lines 338-343

Query string parsing from raw path string (no axum Query extractor available):

```rust
let query = path_with_query.splitn(2, '?').nth(1).unwrap_or("");
let raw_path = query
    .split('&')
    .find(|p| p.starts_with("path="))
    .and_then(|p| p.strip_prefix("path="))
    .unwrap_or("");
// Targeted percent-decode sufficient for Windows paths:
let decoded = raw_path
    .replace("%3A", ":").replace("%3a", ":")
    .replace("%5C", "\\").replace("%5c", "\\")
    .replace("%2F", "/").replace("%2f", "/")
    .replace("%20", " ");
let path = std::path::PathBuf::from(&decoded);
```

Note: The double backslash above becomes a single backslash in the actual string literal -- standard Rust string escaping.

Error responses (match rc-agent pattern):
- path not found -> 404 with JSON error
- path exists but is not a directory -> 400
- cannot read directory (permissions) -> 403

### Pattern 4: /processes Endpoint

**What:** JSON array; each entry has pid (u32), name (string), memory_kb (u64)
**Reference:** sysinfo 0.33 System::processes() -- same version as rc-agent (Cargo.toml line 40)

```rust
fn handle_processes(stream: &mut TcpStream) -> Result<(), Box<dyn std::error::Error>> {
    let mut sys = sysinfo::System::new_all();
    sys.refresh_all();
    let procs: Vec<serde_json::Value> = sys
        .processes()
        .values()
        .map(|p| serde_json::json!({
            "pid": p.pid().as_u32(),
            "name": p.name().to_string_lossy(),
            "memory_kb": p.memory() / 1024,
        }))
        .collect();
    send_response(stream, 200, &serde_json::to_string(&procs)?)
}
```

Performance: System::new_all() + refresh_all() takes ~50-200ms on Windows. Acceptable for an ops tool. Do not cache.

### Pattern 5: Graceful Shutdown (SHARD-06)

**What:** On Ctrl+C, set shutdown flag, stop accepting new connections, join active handler threads.

Key points:
- SIGTERM does not exist on Windows. SetConsoleCtrlHandler handles CTRL_C_EVENT (0) and CTRL_CLOSE_EVENT (2).
- The existing blocking incoming() loop becomes non-blocking + poll.
- JoinHandle Vec with retain(|h| !h.is_finished()) prune prevents unbounded growth.
- Drain = stop accepting; active handlers finish within existing 30s stream timeout.

New statics and signal handler:

```rust
static SHUTDOWN_REQUESTED: AtomicBool = AtomicBool::new(false);

#[cfg(windows)]
unsafe extern "system" fn ctrl_handler(ctrl_type: u32) -> i32 {
    if ctrl_type == 0 || ctrl_type == 2 {  // CTRL_C_EVENT or CTRL_CLOSE_EVENT
        SHUTDOWN_REQUESTED.store(true, Ordering::Release);
        1
    } else {
        0
    }
}
```

Replacement for the accept loop in main():

```rust
#[cfg(windows)]
unsafe { winapi::um::consoleapi::SetConsoleCtrlHandler(Some(ctrl_handler), 1); }
listener.set_nonblocking(true).expect("set_nonblocking");

let mut handles: Vec<std::thread::JoinHandle<()>> = Vec::new();
loop {
    handles.retain(|h| !h.is_finished());
    if SHUTDOWN_REQUESTED.load(Ordering::Acquire) {
        tracing::info!("shutdown requested -- draining {} active connections", handles.len());
        break;
    }
    match listener.accept() {
        Ok((stream, _)) => {
            let n = THREAD_COUNTER.fetch_add(1, Ordering::Relaxed);
            if let Ok(h) = thread::Builder::new()
                .name(format!("sentry-handler-{n}"))
                .spawn(move || { if let Err(e) = handle(stream) { tracing::warn!("{e}"); } })
            {
                handles.push(h);
            }
        }
        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
            thread::sleep(Duration::from_millis(10));
        }
        Err(e) => tracing::error!("accept: {e}"),
    }
}
for h in handles { let _ = h.join(); }
tracing::info!("rc-sentry shutdown complete");
```

### Pattern 6: Integration Tests

**What:** Inline #[cfg(test)] module in main.rs. Each test gets its own ephemeral port via bind("127.0.0.1:0"). Server thread serves exactly N requests via incoming().take(N).

Key test helpers:

```rust
fn start_test_server(requests: usize) -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    START_TIME.get_or_init(std::time::Instant::now);
    std::thread::spawn(move || {
        for stream in listener.incoming().take(requests).flatten() {
            let _ = handle(stream);
        }
    });
    std::thread::sleep(std::time::Duration::from_millis(50));
    port
}

fn http_get(port: u16, path: &str) -> String {
    use std::io::{Read, Write};
    let mut s = std::net::TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
    s.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
    write!(s, "GET {} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n", path).unwrap();
    let mut resp = String::new();
    s.read_to_string(&mut resp).unwrap();
    resp
}

fn http_post(port: u16, path: &str, body: &str) -> String {
    use std::io::{Read, Write};
    let mut s = std::net::TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
    s.set_read_timeout(Some(std::time::Duration::from_secs(10))).unwrap();
    write!(s,
        "POST {} HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        path, body.len(), body
    ).unwrap();
    let mut resp = String::new();
    s.read_to_string(&mut resp).unwrap();
    resp
}
```

The 7 test functions:
- `test_ping` -- asserts response contains "pong"
- `test_health_fields` -- asserts response contains "status", "uptime_secs", "exec_slots_available", "hostname"
- `test_version_fields` -- asserts response contains "version", "git_hash"
- `test_files_directory` -- GET /files?path=C%3A%5C, asserts not HTTP 500
- `test_processes_fields` -- asserts HTTP 200, contains "pid", "name", "memory_kb"
- `test_exec_echo` -- POST /exec with cmd "echo hello", asserts stdout contains "hello" and exit_code 0
- `test_404_unknown_path` -- asserts HTTP 404

### Anti-Patterns to Avoid
- **Blocking incoming() in tests without take(N):** Background thread never exits; zombie threads accumulate.
- **Shared port across tests:** cargo test runs tests in parallel; port conflicts cause flaky failures.
- **System::new_all() cached between calls:** Always instantiate fresh per /processes request.
- **Closing TcpListener from signal handler:** Undefined behavior on Windows; use AtomicBool + non-blocking poll.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Process enumeration | Custom WMI/WMIC or tasklist parsing | sysinfo 0.33 System::processes() | Cross-platform, handles WOW64, unicode names, already in workspace |
| Git hash at build time | Runtime git subprocess in main() | build.rs cargo:rustc-env=GIT_HASH | Zero runtime cost, CI-safe, fallback to "dev" |
| Ctrl+C interception | Polling or signal-poll loop | SetConsoleCtrlHandler (winapi) | Correct OS signal path; polling burns CPU, may miss close events |
| URL percent-decode | Full RFC 3986 decoder | Targeted replace: %3A, %5C, %2F, %20 | Full decoder adds deps; these 4 cover all Windows path chars in practice |

## Common Pitfalls

### Pitfall 1: tokio Contamination via sysinfo
**What goes wrong:** sysinfo 0.33 has an optional async feature. Cargo feature unification could activate tokio if another workspace member requests it.
**Why it happens:** Workspace-level feature unification merges all feature requests for a given dep.
**How to avoid:** Add `sysinfo = "0.33"` without extra features. After adding: `cargo tree -p rc-sentry | grep tokio` must output nothing.
**Warning signs:** Any tokio output from that tree command after adding sysinfo.

### Pitfall 2: Query String Not Matched by Exact Path
**What goes wrong:** `("GET", "/files")` never fires for `/files?path=...` because path contains query string.
**Why it happens:** read_request() returns the full path including query string verbatim from the HTTP request line.
**How to avoid:** Match guard: `("GET", p) if p.starts_with("/files") => handle_files(&mut stream, p)`

### Pitfall 3: exec_slots_available Calculation
**What goes wrong:** rc-agent uses Semaphore with available_permits(). rc-sentry EXEC_SLOTS counts in-use slots (0..=4), not available permits.
**How to avoid:** `exec_slots_available = MAX_EXEC_SLOTS - EXEC_SLOTS.load(Ordering::Acquire)`

### Pitfall 4: START_TIME Not Initialized Before /health
**What goes wrong:** OnceLock returns None; uptime_secs = 0 even after hours of uptime.
**How to avoid:** `START_TIME.get_or_init(std::time::Instant::now);` as the very FIRST line of main().

### Pitfall 5: JoinHandle Vec Grows Without Bound
**What goes wrong:** Over days, handles Vec grows one entry per request.
**How to avoid:** `handles.retain(|h| !h.is_finished());` at top of each accept loop iteration. JoinHandle::is_finished() stable since Rust 1.61 (project: 1.93.1).

### Pitfall 6: sysinfo process name is OsStr
**What goes wrong:** Calling .to_string() may produce wrong output or panic on non-UTF-8 process names.
**How to avoid:** Always use .to_string_lossy() -- confirmed pattern from rc-agent remote_ops.rs lines 293-296.

### Pitfall 7: set_nonblocking Called Before Bind
**What goes wrong:** set_nonblocking must be called after bind() but before the accept loop.
**How to avoid:** Call listener.set_nonblocking(true) immediately after bind succeeds, before the loop.

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Blocking incoming() accept loop | Non-blocking poll + AtomicBool shutdown flag | Phase 72 | Enables graceful shutdown (SHARD-06) |
| No START_TIME static | OnceLock<Instant> initialized in main() | Phase 72 | /health uptime_secs accurate |
| 2 endpoints: /ping, /exec | 6 endpoints: /ping, /exec, /health, /version, /files, /processes | Phase 72 | Complete fallback ops tool |
| No build.rs | build.rs embeds GIT_HASH from git rev-parse | Phase 72 | /version git_hash field |
| No tests | 7 integration tests via ephemeral TcpStream on port 0 | Phase 72 | TEST-04 coverage |

**Deprecated/outdated:**
- Blocking `for stream in listener.incoming()` loop: replaced in Phase 72 with non-blocking + SHUTDOWN_REQUESTED poll.

## Open Questions

1. **winapi version in rc-sentry**
   - What we know: rc-agent uses winapi (SetHandleInformation) but rc-agent/Cargo.toml was not fully read in this session.
   - What's unclear: Exact winapi version in rc-agent/Cargo.toml.
   - Recommendation: Read rc-agent/Cargo.toml in Plan 1 before writing the dep. Version 0.3 is de-facto standard.

2. **handle() visibility for test module**
   - What we know: handle() is currently private. Rust inline test modules can access private items.
   - Recommendation: Keep handle() private. Tests in the #[cfg(test)] module at the bottom of main.rs can call it directly.

3. **sysinfo parallelism in tests**
   - What we know: Each test gets a separate port; each /processes call creates its own System instance on its own thread.
   - Recommendation: No shared state -- tests safe to run in parallel.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (built-in, no external harness) |
| Config file | none -- inline #[cfg(test)] module in crates/rc-sentry/src/main.rs |
| Quick run command | `cargo test -p rc-sentry` |
| Full suite command | `cargo test -p rc-sentry && cargo tree -p rc-sentry \| grep tokio` |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| SEXP-01 | /health returns status, uptime_secs, exec_slots_available, hostname | integration | `cargo test -p rc-sentry test_health_fields` | Wave 0 |
| SEXP-02 | /version returns version + git_hash | integration | `cargo test -p rc-sentry test_version_fields` | Wave 0 |
| SEXP-03 | /files?path=C%3A%5C returns 200 (no 500) | integration | `cargo test -p rc-sentry test_files_directory` | Wave 0 |
| SEXP-04 | /processes returns array with pid, name, memory_kb | integration | `cargo test -p rc-sentry test_processes_fields` | Wave 0 |
| SHARD-06 | Ctrl+C triggers SHUTDOWN_REQUESTED, accept loop exits | manual-only | N/A -- OS signal not injectable from unit tests | N/A |
| TEST-04 | All 6 endpoints covered by 7 tests | integration | `cargo test -p rc-sentry` | Wave 0 |

**SHARD-06 manual verification:** Build release binary. Run `rc-sentry.exe 9191`. Press Ctrl+C. Confirm "shutdown requested" log line and clean process exit.

### Sampling Rate
- **Per task commit:** `cargo test -p rc-sentry`
- **Per wave merge:** `cargo test -p rc-sentry && cargo tree -p rc-sentry | grep tokio`
- **Phase gate:** All 7 tests green + manual Ctrl+C verification + zero tokio in cargo tree

### Wave 0 Gaps
- [ ] `crates/rc-sentry/build.rs` -- copy from rc-agent/build.rs verbatim (required for GIT_HASH / SEXP-02)
- [ ] `crates/rc-sentry/Cargo.toml`: add sysinfo = "0.33" and winapi target dep with consoleapi feature
- [ ] Integration test module in crates/rc-sentry/src/main.rs: 7 test functions
- [ ] START_TIME OnceLock static + VERSION/BUILD_ID constants in main.rs
- [ ] SHUTDOWN_REQUESTED AtomicBool static + ctrl_handler function in main.rs

## Sources

### Primary (HIGH confidence)
- `crates/rc-agent/src/remote_ops.rs` lines 276-376 -- exact JSON schemas for /health and /files; FileEntry struct; sysinfo::System usage; START_TIME OnceLock pattern
- `crates/rc-agent/build.rs` all 18 lines -- GIT_HASH embed pattern, direct copy template
- `crates/rc-sentry/src/main.rs` full 253-line file -- Phase 71 final state
- `crates/rc-common/src/exec.rs` full file -- run_cmd_sync signature, ExecResult fields
- `crates/rc-sentry/Cargo.toml` -- current deps confirmed; sysinfo and winapi absent
- `Cargo.toml` (workspace) -- sysinfo NOT in workspace.dependencies; must be added directly to rc-sentry
- `crates/rc-agent/Cargo.toml` line 40 + `crates/racecontrol/Cargo.toml` line 58 -- sysinfo = "0.33" confirmed
- `.planning/REQUIREMENTS.md` -- authoritative definitions for SEXP-01..04, SHARD-06, TEST-04
- `.planning/STATE.md` -- constraint: rc-sentry MUST stay stdlib-only, never add tokio

### Secondary (MEDIUM confidence)
- Windows SetConsoleCtrlHandler API: standard Win32 pattern; winapi 0.3 provides Rust bindings; consistent with rc-agent existing winapi usage

### Tertiary (LOW confidence)
- None -- all findings directly verified from project source files

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all deps and versions verified from actual Cargo.toml files in this repo
- Architecture: HIGH -- endpoint schemas copied directly from working rc-agent implementation
- Pitfalls: HIGH -- tokio contamination, query string matching, OnceLock init order all verified by reading actual source
- Integration test pattern: HIGH -- stdlib TcpStream + port 0 + take(N) is an established Rust stdlib testing idiom

**Research date:** 2026-03-20 IST
**Valid until:** 2026-04-20 (sysinfo 0.33 and winapi 0.3 are stable; stdlib patterns do not change)
