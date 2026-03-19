# Phase 46: Crash Safety + Panic Hook - Research

**Researched:** 2026-03-19
**Domain:** Rust panic handling, FFB safety, port binding verification, WebSocket startup protocol
**Confidence:** HIGH

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| SAFETY-01 | Custom panic hook: zero FFB + show error lock screen + log crash + clean exit | `std::panic::set_hook()` is stable Rust; must be sync; FFB zero via existing `FfbController::zero_force()`; lock screen via state mutation + existing HTTP polling |
| SAFETY-02 | Port bind failure detection: if :18923 or :8090 already in use, log clear error + exit within 5s | lock_screen.rs silently returns on bind fail; remote_ops.rs retries 10x (30s) then silently returns — both must surface errors to main.rs and cause exit |
| SAFETY-03 | FFB zero retry: 3 attempts at 100ms intervals, log final result | Current zero_force() makes one attempt; retry wrapper needed around it |
| SAFETY-04 | BootVerification message: WS connected + lock screen port bound + remote ops port bound + HID status + UDP port status, received within 30s of startup | Extend existing `AgentMessage::StartupReport` with new fields OR add new `AgentMessage::BootVerification` variant |
| SAFETY-05 | `cargo test -p rc-agent-crate` passes with all new safety tests green | Test infra already exists; panic hook tests need Windows-specific workarounds |
</phase_requirements>

---

## Summary

Phase 46 adds five defensive layers to rc-agent to ensure pods are never left in a hazardous state after a crash. The Conspit Ares 8Nm wheelbase is a physical hazard when FFB is engaged with no game controlling it — an unhandled Rust panic currently exits silently with the wheelbase retaining whatever torque the game last commanded.

The work splits cleanly into four implementation areas: (1) panic hook installation at process entry, (2) port binding failure propagation from spawned tasks back to main, (3) retry wrapper around FFB zero, and (4) an extended startup message sent over WebSocket once all subsystems initialize.

The existing codebase already has most plumbing needed. `FfbController::zero_force()` is non-panicking and works. `LockScreenManager::start_server()` spawns a task that calls `serve_lock_screen()` which silently returns on bind failure — but main.rs cannot observe that failure today. `remote_ops::start()` is fire-and-forget: it spawns a task, retries 10x over 30s, then silently gives up. The lock screen `wait_for_self_ready()` polls for 5s and warns if the port never responds — this warning is already a weak signal of a bind failure, but no action is taken. The plan is to harden these paths so failures are observable and cause clean exit.

**Primary recommendation:** Install the panic hook first (SAFETY-01, highest risk item), then wire port-bind result channels to main.rs, then add FFB retry, then extend the StartupReport to become BootVerification.

---

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `std::panic` | stdlib | Panic hook installation | Zero-dependency; `set_hook()` is stable since Rust 1.10 |
| `std::sync::atomic` | stdlib | Panic hook state flags | Hook closure must be Send + Sync; atomics are the correct sync primitive |
| `tokio::sync::oneshot` | tokio | Port-bind result signaling | oneshot::channel is the standard pattern for task→caller "did you succeed?" signals |
| `std::process` | stdlib | Clean exit from hook | `std::process::exit(1)` is the only correct exit from a panic hook — normal stack unwinding is unsafe inside a hook |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `tracing` | already in Cargo.toml | Logging inside panic hook | Use carefully — async-signal-safe concerns; tracing is safe to call from sync context |
| `hidapi` | already in Cargo.toml | FFB zero inside panic hook | Already used by FfbController; safe to call from hook if initialized |

**No new dependencies required for this phase.** All needed primitives are in stdlib or already in Cargo.toml.

---

## Architecture Patterns

### Recommended Project Structure

No new files needed. Changes are localized to:
```
crates/rc-agent/src/
├── main.rs            — panic hook installation (top of main()), port-bind channels, BootVerification send
├── ffb_controller.rs  — zero_force_with_retry() new method
├── lock_screen.rs     — start_server_checked() returning Result, or bind result via oneshot
├── remote_ops.rs      — start() already returns (), needs to signal bind result back
└── (protocol changes in rc-common/src/protocol.rs — extend StartupReport or new variant)
```

### Pattern 1: Panic Hook Installation

**What:** `std::panic::set_hook()` registers a closure that runs synchronously when any thread panics, before the stack unwinds. The closure MUST be `Send + Sync + 'static` and should do minimal work: zero FFB, write a log line, call `std::process::exit(1)`.

**When to use:** Install as early as possible in `main()` — before any other initialization — so even config-load panics are caught.

**Critical constraints for panic hooks:**
- Cannot call async code (`await` is forbidden — no tokio runtime available in hook)
- Cannot allocate heap memory safely (allocator may be in inconsistent state on OOM panics)
- Must use pre-allocated state or stack buffers where possible
- `std::process::exit()` is safe to call; `std::process::abort()` is an alternative that skips destructors
- Tracing macros that call `eprintln!` or write to file via `OpenOptions` are safe (no allocator needed for fixed strings, but format strings allocate — use pre-composed error strings)

**Example:**
```rust
// Source: Rust std docs https://doc.rust-lang.org/std/panic/fn.set_hook.html
// Install BEFORE any other init in main()

use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

// Pre-shared state for hook closure (set_hook takes 'static closure)
static PANIC_OCCURRED: AtomicBool = AtomicBool::new(false);

// Capture config values before hook installation (hook can't read config)
let ffb_vid: u16 = 0x1209;  // read from config later, but use defaults here
let ffb_pid: u16 = 0xFFB0;

std::panic::set_hook(Box::new(move |info| {
    // Guard: only run once if somehow called recursively
    if PANIC_OCCURRED.swap(true, Ordering::SeqCst) {
        return;
    }

    // 1. Log crash info using eprintln! (safe, no allocator)
    eprintln!("[PANIC] rc-agent panic: {:?}", info);

    // 2. Append to rc-bot-events.log (sync file write, safe)
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(r"C:\RacingPoint\rc-bot-events.log")
        .and_then(|mut f| {
            use std::io::Write;
            writeln!(f, "[PANIC] rc-agent crashed: {:?}", info)
        });

    // 3. Zero FFB (sync HID write — safe to call in hook)
    let ffb = FfbController::new(ffb_vid, ffb_pid);
    for attempt in 1..=3 {
        match ffb.zero_force() {
            Ok(_) => break,
            Err(e) => eprintln!("[PANIC] FFB zero attempt {} failed: {}", attempt, e),
        }
    }

    // 4. Update lock screen state to show error (mutex write — generally safe)
    // Note: if the panic originated in a mutex-holding thread, this may deadlock.
    // Use try_lock() instead of lock() to avoid deadlock risk.
    if let Some(lock_state) = PANIC_LOCK_STATE.get() {
        if let Ok(mut s) = lock_state.try_lock() {
            *s = LockScreenState::ConfigError {
                message: "System Error — Please Contact Staff".to_string(),
            };
        }
    }

    // 5. Exit cleanly (no unwinding — we're in a hook)
    std::process::exit(1);
}));
```

**Sharing state with the hook:** The hook closure is `'static`, so it cannot borrow from `main()`. Use one of:
- `static OnceLock<Arc<Mutex<LockScreenState>>>` — set after LockScreenManager is created
- `static AtomicBool` — for simple flags
- Pre-captured values in the closure (VID/PID for FFB)

### Pattern 2: Port-Bind Result Signaling via oneshot

**What:** Convert fire-and-forget task spawns into observable operations by sending bind result back to `main()` via `tokio::sync::oneshot`.

**Current problem:**
- `lock_screen.rs`: `start_server()` spawns a task → `serve_lock_screen()` returns on bind fail → main never knows
- `remote_ops.rs`: `start()` spawns a task → retries 10x then returns → main never knows

**Pattern:**
```rust
// In lock_screen.rs: new method that signals bind result
pub fn start_server_checked(&self) -> tokio::sync::oneshot::Receiver<Result<(), String>> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let state = self.state.clone();
    let port = self.port;
    // ... other clones
    tokio::spawn(async move {
        // Attempt bind
        let addr: std::net::SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();
        let socket = tokio::net::TcpSocket::new_v4().unwrap();
        let _ = socket.set_reuseaddr(true);
        match socket.bind(addr) {
            Ok(()) => {
                let _ = tx.send(Ok(())); // Signal success before starting server
                // ... continue serving
            }
            Err(e) => {
                let _ = tx.send(Err(format!("port {} bind failed: {}", port, e)));
                return; // Stop here
            }
        }
        // ... rest of serve loop
    });
    rx
}

// In main.rs:
let lock_rx = lock_screen.start_server_checked();
// ... later, after all starts:
match lock_rx.await {
    Ok(Ok(())) => tracing::info!("Lock screen server bound"),
    Ok(Err(e)) => {
        tracing::error!("FATAL: {}", e);
        std::process::exit(1);
    }
    Err(_) => tracing::warn!("Lock screen bind result channel dropped"),
}
```

**Timing note:** The current code already calls `lock_screen.wait_for_self_ready()` which polls for 5s. If we add a oneshot channel, we can skip the poll and directly await the channel result — cleaner and faster.

### Pattern 3: FFB Zero with Retry

**What:** Wrap `zero_force()` in a retry loop. The current zero_force() is already non-panicking. We need 3 attempts at 100ms intervals.

```rust
// In ffb_controller.rs — new public method
pub fn zero_force_with_retry(&self, attempts: u8, delay_ms: u64) -> bool {
    for attempt in 1..=attempts {
        match self.zero_force() {
            Ok(true) => {
                tracing::info!("FFB zero succeeded on attempt {}", attempt);
                return true;
            }
            Ok(false) => {
                // Device not found — not a retry-able condition
                tracing::debug!("FFB zero: device not found");
                return false;
            }
            Err(e) => {
                tracing::warn!("FFB zero attempt {}/{} failed: {}", attempt, attempts, e);
                if attempt < attempts {
                    std::thread::sleep(std::time::Duration::from_millis(delay_ms));
                }
            }
        }
    }
    tracing::error!("FFB zero failed after {} attempts", attempts);
    false
}
```

**Usage:** Replace bare `ffb.zero_force()` calls in startup (main.rs lines 451-457 and 571-578) with `zero_force_with_retry(3, 100)`. The panic hook also uses this (sync-safe).

### Pattern 4: BootVerification Message

**What:** After all subsystems initialize successfully (WS connected, ports bound, HID detected), send a single message to the server with full health status. This tells the server "this pod is properly up".

**Option A — Extend StartupReport:** Add new fields to `AgentMessage::StartupReport`:
```rust
StartupReport {
    pod_id: String,
    version: String,
    uptime_secs: u64,
    config_hash: String,
    crash_recovery: bool,
    repairs: Vec<String>,
    // NEW for Phase 46:
    lock_screen_port_bound: bool,
    remote_ops_port_bound: bool,
    hid_detected: bool,
    udp_ports_bound: Vec<u16>,
}
```

**Option B — New AgentMessage::BootVerification variant:** Cleaner separation of concerns. Server handler for `StartupReport` does not need to change. The planner should decide; both compile identically.

**Recommendation:** Option A (extend StartupReport) — fewer changes to protocol.rs and server-side handler. The server already has a `StartupReport` handler; adding fields with `#[serde(default)]` is backward-compatible if old agents send the message without new fields.

**Backward compat:** Use `#[serde(default)]` on new fields so old agents (v0.5.x) that don't send these fields do not cause deserialization errors on the server.

**When to send:** After `startup_report_sent = true` flag check (main.rs ~line 694). Currently this fires after first WS connect. With Phase 46, we also need to know if lock screen and remote ops bound successfully — which requires the oneshot signals to have resolved BEFORE we enter the WS reconnect loop. The startup sequence should:
1. Start servers (lock screen, remote ops) — get oneshot receivers
2. Await bind results — exit on failure
3. Enter WS reconnect loop
4. On first connect: send extended StartupReport with all bind statuses

### Anti-Patterns to Avoid

- **Calling `tokio::runtime::block_on()` from panic hook:** The Tokio runtime may be partially torn down during a panic. Sync HID writes and file writes are safe; async is not.
- **Using `lock()` in panic hook:** If the panic came from a thread holding the lock, `lock()` will deadlock. Always use `try_lock()` in panic hooks.
- **Installing hook after `set_hook()` call point:** Panics that occur during config loading or tracing init will miss the hook if it's installed after those calls. Install hook as first thing in `main()`.
- **Retrying bind indefinitely in silent background tasks:** The current remote_ops retry (10 attempts, 30s) is silent to main. This is acceptable for CLOSE_WAIT recovery (Phase 45 fixed this with SO_REUSEADDR) but not acceptable for initial bind failures where another process has the port.
- **Two lock screen servers running simultaneously:** The early lock screen (lines 332-366 in main.rs) is started for config error display, then dropped, and a new LockScreenManager is created. Phase 46 must be careful not to try to bind 18923 twice during the startup window. The drop of `early_lock_screen` should close the socket before the new manager binds.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Panic handler framework | Custom signal handler | `std::panic::set_hook()` | stdlib, zero-cost, correct semantics |
| Port availability check before bind | Pre-scan with `TcpStream::connect()` | bind and handle `EADDRINUSE` error | TOCTOU race: port can be taken between check and bind |
| Async state from panic hook | Any async/await in hook | Sync file write + sync HID | Tokio runtime is unsafe to call from panic hook |
| HID re-initialization in hook | Re-running full init | `FfbController::new()` + `zero_force()` | FfbController is lightweight, idempotent on construction |

---

## Common Pitfalls

### Pitfall 1: Panic Hook Reentrancy / Deadlock
**What goes wrong:** Panic hook calls `lock()` on a mutex. If the panic originated in code holding that mutex, the hook deadlocks, rc-agent hangs forever, and FFB is never zeroed — the exact hazard we're trying to prevent.
**Why it happens:** `Mutex::lock()` blocks if the mutex is taken. A panic unwinds the stack but doesn't necessarily release mutexes (depends on where the panic occurs relative to the `MutexGuard` drop).
**How to avoid:** Use `try_lock()` in all panic hook code. If `try_lock()` fails, skip the state mutation and proceed to `exit(1)`. The FFB zero does not need a mutex — it constructs a new `FfbController`.
**Warning signs:** rc-agent hangs after a simulated panic in tests.

### Pitfall 2: Early Lock Screen Conflicts with Main Lock Screen
**What goes wrong:** `early_lock_screen.start_server()` binds port 18923. Then `drop(early_lock_screen)` is called. Then `lock_screen.start_server()` tries to bind 18923 again. If the drop doesn't close the socket fast enough (or if the spawned task is still holding the listener), the second bind fails.
**Why it happens:** `LockScreenManager::drop()` does not explicitly close the listening socket. The tokio task holding the listener is still alive when the manager is dropped.
**How to avoid:** Before calling `start_server_checked()` for the main lock screen, add a small `sleep(100ms)` or restructure main.rs to avoid the double-bind. Alternatively, skip the early lock screen entirely and use the main lock screen from the start — but that's a bigger refactor.
**Warning signs:** main lock screen bind fails with EADDRINUSE on first startup.

### Pitfall 3: Startup Report Deserialization Breaks Old Server
**What goes wrong:** Extending `AgentMessage::StartupReport` with new fields causes `serde_json::from_str()` to fail on the racecontrol server if it expects an exact schema.
**Why it happens:** serde by default fails on unknown fields (though the default for derived Deserialize is to ignore them). The risk is the opposite: the server deserializes the message with the old struct and missing fields cause `Option<T>` fields to be None or fail if not wrapped in Option.
**How to avoid:** Add new fields with `#[serde(default)]` on both the struct definition AND the field — so old agents that send the old message get `false`/empty-vec defaults, and new agents get the real values. Verify with a roundtrip test.
**Warning signs:** Server logs deserialization errors for StartupReport messages from Phase 46 agents.

### Pitfall 4: FFB Zero Fails in Panic Hook Due to HID Enumeration Race
**What goes wrong:** `hidapi::HidApi::new()` inside the panic hook fails because the HID subsystem is being torn down or another thread holds the HID API handle.
**Why it happens:** HID API initialization can fail if USB state is disturbed. The panic hook may run while other threads are still accessing HID.
**How to avoid:** The panic hook should catch `Err` from `zero_force()` and log it without panicking again (no `unwrap()`). The retry loop (3x) mitigates transient failures. Accept that on rare HID race conditions, the zero may not succeed — log it and exit anyway.
**Warning signs:** Test showing FFB zero failure in panic hook despite wheelbase being connected.

### Pitfall 5: Port Bind Oneshot Dropped Before Main Awaits It
**What goes wrong:** If the spawned server task panics or exits before sending on the oneshot channel, the receiver gets `Err(RecvError)`. Main.rs must handle this as a bind failure, not a success.
**Why it happens:** oneshot sender is dropped without sending if the task panics.
**How to avoid:** Treat `oneshot::Receiver::await` returning `Err(_)` as equivalent to a bind failure — log and exit.

---

## Code Examples

Verified patterns from stdlib and existing codebase:

### Minimal Safe Panic Hook
```rust
// Source: std::panic docs + existing ffb_controller.rs pattern
// Install as first line of main()
use std::sync::atomic::{AtomicBool, Ordering};
static PANIC_HOOK_ACTIVE: AtomicBool = AtomicBool::new(false);

std::panic::set_hook(Box::new(|panic_info| {
    if PANIC_HOOK_ACTIVE.swap(true, Ordering::SeqCst) {
        return; // recursive panic guard
    }
    eprintln!("[rc-agent PANIC] {:?}", panic_info);
    // File write (sync, safe)
    use std::io::Write;
    let _ = std::fs::OpenOptions::new()
        .create(true).append(true)
        .open(r"C:\RacingPoint\rc-bot-events.log")
        .and_then(|mut f| writeln!(f, "[PANIC] {:?}", panic_info));
    // FFB zero (sync HID write, safe)
    let ffb = crate::ffb_controller::FfbController::new(0x1209, 0xFFB0);
    for _ in 0..3 {
        if matches!(ffb.zero_force(), Ok(_)) { break; }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    std::process::exit(1);
}));
```

### FFB zero_force_with_retry
```rust
// In ffb_controller.rs — new method (sync, safe to call from panic hook)
pub fn zero_force_with_retry(&self, attempts: u8, delay_ms: u64) -> bool {
    for i in 1..=attempts {
        match self.zero_force() {
            Ok(true) => { tracing::info!("FFB zero ok (attempt {})", i); return true; }
            Ok(false) => { return false; } // no device
            Err(e) => {
                tracing::warn!("FFB zero attempt {}/{}: {}", i, attempts, e);
                if i < attempts {
                    std::thread::sleep(std::time::Duration::from_millis(delay_ms));
                }
            }
        }
    }
    tracing::error!("FFB zero failed after {} attempts", attempts);
    false
}
```

### Lock Screen start_server_checked()
```rust
// In lock_screen.rs — returns oneshot receiver with bind result
pub fn start_server_checked(&self) -> tokio::sync::oneshot::Receiver<Result<u16, String>> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let state = self.state.clone();
    let event_tx = self.event_tx.clone();
    let port = self.port;
    let wallpaper_url = self.wallpaper_url.clone();
    tokio::spawn(async move {
        let addr: std::net::SocketAddr =
            format!("127.0.0.1:{}", port).parse().unwrap();
        let socket = match tokio::net::TcpSocket::new_v4() {
            Ok(s) => s,
            Err(e) => { let _ = tx.send(Err(e.to_string())); return; }
        };
        let _ = socket.set_reuseaddr(true);
        if let Err(e) = socket.bind(addr) {
            let _ = tx.send(Err(format!("port {} bind failed: {}", port, e)));
            return;
        }
        let listener = match socket.listen(128) {
            Ok(l) => { let _ = tx.send(Ok(port)); l }
            Err(e) => { let _ = tx.send(Err(e.to_string())); return; }
        };
        // ... existing serve loop with the listener
        serve_with_listener(listener, state, event_tx, wallpaper_url).await;
    });
    rx
}
```

### remote_ops start_checked()
```rust
// In remote_ops.rs — existing retry loop already correct for CLOSE_WAIT,
// but first-attempt failure (true EADDRINUSE) should be signaled
pub fn start_checked(port: u16) -> tokio::sync::oneshot::Receiver<Result<u16, String>> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    START_TIME.get_or_init(Instant::now);
    tokio::spawn(async move {
        let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
        // First attempt only — if it fails immediately (true conflict), signal failure
        // Subsequent CLOSE_WAIT retries are expected and silent
        let first_result = try_bind(addr).await;
        match first_result {
            Ok(listener) => {
                let _ = tx.send(Ok(port));
                // ... serve with listener
            }
            Err(e) if is_address_in_use(&e) => {
                // Try with retries (CLOSE_WAIT recovery) — but signal failure after 3s
                // not 30s, since if it's a real conflict it won't clear
                tokio::time::sleep(Duration::from_secs(3)).await;
                match try_bind(addr).await {
                    Ok(listener) => { let _ = tx.send(Ok(port)); /* serve */ }
                    Err(e) => { let _ = tx.send(Err(format!("port {} busy: {}", port, e))); }
                }
            }
            Err(e) => { let _ = tx.send(Err(e.to_string())); }
        }
    });
    rx
}
```

### Extended StartupReport fields (protocol.rs)
```rust
// Source: existing protocol.rs StartupReport variant, extended
AgentMessage::StartupReport {
    pod_id: String,
    version: String,
    uptime_secs: u64,
    config_hash: String,
    crash_recovery: bool,
    repairs: Vec<String>,
    // Phase 46 additions — all #[serde(default)] for backward compat
    #[serde(default)]
    lock_screen_port_bound: bool,
    #[serde(default)]
    remote_ops_port_bound: bool,
    #[serde(default)]
    hid_detected: bool,
    #[serde(default)]
    udp_ports_bound: Vec<u16>,
}
```

### startup-verify.sh E2E script pattern (mirrors close-wait.sh)
```bash
#!/bin/bash
# tests/e2e/fleet/startup-verify.sh
# After agent restart, verify BootVerification received by server within 30s,
# all ports bound, correct build_id on all pods.
SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
source "$SCRIPT_DIR/../lib/common.sh"
source "$SCRIPT_DIR/../lib/pod-map.sh"

SERVER_URL="${RC_BASE_URL:-http://192.168.31.23:8080/api/v1}"

for POD_NUM in $(seq 1 8); do
    POD_ID="pod-${POD_NUM}"
    POD_IP=$(pod_ip "$POD_ID")
    # Check /ping on :8090
    PING=$(curl -s --connect-timeout 1 --max-time 2 "http://${POD_IP}:8090/ping" 2>/dev/null)
    if [ "$PING" != "pong" ]; then
        skip "${POD_ID}: not reachable"
        continue
    fi
    # Check lock screen port via /info or direct probe
    LOCK_OK=$(curl -s --connect-timeout 1 --max-time 2 "http://${POD_IP}:18923/" 2>/dev/null | grep -c "Racing" || echo 0)
    if [ "$LOCK_OK" -gt 0 ]; then
        pass "${POD_ID}: lock screen port 18923 bound"
    else
        fail "${POD_ID}: lock screen port 18923 not responding"
    fi
    # Check fleet health for ws_connected (BootVerification proxy)
    HEALTH=$(curl -s --max-time 5 "${SERVER_URL}/fleet/health" 2>/dev/null)
    WS=$(echo "$HEALTH" | python3 -c "import sys,json; d=json.load(sys.stdin); pods=[p for p in d.get('pods',[]) if p.get('id')=='${POD_ID}']; print(pods[0].get('ws_connected',False) if pods else False)" 2>/dev/null)
    if [ "$WS" = "True" ]; then
        pass "${POD_ID}: WS connected (server received startup)"
    else
        fail "${POD_ID}: WS not connected"
    fi
done
summary_exit
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| No panic hook | `std::panic::set_hook()` stable | Rust 1.10 | All crashes caught before this phase; Phase 46 installs it |
| Manual port-available checks | Bind and handle EADDRINUSE | N/A — always correct | Phase 45 added SO_REUSEADDR to :8090; Phase 46 surfaces bind errors |
| Single FFB zero attempt | Retry wrapper | Phase 46 | Transient HID init failures handled |
| Minimal StartupReport | Extended with port/HID status | Phase 46 | Server gains observability into pod boot quality |

**Deprecated/outdated:**
- `early_lock_screen` double-bind: the current startup sequence creates two LockScreenManagers. This fragile pattern predates Phase 46. Phase 46 should either merge them or carefully sequence the drop.

---

## Open Questions

1. **Lock screen double-bind timing**
   - What we know: `early_lock_screen.start_server()` binds 18923 (lines 334), then is dropped (line 366), then `lock_screen.start_server()` re-binds 18923 (line 539).
   - What's unclear: Does dropping `LockScreenManager` close the port fast enough? The tokio task spawned by `start_server()` holds the listener handle — dropping the manager does not cancel the task.
   - Recommendation: In Phase 46, skip `early_lock_screen.start_server()` during the early phase, OR cancel the early task explicitly via a `JoinHandle` before starting the main lock screen. The simplest fix is to never start the early lock screen's HTTP server — just use a `LockScreenState` that the main lock screen later picks up.

2. **Panic hook VID/PID access**
   - What we know: The panic hook is `'static` and runs before config is loaded if panic occurs early. Default VID:0x1209 PID:0xFFB0 are defined as constants in `main.rs`.
   - What's unclear: Should the hook use hardcoded defaults or capture runtime config values?
   - Recommendation: Install a "pre-config" hook with hardcoded defaults. Reinstall after config loads with the actual values. Or use a `static AtomicU32` to store packed VID/PID that main() updates after loading config.

3. **Server-side handling of new StartupReport fields**
   - What we know: The server has a `StartupReport` handler in `crates/racecontrol-crate/`. It currently logs the fields.
   - What's unclear: Does the server store `lock_screen_port_bound` and `remote_ops_port_bound` in the DB or just log them?
   - Recommendation: For Phase 46, server only needs to log the new fields — no DB schema change. The E2E test verifies the pod health endpoint, not the startup report storage. Extend the log statement only.

---

## Validation Architecture

> nyquist_validation is enabled in .planning/config.json.

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (cargo-nextest available) |
| Config file | `crates/rc-agent/` (no separate nextest.toml needed — inherits workspace) |
| Quick run command | `cargo test -p rc-agent-crate 2>&1 \| tail -20` |
| Full suite command | `cargo nextest run -p rc-agent-crate` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| SAFETY-01 | Panic hook zeroes FFB + logs crash + exits | unit | `cargo test -p rc-agent-crate test_panic_hook` | ❌ Wave 0 |
| SAFETY-01 | Panic hook does not deadlock on try_lock | unit | `cargo test -p rc-agent-crate test_panic_hook_no_deadlock` | ❌ Wave 0 |
| SAFETY-02 | Port bind failure detected and propagated | unit | `cargo test -p rc-agent-crate test_lock_screen_bind_failure` | ❌ Wave 0 |
| SAFETY-03 | FFB zero retries 3x at 100ms intervals | unit | `cargo test -p rc-agent-crate test_zero_force_with_retry` | ❌ Wave 0 |
| SAFETY-04 | BootVerification fields serialize correctly | unit | `cargo test -p rc-common test_startup_report_boot_verification` | ❌ Wave 0 |
| SAFETY-05 | All safety tests pass | suite | `cargo test -p rc-agent-crate` | ❌ Wave 0 |
| E2E | startup-verify.sh passes on all pods after restart | e2e-shell | `bash tests/e2e/fleet/startup-verify.sh` | ❌ Wave 0 |

**Note on SAFETY-01 testing:** Testing a real panic hook with `std::process::exit(1)` in unit tests is difficult — `exit()` would kill the test runner. The standard pattern is:
- Test the hook behavior indirectly: verify the lock state mutation, FFB zero, and log write happen in isolation (without exit).
- Use a `test_mode` flag (static AtomicBool) that makes the hook skip `exit(1)` in test context.
- Or test components separately: test `zero_force_with_retry()` directly, test log write function directly.

### Sampling Rate
- **Per task commit:** `cargo test -p rc-agent-crate 2>&1 | tail -30`
- **Per wave merge:** `cargo nextest run -p rc-agent-crate && cargo test -p rc-common`
- **Phase gate:** Full suite green + `bash tests/e2e/fleet/startup-verify.sh` passes on Pod 8

### Wave 0 Gaps
- [ ] `crates/rc-agent/src/ffb_controller.rs` — add `test_zero_force_with_retry` (mock device not found → 3 Err attempts logged)
- [ ] `crates/rc-agent/src/lock_screen.rs` — add `test_start_server_checked_bind_failure` (bind a port, then try to bind it again → Err received on rx)
- [ ] `crates/rc-common/src/protocol.rs` — add `test_startup_report_new_fields_roundtrip` (serialize with new fields, deserialize, verify defaults for missing)
- [ ] `tests/e2e/fleet/startup-verify.sh` — new E2E script sourcing lib/common.sh + lib/pod-map.sh

---

## Sources

### Primary (HIGH confidence)
- Rust stdlib `std::panic` module — `set_hook()` signature, closure constraints, `PanicInfo` API
- Existing `crates/rc-agent/src/ffb_controller.rs` — `zero_force()` implementation, non-panicking contract
- Existing `crates/rc-agent/src/main.rs` — startup sequence, port start calls, StartupReport send
- Existing `crates/rc-agent/src/lock_screen.rs` — `serve_lock_screen()` bind logic (lines 644-658), `wait_for_self_ready()` polling
- Existing `crates/rc-agent/src/remote_ops.rs` — 10-retry bind loop (lines 91-141)
- Existing `crates/rc-common/src/protocol.rs` — `AgentMessage::StartupReport` current fields
- Existing `crates/rc-agent/src/self_monitor.rs` — `log_event()` pattern for rc-bot-events.log writes
- `.planning/ROADMAP.md` Phase 46 section — success criteria and requirements

### Secondary (MEDIUM confidence)
- Rust reference for panic hook constraints — hook closure is `Send + Sync + 'static`, async not safe inside hook
- `tokio::sync::oneshot` docs — standard pattern for task-to-caller result signaling

### Tertiary (LOW confidence)
- HID enumeration safety from panic hook — no official documentation; inference from hidapi behavior and sync nature of HID writes

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all libraries are already in Cargo.toml or stdlib
- Architecture: HIGH — patterns verified against existing codebase; no speculative APIs
- Pitfalls: HIGH — deadlock in panic hook is a well-known Rust pattern; double-bind is directly observable in main.rs source
- Protocol extension: HIGH — serde #[serde(default)] is verified stdlib behavior

**Research date:** 2026-03-19 IST
**Valid until:** 2026-04-19 (stable domain — Rust stdlib panic hooks don't change)
