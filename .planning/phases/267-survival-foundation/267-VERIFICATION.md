---
phase: 267-survival-foundation
verified: 2026-03-30T09:00:00+05:30
status: gaps_found
score: 4/5 success criteria verified
gaps:
  - truth: "HEAL_IN_PROGRESS sentinel is checked by rc-sentry, RCWatchdog, self_monitor, pod_monitor, and WoL before acting"
    status: partial
    reason: "pod_healer and wol.rs do not call any sentinel/lease check. They contain TODO(267-02) comments referencing state.lease_manager.has_active_lease() — a method that does not exist on LeaseManager. Plan 267-02 (LeaseManager) is complete and in AppState, so the integration can be done now. The 3 pod-side systems (rc-sentry, rc-watchdog, self_monitor) are fully wired."
    artifacts:
      - path: "crates/racecontrol/src/pod_healer.rs"
        issue: "Lines 649-654 and 969-971 have TODO(267-02) placeholder comments. No call to state.lease_manager.get_lease() or any sentinel check is present. The referenced method has_active_lease() does not exist on LeaseManager."
      - path: "crates/racecontrol/src/wol.rs"
        issue: "Lines 14-17 are doc-comment TODOs only. The caller (pod_healer) has no actual lease check before calling send_wol(). No lease guard is enforced."
    missing:
      - "Add call to state.lease_manager.get_lease(&pod.id) in run_graduated_recovery() before the recovery action block — if Some(lease) return early with a log entry carrying lease.action_id"
      - "Add call to state.lease_manager.get_lease(&pod.id) in the WoL path (~line 969) before wol::send_wol() — same early-return pattern"
      - "Either add a has_active_lease() convenience method to LeaseManager or use get_lease().is_some() directly"
  - truth: "OTA_DEPLOYING and HEAL_IN_PROGRESS sentinel checks are present in all existing recovery code paths"
    status: partial
    reason: "Same root cause as above. pod_healer and wol are the server-side recovery paths. They rely on COORD-02 (recovery_intents) and pod_deploy_states as substitutes, which are different coordination mechanisms — not the sentinel/lease protocol defined in SF-05."
    artifacts:
      - path: "crates/racecontrol/src/pod_healer.rs"
        issue: "SF-05 check is a TODO comment, not an implemented guard. The existing COORD-02/COORD-03 checks cover different scenarios (planned self-restarts, OTA binary staging) but do not check the HEAL_IN_PROGRESS lease for Layer1/2/3 healers that will be built in phases 268-272."
      - path: "crates/racecontrol/src/wol.rs"
        issue: "No lease check before WoL send. WoL is the final step in graduated recovery and can wake a pod that a healing layer has exclusive lease on."
    missing:
      - "Same fix as gap 1 — the two gaps share the same root cause and the same fix"
---

# Phase 267: Survival Foundation Verification Report

**Phase Goal:** All 5 existing recovery systems coordinate via shared sentinel protocol and structured types so they cannot fight each other over the same patient
**Verified:** 2026-03-30T09:00:00 IST
**Status:** GAPS FOUND
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths (Success Criteria)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | HEAL_IN_PROGRESS sentinel is checked by rc-sentry, RCWatchdog, self_monitor, pod_monitor, and WoL before acting | PARTIAL | rc-sentry (lines 1135-1156), rc-watchdog (lines 178-193), self_monitor (lines 348-370) all call `any_sentinel_active()`. pod_healer and wol.rs have TODO comments only — no actual check implemented. |
| 2 | Server grants heal leases with TTL via POST /api/v1/pods/{id}/heal-lease | VERIFIED | `survival_routes()` registered at line 63 of routes.rs. POST /pods/{pod_id}/heal-lease returns `HealLeaseResponse { granted, lease, reason }` with 200 (granted) or 409 (denied). TTL enforced via `expires_at` field. |
| 3 | Every cross-layer operation log entry carries the same action_id | VERIFIED | `ActionId` is UUID v4, present in `HealLease`, `HealSentinel`, `SurvivalReport`, `DiagnosisContext`. All 3 wired sentinel checks log `action_id = %sentinel.action_id`. survival.rs logs `action_id = %req.action_id` on grant and renew. |
| 4 | SurvivalReport, HealLease, BinaryManifest, DiagnosisContext structs exist in rc-common | VERIFIED | All 4 structs present in `crates/rc-common/src/survival_types.rs`, exported via `pub mod survival_types` in lib.rs. 18 tests passing, 0 errors on `cargo check -p rc-common`. |
| 5 | OTA_DEPLOYING and HEAL_IN_PROGRESS sentinel checks are present in all existing recovery code paths | PARTIAL | Same gap as SC #1. pod_healer and wol.rs are the two unguarded server-side recovery paths. |

**Score: 3/5 truths fully verified, 2/5 partial**

---

## Required Artifacts

### Plan 01 Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/rc-common/src/survival_types.rs` | All survival foundation types + sentinel protocol | VERIFIED | 740 lines. All exported types present: ActionId, SentinelKind, SurvivalLayer, HealSentinel, SurvivalReport, HealLease, HealLeaseRequest, HealLeaseResponse, BinaryManifest, DiagnosisContext, OpenRouterDiagnose trait, DiagnosisResult, DiagnosisFinding, FindingSeverity, DiagnosisError. Sentinel helpers: try_acquire_sentinel, check_sentinel, release_sentinel, any_sentinel_active. 18 tests passing. |
| `crates/rc-common/src/lib.rs` | Module export for survival_types | VERIFIED | Line 12: `pub mod survival_types` present. |

### Plan 02 Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/racecontrol/src/api/survival.rs` | Heal lease endpoints + LeaseManager | VERIFIED | 555 lines. LeaseManager with request_lease, renew_lease, release_lease, get_lease. All 3 Axum handlers present. 11 tests passing. |
| `crates/racecontrol/src/api/routes.rs` | Route registration for /api/v1/pods/:id/heal-lease | VERIFIED | Line 12: `use super::survival`. Line 63: `.merge(survival::survival_routes())`. |
| `crates/racecontrol/src/state.rs` | LeaseManager in AppState | VERIFIED | Line 9: `use crate::api::survival::LeaseManager`. Line 232: `pub lease_manager: std::sync::Arc<LeaseManager>`. Line 311: initialized with `Arc::new(LeaseManager::new())`. |

### Plan 03 Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/rc-sentry/src/tier1_fixes.rs` | Sentinel check in handle_crash and enter_maintenance_mode before restart | VERIFIED | `any_sentinel_active()` called at line 1140 (handle_crash) and line 843 (enter_maintenance_mode). Both log skip with action_id. |
| `crates/rc-watchdog/src/service.rs` | Sentinel check in main poll loop before restart | VERIFIED | `any_sentinel_active()` called at line 183. Logs layer, action_id, remaining TTL. `continue`s poll cycle. |
| `crates/rc-agent/src/self_monitor.rs` | Sentinel check before relaunch_self | VERIFIED | `any_sentinel_active()` called at line 353. Logs action_id and layer. RESTART_COUNT fetch_sub included (per deviation note). |
| `crates/racecontrol/src/pod_healer.rs` | Sentinel/lease check before AI escalation and graduated recovery | STUB | Lines 649-654 and 969-971 are TODO comments only. `state.lease_manager.get_lease()` is never called. `has_active_lease()` referenced in the TODO does not exist on LeaseManager. |
| `crates/racecontrol/src/wol.rs` | Sentinel check before WoL packet send | STUB | Lines 14-17 are doc-comment TODOs. No lease check exists before `wol::send_wol()` is called in pod_healer. |

---

## Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `rc-common/src/survival_types.rs` | `rc-common/src/lib.rs` | `pub mod survival_types` | WIRED | Confirmed line 12 of lib.rs |
| `racecontrol/src/api/survival.rs` | `rc-common/src/survival_types.rs` | `use rc_common::survival_types` | WIRED | Line 24 of survival.rs |
| `racecontrol/src/api/routes.rs` | `racecontrol/src/api/survival.rs` | `survival_routes()` | WIRED | Lines 12 and 63 of routes.rs |
| `rc-sentry/src/tier1_fixes.rs` | `rc-common/src/survival_types.rs` | `any_sentinel_active` | WIRED | Lines 842 and 1139 |
| `rc-watchdog/src/service.rs` | `rc-common/src/survival_types.rs` | `any_sentinel_active` | WIRED | Line 182 |
| `rc-agent/src/self_monitor.rs` | `rc-common/src/survival_types.rs` | `any_sentinel_active` | WIRED | Line 352 |
| `racecontrol/src/pod_healer.rs` | `racecontrol/src/api/survival.rs` | `state.lease_manager.get_lease()` | NOT_WIRED | Only TODO comments at lines 649-654 and 969-971. Method referenced (`has_active_lease`) does not exist. |
| `racecontrol/src/wol.rs` | sentinel/lease check | caller responsibility | NOT_WIRED | No lease guard at the WoL call site in pod_healer. |

---

## Data-Flow Trace (Level 4)

Not applicable — this phase produces types, sentinel helpers, and coordination endpoints. No UI components render data from these artifacts.

---

## Behavioral Spot-Checks

Step 7b: SKIPPED — server is not running in this context. The LeaseManager is in-memory only; behavioral checks require a live server process. The 11 unit tests in survival.rs cover all grant/deny/renew/release paths and serve as the functional verification.

---

## Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| SF-01 | 267-01, 267-03 | HEAL_IN_PROGRESS sentinel with JSON payload checked by all 5 recovery systems | PARTIAL | Sentinel struct and file helpers fully implemented. 3/5 systems check it (rc-sentry, rc-watchdog, self_monitor). pod_healer and wol.rs are not wired. |
| SF-02 | 267-02 | Server-arbitrated heal lease with TTL, grant/renew/release | SATISFIED | LeaseManager in AppState. 3 endpoints live. 11 tests passing. Expired leases auto-freed. |
| SF-03 | 267-01, 267-02 | Structured action_id logging across all cross-layer operations | SATISFIED | ActionId is UUID v4 present in all cross-layer structs. All sentinel checks and lease operations log action_id. |
| SF-04 | 267-01 | SurvivalReport, HealLease, BinaryManifest, DiagnosisContext in rc-common | SATISFIED | All 4 structs present, substantive, exported, 18 tests passing. |
| SF-05 | 267-03 | All 5 existing recovery systems check HEAL_IN_PROGRESS + OTA_DEPLOYING before acting | PARTIAL | rc-sentry, rc-watchdog, self_monitor: wired. pod_healer and wol.rs: TODO comments only — not wired. |

No orphaned requirements found. All 5 SF-* IDs are declared in PLAN frontmatter and present in REQUIREMENTS.md with Phase 267 mapping.

---

## Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `crates/racecontrol/src/pod_healer.rs` | 649-654 | `TODO(267-02): ... has_active_lease()` — method does not exist on LeaseManager | Blocker | pod_healer proceeds to graduated recovery without any lease check. Future Phase 268-272 healers that hold a lease will fight pod_healer. |
| `crates/racecontrol/src/pod_healer.rs` | 969-971 | `TODO(267-02): Check state.lease_manager.has_active_lease(&pod.id)` before WoL | Blocker | WoL can wake a pod under active heal control. |
| `crates/racecontrol/src/wol.rs` | 14-17 | Doc-comment TODO only — no enforcement at call site | Warning | Caller (pod_healer) ignores the doc comment. |

Note: The SUMMARY.md for Plan 03 explicitly describes these as "placeholder comments for Plan 267-02 integration" — but Plan 267-02 is now complete. The TODOs reference a method (`has_active_lease`) that does not exist; they reference `get_lease()` would need to be used instead (`get_lease().is_some()` or a new convenience wrapper). These are not pre-existing TODOs waiting on a future plan — the dependency is resolved.

---

## Human Verification Required

None — all verification items are programmatically checkable for this phase (types, sentinel file protocol, API wiring).

---

## Gaps Summary

The phase delivers 3 of its 5 success criteria fully. The two failures share a single root cause:

**pod_healer and wol.rs are not wired to the lease/sentinel protocol.**

The plan (267-03) explicitly deferred this with TODO(267-02) comments, anticipating that Plan 267-02 (LeaseManager) had not yet shipped. Plan 267-02 IS now shipped (commit ef232dc6) and `state.lease_manager` IS available in AppState. The integration can be completed with roughly 6 lines of code in pod_healer at two call sites:

1. In `run_graduated_recovery()` after the COORD-03 check (~line 654): `if state.lease_manager.get_lease(&pod.id).is_some() { /* log + return */ }`
2. Before the WoL send (~line 969): same pattern.

The `has_active_lease()` method referenced in the TODOs does not exist — it must either be added to LeaseManager as a convenience wrapper around `get_lease().is_some()`, or the call sites must use `get_lease()` directly.

Until these two call sites are wired, the phase goal ("all 5 existing recovery systems coordinate via shared sentinel protocol") is not achieved. The future healing layers (Phases 268-272) will be able to hold a lease via the API, but pod_healer (which runs continuously on the server) will not yield to them.

---

## Commits Verified

| Hash | Plan | Description |
|------|------|-------------|
| `7dc4ddee` | 267-01 | add survival_types.rs with all foundation types and sentinel protocol |
| `f7f7598d` | 267-02 | implement LeaseManager and heal-lease endpoints |
| `ef232dc6` | 267-02 | wire survival endpoints into routes and AppState |
| `335eebc0` | 267-03 | sentinel checks in rc-sentry handle_crash and rc-watchdog poll loop |
| `7356fcf8` | 267-03 | sentinel checks in self_monitor, pod_healer, and wol |

All 5 commits confirmed present in git log.

---

_Verified: 2026-03-30T09:00:00 IST_
_Verifier: Claude (gsd-verifier)_
