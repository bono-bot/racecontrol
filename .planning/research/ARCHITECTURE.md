# Architecture Research

**Domain:** rc-agent / rc-sentry hardening and refactoring (v11.0)
**Researched:** 2026-03-20
**Confidence:** HIGH — based on direct source inspection of all affected files

---

## Current System Overview

```
+-----------------------------------------------------------------+
|                    Pod (192.168.31.x)                           |
|                                                                 |
|  +-----------------------------------------------------------+  |
|  |  rc-agent (port 8090)  -- tokio/axum, 3400-line main.rs  |  |
|  |                                                           |  |
|  |  +----------+  +----------+  +----------+  +----------+  |  |
|  |  |remote_ops|  |billing_  |  |failure_  |  |ffb_ctrl  |  |  |
|  |  |(Axum8090)|  |guard     |  |monitor   |  |ler       |  |  |
|  |  +----------+  +----------+  +----------+  +----------+  |  |
|  |        |            |             |              |        |  |
|  |  +-----v------------v-------------v--------------v-----+  |  |
|  |  |         main.rs event loop (select!)                |  |  |
|  |  |  AppState | WS reconnect | Config | Channels        |  |  |
|  |  +-----------------------------------------------------+  |  |
|  +-----------------------------------------------------------+  |
|                                                                 |
|  +-----------------------------------------------------------+  |
|  |  rc-sentry (port 8091) -- std::net, 155 lines            |  |
|  |                                                           |  |
|  |  /ping   /exec   (OPTIONS CORS)                          |  |
|  |  No timeout enforcement | No output truncation            |  |
|  |  Unbounded thread spawn per connection                    |  |
|  +-----------------------------------------------------------+  |
+-----------------------------------------------------------------+
          | WebSocket                       | HTTP :8090
          | UDP heartbeat                   |
          v                                 v
+-----------------------------------------------------------------+
|   racecontrol (Server .23:8080) -- Axum                         |
+-----------------------------------------------------------------+
          | cloud_sync
          v
+-----------------------------------------------------------------+
|   app.racingpoint.cloud (Bono VPS :443)                         |
+-----------------------------------------------------------------+
```

---

## What v11.0 Changes

### New vs Modified Components

| Component | Status | What Changes |
|-----------|--------|--------------|
| `rc-sentry/src/main.rs` | **Modified** | Add timeout enforcement, output truncation, concurrency semaphore, structured logging, new endpoints |
| `rc-agent/src/main.rs` | **Modified** | Extract ~2800 lines into new modules; main.rs becomes orchestrator only |
| `rc-agent/src/app_state.rs` | **New** | AppState struct and derived types extracted from main.rs |
| `rc-agent/src/ws_handler.rs` | **New** | WS reconnect loop, message send/recv, ping/pong |
| `rc-agent/src/config.rs` | **New** | AgentConfig, PodConfig, CoreConfig, WheelbaseConfig, etc. |
| `rc-agent/src/event_loop.rs` | **New** | select! dispatch logic -- the inner event loop body |
| `rc-common/src/exec.rs` | **New** | Shared exec primitive: semaphore + timeout + truncation |
| `rc-common/src/http_util.rs` | **New** | Shared HTTP response helpers for rc-sentry (JSON, plain text, CORS) |
| `rc-agent/tests/` | **New** | Integration tests for billing_guard, failure_monitor, ffb safety |
| `rc-sentry/tests/` | **New** | Integration tests for /health, /version, /exec, /processes |

---

## Target Architecture After v11.0

```
rc-agent/src/
  main.rs            (~150 lines)
      |                  init, panic hook, tracing setup,
      |                  spawn everything, enter reconnect
      |                  loop via ws_handler
      +-- config.rs      AgentConfig + all sub-configs
      +-- app_state.rs   AppState (shared runtime state)
      +-- ws_handler.rs  WS connect/reconnect/split, register
      +-- event_loop.rs  select! arms: WS msgs, channels,
      |                  heartbeat, billing ticks
      +-- billing_guard.rs    (unchanged)
      +-- failure_monitor.rs  (unchanged)
      +-- ffb_controller.rs   (unchanged)
      +-- remote_ops.rs       (unchanged, Axum/8090)
      +-- kiosk.rs            (unchanged)
      +-- lock_screen.rs      (unchanged)
      +-- [other 15 modules unchanged]

rc-sentry/src/
  main.rs            (~280 lines)
      |                  listen loop, per-connection threads
      |                  with semaphore guard, structured
      |                  log lines (JSON to stderr)
      +-- handle()           request parse, route dispatch
      +-- handle_exec()      calls rc_common::exec::run_cmd_sync()
      +-- handle_health()    NEW: uptime, version, active_conns
      +-- handle_version()   NEW: GIT_HASH + pkg version
      +-- handle_files()     NEW: dir listing (path query param)
      +-- handle_processes() NEW: running process snapshot

rc-common/src/
  exec.rs            NEW: ExecRequest, ExecResult, run_cmd_sync, run_cmd_async
                     Shared between remote_ops.rs AND rc-sentry
                     Sync path for rc-sentry (std::net threads)
                     Async path for rc-agent remote_ops (tokio)
  http_util.rs       NEW: json_response(), plain_response(), cors_ok()
                     Only for rc-sentry -- rc-agent uses axum
```

---

## Component Responsibilities (Current to Target)

| Module | Current Responsibility | v11.0 Change |
|--------|------------------------|--------------|
| `main.rs` | Everything: config, state, WS, event loop, panic hook, startup | Orchestrator only: init + spawn. Delegates to 4 new modules |
| `config.rs` (new) | -- | All *Config structs, load_config(), default_* fns |
| `app_state.rs` (new) | -- | AppState, LaunchState, CrashRecoveryState, shared Arcs |
| `ws_handler.rs` (new) | -- | connect_and_run(): WS connect, split, register, drive event loop |
| `event_loop.rs` (new) | -- | run_event_loop(): select! arms, message dispatch |
| `rc-sentry main.rs` | Single-file std::net HTTP, no timeouts | Hardened: semaphore, timeout_ms honoured, output truncated, structured log, 4 new endpoints |
| `rc-common exec.rs` (new) | -- | run_cmd(cmd, timeout_ms) -> ExecResult -- sync + async variant |
| `rc-common http_util.rs` (new) | -- | Raw HTTP response formatting (used by rc-sentry only) |

---

## Data Flow Changes

### rc-agent main.rs Decomposition Flow

**Before:**
```
main() [3400 lines]
  all logic inline
```

**After:**
```
main() [~150 lines]
  -> config::load_config()         reads rc-agent.toml
  -> app_state::AppState::new()    builds shared state from config
  -> spawn all background tasks    (unchanged spawns)
  -> ws_handler::connect_and_run() drives WS + event loop
        -> event_loop::run_one_iteration()   called per select! tick
```

Channels that cross the module boundary into event_loop:

| Channel | Direction | Owner after extraction |
|---------|-----------|----------------------|
| `failure_monitor_tx: watch::Sender<FailureMonitorState>` | event_loop writes, failure_monitor reads | event_loop |
| `ws_exec_result_tx: mpsc::Sender<AgentMessage>` | WS handler tasks write, event_loop drains | ws_handler |
| `lock_event_rx: mpsc::Receiver<LockScreenEvent>` | lock_screen sends, event_loop drains | event_loop |
| `heartbeat_event_rx: mpsc::Receiver<HeartbeatEvent>` | heartbeat sends, event_loop drains | event_loop |
| `signal_rx: mpsc::Receiver<DetectorSignal>` | HID/UDP sends, event_loop drains | event_loop |
| `ai_result_rx: mpsc::Receiver<AiDebugSuggestion>` | ai_debugger sends, event_loop drains | event_loop |

All channels pass into event_loop via an `EventLoopArgs` struct to avoid a >10-parameter function signature.

### rc-sentry Hardening Flow

**Before:**
```
incoming connection
  -> thread::spawn (unbounded)
  -> handle()
  -> handle_exec()
  -> Command::new("cmd.exe") blocking, no timeout, no output limit
  -> send_response()
```

**After:**
```
incoming connection
  -> ACTIVE_CONNS check (AtomicUsize >= MAX_CONNS -> send 503, drop)
  -> ACTIVE_CONNS.fetch_add(1)
  -> thread::spawn
  -> handle()
  -> handle_exec()
  -> rc_common::exec::run_cmd_sync(req)
       timeout via thread + channel recv_timeout
       output truncated to 64KB before returning
  -> structured log line written to stderr:
       {"ts":"...","method":"POST","path":"/exec","status":200,"ms":42}
  -> send_response()
  -> ACTIVE_CONNS.fetch_sub(1)
```

### rc-common Shared Exec API Design

rc-sentry is std-only (no tokio). rc-agent remote_ops is async/tokio. The shared exec primitive must support both without pulling tokio into rc-sentry.

```
rc-common/src/exec.rs:

pub struct ExecRequest {
    pub cmd: String,
    pub timeout_ms: u64,         // 0 = use default (30_000)
    pub max_output_bytes: usize, // 0 = use default (65_536)
}

pub struct ExecResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub timed_out: bool,
    pub truncated: bool,
}

// Sync path: for rc-sentry (std::net thread)
// Uses thread::spawn + channel.recv_timeout() for timeout
pub fn run_cmd_sync(req: &ExecRequest) -> ExecResult

// Async path: for rc-agent remote_ops (tokio)
// Uses tokio::time::timeout + tokio::process::Command
// Feature-gated so rc-sentry does not pull in tokio
#[cfg(feature = "async-exec")]
pub async fn run_cmd_async(req: &ExecRequest) -> ExecResult
```

Cargo feature in rc-common:
```
[features]
default = []
async-exec = ["dep:tokio"]
```

rc-sentry Cargo.toml: `rc-common = { workspace = true }` (no async-exec)
rc-agent Cargo.toml: `rc-common = { workspace = true, features = ["async-exec"] }`

This ensures rc-sentry binary stays minimal -- tokio is not linked.

---

## New rc-sentry Endpoints

| Endpoint | Method | Returns | Notes |
|----------|--------|---------|-------|
| `/ping` | GET | `"pong"` (text/plain) | Existing, unchanged |
| `/exec` | POST | `{stdout, stderr, exit_code, timed_out, truncated}` | Hardened |
| `/health` | GET | `{uptime_secs, active_conns, max_conns, version, build_id}` | New |
| `/version` | GET | `{version, build_id}` | New -- mirrors rc-agent :8090/health response |
| `/files` | GET | `{entries:[{name,size,is_dir,modified_secs}]}` | New, ?path= query param |
| `/processes` | GET | `{processes:[{pid,name,memory_kb}]}` | New, top 50 by memory |

All new endpoints use std only. `/processes` requires adding `sysinfo = "0.33"` to rc-sentry Cargo.toml. `/health` uptime via `OnceLock<Instant>` initialized at main() entry -- same pattern already used in remote_ops.rs.

---

## Architectural Patterns

### Pattern 1: Mechanical Module Extraction via Function Boundary

The rc-agent main.rs decomposition should NOT redesign the data model. The select! body is already structured -- it is a mechanical extraction problem, not an architectural redesign.

**What:** Extract the select! body into `run_one_iteration(args: &mut EventLoopArgs) -> LoopControl`. The compiler enforces correctness.

**When to use:** Any block over 200 lines that has a single clear input/output contract.

**Trade-offs:**
- Pro: Minimal diff, compiler-verified, no behavioural change
- Pro: Each extracted module can grow its own tests independently
- Con: EventLoopArgs becomes a large struct -- acceptable since it replaces local variables, not a long-term API

### Pattern 2: OnceLock + AtomicUsize for rc-sentry Globals

rc-sentry is multi-threaded std. Thread-safe globals without Mutex overhead.

**What:** `OnceLock<Instant>` for start time, `AtomicUsize` for active connection count.

**When to use:** Single-writer globals that are set once (start time) or incremented/decremented atomically (connection count).

**Trade-offs:**
- Pro: Zero lock contention, appropriate for this access pattern
- Con: AtomicUsize does not prevent the count going below zero on bug -- guard with saturating_sub in fetch_sub

### Pattern 3: Characterization Tests Before Refactor (Standing Rule)

Write tests that pin current behaviour, verify green, refactor, verify still green.

**Extraction order for rc-agent:**
1. `config.rs` -- no runtime effects, pure deserialization, trivial to test
2. `app_state.rs` -- pure data construction, no channels
3. `ws_handler.rs` -- WS lifecycle, depends on config + app_state
4. `event_loop.rs` -- select! dispatch, most complex, protected by tests from steps 1-3

### Pattern 4: Sync Timeout via Thread + Channel (rc-sentry exec)

rc-sentry has no tokio. Timeout is implemented by running the command in a dedicated thread and using `channel.recv_timeout()` in the handler thread.

**What:** The spawned command thread sends its result on a channel. The handler thread waits with a deadline. On timeout, the handler returns a timeout error -- the command thread continues in background until the child process exits naturally.

**When to use:** Any blocking operation that needs a timeout in a std (non-async) context.

**Trade-offs:**
- Pro: No tokio dependency, correct deadline semantics
- Con: The child process is not killed on timeout (Windows kill requires a handle; keepng it simple is correct for an LAN admin tool with short-lived commands)
- Con: One extra thread per exec call -- bounded by MAX_CONNS semaphore

---

## Build Order

Dependencies determine ordering. Build from leaves to roots:

```
Step 1: rc-common
  - Add exec.rs (ExecRequest, ExecResult, run_cmd_sync, run_cmd_async feature-gated)
  - Add http_util.rs (json_response, plain_response, cors_ok)
  - cargo test -p rc-common

Step 2: rc-sentry
  - Update Cargo.toml: add rc-common, sysinfo
  - Harden handle_exec(): call rc_common::exec::run_cmd_sync
  - Add /health, /version, /files, /processes handlers
  - Add structured log line per request
  - Add AtomicUsize concurrency guard
  - cargo test -p rc-sentry
  - cargo build --release --bin rc-sentry

Step 3: rc-agent (decomposition)
  - Write characterization tests for billing_guard, failure_monitor, ffb safety
  - Extract config.rs -- cargo test -p rc-agent
  - Extract app_state.rs -- cargo test -p rc-agent
  - Extract ws_handler.rs -- cargo test -p rc-agent
  - Extract event_loop.rs -- cargo test -p rc-agent
  - Update remote_ops.rs to use rc_common::exec::run_cmd_async
  - cargo build --release --bin rc-agent

Step 4: Integration verification
  - Deploy rc-sentry to Pod 8 (canary)
  - Verify /health, /exec with timeout, /processes via curl
  - Deploy rc-agent to Pod 8, verify no regression (billing, game launch, WS)
  - Roll to remaining pods
```

**Why this order:**
- rc-common changes must land before rc-sentry or rc-agent can use them
- rc-sentry is independent of rc-agent -- both can be developed in parallel once rc-common is ready
- rc-agent decomposition is highest risk -- do last, after tests protect critical paths
- Both rc-sentry and rc-agent depend on rc-common, not on each other

---

## Integration Points

### rc-sentry -> rc-common (new dependency)

| Integration | Type | Notes |
|-------------|------|-------|
| `rc_common::exec::run_cmd_sync` | Direct fn call | No async, no tokio in rc-sentry |
| `rc_common::exec::ExecRequest` | Struct | Parsed from JSON body in handle_exec() |
| `rc_common::exec::ExecResult` | Struct | Serialized as JSON in send_response() |
| `rc_common::http_util::*` | Fn calls | Replace local send_response/send_plain/send_cors_preflight |

rc-sentry Cargo.toml additions:
```
rc-common = { workspace = true }
sysinfo = "0.33"
```

### rc-agent remote_ops -> rc-common (optional, improves consistency)

| Integration | Type | Notes |
|-------------|------|-------|
| `rc_common::exec::run_cmd_async` | Async fn call | Replaces inline tokio::process::Command in exec_command() |

This removes duplicated truncation/timeout logic in remote_ops.rs. The ExecRequest/ExecResult types map to the existing JSON wire format -- no server-side changes needed.

rc-agent Cargo.toml: add `features = ["async-exec"]` to rc-common dependency.

### rc-agent main.rs -> new modules

| Boundary | Communication | Notes |
|----------|---------------|-------|
| main -> config | `config::load_config() -> AgentConfig` | No channels, pure return |
| main -> app_state | `AppState::new(&config)` | Constructs Arcs passed to tasks |
| main -> ws_handler | `ws_handler::connect_and_run(state, config).await` | Owns reconnect loop |
| ws_handler -> event_loop | `event_loop::run_one_iteration(&mut EventLoopArgs)` | Called per select! tick |
| event_loop -> all tasks | Existing channels (watch, mpsc) | Unchanged types and directions |

---

## Anti-Patterns to Avoid

### Anti-Pattern 1: Adding tokio to rc-sentry

**What people do:** Pull in tokio for "proper" async while hardening rc-sentry.

**Why it's wrong:** rc-sentry's value is its minimal binary with no async runtime. Adding tokio adds ~1MB binary size, runtime feature coordination, and eliminates the "independent of rc-agent failure modes" property. A panic in the tokio runtime of rc-sentry would mirror rc-agent failure modes.

**Do this instead:** Use the thread + channel timeout pattern via rc-common exec.rs. Keep rc-sentry as std-only.

### Anti-Pattern 2: Big-Bang Refactor of main.rs

**What people do:** Delete main.rs, rewrite as 4 files simultaneously, push when it compiles.

**Why it's wrong:** The 3400-line main.rs has no unit tests. A big-bang rewrite has no regression safety net. One incorrect channel direction produces a runtime bug indistinguishable from a pre-existing bug.

**Do this instead:** Extract one module at a time in compilation order. Each extraction compiles and passes cargo test before moving to the next.

### Anti-Pattern 3: God Object AppState with Mutex Fields

**What people do:** Create a large AppState with Mutex-wrapped fields, pass Arc<AppState> to every module.

**Why it's wrong:** The current design uses fine-grained channels (watch, mpsc) with documented ownership and flow direction. Replacing with a shared Mutex<AppState> collapses the ownership model and risks deadlocks when two modules both try to lock state during event processing.

**Do this instead:** Keep channels as-is. Extracted modules receive only the channels they need, not a god object.

### Anti-Pattern 4: Calling sysinfo in the rc-sentry Accept Loop

**What people do:** Call sysinfo::System::refresh_all() in the main accept loop to keep a process cache warm.

**Why it's wrong:** refresh_all() can take 100ms+ on Windows (iterates all processes via NtQuerySystemInformation). This blocks new connections from being accepted.

**Do this instead:** Spawn a thread per connection (which rc-sentry already does). The process scan runs inside the spawned thread, not in the accept loop. Scan on each /processes request -- the endpoint is low-frequency staff tooling, not a hot path.

---

## Scaling Considerations

This is a fixed 8-pod venue. Scaling is not a concern. The hardening targets reliability, not throughput.

| Concern | At 8 pods (current) | Notes |
|---------|---------------------|-------|
| rc-sentry concurrency | 1-3 simultaneous admin calls in practice | Cap at 16 -- comfortable margin |
| rc-agent event loop | Single tokio select! drives one pod | Adequate, no concern |
| rc-common exec | Semaphore per caller (8 in remote_ops, 4 in WS) | Keep per-module semaphores separate |

---

## Sources

- Direct source inspection: `crates/rc-sentry/src/main.rs` (155 lines, 2026-03-20)
- Direct source inspection: `crates/rc-agent/src/main.rs` (3400+ lines, 2026-03-20)
- Direct source inspection: `crates/rc-agent/src/remote_ops.rs` (semaphore, timeout, truncation patterns)
- Direct source inspection: `crates/rc-agent/src/billing_guard.rs`, `failure_monitor.rs`, `ffb_controller.rs`
- Direct source inspection: `crates/rc-common/src/lib.rs`, `protocol.rs`
- Direct source inspection: `crates/rc-sentry/Cargo.toml`, `crates/rc-agent/Cargo.toml`
- Project context: `.planning/PROJECT.md` (v11.0 requirements)
- Operational rules: `CLAUDE.md` (build commands, standing process rules, deployment rules)

---

*Architecture research for: rc-agent/rc-sentry hardening and refactoring (v11.0)*
*Researched: 2026-03-20 IST*
