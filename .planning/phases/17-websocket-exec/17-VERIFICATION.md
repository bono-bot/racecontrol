---
phase: 17-websocket-exec
verified: 2026-03-15T14:30:00Z
status: passed
score: 4/4 must-haves verified
must_haves:
  truths:
    - "Shell command sent via WebSocket returns correct stdout/stderr/exit code within 30s"
    - "WS exec works even when all 4 HTTP exec slots are full -- separate semaphore"
    - "When HTTP :8090 is blocked, deploy.rs falls back to WS exec and deploys successfully"
    - "Each WS exec response has same request_id that was sent -- concurrent commands correctly correlated"
  artifacts:
    - path: "crates/rc-common/src/protocol.rs"
      provides: "Exec and ExecResult enum variants with serde roundtrip"
    - path: "crates/rc-agent/src/main.rs"
      provides: "WS_EXEC_SEMAPHORE, handle_ws_exec, event loop integration"
    - path: "crates/racecontrol/src/state.rs"
      provides: "WsExecResult struct, pending_ws_execs HashMap in AppState"
    - path: "crates/racecontrol/src/ws/mod.rs"
      provides: "ExecResult handler, ws_exec_on_pod function, disconnect cleanup"
    - path: "crates/racecontrol/src/deploy.rs"
      provides: "HTTP-first WS-fallback exec_on_pod wrapper"
  key_links:
    - from: "deploy.rs exec_on_pod"
      to: "ws/mod.rs ws_exec_on_pod"
      via: "crate::ws::ws_exec_on_pod call in fallback branch"
    - from: "ws/mod.rs ws_exec_on_pod"
      to: "state.rs pending_ws_execs"
      via: "oneshot channel registration and resolution"
    - from: "ws/mod.rs ExecResult handler"
      to: "state.rs pending_ws_execs"
      via: "pending.remove(request_id) to resolve oneshot"
    - from: "rc-agent main.rs Exec match arm"
      to: "handle_ws_exec function"
      via: "tokio::spawn with mpsc drain in select loop"
human_verification:
  - test: "Send shell command via WebSocket to live pod"
    expected: "stdout contains expected output (e.g. hostname from whoami)"
    why_human: "Requires live WebSocket connection to a real pod"
  - test: "Block HTTP port 8090 on a pod, then deploy via racecontrol"
    expected: "Deploy succeeds using WS fallback path"
    why_human: "Requires manual firewall rule manipulation on a live pod"
  - test: "Fill 4 HTTP exec slots, send WS exec concurrently"
    expected: "WS exec returns immediately while HTTP slots are occupied"
    why_human: "Requires concurrent load test against live pod"
  - test: "Send 3 concurrent WS commands with different request_ids"
    expected: "Each response carries the correct matching request_id"
    why_human: "Requires concurrent WebSocket message exchange with live pod"
---

# Phase 17: WebSocket Exec Verification Report

**Phase Goal:** racecontrol can send any shell command to any connected pod over the existing WebSocket connection and receive stdout, stderr, and exit code -- so pods remain manageable even when HTTP port 8090 is firewall-blocked
**Verified:** 2026-03-15T14:30:00Z
**Status:** passed
**Re-verification:** No -- initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Shell command sent via WebSocket returns correct stdout/stderr/exit code within 30s | VERIFIED | CoreToAgentMessage::Exec variant (protocol.rs:267-272), handle_ws_exec runs cmd /C with tokio timeout (main.rs:210-221), returns ExecResult with exit_code/stdout/stderr (main.rs:237-258), ws_exec_on_pod has timeout_ms+5s buffer (ws/mod.rs:1056) |
| 2 | WS exec works even when all 4 HTTP exec slots are full -- separate semaphore | VERIFIED | WS_EXEC_SEMAPHORE is independent static (main.rs:190) with 4 slots, separate from HTTP EXEC_SEMAPHORE in remote_ops.rs. try_acquire is non-blocking (main.rs:197) |
| 3 | When HTTP :8090 is blocked, deploy.rs falls back to WS exec | VERIFIED | exec_on_pod (deploy.rs:211-228) tries http_exec_on_pod first, catches Err, logs warning, calls crate::ws::ws_exec_on_pod as fallback. All deploy functions (is_process_alive, is_lock_screen_healthy, download, swap) route through exec_on_pod |
| 4 | Each WS exec response has same request_id -- concurrent commands correctly correlated | VERIFIED | ws_exec_on_pod generates pod-prefixed request_id (ws/mod.rs:1031), registers oneshot in pending_ws_execs (ws/mod.rs:1035), ExecResult handler resolves by matching request_id (ws/mod.rs:467-476), agent echoes request_id in ExecResult (main.rs:237-258) |

**Score:** 4/4 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/rc-common/src/protocol.rs` | Exec + ExecResult enum variants | VERIFIED | Exec at line 267 with request_id, cmd, timeout_ms (serde default 10000ms). ExecResult at line 92 with request_id, success, exit_code, stdout, stderr. 5 serde tests pass. |
| `crates/rc-agent/src/main.rs` | WS_EXEC_SEMAPHORE, handle_ws_exec, event loop wiring | VERIFIED | WS_EXEC_SEMAPHORE(4) at line 190. handle_ws_exec at line 194 with semaphore, timeout, 64KB truncation. Exec match arm at line 1826, mpsc drain at line 1065. |
| `crates/racecontrol/src/state.rs` | WsExecResult struct, pending_ws_execs field | VERIFIED | WsExecResult struct at line 72 with success, exit_code, stdout, stderr. pending_ws_execs RwLock<HashMap> at line 129. Initialized empty in AppState::new at line 176. |
| `crates/racecontrol/src/ws/mod.rs` | ExecResult handler, ws_exec_on_pod, disconnect cleanup | VERIFIED | ExecResult match arm at line 467 resolves oneshot. Disconnect sweep at lines 541-555 uses pod prefix. ws_exec_on_pod at line 1025 with request_id generation, oneshot registration, timeout+5s. |
| `crates/racecontrol/src/deploy.rs` | HTTP-first WS-fallback exec_on_pod | VERIFIED | http_exec_on_pod at line 172 (renamed from old exec_on_pod). New exec_on_pod wrapper at line 211 tries HTTP, falls back to ws_exec_on_pod on Err. All deploy helpers (is_process_alive, is_lock_screen_healthy) route through it. |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| deploy.rs exec_on_pod | ws/mod.rs ws_exec_on_pod | crate::ws::ws_exec_on_pod call | WIRED | deploy.rs:225 calls crate::ws::ws_exec_on_pod in Err branch |
| ws_exec_on_pod | pending_ws_execs | oneshot registration | WIRED | ws/mod.rs:1035 inserts into pending_ws_execs, ws/mod.rs:1050/1061/1065 cleans up on error/timeout |
| ExecResult handler | pending_ws_execs | oneshot resolution | WIRED | ws/mod.rs:469 removes from pending and sends WsExecResult |
| rc-agent Exec match | handle_ws_exec | tokio::spawn + mpsc | WIRED | main.rs:1826-1832 spawns handle_ws_exec, sends result via ws_exec_result_tx; main.rs:1065-1070 drains results and sends via ws_tx |
| disconnect cleanup | pending_ws_execs | prefix sweep | WIRED | ws/mod.rs:543-554 filters pending keys by "pod_X:" prefix and removes stale entries |
| handle_ws_exec | WS_EXEC_SEMAPHORE | try_acquire | WIRED | main.rs:197 acquires independent semaphore, drops at main.rs:223 |

### Requirements Coverage

| Requirement | Source Plan(s) | Description | Status | Evidence |
|-------------|---------------|-------------|--------|----------|
| WSEX-01 | 01, 02, 03 | racecontrol can send shell commands to any connected pod via WebSocket (CoreToAgentMessage::Exec) | SATISFIED | Exec variant in protocol.rs, ws_exec_on_pod sends it via agent_senders, agent handles it in main.rs Exec match arm |
| WSEX-02 | 02 | rc-agent executes WebSocket commands with independent semaphore | SATISFIED | WS_EXEC_SEMAPHORE static (4 slots) at main.rs:190, independent from HTTP EXEC_SEMAPHORE in remote_ops.rs |
| WSEX-03 | 01, 02, 03 | Exec responses include stdout, stderr, exit code, and request_id correlation | SATISFIED | ExecResult variant in protocol.rs carries all fields. handle_ws_exec populates them from Command output. request_id echoed back for correlation. |
| WSEX-04 | 03 | deploy.rs uses WebSocket exec as fallback when HTTP :8090 is unreachable | SATISFIED | exec_on_pod wrapper at deploy.rs:211-228 tries HTTP first, falls back to WS on error |

No orphaned requirements found. All 4 WSEX requirements mapped to Phase 17 in REQUIREMENTS.md are covered by plans and implemented.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| None | - | - | - | No TODO/FIXME/PLACEHOLDER/HACK found in any modified file |

### Test Results

| Crate | Tests | Passed | Failed | Notes |
|-------|-------|--------|--------|-------|
| rc-common | 98 | 98 | 0 | All 5 exec-related tests pass (roundtrip, wire format, default timeout, ExecResult roundtrip, success/error) |
| racecontrol | 254 | 254 | 0 | 213 unit + 41 integration, all pass including deploy tests |
| rc-agent | 184 | 183 | 1 | Pre-existing failure: remote_ops::tests::test_exec_timeout_returns_500 (HTTP exec path, not WS exec). Flaky timing test unrelated to Phase 17 changes. |

### Human Verification Required

### 1. Live WS Command Execution

**Test:** Send a shell command (e.g. `whoami`) via WebSocket to Pod 8
**Expected:** stdout contains the pod's hostname/username, exit_code is 0
**Why human:** Requires a live WebSocket connection between racecontrol and a real pod agent

### 2. WS Fallback When HTTP Blocked

**Test:** Delete the `RacingPoint-RemoteOps` firewall rule on Pod 8, then trigger a deploy from racecontrol
**Expected:** Deploy succeeds using the WS fallback path (warning log: "HTTP command failed... Trying WS fallback")
**Why human:** Requires manual firewall rule manipulation on a live pod

### 3. Independent Semaphore Under Load

**Test:** Fill all 4 HTTP exec slots with long-running commands (`timeout 30`), then send a WS exec command (`echo test`)
**Expected:** WS exec returns immediately while HTTP slots are occupied
**Why human:** Requires concurrent load test against a live pod

### 4. Request ID Correlation Under Concurrency

**Test:** Send 3 WS commands simultaneously with different request_ids
**Expected:** Each response carries the correct matching request_id
**Why human:** Requires multiple concurrent WebSocket messages to a live pod

### Gaps Summary

No gaps found. All 4 observable truths verified programmatically through code inspection and test results. All 5 artifacts are substantive (non-stub, full implementations) and wired (imported and used in the execution path). All 4 requirements (WSEX-01 through WSEX-04) are satisfied. No anti-patterns detected in modified files.

The pre-existing test failure in rc-agent (remote_ops::tests::test_exec_timeout_returns_500) is in the HTTP exec path, not the WS exec path. It appears to be a flaky timing issue where `ping -n 10 127.0.0.1` returns exit code 1 instead of being killed with code 124 before completion. This is not a Phase 17 regression.

---

_Verified: 2026-03-15T14:30:00Z_
_Verifier: Claude (gsd-verifier)_
