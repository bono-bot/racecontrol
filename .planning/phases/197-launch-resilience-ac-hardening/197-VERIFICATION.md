---
phase: 197-launch-resilience-ac-hardening
verified: 2026-03-26T00:00:00+05:30
status: passed
score: 16/16 must-haves verified
re_verification: false
gaps: []
human_verification:
  - test: "Launch AC on a real pod with 10+ historical sessions"
    expected: "Timeout in check_game_health adapts to median+2*stdev of actual data, not 120s"
    why_human: "query_dynamic_timeout reads from launch_events table which requires real launch history — cannot populate in unit test"
  - test: "Cause a game crash (kill acs.exe) while billing is active"
    expected: "Race Engineer fires, game relaunches within 60s; second crash also relaunches; third crash pauses billing and sends WhatsApp"
    why_human: "Race Engineer relaunch path requires a live pod, billing session, and actual game process — cannot simulate in test"
  - test: "Trigger a game launch when MAINTENANCE_MODE file exists at C:\\RacingPoint"
    expected: "Launch blocked immediately with 'Pre-launch check failed: MAINTENANCE_MODE active' error shown in admin dashboard"
    why_human: "Requires a live pod with the sentinel file present — unit tests use injectable temp dirs"
---

# Phase 197: Launch Resilience AC Hardening Verification Report

**Phase Goal:** Game launches are resilient with dynamic timeouts tuned from historical data, pre-launch health checks, structured error taxonomy, auto-retry with clean state reset, and AC-specific reliability improvements -- launch failures recover automatically in under 60 seconds
**Verified:** 2026-03-26T00:00:00+05:30
**Status:** PASSED
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Dynamic timeout uses median+2*stdev from historical data (min 3 samples, floor 30s) | VERIFIED | `query_dynamic_timeout()` at metrics.rs:268; `timeout_secs.max(30)` at metrics.rs:306; 5 unit tests at metrics.rs:360-414 |
| 2 | First launch on new combo uses AC=120s, others=90s default | VERIFIED | game_launcher.rs:238-241: `SimType::AssettoCorsa => 120, _ => 90`; check_game_health fallback at line 924-927 also uses 90 |
| 3 | exit code 0xC0000005 classified as `ProcessCrash { exit_code: 3221225477 }` not Unknown | VERIFIED | `classify_error_taxonomy(msg, exit_code)` at game_launcher.rs:1079-1082 checks exit_code first; test_classify_error_taxonomy_exit_code_access_violation at line 2076 |
| 4 | Rapid duplicate Error events on same pod result in exactly 1 relaunch | VERIFIED | Single write lock block at game_launcher.rs:692-712 atomically reads count, increments, returns decision; test_race_engineer_atomic_single_relaunch at line 2126 |
| 5 | Timeout in check_game_health triggers Race Engineer auto-relaunch | VERIFIED | game_launcher.rs:998-1001: `handle_game_state_update(state, info).await` called on timeout; comment at line 1000 confirms no separate broadcast |
| 6 | stop_game() logs actual sim_type, not empty string | VERIFIED | game_launcher.rs:476: `log_pod_activity(..., &info.sim_type.to_string(), ...)` and line 487: `let sim_type_str = info.sim_type.to_string()`; test at line 2243 |
| 7 | After 2 failed retries, WhatsApp alert sent to 917075778180 | VERIFIED | `send_staff_launch_alert()` defined at game_launcher.rs:856; called at line 825; staff number at line 891: `"917075778180"` |
| 8 | Relaunch with launch_args=None rejected with clear error message | VERIFIED | game_launcher.rs:373: `"Cannot relaunch pod {} — original launch args unavailable"`; Race Engineer guard at line 696-701 |
| 9 | Game crashes do NOT create MAINTENANCE_MODE sentinel | VERIFIED | grep returned zero production hits; test_race_engineer_no_maintenance_mode_sentinel_written at line 2291 |
| 10 | Pre-flight checks block launch when MAINTENANCE_MODE/OTA_DEPLOYING/orphan/disk-low | VERIFIED | `pre_launch_checks()` at game_process.rs:84; `check_sentinel_files_in_dir()` at line 59; tests at lines 638, 658 |
| 11 | Failed pre-launch check sends GameState::Error with specific reason to server | VERIFIED | ws_handler.rs:304-319: error path sends `GameLaunchInfo { game_state: GameState::Error, error_message: Some(format!(...)) }` via ws_tx |
| 12 | AC post-kill polls for acs.exe absence (max 5s) instead of sleeping 2s | VERIFIED | `wait_for_acs_exit(5)` called at ac_launcher.rs:294; `fn wait_for_acs_exit` defined at line 1157; no `sleep(Duration::from_secs(2))` found |
| 13 | AC load polls for PID stability (max 30s) instead of sleeping 8s | VERIFIED | `wait_for_ac_ready(30)` called at ac_launcher.rs:369; `fn wait_for_ac_ready` defined at line 1173; no `sleep(Duration::from_secs(8))` found |
| 14 | CM timeout is 30s with 5s progress logging | VERIFIED | `wait_for_ac_process(30)` at ac_launcher.rs:321; progress log "CM progress: checking acs.exe..." at line 1145; comment at line 1131 |
| 15 | CM fallback uses find_acs_pid() for fresh PID instead of stale child.id() | VERIFIED | ac_launcher.rs:343-350: AC-04 comment + `find_acs_pid().unwrap_or_else(|| child.id())` + `persist_pid(fresh_pid)` |
| 16 | Args with spaces in paths not broken by split_whitespace | VERIFIED | `parse_launch_args()` at game_process.rs:71; call site at line 316; split_whitespace appears only in comments (lines 70, 315); tests at lines 685-713 |

**Score:** 16/16 truths verified

---

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/racecontrol/src/metrics.rs` | `query_dynamic_timeout()` function | VERIFIED | Exists at line 268; 5 tests (360-414); floor guard at line 306 |
| `crates/rc-common/src/types.rs` | `exit_code: Option<i32>` on GameLaunchInfo | VERIFIED | Found at line 438 with serde default+skip_serializing_if |
| `crates/racecontrol/src/game_launcher.rs` | Atomic Race Engineer, timeout routing, WhatsApp alert, stop_game fix | VERIFIED | All features present; 9+ new tests; `dynamic_timeout_secs` field at line 32 |
| `crates/rc-agent/src/ws_handler.rs` | Pre-launch checks wired in LaunchGame handler | VERIFIED | Block at lines 294-324 with spawn_blocking + GameState::Error on failure |
| `crates/rc-agent/src/ac_launcher.rs` | Polling waits, CM 30s timeout, fresh PID | VERIFIED | All functions present; wait_for_acs_exit + wait_for_ac_ready + wait_for_ac_process(30) |
| `crates/rc-agent/src/game_process.rs` | pre_launch_checks(), clean_state_reset(), parse_launch_args() | VERIFIED | All three pub functions present; 7+ tests covering each |

---

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `game_launcher.rs` | `metrics.rs` | `metrics::query_dynamic_timeout()` | WIRED | Called at game_launcher.rs:243 inside `launch_game()` |
| `game_launcher.rs` | `check_game_health -> Race Engineer` | `handle_game_state_update(state, info).await` | WIRED | Timeout path at game_launcher.rs:1001 calls handle_game_state_update directly |
| `ws_handler.rs` | `game_process.rs` | `crate::game_process::pre_launch_checks()` | WIRED | ws_handler.rs:297 wraps in spawn_blocking; result dispatches GameState::Error or Continue |
| `ac_launcher.rs` | `find_acs_pid` | polling loop and CM fresh PID fallback | WIRED | wait_for_acs_exit and wait_for_ac_ready both call find_acs_pid(); CM fallback at line 346 |

---

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| LAUNCH-08 | 197-01 | Dynamic timeout: median+2*stdev, per sim/car/track | SATISFIED | `query_dynamic_timeout()` in metrics.rs; called in `launch_game()` at line 243; stored as `dynamic_timeout_secs` |
| LAUNCH-09 | 197-01 | Default timeouts: AC=120s, others=90s | SATISFIED | game_launcher.rs:238-241 (launch_game) and 924-926 (check_game_health) both use AC=120, _=90 |
| LAUNCH-10 | 197-02 | Pre-launch health checks: orphan exe, disk, MAINTENANCE_MODE, OTA_DEPLOYING | SATISFIED | `pre_launch_checks()` at game_process.rs:84 checks all 4 conditions |
| LAUNCH-11 | 197-01, 197-02 | Clean state reset: kill all 13 game exe names, delete game.pid | SATISFIED | `clean_state_reset()` at game_process.rs:135; calls `all_game_process_names()` (13 names) + `clear_persisted_pid()` |
| LAUNCH-12 | 197-01 | Auto-retry: 2 attempts max, same launch_args, then WhatsApp alert | SATISFIED | Race Engineer at game_launcher.rs:702: `auto_relaunch_count < 2`; alert on exhaustion at line 825 |
| LAUNCH-13 | 197-01 | ErrorTaxonomy: typed exit codes beat string matching | SATISFIED | `classify_error_taxonomy()` at line 1079 checks exit_code first, falls back to string heuristics |
| LAUNCH-14 | 197-01 | Game crash counter separate from pod health — no MAINTENANCE_MODE from Race Engineer | SATISFIED | Zero production MAINTENANCE_MODE hits in game_launcher.rs; test_race_engineer_no_maintenance_mode_sentinel_written |
| LAUNCH-15 | 197-01 | WhatsApp alert after 2 failed retries with structured content | SATISFIED | `send_staff_launch_alert()` defined at line 856; called at line 825; sends to 917075778180 via Evolution API |
| LAUNCH-16 | 197-01 | Null launch_args relaunch rejected with clear message | SATISFIED | Race Engineer guard at line 696-701; relaunch_game() guard at line 373 |
| LAUNCH-17 | 197-01 | Race Engineer atomic: single write lock for counter check+increment+decision | SATISFIED | Single `active_games.write().await` block at lines 692-712 performs all three atomically |
| LAUNCH-18 | 197-01 | Timeout fires Race Engineer (not just Error state) | SATISFIED | check_game_health timeout path calls `handle_game_state_update(state, info).await` at line 1001 |
| LAUNCH-19 | 197-01, 197-02 | stop_game() logs sim_type (not empty string); arg parsing handles spaces | SATISFIED | stop_game at line 476 uses `info.sim_type.to_string()`; parse_launch_args replaces split_whitespace |
| AC-01 | 197-02 | AC post-kill polls for acs.exe absence (max 5s) | SATISFIED | `wait_for_acs_exit(5)` at ac_launcher.rs:294; 500ms poll interval |
| AC-02 | 197-02 | AC load polls for AC window/PID stability (max 30s) | SATISFIED | `wait_for_ac_ready(30)` at ac_launcher.rs:369; PID-stability polling (same PID alive 3s) |
| AC-03 | 197-02 | CM timeout 30s with 5s progress logging | SATISFIED | `wait_for_ac_process(30)` at line 321; progress log at line 1145 |
| AC-04 | 197-02 | CM fallback: fresh PID via find_acs_pid() not stale child.id() | SATISFIED | Lines 343-350: AC-04 comment + find_acs_pid() + persist_pid(fresh_pid) |

All 16 requirements (LAUNCH-08 through LAUNCH-19, AC-01 through AC-04) are satisfied. No orphaned requirements found.

---

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `game_process.rs` | 70, 315 | `split_whitespace` in comments only | Info | Doc/comment only — production code uses `parse_launch_args()`. No impact. |

No blockers or warnings found. The `split_whitespace` references are exclusively in documentation strings explaining what the old bug was.

---

### Human Verification Required

#### 1. Live Dynamic Timeout Adaptation

**Test:** On a pod that has run AC 10+ times, launch a new AC session and check racecontrol logs for the dynamic timeout value used.
**Expected:** Log line "dynamic timeout Xs for AssettoCorsaCompetizione/..." where X is computed from median+2*stdev of past durations, not 120.
**Why human:** Requires populated `launch_events` table with real session data. Unit tests use controlled in-memory SQLite with synthetic rows.

#### 2. Race Engineer Full Recovery Cycle

**Test:** Start a billing session on a pod, kill acs.exe, observe restart; kill again, observe second restart; kill third time, observe billing pause and WhatsApp message to staff phone.
**Expected:** Two successful relaunches within ~30s each, then billing paused with WhatsApp alert referencing pod ID, game name, and error taxonomy.
**Why human:** Requires a live pod, active billing session, real WhatsApp Evolution API config, and network reachability to 917075778180.

#### 3. Pre-launch MAINTENANCE_MODE Block

**Test:** Create `C:\RacingPoint\MAINTENANCE_MODE` on a pod, attempt to launch a game from the kiosk.
**Expected:** Launch fails immediately with "Pre-launch check failed: MAINTENANCE_MODE active" visible in admin dashboard / pod status.
**Why human:** Unit tests use injectable temp dir paths. Real sentinel file at `C:\RacingPoint\` requires a live pod.

---

### Commits Verified

All 4 phase commits confirmed in git history:
- `42f87b0c` — feat(197-01): dynamic timeout + exit_code taxonomy + atomic classify (LAUNCH-08, LAUNCH-09)
- `5019e476` — feat(197-01): atomic Race Engineer + null args guard + WhatsApp alert + stop_game fix (LAUNCH-14 thru LAUNCH-19)
- `7a05058b` — feat(197-02): pre-launch checks, clean state reset, arg parsing fix
- `b8cff553` — feat(197-02): AC polling waits, CM 30s timeout, fresh PID on fallback

---

### Summary

Phase 197 goal is fully achieved. All 16 requirements are implemented with substantive code (not stubs), properly wired, and covered by unit tests.

Key verifications:
- `query_dynamic_timeout()` exists, is called in `launch_game()`, stores result in `dynamic_timeout_secs`, and is read by `check_game_health()`.
- Race Engineer uses a **single** write lock block — the TOCTOU duplicate-relaunch bug is fixed.
- Timeout in `check_game_health()` now routes through `handle_game_state_update()` — Race Engineer fires on timeout, not just on crash.
- WhatsApp alert is wired: `send_staff_launch_alert()` is defined and called in the exhausted-retries path.
- `pre_launch_checks()` is wired into the `LaunchGame` handler in `ws_handler.rs` via `spawn_blocking`.
- Hardcoded 2s and 8s sleeps in `ac_launcher.rs` are confirmed absent; polling functions are called at the correct call sites.
- `parse_launch_args()` replaces `split_whitespace` in `GameProcess::launch()`; `split_whitespace` appears only in explanatory comments.

3 human-verification items remain (live pod scenarios): dynamic timeout from real history, Race Engineer full recovery cycle, and MAINTENANCE_MODE block behavior. These do not block the phase — they require live pod access and real data that cannot be automated.

---

_Verified: 2026-03-26T00:00:00+05:30_
_Verifier: Claude (gsd-verifier)_
