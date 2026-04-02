---
phase: 295-config-schema-validation
plan: 01
subsystem: config
tags: [rust, toml, serde, rc-common, rc-agent, config-schema, schema-version]

requires:
  - phase: none
    provides: existing rc-agent config.rs with AgentConfig struct

provides:
  - rc-common::config_schema module with AgentConfig + all sub-structs
  - schema_version field for forward compatibility (SCHEMA-04)
  - lenient TOML parsing with warnings for unknown/mistyped fields (SCHEMA-02, SCHEMA-03)
  - single source of truth for pod config — no struct duplication (SCHEMA-01)

affects:
  - phase-296-config-push-channel (imports AgentConfig from rc-common)
  - any future crate that needs AgentConfig (use rc_common::config_schema)

tech-stack:
  added:
    - toml added to rc-common Cargo.toml (for config_schema tests)
  patterns:
    - Shared config structs in rc-common, rc-agent re-exports via `pub use`
    - Two-pass lenient TOML deserialization (raw Value + typed fallback)
    - Feature-gated type conflict resolution via From<> conversion + explicit re-exports

key-files:
  created:
    - crates/rc-common/src/config_schema.rs
  modified:
    - crates/rc-common/src/lib.rs
    - crates/rc-common/Cargo.toml
    - crates/rc-agent/src/config.rs
    - crates/rc-agent/src/game_process.rs
    - crates/rc-agent/src/ai_debugger.rs
    - crates/rc-agent/src/main.rs
    - crates/rc-agent/src/event_loop.rs
    - crates/rc-agent/src/ws_handler.rs

key-decisions:
  - "AgentConfig.ai_debugger stays as rc_common stub type; ai-debugger feature uses From<> to convert to full type"
  - "GameExeConfig moved to rc-common, game_process.rs re-exports it via pub use"
  - "lenient_deserialize uses two-pass: full parse first, field-by-field fallback on type error"
  - "All sub-config structs add Clone + Serialize for Phase 296 WS push"

requirements-completed: [SCHEMA-01, SCHEMA-02, SCHEMA-03, SCHEMA-04]

duration: 45min
completed: 2026-04-01
---

# Phase 295 Plan 01: Config Schema Validation Summary

**Shared AgentConfig in rc-common with schema_version, lenient TOML parsing (warn on unknown/mistyped fields), and single-source-of-truth struct definitions**

## Performance

- **Duration:** ~45 min
- **Started:** 2026-04-01T12:00:00Z
- **Completed:** 2026-04-01T12:45:00Z
- **Tasks:** 2
- **Files modified:** 8

## Accomplishments

- Created `crates/rc-common/src/config_schema.rs` with all AgentConfig sub-structs (Clone + Serialize + Deserialize), schema_version field defaulting to 1
- Rewrote rc-agent config.rs to re-export from rc-common, implementing lenient two-pass TOML parsing that warns on unknown fields (SCHEMA-02) and falls back to defaults on type errors (SCHEMA-03)
- Eliminated duplicate GameExeConfig by moving it to rc-common, having game_process.rs re-export it
- Resolved ai-debugger feature-gate conflict via From<rc_common::AiDebuggerConfig> for crate::ai_debugger::AiDebuggerConfig

## Task Commits

1. **Task 1: Create config_schema.rs in rc-common** - `70fdb758` (feat)
2. **Task 2: Update rc-agent to re-export + lenient parsing** - `8566e6a5` (feat)

## Files Created/Modified

- `crates/rc-common/src/config_schema.rs` - Shared AgentConfig + all sub-structs with schema_version
- `crates/rc-common/src/lib.rs` - Added `pub mod config_schema`
- `crates/rc-common/Cargo.toml` - Added toml dependency for config_schema tests
- `crates/rc-agent/src/config.rs` - Re-exports from rc-common, lenient_deserialize(), validates, tests
- `crates/rc-agent/src/game_process.rs` - Replaced local GameExeConfig with rc-common re-export
- `crates/rc-agent/src/ai_debugger.rs` - Added From<rc_common::AiDebuggerConfig> conversion
- `crates/rc-agent/src/main.rs` - Added feature-gated AiDebuggerConfig conversion at self_monitor::spawn
- `crates/rc-agent/src/event_loop.rs` - Updated 2 analyze_crash calls with .into()
- `crates/rc-agent/src/ws_handler.rs` - Updated 2 analyze_crash calls with .into()

## Decisions Made

- **AiDebuggerConfig conflict:** The feature-gated `ai_debugger::AiDebuggerConfig` (with openrouter fields) and the rc-common stub are different types. Resolution: AgentConfig.ai_debugger stays as the common stub for TOML parsing; all sites calling `analyze_crash` convert via `.into()`. This avoids needing conditional compilation on the AgentConfig struct itself.
- **lenient_deserialize approach:** Chose two-pass strategy (full parse first, field-by-field fallback) rather than adding `serde_ignored` dependency. Simpler, no new dependency, handles 95% of real-world cases.
- **GameExeConfig:** Fully moved to rc-common with `pub use` re-export in game_process.rs. No migration cost since both structs had identical fields.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] ai-debugger feature caused AiDebuggerConfig type conflict**
- **Found during:** Task 2 (rc-agent re-export implementation)
- **Issue:** `pub use rc_common::config_schema::*` exported common `AiDebuggerConfig`, conflicting with `crate::ai_debugger::AiDebuggerConfig` when the ai-debugger feature was on. Caused 11 E0308 type mismatch errors in event_loop.rs, ws_handler.rs, main.rs.
- **Fix:** (1) Feature-conditional re-exports: when `ai-debugger` ON, use explicit named re-exports excluding AiDebuggerConfig. (2) Added `From<rc_common::AiDebuggerConfig>` impl in ai_debugger.rs. (3) Updated 5 call sites with `.into()`.
- **Files modified:** crates/rc-agent/src/config.rs, crates/rc-agent/src/ai_debugger.rs, crates/rc-agent/src/main.rs, crates/rc-agent/src/event_loop.rs, crates/rc-agent/src/ws_handler.rs
- **Verification:** cargo test -p rc-agent-crate -- config passes (46/46), racecontrol-crate compiles
- **Committed in:** 8566e6a5 (Task 2 commit)

**2. [Rule 1 - Bug] GameExeConfig duplicate caused type mismatch**
- **Found during:** Task 2
- **Issue:** game_process.rs had its own GameExeConfig struct; GamesConfig in rc-common uses rc-common's GameExeConfig. Type mismatch in event_loop.rs when passing config to game_process::launch.
- **Fix:** Removed game_process::GameExeConfig, added `pub use rc_common::config_schema::GameExeConfig`. All references now resolve to one type.
- **Files modified:** crates/rc-agent/src/game_process.rs
- **Verification:** cargo check -p racecontrol-crate passes
- **Committed in:** 8566e6a5 (Task 2 commit)

---

**Total deviations:** 2 auto-fixed (both Rule 1 — bugs discovered during refactoring)
**Impact on plan:** Both fixes essential for the re-export approach to work. No scope creep.

## Issues Encountered

- Initial AiDebuggerConfig conflict required understanding the `default = ["ai-debugger", ...]` feature in rc-agent Cargo.toml — the feature is always on in standard builds, making the conflict immediate.

## Known Stubs

None — all config fields are wired to real TOML parsing. The `AiDebuggerConfig` in rc-common is intentionally minimal (stub) because the full version is in ai_debugger.rs and requires the ai-debugger feature.

## Next Phase Readiness

- Phase 296 (config push channel) can now `use rc_common::config_schema::AgentConfig` directly without duplicating the struct
- Clone + Serialize derives on all sub-structs enable WS push in Phase 296
- schema_version field ready for forward-compatibility validation in Phase 296

---
*Phase: 295-config-schema-validation*
*Completed: 2026-04-01*
