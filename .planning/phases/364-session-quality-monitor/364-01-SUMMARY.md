---
phase: 364-session-quality-monitor
plan: "01"
subsystem: session-quality
tags: [telemetry, quality-monitoring, billing-suspect, feature-flag]
dependency_graph:
  requires: []
  provides: [TelemetryQualityGap-variant, SessionStalled-variant, append_suspect_reason-helper, phase364_quality_monitor-flag]
  affects: [billing_sessions.suspect_reasons, failure_monitor-detection-rules]
tech_stack:
  added: []
  patterns: [json_insert-atomic-append, feature-flag-guard, snapshot-and-drop-lock]
key_files:
  created: []
  modified:
    - crates/rc-common/src/protocol.rs
    - crates/rc-agent/src/failure_monitor.rs
    - crates/racecontrol/src/bot_coordinator.rs
    - crates/racecontrol/src/ws/mod.rs
    - crates/racecontrol/src/db/mod.rs
decisions:
  - "Used actual feature_flags RwLock pattern instead of non-existent is_feature_enabled() function"
  - "Snapshot-and-drop lock pattern for all RwLock reads (CLAUDE.md compliance)"
  - "session_id field name (not billing_session_id) matching BillingTimer struct"
metrics:
  duration_seconds: 2226
  completed: "2026-04-11T00:57:01Z"
  tasks_completed: 6
  tasks_total: 6
  files_modified: 5
  tests_added: 12
  tests_total_pass: "rc-common all + rc-agent 775 + racecontrol 928"
---

# Phase 364 Plan 01: TelemetryQualityGap + SessionStalled Detection Summary

Wire >500ms UDP silence (TelemetryQualityGap) and 15s telemetry stall (SessionStalled) as in-flight session quality signals with atomic suspect_reasons append via json_insert.

## Changes Made

### Task 1: Protocol Variants (rc-common)
- Added `TelemetryQualityGap { pod_id, gap_ms }` variant to AgentMessage enum
- Added `SessionStalled { pod_id, silence_seconds }` variant to AgentMessage enum
- Both use automatic snake_case serde serialization
- Added 2 JSON roundtrip tests confirming correct serialization

### Task 2: Agent Detection Rules (rc-agent)
- Added `QUALITY_GAP_MS = 500` and `STALL_WARN_SECS = 15` constants
- Added `quality_gap_fired` and `stall_warn_fired` suppression flags (fire-once-per-silence-window)
- QUALITY-01: Fires TelemetryQualityGap when `last_udp_secs_ago * 1000 >= 500` (1s floor approximation)
- STALL-01: Fires SessionStalled when `last_udp_secs_ago >= 15`
- Both reset when UDP data resumes; both reset on billing stop/game exit
- Added 4 unit tests for threshold logic

### Task 3: Server Handlers (bot_coordinator)
- Added `append_suspect_reason()` helper using `json_insert(suspect_reasons, '$[#]', reason)` for atomic append
- Added `handle_telemetry_quality_gap()` with feature flag guard, GameState::Running guard, billing guard
- Added `handle_session_stalled()` with same guards
- All handlers use snapshot-and-drop lock pattern (CLAUDE.md never-hold-lock-across-await)
- Quality gap buckets gap_ms to nearest 500ms for reason string
- Added 5 unit tests for guard logic and bucket rounding

### Task 4: WS Routing (ws/mod.rs)
- Added match arms for TelemetryQualityGap and SessionStalled in agent message dispatch
- Both route to their respective bot_coordinator handlers

### Task 5: Feature Flag (db/mod.rs)
- Seeded `phase364_quality_monitor` flag with `enabled=1` via INSERT OR IGNORE
- Added migration test confirming flag is seeded correctly

### Task 6: Full Suite Verification
- rc-common: all tests pass
- rc-agent-crate: 775 passed, 0 failed
- racecontrol-crate: 928 passed, 0 failed
- Zero regressions

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Feature flag access pattern mismatch**
- **Found during:** Task 3
- **Issue:** Plan referenced `crate::feature_flags::is_feature_enabled(state, "name")` which does not exist
- **Fix:** Used actual pattern from Phase 363: `state.feature_flags.read().await.get("name").map(|r| r.enabled).unwrap_or(true)` with snapshot-and-drop
- **Files modified:** crates/racecontrol/src/bot_coordinator.rs

**2. [Rule 3 - Blocking] BillingTimer field name**
- **Found during:** Task 3
- **Issue:** Plan used `t.billing_session_id` but BillingTimer struct has `t.session_id`
- **Fix:** Used correct field name `t.session_id`
- **Files modified:** crates/racecontrol/src/bot_coordinator.rs

## Commits

| Task | Commit | Description |
|------|--------|-------------|
| 1 | df52d1fb | feat(364-01): add TelemetryQualityGap + SessionStalled protocol variants |
| 2 | 8edfa9ba | feat(364-01): add QUALITY-01 and STALL-01 detection in failure_monitor |
| 3-5 | 37f4e49c | feat(364-01): add server handlers, WS routing, and feature flag |

## Known Stubs

None -- all data paths are wired end-to-end from agent detection through server handler to DB write.

## Self-Check: PASSED

- All 5 modified files exist on disk
- All 3 commit hashes verified in git log
- TelemetryQualityGap found in 4 crate files (protocol, failure_monitor, ws/mod, bot_coordinator)
- SessionStalled found in same 4 crate files
- rc-common tests: all pass
- rc-agent-crate: 775 pass, 0 fail
- racecontrol-crate: 928 pass, 0 fail
