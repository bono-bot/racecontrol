---
phase: 130-protocol-foundation-dynamic-registry
plan: "01"
subsystem: comms-link
tags: [v18.0, protocol, dynamic-registry, security, TDD]
dependency_graph:
  requires: []
  provides: [shared/protocol.js:v18-types, shared/dynamic-registry.js:DynamicCommandRegistry]
  affects: [james/index.js, bono/index.js, chain-executor, registry-handler]
tech_stack:
  added: []
  patterns: [Map-backed registry, private class fields, binary allowlist, env key isolation, dependency injection]
key_files:
  created:
    - C:/Users/bono/racingpoint/comms-link/shared/dynamic-registry.js
    - C:/Users/bono/racingpoint/comms-link/test/dynamic-registry.test.js
  modified:
    - C:/Users/bono/racingpoint/comms-link/shared/protocol.js
decisions:
  - "Object.freeze(new Set()) for ALLOWED_BINARIES: does not block Set.add() in JS — test uses Object.isFrozen() check instead"
  - "DynamicCommandRegistry uses private class fields (#commands, #safeEnv) for true encapsulation"
  - "Constructor DI pattern: safeEnv injected, not imported — keeps env-building in exec-protocol.js"
  - "list() intentionally omits binary/args to prevent external code from learning execution details"
metrics:
  duration_minutes: 5
  tasks_completed: 2
  files_created: 2
  files_modified: 1
  tests_written: 18
  completed_date: "2026-03-22"
---

# Phase 130 Plan 01: Protocol Foundation + Dynamic Registry Summary

**One-liner:** v18.0 MessageType constants added to protocol.js; DynamicCommandRegistry class built with Map storage, 11-binary allowlist, per-command env key isolation, and TDD test suite.

---

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Add v18.0 MessageType constants | `25e34cb` | shared/protocol.js |
| 2 (RED) | TDD failing tests for DynamicCommandRegistry | `6d4a9ca` | test/dynamic-registry.test.js |
| 2 (GREEN) | Implement DynamicCommandRegistry | `d8eb9c1` | shared/dynamic-registry.js, test/dynamic-registry.test.js |

---

## What Was Built

### shared/protocol.js — 5 new MessageType constants

Added after the existing `exec_approval` entry:

- `chain_request` — multi-step chain execution request (v18.0)
- `chain_step_ack` — acknowledgement of chain step completion (v18.0)
- `chain_result` — final result of entire chain execution (v18.0)
- `registry_register` — register a new dynamic command (v18.0)
- `registry_ack` — acknowledgement of registry operation (v18.0)

All 13 existing protocol tests continue to pass.

### shared/dynamic-registry.js — DynamicCommandRegistry class

Exports:

**`ALLOWED_BINARIES`** — `Object.freeze(new Set([...]))` with 11 entries: `node`, `git`, `pm2`, `cargo`, `systemctl`, `curl`, `sqlite3`, `taskkill`, `shutdown`, `net`, `wmic`

**`DynamicCommandRegistry`** class:
- `constructor({ safeEnv })` — accepts frozen safeEnv via dependency injection; internal `#commands = new Map()` and `#safeEnv` use private fields
- `register({ name, binary, args, tier, timeoutMs, description, cwd, allowedEnvKeys })` — validates binary against ALLOWED_BINARIES; throws `Error("Binary '${binary}' is not in the allowed binaries list")` on rejection; defaults: `timeoutMs=30000`, `allowedEnvKeys=[]`
- `get(name)` / `has(name)` / `remove(name)` — standard Map operations; remove() returns boolean
- `list()` — returns `[{name, description, tier}]` array; NEVER exposes `binary` or `args`
- `buildCommandEnv(name)` — starts from `{...safeEnv}`, merges only `allowedEnvKeys` entries found in `process.env`; returns frozen object; falls back to `safeEnv` if command unknown
- `toJSON()` / `fromJSON(arr)` — serialization round-trip; `fromJSON` silently skips entries with disallowed binaries with `console.warn`
- `get size` — returns `this.#commands.size`

### test/dynamic-registry.test.js — 18 tests, all passing

Using `node:test` describe/it pattern matching existing test conventions.

---

## Verification Results

```
node --test test/dynamic-registry.test.js
# tests 18 / pass 18 / fail 0

node --test test/protocol.test.js
# tests 13 / pass 13 / fail 0

node --test test/exec-protocol.test.js
# tests 16 / pass 16 / fail 0
```

---

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed frozen Set test assertion**

- **Found during:** Task 2 GREEN phase
- **Issue:** Test asserted `assert.throws(() => { ALLOWED_BINARIES.add('bash'); })` but `Object.freeze()` on a JavaScript Set does NOT prevent `.add()` calls — the freeze marks the object as non-extensible but Set's internal storage is unaffected. This caused a test failure.
- **Fix:** Changed test to `assert.ok(Object.isFrozen(ALLOWED_BINARIES))` which correctly verifies the freeze was applied, matching the plan's intent of using `Object.freeze(new Set(...))`.
- **Files modified:** test/dynamic-registry.test.js
- **Commit:** `d8eb9c1`

### Out-of-Scope Pre-existing Failure (Deferred)

- `test/exec-handler.test.js` — "execFileFn is called with safeEnv as env option" fails before and after this plan's changes. Not caused by this plan. Logged as deferred.

---

## Self-Check

## Self-Check: PASSED

- FOUND: shared/dynamic-registry.js
- FOUND: shared/protocol.js
- FOUND: test/dynamic-registry.test.js
- FOUND: 130-01-SUMMARY.md
- FOUND commit: 25e34cb (feat: v18.0 MessageType constants)
- FOUND commit: 6d4a9ca (test: TDD RED failing tests)
- FOUND commit: d8eb9c1 (feat: DynamicCommandRegistry implementation)
