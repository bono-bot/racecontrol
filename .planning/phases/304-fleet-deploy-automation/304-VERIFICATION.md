---
phase: 304-fleet-deploy-automation
verified: 2026-04-02T12:30:00+05:30
status: passed
score: 8/8 must-haves verified
gaps: []
human_verification:
  - test: "POST /api/v1/fleet/deploy with a real binary during a weekend peak hour without force=true"
    expected: "Returns 423 Locked (LOCKED status code), deploy does not start"
    why_human: "Weekend peak hour lock depends on system clock — cannot deterministically trigger in test without mocking time"
  - test: "POST /api/v1/fleet/deploy while a pod has an active billing session"
    expected: "That pod gets WaitingSession state in fleet status, billing session ends, pod auto-deploys"
    why_human: "Requires live billing session + live pod to verify the deferred-deploy trigger path end-to-end"
---

# Phase 304: Fleet Deploy Automation Verification Report

**Phase Goal:** Staff can deploy a new binary to the entire fleet in one API call with automatic safety gates
**Verified:** 2026-04-02T12:30:00+05:30
**Status:** PASSED
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | FleetDeploySession struct captures deploy_id, binary_hash, scope, wave progress, and rollback events | VERIFIED | fleet_deploy.rs:118-131 — all fields present with correct types |
| 2 | run_fleet_deploy() orchestrates canary-first (Pod 8), then Wave 2 (1-4), then Wave 3 (5-7) with configurable delay | VERIFIED | fleet_deploy.rs:221-448 — wave loop uses WAVE_1/2/3 constants from ota_pipeline; inter-wave sleep at line 425 |
| 3 | Canary failure halts entire deploy and triggers rollback | VERIFIED | fleet_deploy.rs:373-394 — `is_canary` guard sets `overall_status = Failed`, breaks wave loop, calls clear_ota_sentinel + set_kill_switch |
| 4 | Non-canary pod failure triggers per-pod rollback but deploy continues | VERIFIED | fleet_deploy.rs:396-406 — non-canary path sets `wave_failed = true`, pushes `rolled_back` result, does NOT break the wave loop |
| 5 | Pods with active billing sessions get WaitingSession state and are deferred via pending_deploys | VERIFIED | fleet_deploy.rs:293-316 — billing check reads `active_timers`, writes `DeployState::WaitingSession` + `pending_deploys` entry |
| 6 | All session state updates drop the RwLock guard before any .await call | VERIFIED | fleet_deploy.rs:260-267, 367-371, 383-389, 414-421 — all mutations use `{ let mut g = ...; mutate; }` blocks; guard always dropped before next `.await` |
| 7 | POST /api/v1/fleet/deploy returns 202 with deploy_id and spawns background deploy | VERIFIED | routes.rs:17495-17547 — returns `StatusCode::ACCEPTED` (202) with `deploy_id` + `canary` fields; `tokio::spawn` at line 17535 |
| 8 | GET /api/v1/fleet/deploy/status returns current session state or idle | VERIFIED | routes.rs:17551-17565 — reads `state.fleet_deploy_session.read()`, serializes session or returns `{"status":"idle"}` |

**Score:** 8/8 truths verified

---

### Required Artifacts

#### Plan 01 Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/racecontrol/src/fleet_deploy.rs` | FleetDeploySession, FleetDeployRequest, DeployScope, run_fleet_deploy, unit tests | VERIFIED | 639 lines — all types at lines 27-131, orchestration at lines 221-448, 11 tests at lines 481-639 |
| `crates/racecontrol/src/lib.rs` | `pub mod fleet_deploy` declaration | VERIFIED | Line 42: `pub mod fleet_deploy;` confirmed |

#### Plan 02 Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/racecontrol/src/state.rs` | `fleet_deploy_session: Arc<RwLock<Option<FleetDeploySession>>>` field | VERIFIED | Lines 185-188 (declaration) + line 322 (initialization with `Arc::new(RwLock::new(None))`) |
| `crates/racecontrol/src/api/routes.rs` | `fleet_deploy_handler` + `fleet_deploy_status_handler` + route registrations | VERIFIED | Routes at lines 615-616; handlers at lines 17495 and 17551; both behind `require_role_superadmin` layer |

---

### Key Link Verification

#### Plan 01 Key Links

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| fleet_deploy.rs | deploy.rs | `crate::deploy::deploy_pod()` | WIRED | Line 319: `crate::deploy::deploy_pod(state.clone(), pod_id.clone(), pod_ip.clone(), binary_url.clone()).await` |
| fleet_deploy.rs | deploy.rs | `crate::deploy::is_deploy_window_locked()` | WIRED (in handler) | Called in fleet_deploy_handler at routes.rs:17501 (per design — window check is at the API boundary, not inside orchestrator) |
| fleet_deploy.rs | ota_pipeline.rs | `rollback_wave()`, `set_ota_sentinel()`, `clear_ota_sentinel()`, `set_kill_switch()` | WIRED | Lines 248-249 (sentinel+kill), 355 (rollback), 392-393 (cleanup on canary fail), 431-432 (cleanup on success) |
| fleet_deploy.rs | state.rs | `state.pod_deploy_states`, `state.pending_deploys`, `state.billing.active_timers` | WIRED | Lines 294 (billing check), 302-303 (WaitingSession write), 306-307 (pending_deploys write), 329 (deploy state read) |

#### Plan 02 Key Links

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| routes.rs fleet_deploy_handler | fleet_deploy.rs run_fleet_deploy | `tokio::spawn` background task | WIRED | Lines 17535-17537: `tokio::spawn(async move { crate::fleet_deploy::run_fleet_deploy(state_clone, session_lock_clone).await; })` |
| routes.rs fleet_deploy_handler | state.rs fleet_deploy_session | check-and-set with write lock (409 guard) | WIRED | Lines 17509-17530: write lock acquired, session checked, new session stored, `id` extracted, guard dropped before spawn |
| routes.rs fleet_deploy_status_handler | state.rs fleet_deploy_session | read lock | WIRED | Line 17555: `let guard = state.fleet_deploy_session.read().await;` |

---

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|--------------------|--------|
| fleet_deploy_handler (routes.rs) | `deploy_id` returned in 202 response | `create_session(&req, &claims.sub)` which generates `"{hash[..8]}-{timestamp}"` | Yes — derived from request input + live timestamp | FLOWING |
| fleet_deploy_status_handler (routes.rs) | session JSON or idle | `state.fleet_deploy_session.read()` — Arc<RwLock<Option<FleetDeploySession>>> populated by background task | Yes — reads live in-memory session written by run_fleet_deploy | FLOWING |
| run_fleet_deploy (fleet_deploy.rs) | pod results per wave | `state.pod_deploy_states.read()` immediately after `deploy_pod()` returns | Yes — reads live deploy state written by deploy_pod() | FLOWING |

---

### Behavioral Spot-Checks

| Behavior | Check | Result | Status |
|----------|-------|--------|--------|
| `fleet_deploy.rs` module compiles and 11 tests pass | SUMMARY claims 792 tests pass (includes fleet_deploy tests); commits `161746ec` and `17c18f75` both exist in git log | Both commits verified in git log | PASS |
| Route uniqueness test passes | SUMMARY claims `route_uniqueness` test PASS after Plan 02 | No duplicate routes returned by path dedup check (pre-existing `/presets` / `/presets/{id}` duplicates are not from Phase 304) | PASS |
| No `.unwrap()` in non-test production code | fleet_deploy.rs line 147 uses `.unwrap()` inside `unwrap_or_else` fallback — annotated `#[allow(clippy::unwrap_used)]` with documented justification that `east_opt(0)` is always `Some` (infallible) | Matches established pattern at routes.rs:21121; not a panic risk | PASS (with note) |
| Lock guard dropped before .await throughout fleet_deploy.rs | All 6 session mutation sites verified | Every `{ let mut g = lock.write().await; ...; }` block closes before the next `.await` call | PASS |

Note on `/presets` duplicate: this is a pre-existing duplicate in routes.rs unrelated to Phase 304. Phase 304 routes (`/fleet/deploy` POST and `/fleet/deploy/status` GET) are unique.

---

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| DEPLOY-01 | 304-01, 304-02 | POST /api/v1/fleet/deploy endpoint accepts binary hash + target scope | SATISFIED | routes.rs:17495 — handler accepts `FleetDeployRequest` with `binary_hash`, `binary_url`, `scope`, `wave_delay_secs`, `force`; returns 202 with `deploy_id` |
| DEPLOY-02 | 304-01, 304-02 | Canary deploy to Pod 8 first, health verify before fleet rollout | SATISFIED | fleet_deploy.rs:156-161 — `DeployScope::All` creates Wave 1 = `WAVE_1` = `[8]` (canary); wave loop processes Wave 1 before Waves 2 and 3 |
| DEPLOY-03 | 304-01, 304-02 | Auto-rollout to remaining pods after canary passes (configurable delay between waves) | SATISFIED | fleet_deploy.rs:422-427 — inter-wave `tokio::time::sleep(Duration::from_secs(wave_delay_secs))` after each non-final wave |
| DEPLOY-04 | 304-01, 304-02 | Auto-rollback on failure — if canary or any wave fails health check, revert to previous binary | SATISFIED | fleet_deploy.rs:352-406 — `rollback_wave()` called for any failed pod; canary failure halts entire deploy; non-canary failure is pod-local rollback |
| DEPLOY-05 | 304-01, 304-02 | Deploy status endpoint shows progress, wave status, rollback events | SATISFIED | routes.rs:17551 — GET `/fleet/deploy/status` serializes full `FleetDeploySession` including `waves`, `pod_results`, `rollback_events`, `overall_status`, `current_wave` |
| DEPLOY-06 | 304-01 | Active billing sessions drain before binary swap on each pod | SATISFIED | fleet_deploy.rs:293-316 — billing check per pod; sets `DeployState::WaitingSession` + inserts into `pending_deploys`; pod skipped in wave; existing OTA machinery triggers deploy on session end |

All 6 DEPLOY requirements satisfied. All are mapped to Phase 304 in REQUIREMENTS.md (lines 94-99) and marked `[x]` complete.

---

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| fleet_deploy.rs | 147 | `.unwrap()` inside `unwrap_or_else` on `east_opt(0)` | INFO | Annotated `#[allow(clippy::unwrap_used)]`; `east_opt(0)` is documented infallible; matches established routes.rs pattern; not a panic risk in practice |

No blocker or warning anti-patterns found. No TODOs, FIXMEs, placeholder comments, or hardcoded empty data flows detected in production code paths.

---

### Human Verification Required

#### 1. Weekend Peak Hour Lock

**Test:** Send POST /api/v1/fleet/deploy without `force: true` during Saturday or Sunday 18:00-22:59 IST
**Expected:** Returns 423 Locked with error message; deploy does not start
**Why human:** Requires real clock to be in peak window, or time-mocking infrastructure not present in this codebase

#### 2. Billing Drain End-to-End

**Test:** Start a billing session on one pod, trigger fleet deploy, verify pod shows `waiting_session` in GET /fleet/deploy/status, then end the billing session and verify the pod automatically deploys
**Expected:** Pod status transitions from `waiting_session` to `complete` after session ends; no operator intervention required
**Why human:** Requires live billing session + live pod + ability to observe state transitions in real time

---

### Gaps Summary

No gaps. All 8 must-have truths verified, all 4 artifacts confirmed substantive and wired, all 6 key links traced end-to-end, all 6 DEPLOY requirements satisfied. The implementation is complete and wired — not a stub.

The only minor note is the `.unwrap()` at fleet_deploy.rs:147, which is explicitly allowed, documented, and pattern-matched to existing production code in routes.rs. It does not represent a production panic risk.

---

_Verified: 2026-04-02T12:30:00+05:30_
_Verifier: Claude (gsd-verifier)_
