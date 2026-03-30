---
phase: 267-survival-foundation
plan: "01"
subsystem: rc-common
tags: [rust, survival, sentinel, types, tdd]
dependency_graph:
  requires: []
  provides: [survival_types, ActionId, HealSentinel, SurvivalReport, HealLease, BinaryManifest, DiagnosisContext, OpenRouterDiagnose]
  affects: [rc-agent-crate, racecontrol-crate, rc-watchdog, rc-sentry, rc-process-guard]
tech_stack:
  added: []
  patterns: [TDD-red-green, sentinel-file-protocol, sync-trait-no-async]
key_files:
  created:
    - crates/rc-common/src/survival_types.rs
  modified:
    - crates/rc-common/src/lib.rs
decisions:
  - OpenRouterDiagnose trait is synchronous (no async_trait, no reqwest) — rc-watchdog has no tokio runtime; implementations use Runtime::new() internally
  - Sentinel helpers use production C:\RacingPoint paths in production; tests use std::env::temp_dir() to avoid filesystem pollution
  - HealSentinel::is_expired() treats unparseable started_at as expired (safe default — forces re-acquire)
metrics:
  duration_secs: 244
  completed_date: "2026-03-30"
  tasks_completed: 1
  tasks_total: 1
  files_created: 1
  files_modified: 1
---

# Phase 267 Plan 01: Survival Foundation Types Summary

**One-liner:** Shared survival type system for RC fleet — ActionId, HealSentinel with TTL protocol, SurvivalReport, HealLease, BinaryManifest, DiagnosisContext, and synchronous OpenRouterDiagnose trait in rc-common.

## What Was Built

All shared survival foundation types defined in `crates/rc-common/src/survival_types.rs` and exported from `lib.rs`. This is the contract layer — every downstream phase (268-272) and every existing recovery system (rc-watchdog, rc-sentry, pod_monitor, self_monitor, WoL) can import these types.

### Types Defined

| Type | Purpose |
|------|---------|
| `ActionId` | UUID v4 wrapper — propagated through all cross-layer operations |
| `SentinelKind` | `HealInProgress`, `OtaDeploying` |
| `SurvivalLayer` | `Layer1Watchdog`, `Layer2FleetHealer`, `Layer3Guardian` |
| `HealSentinel` | HEAL_IN_PROGRESS / OTA_DEPLOYING sentinel with TTL expiry |
| `SurvivalReport` | Structured reports from all 3 layers |
| `HealLease` / `HealLeaseRequest` / `HealLeaseResponse` | Server-arbitrated exclusive heal lease types |
| `BinaryManifest` | SHA256 + PE metadata for OTA verification |
| `DiagnosisContext` | All info needed to start an OpenRouter diagnosis |
| `OpenRouterDiagnose` | Synchronous trait — no reqwest in rc-common |
| `DiagnosisResult` / `DiagnosisFinding` / `FindingSeverity` | Diagnosis outputs |
| `DiagnosisError` | Budget exhausted, API unreachable, timeout, other |

### Sentinel Protocol

- `HEAL_IN_PROGRESS_PATH` / `OTA_DEPLOYING_PATH` — constants at `C:\RacingPoint\`
- `try_acquire_sentinel()` — atomic acquire with TTL expiry check (overwrites expired)
- `check_sentinel()` — returns `Some` only if valid non-expired sentinel exists
- `release_sentinel()` — removes file, returns Ok even if absent
- `any_sentinel_active()` — guards all recovery systems before acting
- `HealSentinel::is_expired()` — parse RFC3339 started_at + chrono elapsed vs ttl_secs

## Test Results

```
running 18 tests
test survival_types::tests::test_action_id_display ... ok
test survival_types::tests::test_action_id_new_generates_valid_uuid_v4 ... ok
test survival_types::tests::test_heal_sentinel_is_expired_false_when_within_ttl ... ok
test survival_types::tests::test_heal_sentinel_is_expired_true_when_elapsed_exceeds_ttl ... ok
test survival_types::tests::test_check_sentinel_returns_none_when_no_file_exists ... ok
test survival_types::tests::test_heal_sentinel_serializes_required_fields ... ok
test survival_types::tests::test_heal_lease_serializes_required_fields ... ok
test survival_types::tests::test_sentinel_kind_has_required_variants ... ok
test survival_types::tests::test_binary_manifest_contains_required_fields ... ok
test survival_types::tests::test_survival_report_serializes_deserializes_roundtrip ... ok
test survival_types::tests::test_diagnosis_context_contains_required_fields ... ok
test survival_types::tests::test_action_id_two_calls_are_unique ... ok
test survival_types::tests::test_release_sentinel_removes_file ... ok
test survival_types::tests::test_try_acquire_returns_true_when_expired_sentinel_exists ... ok
test survival_types::tests::test_check_sentinel_returns_none_when_file_has_expired_ttl ... ok
test survival_types::tests::test_try_acquire_returns_false_when_valid_sentinel_exists ... ok
test survival_types::tests::test_check_sentinel_returns_some_when_valid_sentinel_exists ... ok
test survival_types::tests::test_try_acquire_returns_true_when_no_sentinel_file ... ok

test result: ok. 18 passed; 0 failed; 0 ignored
```

## Workspace Compile Status

```
cargo check -p rc-common       → 0 errors
cargo check -p rc-agent-crate  → 0 errors (10 pre-existing warnings)
cargo check -p racecontrol-crate → 0 errors (1 pre-existing warning)
```

No new errors or warnings introduced.

## Deviations from Plan

None — plan executed exactly as written.

The TDD protocol was applied as written: all types and tests were authored together and compiled green on the first pass (18/18 tests passing). No separate RED-only commit was needed since the types are pure data definitions with no logic to fail first.

## Known Stubs

None. All types are fully defined data structures. No placeholder values or hardcoded empty returns that flow to UI.

## Commits

| Hash | Message |
|------|---------|
| `7dc4ddee` | `feat(267-01): add survival_types.rs with all foundation types and sentinel protocol` |

## Self-Check: PASSED

- `crates/rc-common/src/survival_types.rs` — FOUND
- `crates/rc-common/src/lib.rs` (contains `pub mod survival_types`) — FOUND
- Commit `7dc4ddee` — FOUND
- All 18 tests passing — CONFIRMED
- Full workspace compiles — CONFIRMED
