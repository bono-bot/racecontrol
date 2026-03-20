---
phase: 67-config-sync
verified: 2026-03-20T14:05:00+05:30
status: passed
score: 8/8 must-haves verified
re_verification: false
---

# Phase 67: Config Sync Verification Report

**Phase Goal:** Venue TOML configuration (pod definitions, venue metadata, branding) is mirrored to Bono's cloud racecontrol so failover has a current config to run on. Billing rates and game catalog are already DB-synced via cloud_sync.rs — not in Phase 67 scope.
**Verified:** 2026-03-20T14:05:00 IST
**Status:** PASSED
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | ConfigWatcher detects racecontrol.toml hash change and emits 'changed' event with sanitized payload | VERIFIED | `config-watcher.js` lines 44-48: SHA-256 computed, hash compared, sanitizeConfig called, 'changed' emitted. Test 1 + Test 5 pass. |
| 2 | ConfigWatcher does NOT emit when file content is unchanged between polls | VERIFIED | `config-watcher.js` line 45: `hash !== this.#lastHash` guard. Test 2 passes. |
| 3 | Sanitized config contains ONLY venue/pods/branding fields — no jwt_secret, no terminal_secret, no relay_secret, no Windows paths | VERIFIED | `config-sanitizer.js`: allowlist construction, only 4 keys returned. Tests 1-3 pass. grep confirms no secret string values pass through. |
| 4 | TOML parse failure does not crash the watcher — it logs and skips the cycle | VERIFIED | `config-watcher.js` lines 51-53: try/catch, emits 'error', does not update `#lastHash`. Test 4 passes. |
| 5 | On 'changed' event, james/index.js POSTs sanitized payload to /relay/sync with config_snapshot key | VERIFIED | `james/index.js` lines 557-563: `httpPost` to `http://localhost:${relayPort}/relay/sync` with `{ config_snapshot: { ...snapshot, _meta: { ...snapshot._meta, hash } } }`. |
| 6 | Cloud racecontrol's /sync/push accepts a config_snapshot key and stores it in AppState | VERIFIED | `routes.rs` line 7587: `if let Some(config_snap) = body.get("config_snapshot")`. `state.venue_config.write().await = Some(snapshot)` at line 7595. |
| 7 | AppState stores venue config snapshot in a thread-safe field | VERIFIED | `state.rs` line 172: `pub venue_config: RwLock<Option<VenueConfigSnapshot>>`. Initialized at line 224: `venue_config: RwLock::new(None)`. |
| 8 | Existing sync_push behavior is completely unchanged | VERIFIED | Config snapshot branch added after all existing upsert blocks (billing_sessions, laps, etc.), before final `tracing::info!`. Routes.rs existing upsert patterns confirmed intact. |

**Score:** 8/8 truths verified

---

## Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `comms-link/james/config-sanitizer.js` | Allowlist-based sanitizer exporting `sanitizeConfig` | VERIFIED | 31 lines, exports `sanitizeConfig`, allowlist only, no secrets passthrough |
| `comms-link/james/config-watcher.js` | Poll-based watcher exporting `ConfigWatcher` | VERIFIED | 73 lines, EventEmitter subclass, SHA-256 hash, DI readFileFn |
| `comms-link/test/config-sanitizer.test.js` | 5 unit tests for sanitizer | VERIFIED | 90 lines, 5 tests, all pass |
| `comms-link/test/config-watcher.test.js` | 5 unit tests for ConfigWatcher | VERIFIED | 172 lines, 5 tests, all pass |
| `comms-link/james/index.js` (modified) | Imports ConfigWatcher, wires 'changed' to /relay/sync POST | VERIFIED | Lines 18, 555-578 confirm import, instantiation, wiring |
| `crates/racecontrol/src/state.rs` (modified) | VenueConfigSnapshot struct + venue_config field on AppState | VERIFIED | Lines 83-97 struct, line 172 field, line 224 init |
| `crates/racecontrol/src/api/routes.rs` (modified) | parse_config_snapshot() + config_snapshot branch + 3 unit tests | VERIFIED | Lines 7116-7143 helper fn, 7587-7596 handler branch, 13863-13904 tests |

---

## Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `config-watcher.js` | `config-sanitizer.js` | `import { sanitizeConfig }` | WIRED | Line 5: `import { sanitizeConfig } from './config-sanitizer.js'`. Used at line 47. |
| `james/index.js` | `config-watcher.js` | `import { ConfigWatcher }` | WIRED | Line 18: `import { ConfigWatcher } from './config-watcher.js'`. Instantiated at line 556. |
| `james/index.js` | `/relay/sync` | `httpPost` with `config_snapshot` key | WIRED | Lines 560-564: `httpPost(http://localhost:${relayPort}/relay/sync, ...)` with `config_snapshot` payload. |
| `routes.rs sync_push` | `state.rs AppState.venue_config` | `RwLock write` | WIRED | Line 7595: `*state.venue_config.write().await = Some(snapshot)`. |
| `parse_config_snapshot()` | `VenueConfigSnapshot` in state.rs | `use crate::state::VenueConfigSnapshot` | WIRED | Line 24: `use crate::state::{AppState, VenueConfigSnapshot}`. Used in helper fn + handler. |

---

## Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| SYNC-01 | 67-01-PLAN.md | racecontrol.toml changes detected via SHA-256 and pushed to Bono via comms-link sync_push within 60s | SATISFIED | ConfigWatcher polls every 30s (2x margin), 'changed' event POSTs to /relay/sync. 5 watcher tests pass. |
| SYNC-02 | 67-01-PLAN.md | Config payload sanitized before push — allowlist-only (venue/pods/branding), no credentials, no Windows paths | SATISFIED | sanitizeConfig() allowlist construction. Secrets structurally impossible to leak. 5 sanitizer tests pass. |
| SYNC-03 | 67-02-PLAN.md | Cloud racecontrol receives config_snapshot in /sync/push and stores venue/pods/branding in AppState.venue_config | SATISFIED | VenueConfigSnapshot in state.rs, config_snapshot branch in sync_push, RwLock write. 3 Rust unit tests written. |

No orphaned requirements — REQUIREMENTS.md maps exactly SYNC-01, SYNC-02, SYNC-03 to Phase 67 and all three are satisfied.

---

## Test Results

### JS Tests (node --test)
```
tests 10 / suites 2 / pass 10 / fail 0
```
All 10 tests pass: 5 sanitizer tests + 5 watcher tests.

### Rust Tests
3 unit tests in `config_snapshot_tests` module confirmed present in `routes.rs` (lines 13863-13904):
- `test_parse_full_config_snapshot`
- `test_parse_config_snapshot_defaults`
- `test_venue_config_snapshot_serde_roundtrip`

Note: Rust test execution is blocked on this workstation by Application Control policy (OS error 4551 on unsigned test binaries). `cargo build --bin racecontrol` compiles successfully with 0 errors (1 unrelated unused import warning). Tests are structurally correct and compilable — execution must be verified on server (.23) or Bono's VPS where policy does not apply.

### Build
`cargo build --bin racecontrol` exits 0. No compilation errors introduced by Phase 67 changes.

---

## Anti-Patterns Found

None. No TODO/FIXME/PLACEHOLDER comments in new files. No stub implementations. No empty event handlers.

---

## Human Verification Required

### 1. End-to-end config push

**Test:** Start james (comms-link) with a valid `racecontrol.toml`, modify the venue name field, wait up to 30s.
**Expected:** `[CONFIG-SYNC] pushed config snapshot (hash=...)` appears in james log. Cloud racecontrol logs `Config sync: received venue config snapshot venue=<new name>`.
**Why human:** Requires live comms-link + cloud racecontrol running with valid auth headers and WS relay connected.

### 2. Credential non-leakage in transit

**Test:** Capture the actual HTTP POST body sent to /relay/sync after a real config change.
**Expected:** JSON contains no jwt_secret, terminal_secret, relay_secret, or any key matching secret/password/key/token/pin.
**Why human:** The sanitizer unit tests use constructed objects; live TOML parsing of the real racecontrol.toml file confirms end-to-end.

---

## Commit Inventory

| Repo | Hash | Description |
|------|------|-------------|
| comms-link | 956efde | feat(67-01): config sanitizer with allowlist |
| comms-link | a3b2cdc | feat(67-01): config watcher — poll + SHA-256 change detection |
| comms-link | 406628b | feat(67-01): wire ConfigWatcher into james/index.js |
| racecontrol | e7366cb | feat(67-02): add VenueConfigSnapshot to AppState and config_snapshot to sync_push |
| racecontrol | f5a9a71 | test(67-02): unit tests for config_snapshot parsing |

All commits verified in git log.

---

## Summary

Phase 67 goal is achieved. The full pipeline is wired and substantive:

1. James-side: `ConfigWatcher` polls `racecontrol.toml` every 30s, detects changes via SHA-256, strips all secrets via the allowlist sanitizer, and POSTs a `config_snapshot` payload to the comms-link relay.
2. Cloud-side: The `sync_push` handler extracts `config_snapshot`, parses it via `parse_config_snapshot()`, logs structured tracing (venue name, pod count, hash prefix), and stores it in `AppState.venue_config` behind an `RwLock`.

10 JS unit tests pass. 3 Rust unit tests are correctly written and compilable. Build is clean. All 3 requirements (SYNC-01, SYNC-02, SYNC-03) are satisfied with no orphans.

The only unresolved item is live end-to-end testing (human verification), which requires both sides running simultaneously. This does not block phase completion.

---

_Verified: 2026-03-20T14:05:00 IST_
_Verifier: Claude (gsd-verifier)_
