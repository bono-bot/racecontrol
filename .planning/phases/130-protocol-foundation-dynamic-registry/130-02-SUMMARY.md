---
phase: 130-protocol-foundation-dynamic-registry
plan: "02"
subsystem: comms-link
tags: [v18.0, protocol, dynamic-registry, exec-handler, persistence, http-endpoint, wiring]

# Dependency graph
requires:
  - phase: 130-01
    provides: DynamicCommandRegistry class with Map storage, binary allowlist, env isolation, toJSON/fromJSON
provides:
  - ExecHandler with dynamic-first/static-fallback command lookup (DREG-04)
  - completedExecs LRU eviction cap at 10000 entries (tech debt resolved)
  - POST /relay/registry/register HTTP endpoint on James (DREG-01)
  - WS registry_register handler on both James and Bono (DREG-01 "either side")
  - data/dynamic-commands.json persistence on both sides with startup reload
  - Integration test suite for unified lookup and env isolation
affects: [james/index.js, bono/index.js, james/exec-handler.js, chain-executor, registry-handler]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - dynamic-first static-fallback pattern (dynamicRegistry?.get(command) ?? commandRegistry[command])
    - LRU eviction via insertion-ordered Set delete+add cycle
    - fire-and-forget async IIFE for non-blocking startup persistence in sync wireBono()

key-files:
  created:
    - C:/Users/bono/racingpoint/comms-link/test/dynamic-registry-integration.test.js
  modified:
    - C:/Users/bono/racingpoint/comms-link/james/exec-handler.js
    - C:/Users/bono/racingpoint/comms-link/james/index.js
    - C:/Users/bono/racingpoint/comms-link/bono/index.js

key-decisions:
  - "wireBono() is a sync function -- persistence loading uses fire-and-forget async IIFE to avoid making wireBono async (avoids call-site changes)"
  - "LRU eviction uses Set insertion order property -- delete oldest (values().next().value) before add"
  - "#trackCompleted() private method centralizes both eviction check and add -- all 4 call sites updated"
  - "bono/index.js uses ws.send(JSON.stringify(createMessage(...))) for registry_ack -- matches existing WS send pattern in wireBono"

patterns-established:
  - "dual-registry lookup: dynamicRegistry?.get(name) ?? staticRegistry[name] -- check dynamic first, null-safe optional chaining"
  - "#trackCompleted() pattern -- extract eviction+add to private method for DRY completedExecs management"

requirements-completed: [DREG-03, DREG-04]

# Metrics
duration_minutes: 6
tasks_completed: 2
files_created: 1
files_modified: 3
tests_written: 6
completed_date: "2026-03-22"
---

# Phase 130 Plan 02: Dynamic Registry Wiring Summary

**ExecHandler wired with dynamic-first/static-fallback lookup, completedExecs LRU-capped at 10000, HTTP /relay/registry/register endpoint and WS registry_register handler deployed on both James and Bono with JSON persistence.**

---

## Performance

- **Duration:** 6 min
- **Started:** 2026-03-21T21:36:17Z
- **Completed:** 2026-03-21T21:43:07Z
- **Tasks:** 2
- **Files modified:** 4 (3 modified + 1 created)

## Accomplishments

- ExecHandler: `#dynamicRegistry` field added, command lookup uses `dynamicRegistry?.get(command) ?? commandRegistry[command]` pattern (DREG-04)
- ExecHandler: Per-command env isolation via `buildCommandEnv()` for dynamic commands, `#safeEnv` for static (DREG-05)
- ExecHandler: `#trackCompleted()` private method with LRU eviction -- `completedExecs` capped at 10000 entries (tech debt resolved)
- James: `POST /relay/registry/register`, `DELETE /relay/registry/:name`, `GET /relay/registry/list` HTTP endpoints + WS `registry_register` handler
- Bono: `bonoDynamicRegistry` instantiated, passed to `bonoExecHandler`, WS `registry_register` handler wired
- Both sides: `data/dynamic-commands.json` persistence with startup reload
- 6 integration tests all passing: dynamic lookup, static fallback, env isolation, empty env keys, LRU eviction, backward compat

## Task Commits

1. **Task 1: Add dynamic registry support to ExecHandler + fix completedExecs** - `2ee10f2` (feat)
2. **Task 2: HTTP registration endpoint + persistence + WS handler wiring (James and Bono)** - `43f9faf` (feat)

## Files Created/Modified

- `james/exec-handler.js` - Added `#dynamicRegistry` field, dual-lookup in `handleExecRequest`, per-command env isolation in `#execute`, `#trackCompleted()` with LRU eviction replacing all `#completedExecs.add()` calls
- `test/dynamic-registry-integration.test.js` - 6 integration tests for unified lookup, env isolation, LRU eviction, backward compat
- `james/index.js` - DynamicCommandRegistry import, instantiation, `persistDynamicRegistry()`, startup load from disk, ExecHandler `dynamicRegistry` option, HTTP endpoints, WS `registry_register` handler
- `bono/index.js` - DynamicCommandRegistry import, `bonoDynamicRegistry` instantiation, `persistBonoDynamicRegistry()`, startup load (async IIFE), `bonoExecHandler` `dynamicRegistry` option, WS `registry_register` handler

## Decisions Made

- `wireBono()` is a synchronous export function -- loading persistence with `await` at the top level was not possible without making the function async and updating all call sites. Used a fire-and-forget async IIFE (`(async () => { ... })()`) instead -- clean and non-blocking.
- LRU eviction exploits JavaScript `Set` insertion-order iteration: `values().next().value` gives the oldest entry in O(1). Delete then add maintains ordering correctly.
- Bono WS message sends use `ws.send(JSON.stringify(createMessage(...)))` matching the pattern already established in wireBono's exec_request/task_request handlers.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- One pre-existing test failure in `test/exec-handler.test.js` ("execFileFn is called with safeEnv as env option") was already present before this plan (documented in 130-01-SUMMARY.md). The test sends `reason: 'test'` via `makeMsg()` which causes `EXEC_REASON: 'test'` to be added to the env, but the test asserts `deepStrictEqual(capturedEnv, SAFE_ENV)`. Not caused by this plan. Deferred.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Dynamic command registration is fully end-to-end: either side can register a command via HTTP (James) or WS `registry_register` (either side) and invoke it immediately
- Static COMMAND_REGISTRY completely untouched -- all 20 static commands work identically
- Persistence survives restarts on both James and Bono
- Ready for chain execution (v18.0 chain_request/chain_result protocol types already in place from Plan 01)

---
*Phase: 130-protocol-foundation-dynamic-registry*
*Completed: 2026-03-22*
