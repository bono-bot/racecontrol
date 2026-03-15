---
phase: 18-startup-self-healing
verified: 2026-03-15T10:05:00Z
status: passed
score: 7/7 must-haves verified
re_verification: false
---

# Phase 18: Startup Self-Healing Verification Report

**Phase Goal:** rc-agent verifies and repairs its own prerequisites (config, start script, registry key) on every startup, reports startup status to rc-core after connecting, and captures startup errors to a log for post-mortem diagnosis.
**Verified:** 2026-03-15T10:05:00Z
**Status:** PASSED
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | If rc-agent.toml is deleted, the next startup regenerates it from an embedded template with the correct pod number derived from COMPUTERNAME | VERIFIED | `self_heal.rs:repair_config()` calls `detect_pod_number()` (reads COMPUTERNAME), replaces `{pod_number}/{pod_name}` in `CONFIG_TEMPLATE` (embedded via `include_str!`), validates TOML, writes file. Test `test_repair_config_generates_valid_toml` confirms pod=3 generates valid TOML with `number = 3`. |
| 2 | If start-rcagent.bat is deleted, the next startup recreates it with CRLF line endings | VERIFIED | `self_heal.rs:repair_start_script()` writes the `START_SCRIPT_CONTENT` const with explicit `\r\n`. Test `test_repair_start_script_crlf` confirms `\r\n` is present and `@echo off`/`start` lines exist. |
| 3 | If the HKLM Run key for RCAgent is deleted, the next startup recreates it pointing to start-rcagent.bat | VERIFIED | `self_heal.rs:registry_key_exists()` runs `reg query`, `repair_registry_key()` runs `reg add` with CREATE_NO_WINDOW. Both use `#[cfg(windows)]` CommandExt correctly. |
| 4 | Self-heal repairs are non-fatal: if any repair fails, rc-agent logs a warning and continues | VERIFIED | Each of the 4 repair branches in `run()` wraps repair calls in `match Ok/Err` — errors push to `result.errors` and log at ERROR, then execution continues. Function never panics. |
| 5 | A phased startup log at C:\RacingPoint\rc-agent-startup.log records each startup phase with a timestamp | VERIFIED | `startup_log.rs:write_phase()` builds RFC3339 timestamp + `phase=X details` line. 8 `write_phase()` calls in `main.rs`: init, lock_screen, self_heal, config_loaded, firewall, http_server, websocket, complete. Tests confirm file creation, append, and truncation on first call. |
| 6 | If rc-agent crashes mid-startup, the log file shows the last phase reached before exit | VERIFIED | `startup_log.rs:detect_crash_recovery_from()` reads last non-empty line, returns `true` if it does not contain `phase=complete`. `detect_crash_recovery()` is called BEFORE `write_phase("init")` (which truncates), preserving the previous run's log for inspection. Tests `test_detect_crash_incomplete` and `test_detect_crash_complete` confirm behavior. |
| 7 | rc-core logs a startup report from each pod within 10 seconds of the pod's WebSocket connecting, with version, uptime, config_hash, crash_recovery, and repairs | VERIFIED | `AgentMessage::StartupReport` added to `protocol.rs` with all 6 fields. `main.rs` sends it immediately after `Register` succeeds, guarded by `startup_report_sent` flag. `ws/mod.rs` handles it with `tracing::info!` + WARN for crash/repairs + `fleet_health::store_startup_report()`. Serde roundtrip tests pass. |

**Score: 7/7 truths verified**

---

## Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/rc-agent/src/self_heal.rs` | Self-heal module: config, script, registry repair + `config_hash()` | VERIFIED | 434 lines. Exports `run()`, `SelfHealResult`, `detect_pod_number_from()`, `config_hash()`. 9 passing tests. No `.unwrap()` in production code. |
| `crates/rc-agent/src/startup_log.rs` | Phased startup log: `write_phase()`, `detect_crash_recovery()` | VERIFIED | 199 lines. Exports `write_phase()`, `write_phase_to()`, `detect_crash_recovery()`, `detect_crash_recovery_from()`. 7 passing tests. `AtomicBool` for first-write-truncates semantics. |
| `crates/rc-agent/src/main.rs` | Startup sequence with self-heal before `load_config()`, `startup_log` at each phase | VERIFIED | `mod self_heal` (line 13), `mod startup_log` (line 15). `self_heal::run()` called at line 339 (before `load_config()` at line 352). 8 `write_phase()` calls at correct phases. |
| `crates/rc-common/src/protocol.rs` | `AgentMessage::StartupReport` variant with 6 fields and serde roundtrip tests | VERIFIED | Variant at lines 101-108 with `pod_id`, `version`, `uptime_secs`, `config_hash`, `crash_recovery`, `repairs` fields. 2 passing roundtrip tests (`test_startup_report_roundtrip`, `test_startup_report_crash_recovery`). |
| `crates/rc-agent/src/main.rs` | StartupReport sent once after first WS connection | VERIFIED | `startup_report_sent` flag (line 602). Send block at lines 660-684, guarded by `!startup_report_sent`, set to `true` on successful send. Fire-and-forget with WARN on failure. |
| `crates/rc-core/src/ws/mod.rs` | Handler for `AgentMessage::StartupReport` that logs and records pod startup info | VERIFIED | Match arm at line 481. Logs at INFO with all 6 fields. WARN on `crash_recovery=true` and non-empty repairs. Also calls `fleet_health::store_startup_report()` (bonus: stores in fleet health state for Phase 21). |
| `deploy/rc-agent.template.toml` | Config template embedded via `include_str!()` in self_heal.rs | VERIFIED | File exists with `{pod_number}` and `{pod_name}` placeholders. Embedded at `self_heal.rs:20` via `include_str!("../../../deploy/rc-agent.template.toml")`. |

---

## Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `main.rs` | `self_heal.rs` | `mod self_heal` + `self_heal::run()` before `load_config()` | WIRED | `mod self_heal` at line 13; `self_heal::run(&exe_dir)` at line 339; `load_config()` at line 352. |
| `main.rs` | `startup_log.rs` | `mod startup_log` + `startup_log::write_phase()` at each phase | WIRED | `mod startup_log` at line 15; 8 `startup_log::write_phase()` calls confirmed across startup sequence. |
| `self_heal.rs` | `deploy/rc-agent.template.toml` | `include_str!()` compile-time embed | WIRED | `const CONFIG_TEMPLATE: &str = include_str!("../../../deploy/rc-agent.template.toml")` at line 20. Build succeeds, confirming path resolves correctly at compile time. |
| `self_heal.rs` | `reg` command | `std::process::Command` for registry operations | WIRED | `registry_key_exists()` runs `reg query` (line 209); `repair_registry_key()` runs `reg add` (line 230). Both use `#[cfg(windows)] cmd.creation_flags(CREATE_NO_WINDOW)`. |
| `main.rs` | `protocol.rs` | `AgentMessage::StartupReport` enum variant | WIRED | `AgentMessage::StartupReport { ... }` constructed at lines 662-675 with all 6 fields. |
| `ws/mod.rs` | `protocol.rs` | match arm for `AgentMessage::StartupReport` | WIRED | Pattern match at line 481 with all 6 fields destructured and used. |
| `main.rs` | `self_heal.rs` | `heal_result` and `crash_recovery` used in `StartupReport` | WIRED | `heal_result.config_repaired`, `heal_result.script_repaired`, `heal_result.registry_repaired` used at lines 670-672; `crash_recovery` at line 667. Both variables remain in scope from Plan 01 wiring. |

---

## Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| HEAL-01 | 18-01 | rc-agent verifies config, start script, and registry key on every startup — repairs if missing | SATISFIED | `self_heal::run()` called unconditionally at startup before `load_config()`. Checks and repairs all three. 9 unit tests cover the repair logic. |
| HEAL-02 | 18-02 | rc-agent reports startup status to rc-core immediately after WebSocket connect | SATISFIED | `StartupReport` sent after `Register` succeeds, once per process lifetime. Fields: version, uptime_secs, config_hash, crash_recovery, repairs. rc-core logs and stores to fleet health. |
| HEAL-03 | 18-01 | Startup errors are captured to a log file before rc-agent exits (for post-mortem) | SATISFIED | `startup_log::write_phase()` called at 8 phases. `detect_crash_recovery()` reads previous log before truncating. If rc-agent crashes mid-startup, log shows last reached phase. |

---

## Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `self_heal.rs` | 340, 670-672 | `heal_result.defender_repaired` is not included in the startup log's repairs list or the StartupReport's repairs Vec | Info | Defender repair events are silently omitted from rc-core visibility and the startup log's repairs line. The Defender repair itself works correctly — it is only the reporting that is incomplete. Does not block any requirement. |

No TODOs, FIXMEs, stubs, placeholder returns, or empty implementations found. No `.unwrap()` in production code paths (only in `#[cfg(test)]` blocks, which is correct).

---

## Test Results

All tests passing as of verification:

| Crate | Tests | Result |
|-------|-------|--------|
| rc-common | 106 | PASS (includes `test_startup_report_roundtrip`, `test_startup_report_crash_recovery`) |
| rc-agent | 200 (9 self_heal + 7 startup_log + 184 existing) | PASS |
| rc-core | 238 unit + 41 integration = 279 | PASS (includes 13 fleet_health tests) |
| **Total** | **585** | **PASS — no regressions** |

Notable: rc-core gained a `fleet_health.rs` module (not in the plan) with 13 tests covering `store_startup_report`, `clear_on_disconnect`, and WS sender liveness — this is bonus work that prepares for Phase 21 (Fleet Dashboard).

---

## Human Verification Required

### 1. Config auto-repair on a real pod

**Test:** Delete `C:\RacingPoint\rc-agent.toml` on Pod 8. Reboot the pod. Check if a new config exists after rc-agent starts.
**Expected:** New `rc-agent.toml` with `number = 8` and `name = "Pod 8"` generated from the embedded template. Startup log shows `phase=self_heal repairs=config`.
**Why human:** COMPUTERNAME-based pod number detection requires a real pod (test uses tempdir + explicit pod number, not COMPUTERNAME).

### 2. Start script auto-repair

**Test:** Delete `C:\RacingPoint\start-rcagent.bat` on Pod 8. Reboot. Check if the file is recreated and has CRLF line endings.
**Expected:** File recreated with correct content and CRLF. Pod restarts normally on next reboot.
**Why human:** Requires live pod reboot cycle.

### 3. Startup log phases readable on pod

**Test:** After a normal Pod 8 boot, read `C:\RacingPoint\rc-agent-startup.log`.
**Expected:** All 8 phases present in order: init, lock_screen, self_heal, config_loaded, firewall, http_server, websocket, complete. Each line has an ISO-8601 timestamp.
**Why human:** Log file is written to `C:\RacingPoint\` which exists only on pods, not on James's machine.

### 4. Crash recovery detection

**Test:** Kill rc-agent mid-startup (e.g., after `phase=config_loaded` but before `phase=complete`). Restart rc-agent. Check `C:\RacingPoint\rc-agent-startup.log` first line or rc-agent logs for crash recovery warning.
**Expected:** rc-agent logs `"Detected crash recovery -- previous startup did not complete"` at WARN. New startup log begins with `phase=init` (previous log truncated) and all phases complete.
**Why human:** Requires controlled mid-startup kill on a live pod.

### 5. StartupReport visible in rc-core logs

**Test:** Restart Pod 8. Within 10 seconds of connection, check rc-core logs.
**Expected:** Log line like `Pod pod_8 startup report: version=0.X.X, uptime=Xs, config_hash=XXXX, crash_recovery=false, repairs=[]`.
**Why human:** Requires live pod + rc-core running and log access.

---

## Bonus: fleet_health.rs (beyond plan scope)

The implementation added `crates/rc-core/src/fleet_health.rs` (247 lines, 13 tests) which was not in the Phase 18 plans. This module:
- Stores `StartupReport` data (version, agent_started_at, crash_recovery) per pod in `AppState::pod_fleet_health`
- Provides `GET /api/v1/fleet/health` endpoint returning all 8 pods' health status
- Runs a background HTTP probe loop (`:8090/health`) every 15 seconds
- Will serve Phase 21 (Fleet Dashboard) directly

This is additive work that does not affect Phase 18 requirements but is worth noting for Phase 21 planning.

---

## Summary

Phase 18 goal is fully achieved. All three requirements (HEAL-01, HEAL-02, HEAL-03) are satisfied with substantive implementations — no stubs, no placeholder returns, no missing wiring. The self-heal module runs synchronously before config load, the startup log captures 8 phases with crash-recovery detection, and the StartupReport protocol flows end-to-end from rc-agent through the WebSocket to rc-core with logging and fleet health storage.

The one informational finding (Defender repair not reported in StartupReport repairs list) is an oversight that does not block any requirement. A pod that auto-repairs its Defender exclusion will show `repairs=[]` in the startup report rather than `repairs=["defender"]`, which could cause minor confusion during debugging but has no operational impact.

585 tests green. No regressions.

---

_Verified: 2026-03-15T10:05:00Z_
_Verifier: Claude (gsd-verifier)_
