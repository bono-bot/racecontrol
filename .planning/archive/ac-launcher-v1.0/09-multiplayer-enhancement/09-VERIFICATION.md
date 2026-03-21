---
phase: 09-multiplayer-enhancement
verified: 2026-03-14T05:30:00Z
status: gaps_found
score: 4/5 must-haves verified
gaps:
  - truth: "AI difficulty level from host's chosen tier flows through to AssettoServer extra_cfg.yml"
    status: partial
    reason: "ai_level is computed in start_ac_lan_for_group but never passed to start_ac_server; generate_extra_cfg_yml always receives None, so AiAggression is never written"
    artifacts:
      - path: "crates/rc-core/src/ac_server.rs"
        issue: "start_ac_server calls generate_extra_cfg_yml(&config, None) -- no ai_level parameter accepted"
      - path: "crates/rc-core/src/multiplayer.rs"
        issue: "ai_level computed at line 1016-1023 but only used in log message at line 1058, never passed through"
    missing:
      - "start_ac_server needs an ai_level: Option<u32> parameter"
      - "start_ac_lan_for_group must pass computed ai_level to start_ac_server"
      - "start_ac_server must pass ai_level to generate_extra_cfg_yml instead of None"
---

# Phase 9: Multiplayer Enhancement Verification Report

**Phase Goal:** Multi-pod multiplayer races have AI grid fillers, synchronized billing, and a lobby experience
**Verified:** 2026-03-14T05:30:00Z
**Status:** gaps_found
**Re-verification:** No -- initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Multiple pods join same AC server race with JSON launch_args | VERIFIED | ac_server.rs:562-574 sends JSON with game_mode "multi", server_ip, server_http_port. Agent parses this correctly. |
| 2 | AI opponents fill remaining grid spots with AI=fixed in entry_list.ini | VERIFIED | generate_entry_list_ini (ac_server.rs:348-351) writes AI=fixed when ai_mode is Some. start_ac_lan_for_group (multiplayer.rs:1042-1055) adds AI fillers with ai_mode: Some("fixed"). extra_cfg.yml with EnableAi: true written at ac_server.rs:447-452. |
| 3 | Billing synchronized across pods (starts when all LIVE, individual disconnect stops) | VERIFIED | MultiplayerBillingWait coordinator in billing.rs:256-262. handle_game_status_update checks group membership (billing.rs:361+). 60s timeout eviction at billing.rs:544+. All 4 auth call sites pass group_session_id (auth/mod.rs:426-445, 554-575, 671-692, 1250-1271). 11 dedicated tests all passing. |
| 4 | PWA lobby shows track/car/AI count and player check-in status | VERIFIED | GroupSessionInfo TypeScript type has track?, car?, ai_count?, difficulty_tier? (api.ts:341-344). page.tsx:120-141 renders 3-column info cards. formatDisplayName at page.tsx:248-253 converts AC IDs. Status message with remaining player count at page.tsx:233-242. Pod numbers shown at page.tsx:208-210. |
| 5 | AI difficulty level from host's chosen tier flows through to AssettoServer extra_cfg.yml | PARTIAL | ai_level is computed from difficulty_tier in start_ac_lan_for_group (multiplayer.rs:1016-1023) but start_ac_server (ac_server.rs:401-404) does not accept ai_level param. generate_extra_cfg_yml is called with None (ac_server.rs:447), so AiAggression is never written. EnableAi: true IS written, so AI works -- just at default aggression. |

**Score:** 4/5 truths verified (1 partial)

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/rc-common/src/ai_names.rs` | AI_DRIVER_NAMES + pick_ai_names() | VERIFIED | 60 names, pick_ai_names uses rand::shuffle, 3 tests, pub exported via lib.rs:5 |
| `crates/rc-common/src/types.rs` | AcEntrySlot.ai_mode, GroupSessionInfo enrichment | VERIFIED | ai_mode: Option<String> with serde(default, skip_serializing_if) at line 451. GroupSessionInfo has track/car/ai_count/difficulty_tier (lines 733-743). 5 dedicated serde tests. |
| `crates/rc-core/src/ac_server.rs` | generate_entry_list_ini with AI=fixed, extra_cfg.yml, JSON launch_args | VERIFIED | AI=fixed at lines 348-351. generate_extra_cfg_yml at lines 380-397. JSON launch_args at lines 562-574. 6 tests covering AI entry list and extra_cfg. |
| `crates/rc-core/src/multiplayer.rs` | AI fillers in start_ac_lan_for_group, enriched build_group_session_info | VERIFIED | AI fillers at lines 1042-1055, pit count query at 989-1000, DB enrichment UPDATE at 1106-1113, build_group_session_info enrichment at 1275-1311. |
| `crates/rc-core/src/billing.rs` | MultiplayerBillingWait, group-aware billing | VERIFIED | MultiplayerBillingWait struct at line 256, multiplayer_waiting on BillingManager at line 272, group-aware handle_game_status_update at line 361+, multiplayer_billing_timeout at line 544, 11 multiplayer billing tests. |
| `crates/rc-core/src/auth/mod.rs` | group_session_id pass-through | VERIFIED | All 4 defer_billing_start call sites query group_session_members and pass group_session_id (lines 426-445, 554-575, 671-692, 1250-1271). |
| `crates/rc-core/src/db/mod.rs` | ALTER TABLE for track/car/ai_count | VERIFIED | Idempotent migrations at lines 1751, 1754, 1757. |
| `pwa/src/lib/api.ts` | GroupSessionInfo with track/car/ai_count/difficulty_tier | VERIFIED | Optional fields at lines 341-344. Used in group page. |
| `pwa/src/app/book/group/page.tsx` | Info cards, formatDisplayName, status count | VERIFIED | 3-column grid at lines 121-141, formatDisplayName at 248-253, dynamic status at 233-242, pod numbers at 208-210. |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| multiplayer.rs | rc-common::ai_names | pick_ai_names() call | WIRED | Line 1043: `rc_common::ai_names::pick_ai_names(ai_count)` |
| ac_server.rs | rc-agent ac_launcher | JSON launch_args with game_mode multi | WIRED | Lines 562-574 send JSON; rc-agent parses game_mode=="multi" |
| multiplayer.rs | api.ts (via API) | GroupSessionInfo JSON with track/car/ai_count | WIRED | build_group_session_info populates fields (lines 1297-1311), TypeScript type matches (api.ts:331-345), page.tsx renders (lines 120-141) |
| billing.rs | ws/mod.rs | handle_game_status_update called from WS handler | WIRED | ws/mod.rs:386 calls billing::handle_game_status_update |
| auth/mod.rs | billing.rs | defer_billing_start with group_session_id | WIRED | All 4 call sites query group_session_members and pass result |
| multiplayer.rs -> ac_server.rs | ai_level NOT passed | generate_extra_cfg_yml gets None | NOT_WIRED | start_ac_server does not accept ai_level param; computed value unused |
| rc-agent ac_launcher | rc-common ai_names | import for AI_DRIVER_NAMES | WIRED | ac_launcher.rs:112 imports both AI_DRIVER_NAMES and pick_ai_names |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| MULT-01 | 09-01 | Multiple pods join same AC server race | SATISFIED | JSON launch_args with game_mode "multi" sent to all pods (ac_server.rs:560-581) |
| MULT-02 | 09-01 | AI fills remaining grid spots | SATISFIED | AI fillers with AI=fixed added in multiplayer.rs:1042-1055, EnableAi: true in extra_cfg.yml. AiAggression not set (minor gap). |
| MULT-03 | 09-02 | Billing synchronized across pods | SATISFIED | MultiplayerBillingWait coordinator, 60s timeout, individual disconnect. 11 tests covering all scenarios. |
| MULT-04 | 09-03 | PWA lobby shows session info | SATISFIED | Track/car/AI count info cards, dynamic status count, pod numbers per player. |
| MULT-05 | 09-01 | Uses existing ac_server.rs infrastructure | SATISFIED | start_ac_lan_for_group calls crate::ac_server::start_ac_server (multiplayer.rs:1103) |
| MULT-06 | 09-01 | AI names from shared 60-name pool | SATISFIED | 60 names in rc-common::ai_names, used by both rc-agent and rc-core |

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| None | - | - | - | No anti-patterns found in Phase 9 files |

No TODOs, FIXMEs, placeholders, or stub implementations found in any Phase 9 modified files.

### Compiler Warnings (pre-existing, not Phase 9)

- `pwa/src/components/TelemetryChart.tsx` -- missing recharts module (commit 179aa02, pre-Phase 9)
- Several unused variable warnings in routes.rs, multiplayer.rs -- pre-existing

### Test Results

- **rc-common:** 93/93 passed (includes 3 AI names tests, 5 AcEntrySlot/GroupSessionInfo serde tests)
- **rc-core (lib):** 197/197 passed (includes 6 AC server AI tests, 11 multiplayer billing sync tests)
- **TypeScript:** Pre-existing recharts error in TelemetryChart.tsx; Phase 9 files (api.ts, page.tsx) have no type errors

### Commits Verified

| Commit | Plan | Description | Verified |
|--------|------|-------------|----------|
| f512274 | 09-01 Task 1 | AI names to rc-common, ai_mode on AcEntrySlot | Present in git log |
| ae7ef38 | 09-01 Task 2 | AI fillers, LaunchGame JSON fix, extra_cfg.yml, DB enrichment | Present in git log |
| d281b1d | 09-02 Task 1 | MultiplayerBillingWait coordinator | Present in git log |
| fb06b50 | 09-02 Task 2 | 60s timeout eviction tests | Present in git log |
| f72dd72 | 09-03 Task 1 | TypeScript type updates | Present in git log |
| f6c8d67 | 09-03 Task 2 | Lobby UI enrichment | Present in git log |

### Human Verification Required

### 1. Multiplayer Race End-to-End

**Test:** Start a multiplayer group session from kiosk with 2 pods, wait for both to join and race against AI opponents
**Expected:** Both pods launch AC via Content Manager, connect to LAN server, AI opponents visible on grid, billing starts simultaneously when both reach LIVE status
**Why human:** Requires physical pods, AC game, and AssettoServer running on the server

### 2. PWA Lobby Visual Verification

**Test:** Open PWA /book/group page on phone while a group session is active with track/car data populated
**Expected:** Three info cards (Track, Car, AI Opponents) visible with formatted names (e.g., "Ferrari 488 Gt3" not "ks_ferrari_488_gt3"), dynamic player count in status message
**Why human:** Visual layout, text formatting, responsive design on mobile

### 3. Billing Disconnect Behavior

**Test:** During a multiplayer race, disconnect one pod (kill AC process) while others continue
**Expected:** Disconnected pod billing stops, other pods continue billing normally
**Why human:** Requires real network disconnect or process kill during active session

### Gaps Summary

One partial gap found:

**AI Difficulty Level Not Wired to AssettoServer:** The `ai_level` value is correctly computed from the host's difficulty tier (Rookie=75, Amateur=82, SemiPro=87, Pro=93, Alien=98) in `start_ac_lan_for_group`, but `start_ac_server` does not accept an `ai_level` parameter. It always calls `generate_extra_cfg_yml(&config, None)`, meaning the `AiAggression` field is never written to extra_cfg.yml. The AI opponents will work (EnableAi: true is written) but will use AssettoServer's default aggression instead of the host's chosen difficulty.

**Impact:** Low-medium. AI opponents function correctly but do not respect the difficulty tier selection. All other multiplayer features (joining, billing sync, lobby UI) work as designed.

**Fix:** Add `ai_level: Option<u32>` parameter to `start_ac_server`, pass it through from `start_ac_lan_for_group`, and use it in the `generate_extra_cfg_yml` call instead of `None`.

---

_Verified: 2026-03-14T05:30:00Z_
_Verifier: Claude (gsd-verifier)_
