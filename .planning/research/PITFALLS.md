# Pitfalls Research

**Domain:** Hardening a minimal backup HTTP server, decomposing a 3,400-line async main.rs, extracting shared library patterns, and testing safety-critical HID/hardware code in a 24/7 Rust sim racing fleet management system.
**Researched:** 2026-03-20
**Confidence:** HIGH — based on direct code analysis of rc-sentry/src/main.rs (155 lines), rc-agent/src/main.rs (3,400+ lines), rc-agent/src/remote_ops.rs, rc-agent/src/ffb_controller.rs, rc-agent/src/billing_guard.rs, and rc-common/src/lib.rs. No speculation — every pitfall is grounded in observed code structure.

---

## Critical Pitfalls

### Pitfall 1: rc-sentry Timeout Field Is Parsed But Not Enforced

**What goes wrong:**
The `timeout_ms` field is parsed from the JSON body (`let _timeout_ms = parsed["timeout_ms"].as_u64().unwrap_or(30_000)`) but the variable is prefixed with `_` and never used. `Command::new("cmd.exe").output()` blocks the spawned thread indefinitely. A caller sending a long-running command (e.g., `xcopy` or `robocopy` on a large directory) will block that thread forever.

**Why it happens:**
The original author added the field to the API contract for forward compatibility but did not yet wire up the enforcement. The `_` prefix suppresses the dead variable warning, making it invisible in normal builds.

**How to avoid:**
Replace `Command::output()` with a channel + thread approach: spawn a child process, then wait on a recv-with-timeout. Since rc-sentry is pure stdlib with no async, use `std::sync::mpsc::channel` + `recv_timeout`:
```rust
let (tx, rx) = std::sync::mpsc::channel();
let child = /* spawn cmd.exe */;
std::thread::spawn(move || { tx.send(child.wait_with_output()); });
match rx.recv_timeout(Duration::from_millis(timeout_ms)) {
    Ok(result) => ...
    Err(RecvTimeoutError::Timeout) => { /* kill child, return 124 */ }
}
```
Do NOT introduce `tokio` — it breaks the stdlib-only constraint that makes rc-sentry reliable.

**Warning signs:**
Grep for `_timeout_ms` in rc-sentry/src/main.rs. If still prefixed with `_`, enforcement is missing.

**Phase to address:**
rc-sentry hardening phase (first sentry phase). Must be fixed before adding new endpoints — new endpoints could expose even longer-running commands.

---

### Pitfall 2: rc-sentry Thread-Per-Connection with No Concurrency Limit

**What goes wrong:**
`for stream in listener.incoming().flatten() { std::thread::spawn(move || handle(stream)); }` spawns an unbounded number of OS threads. If a pod is hammered with concurrent requests (e.g., racecontrol's fleet_health poller, a deploy script, and a manual curl all arriving simultaneously), thread count grows without bound. On Windows, each thread consumes ~1MB stack by default. 1,000 concurrent threads = 1GB RAM before any work is done.

**Why it happens:**
Simple thread-per-connection is the obvious starting point for a stdlib HTTP server. The concurrency concern is not visible during normal single-request use.

**How to avoid:**
Add a `Arc<Semaphore>` (stdlib counting semaphore via `std::sync::Mutex<usize>`) before spawning. Reject with HTTP 503 if at limit. A limit of 8 concurrent requests matches rc-agent's `remote_ops.rs` `MAX_CONCURRENT_EXECS = 8` and is sufficient for all realistic fleet operations. Implement using `AtomicUsize::fetch_add` + compare, decrement in thread on exit.

**Warning signs:**
No `AtomicUsize` or `Mutex<usize>` in rc-sentry/src/main.rs. If `thread::spawn` is called unconditionally on every incoming connection, the limit is missing.

**Phase to address:**
rc-sentry hardening phase. Address at the same time as timeout enforcement — both are about preventing a single slow/misbehaving caller from degrading the backup access tool.

---

### Pitfall 3: rc-sentry New Endpoints Exposing Process List or File Contents Externally

**What goes wrong:**
rc-sentry deliberately has no auth ("SECURITY: internal-only tool for LAN management"). Adding `/processes` (which reveals all running PIDs and exe paths) and `/files` (which can read arbitrary file paths) to rc-sentry exposes more attack surface than the current `/exec`. If a pod's Windows Firewall rule is misconfigured or if the venue network is ever bridged to a guest WiFi, these endpoints become a directory traversal and process enumeration tool accessible to any device on the subnet.

**Why it happens:**
The same reasoning used to justify `/exec` ("it's LAN-only, equivalent to SSH") is applied to new endpoints without considering that `/processes` and `/files` return structured data that is more easily scraped by automated tools than raw shell output. The low barrier of "it's all LAN anyway" leads to expanding the attack surface without re-evaluating the threat model.

**How to avoid:**
- `/processes` endpoint: return only the fields racecontrol actually needs (process name, PID, status). Do NOT include full command-line arguments (may contain credentials, tokens, or paths). Apply a same process-name allowlist filter as kiosk.rs uses.
- `/files` endpoint: restrict to a whitelist of directories (`C:\RacingPoint\`, `C:\Users\bono\racingpoint\deploy-staging\` equivalent on pods). Reject path traversal (`..`) at the handler level. Return metadata only by default; body only on explicit `?content=true`.
- `/version` and `/health` are safe — metadata only.
- Document the decision in the endpoint comment with explicit reasoning (follow the pattern in rc-sentry main.rs line 7-10).

**Warning signs:**
New endpoints that accept an arbitrary `path` query parameter without canonicalization + allowlist check. Any endpoint that returns `Command::output()` of `tasklist /v` or `dir /s` (unbounded output).

**Phase to address:**
rc-sentry endpoint expansion phase. Must be reviewed as a security gate before merge, not as an afterthought.

---

### Pitfall 4: rc-agent main.rs Decomposition Breaking the select! Event Loop

**What goes wrong:**
The inner `loop { tokio::select! { ... } }` (starting at line 1087) has 10+ arms that share mutable local variables: `game_process`, `launch_state`, `crash_recovery`, `last_ac_status`, `ac_status_stable_since`, `last_launch_args_stored`, `current_driver_name`, `last_ffb_percent`, `last_ffb_preset`, `session_max_speed_kmh`, `session_race_position`, `blank_timer`, `blank_timer_armed`. These are all `!Send` or tightly coupled. Naively extracting handlers into separate functions causes borrow checker failures because a `select!` arm cannot hold a mutable borrow across an await point when another arm might also need the same variable.

**Why it happens:**
The select! macro is ergonomic for small loops but does not compose into function boundaries cleanly. Extracting a handler like `handle_billing_started(...)` requires passing all the shared state by mutable reference, which violates Rust's borrow rules if other arms also hold borrows. Developers attempt to extract and immediately hit E0502 / E0505 borrow conflicts.

**How to avoid:**
- Extract handlers into free functions that take ALL needed state as explicit mutable parameters (not `&mut self` on a struct containing the whole state — that forces a single large struct). This is verbose but compiles.
- Alternatively: move all per-connection mutable state into a `ConnectionState` struct and pass `&mut ConnectionState` to each extracted handler. The select! loop becomes thin: it calls `conn_state.handle_heartbeat(...)`, `conn_state.handle_billing_started(...)`, etc.
- Do NOT use `Arc<Mutex<T>>` for state that was previously local variables — this introduces lock contention inside an async loop and breaks the single-threaded reasoning that makes the current code correct.
- The outer loop variables (things that persist across reconnections: `reconnect_attempt`, `startup_complete_logged`, `startup_report_sent`, `ws_disconnected_at`) belong in a separate struct from the inner loop variables.
- Keep the panic hook, single-instance guard, and startup sequence in main.rs. Only extract the event loop body.

**Warning signs:**
- Borrow checker errors mentioning "cannot borrow `game_process` as mutable because it is also borrowed as immutable" inside select! arms.
- `Arc<Mutex<T>>` added to variables that were previously plain local variables — this is a red flag that the decomposition is fighting the borrow checker with runtime locks instead of structural fixes.
- Any `unsafe` added to work around borrow issues.

**Phase to address:**
rc-agent decomposition phase. Run `cargo test -p rc-agent` after every extraction step — the test suite catching a regression is better than discovering it on a live pod.

---

### Pitfall 5: rc-common Extraction Breaking racecontrol Compilation

**What goes wrong:**
rc-common is a dependency of both rc-agent AND racecontrol (the server). Adding the extracted exec/HTTP patterns to rc-common means racecontrol must also compile those new modules — and racecontrol may pull in new transitive dependencies that conflict with its existing dependency tree. The most common failure: rc-sentry is stdlib-only but any rc-common code shared with rc-sentry must ALSO be stdlib-only. If a shared utility function accidentally imports `tokio` or `reqwest`, rc-sentry fails to build.

**Why it happens:**
During extraction, a developer copies working code from rc-agent (which has tokio/reqwest) into rc-common without checking what rc-sentry links against. rc-sentry has no async runtime; adding an `async fn` anywhere in rc-common causes rc-sentry's linker to fail or pulls in tokio as a dependency rc-sentry never needed.

**How to avoid:**
- In rc-common/Cargo.toml, use Cargo features to gate async/HTTP utilities: `[features] async = ["tokio", "reqwest"]`. rc-agent enables `rc-common = { features = ["async"] }`. rc-sentry uses `rc-common = {}` (no features).
- Alternatively: keep rc-sentry's shared code (command execution, response formatting) as a `mod` inside rc-sentry itself rather than moving to rc-common. Only move types that are truly dependency-free (pure data structs, serializable enums) into rc-common.
- After every rc-common change, run `cargo build --bin rc-sentry` explicitly — it will not be caught by `cargo test -p rc-common` alone because the test runner uses the rc-common test binary, not the rc-sentry binary.

**Warning signs:**
- rc-common/Cargo.toml gains `tokio` or `reqwest` in `[dependencies]` (not `[dev-dependencies]`).
- `cargo build --bin rc-sentry` starts taking significantly longer (new async runtime being linked).
- Any `async fn` in rc-common without a feature gate.

**Phase to address:**
rc-common extraction phase. Define feature gating BEFORE moving any code — retrofitting features after extraction is more error-prone than designing the boundary upfront.

---

### Pitfall 6: Tests Accidentally Triggering Real HID/FFB Hardware

**What goes wrong:**
`ffb_controller.rs` opens the OpenFFBoard HID device (`VID:0x1209 PID:0xFFB0`) and sends low-level USB HID reports. If a unit test instantiates `FfbController::new(0x1209, 0xFFB0)` and calls any method, it will attempt to open the real wheelbase USB device on the test machine. On James's workstation (RTX 4070, development machine), a Conspit Ares wheelbase is NOT connected — the test will fail with a USB error. On a pod, the same test will silently send a zero-torque command to the real wheelbase during the test run, which is safe but means tests are interacting with production hardware.

**Why it happens:**
The FfbController design is correct for production (lazy device open, non-panicking, retries). But it has no mock/test seam — there is no trait abstraction or dependency injection that would let tests substitute a no-op implementation.

**How to avoid:**
- Introduce a `FfbBackend` trait with `send_report(&self, data: &[u8]) -> Result<()>`. Production code uses `HidBackend` (real hidapi). Tests use `NullBackend` that returns `Ok(())` immediately.
- `FfbController` becomes `FfbController<B: FfbBackend>`. The panic hook uses `FfbController<HidBackend>` — the VID/PID are hardcoded there anyway.
- Alternatively (simpler, no trait refactor): add `#[cfg(test)]` guard in `zero_force_with_retry` and `set_gain` that returns the expected value without opening USB. This is acceptable for a safety-critical binary where the test seam is explicitly documented as test-only.
- The driving_detector.rs HID input path has the same issue — `parse_openffboard_report` is pure data (safe to test), but `run_hid_monitor` opens real USB. Tests of the detector must test only the pure parsing functions, not the monitor loop.

**Warning signs:**
- Test output includes "hidapi: device not found" or "failed to open HID device" — tests are trying to open real hardware.
- Tests pass on pods but fail on James's workstation (or vice versa) based on USB device presence — hardware dependency in test suite.
- Any test file that imports `ffb_controller::FfbController` and calls any method other than `new()`.

**Phase to address:**
Testing phase. The trait/backend abstraction should be designed before writing FFB unit tests — trying to add tests to the current FfbController directly will fail or require unsafe hardware access.

---

### Pitfall 7: rc-agent Panic Hook Using Hardcoded VID/PID That Diverges from Config

**What goes wrong:**
The panic hook (main.rs lines 396-438) hardcodes `let ffb_vid: u16 = 0x1209; let ffb_pid: u16 = 0xFFB0;` before the config is loaded. This is intentional (hook must work even if config fails to load). But if the wheelbase hardware changes (different model, different VID/PID), the panic hook will silently fail to zero the wheelbase because the hook cannot be updated to read from config — it captures the hardcoded values in the closure.

The same panic hook calls `ffb_controller::FfbController::new(ffb_vid, ffb_pid)` and `ffb.zero_force_with_retry(3, 100)` synchronously in the panic context. If this function call itself panics (e.g., hidapi library panic), the `PANIC_HOOK_ACTIVE` guard prevents infinite recursion but the safety zeroing silently fails.

**Why it happens:**
Panic hooks are an inherently constrained environment: no async, no allocator-dependent code after a heap corruption, no access to runtime state. The tradeoff between "always works" (hardcoded) vs "configurable" (reads from config) was made correctly — but the tradeoff is not documented, so future developers may try to "improve" it by reading from config and break the safety guarantee.

**How to avoid:**
- Add a prominent comment in the panic hook: `// INTENTIONALLY HARDCODED: panic hook cannot access config state. If hardware changes, update this constant.`
- Add a startup assertion (after config load) that logs a WARNING if `config.wheelbase.vendor_id != 0x1209 || config.wheelbase.product_id != 0xFFB0`. This makes divergence visible in logs without breaking the hook.
- In tests for the panic hook, test the `PANIC_HOOK_ACTIVE` guard separately from the FFB call to avoid hardware interaction (see Pitfall 6).

**Warning signs:**
- A developer opens a PR that changes the panic hook to read from a `static` or `OnceLock` initialized after config load — this breaks the "works before config loads" guarantee.
- The hardcoded constants differ from `config.wheelbase.vendor_id/product_id` values in rc-agent.toml on any pod.

**Phase to address:**
Testing phase (when writing panic hook tests). Also relevant to decomposition phase — if main.rs is split, the panic hook must remain in main.rs and not be moved to a module that initializes after config load.

---

### Pitfall 8: rc-sentry Output Truncation Not Matching rc-agent Behavior

**What goes wrong:**
rc-agent's `handle_ws_exec` truncates stdout/stderr at 64KB with a `\n... [truncated]` suffix. rc-sentry currently has no truncation — `String::from_utf8_lossy(&output.stdout)` and `String::from_utf8_lossy(&output.stderr)` return the full output. If a command on a pod produces large output (e.g., `dir /s C:\`, `tasklist /v`, log file reads), rc-sentry will return the entire output as a JSON response body, potentially exhausting the HTTP response buffer or producing a multi-megabyte response that the caller does not expect.

**Why it happens:**
Output truncation was added to rc-agent's WS path specifically because WebSocket frames have a practical size limit. The HTTP response path in rc-sentry has no such limit — but the same commands are run via both paths, so the same large outputs can appear.

**How to avoid:**
Implement the same truncation logic as rc-agent: `const MAX_OUTPUT: usize = 64 * 1024;`. Apply before JSON serialization. The constant is already named in rc-agent — define it in rc-common once the extraction phase runs, and reference it from both.

**Warning signs:**
rc-sentry responding with HTTP bodies larger than 64KB in testing. Specifically: `curl -s http://pod:8091/exec -d '{"cmd":"dir /s C:\\"}'` returning more than ~65KB.

**Phase to address:**
rc-sentry hardening phase, same time as timeout enforcement.

---

### Pitfall 9: rc-sentry HTTP Parser Failing on Partial Reads

**What goes wrong:**
rc-sentry reads the request with a single `stream.read(&mut buf)` call. On a real TCP socket, `read()` is not guaranteed to return the entire request in one call — it may return only the HTTP headers, with the body arriving in a subsequent read. For small JSON bodies (typical use), this works in practice because the OS TCP stack delivers the entire request in one segment. For larger bodies or slower clients, the first `read()` returns only headers, the body is empty, `serde_json::from_str("")` returns `Value::Null`, `cmd` is `""`, and the endpoint returns `400 Bad Request`.

**Why it happens:**
Single `read()` on a TCP socket is a well-known footgun in stdlib HTTP parsing. It works 99% of the time in LAN use (small payloads, fast delivery) but fails under load, with large bodies, or from curl on Windows (which sometimes fragments requests). The `MAX_BODY = 64 * 1024` constant suggests the author planned for larger requests but did not implement looped reads.

**How to avoid:**
Implement a `read_http_request(stream)` helper that reads until `\r\n\r\n` is found (headers complete), then reads `Content-Length` more bytes for the body. This is ~30 lines of stdlib code and eliminates the partial-read failure mode. See the existing `request.find("\r\n\r\n")` logic in `handle_exec` — the request parsing already handles both `\r\n\r\n` and `\n\n` but the read loop to fill the buffer does not.

**Warning signs:**
Intermittent `400 Bad Request` from rc-sentry when sending JSON bodies larger than ~1KB. curl with `--data-binary @file` where file > 1KB failing against rc-sentry but succeeding against rc-agent:8090.

**Phase to address:**
rc-sentry hardening phase. Low complexity fix, high reliability payoff.

---

### Pitfall 10: Integration Tests Starting rc-sentry on Port 8091 Colliding with Production

**What goes wrong:**
If integration tests for rc-sentry bind to `0.0.0.0:8091`, they will fail on a pod where the real rc-sentry is already running on that port. They will also fail on James's workstation if a test run was started while monitoring a live pod over SSH tunneling. The test binary exits with "bind failed: address already in use" and the test suite reports all rc-sentry tests as failed — but no test actually ran.

**Why it happens:**
rc-sentry's port selection hardcodes `DEFAULT_PORT: u16 = 8091`. The main binary already supports a port argument (`std::env::args().nth(1).and_then(...)`) but the test harness cannot pass argv to `main()`.

**How to avoid:**
- Factor out `fn run_server(port: u16)` from `fn main()`. Tests call `run_server(0)` (OS-assigned ephemeral port) and retrieve the actual bound port from the `TcpListener` before handing it to the test.
- Alternatively: in test configuration, use port 0 and query `listener.local_addr()?.port()` after bind.
- Never hardcode port 8091 in integration tests — always use port 0 + query.

**Warning signs:**
Integration test file contains `TcpStream::connect("127.0.0.1:8091")` — it is assuming the port rather than receiving it from the server under test.

**Phase to address:**
Testing phase (rc-sentry endpoint tests). The refactor that enables testing (factoring out `run_server`) is also the refactor that enables adding new endpoints cleanly — do both at the same time.

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Keep timeout_ms as `_timeout_ms` (parsed but unused) | API contract exists in JSON without enforcement complexity | Long-running commands block rc-sentry threads forever | Never — fix in hardening phase |
| Copy-paste exec + truncation logic from rc-agent into rc-sentry instead of extracting to rc-common | Faster to ship, no feature-gating complexity | Divergence: rc-sentry truncation threshold may drift from rc-agent's 64KB | Only acceptable as a temporary workaround before rc-common extraction |
| Leave rc-agent main.rs as a single 3,400-line file | No refactor risk, no borrow checker battles | Every feature addition requires understanding the entire file, select! arms grow, merge conflicts are large | Not acceptable beyond v11.0 — decomposition is the explicit goal |
| Use `Arc<Mutex<T>>` for shared event loop state during decomposition | Compiles quickly, avoids borrow checker errors | Lock contention in async hot path, deadlock risk if lock held across await | Never for state that was previously local variables in an async select! loop |
| Skip FfbBackend trait, use `#[cfg(test)]` stubs inline | Faster to write first tests | Tests are fragile and production code has test-only branches | Acceptable for first test pass, must be replaced before adding more FFB tests |

---

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| rc-common used by rc-sentry | Adding `async fn` or `tokio` dependency to rc-common without feature gating | Cargo features: `[features] async = ["tokio"]`, rc-sentry does not enable it |
| rc-common used by racecontrol | Changing a shared type in rc-common without updating racecontrol's deserialization | `cargo build --bin racecontrol` after every rc-common type change, check for serde compatibility |
| hidapi in test environment | Tests calling FfbController methods, failing because no wheelbase on CI/dev machine | FfbBackend trait or `#[cfg(test)]` return-early guard; document test environment requirements |
| rc-sentry HTTP in integration tests | Binding to hardcoded port 8091, colliding with production rc-sentry | Extract `run_server(port: u16)`, tests use port 0 + query bound address |
| billing_guard HTTP client | Tests spawning billing_guard, which makes real HTTP requests to `http://127.0.0.1:8080/api/v1/billing/...` | Mock the HTTP layer with `wiremock` or extract `attempt_orphan_end` as an injectable function; never call production endpoints in unit tests |
| WS exec result channel in tests | Tests for WS exec path require a live `mpsc::Sender<AgentMessage>` — if channel is dropped, sends return `Err` silently | Always keep the `Receiver` alive for the duration of the test; assert that expected messages arrive |

---

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| rc-sentry spawning OS thread per request | Thread count climbs during fleet-wide operations (deploy to 8 pods simultaneously) | Concurrency limit via AtomicUsize semaphore before spawn | At 8+ simultaneous requests from deploy scripts + fleet_health poller |
| rc-sentry no output truncation | Multi-MB HTTP responses from `dir /s` or log reads | Truncate at 64KB before JSON serialization | On first large-output command |
| Tokio accidentally added to rc-sentry | Binary size grows from ~200KB to 4MB+; startup time increases | stdlib-only constraint enforced in Cargo.toml; CI binary-size check | On first `tokio` import |
| rc-agent select! loop with new slow arm | One slow handler (e.g., a new endpoint poll) delays all other arms | Every new select! arm must be either instant or must spawn a task; never block inside select! arm | When poll interval is too short for the work being done |

---

## Security Mistakes

| Mistake | Risk | Prevention |
|---------|------|------------|
| `/processes` returning full command-line args | Exposes credentials or paths from process arguments visible on the pod | Return only `name`, `pid`, `status` — no cmdline args |
| `/files` accepting arbitrary path without canonicalization | Directory traversal: `../../RacingPoint/rc-agent.toml` | `Path::canonicalize()` + prefix check against allowlist of `["C:\\RacingPoint\\", ...]` |
| rc-sentry endpoints added without updating the security comment | Future readers don't know which endpoints are deliberately unauthenticated vs oversight | Update the docblock comment (lines 7-10 of rc-sentry/src/main.rs) for every new endpoint |
| Test binary left bound on port 8091 after test crash | Port remains occupied, blocks next test run and production rc-sentry deploy | Use port 0 in tests; add Drop impl or explicit close in test teardown |

---

## "Looks Done But Isn't" Checklist

- [ ] **rc-sentry timeout enforcement:** Field is parsed (`timeout_ms`) — verify it is actually used to kill the child process, not just read and discarded. Grep for `recv_timeout` or equivalent.
- [ ] **rc-sentry concurrency limit:** Verify `AtomicUsize` or semaphore is decremented on thread exit even when the handler returns an `Err` (check the error path, not just the success path).
- [ ] **rc-common feature gating:** After rc-common extraction, `cargo build --bin rc-sentry` must succeed without any async runtime. Run it explicitly — `cargo test -p rc-common` does not catch this.
- [ ] **Decomposed modules compile independently:** Each new module file must compile with `cargo check -p rc-agent` after extraction. Check that no module imports from main.rs directly.
- [ ] **FFB tests don't touch hardware:** Run `cargo test -p rc-agent` on James's workstation (no wheelbase connected). Any "HID not found" in test output = a test is touching real hardware.
- [ ] **Billing guard tests don't call production HTTP:** Run with `RUST_LOG=debug` and check for any `attempt_orphan_end` log lines during test — means real HTTP calls are happening.
- [ ] **rc-sentry new endpoints return correct Content-Type:** `/health` and `/version` return `application/json`, not `text/plain`. Check with `curl -I`.
- [ ] **select! loop extract leaves panic hook in main.rs:** After decomposition, verify `PANIC_HOOK_ACTIVE` and `PANIC_LOCK_STATE` are still set before any other init. They must not move to a module that initializes after config load.

---

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| rc-sentry thread blocked by timeout-less command | LOW | `sc stop rc-sentry && sc start rc-sentry` on the affected pod via another sentry instance or direct SSH |
| rc-agent select! loop broken by bad decomposition | HIGH | `git revert` the decomposition commit; re-deploy previous rc-agent.exe to affected pods via pendrive (D:\pod-deploy\install.bat) |
| rc-common change breaks racecontrol compile | MEDIUM | `git revert` the rc-common change; racecontrol server stays on previous binary until fix is deployed |
| FFB test triggers real hardware zero during test | LOW | Hardware responds to zero-torque command safely (this is the safety command); no physical damage, but re-run FFB calibration if preset was disturbed |
| rc-sentry port 8091 occupied by test process | LOW | `netstat -ano | findstr :8091` on the pod, `taskkill /PID <pid> /F` to release port |
| Panic hook VID/PID diverges from config | MEDIUM | Update hardcoded constants in main.rs panic hook to match new wheelbase VID/PID, rebuild and deploy |

---

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| timeout_ms not enforced in rc-sentry | rc-sentry hardening | `curl -s http://pod:8091/exec -d '{"cmd":"timeout /t 60","timeout_ms":500}'` returns error within 1s |
| Unbounded threads in rc-sentry | rc-sentry hardening | Concurrent load test: 20 parallel requests returns HTTP 503 for requests > limit |
| New endpoints exposing sensitive data | rc-sentry endpoint expansion | Code review gate: no cmdline args in /processes, path canonicalization in /files |
| select! loop decomposition borrow errors | rc-agent decomposition | `cargo build --bin rc-agent` clean after every extraction step |
| rc-common breaking rc-sentry | rc-common extraction | `cargo build --bin rc-sentry` added to CI after every rc-common change |
| Tests triggering real HID | Testing phase | Test suite passes on James's workstation with no wheelbase connected |
| Panic hook hardcoded VID/PID drift | Testing + decomposition | Startup log includes WARNING if config VID/PID != hook VID/PID |
| Output truncation divergence | rc-sentry hardening | rc-sentry and rc-agent truncate at same threshold (64KB) — use shared constant after extraction |
| Partial TCP read in rc-sentry | rc-sentry hardening | Integration test sends 2KB JSON body fragmented over two writes, verifies correct parse |
| Port 8091 collision in tests | Testing phase | Integration tests bind to port 0 and query bound address — no hardcoded port |

---

## Sources

- Direct code analysis: `crates/rc-sentry/src/main.rs` (full file, 155 lines)
- Direct code analysis: `crates/rc-agent/src/main.rs` (lines 1-1165 reviewed)
- Direct code analysis: `crates/rc-agent/src/ffb_controller.rs` (lines 1-80 reviewed)
- Direct code analysis: `crates/rc-agent/src/remote_ops.rs` (lines 1-80 reviewed)
- Direct code analysis: `crates/rc-agent/src/billing_guard.rs` (lines 1-60 reviewed)
- Direct code analysis: `crates/rc-common/src/lib.rs` (full file)
- Project context: `.planning/PROJECT.md` (v11.0 requirements, constraints, decisions)
- Known in-code pitfall documentation: `main.rs` line 1144 `// Pitfall 1: guard with game_process.is_some()` — confirms the codebase already tracks pitfalls in code comments

---
*Pitfalls research for: rc-sentry hardening, rc-agent decomposition, rc-common extraction, safety-critical testing*
*Researched: 2026-03-20*
