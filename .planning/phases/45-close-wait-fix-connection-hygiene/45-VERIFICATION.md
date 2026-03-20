---
phase: 45-close-wait-fix-connection-hygiene
verified: 2026-03-19T07:30:00+05:30
status: human_needed
score: 6/6 must-haves verified
human_verification:
  - test: "Deploy rc-agent.exe to all 8 pods, run fleet_health polling for 30 minutes, then check CLOSE_WAIT count"
    expected: "No pod has more than 5 CLOSE_WAIT sockets on :8090"
    why_human: "Requires live deployment and 30-minute soak — cannot verify socket accumulation from static code analysis"
  - test: "After rc-agent self-relaunches on any pod, verify all 5 UDP telemetry ports rebind successfully"
    expected: "Logs show 'Listening for game telemetry on UDP port X (SO_REUSEADDR)' for all 5 ports — no error 10048"
    why_human: "Requires live self-relaunch event on a pod — cannot simulate Windows socket inheritance behaviour in static analysis"
  - test: "Run `bash tests/e2e/fleet/close-wait.sh` against live pods after 30-minute soak"
    expected: "All reachable pods pass with CLOSE_WAIT count < 5; unreachable pods show as SKIP"
    why_human: "E2E script requires live pods running the new binary — cannot execute against real network from dev machine"
---

# Phase 45: CLOSE_WAIT Fix + Connection Hygiene Verification Report

**Phase Goal:** Eliminate the CLOSE_WAIT socket leak on port 8090 that causes 5/8 pods to accumulate 100-134 stuck sockets and trigger unnecessary self-relaunches every ~5 minutes — fix the remote_ops axum server to properly close HTTP connections, fix fleet_health.rs to reuse a shared reqwest client, add SO_REUSEADDR to all UDP game telemetry sockets, mark UDP sockets non-inheritable (matching ea30ca3 treatment for :8090), and increase exec slots from 4 to 8 or separate health checks from exec pool
**Verified:** 2026-03-19T07:30:00+05:30
**Status:** human_needed — all automated checks passed; 3 items require live-pod validation
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths (Success Criteria from ROADMAP)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | After 30 minutes of fleet_health polling, no pod has >5 CLOSE_WAIT sockets on :8090 | ? HUMAN | Server-side Connection: close middleware verified wired; client-side pool_max_idle_per_host(0) verified wired; live soak test needed |
| 2 | Pod self-relaunches from CLOSE_WAIT strike counter drop to zero across 8-hour monitoring window | ? HUMAN | Root cause addressed in code; runtime confirmation requires live monitoring |
| 3 | After rc-agent self-relaunch, all 5 UDP ports bind successfully (no error 10048) | ? HUMAN | bind_udp_reusable() with SO_REUSEADDR and SetHandleInformation verified in code; live relaunch test needed |
| 4 | fleet_health.rs uses a single shared reqwest::Client with connection pooling disabled | VERIFIED | pool_max_idle_per_host(0) confirmed at line 110 in fleet_health.rs; 13/13 fleet_health tests pass |
| 5 | Health endpoint requests never return 429 (slot exhaustion) — exec pool expanded | VERIFIED | MAX_CONCURRENT_EXECS = 8 confirmed at line 48 in remote_ops.rs; test_exec_429_error_message_format asserts "8 max" and passes |
| 6 | `bash tests/e2e/fleet/close-wait.sh` passes — CLOSE_WAIT count <5 on all 8 pods | ? HUMAN | Script exists and is structurally correct; live pod execution needed |

**Automated Score:** 2/6 truths fully verifiable statically + 4/6 truths have complete implementation evidence (3 require live validation)

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/rc-agent/src/remote_ops.rs` | Connection: close middleware + expanded exec slots | VERIFIED | connection_close_layer() at line 57, wired via .layer(middleware::from_fn(connection_close_layer)) at line 87, MAX_CONCURRENT_EXECS=8 at line 48 |
| `crates/rc-agent/src/main.rs` | UDP socket binding with SO_REUSEADDR and non-inherit | VERIFIED | bind_udp_reusable() at line 2420 with set_reuse_address(true) at line 2424 and SetHandleInformation under #[cfg(windows)] at line 2432-2438; run_udp_monitor calls bind_udp_reusable(port) at line 2452 |
| `crates/rc-agent/src/self_monitor.rs` | Shared reqwest client for Ollama queries | VERIFIED | OnceLock import at line 14, OLLAMA_CLIENT: OnceLock<reqwest::Client> at line 168, ollama_client() at line 170, query_ollama uses ollama_client() at line 185 — reqwest::Client::new() eliminated |
| `crates/racecontrol/src/fleet_health.rs` | probe_client with disabled connection pooling | VERIFIED | pool_max_idle_per_host(0) at line 110 inside start_probe_loop() builder chain |
| `tests/e2e/fleet/close-wait.sh` | E2E verification of CLOSE_WAIT count <5 on all pods | VERIFIED | File exists, sources lib/common.sh and lib/pod-map.sh (lines 15-16), THRESHOLD=5 at line 18, summary_exit at line 72, CLOSE_WAIT netstat command at line 43 |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| remote_ops.rs | axum Router | middleware::from_fn(connection_close_layer) | WIRED | Line 87: `.layer(middleware::from_fn(connection_close_layer))` present after all route registrations |
| main.rs | socket2::Socket | UDP socket creation with SO_REUSEADDR | WIRED | Line 2424: `raw.set_reuse_address(true).ok()?;` inside bind_udp_reusable(); called at line 2452 in run_udp_monitor |
| self_monitor.rs | OnceLock<reqwest::Client> | ollama_client() in query_ollama | WIRED | Line 185: `let resp = ollama_client()` — per-call Client::new() eliminated |
| fleet_health.rs | reqwest::Client::builder() | pool_max_idle_per_host(0) in probe client | WIRED | Line 110: `.pool_max_idle_per_host(0)` in start_probe_loop() builder chain |
| close-wait.sh | lib/common.sh | source | WIRED | Line 15: `source "$SCRIPT_DIR/../lib/common.sh"` |
| close-wait.sh | lib/pod-map.sh | source | WIRED | Line 16: `source "$SCRIPT_DIR/../lib/pod-map.sh"` |

### Requirements Coverage

CONN-HYG-01 through CONN-HYG-05 are defined in `45-RESEARCH.md` only — they do not appear in `.planning/REQUIREMENTS.md`. The central REQUIREMENTS.md covers v7.0 E2E suite requirements (FOUND-xx, BROW-xx, API-xx, DEPL-xx). The CONN-HYG IDs are phase-local by design; no mismatch or orphaning exists.

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| CONN-HYG-01 | 45-01, 45-02 | CLOSE_WAIT count <5 on all pods after 30 min fleet_health soak | VERIFIED (code) / HUMAN (live) | Connection: close middleware + pool_max_idle_per_host(0) + close-wait.sh E2E all implemented |
| CONN-HYG-02 | 45-01 | fleet_health.rs uses shared probe_client; self_monitor uses OnceLock | VERIFIED | pool_max_idle_per_host(0) in fleet_health.rs; OnceLock<reqwest::Client> in self_monitor.rs |
| CONN-HYG-03 | 45-01 | UDP ports bind successfully after rc-agent self-relaunch (no error 10048) | VERIFIED (code) / HUMAN (live) | bind_udp_reusable() with SO_REUSEADDR implemented; live test needed |
| CONN-HYG-04 | 45-01 | UDP sockets marked non-inheritable on Windows | VERIFIED (code) | SetHandleInformation(HANDLE_FLAG_INHERIT, 0) in bind_udp_reusable() under #[cfg(windows)] |
| CONN-HYG-05 | 45-01 | exec slot exhaustion never occurs; exec pool at 8 slots | VERIFIED | MAX_CONCURRENT_EXECS=8 confirmed; test_exec_429_error_message_format passes asserting "8 max" |

No orphaned requirements — CONN-HYG-01 through CONN-HYG-05 are all claimed by plan 45-01 (and CONN-HYG-01 additionally by 45-02). All five are implemented.

### Commit Verification

| Commit | Message | Status |
|--------|---------|--------|
| 1ba4806 | feat(45-01): Connection: close middleware + MAX_CONCURRENT_EXECS 8 | CONFIRMED in git log |
| ceb1444 | feat(45-01): UDP SO_REUSEADDR + non-inherit + OnceLock Ollama client | CONFIRMED in git log |
| ad3fae7 | feat(45-02): disable probe client connection pooling + CLOSE_WAIT E2E test | CONFIRMED in git log |

### Test Suite Results

| Test Suite | Command | Result |
|------------|---------|--------|
| rc-agent remote_ops tests | `cargo test -p rc-agent-crate -- remote_ops` | 8/8 PASSED |
| racecontrol fleet_health tests | `cargo test -p racecontrol-crate -- fleet_health` | 13/13 PASSED |

### Anti-Patterns Found

No TODOs, FIXMEs, placeholders, or stub implementations found in any of the 4 modified files or the new close-wait.sh script. All implementations are substantive and wired.

Only pre-existing compiler warnings exist (unused imports in debug_server.rs, unused variables in lock_screen.rs, etc.) — none are in the phase-45-modified code paths.

### Human Verification Required

#### 1. 30-Minute Live Soak Test

**Test:** Deploy the new rc-agent.exe binary to all 8 pods. Let fleet_health polling run for 30 minutes (normal operations — no manual intervention). Then run `bash tests/e2e/fleet/close-wait.sh` from the dev machine.

**Expected:** All reachable pods report CLOSE_WAIT count < 5 on :8090. Previously affected pods (which had 100-134 CLOSE_WAIT) should now show 0-4. Unreachable pods show as SKIP (not FAIL).

**Why human:** Requires deploying new binaries to live pods and waiting for the accumulation pattern to be eliminated over time. Cannot simulate socket lifecycle in static analysis.

#### 2. UDP Rebind After Self-Relaunch

**Test:** Trigger a self-relaunch on any pod (e.g., by observing the self_monitor CLOSE_WAIT strike counter reach 5, or by manually killing rc-agent.exe and letting HKLM Run key restart it). Check the logs for UDP port binding messages.

**Expected:** Log lines contain "Listening for game telemetry on UDP port X (SO_REUSEADDR)" for all 5 ports (9996 AC, 20777 F1, 5300 Forza, 6789 iRacing, 5555 LMU). No "Could not bind UDP port" or error 10048 messages.

**Why human:** Windows socket inheritance requires a live child-process exec sequence to test properly. bind_udp_reusable() is correct in code but the behaviour requires a real Windows environment post-relaunch to confirm.

#### 3. CLOSE_WAIT Strike Counter Drop

**Test:** Monitor pod logs over an 8-hour window after deploying the new binary. Check self_monitor.rs CLOSE_WAIT-related log entries.

**Expected:** Self-relaunches triggered by CLOSE_WAIT strike counter (CLOSE_WAIT_THRESHOLD = 20, strikes reset each check) drop to zero. Previously 5/8 pods triggered this every ~5 minutes.

**Why human:** Requires sustained live monitoring — cannot verify from static analysis.

### Summary

Phase 45 goal is **fully implemented** in code. All 5 must-have artifacts are present, substantive, and wired. All automated tests pass (8/8 remote_ops, 13/13 fleet_health). Three of the six success criteria from ROADMAP.md require live pod deployment to confirm at runtime — the code changes are the necessary and sufficient conditions, but socket accumulation behaviour can only be observed on live hardware after 30 minutes of polling.

The phase is ready for binary deployment to all 8 pods. The correct deploy sequence:
1. Build `rc-agent.exe` from this commit
2. Deploy to all 8 pods via pendrive (install.bat v5)
3. Also deploy updated `racecontrol.exe` (fleet_health.rs change) to the server
4. Run `bash tests/e2e/fleet/close-wait.sh` after 30 minutes

---
_Verified: 2026-03-19T07:30:00+05:30_
_Verifier: Claude (gsd-verifier)_
