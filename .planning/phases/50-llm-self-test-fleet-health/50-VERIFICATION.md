---
phase: 50-llm-self-test-fleet-health
verified: 2026-03-19T05:30:00+05:30
status: passed
score: 6/6 must-haves verified
re_verification: false
---

# Phase 50: LLM Self-Test + Fleet Health Verification Report

**Phase Goal:** rc-agent runs 18 deterministic self-test probes at startup and on-demand (WS, lock screen, remote ops, overlay, debug server, 5 UDP ports, HID, Ollama, CLOSE_WAIT, single instance, disk, memory, shader cache, build_id, billing state, session ID, GPU temp, Steam), feeds results to local LLM for a HEALTHY/DEGRADED/CRITICAL verdict with correlation analysis and auto-fix recommendations, server exposes /api/v1/pods/{id}/self-test endpoint for fleet-wide health checks, and auto-fix patterns 8-14 are wired into ai_debugger.rs (DirectX reset, shader cache clear, memory pressure, DLL repair, Steam restart, performance throttle, network adapter reset)
**Verified:** 2026-03-19T05:30:00+05:30 (IST)
**Status:** PASSED
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | rc-agent runs 18+ deterministic self-test probes without panicking or hanging | VERIFIED | `self_test.rs` runs 22 probes via `tokio::join!`, each wrapped in `timed_probe()` with `Duration::from_secs(10)` timeout. All 18 logical probe functions present plus parameterized sub-probes for TCP/UDP ports. |
| 2 | Each probe returns pass/fail/skip with a human-readable detail string within 10s | VERIFIED | `timed_probe()` wrapper enforces 10s timeout, returns `Fail` with "probe timed out after 10s" on timeout. All probe functions return `ProbeResult { name, status, detail }`. |
| 3 | LLM verdict generation returns HEALTHY/DEGRADED/CRITICAL with deterministic fallback | VERIFIED | `get_llm_verdict()` calls Ollama, `parse_verdict_response()` extracts VERDICT line. On Ollama failure, `deterministic_verdict()` is called. `VerdictLevel` serializes as `SCREAMING_SNAKE_CASE`. 17 unit tests covering serde, parse, and deterministic fallback all pass. |
| 4 | Startup self-test runs after port binds and includes verdict in StartupReport | VERIFIED | `main.rs:830` calls `self_test::run_all_probes()` in "Phase 50: Startup Self-Test" section after port binding. `StartupReport` carries `startup_self_test_verdict: Option<String>` and `startup_probe_failures: u8` (lines 124-127 in protocol.rs). |
| 5 | GET /api/v1/pods/{id}/self-test dispatches RunSelfTest via WS and returns probe results + LLM verdict within 30s | VERIFIED | Route registered at `routes.rs:51`. `pod_self_test` handler (line 757) sends `CoreToAgentMessage::RunSelfTest`, awaits oneshot with `Duration::from_secs(30)` timeout (line 794). `ws/mod.rs:684` resolves oneshot on `SelfTestResult`. |
| 6 | Auto-fix patterns 8-14 are wired into ai_debugger.rs and trigger on correct keywords | VERIFIED | Six fix functions implemented at lines 754-935: `fix_directx_shader_cache`, `fix_memory_pressure`, `fix_dll_repair`, `fix_steam_restart`, `fix_performance_throttle`, `fix_network_adapter_reset`. Six match arms wired at lines 486-516 in `try_auto_fix()`. |

**Score:** 6/6 truths verified

---

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/rc-agent/src/self_test.rs` | 18 probe functions + SelfTestReport + LLM verdict + run_all_probes() | VERIFIED | 1046 lines. All required exports present: `ProbeResult`, `ProbeStatus`, `SelfTestReport`, `SelfTestVerdict`, `VerdictLevel`, `run_all_probes`, `get_llm_verdict`, `deterministic_verdict`, `probe_udp_port_from_netstat_output`. 22 probes in `tokio::join!`. 17 unit tests. |
| `crates/rc-common/src/protocol.rs` | RunSelfTest + SelfTestResult WS message variants + StartupReport Phase 50 fields | VERIFIED | `RunSelfTest { request_id }` in `CoreToAgentMessage` (line 396). `SelfTestResult { pod_id, request_id, report }` in `AgentMessage` (line 204). `startup_self_test_verdict: Option<String>` and `startup_probe_failures: u8` in `StartupReport` (lines 124-127) with `#[serde(default)]`. 4 Phase 50 protocol tests present. |
| `crates/rc-agent/src/main.rs` | mod self_test declaration + startup self-test call + RunSelfTest handler | VERIFIED | `mod self_test;` at line 17. Startup call at lines 830-845. `CoreToAgentMessage::RunSelfTest` handler at lines 2638-2663 spawns `run_all_probes` + `get_llm_verdict` and sends `SelfTestResult` via `ws_exec_result_tx`. |
| `crates/rc-agent/src/ai_debugger.rs` | Auto-fix patterns 8-14: 6 fix functions + 6 match arms | VERIFIED | `fix_directx_shader_cache` (line 754), `fix_memory_pressure` (line 786), `fix_dll_repair` (line 831), `fix_steam_restart` (line 850), `fix_performance_throttle` (line 868), `fix_network_adapter_reset` (line 895). All 6 match arms in `try_auto_fix()` at lines 486-516. High Performance GUID `8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c` present. |
| `crates/racecontrol/src/state.rs` | pending_self_tests field in AppState | VERIFIED | `pub pending_self_tests: RwLock<HashMap<String, (String, tokio::sync::oneshot::Sender<serde_json::Value>)>>` at line 144. Initialized with `RwLock::new(HashMap::new())` at line 199. |
| `crates/racecontrol/src/api/routes.rs` | GET /api/v1/pods/{id}/self-test handler | VERIFIED | `.route("/pods/{id}/self-test", get(pod_self_test))` at line 51. `async fn pod_self_test(` at line 757. `CoreToAgentMessage::RunSelfTest` dispatch at line 784. 30s timeout at line 794. |
| `crates/racecontrol/src/ws/mod.rs` | SelfTestResult handler + disconnect cleanup | VERIFIED | `AgentMessage::SelfTestResult` match arm at line 679 resolves `pending_self_tests`. Disconnect cleanup uses `pending.retain(|_req_id, (pid, _tx)| pid != pod_id)` at line 744. |
| `tests/e2e/fleet/pod-health.sh` | Fleet-wide self-test E2E verification | VERIFIED | 92 lines. Sources `common.sh` + `pod-map.sh`. Calls `GET /api/v1/pods/{id}/self-test` with `SELFTEST_TIMEOUT=35`. 3-gate check: reachable, HTTP 200, HEALTHY verdict. Logs failed probe names on DEGRADED. Ends with `summary_exit`. |
| `tests/e2e/run-all.sh` | pod-health.sh wired as final phase gate | VERIFIED | `run_phase "fleet-health"` calls `pod-health.sh` at line 168. `fleet_health` key in summary JSON at line 213. Skip path when deploy skipped at line 179. |

---

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `self_test.rs` | `udp_heartbeat.rs` | `HeartbeatStatus.ws_connected.load(Ordering::Relaxed)` | WIRED | `probe_ws_connected` at line 83 reads `status.ws_connected.load(Ordering::Relaxed)`. `probe_billing_state` reads `billing_active`. |
| `self_test.rs` | Ollama | `query_ollama_for_verdict` | WIRED | `get_llm_verdict` calls `query_ollama_for_verdict()` at line 728. Falls back to `deterministic_verdict()` on failure. |
| `main.rs` | `self_test.rs` | `self_test::run_all_probes` at startup | WIRED | `self_test::run_all_probes(heartbeat_status.clone(), ...)` at line 830. |
| `main.rs` | `self_test.rs` | `RunSelfTest` handler calls `run_all_probes` + `get_llm_verdict` | WIRED | Lines 2646-2650 spawn async task calling both functions, stores `report.verdict = Some(verdict)`, sends `SelfTestResult` via `result_tx`. |
| `routes.rs` | `state.rs` | `pending_self_tests` oneshot channel | WIRED | `state.pending_self_tests.write().await` at lines 779, 785, 801. |
| `ws/mod.rs` | `state.rs` | `pending_self_tests.remove()` on SelfTestResult | WIRED | `pending.remove(request_id.as_str())` at line 685. |
| `ws/mod.rs` | `state.rs` | `pending_self_tests.retain()` on disconnect | WIRED | `pending.retain(|_req_id, (pid, _tx)| pid != pod_id)` at line 744. |
| `ai_debugger.rs` | `try_auto_fix` match arms | All 6 fix functions reachable from `try_auto_fix()` | WIRED | 6 match arms at lines 486-516 return `Some(fix_*())`. `None` falls through. False-positive guard tested. |
| `pod-health.sh` | `routes.rs` | `curl GET /api/v1/pods/{pod_id}/self-test` | WIRED | Line 42: `"${SERVER_URL}/pods/${POD_ID}/self-test"` — correct path matching route registration. |
| `run-all.sh` | `pod-health.sh` | `run_phase "fleet-health" bash fleet/pod-health.sh` | WIRED | Line 168: `run_phase "fleet-health" bash "$SCRIPT_DIR/fleet/pod-health.sh"`. |

---

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| SELFTEST-01 | 50-01 | self_test.rs module with 18 deterministic probes — pass/fail/skip with detail string, 10s timeout | SATISFIED | 22 probes in `self_test.rs` (including all 18 logical probes + parameterized sub-probes). `timed_probe()` enforces 10s. All probe functions return typed `ProbeResult`. |
| SELFTEST-02 | 50-01 | Local LLM verdict — HEALTHY/DEGRADED/CRITICAL with correlation analysis + auto-fix recommendations | SATISFIED | `get_llm_verdict()` queries Ollama with structured prompt. `parse_verdict_response()` extracts VERDICT/CORRELATION/FIX. `deterministic_verdict()` as fallback. `SelfTestVerdict` carries `level`, `analysis`, `auto_fix_recommendations`. |
| SELFTEST-03 | 50-03 | Server endpoint GET /api/v1/pods/{id}/self-test — 30s timeout, returns full probe results + LLM verdict | SATISFIED | Route at `routes.rs:51`. Handler dispatches `RunSelfTest` via WS, awaits oneshot with 30s timeout. Agent runs probes + LLM verdict and returns `SelfTestResult`. |
| SELFTEST-04 | 50-02 | Auto-fix patterns 8-14 in ai_debugger.rs — DirectX, memory, DLL, Steam, performance, network | SATISFIED | 6 fix functions (`fix_directx_shader_cache` through `fix_network_adapter_reset`) at lines 754-935. 6 match arms in `try_auto_fix()`. Uses `hidden_cmd()` for `CREATE_NO_WINDOW`. |
| SELFTEST-05 | 50-03 | E2E test pod-health.sh — trigger self-test on all 8 pods, assert HEALTHY, wired into run-all.sh | SATISFIED | `tests/e2e/fleet/pod-health.sh` exists with 3-gate check. `run-all.sh` has Phase 5 Fleet Health gate. `bash -n` syntax check confirmed by SUMMARY. |
| SELFTEST-06 | 50-01 | Self-test at rc-agent startup and on-demand — startup results in BootVerification/StartupReport | SATISFIED | Startup probe call at `main.rs:830-845`, verdict stored in `startup_self_test_verdict`. On-demand via `CoreToAgentMessage::RunSelfTest` at `main.rs:2638`. `StartupReport` carries both new Phase 50 fields. |

All 6 requirements satisfied. No orphaned requirements.

---

### Anti-Patterns Found

No blockers or warnings found during scan.

| File | Pattern | Severity | Notes |
|------|---------|----------|-------|
| `self_test.rs` | `probe_session_id` always returns `Skip` | INFO | Intentional — documented in plan as "Session ID requires billing context not accessible here". Not a stub; design choice. |
| `self_test.rs` | `probe_billing_state` always returns `Pass` | INFO | Intentional — billing state is informational, always passes. Detail string carries the actual value. |

No `TODO`, `FIXME`, `placeholder` comments found in Phase 50 files. No empty handlers. No static/fake returns.

---

### Human Verification Required

#### 1. Live Fleet Self-Test Execution

**Test:** With all 8 pods running rc-agent, run `bash tests/e2e/fleet/pod-health.sh` or `curl http://192.168.31.23:8080/api/v1/pods/pod-1/self-test`
**Expected:** JSON response with 22 probe results and a `verdict.level` of `"HEALTHY"` within 30s
**Why human:** Requires live pods, WS connections, and Ollama running on each pod. Cannot verify probe execution paths without running hardware.

#### 2. LLM Verdict Quality

**Test:** Trigger a self-test on a pod where some probes fail (e.g., a pod with a disconnected wheelbase)
**Expected:** LLM verdict correctly labels it `DEGRADED` or `CRITICAL` with a meaningful CORRELATION analysis
**Why human:** LLM response quality (correlation accuracy, recommendation usefulness) cannot be verified programmatically.

#### 3. Auto-fix Pattern 8 (DirectX Shader Cache Clear)

**Test:** Inject keyword "DirectX error on shader compile" into the AI debug path, observe `fix_directx_shader_cache` execution on a pod
**Expected:** `AutoFixResult` with `fix_type="directx_shader_cache"`, NVIDIA GLCache directories cleared
**Why human:** Actual filesystem mutation requires live pod with NVIDIA GPU. Cannot verify directory removal without running hardware.

---

### Gaps Summary

No gaps found. All 6 requirements are satisfied, all 9 required artifacts exist and are substantive, and all 10 key links are wired. The implementation exceeds the minimum 18-probe requirement by adding 4 additional sub-probes (22 total), which is a deliberate and documented design decision.

---

## Verification Notes

1. **22 vs 18 probes:** The plan specified 18 logical probes. The implementation runs 22 via `tokio::join!` because `probe_tcp_port` and `probe_udp_port` are parameterized and called once per port. The SUMMARY correctly documents this as "22 concurrent probes including parameterized TCP/UDP sub-probes." All 18 logical probe categories from SELFTEST-01 are covered.

2. **Protocol backward compatibility:** Verified — old `StartupReport` JSON without Phase 50 fields deserializes with `startup_self_test_verdict: None` and `startup_probe_failures: 0` via `#[serde(default)]`. Test `test_startup_report_phase50_backward_compat` in protocol.rs covers this.

3. **WS send path:** The `RunSelfTest` handler in `main.rs` correctly sends `SelfTestResult` via `ws_exec_result_tx` (the mpsc channel that feeds `ws_tx`) rather than `ws_tx` directly. This is correct because `SplitSink` is not `Clone` — the mpsc/select loop pattern is the established pattern for this codebase.

4. **Auto-fix uses `hidden_cmd()`:** The SUMMARY notes all 6 new fix functions use `hidden_cmd()` instead of the plan's `std::process::Command::new()`. This is an improvement — `hidden_cmd()` adds `CREATE_NO_WINDOW` on Windows, preventing console flashes on pods. The functional behavior is identical.

---

_Verified: 2026-03-19T05:30:00+05:30 (IST)_
_Verifier: Claude (gsd-verifier)_
