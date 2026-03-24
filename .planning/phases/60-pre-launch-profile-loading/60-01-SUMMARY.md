---
phase: 60
plan: 01
status: complete
started: 2026-03-25
completed: 2026-03-25
commits:
  - hash: "08ebc5ad"
    message: "feat(60-01): implement pre_load_game_preset with TDD"
  - hash: "6345a0c2"
    message: "feat(60-02): wire pre-launch hook into LaunchGame handler"
---

# Plan 60-01 Summary: Pre-Launch FFB Preset Loading

## What was built

### Task 1: pre_load_game_preset with TDD (ffb_controller.rs)

5 new functions implementing pre-launch FFB preset loading:

1. **`sim_type_to_game_key(SimType)`** — maps SimType variants to ConspitLink game keys:
   - AC, ACRally → "Assetto Corsa" | F125 → "F1 25" | ACE → "ASSETTO_CORSA_EVO"
   - IRacing, LMU, Forza, ForzaH5 → None (unrecognized)

2. **`force_preset_via_global_json(key, dir)`** — writes `LastUsedPreset` to Global.json via serde_json parse-modify-write. Returns Err on failure, never panics.

3. **`apply_unrecognized_game_fallback(sim_type)`** — 50% power cap via `set_gain(50)` + gentle centering via `set_idle_spring(500)`. HID failures are non-fatal.

4. **`wait_for_cl_or_force_preset(key, dir)`** — if CL running: trust auto-detect (3s grace). If CL NOT running: start CL, force preset, restart CL. Respects SESSION_END_IN_PROGRESS guard.

5. **`pre_load_game_preset(sim_type, dir)`** — public entry point dispatching recognized → wait/force, unrecognized → safe fallback.

7 unit tests: 3 for sim_type_to_game_key (recognized/unrecognized/rally), 2 for force_preset (write/missing), 2 for pre_load (recognized/unrecognized).

### Task 2: Wire hook into LaunchGame handler (ws_handler.rs)

Inserted `spawn_blocking(pre_load_game_preset)` call between safe mode entry block and game-specific spawn branches. All 3 result cases (Ok/Err/Panic) handled with appropriate logging. Game launch ALWAYS proceeds.

## Requirements satisfied

| Req | How |
|-----|-----|
| PROF-03 | pre_load_game_preset called before game spawn, forces correct preset for recognized games |
| PROF-05 | Unrecognized games get 50% power + 500 idlespring, Global.json untouched |

## Test results

- 44/44 ffb_controller tests pass (7 new + 37 existing)
- 169/169 rc-common tests pass
- Build succeeds (dev profile)

## Decisions made

- AssettoCorsaRally → "Assetto Corsa" (shares AC physics engine)
- Only force preset when CL was NOT running (trusts Phase 59 auto-detect otherwise)
- idlespring fallback = 500 (gentle), not 2000 (avoids snap-back)
- Pre-load failure non-fatal — game always launches
