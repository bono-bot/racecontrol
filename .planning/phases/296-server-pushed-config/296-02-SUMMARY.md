---
phase: 296-server-pushed-config
plan: "02"
subsystem: config
tags: [rust, rc-agent, websocket, hot-reload, config-push, sha256, boot-resilience]

requires:
  - phase: 296-01
    provides: "FullConfigPushPayload type in rc-common/types.rs, FullConfigPush variant in CoreToAgentMessage protocol, server-side config_push.rs sender"

provides:
  - "FullConfigPush handler in ws_handler.rs: hash dedup (PUSH-06), hot/cold reload (PUSH-03/04), local persist (PUSH-05)"
  - "persist_server_config / load_server_config in config.rs for boot resilience"
  - "load_config() fallback to rc-agent-server-config.json when no TOML found (PUSH-05)"
  - "HOT_RELOAD_CONFIG_FIELDS and COLD_CONFIG_FIELDS classification arrays"
  - "compute_config_hash_local() SHA-256 helper in ws_handler.rs"
  - "9 new tests: 5 in server_config_persistence_tests, 4 in ws_handler::tests"

affects:
  - "296-03 (if any): consumes persist_server_config and HOT/COLD field classification"
  - "Phase 297, 298, 299: depend on Phase 296 completing"

tech-stack:
  added: []
  patterns:
    - "Hot/cold field split: HOT fields applied immediately, COLD logged as pending-restart, both persisted for next boot"
    - "Boot resilience via local JSON cache: rc-agent-server-config.json in exe directory"
    - "Hash dedup: skip processing if config_hash matches current state hash (PUSH-06)"

key-files:
  created: []
  modified:
    - "crates/rc-agent/src/config.rs — persist_server_config, load_server_config, HOT/COLD field arrays, load_config fallback, 5 new tests"
    - "crates/rc-agent/src/ws_handler.rs — FullConfigPush match arm, apply_full_config(), compute_config_hash_local(), 4 new tests"

key-decisions:
  - "compute_config_hash_local implemented locally in ws_handler.rs (not imported from racecontrol crate) — racecontrol is not a dependency of rc-agent"
  - "apply_full_config replaces state.config unconditionally after logging hot/cold changes — cold fields stored in config for accurate future hash comparison but NOT applied to subsystems"
  - "kiosk_enabled in AppState updated immediately on kiosk.enabled hot-reload — all other hot-field changes logged only (subsystems read from state.config on next tick)"
  - "persist_server_config_to and load_server_config_from are pub(crate) testable variants; prod wrappers use server_config_path()"

patterns-established:
  - "TDD: RED (5+4 failing tests) → GREEN (implement functions) → REFACTOR (not needed)"
  - "Boot resilience pattern: load_config() tries TOML search paths first, falls back to server config JSON cache"

requirements-completed: [PUSH-03, PUSH-04, PUSH-05]

duration: 35min
completed: 2026-04-01
---

# Phase 296 Plan 02: Server-Pushed Config — Agent-Side Handler Summary

**FullConfigPush handler in rc-agent with SHA-256 hash dedup, hot/cold field reload split, and local JSON persistence for server-down boot resilience**

## Performance

- **Duration:** 35 min
- **Started:** 2026-04-01T14:15:00Z
- **Completed:** 2026-04-01T14:50:00Z
- **Tasks:** 1 (TDD)
- **Files modified:** 2

## Accomplishments

- Agent now handles `FullConfigPush` messages: compares hash (PUSH-06), skips if unchanged, persists locally (PUSH-05), applies hot fields immediately (PUSH-03), logs cold fields as pending-restart (PUSH-04)
- `load_config()` fallback chain: TOML search paths first, then `rc-agent-server-config.json` cache — pods survive server-unreachable boots
- 9 new tests all pass: 5 in `server_config_persistence_tests`, 4 in `ws_handler::tests`
- Existing `ConfigPush` handler preserved untouched for v22.0 backward compatibility

## Task Commits

1. **Task 1: FullConfigPush handler — hash dedup + hot/cold reload + local persist** - `5384bc48` (feat)

## Files Created/Modified

- `crates/rc-agent/src/config.rs` — Added `persist_server_config`, `load_server_config`, `server_config_path`, `HOT_RELOAD_CONFIG_FIELDS`, `COLD_CONFIG_FIELDS`, load_config fallback, testable `_to`/`_from` variants, 5 tests
- `crates/rc-agent/src/ws_handler.rs` — Added `FullConfigPush` match arm, `apply_full_config()`, `compute_config_hash_local()`, 4 tests

## Decisions Made

- `compute_config_hash_local` implemented locally (not imported from racecontrol) — racecontrol is a separate crate, rc-agent cannot depend on it. Same SHA-256 algorithm; must stay in sync.
- `apply_full_config` replaces `state.config` with full new config even for cold-field-only changes, so subsequent hash comparisons use the "last received" baseline accurately.
- `kiosk_enabled` in AppState is updated immediately (it's the live gate for kiosk behavior); all other hot fields are logged and picked up by subsystems on their next tick from `state.config`.
- Testable `persist_server_config_to(path)` / `load_server_config_from(path)` variants avoid needing to mock exe directory in tests.

## Deviations from Plan

None — plan executed exactly as written.

## Issues Encountered

The changes were committed by a context-doc agent (commit `5384bc48`) which bundled our implementation with a `300-CONTEXT.md` doc commit. The code is correctly committed and all tests pass.

Pre-existing `racecontrol-crate` integration test failures (`BillingTimer` missing `nonce` field) — confirmed pre-existing by stash test, not caused by this plan.

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

- Phase 296 plan 02 complete: agent can receive, validate, persist, and apply `FullConfigPush` from server
- Requirements PUSH-03, PUSH-04, PUSH-05 satisfied
- Phases 297, 298, 299 can now proceed (all depend on Phase 296 completing)
- Pending: git push to origin (auto-push rule)

---
*Phase: 296-server-pushed-config*
*Completed: 2026-04-01*
