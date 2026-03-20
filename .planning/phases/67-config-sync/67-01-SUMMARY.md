---
phase: 67-config-sync
plan: 01
subsystem: comms-link/james
tags: [config-sync, security, allowlist, file-watcher, tdd]
dependency_graph:
  requires: []
  provides: [config-watcher, config-sanitizer, relay-sync-push]
  affects: [james/index.js, comms-link relay server]
tech_stack:
  added: [toml@3.0.0]
  patterns: [poll+hash EventEmitter, allowlist sanitizer, DI readFileFn for testing, TDD red-green]
key_files:
  created:
    - C:/Users/bono/racingpoint/comms-link/james/config-sanitizer.js
    - C:/Users/bono/racingpoint/comms-link/james/config-watcher.js
    - C:/Users/bono/racingpoint/comms-link/test/config-sanitizer.test.js
    - C:/Users/bono/racingpoint/comms-link/test/config-watcher.test.js
  modified:
    - C:/Users/bono/racingpoint/comms-link/james/index.js
    - C:/Users/bono/racingpoint/comms-link/package.json
decisions:
  - "Allowlist approach for sanitizer: only venue/pods/branding pass through -- never denylist, denylists have gaps"
  - "httpPost used instead of fetch for relay/sync POST -- already imported, consistent with existing patterns"
  - "RACECONTROL_TOML_PATH env var for configurable path -- default C:/RacingPoint/racecontrol.toml"
  - "Error in #poll emits 'error' without updating lastHash -- next cycle retries from last known good hash"
  - "30s pollMs default -- satisfies 60s SLA with 2x margin"
metrics:
  duration_minutes: 3
  tasks_completed: 3
  tasks_total: 3
  files_created: 4
  files_modified: 2
  tests_added: 10
  completed_date: "2026-03-20T13:01:25Z"
requirements_satisfied: [SYNC-01, SYNC-02]
---

# Phase 67 Plan 01: Config Sync -- James-Side Watcher and Sanitizer Summary

**One-liner:** Poll-based racecontrol.toml watcher with SHA-256 change detection, TOML parsing, and allowlist sanitizer that strips all secrets before pushing venue/pods/branding snapshots to /relay/sync.

## What Was Built

Three files implementing the James-side of the config sync pipeline:

1. **`james/config-sanitizer.js`** — Pure function `sanitizeConfig(parsed)` using an explicit allowlist. Only `venue`, `pods`, `branding`, and `_meta` fields are returned. Auth, cloud, bono, database, server, gmail, and ac_server sections are structurally impossible to leak because they are never referenced in the output construction.

2. **`james/config-watcher.js`** — `ConfigWatcher` class extending EventEmitter. Follows the `LogbookWatcher` pattern (same codebase). Polls file every 30s (default), computes SHA-256, and only parses TOML + sanitizes when the hash actually changes. Errors (file not found, partial write, invalid TOML) emit `'error'` without crashing and without updating `lastHash` so the next cycle retries cleanly.

3. **`james/index.js`** (modified) — Instantiates `ConfigWatcher` after relay server starts. Wires `'changed'` events to POST `config_snapshot` payload to `/relay/sync` using the existing `httpPost` helper. Wires `'error'` events to `console.warn` (non-fatal retry).

## Test Coverage

10 unit tests using Node.js built-in `node:test` + mock timers:

| File | Tests | Coverage |
|------|-------|----------|
| config-sanitizer.test.js | 5 | Allowlist keys only, deep secret check, no Windows paths, missing section defaults, _meta fields |
| config-watcher.test.js | 5 | Changed event on hash diff, no emit on identical, error on read fail, error on TOML parse fail, snapshot sanitized (no auth/cloud/bono) |

## Commits

| Hash | Description |
|------|-------------|
| 956efde | feat(67-01): config sanitizer with allowlist -- venue/pods/branding only |
| a3b2cdc | feat(67-01): config watcher -- poll + SHA-256 change detection + sanitized snapshot |
| 406628b | feat(67-01): wire ConfigWatcher into james/index.js for config sync |

## Deviations from Plan

None - plan executed exactly as written.

The plan suggested using `fetch` OR `httpPost` for the relay call. `httpPost` was chosen for consistency with the existing codebase (already imported in index.js, same pattern used for all other relay calls).

## Self-Check: PASSED

All 4 created files confirmed on disk. All 3 task commits verified in git log.

## Security Properties

- **Allowlist boundary:** `sanitizeConfig()` constructs output by explicit field extraction only. No spread operators, no `Object.assign(parsed, ...)`, no passthrough of unknown keys.
- **jwt_secret, terminal_secret, relay_secret, evolution_api_key, gmail.*, database.path, server.host/port, ac_server.*:** Cannot appear in output by construction.
- **Windows paths:** Not in allowlist, cannot appear in output.
- **TOML parse error:** Emits `'error'`, does not push partial/corrupted data.
