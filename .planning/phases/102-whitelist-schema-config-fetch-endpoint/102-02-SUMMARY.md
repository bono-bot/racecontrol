---
phase: 102-whitelist-schema-config-fetch-endpoint
plan: "02"
subsystem: api
tags: [process-guard, whitelist, axum, rust, merge-logic, http-endpoint]

requires:
  - phase: 102-01
    provides: ProcessGuardConfig/AllowedProcess/ProcessGuardOverride structs in config.rs; Config.process_guard field
  - phase: 101-protocol-foundation
    provides: MachineWhitelist type in rc-common/src/types.rs

provides:
  - process_guard::merge_for_machine() pure function merging global + per-machine overrides into MachineWhitelist
  - process_guard::machine_type_for_id() mapping pod-1..pod-8/james/server to machine_type string
  - process_guard::get_whitelist_handler: GET /api/v1/guard/whitelist/{machine_id} HTTP endpoint
  - Route registered in public_routes() in api/routes.rs (no auth, internal LAN)
  - 8 unit tests covering all merge scenarios and machine type mapping

affects:
  - 103-process-scan (rc-agent fetches whitelist via this endpoint on WS connect)
  - 104-reporting (violation reporting uses machine_id validated by same machine_type_for_id logic)
  - 105-enforcement (enforcement uses merged whitelist from this endpoint)

tech-stack:
  added: []
  patterns:
    - "HashSet-based merge: build process set from global allowed, apply deny removals, then add extras — produces sorted Vec for deterministic JSON"
    - "machine_type_for_id maps concrete IDs (pod-1..pod-8) to type strings (pod/james/server); None for unknowns → 404"
    - "Handler reads state.config.process_guard directly — no separate AppState field needed, consistent with state.config.watchdog pattern"

key-files:
  created:
    - crates/racecontrol/src/process_guard.rs
  modified:
    - crates/racecontrol/src/api/routes.rs
    - crates/rc-common/src/protocol.rs

key-decisions:
  - "No guard_config field on AppState — handler reads state.config.process_guard directly, consistent with how watchdog/bono/gmail sections are accessed"
  - "Route added to public_routes() not staff_routes() — pods call this on WS connect before any auth session exists"
  - "process_guard.rs was already committed (add07ff, 41bcefa) by a previous session — this session verified tests pass and fixed the blocking rc-common compile error"

patterns-established:
  - "Process guard merge: global allowed filtered by machine_type, then per-machine deny subtracted, then allow_extra added; all lowercased; output sorted for determinism"
  - "machine_type_for_id: pod-N only valid for N in 1..=8; anything else returns None; callers convert None to 404"

requirements-completed: [GUARD-06]

duration: 20min
completed: 2026-03-21
---

# Phase 102 Plan 02: Process Guard Whitelist Fetch Endpoint Summary

**GET /api/v1/guard/whitelist/{machine_id} endpoint with merge_for_machine() logic: global entries filtered by machine type, per-machine deny/allow_extra overrides applied, returns sorted MachineWhitelist JSON or 404 for unknown machine IDs**

## Performance

- **Duration:** 20 min
- **Started:** 2026-03-21T08:00:00Z
- **Completed:** 2026-03-21T08:27:38Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments

- Verified `crates/racecontrol/src/process_guard.rs` is complete with `merge_for_machine()`, `machine_type_for_id()`, `get_whitelist_handler`, and 8 unit tests — all previously committed by `add07ff`
- Verified `GET /api/v1/guard/whitelist/{machine_id}` route is registered in `public_routes()` — previously committed by `41bcefa`
- Fixed pre-existing blocking compile error in `rc-common/src/protocol.rs`: duplicate `EnterFreedomMode` and `ExitFreedomMode` variants in `CoreToAgentMessage` prevented all racecontrol tests from running
- All 8 process_guard tests pass; full suite at 60/66 pass (6 pre-existing billing/notification failures unrelated to this work)

## Task Commits

Each task was committed atomically:

1. **Task 1: Create process_guard.rs with merge logic and HTTP handler** - `add07ff` (feat) [previous session]
2. **Task 2: Wire process_guard into lib.rs, state.rs, route registration** - `41bcefa` (feat) [previous session]
3. **Deviation fix: Remove duplicate CoreToAgentMessage variants** - `ad364f3` (fix) [this session]

## Files Created/Modified

- `crates/racecontrol/src/process_guard.rs` - merge_for_machine(), machine_type_for_id(), get_whitelist_handler, 8 tests
- `crates/racecontrol/src/api/routes.rs` - Added `use crate::process_guard` and `.route("/guard/whitelist/{machine_id}", get(process_guard::get_whitelist_handler))` in public_routes()
- `crates/rc-common/src/protocol.rs` - Removed duplicate EnterFreedomMode/ExitFreedomMode variants (E0428 compile errors)

## Decisions Made

- No separate `guard_config` field on AppState — `state.config.process_guard` is sufficient; consistent with how other optional config sections (`watchdog`, `bono`, `gmail`) are accessed
- Route placed in `public_routes()` — pods call this before any auth session exists, must be accessible without JWT
- Steam entries belong in pod `deny_processes` only (not global allowed) — enforces v12.1 trigger incident rule (from Plan 01, carried forward)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Removed duplicate EnterFreedomMode/ExitFreedomMode variants from CoreToAgentMessage**
- **Found during:** Task 1 verification (running process_guard tests)
- **Issue:** Commit `036fd79` ("fix: add EnterFreedomMode/ExitFreedomMode protocol variants") added these variants at lines 474-477, but they were already present at lines 332-335. The duplicate caused 5 E0428/E0004 compile errors in rc-common, blocking all racecontrol tests.
- **Fix:** Removed the duplicate trailing variants (lines 473-477 in original file). The original definitions at lines 332-335 were preserved intact.
- **Files modified:** `crates/rc-common/src/protocol.rs`
- **Verification:** `cargo build -p racecontrol-crate` produces zero errors; all 8 process_guard tests pass
- **Committed in:** `ad364f3`

---

**Total deviations:** 1 auto-fixed (blocking compile error in dependency)
**Impact on plan:** Essential fix — tests could not run without it. No scope change.

## Issues Encountered

- Both Task 1 and Task 2 were already committed in a prior session (add07ff and 41bcefa). This session's primary work was test verification and fixing the rc-common blocking compile error.
- Package name is `racecontrol-crate` (not `racecontrol`) — use `-p racecontrol-crate` for cargo test/build commands. Known from Plan 01.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Pods and James can now `curl http://192.168.31.23:8080/api/v1/guard/whitelist/pod-8` to fetch their merged whitelist
- `GET /api/v1/guard/whitelist/james` returns 200 with ollama.exe, code.exe in processes; steam.exe absent
- `GET /api/v1/guard/whitelist/pod-99` returns 404 (out-of-range pod number)
- Phase 103 (rc-agent process scanner) can now fetch whitelist on WS connect and begin scanning

## Self-Check: PASSED

- `crates/racecontrol/src/process_guard.rs`: FOUND
- `crates/racecontrol/src/api/routes.rs` contains guard/whitelist route: FOUND
- commit `add07ff`: FOUND (feat - process_guard.rs)
- commit `41bcefa`: FOUND (feat - route registration)
- commit `ad364f3`: FOUND (fix - duplicate variants)
- All 8 process_guard tests: PASSING
- Zero compile errors

---
*Phase: 102-whitelist-schema-config-fetch-endpoint*
*Completed: 2026-03-21*
