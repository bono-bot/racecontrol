---
phase: 364-session-quality-monitor
plan: "02"
subsystem: session-quality
tags: [lap-consistency, 3-sigma, outlier-detection, billing-suspect, feature-flag]
dependency_graph:
  requires: [364-01]
  provides: [check_lap_consistency, check_outlier, recent_lap_times, CONSIST-01]
  affects: [billing_sessions.suspect_reasons, PodInfo.recent_lap_times]
tech_stack:
  added: []
  patterns: [rolling-window-stats, 3-sigma-outlier, snapshot-and-drop-lock, feature-flag-guard]
key_files:
  created:
    - crates/racecontrol/src/lap_consistency.rs
  modified:
    - crates/rc-common/src/types.rs
    - crates/racecontrol/src/lib.rs
    - crates/racecontrol/src/ws/mod.rs
    - crates/racecontrol/src/billing.rs
    - crates/racecontrol/src/api/routes.rs
    - crates/racecontrol/src/main.rs
    - crates/rc-agent/src/main.rs
decisions:
  - "recent_lap_times placed in rc-common PodInfo (not racecontrol state.rs) so all construction sites in routes.rs, main.rs, rc-agent/main.rs get the field atomically"
  - "Feature flag pattern matches Plan 01 (state.feature_flags.read snapshot) not non-existent is_feature_enabled()"
  - "check_outlier is pure (no AppState) enabling deterministic unit tests without runtime setup"
  - "Lock dropped before async DB call (CLAUDE.md never-hold-lock-across-await)"
  - "stddev < 2000ms guard prevents false positives in tight sessions (racing lap-times typically 1-3s std dev)"
metrics:
  duration_seconds: 0
  completed: "2026-04-11T06:27:17+05:30"
  tasks_completed: 6
  tasks_total: 6
  files_modified: 7
  tests_added: 5
  tests_total_pass: "racecontrol-crate lap_consistency: 5 passed, build green"
---

# Phase 364 Plan 02: Lap Consistency Checker (3-Sigma Outlier Detection) Summary

Rolling 3-sigma statistical outlier detector for LapCompleted events -- flags corrupt/anomalous lap times as `lap_outlier_lapN` in billing_sessions.suspect_reasons before session end.

## Changes Made

### Task 1: Add recent_lap_times to PodInfo
- Added `pub recent_lap_times: std::collections::VecDeque<u32>` to PodInfo in `rc-common/src/types.rs`
- Field has `#[serde(skip)]` (server-side runtime state, not serialized over wire)
- All construction sites updated: routes.rs (7 sites), main.rs, rc-agent/main.rs

### Task 2: Create lap_consistency.rs module
- New `crates/racecontrol/src/lap_consistency.rs` with:
  - `check_lap_consistency()` async function: feature flag guard + valid-lap filter + lock snapshot + outlier check + history append + DB write
  - `check_outlier()` pure function: 3-sigma with MIN_LAPS=3 and MIN_STDDEV=2000ms guards
  - 5 unit tests: fewer-than-3-laps guard, low-stddev guard, extreme-outlier flag, normal-variation no-flag, stddev boundary (high-variance flag + within-band no-flag)
- All tests pass: 5/5

### Task 3: Register module in lib.rs
- Added `pub mod lap_consistency;` at line 92 in `crates/racecontrol/src/lib.rs`

### Task 4: Wire into ws/mod.rs LapCompleted handler
- Added call to `crate::lap_consistency::check_lap_consistency(&state, &lap).await` after `persist_lap` in the `AgentMessage::LapCompleted` match arm
- Gated by `if lap.valid` (valid laps only)
- Comment: `// Phase 364 CONSIST-01: in-flight lap consistency check`

### Task 5: Clear lap history on session end in billing.rs
- Added `pod.recent_lap_times.clear()` in `post_session_hooks()` at line 4730
- Comment: `// Phase 364 CONSIST-01: clear rolling lap history to prevent stale data leaking to next session`
- Prevents cross-session contamination (customer A's lap history leaking into customer B's session)

### Task 6: Full suite green check
- `cargo test -p racecontrol-crate -- lap_consistency`: 5 passed, 0 failed
- `cargo build -p racecontrol-crate`: exits 0, no errors

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Feature flag pattern**
- **Found during:** Task 2 (implementation)
- **Issue:** Plan referenced `crate::feature_flags::is_feature_enabled(state, "name")` which does not exist (same issue as Plan 01)
- **Fix:** Used actual lock-snapshot pattern: `let guard = state.feature_flags.read().await; guard.get("phase364_quality_monitor").map(|r| r.enabled).unwrap_or(true)` -- guard drops at end of `{ }` block before any `.await`
- **Files modified:** crates/racecontrol/src/lap_consistency.rs

**2. [Rule 2 - Missing Functionality] PodInfo location**
- **Found during:** Task 1
- **Issue:** Plan said add `recent_lap_times` to `crates/racecontrol/src/state.rs`, but PodInfo is actually defined in `crates/rc-common/src/types.rs`. Multiple construction sites across 3 crates would not compile if only state.rs was updated.
- **Fix:** Added field to rc-common/src/types.rs (actual PodInfo location). Updated all 7+ construction sites across routes.rs, main.rs, rc-agent/main.rs.
- **Files modified:** crates/rc-common/src/types.rs, crates/racecontrol/src/api/routes.rs, crates/racecontrol/src/main.rs, crates/rc-agent/src/main.rs

## Commits

| Task | Commit | Description |
|------|--------|-------------|
| 1-5 | d70c9c4c | feat(364-02): lap consistency checker -- 3-sigma outlier detection (GLD-D-02) |

Note: Previous agent committed all 6 tasks atomically in a single commit (d70c9c4c). This is the complete implementation; no additional tasks remain.

## Known Stubs

None -- all data paths are wired end-to-end:
- LapCompleted event -> check_lap_consistency() -> check_outlier() -> append_suspect_reason() -> billing_sessions.suspect_reasons DB write
- Session end -> post_session_hooks() -> recent_lap_times.clear()

## Self-Check: PASSED

- `crates/racecontrol/src/lap_consistency.rs` exists (5631 bytes, 148 lines)
- `pub mod lap_consistency` in lib.rs line 92: confirmed
- `check_lap_consistency` in ws/mod.rs line 913: confirmed
- `recent_lap_times.clear()` in billing.rs line 4730: confirmed
- `recent_lap_times` field in rc-common/src/types.rs line 124: confirmed
- Commit d70c9c4c in git log: confirmed
- `cargo test -p racecontrol-crate -- lap_consistency`: 5 passed, 0 failed
- `cargo build -p racecontrol-crate`: exits 0
