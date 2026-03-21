---
phase: 131-shell-relay
plan: "01"
subsystem: comms-link
tags: [v18.0, shell-relay, security, exec-handler, approval-queue, tdd]

# Dependency graph
requires:
  - phase: 130-02
    provides: DynamicCommandRegistry with ALLOWED_BINARIES, ExecHandler wiring on both sides
provides:
  - ShellRelayHandler class: binary allowlist, hardcoded APPROVE tier, approval queue with timeout default-deny
  - ShellRelayHandler wired in james/index.js and bono/index.js on both sides
  - POST /relay/shell HTTP endpoint on James for sending shell requests to Bono
  - exec_approval handler added to bono/index.js (was previously missing)
affects: [james/index.js, bono/index.js, shared/shell-relay-handler.js]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "ShellRelayHandler: completely separate class from ExecHandler -- no shared tier routing code path"
    - "SHELL_RELAY_TIER const = 'approve' -- tier hardcoded at constant level, never from payload"
    - "Binary allowlist check fires before ANY notification -- disallowed binary rejected silently"
    - "Dedup via completedExecs Set + pendingApprovals Map double-guard (blocks both queued and completed)"

key-files:
  created:
    - C:/Users/bono/racingpoint/comms-link/shared/shell-relay-handler.js
    - C:/Users/bono/racingpoint/comms-link/test/shell-relay-handler.test.js
  modified:
    - C:/Users/bono/racingpoint/comms-link/james/index.js
    - C:/Users/bono/racingpoint/comms-link/bono/index.js

key-decisions:
  - "ShellRelayHandler completely separate from ExecHandler -- no switch statement for tier routing, no tier parameter from payload ever used"
  - "SHELL_RELAY_TIER constant = 'approve' -- strings 'auto' and 'notify' do not appear as tier values anywhere in the file"
  - "Binary allowlist rejection happens BEFORE notifyFn is called -- security property: no notification leaks for rejected binaries"
  - "bono/index.js exec_approval handler added as new block (it did not exist) -- placed before exec_request handler following pre-existing pattern"
  - "dedup check covers both pendingApprovals (in-flight) and completedExecs (finished) -- prevents requeuing an already-queued execId"

requirements-completed: [SHRL-01, SHRL-02, SHRL-03, SHRL-04, SHRL-05]

# Metrics
duration_minutes: 4
tasks_completed: 2
files_created: 2
files_modified: 2
tests_written: 14
completed_date: "2026-03-22"
---

# Phase 131 Plan 01: Shell Relay Handler Summary

**ShellRelayHandler class with binary allowlist, hardcoded APPROVE tier, and approval queue with default-deny timeout -- wired into both James and Bono WS routing and HTTP endpoint.**

---

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-21T22:01:59Z
- **Completed:** 2026-03-21T22:05:50Z
- **Tasks:** 2
- **Files:** 4 (2 created + 2 modified)

## Accomplishments

- `shared/shell-relay-handler.js`: ShellRelayHandler class (EventEmitter, DI constructor with execFileFn/sendResultFn/notifyFn/nowFn/approvalTimeoutMs/safeEnv)
- Tier hardcoded as `SHELL_RELAY_TIER = 'approve'` constant -- payload tier field completely ignored
- Binary allowlist validation against `ALLOWED_BINARIES` (from dynamic-registry.js) fires BEFORE any notification
- `execFile` always called with `shell: false` and safeEnv -- no payload env passthrough
- Approval timeout default-deny: pending request auto-rejects after `approvalTimeoutMs` (default 10 min)
- Dedup guard: completedExecs Set + pendingApprovals Map double-check prevents replay attacks
- Result shape: `{ command: '__shell_relay', binary, args, exitCode, stdout, stderr, durationMs, truncated, tier: 'approve' }`
- Notification text includes full "binary arg1 arg2 ..." command string
- `test/shell-relay-handler.test.js`: 14 tests all passing (Tests 1-13 + 12b for cwd undefined case)
- `james/index.js`: ShellRelayHandler imported, instantiated, wired in exec_request routing, exec_approval updated to try shell relay first, `POST /relay/shell` HTTP endpoint added, `shellRelay.shutdown()` in shutdown function
- `bono/index.js`: ShellRelayHandler imported, instantiated as `bonoShellRelay`, exec_request routing updated, exec_approval handler added (was missing), bonoShellRelay returned from wireBono()

## Task Commits

1. **Task 1: TDD -- ShellRelayHandler class** - `8abcf0d` (feat)
2. **Task 2: Wire ShellRelayHandler into James and Bono** - `513f7c6` (feat)

## Files Created/Modified

- `shared/shell-relay-handler.js` - ShellRelayHandler class: approval queue, binary allowlist, hardcoded APPROVE tier, shell:false + safeEnv execution
- `test/shell-relay-handler.test.js` - 14 TDD tests: queuing, rejection, tier enforcement, notification text, execution, approval, rejection, timeout, dedup, pendingApprovals shape, shutdown, cwd, truncation
- `james/index.js` - ShellRelayHandler import + instantiation, exec_request __shell_relay routing, exec_approval shell-relay-first logic, POST /relay/shell endpoint, shellRelay.shutdown()
- `bono/index.js` - ShellRelayHandler import + instantiation (bonoShellRelay), exec_request __shell_relay routing, exec_approval handler (new), bonoShellRelay in return value

## Decisions Made

- ShellRelayHandler is completely separate from ExecHandler: no switch statement for tier routing, no code path that reads the tier from msg.payload. The tier constant `SHELL_RELAY_TIER = 'approve'` is the only tier assignment in the file.
- Binary allowlist check happens before notifyFn is called -- disallowed binary rejects immediately and silently (no WhatsApp notification).
- bono/index.js did not have an exec_approval handler before this plan. Added as a new block placed before the exec_request handler, following the same `if (msg.type === ...)` pattern already used in wireBono().
- Dedup guard covers both in-flight (pendingApprovals Map) and completed (completedExecs Set) execIds -- a second handleShellRequest call with the same execId is a no-op in both cases.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None -- no external service configuration required.

## Next Phase Readiness

- ShellRelayHandler provides a clean security boundary for arbitrary binary+args execution
- Both sides (James and Bono) handle __shell_relay routing in exec_request
- HTTP POST /relay/shell on James (:8766) allows programmatic shell relay dispatch to Bono
- exec_approval on Bono now handles both shell relay and exec handler approvals
- Ready for chain execution integration (v18.0)

---
*Phase: 131-shell-relay*
*Completed: 2026-03-22*
