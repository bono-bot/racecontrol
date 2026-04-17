---
gsd_state_version: 1.0
milestone: v40.0
milestone_name: Game Launch Reliability
status: executing
last_updated: "2026-04-17T23:50:45.879Z"
last_activity: 2026-04-17
progress:
  total_phases: 4
  completed_phases: 4
  total_plans: 4
  completed_plans: 4
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-04-14)

**Core value:** Customers can seamlessly book a sim racing session — single or multiplayer — and start racing with minimal friction, while all lap times, telemetry, and payments are tracked automatically.
**Current focus:** Phase 413 — service-key-provisioning-deploy-server-sh-hardening-option-z-respawn-race-fixes

## Current Phase

**Phase:** 396
**Status:** Executing Phase 413
**Last activity:** 2026-04-17

## Progress

| Phase | Name | Status | Progress |
|-------|------|--------|----------|
| 383 | Deploy & Verify Pipeline | ◐ VPS done, venue rebuilt (58fee487) | 75% |
| 384 | Lap Recording Wiring | ◐ Fix deployed, awaiting customer verification | 80% |
| 385 | Architecture Completion | ◐ ARCH-01+02 done, ARCH-03 6/~15 files split | 70% |
| 386 | Autonomous Pricing Engine | ○ Blocked by 384 + James P356 | 0% |
| 387 | Customer Opt-In/Opt-Out | ○ Blocked by 384 | 0% |
| 388 | Autonomous Marketing Triggers | ○ Blocked by 387 | 0% |
| 389 | Game Launch Completion | ○ Blocked by 383+384 | 0% |
| 390 | Spectator Displays + Cloud | ○ Blocked by 384 | 0% |
| 391 | Digital Staff Operations | ○ Blocked by 384 | 0% |
| 392 | Unified Readiness Review | ○ Blocked by all | 0% |

## Architecture Split Progress (Phase 385)

| Task | Target | Before | After (non-test) | Status |
|------|--------|--------|-------------------|--------|
| ARCH-01 | billing.rs | 9,142 | 386 | Done (5 modules) |
| ARCH-02 | db/mod.rs | 4,926 | 860 | Done (12 migration files) |
| ARCH-03 | game_launcher.rs | 3,524 | 310 | Done (3 modules) |
| ARCH-03 | ac_server.rs | 2,185 | 1,059 | Done (2 modules) |
| ARCH-03 | cafe.rs | 2,172 | 730 | Done (2 modules) |
| ARCH-03 | cloud_sync.rs | 2,176 | 1,529 | Done (1 module) |
| ARCH-03 | ws/mod.rs | 3,185 | 2,457 | Partial (3 handler submodules) |
| ARCH-03 | pod_healer.rs | 2,525 | — | Deferred (sed corrupts file, use Edit tool) |
| ARCH-03 | auth/mod.rs | 2,444 | — | Deferred (complex internal dependencies) |
| ARCH-03 | multiplayer.rs | 1,749 | — | Pending |
| ARCH-03 | config.rs | 1,749 | — | Pending |
| ARCH-03 | fleet_health.rs | 1,652 | — | Pending |
| ARCH-04 | CI gate | — | — | Pending |
| ARCH-05 | Dead code removal | — | — | Pending |

## Session Commits (7 this session, 14 total milestone)

### This session

1. `2599ea9f` — fix(rc-agent): ADAPTER-SWAP-06 port separation
2. `58fee487` — fix(tests): align 4 CI-failing tests
3. `6a51f410` — **billing.rs split** (9,142→386, 5 modules)
4. `1046d301` — **game_launcher.rs split** (3,524→310, 3 modules)
5. `9d722463` — **ws/mod.rs split** (3 handler submodules)
6. `8945b19b` — **cafe.rs + cloud_sync.rs split** (3 modules)
7. `12a4193a` — **ac_server.rs split** (2 modules)

## James Status

- **Server rebuilt:** `58fee487` (verified via SSH, 9/9 pods connected)
- **Lap verification:** Pending — public leaderboard empty, need customer race
- **Comms:** v49 status message sent (rebuild instructions + progress). No human reply yet. Automated fleet-monitor showing people tracker (:8095) offline.
- **James priorities (from comms id 26965):** P356→P357 (business rules), Pod 8 canary, spectator builds, F1 25 audit

## Resume Plan (Next Session)

### Priority 1: Verify laps

```bash

# Check laps table directly

ssh bono@100.82.33.94 "powershell -Command \"(Invoke-WebRequest -Uri 'http://192.168.31.23:8080/api/v1/public/leaderboard' -UseBasicParsing).Content\""
```

If records[] is non-empty → Phase 384 COMPLETE → unblocks all downstream phases.

### Priority 2: Continue ARCH-03 splits

**Use Edit tool, NOT sed** for pod_healer.rs — sed corrupted the file twice.

- pod_healer.rs (2,525): Make types pub first, then extract diagnostics + repair
- auth/mod.rs (2,444): Extract OTP + PIN modules into auth/ submodules
- multiplayer.rs (1,749), config.rs (1,749), fleet_health.rs (1,652)

### Priority 3: If laps verified

- Start Phase 386 (autonomous pricing) or Phase 387 (opt-in/opt-out)
- Phase 386 needs James's Phase 356 (business_rules table) — check status

## Roadmap Evolution

- Phase 413 added: Service key provisioning + deploy-server.sh hardening (Option Z + respawn race fixes) — 2026-04-18. (Initially auto-numbered 315 by `gsd-tools phase add` which collided with shipped v41.0 Phase 315 — manually renumbered to 413 per v52.0's 393-412 range.) Bundles three work-items: (1) Option Z mesh key fetch-at-boot (new server route `GET /api/v1/pods/mesh-service-key` gated by network_middleware, rc-agent MeshKeyCache with `spawn_periodic_refetch`, rewire 3 RCAGENT_SERVICE_KEY env-readers to cache); (2) deploy-server.sh respawn fixes (extend schtasks disable to 8 tasks, unify sentinel on `OTA_DEPLOYING`, replace WINDOWTITLE filter with WMIC commandline match); (3) audit other service-key provisioning paths. Triggered by Gap 4 (pod HKLM key ≠ server TOML key, Tier 0 dead fleet-wide) + deploy abort 03:13 IST 2026-04-18. Cross-system bridge — MMA audit mandatory per standing rules.
- Phase 392.1 inserted after Phase 392: P0 zero-laps 3-layer fix + folded C1 FK-PRAGMA deploy (URGENT, 2026-04-16). Manual insert — `/gsd:insert-phase` parser cannot read racecontrol `### Phase N:` nested heading format, returns `found:false` on all phases 1-393. Parser fix deferred as separate side-task. **Status update 2026-04-16:** CONTEXT.md + 392-1-01-PLAN.md committed in `fd8916d5`. Pre-flight complete (rollback snapshots verified at 176,910,336 B venue / 172,019,712 B cloud; `d24b17f7` in HEAD ancestry; venue build `43e35dc7`, cloud build `fc9dfea2`). **Step 1 ground-truth deviation:** plan assumed `pricing_rules.min_duration_secs` column — no such column or table exists. Actual per-minute tier lives in `pricing_tiers` with `duration_minutes=0` + `billing_mode='per_minute'`; session length comes from customer's `custom_duration_minutes` at booking time (`billing_start_validate.rs:82,362-368`). The validator has only upper-bound checks (`> 1440`) and no minimum floor — a per-minute booking of 1 minute allocates 60s, < fastest-lap ~105s, yielding zero laps. True Layer 1 fix shape: add a minimum-floor check in `validate_splits_and_duration` when `tier_duration_minutes == 0`. Plan Step 1/Step 2 prose needs amendment before code change ships. Binary swap NOT started; paused at Step 1 report for user approval on fix wording + floor value.

## Key Lesson: sed vs Edit Tool

**NEVER use sed for multi-line Rust file modifications.** sed silently empties files when encountering certain patterns (happened twice with pod_healer.rs). Use the Edit tool for all code modifications — it validates changes and reports errors instead of silently corrupting.

## Phase 413 Plan 05 — deploy-server.sh Factor 1 closed (2026-04-17)

**Completed:** 2026-04-17 (parallel Wave 1 executor)
**Scope:** Extend schtasks disable/re-enable list from 2 to 8 RC-related scheduled tasks in all 3 blocks of `scripts/deploy-server.sh`.
**Commits:** `0fc38726` (Task 1 disable block), `e38a9e81` (Task 2 success re-enable), `7c7af7ec` (Task 3 rollback re-enable)
**Files:** `scripts/deploy-server.sh` (+9 lines, -4 lines net across 3 edits)
**Coverage:** 8 tasks × 3 blocks (1 disable + 2 enables each) = 24 `schtasks /Change /TN` invocations (was 6)
**Verification:** bash -n clean; per-task grep counts all match (1 Disable + 2 Enables × 8); taskkill WINDOWTITLE + DEPLOY_IN_PROGRESS fragments preserved intact for Plan 06/07
**Closes:** Factor 1 of the 2026-04-18 03:13 IST deploy abort (RCWatchdog respawn race) — not live-exercised yet; first test on next deploy run
**Summary:** `.planning/phases/413-service-key-provisioning-deploy-server-sh-hardening-option-z-respawn-race-fixes/413-05-SUMMARY.md`

## Phase 413 Plan 06 — deploy-server.sh Factor 2 closed (2026-04-18)

**Completed:** 2026-04-18 (parallel Wave 2 executor, --no-verify)
**Scope:** Rename deploy sentinel from `DEPLOY_IN_PROGRESS` to `OTA_DEPLOYING` in all 3 blocks of `scripts/deploy-server.sh` (write + success-path delete + rollback-path delete). Add Phase 413 Factor 2 explanatory comment above the write block citing `start-racecontrol-watchdog.ps1:61`.
**Commits:** `d92c3843` (Task 1: single atomic rename across all 3 blocks)
**Files:** `scripts/deploy-server.sh` (+7 lines, -3 lines — 3 substring renames + 4 comment lines)
**Before/after counts:**
- `grep -c DEPLOY_IN_PROGRESS scripts/deploy-server.sh` : 3 → 0
- `grep -c OTA_DEPLOYING scripts/deploy-server.sh` : 0 → 5 (3 functional + 2 in comment)
- `grep -c 'del /Q C:\\RacingPoint\\OTA_DEPLOYING' scripts/deploy-server.sh` : 2
- bash -n clean; Plan 05 `RCWatchdog` count=3 preserved; `start-racecontrol-watchdog.ps1` untouched (2 OTA_DEPLOYING hits)
**Deviation (Rule 1, documentation bug):** Plan prescribed comment text contained the literal `DEPLOY_IN_PROGRESS` substring, which contradicted the `grep -c DEPLOY_IN_PROGRESS = 0` acceptance criterion. Reworded comment to `a different sentinel name the PS watchdog never checked` — preserves intent, satisfies the stricter invariant.
**Closes:** Factor 2 of the 2026-04-18 03:13 IST deploy abort — writer + checker now agree on `OTA_DEPLOYING`. PS watchdog will see the sentinel during the next kill→swap→start window and skip its restart. Not live-exercised yet; first test on next `bash scripts/deploy-server.sh` invocation.
**Summary:** `.planning/phases/413-service-key-provisioning-deploy-server-sh-hardening-option-z-respawn-race-fixes/413-06-SUMMARY.md`

## Phase 413 Plan 03 — rc-agent MeshKeyCache boot wire-up closed (2026-04-18)

**Completed:** 2026-04-18 (parallel Wave 2 executor)
**Scope:** Wire `MeshKeyCache` (from Plan 02) into rc-agent's `main.rs` boot sequence. `let mesh_key_cache = crate::mesh_key_cache::new_cache()` placed below `let flags_arc` (same scope). Initial synchronous best-effort fetch + `rc_common::boot_resilience::spawn_periodic_refetch` at 300s interval placed immediately after the feature_flags periodic refetch block. Both insertions feature-gated on `http-client` (matches module gate).
**Commits:** `28de9e30` (Task 1: full wire-up — `+50 lines` to `crates/rc-agent/src/main.rs`, two additive insertions, strictly no modifications to existing code)
**Files:** `crates/rc-agent/src/main.rs` (+50 lines)
**Verification:** `cargo build --release --bin rc-agent` → 0 errors, 100 pre-existing warnings (3 fewer than Plan 02 baseline); `cargo test -p rc-agent-crate --bin rc-agent mesh_key_cache` → 10/10 passing. Acceptance-criteria grep counts all match (new_cache=1, spawn_periodic_refetch=2, "mesh_service_key"=1, fetch_from_server=2). Only mesh_key_cache-related compiler note is `get_key_or_env is never used` — expected until Plan 04.
**Log lines available for Plan 10 verification:** `Mesh key cache initial fetch ok/failed`, `Mesh key cache periodic re-fetch started (interval=300s)`, plus rc_common's `periodic_refetch started/first_success/failed/self_healed resource="mesh_service_key"`.
**Design decisions:** 300s cadence matches feature_flags (same operations profile). Initial fetch non-fatal — Ok→info, Err→warn, never short-circuits boot. `#[allow(unused_variables)]` on the let binding with TODO — Plan 04 removes the allow when it adds the three consumer .clone() calls.
**Deviations:** None. Plan executed exactly as specified. `#[allow(unused_variables)]` explicitly permitted by plan's acceptance-criteria note; feature-gating (`#[cfg(feature = "http-client")]`) followed the plan template.
**Next plan (04):** Rewire the three RCAGENT_SERVICE_KEY env consumers (ai_debugger.rs:779, remote_ops.rs:165, ws_handler.rs:431) to use `mesh_key_cache::get_key_or_env(&cache).await.unwrap_or_default()`. Closes Gap 4 (pod HKLM key ≠ server TOML key silent 401 fleet-wide).
**Summary:** `.planning/phases/413-service-key-provisioning-deploy-server-sh-hardening-option-z-respawn-race-fixes/413-03-SUMMARY.md`

## Phase 413 Plan 02 — rc-agent MeshKeyCache (Option Z data layer) closed (2026-04-17)

**Completed:** 2026-04-17 (parallel Wave 1 executor)
**Scope:** New module `crates/rc-agent/src/mesh_key_cache.rs` (329 lines) — `MeshKeyCache = Arc<RwLock<Option<String>>>` type + `fetch_from_server` HTTP client + `get_key_or_env` helper. Wire-up in main.rs via `mod mesh_key_cache;` gated on `http-client` feature.
**Commits:** `45d85c14` (Task 1: module + Cargo.toml wiremock dep — commit mislabeled "413-01" due to parallel-agent commit-collision; all Task 1 files are present), `85b1968e` (Task 2: `mod mesh_key_cache;` registration in main.rs)
**Files:** `crates/rc-agent/src/mesh_key_cache.rs` (new, 329 lines), `crates/rc-agent/Cargo.toml` (+wiremock dev-dep), `crates/rc-agent/src/main.rs` (+2 lines for mod), `Cargo.lock` (wiremock transitive deps)
**Tests:** 10 unit tests (`cargo test -p rc-agent-crate --bin rc-agent mesh_key_cache`) — all passing. Coverage: 200+non-empty/200+empty/400/403/500/network-error/empty-overwrites-existing/cache-hit/env-fallback/both-empty.
**W5 observability:** 403/FORBIDDEN logged at `tracing::warn!`; other non-2xx at `debug!`. Cache behavior identical — `error_for_status()?` propagates Err to periodic_refetch, preserving last-known-good.
**Deviations documented:** (1) Rule 3 no-lib.rs → `mod` in main.rs; (2) Rule 3 `--lib` flag swapped for `--bin rc-agent` in verify commands; (3) Rule 3 parallel-agent commit-collision absorbed Task 1 files into `45d85c14` (branded 413-01). No code lost; all deviations noted in SUMMARY.
**Next plan (03):** Wire `MeshKeyCache` into `main.rs` boot sequence via `rc_common::boot_resilience::spawn_periodic_refetch`.
**Summary:** `.planning/phases/413-service-key-provisioning-deploy-server-sh-hardening-option-z-respawn-race-fixes/413-02-SUMMARY.md`

---
*Last updated: 2026-04-18 IST — Phase 413-03 (rc-agent MeshKeyCache boot wire-up) closed; cache now live + periodically refreshed at 300s (`28de9e30`)*
*Previous: 2026-04-18 IST — Phase 413-06 (deploy-server.sh sentinel unified on OTA_DEPLOYING) closed; Factor 2 of 2026-04-18 03:13 IST deploy abort resolved (`d92c3843`)*
*Previous: 2026-04-17 IST — Phase 413-02 (rc-agent MeshKeyCache) closed; 10 tests green, release build clean*
*Previous: 2026-04-14 12:00 IST — 7 commits pushed, 6 file splits shipped, James server rebuilt*
