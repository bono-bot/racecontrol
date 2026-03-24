---
phase: 178-agent-sentry-consumer
verified: 2026-03-24T13:00:00+05:30
status: passed
score: 14/14 must-haves verified
re_verification: false
---

# Phase 178: Agent-Sentry Consumer Verification Report

**Phase Goal:** rc-agent and rc-sentry receive flag updates, config pushes, and OTA download messages — rc-agent over WebSocket with hot-reload and offline cache, rc-sentry via local config file push from rc-agent (rc-sentry has no WS connection to server). Both write sentinel files before binary swap.
**Verified:** 2026-03-24T13:00:00 IST
**Status:** PASSED
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | rc-agent stores flags in-memory and reads them synchronously without server round-trip | VERIFIED | `feature_flags.rs:32-39` — FeatureFlags struct with `flags: HashMap<String, bool>` and `kill_switches: HashMap<String, bool>` held in `Arc<RwLock<FeatureFlags>>` on AppState |
| 2 | rc-agent persists flags to `C:\RacingPoint\flags-cache.json` on every FlagSync receive | VERIFIED | `feature_flags.rs:108-119` — `apply_sync()` calls `persist_to_disk()` with atomic tmp+rename; `ws_handler.rs:995` calls `flags.apply_sync(&payload)` |
| 3 | rc-agent reads flags-cache.json on startup before WS connects — defaults to all-true if no cache | VERIFIED | `main.rs:607` — `flags_arc = Arc::new(RwLock::new(FeatureFlags::load_from_cache()))` created before AppState; `feature_flags.rs:51-85` — returns `Self::new()` (empty = defaults to true) on any error |
| 4 | FlagSync WS message updates in-memory flags within the same event loop tick | VERIFIED | `ws_handler.rs:992-997` — `CoreToAgentMessage::FlagSync(payload)` acquires write lock and calls `apply_sync` synchronously in the match arm |
| 5 | kill_* flags override all other flag logic | VERIFIED | `feature_flags.rs:93-101` — `flag_enabled()` checks `kill_<name>` in `kill_switches` first and returns `false` immediately if active, before consulting `flags` map |
| 6 | FlagCacheSync sent on WS connect with cached_version so server sends delta | VERIFIED | `main.rs:931-946` — `AgentMessage::FlagCacheSync` sent after Register on every WS connect with `flags.cached_version()` |
| 7 | ConfigPush WS message updates hot-reloadable fields in-memory without restart | VERIFIED | `ws_handler.rs:1009-1046` — `CoreToAgentMessage::ConfigPush` handler; `HOT_RELOAD_FIELDS` = `["billing_rates", "game_limits", "process_guard_whitelist", "debug_verbosity"]`; `process_guard_whitelist` hot-reloaded via `Arc<RwLock<MachineWhitelist>>` |
| 8 | ConfigPush for non-reloadable fields logs warning and is ignored | VERIFIED | `ws_handler.rs:1011,1015-1017` — `NON_RELOAD_FIELDS = ["port", "ws_url", "pod_number", "pod_id"]`; logs warn `"ignoring non-reloadable field … (requires restart)"` and `continue` |
| 9 | ConfigAck sent back to server with pod_id and seq_num after every ConfigPush | VERIFIED | `ws_handler.rs:1039-1045` — `AgentMessage::ConfigAck(ConfigAckPayload { pod_id, sequence, accepted })` pushed to `conn.pending_acks`; `event_loop.rs:1371-1375` — drained via `ws_tx.send` after each message |
| 10 | rc-agent writes sentry-flags.json for rc-sentry to consume on its watchdog cycle | VERIFIED | `feature_flags.rs:126-144` — `write_sentry_flags()` writes atomic tmp+rename to `C:\RacingPoint\sentry-flags.json`; called from `apply_sync()` after every FlagSync |
| 11 | rc-sentry reads sentry-flags.json on its 5s watchdog poll if the file exists | VERIFIED | `rc-sentry/watchdog.rs:188-202` — reads `sentry-flags.json` at start of each watchdog tick into `sentry_flags: Option<Value>` |
| 12 | LaunchGame handler checks game_launch feature flag before proceeding | VERIFIED | `ws_handler.rs:287-289` — `flags.flag_enabled("game_launch")` check at top of LaunchGame handler; returns `HandleResult::Continue` if disabled |
| 13 | billing_guard poll loop checks billing_guard feature flag each tick | VERIFIED | `billing_guard.rs:82` — `ff.flag_enabled("billing_guard")` checked after `interval.tick().await`; `continue` if disabled |
| 14 | FlagSync, ConfigPush, ConfigAck, KillSwitch, FlagCacheSync, OtaDownload, OtaAck TypeScript interfaces exist and are contract-tested | VERIFIED | `packages/shared-types/src/ws-messages.ts` — 7 interfaces; `packages/contract-tests/src/ws-messages.contract.test.ts` — 10 tests under `SYNC-03`; fixture at `packages/contract-tests/src/fixtures/ws-messages.json` |

**Score:** 14/14 truths verified

---

## Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/rc-agent/src/feature_flags.rs` | FeatureFlags struct, flag_enabled(), load_from_cache(), apply_sync(), apply_kill_switch(), persist_to_disk(), write_sentry_flags() | VERIFIED | All 7 methods present; 195 LOC; no `.unwrap()` in production paths |
| `crates/rc-agent/src/app_state.rs` | `flags: Arc<RwLock<FeatureFlags>>` field | VERIFIED | Line 49: `pub(crate) flags: Arc<RwLock<FeatureFlags>>` |
| `crates/rc-agent/src/ws_handler.rs` | FlagSync, KillSwitch, ConfigPush/ConfigAck match arms; LaunchGame flag gate | VERIFIED | All 4 handlers present at lines 992, 1000, 1009, 287 |
| `crates/rc-agent/src/event_loop.rs` | `pending_acks: Vec<AgentMessage>` in ConnectionState; drain loop | VERIFIED | Lines 100 (field), 135 (init), 1371 (drain) |
| `crates/rc-agent/src/main.rs` | `flags_arc` initialized from `load_from_cache()`; FlagCacheSync sent after Register | VERIFIED | Lines 607 (init), 935 (FlagCacheSync send) |
| `crates/rc-agent/src/billing_guard.rs` | `flag_enabled("billing_guard")` check in poll loop | VERIFIED | Line 82; `flags: Arc<RwLock<FeatureFlags>>` parameter at line 65 |
| `crates/rc-sentry/src/main.rs` | `read_sentry_flags()`, `/flags` HTTP endpoint | VERIFIED | Lines 391 (route), 461-471 (implementation) |
| `crates/rc-sentry/src/watchdog.rs` | sentry-flags.json read at tick start; `restart_suppressed` kill switch gating | VERIFIED | Lines 188-202 (read), 241-245 (suppression logic) |
| `packages/shared-types/src/ws-messages.ts` | 7 TypeScript WS payload interfaces | VERIFIED | All 7 interfaces at lines 2, 8, 15, 22, 29, 36, 44 |
| `packages/shared-types/src/index.ts` | Re-exports all 7 interfaces from ws-messages | VERIFIED | Line 6: exports all 7 types |
| `packages/contract-tests/src/ws-messages.contract.test.ts` | Contract tests for all 7 payload types | VERIFIED | 10 tests; SYNC-03 describe block |
| `packages/contract-tests/src/fixtures/ws-messages.json` | Sample payloads with Rust snake_case field names | VERIFIED | File exists with `flag_sync`, `config_push`, `ota_download`, `kill_switch`, `config_ack`, `ota_ack`, `flag_cache_sync` |

---

## Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `ws_handler.rs` | `feature_flags.rs` | `flags.apply_sync(&payload)` in FlagSync arm | WIRED | ws_handler.rs:995 calls apply_sync; apply_sync calls persist_to_disk + write_sentry_flags |
| `ws_handler.rs` | `feature_flags.rs` | `flags.apply_kill_switch(&payload)` in KillSwitch arm | WIRED | ws_handler.rs:1003 |
| `main.rs` | `feature_flags.rs` | `FlagCacheSync` sent after Register on every WS connect | WIRED | main.rs:935-946 (NOTE: plan specified event_loop.rs but code lives in main.rs which IS the WS loop — functionally equivalent) |
| `ws_handler.rs` | server | `ConfigAck` via `conn.pending_acks` drain in event_loop | WIRED | ws_handler.rs:1045; event_loop.rs:1371-1375 |
| `billing_guard.rs` | `feature_flags.rs` | `ff.flag_enabled("billing_guard")` in poll loop | WIRED | billing_guard.rs:82; flags Arc shared via main.rs:626 |
| `rc-sentry/watchdog.rs` | `C:\RacingPoint\sentry-flags.json` | `fs::read_to_string` at watchdog tick start | WIRED | watchdog.rs:191 |
| `rc-sentry/main.rs` | `C:\RacingPoint\sentry-flags.json` | `/flags` HTTP endpoint reads file | WIRED | main.rs:391, 464-465 |
| `ws_handler.rs` (LaunchGame) | `feature_flags.rs` | `flags.flag_enabled("game_launch")` | WIRED | ws_handler.rs:287 |

---

## Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|------------|------------|-------------|--------|----------|
| FF-04 | 178-01 | rc-agent caches flags in-memory (Arc<RwLock>) for synchronous reads — no server round-trip per flag check | SATISFIED | `app_state.rs:49` — `Arc<RwLock<FeatureFlags>>`; `flag_enabled()` is synchronous on held read guard |
| FF-05 | 178-01 | rc-agent persists flags to flags-cache.json on every receive; reads on startup before server connects | SATISFIED | `feature_flags.rs:51-85` (load), `feature_flags.rs:171-194` (persist); `main.rs:607` (startup load) |
| FF-07 | 178-01 | Flag changes propagate to all connected pods within seconds — no deploy or restart required | SATISFIED | FlagSync WS handler at ws_handler.rs:992-997 — in-memory update on receive; no restart needed |
| FF-08 | 178-01 | Kill switch flags (kill_*) evaluated before all other flag logic | SATISFIED | `feature_flags.rs:95-98` — kill switch checked first, returns false immediately |
| CP-03 | 178-02 | rc-agent hot-reloads supported config fields without restart; restart-required fields documented and excluded | SATISFIED | `ws_handler.rs:1010-1017` — HOT_RELOAD_FIELDS / NON_RELOAD_FIELDS constants; non-reloadable fields logged-and-skipped |
| SYNC-03 | 178-03 | New WS message types in rc-common AND shared TypeScript types; contract tests verify both sides match | SATISFIED | `ws-messages.ts` — 7 interfaces; `ws-messages.contract.test.ts` — 10 contract tests with fixtures |
| CF-04 | 178-02 (cross-ref) | rc-sentry Cargo.toml has feature flags for optional modules (watchdog, tier1-fixes, ai-diagnosis) | SATISFIED (Phase 176) | `rc-sentry/Cargo.toml:21-25` — 3 Cargo features. Completed Phase 176. Plan 02 listed it as reference, not new implementation. |

**Orphaned requirements check:** REQUIREMENTS.md tracking table maps FF-04, FF-05, FF-07, FF-08, CP-03, SYNC-03 to Phase 178. CF-04 maps to Phase 176 (already complete). No orphaned requirements — all Phase 178 requirements claimed in plans and verified in code.

---

## Anti-Patterns Found

| File | Pattern | Severity | Impact |
|------|---------|----------|--------|
| None | — | — | — |

Scanned: `feature_flags.rs`, `billing_guard.rs`, `ws_handler.rs` (new sections), `event_loop.rs` (new sections), `rc-sentry/main.rs` (new sections), `rc-sentry/watchdog.rs` (new sections), `ws-messages.ts`, `ws-messages.contract.test.ts`

No `.unwrap()` in production Rust paths. No `TODO`/`FIXME`/`PLACEHOLDER` comments. No stub implementations. No empty handlers. Compilation: `cargo check -p rc-agent-crate -p rc-sentry` finishes with 0 errors (60 pre-existing warnings in unrelated code).

---

## Human Verification Required

None. All truths are verifiable programmatically (file existence, content, wiring). No UI changes, no real-time behavior, no external service integration in this phase.

---

## Structural Note: FlagCacheSync in main.rs vs event_loop.rs

Plan 01 specified `FlagCacheSync` should be sent from `event_loop.rs`, and added it to the key_links frontmatter pointing at `event_loop.rs`. The implementation placed it in `main.rs` (around line 931-946), which is where the WS connection loop, Register send, and all per-connection setup actually lives in rc-agent. The `event_loop.rs` file handles the per-message dispatch loop (ConnectionState, handle_ws_message calls, pending_acks drain). The FlagCacheSync is sent in the correct location functionally — immediately after Register on every WS connect — just in a different file than the plan anticipated. This is a non-issue.

---

## Gaps Summary

No gaps. All 14 observable truths verified. All artifacts exist, are substantive, and are wired. All 7 required requirements satisfied. No anti-patterns in Phase 178 code.

---

_Verified: 2026-03-24T13:00:00 IST_
_Verifier: Claude (gsd-verifier)_
