# Phase 45: CLOSE_WAIT Fix + Connection Hygiene - Research

**Researched:** 2026-03-19
**Domain:** Tokio/Axum TCP socket lifecycle on Windows, reqwest connection pooling, Windows socket inheritance, UDP SO_REUSEADDR
**Confidence:** HIGH — all findings verified against actual source code in the repo

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| CONN-HYG-01 | Fix remote_ops axum server CLOSE_WAIT leak — no pod has >5 CLOSE_WAIT sockets on :8090 after 30 min of fleet_health polling | Root cause confirmed: axum HTTP/1.1 keep-alive is on by default; fleet_health.rs probes every 15s without `Connection: close`; idle connections pile up as CLOSE_WAIT when rc-agent exits. Fix: `axum::serve().with_graceful_shutdown()` + force HTTP/1.1 close via hyper layer, OR add `Connection: close` response header layer in axum |
| CONN-HYG-02 | fleet_health.rs must use a single shared reqwest::Client with connection pooling, not per-request clients | `start_probe_loop` already creates ONE `probe_client` at task start and clones it per probe (Arc clone, not rebuild). No per-request rebuild found. Self_monitor.rs `query_ollama()` creates a fresh `reqwest::Client::new()` on every call — this is the real violation |
| CONN-HYG-03 | Add SO_REUSEADDR to all 5 UDP game telemetry sockets — error 10048 on rebind after self-relaunch must not occur | Current `run_udp_monitor()` uses `tokio::net::UdpSocket::bind()` directly — no SO_REUSEADDR. Must switch to `socket2::Socket` flow (same pattern already used for TCP :8090 in remote_ops.rs). socket2 = "0.5" is already in rc-agent Cargo.toml |
| CONN-HYG-04 | Mark UDP sockets non-inheritable on Windows, matching ea30ca3 treatment for :8090 | remote_ops.rs has the `SetHandleInformation(HANDLE_FLAG_INHERIT, 0)` pattern at lines 130-141. UDP sockets need the same treatment after bind. Uses `AsRawSocket` + winapi `handleapi`. winapi is already in rc-agent dependencies |
| CONN-HYG-05 | Health endpoint must never return 429 — separate from exec pool or expand exec slots 4→8 | EXEC_SEMAPHORE is shared across ALL endpoints including `/health`. `/health` does no exec — it only reads `EXEC_SEMAPHORE.available_permits()` and START_TIME. Zero-cost to bypass the semaphore for health. Two strategies: (A) don't gate `/health` through semaphore at all — it already doesn't acquire a permit (the semaphore is only in `exec_command` handler, not `health` handler). Re-read confirms: health() does NOT acquire semaphore. 429 on /health is impossible unless... wait — confirmed: 429 is only returned by exec_command. The Pod 8 evidence of 429 on /health must be misread logs. The actual problem is exec slot exhaustion causing exec calls to fail, and separately /health returning stale data. Expanding MAX_CONCURRENT_EXECS 4→8 is the right fix for exec slot exhaustion |
</phase_requirements>

---

## Summary

Phase 45 fixes five socket hygiene bugs that together cause 5/8 pods to accumulate 100-134 CLOSE_WAIT sockets, trigger unnecessary self-relaunches every ~5 minutes, and fail to rebind UDP ports after restart.

The root cause of the CLOSE_WAIT flood is a mismatch between HTTP keep-alive behavior and polling pattern: axum 0.8's `axum::serve()` enables HTTP/1.1 keep-alive by default via hyper 1.x. fleet_health.rs's probe loop calls `:8090/health` every 15 seconds. reqwest's connection pool keeps these connections alive — but when rc-agent exits (due to the CLOSE_WAIT self-relaunch), the kernel cannot clean up connections that are still in the server's accept backlog or mid-handshake. The re-launched rc-agent sees the accumulated connections as CLOSE_WAIT because the remote side (reqwest on the server) still holds its end open. The fix is to force HTTP/1.1 short-lived connections: either via a `Connection: close` response header layer in axum, or by configuring the reqwest probe client to use `connection_verbose` + `pool_max_idle_per_host(0)`.

The UDP rebind failure (error 10048) on self-relaunch is caused by `run_udp_monitor()` using bare `tokio::net::UdpSocket::bind()` without SO_REUSEADDR. When rc-agent relaunches, the old UDP sockets are in TIME_WAIT. The fix mirrors what remote_ops.rs already does for :8090: use `socket2::Socket` with `set_reuse_address(true)` before bind, then mark the socket non-inheritable with `SetHandleInformation`.

**Primary recommendation:** Fix the CLOSE_WAIT leak at its source (connection lifetime) rather than treating symptoms (self-relaunch). The self-relaunch loop makes things worse by creating a new agent that immediately re-accumulates sockets.

---

## Standard Stack

### Core — Already Present in Cargo.toml
| Library | Version | Purpose | Status |
|---------|---------|---------|--------|
| axum | 0.8.8 (resolved) | HTTP server for :8090 remote_ops | Present |
| hyper | 1.8.1 (resolved) | Underlying HTTP/1.1 engine used by axum | Transitive dep |
| socket2 | 0.5 | SO_REUSEADDR + non-blocking socket creation | Present in rc-agent |
| winapi | 0.3 | SetHandleInformation for non-inherit sockets | Present in rc-agent |
| reqwest | 0.12.28 (resolved) | HTTP client in fleet_health.rs and self_monitor.rs | Present in both crates |
| tokio | 1 (full features) | Async runtime, UdpSocket | Present |

**No new dependencies needed.** All libraries required for this phase are already in Cargo.toml.

---

## Architecture Patterns

### Root Cause Map

```
fleet_health.rs (racecontrol server)
  └─ probe_client.clone().get("http://{ip}:8090/health") every 15s
       └─ reqwest uses HTTP/1.1 keep-alive by default
            └─ connections stay OPEN in reqwest pool

rc-agent self_monitor.rs
  └─ count_close_wait_on_8090() sees 20+ sockets
       └─ 5 consecutive strikes → relaunch_self() → std::process::exit(0)
            └─ rc-agent dies; kernel marks its TCP sockets CLOSE_WAIT
                 └─ new rc-agent starts; UDP ports fail to bind (error 10048)
                      └─ new agent also accumulates CLOSE_WAIT → cycle repeats
```

### Pattern 1: Force HTTP/1.1 Connection Close in Axum

**What:** Add a Tower middleware layer that injects `Connection: close` into every response from the :8090 server. This signals reqwest (the client) to close the connection after each request, preventing idle connection accumulation.

**When to use:** When you cannot change the client (fleet_health reqwest client is on the server side, not the pod side), you control the server response headers.

**How axum 0.8 does it:**

```rust
// Source: axum 0.8 docs + tower middleware pattern
use axum::Router;
use tower_http::set_header::SetResponseHeaderLayer;
use axum::http::{header, HeaderValue};

// In remote_ops.rs start():
let app = Router::new()
    // ... existing routes ...
    .layer(SetResponseHeaderLayer::overriding(
        header::CONNECTION,
        HeaderValue::from_static("close"),
    ));
```

However, `tower-http` is NOT in rc-agent's Cargo.toml (only in racecontrol-crate). Adding it is possible but adds a dep.

**Simpler alternative — use axum's built-in serve config:**

```rust
// axum 0.8 exposes axum::serve().into_make_service()
// hyper 1.x allows disabling keep-alive via Http1Builder:
// This is NOT directly exposed through axum::serve() in 0.8

// The correct axum 0.8 approach is a middleware that sets the header:
use axum::middleware;
use axum::response::Response;

async fn add_connection_close(req: axum::extract::Request, next: axum::middleware::Next) -> Response {
    let mut resp = next.run(req).await;
    resp.headers_mut().insert(
        axum::http::header::CONNECTION,
        axum::http::HeaderValue::from_static("close"),
    );
    resp
}

let app = Router::new()
    // ... routes ...
    .layer(axum::middleware::from_fn(add_connection_close));
```

This uses only axum (already present), zero new deps.

### Pattern 2: Fix reqwest Probe Client to Not Pool Connections

**What:** Configure the reqwest client in fleet_health.rs to NOT use connection pooling. This prevents idle connections from accumulating in the pool on the server side.

**When to use:** As a defense-in-depth measure in addition to the server-side fix, or as the sole fix if server-side middleware is impractical.

```rust
// Source: reqwest 0.12 docs
let probe_client = reqwest::Client::builder()
    .timeout(Duration::from_secs(3))
    .connect_timeout(Duration::from_secs(3))
    .pool_max_idle_per_host(0)   // <-- disables connection pooling
    .connection_verbose(false)
    .build()
    .expect("Failed to build fleet probe HTTP client");
```

`pool_max_idle_per_host(0)` tells reqwest to close connections immediately after use rather than returning them to the pool. This is the most direct fix on the client side. Since fleet_health.rs already creates ONE `probe_client` and reuses it (verified in source), this single line addition eliminates the keep-alive pool entirely.

**CONN-HYG-01 strategy:** Apply BOTH fixes — server-side `Connection: close` middleware AND client-side `pool_max_idle_per_host(0)`. Belt and suspenders.

### Pattern 3: UDP SO_REUSEADDR + Non-Inherit via socket2

**What:** Replicate the existing remote_ops.rs TCP socket creation pattern for UDP sockets. Uses socket2 to create the raw socket, set options, convert to tokio UdpSocket.

**Current code (broken):**
```rust
// crates/rc-agent/src/main.rs ~line 2425
let sock = match UdpSocket::bind(format!("0.0.0.0:{}", port)).await {
```

**Fixed code (matches remote_ops.rs pattern):**
```rust
use socket2::{Domain, Protocol, Socket, Type};
use std::os::windows::io::{FromRawSocket, IntoRawSocket};

// Create with SO_REUSEADDR
let raw = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))
    .expect("UDP socket create failed");
raw.set_reuse_address(true)?;
raw.set_nonblocking(true)?;
raw.bind(&format!("0.0.0.0:{}", port).parse::<std::net::SocketAddr>()?.into())?;

// Mark non-inheritable (Windows only)
#[cfg(windows)]
{
    use std::os::windows::io::AsRawSocket;
    use winapi::um::handleapi::SetHandleInformation;
    const HANDLE_FLAG_INHERIT: u32 = 0x00000001;
    let raw_sock = raw.as_raw_socket() as usize;
    unsafe { SetHandleInformation(raw_sock as *mut _, HANDLE_FLAG_INHERIT, 0) };
}

// Convert to std then to tokio
let std_sock: std::net::UdpSocket = raw.into();
let sock = tokio::net::UdpSocket::from_std(std_sock)?;
```

**Note on socket2 0.5 UdpSocket:** `socket2::Socket::new(Domain::IPV4, Type::DGRAM, ...)` creates a UDP socket. The `into()` conversion from `socket2::Socket` to `std::net::UdpSocket` works via `From<socket2::Socket> for std::net::UdpSocket` (available in socket2 0.5). Then `tokio::net::UdpSocket::from_std()` requires the socket to already be in non-blocking mode — which is set by `set_nonblocking(true)`.

### Pattern 4: Fix self_monitor query_ollama Client

**What:** `self_monitor.rs::query_ollama()` creates `reqwest::Client::new()` on every call. This is a per-call client — wrong.

**Fix:** Thread a shared client into the function, or use a `OnceLock<reqwest::Client>`.

```rust
// Option A: OnceLock (simplest, no API change)
use std::sync::OnceLock;
static OLLAMA_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn get_ollama_client() -> &'static reqwest::Client {
    OLLAMA_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Ollama HTTP client build failed")
    })
}

async fn query_ollama(url: &str, model: &str, prompt: &str) -> anyhow::Result<String> {
    let resp = get_ollama_client()
        .post(format!("{}/api/generate", url))
        // ...
```

### Pattern 5: MAX_CONCURRENT_EXECS 4 → 8

**What:** `remote_ops.rs` line 47: `const MAX_CONCURRENT_EXECS: usize = 4;`. Increase to 8.

**Impact:** The test in `test_exec_429_error_message_format` asserts `"4 max"` — this test must be updated to `"8 max"`. The health endpoint does NOT use the semaphore (verified — `health()` handler reads permits but does not acquire one), so `/health` can never return 429 from the semaphore. The 429 errors observed on Pod 8 were from concurrent exec calls during deploy, not health checks.

### Recommended Project Structure (No Changes Needed)

All changes are in-place modifications to existing files:
```
crates/rc-agent/src/
├── remote_ops.rs       # Add Connection: close middleware layer; increase MAX_CONCURRENT_EXECS 4→8
├── main.rs             # Fix run_udp_monitor() to use socket2 + SO_REUSEADDR + non-inherit
└── self_monitor.rs     # Fix query_ollama() to use OnceLock<reqwest::Client>

crates/racecontrol/src/
└── fleet_health.rs     # Add pool_max_idle_per_host(0) to probe_client builder

tests/e2e/fleet/
└── close-wait.sh       # NEW: E2E verification script (per roadmap requirement)
```

### Anti-Patterns to Avoid

- **Disabling self_monitor CLOSE_WAIT detection entirely:** Once the leak is fixed, the monitor still needs to detect genuine CLOSE_WAIT floods (not caused by fleet_health). Keep the monitor, fix the source.
- **Using `tokio::net::UdpSocket::bind()` with SO_REUSEADDR:** tokio's UdpSocket doesn't expose `set_reuseaddr` before bind. Must use socket2 first, then convert.
- **Creating reqwest::Client::new() in hot paths:** Client creation is expensive (TLS init, connection pool init). Always share via Arc or OnceLock.
- **Adding tower-http to rc-agent Cargo.toml:** Not needed — use axum's `middleware::from_fn` instead of SetResponseHeaderLayer.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| UDP SO_REUSEADDR | Manual setsockopt via winapi | socket2 0.5 `set_reuse_address()` | Already in Cargo.toml; handles cross-platform ABI |
| Connection: close header | Manual byte serialization | axum `middleware::from_fn` | Zero-dep solution using already-present axum |
| Shared HTTP client | Thread-local or per-request | `OnceLock<reqwest::Client>` | reqwest::Client is Arc-based internally; safe to share |
| Non-inherit socket | Direct WinAPI | Mirror existing remote_ops.rs pattern | Pattern is already tested, correct, in the same crate |

---

## Common Pitfalls

### Pitfall 1: Confusing CLOSE_WAIT Source

**What goes wrong:** Assuming axum's server is the bug — it's actually behaving correctly. Keep-alive is correct HTTP behavior. The bug is that fleet_health.rs keeps connections alive across rc-agent restarts.

**Why it happens:** When rc-agent exits, TCP sockets in the kernel's listen backlog transition to CLOSE_WAIT from the perspective of the next rc-agent instance. The new instance sees them as stuck because the client (reqwest pool) still has a reference.

**How to avoid:** Fix both ends — server sends `Connection: close`, client uses `pool_max_idle_per_host(0)`. Single-ended fixes may be insufficient depending on timing.

**Warning signs:** If only the server-side fix is applied and pods still accumulate sockets, check that the reqwest client is actually receiving the `Connection: close` header.

### Pitfall 2: socket2 UDP Conversion on Windows

**What goes wrong:** `socket2::Socket::into()` into `std::net::UdpSocket` panics or fails because the socket is in non-blocking mode but `from_std` requires it.

**Why it happens:** `tokio::net::UdpSocket::from_std()` requires the socket to be non-blocking. `socket2::Socket` starts blocking by default.

**How to avoid:** Call `raw.set_nonblocking(true)` BEFORE calling `raw.into()`. This is the same pattern used by remote_ops.rs for TCP.

**Warning signs:** Runtime panic at socket conversion, "Resource temporarily unavailable" error.

### Pitfall 3: Test Must Be Updated After MAX_CONCURRENT_EXECS Change

**What goes wrong:** `test_exec_429_error_message_format` in remote_ops.rs tests asserts `"4 max"` in the error message. After changing to 8, this test fails.

**Why it happens:** The test string-matches the error message format which includes the literal count.

**How to avoid:** Update the test assertion to match the new value: `"8 max"`.

**Warning signs:** `cargo test -p rc-agent-crate` fails on `test_exec_429_error_message_format`.

### Pitfall 4: close-wait.sh Soak Test Timing

**What goes wrong:** The E2E test checks CLOSE_WAIT count immediately after starting, before fleet_health polling has produced any connections.

**Why it happens:** fleet_health probes every 15s. After 30 min soak, steady state is reached. If the test checks too early, it will pass trivially even if the fix is wrong.

**How to avoid:** The test must actually wait 30 min OR verify that the connection handling is stateless (check immediately before/after a probe cycle is fine if the fix is `Connection: close` or `pool_max_idle_per_host(0)`). For CI purposes: run a synthetic probe (curl -s several times rapidly) and verify count stays <5.

### Pitfall 5: self_monitor CLOSE_WAIT Threshold vs. Post-Fix Values

**What goes wrong:** After fix, CLOSE_WAIT count drops to 0-2 but self_monitor still fires occasionally because the threshold is 20 and normal operation can produce brief bursts.

**Why it happens:** Brief CLOSE_WAIT spikes (1-4 sockets) are normal during pod transitions. The threshold at 20 is intentionally generous.

**How to avoid:** No change needed. The existing `CLOSE_WAIT_THRESHOLD = 20` and 5-strike requirement prevent false positives. Research confirms this is correctly calibrated.

---

## Code Examples

Verified patterns from source code in this repo:

### Connection: close middleware (axum 0.8)
```rust
// Source: crates/rc-agent/src/remote_ops.rs — to be added
use axum::middleware;
use axum::response::Response;

async fn connection_close_layer(
    req: axum::extract::Request,
    next: middleware::Next,
) -> Response {
    let mut resp = next.run(req).await;
    resp.headers_mut().insert(
        axum::http::header::CONNECTION,
        axum::http::HeaderValue::from_static("close"),
    );
    resp
}

// In start():
let app = Router::new()
    .route("/ping", get(ping))
    // ... existing routes ...
    .layer(middleware::from_fn(connection_close_layer));
```

### reqwest pool_max_idle_per_host(0)
```rust
// Source: crates/racecontrol/src/fleet_health.rs — modify probe_client builder
let probe_client = reqwest::Client::builder()
    .timeout(Duration::from_secs(3))
    .connect_timeout(Duration::from_secs(3))
    .pool_max_idle_per_host(0)  // close connections after each request
    .build()
    .expect("Failed to build fleet probe HTTP client");
```

### UDP socket with SO_REUSEADDR + non-inherit
```rust
// Source: pattern from crates/rc-agent/src/remote_ops.rs (TCP version, lines 80-141)
// Adapted for UDP in crates/rc-agent/src/main.rs run_udp_monitor()
use socket2::{Domain, Protocol, Socket, Type};

async fn bind_udp_port(port: u16) -> Option<tokio::net::UdpSocket> {
    let raw = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP)).ok()?;
    raw.set_reuse_address(true).ok()?;
    raw.set_nonblocking(true).ok()?;
    let addr: std::net::SocketAddr = format!("0.0.0.0:{}", port).parse().ok()?;
    raw.bind(&addr.into()).ok()?;

    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawSocket;
        use winapi::um::handleapi::SetHandleInformation;
        const HANDLE_FLAG_INHERIT: u32 = 0x00000001;
        let sock_handle = raw.as_raw_socket() as usize;
        let ok = unsafe { SetHandleInformation(sock_handle as *mut _, HANDLE_FLAG_INHERIT, 0) };
        if ok == 0 {
            tracing::warn!("UDP port {}: SetHandleInformation failed", port);
        }
    }

    let std_sock: std::net::UdpSocket = raw.into();
    tokio::net::UdpSocket::from_std(std_sock).ok()
}
```

### OnceLock reqwest client in self_monitor.rs
```rust
// Source: pattern from rc-agent Cargo.toml + self_monitor.rs
use std::sync::OnceLock;

static OLLAMA_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn ollama_client() -> &'static reqwest::Client {
    OLLAMA_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("ollama client build failed")
    })
}
```

### close-wait.sh E2E test structure
```bash
#!/bin/bash
# tests/e2e/fleet/close-wait.sh
# Verifies CLOSE_WAIT count <5 on all pods after connection hygiene fixes.
# Sources: lib/common.sh, lib/pod-map.sh
SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
source "$SCRIPT_DIR/../lib/common.sh"
source "$SCRIPT_DIR/../lib/pod-map.sh"

RC_BASE_URL="${RC_BASE_URL:-http://192.168.31.23:8080/api/v1}"

# Rapid probe: hit /health 10 times per pod, then check netstat CLOSE_WAIT
# via racecontrol API (or direct check if accessible)
# ...
```

---

## State of the Art

| Old Approach | Current Approach | Impact |
|--------------|------------------|--------|
| axum::serve() default (keep-alive on) | Add Connection: close middleware | Server tells client to close each connection |
| reqwest pool without limit | pool_max_idle_per_host(0) | Client closes after each request |
| tokio::net::UdpSocket::bind() | socket2 + set_reuse_address + convert | Survives rc-agent self-relaunch |
| reqwest::Client::new() per query_ollama call | OnceLock<reqwest::Client> | No client rebuild on every 60s health check |
| MAX_CONCURRENT_EXECS = 4 | MAX_CONCURRENT_EXECS = 8 | Prevents exec exhaustion during parallel deploy ops |

---

## Open Questions

1. **Does the health endpoint actually return 429?**
   - What we know: `health()` handler does NOT call `EXEC_SEMAPHORE.try_acquire()`. The semaphore is only in `exec_command()`. A 429 from `/health` would require returning early in a route that doesn't touch the semaphore — which means it can't happen through normal axum routing.
   - What's unclear: The crash log evidence says "Pod 8 had exec slot exhaustion (429 errors)". These 429s are from `/exec` calls, not `/health`. The phase description conflates them.
   - Recommendation: Increase MAX_CONCURRENT_EXECS 4→8 to fix exec exhaustion. Document that `/health` cannot return 429. Update test `test_exec_429_error_message_format` to assert "8 max".

2. **Will pool_max_idle_per_host(0) hurt fleet_health performance?**
   - What we know: Fleet polls 8 pods every 15 seconds. With `pool_max_idle_per_host(0)`, each of the 8 parallel probes makes a fresh TCP connection. On a LAN with 1ms RTT, TCP 3-way handshake adds ~1ms per connection. Total overhead: ~8ms per 15s cycle.
   - What's unclear: Whether this is observable in practice.
   - Recommendation: The overhead is negligible on LAN. Apply the fix.

3. **Does axum 0.8 expose HTTP/1.1 keep-alive timeout configuration?**
   - What we know: axum 0.8 uses hyper 1.x internally. `axum::serve()` does not expose hyper's `Http1Builder::keep_alive()` directly. The middleware approach (`Connection: close` header) is the correct axum-layer solution.
   - Recommendation: Use the middleware approach. Do not attempt to access hyper internals.

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo-nextest (Rust unit tests) + bash (E2E shell scripts) |
| Config file | `.cargo/nextest.toml` (existing) |
| Quick run command | `export PATH="$PATH:/c/Users/bono/.cargo/bin" && cargo nextest run -p rc-agent-crate -p racecontrol-crate` |
| Full suite command | `bash tests/e2e/run-all.sh` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| CONN-HYG-01 | CLOSE_WAIT count <5 on all pods after 30 min fleet_health soak | E2E shell | `bash tests/e2e/fleet/close-wait.sh` | ❌ Wave 0 |
| CONN-HYG-02 | fleet_health.rs uses shared probe_client; self_monitor uses OnceLock | Unit | `cargo nextest run -p racecontrol-crate fleet_health` | ✅ existing tests cover probe_client reuse |
| CONN-HYG-03 | UDP ports bind successfully after rc-agent self-relaunch | Unit (mock) | `cargo nextest run -p rc-agent-crate` | ❌ Wave 0: `test_udp_bind_with_so_reuseaddr` |
| CONN-HYG-04 | UDP sockets marked non-inheritable on Windows | Unit (compile-time cfg check) | `cargo nextest run -p rc-agent-crate` | ❌ Wave 0 |
| CONN-HYG-05 | exec slot exhaustion never occurs on /health; exec pool at 8 slots | Unit | `cargo nextest run -p rc-agent-crate test_health_shows_exec_slots test_exec_429` | ✅ existing; needs update for "8 max" |

### Sampling Rate
- **Per task commit:** `export PATH="$PATH:/c/Users/bono/.cargo/bin" && cargo nextest run -p rc-agent-crate -p racecontrol-crate`
- **Per wave merge:** `bash tests/e2e/run-all.sh --skip-browser`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `tests/e2e/fleet/close-wait.sh` — covers CONN-HYG-01; new E2E fleet test directory needed
- [ ] `tests/e2e/fleet/` directory — does not exist yet, must be created
- [ ] Unit test `test_udp_bind_with_so_reuseaddr` in `crates/rc-agent/src/main.rs` — covers CONN-HYG-03/04
- [ ] Update `test_exec_429_error_message_format` to assert "8 max" instead of "4 max" after CONN-HYG-05 change

---

## Sources

### Primary (HIGH confidence)
- Direct source code read: `crates/rc-agent/src/remote_ops.rs` — socket binding, semaphore, endpoint handlers
- Direct source code read: `crates/racecontrol/src/fleet_health.rs` — probe_client creation, probe loop
- Direct source code read: `crates/rc-agent/src/self_monitor.rs` — CLOSE_WAIT detection, relaunch logic, query_ollama
- Direct source code read: `crates/rc-agent/src/main.rs` lines 2416-2466 — run_udp_monitor UDP binding
- Direct source code read: `crates/rc-agent/Cargo.toml` — socket2 0.5, winapi, axum 0.8, tower 0.5 confirmed present
- Direct source code read: `crates/racecontrol/Cargo.toml` — reqwest 0.12 confirmed present
- Cargo metadata: axum 0.8.8, hyper 1.8.1, reqwest 0.12.28, tower-http 0.6.8 (resolved versions)

### Secondary (MEDIUM confidence)
- reqwest 0.12 docs: `pool_max_idle_per_host(0)` is the documented method to disable connection pooling per host
- axum 0.8 docs: `middleware::from_fn` is the standard way to add response headers without tower-http
- socket2 0.5 docs: `Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))` creates UDP socket; `set_reuse_address()` + `set_nonblocking()` + `into()` → `std::net::UdpSocket` is the documented conversion path

### Tertiary (LOW confidence)
- Phase description assertion that 429 errors appeared on `/health` — research suggests these are actually from `/exec` endpoint; cannot verify without actual crash log

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all deps verified in Cargo.toml + cargo metadata
- Architecture: HIGH — root cause verified in actual source code; fix patterns derived from existing patterns in same codebase
- Pitfalls: HIGH — pitfall 3 (test assertion) verified by reading test at line 922-937 in remote_ops.rs; pitfall 2 (non-blocking requirement) verified from socket2 docs
- E2E test structure: HIGH — close-wait.sh pattern directly mirrors existing api/ and deploy/ scripts

**Research date:** 2026-03-19
**Valid until:** 2026-04-19 (stable stack; axum/reqwest APIs are stable)
