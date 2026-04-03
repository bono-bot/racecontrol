---
phase: 316-agent-content-scanner-boot-validation
plan: 02
subsystem: rc-agent/content_scanner + rc-agent/ws_handler
tags: [combo-validation, preset-push, boot-validation, content-scanner, ws-handler]
requirements: [COMBO-01, COMBO-02]
dependency_graph:
  requires: [316-01-SUMMARY]
  provides: [validate_ac_combo, validate_ac_combos, find_ac_base_path, PresetPush-handler, ComboValidationReport-send]
  affects: [rc-agent/content_scanner.rs, rc-agent/ws_handler.rs]
tech_stack:
  added: []
  patterns: [tokio::task::spawn_blocking, tokio::spawn, mpsc::Sender.send().await, fail-open filesystem checks]
key_files:
  created: []
  modified:
    - crates/rc-agent/src/content_scanner.rs
    - crates/rc-agent/src/ws_handler.rs
decisions:
  - "validate_ac_combos_at(pod_id, presets, ac_base) internal variant enables testing without global path injection"
  - "unwrap_or_default() on spawn_blocking JoinHandle is intentional — panic in pure filesystem walk returns empty vec rather than propagating"
  - "AI lines check uses existing check_has_ai(track_dir, '') — reuses proven logic for default layout"
  - "Sequencing gate is structural: validation only runs inside PresetPush handler, impossible to fire before presets arrive"
metrics:
  duration: "7m 10s"
  completed_date: "2026-04-03"
  tasks_completed: 2
  tasks_total: 2
  files_modified: 2
  tests_added: 9
  tests_total: 37
---

# Phase 316 Plan 02: AC Combo Validation & PresetPush Handler Summary

**One-liner:** Boot-time AC preset filesystem validation via validate_ac_combo/validate_ac_combos in content_scanner, gated by PresetPush WS handler that sends ComboValidationReport after spawn_blocking filesystem walk.

## What Was Built

### Task 1: validate_ac_combo and validate_ac_combos (content_scanner.rs)

Three new public/internal functions:

1. **`find_ac_base_path() -> Option<PathBuf>`** — Probes default Steam path (`C:\Program Files (x86)\Steam\steamapps\common\assettocorsa`) then 4 alternate drive letter paths. Returns `None` if not found.

2. **`validate_ac_combo(pod_id, preset, ac_base) -> ComboValidationResult`** — Validates one AC preset:
   - Returns `GameNotInstalled` if `ac_base` is None or path doesn't exist
   - Returns `Unknown` with `"not_ac_preset"` if preset.game != "assettoCorsa"
   - Checks `{ac_base}/content/cars/{car}` is a dir (if car is Some)
   - Checks `{ac_base}/content/tracks/{track}` is a dir (if track is Some)
   - Checks AI lines via `check_has_ai(track_dir, "")` — reuses existing proven function
   - All checked paths recorded in `checked_paths` for debugging
   - Returns `Available` if no failures, `Invalid` with reasons if any check fails
   - No `.unwrap()` in production code

3. **`validate_ac_combos_at(pod_id, presets, ac_base) -> Vec<ComboValidationResult>`** — Internal testable variant that accepts injected `ac_base`.

4. **`validate_ac_combos(pod_id, presets) -> Vec<ComboValidationResult>`** — Public production function:
   - Resolves `ac_base` via `find_ac_base_path()`
   - Filters to AC-only presets (`game == "assettoCorsa"`)
   - Calls `validate_ac_combo` for each AC preset
   - Logs INFO summary: `"Combo validation complete: N/M AC presets checked (X available, Y invalid)"`

### Task 2: PresetPush handler in ws_handler.rs

Added a `CoreToAgentMessage::PresetPush(payload)` match arm after the `FullConfigPush` handler:

- Logs `"Presets received: N preset(s) from server"` before spawning validation
- If presets non-empty: spawns tokio task → spawn_blocking → validate_ac_combos → send ComboValidationReport via `ws_exec_result_tx.send().await`
- If presets empty: logs WARN `"PresetPush received with 0 presets — skipping combo validation"` — no validation runs
- Sequencing guarantee is structural: validation only runs inside this handler, so "Presets received" log always precedes "Combo validation complete"

## Tests

37 total tests pass (28 existing + 9 new):

**9 new tests (Task 1):**
- `test_validate_ac_combo_all_present_returns_available` — car + track + ai all present → Available
- `test_validate_ac_combo_missing_car_returns_invalid` — missing car dir → Invalid with "car '...' not found"
- `test_validate_ac_combo_missing_track_returns_invalid` — missing track dir → Invalid with "track '...' not found"
- `test_validate_ac_combo_empty_ai_returns_invalid` — empty ai/ dir → Invalid with "ai lines missing"
- `test_validate_ac_combo_no_car_no_track_returns_available` — both None → Available (nothing to check)
- `test_validate_ac_combo_no_ac_base_returns_game_not_installed` — nonexistent path → GameNotInstalled
- `test_validate_ac_combos_skips_non_ac_presets` — 2 AC + 1 F1 preset → exactly 2 results
- `test_validate_ac_combos_empty_presets_returns_empty` — empty vec → empty vec, no panic
- `test_validate_ac_combo_checked_paths_contains_tested_paths` — car and track paths in checked_paths

## Commits

| Task | Commit | Description |
|------|--------|-------------|
| Task 1 | `562cadb7` | feat(316-02): add validate_ac_combo, validate_ac_combos, find_ac_base_path to content_scanner |
| Task 2 | `f6343a00` | feat(316-02): add PresetPush handler in ws_handler.rs — AC combo validation gated on preset receipt |

## Deviations from Plan

### Auto-fixed Issues

None — plan executed exactly as written.

### Minor Adjustments

**1. validate_ac_combos_at exposed as pub(crate) for testability**
- The plan specified `validate_ac_combos(pod_id, presets)` calling `find_ac_base_path()`. Tests for Test 7 needed to inject an arbitrary base path.
- Added `validate_ac_combos_at(pod_id, presets, ac_base)` as `pub(crate)` internal variant used by both tests and `validate_ac_combos`.
- The public `validate_ac_combos` API is unchanged from spec.

**2. ComboValidationResult import at crate level (not test-only)**
- The `use rc_common::types::{..., ComboValidationResult, ...}` import was added to the top-level use statement since `validate_ac_combo` returns this type in production code.
- Tests removed the duplicate local import that was initially scaffolded in the test block.

## Known Stubs

None. Both functions are fully implemented with real filesystem I/O. The `validate_ac_combo` function probes live paths at runtime — no mock data.

## Self-Check

Files created/modified:
- `crates/rc-agent/src/content_scanner.rs` — FOUND (verified)
- `crates/rc-agent/src/ws_handler.rs` — FOUND (verified)

Commits:
- `562cadb7` — FOUND (verified by git log)
- `f6343a00` — FOUND (verified by git log)

## Self-Check: PASSED
