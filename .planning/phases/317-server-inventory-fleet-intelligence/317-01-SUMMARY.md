---
phase: 317-server-inventory-fleet-intelligence
plan: 01
subsystem: racecontrol/game_inventory
tags: [game-inventory, combo-validation, fleet-validity, server-persistence, whatsapp-alert]
requirements: [INV-02, COMBO-03, COMBO-04]
dependency_graph:
  requires: [316-01-SUMMARY, 316-02-SUMMARY]
  provides: [pod_game_inventory-table, combo_validation_flags-table, game_inventory.rs, fleet_validity-field]
  affects: [racecontrol/db/mod.rs, racecontrol/ws/mod.rs, racecontrol/preset_library.rs, rc-common/types.rs]
tech_stack:
  added: []
  patterns: [tokio::spawn fire-and-forget, INSERT OR REPLACE upsert, compute_fleet_validity aggregation]
key_files:
  created:
    - crates/racecontrol/src/game_inventory.rs
  modified:
    - crates/racecontrol/src/db/mod.rs
    - crates/racecontrol/src/lib.rs
    - crates/racecontrol/src/ws/mod.rs
    - crates/racecontrol/src/preset_library.rs
    - crates/rc-common/src/types.rs
decisions:
  - "GameInventoryUpdate + ComboValidationReport handlers are fire-and-forget tokio::spawn — WS loop never blocked"
  - "INSERT OR REPLACE upsert pattern for both tables — latest scan always wins, no conflicts"
  - "compute_fleet_validity counts only pods that have reported (total from combo_validation_flags), not all 8 pods — avoids false 'partial' when pods haven't validated yet"
  - "auto_disable_invalid_presets checks rows_affected > 0 before alerting — prevents duplicate WhatsApp alerts on repeated ComboValidationReport for same preset"
  - "fleet_validity uses #[serde(default)] so old agents/kiosks that don't understand the field get 'unknown' on deserialize"
  - "WhatsApp alert is NEVER sent while holding a DB transaction or lock — spawned separately per standing rule"
metrics:
  duration: "14m 51s"
  completed_date: "2026-04-03"
  tasks_completed: 4
  tasks_total: 4
  files_modified: 5
  files_created: 1

# Phase 317 Plan 01: Server Inventory & Fleet Intelligence (DB + WS Handlers + fleet_validity)

## One-liner

SQLite pod_game_inventory + combo_validation_flags tables, server-side GameInventoryUpdate/ComboValidationReport WS handlers with auto-disable logic, and fleet_validity field on GET /api/v1/presets.

## What Was Built

### Task 1: DB Migrations (db/mod.rs)

Added two new tables to the `migrate()` function:

- **pod_game_inventory** (INV-02): PRIMARY KEY (pod_id, game_id), stores game scan results from agents. Upserted on each GameInventoryUpdate message. Indexed on game_id for fleet matrix queries.

- **combo_validation_flags** (COMBO-03/04): PRIMARY KEY (pod_id, preset_id), stores ComboValidationResult data from agents. Upserted on each ComboValidationReport message. Indexed on preset_id for fleet validity aggregation.

Migration test `test_game_intelligence_tables_exist` confirms both tables exist after `migrate()`.

### Task 2: game_inventory.rs (new file)

6 public functions:

1. `upsert_pod_game_inventory(db, inventory)` — INSERT OR REPLACE loop over `inventory.games`
2. `upsert_combo_validation_flags(db, pod_id, results)` — INSERT OR REPLACE, failure_reasons as JSON string
3. `compute_fleet_validity(db, preset_id)` — SQL COUNT + SUM → "valid"/"partial"/"invalid"/"unknown"
4. `auto_disable_invalid_presets(db, config, preset_id, preset_name)` — UPDATE game_presets WHERE enabled=1, returns `bool` (true = newly disabled, caller spawns alert)
5. `handle_game_inventory_update(state, inventory)` — fire-and-forget wrapper
6. `handle_combo_validation_report(state, pod_id, results)` — fire-and-forget wrapper, spawns WhatsApp alert when auto_disable returns true

### Task 3: WS Handlers (ws/mod.rs)

Two new match arms before the `_` catch-all:

```rust
AgentMessage::GameInventoryUpdate(inventory) => { /* tokio::spawn handle_game_inventory_update */ }
AgentMessage::ComboValidationReport { pod_id, results } => { /* tokio::spawn handle_combo_validation_report */ }
```

### Task 4: fleet_validity field (types.rs + preset_library.rs)

- `GamePresetWithReliability` gains `fleet_validity: String` with `#[serde(default)]` for backward compatibility
- `list_presets_with_reliability` calls `compute_fleet_validity` per preset before pushing to result vec
- All 5 test construction sites in types.rs updated

## Verification Summary

- `cargo check -p racecontrol-crate` — 0 errors
- `cargo check -p rc-common` — 0 errors
- `cargo test -p racecontrol-crate test_game_intelligence` — 1 passed, 0 failed
- `grep -n "GameInventoryUpdate\|ComboValidationReport" ws/mod.rs` — both handlers present
- `grep -n "fleet_validity" types.rs` — field present at line 1029-1030
- `grep -n "fleet_validity" preset_library.rs` — queried at line 103, used at line 112
- `grep -c "\.unwrap()" game_inventory.rs` — 0

## Deviations from Plan

### Auto-fixed Issues

None — plan executed exactly as written.

## Known Stubs

None — all data flows wired. fleet_validity returns "unknown" when no combo_validation_flags exist for a preset (correct behavior: agents haven't reported yet).

## Self-Check: PASSED

- game_inventory.rs: FOUND
- db/mod.rs migration: FOUND (pod_game_inventory + combo_validation_flags at lines 516-566)
- ws/mod.rs handlers: FOUND at lines 1942 + 1956
- types.rs fleet_validity field: FOUND at line 1029
- preset_library.rs compute_fleet_validity call: FOUND at line 103
- Commits: 461a6624, 3370e8c5, 62316bd5, f9fc4df0 — all exist in git log
