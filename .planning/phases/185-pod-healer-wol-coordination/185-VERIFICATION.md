---
phase: 185-pod-healer-wol-coordination
verified: 2026-03-25T06:45:00+05:30
status: gaps_found
score: 9/11 must-haves verified
re_verification: false
gaps:
  - truth: "COORD-01 enforced at all recovery call sites (rc-sentry, self_monitor, pod_monitor, rc-watchdog)"
    status: partial
    reason: "ProcessOwnership enforcement wired only into pod_healer (TierOneRestart step and heal_pod). rc-sentry, self_monitor, and rc-watchdog do not query process_ownership before restarting rc-agent.exe. REQUIREMENTS-v17.1.md explicitly lists all four components."
    artifacts:
      - path: "crates/rc-sentry/src/"
        issue: "No owner_of or process_ownership usage found"
      - path: "crates/rc-watchdog/src/"
        issue: "No owner_of or process_ownership usage found"
    missing:
      - "ProcessOwnership check in rc-sentry restart path (tier1_fixes.rs or equivalent) before any rc-agent.exe restart"
      - "ProcessOwnership check in rc-watchdog before any restart action"
      - "self_monitor ownership check (or documented rationale for why self_monitor is exempt)"
  - truth: "REQUIREMENTS-v17.1.md updated to mark COORD-01/02/03 and MAINT-04 complete"
    status: failed
    reason: "REQUIREMENTS-v17.1.md still shows all four requirements as [ ] (pending) and Traceability table still says 'Pending' for Phase 185. Requirements file was not updated after implementation."
    artifacts:
      - path: ".planning/REQUIREMENTS-v17.1.md"
        issue: "COORD-01, COORD-02, COORD-03, MAINT-04 still marked [ ] — not updated post-implementation"
    missing:
      - "Mark COORD-01, COORD-02, COORD-03, MAINT-04 as [x] in REQUIREMENTS-v17.1.md"
      - "Update Traceability table Status from 'Pending' to 'Complete' for Phase 185 rows"
---

# Phase 185: Pod Healer WoL Coordination Verification Report

**Phase Goal:** pod_healer queries the recovery events API before escalating to Wake-on-LAN -- if rc-sentry already restarted the pod with spawn_verified: true within the last 60 seconds, WoL is skipped; a WOL_SENT sentinel is written via rc-sentry before sending WoL so all recovery systems see the escalation
**Verified:** 2026-03-25T06:45:00+05:30 (IST)
**Status:** gaps_found
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | pod_healer queries recovery_events ring buffer before WoL; skips WoL if rc-sentry restarted with spawn_verified=true within 60s | VERIFIED | `pod_healer.rs` WakeOnLan step CHECK 1: `state.recovery_events.lock()` + `store.query(Some(&pod.id), Some(60))` + `e.spawn_verified == Some(true)` guard at lines ~780-804 |
| 2 | WOL_SENT sentinel written to pod via rc-sentry /exec before magic packet is sent | VERIFIED | `pod_healer.rs` WakeOnLan step CHECK 3: POST to `http://<pod_ip>:8091/exec` with `echo WOL_SENT > C:\RacingPoint\WOL_SENT` before `wol::send_wol()` at lines ~842-898 |
| 3 | MAINTENANCE_MODE file checked via rc-sentry /files before WoL — WoL never sent to maintenance pod (MAINT-04) | VERIFIED | `pod_healer.rs` WakeOnLan step CHECK 2: GET `http://<pod_ip>:8091/files?path=...MAINTENANCE_MODE`; on 200 response advances to AlertStaff at lines ~807-840 |
| 4 | GRACEFUL_RELAUNCH sentinel check prevents pod_healer from acting during intentional rc-agent self-restart (COORD-03) | VERIFIED | `pod_healer.rs` Gate C in `run_graduated_recovery`: GET `/files?path=...GRACEFUL_RELAUNCH` at port 8091; returns on 200. Lines ~609-635 |
| 5 | RecoveryIntent with 2-min TTL prevents two authorities from recovering same pod+process simultaneously (COORD-02) | VERIFIED | Gate B in `run_graduated_recovery`: `state.recovery_intents.lock().has_active_intent(&pod.id, "rc-agent.exe")` check before any action. Intent registered on TierOneRestart with `RecoveryIntent::new(..., "graduated_tier1_restart")`. Lines ~595-607 and ~722-731 |
| 6 | ProcessOwnership registry initialized at startup with rc-agent.exe owned by RcSentry | VERIFIED | `state.rs` AppState::new(): `ownership.register("rc-agent.exe", RecoveryAuthority::RcSentry)` at lines ~272-278 |
| 7 | ProcessOwnership enforced in pod_healer TierOneRestart step (COORD-01 partial) | VERIFIED | `pod_healer.rs` TierOneRestart: `ownership.owner_of("rc-agent.exe")` check advances to AiEscalation if owner != PodHealer. Lines ~686-700 |
| 8 | heal_pod pod_needs_restart flag guarded by ownership check | VERIFIED | `pod_healer.rs` heal_pod Rule 2 path: `is_restart_owner` computed via `ownership.owner_of("rc-agent.exe").map_or(true, ...)` at lines ~327-331 |
| 9 | WakeOnLan is a new PodRecoveryStep between TierOneRestart and AiEscalation | VERIFIED | `pod_healer.rs` enum `PodRecoveryStep::WakeOnLan` at line 91; TierOneRestart advances to WakeOnLan at line 768; WakeOnLan match arm ends with `tracker.step = PodRecoveryStep::AiEscalation` |
| 10 | COORD-01 enforced across ALL recovery authorities (rc-sentry, self_monitor, rc-watchdog) | FAILED | ProcessOwnership not found in `crates/rc-sentry/` or `crates/rc-watchdog/`. REQUIREMENTS-v17.1.md requires enforcement in "rc-sentry, self_monitor, pod_monitor, and rc-watchdog" — only pod_healer is wired |
| 11 | REQUIREMENTS-v17.1.md updated to reflect completed requirements | FAILED | COORD-01, COORD-02, COORD-03, MAINT-04 still `[ ]` in REQUIREMENTS-v17.1.md; traceability table still shows "Pending" for all four Phase 185 entries |

**Score:** 9/11 truths verified

---

## Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/rc-common/src/recovery.rs` | RecoveryIntent struct with 2-min TTL is_expired() | VERIFIED | Lines 246-280: struct with fields, `is_expired()` checks `num_seconds() > 120`. Tests at lines 483-506. |
| `crates/racecontrol/src/recovery.rs` | RecoveryIntentStore with register/has_active_intent/cleanup_expired | VERIFIED | Lines 66-97: full implementation. 4 tests covering register, expiry, pod scoping, cleanup. |
| `crates/racecontrol/src/state.rs` | AppState with process_ownership and recovery_intents fields | VERIFIED | Lines 203+207: both Mutex fields present. Initialized in AppState::new() at lines 272-279 with rc-agent.exe -> RcSentry. |
| `crates/racecontrol/src/pod_healer.rs` | All coordination gates + WakeOnLan step + WOL_SENT + MAINTENANCE_MODE + recovery event query | VERIFIED | All 3 gates (COORD-01/02/03), WakeOnLan step with all 3 pre-checks, intent registration, send_wol call confirmed. |

---

## Key Link Verification

| From | To | Via | Status | Details |
|------|----|----|--------|---------|
| `pod_healer.rs` | `state.rs` | `state.process_ownership` + `state.recovery_intents` | WIRED | Both fields accessed via `state.process_ownership.lock()` and `state.recovery_intents.lock()` with poison recovery |
| `pod_healer.rs` | `recovery.rs` (RecoveryIntentStore) | `intents.has_active_intent()` + `intents.register()` | WIRED | Both call paths present: check before action (Gate B) and register on TierOneRestart |
| `pod_healer.rs` | `state.recovery_events` (RecoveryEventStore) | `state.recovery_events.lock().query(pod_id, 60s)` | WIRED | CHECK 1 in WakeOnLan step; spawn_verified=true filter confirmed |
| `pod_healer.rs` | rc-sentry `/files` endpoint | HTTP GET for GRACEFUL_RELAUNCH and MAINTENANCE_MODE | WIRED | Both HTTP calls present at port 8091 with 3-second timeouts |
| `pod_healer.rs` | rc-sentry `/exec` endpoint | HTTP POST WOL_SENT sentinel before send_wol | WIRED | POST with `{"cmd": "echo WOL_SENT > C:\\RacingPoint\\WOL_SENT", "timeout_ms": 5000}` before `wol::send_wol()` call |
| `rc-sentry` | `ProcessOwnership` (COORD-01) | `owner_of` call before restart | NOT WIRED | No `ProcessOwnership` or `owner_of` usage found in `crates/rc-sentry/` |
| `rc-watchdog` | `ProcessOwnership` (COORD-01) | `owner_of` call before restart | NOT WIRED | No `ProcessOwnership` or `owner_of` usage found in `crates/rc-watchdog/` |

---

## Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| COORD-01 | 185-01 | ProcessOwnership registry enforced at all restart call sites | PARTIAL | pod_healer wired; rc-sentry and rc-watchdog NOT wired. REQUIREMENTS-v17.1.md says "rc-sentry, self_monitor, pod_monitor, and rc-watchdog". PLAN scoped to pod_healer only — scope gap vs requirement as written. |
| COORD-02 | 185-01 | Recovery intent registry with 2-min TTL prevents simultaneous restarts | SATISFIED | RecoveryIntentStore + RecoveryIntent(is_expired 120s) + Gate B in pod_healer. Full data flow: register on TierOneRestart, check on next cycle or by other authority. |
| COORD-03 | 185-01 | GRACEFUL_RELAUNCH sentinel distinguishes intentional restarts | SATISFIED (pod_healer scope) | Gate C queries rc-sentry /files; returns on 200 with SkipCascadeGuardActive logged. Same scope caveat as COORD-01 (other authorities not wired). |
| MAINT-04 | 185-02 | pod_healer reads MAINTENANCE_MODE via rc-sentry /files before WoL | SATISFIED | CHECK 2 in WakeOnLan step: HTTP GET /files?path=...MAINTENANCE_MODE; skips WoL and advances to AlertStaff on 200 response. |

**Note:** REQUIREMENTS-v17.1.md still marks COORD-01, COORD-02, COORD-03, MAINT-04 as `[ ]` (unchecked) — requirements were implemented but the document was not updated post-phase.

---

## Commit Verification

| Commit | Message | Exists |
|--------|---------|--------|
| `faa8f37d` | feat(185-01): add RecoveryIntent+RecoveryIntentStore and wire into AppState | CONFIRMED |
| `99f2102d` | feat(185-01): wire COORD-01/02/03 coordination gates into pod_healer | CONFIRMED |
| `9abadb82` | feat(185-02): context-aware WoL with recovery event query, MAINTENANCE_MODE check, WOL_SENT sentinel | CONFIRMED |

---

## Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| None found in new code | | No `.unwrap()`, no stubs, no placeholders | | |

All mutex locks in new code use `unwrap_or_else(|e| e.into_inner())` per standing rules. No `TODO`/`FIXME` markers found in modified sections.

---

## Human Verification Required

### 1. WoL Coordination End-to-End

**Test:** Simulate a pod going offline while rc-sentry posts a recovery event with `spawn_verified=true` to the server within 60 seconds of pod_healer's WakeOnLan step triggering. Verify pod_healer logs "skipping WoL, sentry restarted within grace window."
**Expected:** WoL magic packet NOT sent; pod_healer advances to AiEscalation.
**Why human:** Requires a coordinated live test: pod offline + rc-sentry recovery event POST within timing window.

### 2. MAINTENANCE_MODE WoL Block

**Test:** Write `C:\RacingPoint\MAINTENANCE_MODE` on a pod while rc-sentry is alive, then let pod_healer reach WakeOnLan step for that pod.
**Expected:** WoL skipped, `SkipMaintenanceMode` logged, tracker advances to AlertStaff.
**Why human:** Requires live pod, rc-sentry serving /files endpoint, pod_healer cycle reaching WakeOnLan step.

### 3. WOL_SENT Sentinel Visibility

**Test:** Trigger WoL for a pod (rc-sentry reachable, no MAINTENANCE_MODE, no recent sentry restart). Check `C:\RacingPoint\WOL_SENT` exists on the pod immediately after WoL packet is sent.
**Expected:** File `C:\RacingPoint\WOL_SENT` present on pod immediately after WoL escalation.
**Why human:** Requires live pod + rc-sentry /exec functional + WoL step reachable in graduated recovery.

---

## Gaps Summary

Two gaps block full requirement closure:

**Gap 1 — COORD-01 partial scope:** The REQUIREMENTS-v17.1.md definition of COORD-01 requires ProcessOwnership enforcement in four components: rc-sentry, self_monitor, pod_monitor (pod_healer), and rc-watchdog. Phase 185 only wired pod_healer. The PLAN's `must_haves` intentionally scoped this to pod_healer, but the requirement as written is broader. rc-sentry and rc-watchdog are the two remaining components that restart rc-agent.exe and have no ownership check. Without this, the ownership registry exists but doesn't prevent rc-sentry or rc-watchdog from conflicting with pod_healer's intent-registered recovery window. This is the primary coordination guarantee the phase was built for.

**Gap 2 — Requirements document not updated:** REQUIREMENTS-v17.1.md still shows COORD-01, COORD-02, COORD-03, and MAINT-04 as `[ ]` pending. The traceability table still shows "Pending" for all Phase 185 entries. This is a documentation gap but relevant because other phases and planning tools read requirements state.

These two gaps are related — the REQUIREMENTS document reflects that COORD-01 is not fully satisfied (rc-sentry and rc-watchdog unimplemented), which is accurate given gap 1.

---

_Verified: 2026-03-25T06:45:00+05:30 (IST)_
_Verifier: Claude (gsd-verifier)_
