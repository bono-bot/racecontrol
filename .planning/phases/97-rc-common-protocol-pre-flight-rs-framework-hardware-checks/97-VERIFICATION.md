---
phase: 97-rc-common-protocol-pre-flight-rs-framework-hardware-checks
verified: 2026-03-21T10:15:00+05:30
status: passed
score: 10/10 must-haves verified
re_verification: false
gaps: []
human_verification:
  - test: "Deploy to Pod 8, start a session with ConspitLink.exe not running"
    expected: "Session is blocked, PreFlightFailed message visible in racecontrol logs, billing_active stays false"
    why_human: "Requires live pod hardware — ConspitLink.exe process state, HID wheelbase presence, and actual billing flow cannot be simulated in unit tests"
  - test: "Disconnect wheelbase USB, attempt BillingStarted"
    expected: "HID check fails, session blocked, pod stays idle"
    why_human: "FfbBackend::zero_force() result depends on physical USB HID device presence"
  - test: "Leave a game process running (record in game_process state), then trigger BillingStarted"
    expected: "Orphan game process killed via taskkill /F /PID before session starts, check returns Pass"
    why_human: "Requires a live game process PID and real taskkill execution"
---

# Phase 97: rc-common Protocol + pre_flight.rs Framework + Hardware Checks Verification Report

**Phase Goal:** The foundational layer exists and compiles — new AgentMessage variants in rc-common are available to rc-agent, pre_flight.rs owns the concurrent check gate with a hard 5-second timeout, and the three highest-value hardware checks (HID wheelbase, ConspitLink process+config, orphaned game PID-targeted kill) run correctly with one auto-fix attempt each.

**Verified:** 2026-03-21T10:15:00+05:30 (IST)
**Status:** PASSED
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|---------|
| 1 | cargo build --bin rc-agent and cargo build --bin racecontrol both succeed | VERIFIED | Both build with 0 errors; only warnings present |
| 2 | cargo build --bin rc-sentry succeeds (stdlib-only constraint not violated) | VERIFIED | `Finished dev profile` with no errors — only 1.07s compile |
| 3 | PreflightConfig defaults to enabled=true when [preflight] section is missing from TOML | VERIFIED | `Default` impl returns `Self { enabled: true }`; `#[serde(default)]` on `AgentConfig.preflight` field |
| 4 | New AgentMessage variants use pod_id: String matching all existing variants | VERIFIED | `PreFlightPassed { pod_id: String }` and `PreFlightFailed { pod_id: String, ... }` at protocol.rs lines 217-226 |
| 5 | Pre-flight checks run on every BillingStarted before billing_active is set to true | VERIFIED | ws_handler.rs lines 140-164: gate block executes before `billing_active.store(true)` at line 167 |
| 6 | All three checks (HID, ConspitLink, orphan game) run concurrently via tokio::join! with 5-second hard timeout | VERIFIED | `timeout(Duration::from_secs(5), run_concurrent_checks(...))` at pre_flight.rs lines 60-64; `tokio::join!` at lines 122-126 |
| 7 | A failed ConspitLink check attempts one auto-fix (spawn process, wait 2s, re-check) before reporting failure | VERIFIED | `fix_conspit()` called at pre_flight.rs line 85 on Fail; spawn + 2s sleep + re-scan inside `spawn_blocking`; wrapped in `timeout(Duration::from_secs(3))` |
| 8 | Orphan game kill uses PID from state.game_process (never name-based kill) | VERIFIED | `taskkill /F /PID {pid}` at pre_flight.rs lines 241-244; `game_pid` extracted from `state.game_process.as_ref().and_then(|gp| gp.pid)` |
| 9 | When preflight.enabled is false, BillingStarted proceeds directly with no pre_flight::run() call | VERIFIED | `if state.config.preflight.enabled { ... }` guard at ws_handler.rs line 141 |
| 10 | billing_active.store(true) only executes when pre-flight passes | VERIFIED | `billing_active.store(true)` at ws_handler.rs line 167 — physically after the gate block (lines 140-164) which returns `Continue` on `MaintenanceRequired` |

**Score:** 10/10 truths verified

---

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/rc-common/src/protocol.rs` | PreFlightPassed, PreFlightFailed AgentMessage variants + ClearMaintenance CoreToAgentMessage variant | VERIFIED | All three variants present at lines 217-226 and 427-428; round-trip serde tests pass (135/135 rc-common tests) |
| `crates/rc-agent/src/config.rs` | PreflightConfig struct with serde default | VERIFIED | `PreflightConfig { enabled: bool }` at lines 48-57; `Default` impl returns `enabled: true`; wired into `AgentConfig.preflight` at line 23 with `#[serde(default)]` |
| `crates/rc-agent/src/pre_flight.rs` | Concurrent pre-flight check runner with auto-fix | VERIFIED | 391-line real implementation; exports `run`, `PreFlightResult`, `CheckResult`, `CheckStatus`; 6 unit tests pass |
| `crates/rc-agent/src/ws_handler.rs` | Pre-flight gate in BillingStarted handler | VERIFIED | `pre_flight::run` called at line 143; gate block lines 140-164; `billing_active.store(true)` at line 167 |
| `crates/rc-agent/src/main.rs` | mod pre_flight declaration | VERIFIED | `mod pre_flight;` at line 18 |

---

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `ws_handler.rs` | `pre_flight.rs` | `pre_flight::run()` call in BillingStarted arm | WIRED | Line 143: `pre_flight::run(state, ffb_ref).await` |
| `pre_flight.rs` | `ffb_controller.rs` | `ffb.zero_force()` for HID check | WIRED | Line 139: `match ffb.zero_force()` in `check_hid()` |
| `pre_flight.rs` | `sysinfo::System` | `spawn_blocking` process scan for ConspitLink | WIRED | Lines 163-218: `spawn_blocking` with `sys.refresh_processes(ProcessesToUpdate::All, true)` |
| `ws_handler.rs` | `billing_active.store(true)` | Only inside pre-flight Pass branch | WIRED | Line 167 — after gate returns early on `MaintenanceRequired`; no other `billing_active.store(true)` in BillingStarted arm |
| `racecontrol/ws/mod.rs` | `AgentMessage::PreFlightPassed/PreFlightFailed` | Log-only match arms (Phase 98 full handler) | WIRED | Lines 708-714: `tracing::info!` for Passed, `tracing::warn!` for Failed + `log_pod_activity` call |

---

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|---------|
| PF-01 | 97-02 | Pre-flight checks run on every BillingStarted before PIN entry is shown | SATISFIED | BillingStarted arm in ws_handler.rs calls `pre_flight::run()` before `billing_active.store(true)` |
| PF-02 | 97-02 | All checks run concurrently via tokio::join! with 5-second hard timeout | SATISFIED | `tokio::join!(check_hid, check_conspit, check_orphan_game)` wrapped in `timeout(Duration::from_secs(5))` |
| PF-03 | 97-02 | Failed checks attempt one auto-fix before reporting failure | SATISFIED | `fix_conspit()` called once per ConspitLink Fail; orphan game kill IS the fix; HID has no fix (hardware) |
| PF-07 | 97-01 | Pre-flight can be disabled per-pod via rc-agent.toml config flag | SATISFIED | `PreflightConfig.enabled` in config.rs; `if state.config.preflight.enabled` guard in ws_handler.rs |
| HW-01 | 97-02 | Wheelbase HID connected (FfbController::zero_force returns Ok(true)) | SATISFIED | `check_hid()` in pre_flight.rs; `Ok(true)` -> Pass, `Ok(false)` or `Err` -> Fail |
| HW-02 | 97-02 | ConspitLink process running (two-stage: process alive + config files valid) | SATISFIED | `check_conspit()`: Stage 1 = process scan, Stage 2 = `C:\ConspitLink\config.json` JSON validation |
| HW-03 | 97-02 | Auto-fix: restart ConspitLink process if not running | SATISFIED | `fix_conspit()`: spawns `ConspitLink.exe`, sleeps 2s, re-scans; wrapped in `timeout(Duration::from_secs(3))` |
| SYS-01 | 97-02 | No orphaned game process from previous session (kill if found) | SATISFIED | `check_orphan_game()`: PID-targeted `taskkill /F /PID {pid}` via `spawn_blocking`; never name-based |

**All 8 requirements SATISFIED.** No orphaned requirements found.

---

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| None | — | — | — | — |

No TODOs, FIXMEs, empty implementations, placeholder returns, or stub handlers found in the five modified files. The log-only stubs in `racecontrol/ws/mod.rs` for `PreFlightPassed`/`PreFlightFailed` are intentional and documented — full MaintenanceRequired FSM is Phase 98 scope.

---

### Human Verification Required

These items cannot be verified programmatically and require live pod testing:

#### 1. ConspitLink auto-fix on live pod

**Test:** Stop ConspitLink.exe on a pod, then have racecontrol send a BillingStarted message.
**Expected:** rc-agent blocks the session, spawns ConspitLink.exe, waits 2 seconds, re-scans. If ConspitLink starts, session proceeds. If not, `PreFlightFailed` is sent to racecontrol and billing stays inactive.
**Why human:** Requires live ConspitLink.exe process state, real process spawn, and actual WS message flow.

#### 2. Wheelbase HID check on live pod

**Test:** Disconnect the wheelbase USB on Pod 8, attempt a billing session start.
**Expected:** HID check fails (FfbBackend::zero_force returns Ok(false)), session is blocked, pod stays idle.
**Why human:** FfbBackend::zero_force() depends on physical USB HID device presence — cannot mock in integration.

#### 3. Orphan game PID kill on live pod

**Test:** Leave a game process running (state.game_process populated with a real PID), trigger BillingStarted.
**Expected:** taskkill /F /PID {pid} executes successfully, check returns Pass, session proceeds normally.
**Why human:** Requires a live game process PID, real taskkill execution, and verification that billing proceeds after kill.

---

### Gaps Summary

No gaps. All must-haves verified against the actual codebase. The phase goal is fully achieved:

- **Foundational layer:** `AgentMessage::PreFlightPassed`, `AgentMessage::PreFlightFailed`, and `CoreToAgentMessage::ClearMaintenance` exist in rc-common with serde round-trip tests. `PreflightConfig` is wired into `AgentConfig` with `#[serde(default)]`.
- **Compilation:** All three binaries (rc-agent, racecontrol, rc-sentry) build cleanly. 135 rc-common tests pass. 6 pre_flight unit tests pass.
- **Concurrent gate:** `pre_flight::run()` uses `tokio::join!` with a `timeout(Duration::from_secs(5))` hard cap.
- **Hardware checks:** HID via `FfbBackend::zero_force()`, ConspitLink two-stage with `spawn_blocking` + `fix_conspit()` auto-fix, orphan game PID-targeted `taskkill`.
- **Billing safety:** `billing_active.store(true)` is physically after the pre-flight gate; `MaintenanceRequired` path returns `Continue` before ever reaching it.

---

_Verified: 2026-03-21T10:15:00+05:30 (IST)_
_Verifier: Claude (gsd-verifier)_
