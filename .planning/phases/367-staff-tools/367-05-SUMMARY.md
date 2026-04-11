---
plan: 367-05
phase: 367
title: Phase 362 Retro-Validation (GLD-G-05)
subsystem: rc-agent + racecontrol
tags: [testing, retro-validation, config-mismatch, whatsapp-alert, concurrent-load]
completed: "2026-04-11"
duration_mins: 25
tasks_completed: 4
tasks_total: 4
files_created:
  - crates/rc-agent/src/launch_verifier.rs (5 new mismatch tests added)
  - packages/shared-types/src/fleet.ts (ConfigMismatchDetected interface)
files_modified:
  - crates/racecontrol/src/api/routes.rs (superadmin route + handler)
  - crates/racecontrol/tests/integration.rs (8-pod concurrent test)
  - docs/API.md (ConfigMismatchDetected WS event docs)
  - packages/shared-types/src/index.ts (re-export)
key_decisions:
  - Tests placed in launch_verifier.rs (not per-adapter files) since mismatch comparison logic lives there, not in individual sim adapters
  - config_mismatches table created idempotently via CREATE TABLE IF NOT EXISTS inside handler (no migration needed)
  - shared-types/fleet.ts created as new file for fleet-related TS interfaces
dependency_graph:
  requires: [Phase 362 ConfigMismatchDetected pathway]
  provides: [GLD-G-05 retro-validation test infrastructure]
  affects: [racecontrol binary, rc-agent binary]
tech_stack:
  added: [sqlx in-memory SQLite for integration tests, tokio JoinSet for concurrent test]
  patterns: [superadmin-only test endpoint, CREATE TABLE IF NOT EXISTS idempotent migration]
commits:
  - hash: 36f6d2a0
    message: "feat(367-05): Phase 362 retro-validation test harness (GLD-G-05)"
---

# Phase 367 Plan 05: Phase 362 Retro-Validation Summary

One-liner: Superadmin E2E test endpoint + 5 per-adapter mismatch unit tests + 8-pod concurrent load test + API/TS docs close the 4 deferred items from Phase 362.

## Tasks Completed

| # | Task | Status | Commit |
|---|------|--------|--------|
| 01 | Add `POST /internal/test/config-mismatch` superadmin route | Done | 36f6d2a0 |
| 02 | Add unit tests for all 5 sim adapter mismatch detection | Done | 36f6d2a0 |
| 03 | Add 8-pod concurrent mismatch load test | Done | 36f6d2a0 |
| 04 | Update docs — OpenAPI spec + shared-types | Done | 36f6d2a0 |

## Verification Results

### Task 01 — Superadmin Route
- Route registered at line 701 in `crates/racecontrol/src/api/routes.rs` inside the `require_role_superadmin` layer
- Handler `internal_test_config_mismatch_handler` at line 25321 fires WhatsApp alert via `whatsapp_alerter::send_admin_alert`
- Persists to `config_mismatches` table (CREATE TABLE IF NOT EXISTS — idempotent)
- `cargo build --release --bin racecontrol` — 0 errors, 1 pre-existing warning

### Task 02 — 5 Per-Adapter Mismatch Tests
All 5 tests pass in `launch_verifier::tests`:

```
test launch_verifier::tests::test_ac_adapter_car_mismatch_detected ... ok
test launch_verifier::tests::test_acr_adapter_track_mismatch_detected ... ok
test launch_verifier::tests::test_f1_25_adapter_session_type_mismatch_detected ... ok
test launch_verifier::tests::test_iracing_adapter_num_cars_mismatch_detected ... ok
test launch_verifier::tests::test_lmu_adapter_multi_field_mismatch_detected ... ok

test result: ok. 7 passed; 0 failed
```

(7 includes `test_config_verify_mismatch_detected` + `config::tests::lenient_type_mismatch_falls_back_to_default`)

### Task 03 — 8-Pod Concurrent Load Test

```
test test_8pod_concurrent_mismatch_no_drops ... ok
test result: ok. 1 passed; 0 failed
```

Uses sqlx in-memory SQLite with `JoinSet` — no `Instant::now() - Duration::from_secs()` (CI-safe per memory rule).

### Task 04 — Docs
- `grep -n "ConfigMismatchDetected" docs/API.md` — returns match at line 726
- `grep -n "ConfigMismatchDetected" packages/shared-types/src/fleet.ts` — returns match at line 40
- TS interface exported from `packages/shared-types/src/index.ts`

## Deviations from Plan

### Note: Tests in launch_verifier.rs, not per-adapter files

**Found during:** Task 02 exploration

**Issue:** The plan suggested adding `#[cfg(test)]` blocks to each of the 5 sim adapter files (`assetto_corsa.rs`, etc.). The actual mismatch comparison logic lives in `launch_verifier.rs`, not in the individual adapter files. The adapter files only handle config reading/parsing.

**Fix:** Tests placed in `launch_verifier::tests` module where the comparison logic is — each test exercises the full mismatch detection pipeline for that adapter's config type.

**Classification:** Rule 1 (auto-fix) — tests at the real logic layer, not a stub location.

## Known Stubs

None — all 4 tasks are fully implemented and verified.

## Manual Verification Pending (Post-Deploy)

After deploying to server .23:
1. Get superadmin JWT, POST to `http://192.168.31.23:8080/api/v1/internal/test/config-mismatch`
2. Verify WhatsApp alert received on staff phone within 30s
3. Verify `config_mismatches` table row written with matching `pod_id`

This is post-deploy verification only — code-level verification complete.

## Self-Check: PASSED

- `36f6d2a0` exists: `git log --oneline 36f6d2a0 -1` confirmed
- Route registered in superadmin layer: confirmed at lines 700-701
- Handler function exists: confirmed at line 25321
- 7/7 mismatch tests pass: confirmed
- 8-pod concurrent test passes: confirmed
- API.md updated: confirmed at line 726
- shared-types updated: confirmed at line 40
