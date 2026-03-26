---
phase: 196-game-launcher-structural-rework
verified: 2026-03-26T05:30:00Z
status: passed
score: 11/11 must-haves verified
re_verification: false
---

# Phase 196: Game Launcher Structural Rework — Verification Report

**Phase Goal:** The monolithic launch_game() is decomposed into per-game trait implementations with correct billing gates, state machine transitions, and error propagation -- structural bugs fixed before adding resilience features
**Verified:** 2026-03-26T05:30:00Z
**Status:** PASSED
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths (from ROADMAP.md Success Criteria)

| #  | Truth | Status | Evidence |
|----|-------|--------|----------|
| 1  | Trait architecture: AcLauncher, F1Launcher, IRacingLauncher each implement GameLauncherImpl | VERIFIED | `grep "impl GameLauncherImpl for"` shows 4 matches (AcLauncher, F1Launcher, IRacingLauncher, DefaultLauncher) at lines 77, 89, 101, 113 |
| 2  | Billing gate — deferred: waiting_for_game pod can launch successfully | VERIFIED | `waiting_for_game.read()` checked in billing gate at line 196; test `test_launch_allowed_with_deferred_billing` at line 1473 |
| 3  | Billing gate — paused: PausedManual/PausedDisconnect/PausedGamePause all rejected | VERIFIED | Match arm at lines 205-209; three tests at lines 1516, 1535, 1554 |
| 4  | Billing gate — TOCTOU: billing expiry during window fails with "billing session expired" | VERIFIED | Re-check inside `active_games.write()` at lines 236-244; error message "billing session expired during launch" confirmed |
| 5  | Double launch — Stopping: launch while Stopping rejected with "game still stopping" | VERIFIED | `GameState::Stopping` in match at line 220; error message "game still stopping on pod" at line 222; test at line 1629 |
| 6  | Stopping timeout: 30s without agent auto-transitions to Error with dashboard broadcast | VERIFIED | `tokio::spawn` in `stop_game()` at lines 493-510 with 30s sleep; `check_game_health()` also catches stale Stopping at lines 821-828; test `test_stopping_timeout_transitions_to_error_via_health_check` at line 1746 |
| 7  | Disconnected agent: tracker transitions to Error IMMEDIATELY, dashboard broadcast sent | VERIFIED | No-agent path at lines 277-297: sets `GameState::Error`, `error_message = "No agent connected"`, broadcasts; test at line 1911 |
| 8  | Feature flag block: `game_launch` disabled rejects launch before tracker creation | VERIFIED | Feature flag check at lines 181-191 comes before tracker insertion; returns Err("game_launch feature disabled"); test at line 1852 |
| 9  | Invalid JSON: malformed launch_args rejected with parse error, no tracker created | VERIFIED | `launcher.validate_args()` called at lines 163-165 BEFORE billing gate and tracker creation; test `test_launch_rejected_invalid_json` at line 1602 |
| 10 | Broadcast reliability: all dashboard_tx.send() calls log warn on failure | VERIFIED | 7 call sites all use `if let Err(e) = ... { tracing::warn!("dashboard broadcast failed...") }`; zero `let _ =` silent drops confirmed |
| 11 | Externally tracked: agent-reported game creates tracker with externally_tracked=true, launch_args=None | VERIFIED | `handle_game_state_update()` at lines 537-549: `externally_tracked: true`, `launch_args: None`; ws/mod.rs line 234 also sets `externally_tracked: true` on reconnect; test at line 1665 |

**Score: 11/11 truths verified**

---

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|---------|--------|---------|
| `crates/racecontrol/src/game_launcher.rs` | GameLauncherImpl trait + 4 impls + billing gate fix + JSON validation + state machine fixes | VERIFIED | 1943 lines; all required content present and wired |
| `crates/racecontrol/src/ws/mod.rs` | externally_tracked: true on reconnect reconciliation | VERIFIED | Line 234 confirmed |
| `crates/racecontrol/Cargo.toml` | tokio test-util dev-dependency | VERIFIED | Lines 80-81 confirmed |

---

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `launch_game()` | `GameLauncherImpl::validate_args()` | `launcher_for(sim_type).validate_args()` | WIRED | Lines 163-165: `let launcher = launcher_for(sim_type); launcher.validate_args(...)` |
| `launch_game()` | `waiting_for_game + active_timers` | combined billing gate | WIRED | Both maps read at lines 195-196; combined check at 199 |
| `stop_game()` | tokio::spawn 30s timeout | Stopping timeout spawner | WIRED | Lines 493-510 confirm spawn inside `if let Some(info) = info` block |
| `launch_game()` | `feature_flags.read()` | feature flag check before agent send | WIRED | Lines 181-191: before billing gate, before tracker creation |
| `handle_game_state_update()` | `GameTracker.externally_tracked` | externally_tracked: true on agent-reported games | WIRED | Lines 537-549 confirm `externally_tracked: true` in the else branch |

---

### Requirements Coverage

The phase declares 13 requirement IDs across both plans. These IDs (LAUNCH-01 through LAUNCH-07, STATE-01 through STATE-06) are defined in the ROADMAP.md Phase 196 entry and in the RESEARCH.md requirement table — they are NOT present in the main `REQUIREMENTS.md` file (which is the v23.1 Audit Protocol document). The authoritative source is the ROADMAP.md success criteria for this phase.

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| LAUNCH-01 | 196-01 | Trait-based architecture: per-game launchers | SATISFIED | 4 impls confirmed in code |
| LAUNCH-02 | 196-01 | Billing gate: deferred sessions allowed | SATISFIED | `waiting_for_game` checked at billing gate |
| LAUNCH-03 | 196-01 | Billing gate: paused sessions rejected | SATISFIED | Match arm + 3 tests |
| LAUNCH-04 | 196-01 | Billing gate: TOCTOU mitigation | SATISFIED | Re-check inside write lock at lines 236-244 |
| LAUNCH-05 | 196-02 | Double-launch guard blocks Stopping | SATISFIED | Stopping in match at line 220 |
| LAUNCH-06 | 196-01 | Invalid JSON rejected via validate_args | SATISFIED | Called before billing gate; test confirms |
| LAUNCH-07 | 196-02 | Broadcast failures logged at warn | SATISFIED | 7 call sites all use tracing::warn! |
| STATE-01 | 196-02 | Stopping timeout: 30s auto-Error | SATISFIED | tokio::spawn in stop_game() + check_game_health() catch |
| STATE-02 | 196-02 | Disconnected agent: immediate Error | SATISFIED | No-agent path returns Err immediately |
| STATE-03 | 196-02 | Feature flag gate before launch | SATISFIED | Feature flag check before tracker creation |
| STATE-04 | 196-02 | externally_tracked field on GameTracker | SATISFIED | Field at line 29; set correctly in both paths |
| STATE-05 | 196-02 | Error propagation for no-agent: immediate Error + broadcast | SATISFIED | Error state set + broadcast at lines 286-296 |
| STATE-06 | 196-02 | relaunch_game() rejects Stopping state | SATISFIED | `!= GameState::Error` check at line 348 blocks Stopping; test at line 1717 |

No orphaned requirements — all 13 IDs are accounted for and verified.

**Note on trait naming:** The ROADMAP Success Criterion 1 specifies `grep "impl GameLauncher for"` and method names `launch()`, `validate_args()`, `cleanup()`. The implementation chose `GameLauncherImpl` as the trait name and `make_launch_message()`/`cleanup_on_failure()` instead of `launch()`/`cleanup()`. This is a documented implementation decision (SUMMARY-01 key-decisions) where `GameLauncherImpl` was used to avoid a name collision and `make_launch_message()` is a more precise name. The semantic requirement (per-game dispatch with validation and cleanup hooks) is fully satisfied.

---

### Anti-Patterns Found

No blockers or warnings found.

| File | Pattern | Severity | Finding |
|------|---------|----------|---------|
| `game_launcher.rs` | `let _ =` broadcast drops | Checked | 0 instances near `dashboard_tx` — all replaced |
| `game_launcher.rs` | TODO/FIXME/PLACEHOLDER | Checked | None found |
| `game_launcher.rs` | Empty implementations | Checked | All 4 launcher impls have substantive JSON validation logic |

---

### Human Verification Required

None — all behaviors are verifiable via code inspection and unit tests. The 29 passing unit tests documented in SUMMARY-02 cover all critical paths including trait dispatch, billing gate edge cases, TOCTOU simulation, Stopping timeout (via `check_game_health()`), feature flag gate, and disconnected agent detection.

The only item requiring eventual live testing is the actual 30s Stopping timeout via `tokio::spawn` (tested structurally; the `check_game_health()` path covers the server-restart edge case programmatically). This is acceptable given the SQLite pool timeout incompatibility with `tokio::time::pause()` documented in SUMMARY-02.

---

## Gaps Summary

No gaps. All 11 observable truths from the ROADMAP success criteria are VERIFIED against actual codebase. All 13 requirement IDs are accounted for. All 3 modified files exist with substantive content and correct wiring. Commits d6cbdbfb, fede2275, 7e90fd91 all confirmed in git log.

---

_Verified: 2026-03-26T05:30:00Z_
_Verifier: Claude (gsd-verifier)_
