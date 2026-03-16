---
phase: 17
type: research
created: 2026-03-15
---

# Phase 17: WebSocket Exec - Research

**Researched:** 2026-03-15
**Domain:** Rust/Axum WebSocket protocol extension, remote command execution
**Confidence:** HIGH

## Summary

Phase 17 adds remote shell command execution over the existing WebSocket channel between racecontrol and rc-agent. Currently, remote command execution is only available via HTTP POST to port 8090 (the `remote_ops` module in rc-agent). This phase extends the WebSocket protocol so racecontrol can send shell commands to any connected pod without needing HTTP reachability on port 8090.

The codebase is well-structured for this change. The protocol enums (`CoreToAgentMessage` and `AgentMessage`) in `rc-common/src/protocol.rs` use serde-tagged JSON and are the single source of truth for all WebSocket messages. The racecontrol side already tracks per-agent mpsc senders in `AppState.agent_senders`, and the rc-agent side already has a complete match arm for every `CoreToAgentMessage` variant with a catch-all `other =>` at the end. The existing HTTP implementation in `remote_ops.rs` provides a proven pattern for semaphore-gated, timeout-wrapped, CREATE_NO_WINDOW process spawning that can be directly reused.

**Primary recommendation:** Add `Exec` and `ExecResult` variants to the shared protocol, implement the handler in rc-agent's existing match block, and add a `ws_exec_on_pod` function in racecontrol that sends via `agent_senders` and waits for a correlated response. Deploy.rs gets a fallback path when HTTP :8090 is unreachable.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| WSEX-01 | racecontrol can send shell commands to any connected pod via WebSocket (CoreToAgentMessage::Exec) | New `Exec` variant in CoreToAgentMessage with cmd, timeout_ms, request_id fields. Agent sender pattern already exists in `AppState.agent_senders`. |
| WSEX-02 | rc-agent runs WebSocket commands with independent semaphore (separate from HTTP slots) | New `static WS_EXEC_SEMAPHORE: Semaphore` in rc-agent, separate from `remote_ops::EXEC_SEMAPHORE`. Same `const_new(4)` pattern. |
| WSEX-03 | Responses include stdout, stderr, exit code, and request_id correlation | New `ExecResult` variant in AgentMessage. `request_id: String` for correlation. Use `uuid::Uuid::new_v4()` for IDs. |
| WSEX-04 | deploy.rs uses WebSocket as fallback when HTTP :8090 is unreachable | Wrap existing `exec_on_pod` with try-HTTP-first, fallback-to-WS pattern. New `ws_exec_on_pod` function using `agent_senders` + oneshot response channel. |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| tokio | 1.x (workspace) | Async runtime, Semaphore, mpsc, oneshot channels | Already in use everywhere |
| serde/serde_json | 1.x (workspace) | JSON serialization for WebSocket messages | Already in use for all protocol types |
| uuid | 1.x (workspace) | Request ID generation | Already a workspace dependency |
| tokio-tungstenite | 0.26 | WebSocket client (rc-agent side) | Already in use |
| axum | 0.8 (with ws feature) | WebSocket server (racecontrol side) | Already in use |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| tokio::sync::Semaphore | (part of tokio) | Rate-limit concurrent WS commands | WSEX-02: independent semaphore |
| tokio::sync::oneshot | (part of tokio) | Request-response correlation on core side | WSEX-04: wait for result |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| oneshot channel for response | DashMap with request_id key and Sender value | DashMap adds a dependency; oneshot + RwLock HashMap is sufficient given low concurrency |
| uuid for request_id | AtomicU64 counter | Counter is simpler but not globally unique; uuid is already available |

**Installation:** No new dependencies needed. Everything is already in the workspace.

## Architecture Patterns

### Recommended Changes by File

```
crates/rc-common/src/protocol.rs    # Add Exec variant to CoreToAgentMessage, ExecResult to AgentMessage
crates/rc-agent/src/main.rs         # Add match arm for CoreToAgentMessage::Exec (line ~1740)
crates/racecontrol/src/ws/mod.rs        # Add ExecResult handler in agent message match
crates/racecontrol/src/deploy.rs        # Add ws_exec_on_pod fallback, modify exec_on_pod signature
crates/racecontrol/src/state.rs         # Add pending_ws_execs: RwLock<HashMap<String, oneshot::Sender<WsExecResult>>>
```

### Pattern 1: Protocol Extension (CoreToAgentMessage::Exec)

**What:** Add new variant to the shared protocol enum in rc-common.
**When to use:** Every time a new command flows core -> agent.
**Existing pattern (from Ping at protocol.rs line 241):**

```rust
// In CoreToAgentMessage enum (crates/rc-common/src/protocol.rs, after QueryAssistState)
/// Run a shell command on this pod (remote exec via WebSocket)
Exec {
    request_id: String,
    cmd: String,
    #[serde(default = "default_exec_timeout_ms")]
    timeout_ms: u64,
},
```

### Pattern 2: Agent Response (AgentMessage::ExecResult)

**What:** Add new variant for agent -> core results.
**Existing pattern (from Pong, AssistChanged):**

```rust
// In AgentMessage enum (crates/rc-common/src/protocol.rs, after AssistState)
/// Result of a WebSocket command (response to CoreToAgentMessage::Exec)
ExecResult {
    request_id: String,
    success: bool,
    exit_code: Option<i32>,
    stdout: String,
    stderr: String,
},
```

### Pattern 3: Agent-Side Handler (Independent Semaphore)

**What:** Handle incoming commands with a separate semaphore from HTTP.
**Mirrors:** `remote_ops::exec_command` (lines 237-300 of remote_ops.rs).

```rust
// In rc-agent main.rs or new ws_exec.rs module
use tokio::sync::Semaphore;
use tokio::process::Command;
use tokio::time::{timeout, Duration};
#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;
const WS_MAX_CONCURRENT_EXECS: usize = 4;

static WS_EXEC_SEMAPHORE: Semaphore = Semaphore::const_new(WS_MAX_CONCURRENT_EXECS);

async fn handle_ws_exec(request_id: String, cmd: String, timeout_ms: u64) -> AgentMessage {
    let permit = match WS_EXEC_SEMAPHORE.try_acquire() {
        Ok(p) => p,
        Err(_) => {
            return AgentMessage::ExecResult {
                request_id,
                success: false,
                exit_code: None,
                stdout: String::new(),
                stderr: format!("WS slots exhausted ({} max)", WS_MAX_CONCURRENT_EXECS),
            };
        }
    };

    let result = timeout(Duration::from_millis(timeout_ms), async {
        let mut cmd_proc = Command::new("cmd");
        cmd_proc.args(["/C", &cmd]).kill_on_drop(true);
        #[cfg(windows)]
        cmd_proc.creation_flags(CREATE_NO_WINDOW);
        cmd_proc.output().await
    }).await;

    drop(permit);

    match result {
        Ok(Ok(out)) => AgentMessage::ExecResult {
            request_id,
            success: out.status.success(),
            exit_code: out.status.code(),
            stdout: String::from_utf8_lossy(&out.stdout).to_string(),
            stderr: String::from_utf8_lossy(&out.stderr).to_string(),
        },
        Ok(Err(e)) => AgentMessage::ExecResult {
            request_id,
            success: false,
            exit_code: None,
            stdout: String::new(),
            stderr: format!("Failed to run: {}", e),
        },
        Err(_) => AgentMessage::ExecResult {
            request_id,
            success: false,
            exit_code: Some(124),
            stdout: String::new(),
            stderr: format!("Command timed out after {}ms", timeout_ms),
        },
    }
}
```

### Pattern 4: Core-Side Request-Response Correlation

**What:** racecontrol sends command, waits for result with matching request_id.
**Uses:** `oneshot::channel` stored in AppState keyed by request_id.

```rust
// In racecontrol state.rs -- add to AppState:
pub pending_ws_execs: RwLock<HashMap<String, tokio::sync::oneshot::Sender<WsExecResult>>>,

// Data struct for the result (in deploy.rs or new module)
pub struct WsExecResult {
    pub success: bool,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

// Core-side function to send command and await result:
pub async fn ws_exec_on_pod(
    state: &Arc<AppState>,
    pod_id: &str,
    cmd: &str,
    timeout_ms: u64,
) -> Result<(bool, String, String), String> {
    let request_id = uuid::Uuid::new_v4().to_string();
    let (tx, rx) = tokio::sync::oneshot::channel();

    // Register pending response
    state.pending_ws_execs.write().await.insert(request_id.clone(), tx);

    // Send command to agent (clone sender, drop lock immediately)
    let sender = {
        let senders = state.agent_senders.read().await;
        senders.get(pod_id).cloned()
            .ok_or_else(|| format!("Pod {} not connected via WebSocket", pod_id))?
    };

    sender.send(CoreToAgentMessage::Exec {
        request_id: request_id.clone(),
        cmd: cmd.to_string(),
        timeout_ms,
    }).await.map_err(|_| format!("Failed to send to pod {}", pod_id))?;

    // Wait for response with timeout (buffer over command timeout)
    match tokio::time::timeout(Duration::from_millis(timeout_ms + 5000), rx).await {
        Ok(Ok(result)) => Ok((result.success, result.stdout, result.stderr)),
        Ok(Err(_)) => {
            state.pending_ws_execs.write().await.remove(&request_id);
            Err("WS response channel closed unexpectedly".to_string())
        }
        Err(_) => {
            state.pending_ws_execs.write().await.remove(&request_id);
            Err(format!("WS timed out after {}ms", timeout_ms + 5000))
        }
    }
}
```

### Pattern 5: Deploy.rs Fallback Logic

**What:** Try HTTP first, fall back to WebSocket if HTTP is unreachable.
**Where:** Modify `exec_on_pod` in deploy.rs (currently at line 172).

The current `exec_on_pod` signature is:
```rust
async fn exec_on_pod(state, pod_ip, cmd, timeout_ms) -> Result<(bool, String, String), String>
```

Change to accept both pod_id and pod_ip:
```rust
async fn exec_on_pod(state, pod_id, pod_ip, cmd, timeout_ms) -> Result<(bool, String, String), String>
```

All callers in deploy.rs already have both `pod_id` and `pod_ip` in scope.

### Anti-Patterns to Avoid

- **Sharing the HTTP semaphore with WS:** WSEX-02 explicitly requires an independent semaphore. If HTTP slots are exhausted, WS must still work (that is the whole point of the fallback).
- **Blocking the agent event loop:** The handler MUST be spawned as a separate `tokio::spawn` task. The agent's `tokio::select!` loop processes heartbeats, telemetry, and billing ticks. A long-running command must not block those.
- **Fire-and-forget without correlation:** The `request_id` is essential. Without it, racecontrol cannot match responses to requests when multiple commands are in flight.
- **Holding agent_senders lock across await:** Clone the sender, drop the lock, then send. Existing pattern in billing.rs and auth/mod.rs.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Request-response correlation | Custom HashMap + polling loop | `oneshot::channel` in `pending_ws_execs` | Clean, type-safe, auto-cleanup on drop |
| Concurrent limiting | Manual counter with atomics | `tokio::sync::Semaphore::const_new()` | Already proven in remote_ops.rs |
| Process spawning on Windows | Raw `std::process::Command` | `tokio::process::Command` with `kill_on_drop(true)` + `creation_flags(CREATE_NO_WINDOW)` | Prevents zombie processes and console windows |
| Timeout wrapper | Manual `tokio::select!` with sleep | `tokio::time::timeout()` | Cleaner, less error-prone |

**Key insight:** The HTTP implementation in `remote_ops.rs` (lines 237-300) is the exact blueprint. The WS handler is nearly identical but sends the response as an `AgentMessage` instead of an HTTP response.

## Common Pitfalls

### Pitfall 1: Blocking the Agent Event Loop
**What goes wrong:** Calling `Command::output().await` directly in the `tokio::select!` match arm blocks the entire event loop. Heartbeats stop, billing ticks are missed, the pod appears offline.
**Why it happens:** The match arm runs inline in the select loop.
**How to avoid:** `tokio::spawn` the handler. Send the `AgentMessage::ExecResult` via a dedicated mpsc channel that feeds back into the select loop (like `signal_rx` and `ai_result_rx`).
**Warning signs:** Pod appears "disconnected" during long-running commands. Heartbeat interval violated.

### Pitfall 2: Deadlock on agent_senders Lock
**What goes wrong:** On the core side, `ws_exec_on_pod` acquires `agent_senders.read()` to send the command. If the response handler in `ws/mod.rs` also tries to acquire `agent_senders` while write contention exists, you get a deadlock.
**Why it happens:** The read lock is held across the `sender.send().await` call.
**How to avoid:** Clone the sender from `agent_senders`, drop the read lock immediately, then send on the clone. This is already the established pattern in `billing.rs` and `auth/mod.rs`.

### Pitfall 3: Orphaned Pending Entries
**What goes wrong:** If the agent disconnects before responding, the `pending_ws_execs` entry leaks (oneshot sender never fires, receiver hangs until timeout).
**Why it happens:** Agent WebSocket drops, no result is ever sent.
**How to avoid:** The timeout in `ws_exec_on_pod` already handles this (receiver times out and cleans up). Additionally, on agent disconnect in `ws/mod.rs` (around line 500), sweep and remove all `pending_ws_execs` entries for that pod. Use a pod-prefixed request_id (e.g., `pod_3:uuid`) so disconnect cleanup can filter by prefix.

### Pitfall 4: ws_tx Ownership in Spawned Task
**What goes wrong:** The agent's `ws_tx` (WebSocket write half) is used in `tokio::select!` for heartbeats, telemetry, etc. You cannot move it into a spawned task.
**Why it happens:** `ws_tx` is borrowed mutably by the select loop.
**How to avoid:** Use an mpsc channel to forward results back to the select loop, which then sends them via `ws_tx`. Same approach as `signal_rx`, `ai_result_rx`, and `lock_event_rx` -- a dedicated channel for results that the select loop drains.

### Pitfall 5: Missing Serde Roundtrip Tests
**What goes wrong:** New enum variants do not serialize/deserialize as expected, causing silent message drops.
**Why it happens:** The `#[serde(rename_all = "snake_case")]` and `#[serde(tag = "type", content = "data")]` attributes require specific JSON wire format.
**How to avoid:** Write roundtrip serde tests (the protocol.rs tests module has 40+ of these as templates). Always verify wire format matches expectations.

## Code Examples

### Exact Insertion Points

**1. rc-common/src/protocol.rs line ~256** - CoreToAgentMessage enum (before closing brace):
```rust
/// Run a shell command on this pod (remote exec via WebSocket)
Exec {
    request_id: String,
    cmd: String,
    #[serde(default = "default_exec_timeout_ms")]
    timeout_ms: u64,
},
```

Plus a free function after the enum:
```rust
fn default_exec_timeout_ms() -> u64 { 10_000 }
```

**2. rc-common/src/protocol.rs line ~90** - AgentMessage enum (before closing brace):
```rust
/// Result of a WebSocket command (response to CoreToAgentMessage::Exec)
ExecResult {
    request_id: String,
    success: bool,
    exit_code: Option<i32>,
    stdout: String,
    stderr: String,
},
```

**3. rc-agent/src/main.rs line ~1739** - match arm (before `other =>`):
```rust
rc_common::protocol::CoreToAgentMessage::Exec { request_id, cmd, timeout_ms } => {
    tracing::info!("WS exec request {}: {}", request_id, cmd);
    let result_tx = ws_exec_result_tx.clone();
    tokio::spawn(async move {
        let result = handle_ws_exec(request_id, cmd, timeout_ms).await;
        let _ = result_tx.send(result).await;
    });
}
```

**4. rc-agent/src/main.rs** - new select arm in the main `tokio::select!` loop:
```rust
Some(exec_result) = ws_exec_result_rx.recv() => {
    if let Ok(json) = serde_json::to_string(&exec_result) {
        if ws_tx.send(Message::Text(json.into())).await.is_err() {
            tracing::error!("Failed to send exec result, connection lost");
            break;
        }
    }
}
```

**5. racecontrol/src/ws/mod.rs** - ExecResult handler in agent message match:
```rust
AgentMessage::ExecResult { request_id, success, exit_code, stdout, stderr } => {
    tracing::info!("WS exec result {}: success={}", request_id, success);
    let mut pending = state.pending_ws_execs.write().await;
    if let Some(sender) = pending.remove(&request_id) {
        let _ = sender.send(WsExecResult { success, exit_code, stdout, stderr });
    } else {
        tracing::warn!("No pending request for request_id={}", request_id);
    }
}
```

**6. racecontrol/src/deploy.rs** - fallback pattern (rename current fn, add wrapper):
```rust
// Current exec_on_pod (line 172) becomes http_exec_on_pod (same body).
// New exec_on_pod with pod_id parameter and fallback:
async fn exec_on_pod(
    state: &Arc<AppState>,
    pod_id: &str,
    pod_ip: &str,
    cmd: &str,
    timeout_ms: u64,
) -> Result<(bool, String, String), String> {
    match http_exec_on_pod(state, pod_ip, cmd, timeout_ms).await {
        Ok(result) => Ok(result),
        Err(http_err) => {
            tracing::warn!("HTTP failed for {}: {}. Trying WS fallback.", pod_id, http_err);
            ws_exec_on_pod(state, pod_id, cmd, timeout_ms).await
        }
    }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Separate pod-agent binary on port 8090 | Merged remote_ops module in rc-agent on port 8090 | Pre-Phase 17 | Single binary, but still HTTP-dependent |
| HTTP-only remote exec | HTTP primary + WS fallback (Phase 17) | Phase 17 | Works even when firewall blocks port 8090 |

**Why this matters:** Pods occasionally have Windows Firewall issues that block port 8090 even after Phase 16's auto-config. WebSocket on port 8080 is always reachable because it is an outbound connection from pod to core (not inbound). WS provides a guaranteed command channel.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (built-in, Rust edition 2024) |
| Config file | Cargo.toml workspace |
| Quick run command | `cargo test -p rc-common` |
| Full suite command | `cargo test -p rc-common && cargo test -p rc-agent-crate && cargo test -p racecontrol-crate` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| WSEX-01 | Exec variant serializes/deserializes correctly | unit | `cargo test -p rc-common -- protocol::tests::test_exec_roundtrip` | Wave 0 |
| WSEX-01 | CoreToAgentMessage::Exec wire format matches snake_case tag | unit | `cargo test -p rc-common -- protocol::tests::test_exec_wire_format` | Wave 0 |
| WSEX-02 | WS semaphore is independent from HTTP semaphore | unit | `cargo test -p rc-agent-crate -- tests::test_ws_exec_semaphore_independent` | Wave 0 |
| WSEX-03 | ExecResult includes all required fields and request_id correlation | unit | `cargo test -p rc-common -- protocol::tests::test_exec_result_roundtrip` | Wave 0 |
| WSEX-03 | ExecResult success and error variants both serialize | unit | `cargo test -p rc-common -- protocol::tests::test_exec_result_success_and_error` | Wave 0 |
| WSEX-04 | Deploy fallback attempts HTTP then WS | integration | Manual: deploy to pod with port 8090 blocked | manual-only (requires live pod) |

### Sampling Rate
- **Per task commit:** `cargo test -p rc-common && cargo test -p rc-agent-crate`
- **Per wave merge:** `cargo test -p rc-common && cargo test -p rc-agent-crate && cargo test -p racecontrol-crate`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `protocol::tests::test_exec_roundtrip` -- covers WSEX-01 serde
- [ ] `protocol::tests::test_exec_wire_format` -- covers WSEX-01 JSON format
- [ ] `protocol::tests::test_exec_result_roundtrip` -- covers WSEX-03 serde
- [ ] `protocol::tests::test_exec_result_success_and_error` -- covers WSEX-03 variants
- [ ] `protocol::tests::test_exec_default_timeout` -- covers default timeout_ms behavior

## Open Questions

1. **Stdout/stderr size limits for WebSocket messages**
   - What we know: HTTP returns full output without truncation. WebSocket frames can be large (axum/tungstenite defaults allow up to 64MB).
   - What's unclear: Should WS truncate output to prevent overloading the channel? Deploy commands typically produce small output (< 1KB).
   - Recommendation: Truncate to 64KB for safety. Log a warning if truncation occurs. Deploy commands (tasklist, curl, dir) produce tiny output so this is unlikely to hit.

2. **Should deploy.rs signature change?**
   - What we know: Current `exec_on_pod` takes `(state, pod_ip, cmd, timeout_ms)` but not `pod_id`. WS needs `pod_id` (key into `agent_senders`). Deploy functions already have both `pod_id` and `pod_ip` in scope.
   - What's unclear: Should we change the signature or create a new function?
   - Recommendation: Change `exec_on_pod` to accept `(state, pod_id, pod_ip, cmd, timeout_ms)`. All call sites already have both values.

3. **Cleanup of pending_ws_execs on agent disconnect**
   - What we know: Agent disconnect is handled in `ws/mod.rs` around line 500. Cleanup of `agent_senders` and `agent_conn_ids` already happens there.
   - What's unclear: How to identify which pending entries belong to a disconnecting pod.
   - Recommendation: Prefix request_id with pod_id (e.g., `pod_3:uuid-here`) so disconnect cleanup can filter by prefix. Simple and self-documenting.

## Sources

### Primary (HIGH confidence)
- **rc-common/src/protocol.rs** - Full protocol definition, 40+ serde roundtrip tests
- **rc-agent/src/remote_ops.rs** - HTTP implementation (lines 222-300), semaphore pattern, 7 tests
- **rc-agent/src/main.rs** - Agent WebSocket handler, CoreToAgentMessage match (lines 1017-1742)
- **racecontrol/src/ws/mod.rs** - Core WebSocket handler, agent_senders pattern, mpsc forwarding
- **racecontrol/src/deploy.rs** - Deploy executor, exec_on_pod (lines 172-205), 14 tests
- **racecontrol/src/state.rs** - AppState definition, agent_senders/agent_conn_ids (lines 69-119)

### Secondary (MEDIUM confidence)
- **tokio::sync::Semaphore** - API verified via crate docs (const_new, try_acquire)
- **tokio::sync::oneshot** - API verified via crate docs (channel, send, recv)

### Tertiary (LOW confidence)
- None

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - All libraries already in workspace, no new deps
- Architecture: HIGH - Direct extension of existing patterns with code references to exact lines
- Pitfalls: HIGH - Based on reading actual source code and understanding runtime behavior
- Validation: HIGH - Test infrastructure exists, 93+ protocol tests as template

**Research date:** 2026-03-15
**Valid until:** 2026-04-15 (stable codebase, no external dependency changes expected)
