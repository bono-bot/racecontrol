---
phase: 04-safety-enforcement
verified: 2026-03-14T00:00:00Z
status: passed
score: 10/10 must-haves verified
re_verification: false
---

# Phase 4: Safety Enforcement Verification Report

**Phase Goal:** Safety-critical settings are always enforced regardless of session type, and force feedback is handled safely at session boundaries
**Verified:** 2026-03-14
**Status:** PASSED
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| #  | Truth                                                                               | Status     | Evidence                                                                 |
|----|-------------------------------------------------------------------------------------|------------|--------------------------------------------------------------------------|
| 1  | Tyre Grip always 100% in race.ini                                                   | VERIFIED   | `SESSION_START=100` hardcoded in `write_dynamic_track_section()` line 547 |
| 2  | Tyre Grip always 100% in server_cfg.ini                                             | VERIFIED   | Literal `100` in `generate_server_cfg_ini()` line 303 with SAFETY comment |
| 3  | Damage Multiplier always 0% in race.ini [ASSISTS] section                           | VERIFIED   | `DAMAGE=0` hardcoded in `write_assists_section()` line 495               |
| 4  | Damage Multiplier always 0% in assists.ini                                          | VERIFIED   | `DAMAGE=0` hardcoded in `write_assists_ini()` format string line 775     |
| 5  | Damage Multiplier always 0% in server_cfg.ini                                       | VERIFIED   | Literal `0` in `generate_server_cfg_ini()` line 252 with SAFETY comment  |
| 6  | Post-write verification refuses to launch if DAMAGE!=0 or SESSION_START!=100        | VERIFIED   | `verify_safety_settings()` called in `launch_ac()` at line 261 before acs.exe start |
| 7  | FFB zeroed BEFORE game process is killed in every session-end path                  | VERIFIED   | All 7+ paths: `spawn_blocking(zero_force).await.ok()` before `game.stop()` |
| 8  | 500ms delay between FFB zero and game kill in every path                            | VERIFIED   | `tokio::time::sleep(Duration::from_millis(500)).await` after FFB zero in all paths |
| 9  | FfbZeroed and GameCrashed protocol messages serialize/deserialize correctly         | VERIFIED   | Roundtrip tests pass: `test_ffb_zeroed_roundtrip`, `test_game_crashed_roundtrip` |
| 10 | Crash during active billing zeros FFB immediately and sends GameCrashed message     | VERIFIED   | Lines 770-777 in main.rs: immediate `zero_force().await.ok()` then `GameCrashed` then `FfbZeroed` |

**Score:** 10/10 truths verified

### Required Artifacts

| Artifact                                 | Expected                                                             | Status     | Details                                                                 |
|------------------------------------------|----------------------------------------------------------------------|------------|-------------------------------------------------------------------------|
| `crates/rc-agent/src/ac_launcher.rs`    | DAMAGE=0 hardcoded in write_assists_section and write_assists_ini; verify_safety_settings function | VERIFIED | Contains both hardcoded DAMAGE=0 writes, verify_safety_content(), and verify_safety_settings() called at launch_ac line 261 |
| `crates/rc-common/src/protocol.rs`      | FfbZeroed and GameCrashed AgentMessage variants                      | VERIFIED   | Both variants present at lines 60 and 63 with serde roundtrip tests     |
| `crates/rc-core/src/ac_server.rs`       | Safety overrides for damage_multiplier=0 and session_start=100       | VERIFIED   | Lines 252 and 303 override with literals, 2 safety tests pass           |
| `crates/rc-agent/src/main.rs`           | Reordered session-end sequences with FFB-first safety                | VERIFIED   | 7+ paths: BillingStopped, SessionEnded, StopGame, SubSessionEnded, crash-billing, crash-no-billing, crash-recovery, disconnect all zero FFB before game.stop() |
| `crates/rc-core/src/ws/mod.rs`          | Match arms for FfbZeroed and GameCrashed                             | VERIFIED   | Lines 388-395 handle both variants with logging and pod activity tracking |

### Key Link Verification

| From                                     | To                          | Via                                               | Status  | Details                                                                 |
|------------------------------------------|-----------------------------|---------------------------------------------------|---------|-------------------------------------------------------------------------|
| `crates/rc-agent/src/ac_launcher.rs`    | `verify_safety_settings`    | Called in launch_ac before acs.exe start           | WIRED   | Line 261: `verify_safety_settings()?;` between write_assists_ini and game launch |
| `crates/rc-common/src/protocol.rs`      | `crates/rc-agent/src/main.rs` | FfbZeroed variant used in all session-end paths | WIRED   | Pattern `FfbZeroed` found at 8 sites in main.rs (lines 776, 785, 872, 893, 1016, 1054, 1348, 1437) |
| `crates/rc-common/src/protocol.rs`      | `crates/rc-agent/src/main.rs` | GameCrashed variant used on crash during billing | WIRED   | Line 774: `AgentMessage::GameCrashed { pod_id, billing_active: true }` sent |
| `crates/rc-agent/src/main.rs`           | `ffb_controller.rs`         | `zero_force().await` via spawn_blocking            | WIRED   | Pattern `zero_force.*\.await` found at all session-end sites; `spawn_blocking(move \|\| { f.zero_force().ok(); }).await.ok()` |

### Requirements Coverage

| Requirement | Source Plan | Description                                                            | Status    | Evidence                                                                |
|-------------|-------------|------------------------------------------------------------------------|-----------|-------------------------------------------------------------------------|
| BILL-03     | 04-01-PLAN  | Tyre Grip always 100% — enforced in race.ini and server config, not overridable | SATISFIED | SESSION_START=100 hardcoded in write_dynamic_track_section() and generate_server_cfg_ini(); post-write verification guards |
| BILL-04     | 04-01-PLAN  | Damage Multiplier always 0% — enforced in race.ini and server config, not overridable | SATISFIED | DAMAGE=0 hardcoded in write_assists_section(), write_assists_ini(), and generate_server_cfg_ini(); tests verify config with damage=100 still outputs DAMAGE=0 |
| BILL-05     | 04-01-PLAN / 04-02-PLAN | FFB torque zeroed on wheelbase BEFORE game process is killed    | SATISFIED | All 7+ session-end paths in main.rs zero FFB (awaited) with 500ms delay before game.stop(); StopGame handler added FFB zeroing that was previously absent |

All 3 requirements assigned to Phase 4 are SATISFIED. No orphaned requirements found — REQUIREMENTS.md traceability table confirms BILL-03, BILL-04, BILL-05 all map to Phase 4.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `crates/rc-agent/src/main.rs` | 1523 | Disconnect path logs "ws_tx unavailable for FfbZeroed message" | INFO | Not a bug — FfbZeroed cannot be sent when disconnected from core. FFB zero still happens and is awaited. Noted as expected behavior. |

No blockers. No TODO/FIXME/placeholder patterns in any modified file.

**Note on dead code warnings:** The compiler emits warnings for `AcConditions.damage` field (never read) and `AcLaunchParams.conditions` (never read). These are expected — the SUMMARY notes the `damage` field is kept for deserialization backward compatibility after being removed from all INI write paths. Not a safety concern.

### Human Verification Required

#### 1. Physical FFB Ordering on Pod 8

**Test:** Start a session on Pod 8, then end the session (either via billing timeout or StopGame). Watch the Conspit Ares wheelbase while the session closes.
**Expected:** Wheel goes limp (torque drops to zero) BEFORE the AC window closes. There should be a noticeable half-second pause between wheel going limp and the AC process dying.
**Why human:** Cannot verify USB HID command timing, physical force feedback response, or visual AC window closure order from code inspection alone.

#### 2. Game Crash During Active Billing

**Test:** Start a session on Pod 8 with active billing, then kill acs.exe via Task Manager to simulate a crash.
**Expected:** Wheel goes limp immediately (within ~1s of crash detection), logs show "Game crashed during active billing — zeroing FFB immediately", and core receives a GameCrashed message.
**Why human:** Game crash simulation requires a live pod; timing verification requires watching both the wheelbase and the logs in real time.

### Gaps Summary

No gaps. All automated checks passed.

---

## Summary of Verification

**Plan 01 (INI Safety Hardcoding)** is fully verified:
- `DAMAGE=0` is a hardcoded literal in all three INI write paths (race.ini ASSISTS section, assists.ini ASSISTS section, server_cfg.ini). The old `params.conditions.damage` variable is gone from all write paths.
- `SESSION_START=100` is hardcoded in race.ini (was already correct, now regression-guarded by test) and enforced in server_cfg.ini (was using config value, now overridden to literal 100).
- `verify_safety_settings()` re-reads race.ini from disk after writing and before launching acs.exe. The testable string variant `verify_safety_content()` has 3 tests: pass, reject-damage, reject-grip.
- `FfbZeroed` and `GameCrashed` variants exist in `AgentMessage`, serialize correctly, and have match arms in the core WebSocket handler (log-only, correct for this phase).

**Plan 02 (FFB Session-End Safety)** is fully verified:
- All 7+ session-end paths in `main.rs` follow the correct order: `spawn_blocking(zero_force).await.ok()` then `sleep(500ms)` then `game.stop()` then `FfbZeroed` message then `enforce_safe_state()`.
- StopGame handler now includes FFB zeroing (was completely absent before).
- Crash during active billing zeroes FFB immediately and sends both `GameCrashed` and `FfbZeroed` messages.
- Disconnect path zeros FFB before game.stop() with 500ms delay; cannot send `FfbZeroed` message because ws_tx is unavailable — this is acknowledged in a code comment and is expected behavior.
- Test suite: 11 new tests all pass. Full workspace compiles clean (warnings are pre-existing dead code, not regressions).

---

_Verified: 2026-03-14_
_Verifier: Claude (gsd-verifier)_
