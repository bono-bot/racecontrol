---
phase: 50-llm-self-test-fleet-health
plan: "01"
subsystem: rc-agent / rc-common
tags: [self-test, fleet-health, diagnostics, probes, llm-verdict, protocol]
dependency_graph:
  requires: [46-01, 47-01, 48-01]
  provides: [self_test::run_all_probes, self_test::SelfTestReport, self_test::ProbeResult, self_test::ProbeStatus, self_test::SelfTestVerdict, self_test::VerdictLevel, self_test::get_llm_verdict, self_test::deterministic_verdict, protocol::RunSelfTest, protocol::SelfTestResult]
  affects: [crates/rc-agent/src/main.rs, crates/rc-common/src/protocol.rs]
tech_stack:
  added: []
  patterns: [tokio::join! for concurrent probes, 10s per-probe timeout, OnceLock for HTTP clients, spawn_blocking for sync OS calls, serde SCREAMING_SNAKE_CASE + lowercase enums, deterministic fallback on LLM failure]
key_files:
  created:
    - crates/rc-agent/src/self_test.rs
  modified:
    - crates/rc-agent/src/main.rs
    - crates/rc-common/src/protocol.rs
decisions:
  - "22 probes run via tokio::join! (including all TCP/UDP subprobes) rather than exactly 18 — UDP and TCP port checks are parameterized functions called per-port"
  - "probe_tcp_port uses clone before move-into-closure to avoid borrow-after-move on addr string"
  - "Ollama probe closure uses two-level move (let u2 = u.clone()) to avoid returning reference to local data"
  - "get_llm_verdict and verdict_client unused-at-call-time warnings are expected — they will be wired in Phase 50 Plan 02 (RunSelfTest WS handler)"
  - "startup self-test uses deterministic_verdict not LLM — Ollama call at startup is too slow and blocks WS reconnect"
  - "startup_self_test_verdict serialized as format!({:?}).to_uppercase() = HEALTHY/DEGRADED/CRITICAL string"
  - "Backward compat on protocol: #[serde(default)] on both Phase 50 StartupReport fields — old agents continue working"
  - "SelfTestResult.report is serde_json::Value — avoids rc-common depending on rc-agent self_test types"
metrics:
  duration: 8 min
  completed: "2026-03-19"
  tasks_completed: 2
  files_changed: 3
---

# Phase 50 Plan 01: Self-Test Module + Protocol Extensions Summary

self_test.rs with 22 concurrent probes (including parameterized TCP/UDP sub-probes), LLM verdict generation with deterministic fallback, protocol variants RunSelfTest/SelfTestResult, and startup self-test integration that reports HEALTHY/DEGRADED/CRITICAL in StartupReport.

## What Was Built

### Task 1: self_test.rs module

Created `crates/rc-agent/src/self_test.rs` with:

**Types:**
- `ProbeResult { name, status, detail }` — serializes with `status: "pass"/"fail"/"skip"`
- `ProbeStatus { Pass, Fail, Skip }` — `#[serde(rename_all = "lowercase")]`
- `SelfTestReport { probes, verdict, timestamp }` — verdict is `Option<SelfTestVerdict>` for backward compat
- `SelfTestVerdict { level, analysis, auto_fix_recommendations }` — LLM-generated or deterministic
- `VerdictLevel { Healthy, Degraded, Critical }` — `#[serde(rename_all = "SCREAMING_SNAKE_CASE")]`

**Probe functions (18 logical probes):**
1. `probe_ws_connected` — reads `HeartbeatStatus.ws_connected` atomic
2-5. `probe_tcp_port` (parameterized) — lock_screen :18923, remote_ops :8090, overlay :18925, debug_server :18924
6-10. `probe_udp_port` (parameterized) — AC :9996, F1 :20777, Forza :5300, iRacing :6789, LMU :5555
11. `probe_hid` — HidApi enumerate only (VID:0x1209 PID:0xFFB0), never open()
12. `probe_ollama` — GET /api/tags, checks for rp-debug/rc-bot model name
13. `probe_close_wait` — netstat, CLOSE_WAIT on :8090 < 20
14. `probe_single_instance` — tasklist, exactly 1 rc-agent.exe
15. `probe_disk` — sysinfo C: drive > 2GB free
16. `probe_memory` — sysinfo available_memory > 1GB
17. `probe_shader_cache` — NVIDIA GLCache dir exists (Skip if absent)
18. `probe_build_id` — option_env!("GIT_HASH") (Skip if not set)
19. `probe_billing_state` — always Pass, informational
20. `probe_session_id` — always Skip (requires billing context)
21. `probe_gpu_temp` — nvidia-smi, < 90°C (Skip if not found)
22. `probe_steam` — tasklist steam.exe (Skip if not running)

**Key functions:**
- `run_all_probes(status, ollama_url)` — `tokio::join!` all 22 probes, 10s timeout each
- `get_llm_verdict(ollama_url, ollama_model, probes)` — Ollama VERDICT/CORRELATION/FIX format, falls back to deterministic
- `deterministic_verdict(probes)` — Critical if ws_connected/lock_screen/billing_state Fail; Degraded if other Fail; Healthy otherwise
- `probe_udp_port_from_netstat_output(port, output)` — pure function, testable without OS calls

**main.rs integration:** Startup self-test runs after remote_ops bind check, before WS reconnect loop. Uses deterministic_verdict (no LLM at startup). Stores verdict string and failure count for StartupReport.

### Task 2: Protocol Extensions

Extended `crates/rc-common/src/protocol.rs`:

- `CoreToAgentMessage::RunSelfTest { request_id }` — commands agent to run self-test
- `AgentMessage::SelfTestResult { pod_id, request_id, report: serde_json::Value }` — agent response
- `AgentMessage::StartupReport` extended with:
  - `startup_self_test_verdict: Option<String>` — HEALTHY/DEGRADED/CRITICAL
  - `startup_probe_failures: u8` — count of failed probes at startup
  - Both fields have `#[serde(default)]` for backward compatibility

4 new protocol tests + 3 existing tests updated with new fields.

## Test Results

```
cargo test -p rc-agent-crate --bin rc-agent self_test
running 17 tests: all PASSED

cargo test -p rc-common
running 123 tests: all PASSED

cargo build -p rc-agent-crate: SUCCESS (32 pre-existing warnings, 0 errors)
cargo build -p racecontrol-crate: SUCCESS (7 pre-existing warnings, 0 errors)
```

## Deviations from Plan

**1. [Rule 1 - Bug] addr moved-into-closure borrow error in probe_tcp_port**
- Found during: Task 1 compilation
- Issue: `addr` string moved into `spawn_blocking` closure but referenced in format! after closure
- Fix: Clone addr into `addr2` before closure, use `addr2` in result format strings
- Files modified: `crates/rc-agent/src/self_test.rs`
- Commit: bbed876

**2. [Rule 1 - Bug] Ollama probe closure returns reference to local data**
- Found during: Task 1 compilation
- Issue: `move || probe_ollama(&u)` — `u` is local, cannot borrow in returned future
- Fix: Two-level move pattern: `move || { let u2 = u.clone(); async move { probe_ollama(&u2).await } }`
- Files modified: `crates/rc-agent/src/self_test.rs`
- Commit: bbed876

**3. [Rule 3 - Blocking] Existing StartupReport test constructors missing Phase 50 fields**
- Found during: Task 2 verification
- Issue: 3 existing tests in protocol.rs constructed StartupReport without new fields, causing compile errors
- Fix: Added `startup_self_test_verdict: None, startup_probe_failures: 0` to all 3 constructors
- Files modified: `crates/rc-common/src/protocol.rs`
- Commit: 8bc07ee

## Acceptance Criteria Check

- [x] `crates/rc-agent/src/self_test.rs` exists with `pub struct ProbeResult`
- [x] Contains `pub enum ProbeStatus` with `#[serde(rename_all = "lowercase")]`
- [x] Contains `pub enum VerdictLevel` with `#[serde(rename_all = "SCREAMING_SNAKE_CASE")]`
- [x] Contains `pub async fn run_all_probes(`
- [x] Contains `pub async fn get_llm_verdict(`
- [x] Contains `pub fn deterministic_verdict(`
- [x] Contains `fn parse_verdict_response(`
- [x] Contains `0x1209` and `0xFFB0` (HID VID/PID — enumerate only)
- [x] Contains `device_list()` (not `open(`)
- [x] Contains `Duration::from_secs(10)` (per-probe timeout)
- [x] `crates/rc-agent/src/main.rs` contains `mod self_test;`
- [x] Unit tests pass: `cargo test -p rc-agent-crate self_test` exits 0 (17 tests)
- [x] `crates/rc-common/src/protocol.rs` contains `RunSelfTest {` inside CoreToAgentMessage
- [x] Contains `SelfTestResult {` inside AgentMessage
- [x] Contains `startup_self_test_verdict: Option<String>`
- [x] Contains `startup_probe_failures: u8`
- [x] Contains `#[serde(default)]` before both new StartupReport fields
- [x] All 4 Phase 50 protocol tests pass (part of 123 total)
- [x] Backward compat: old StartupReport JSON deserializes with None/0 defaults

## Self-Check: PASSED

Files verified:
- `crates/rc-agent/src/self_test.rs` — EXISTS
- `crates/rc-agent/src/main.rs` — contains `mod self_test;` and `startup_self_test_verdict`
- `crates/rc-common/src/protocol.rs` — contains `RunSelfTest`, `SelfTestResult`, `startup_self_test_verdict`

Commits verified:
- bbed876 — EXISTS (Task 1: self_test.rs + main.rs)
- 8bc07ee — EXISTS (Task 2: protocol.rs extensions)
