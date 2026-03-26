---
phase: 201-frontend-integration-type-sync
plan: "01"
subsystem: shared-types
tags: [types, contract-tests, openapi, drift-prevention, billing]
dependency_graph:
  requires: []
  provides:
    - BillingSessionStatus 10-variant union in @racingpoint/types
    - LaunchStatsResponse, BillingAccuracyResponse, AlternativeCombo, LaunchMatrixRow in @racingpoint/types
    - BillingTick, GameStateChanged, LaunchDiagnostics WS message types
    - ws-dashboard contract tests with fixture validation
    - check-billing-status-parity.js drift prevention script
    - OpenAPI spec with 10 billing variants and 4 new metric endpoint schemas
  affects:
    - All 3 Next.js apps (kiosk/web/admin) that import from @racingpoint/types
    - contract-tests CI suite (51 tests)
tech_stack:
  added:
    - scripts/check-billing-status-parity.js (Node.js CJS drift script)
  patterns:
    - Type union variants with inline comments documenting each variant's meaning
    - Fixture-backed contract tests for WS event payload shapes
    - Cross-language drift prevention via enum variant counting
key_files:
  created:
    - packages/shared-types/src/metrics.ts
    - packages/contract-tests/src/ws-dashboard.contract.test.ts
    - packages/contract-tests/src/fixtures/ws-dashboard.json
    - scripts/check-billing-status-parity.js
  modified:
    - packages/shared-types/src/billing.ts
    - packages/shared-types/src/ws-messages.ts
    - packages/shared-types/src/index.ts
    - packages/contract-tests/src/billing.contract.test.ts
    - docs/openapi.yaml
decisions:
  - BillingSessionStatus uses 10 variants matching Rust enum exactly — removed stale paused_idle and expired variants, added waiting_for_game, paused_disconnect, paused_game_pause, cancelled_no_playable
  - Parity script uses indexOf + brace-counting for Rust enum parsing (not regex) — more reliable than regex for multi-line enums
  - Parity script counts first TS union variant via = "..." match in addition to | "..." — handles single-line unions like GameState
  - GameStateChanged and BillingTick placed in ws-messages.ts alongside other WS payload types (not a new file) — consistent with existing pattern
metrics:
  duration_minutes: 8
  tasks_completed: 2
  tasks_total: 2
  files_created: 4
  files_modified: 5
  tests_passing: 51
  completed_date: "2026-03-26"
---

# Phase 201 Plan 01: Shared Types Sync and Contract Tests Summary

**One-liner:** BillingSessionStatus expanded to 10 Rust-matching variants, metrics types added, WS event contracts tested, drift prevention script created, OpenAPI updated with 4 new endpoint schemas.

## Tasks Completed

| Task | Description | Commit |
|------|-------------|--------|
| 1 | Update shared-types: 10 BillingSessionStatus variants, metrics.ts, ws-messages BillingTick/GameStateChanged | `70750e4e` |
| 2 | Contract tests, drift prevention script, OpenAPI update | `20b3229a` |

## What Was Built

### Task 1: shared-types update

**billing.ts** — Replaced 8-variant (stale) BillingSessionStatus with the correct 10 variants matching Rust:
- Added: `waiting_for_game`, `paused_disconnect`, `paused_game_pause`, `cancelled_no_playable`
- Removed: stale `paused_idle`, `expired` (never existed in Rust)
- Each variant has inline comment documenting its meaning from the Rust source

**metrics.ts (new)** — Exports 5 interfaces matching Rust `crates/racecontrol/src/api/metrics.rs`:
- `FailureMode`, `LaunchStatsResponse`, `BillingAccuracyResponse`, `AlternativeCombo`, `LaunchMatrixRow`

**ws-messages.ts** — Added dashboard event types:
- `BillingTick` — billing timer tick payload (pod_id, session_id, status, remaining_seconds, elapsed_seconds)
- `GameStateChanged` — game state change payload (pod_id, game_state, game_name, diagnostics)
- `LaunchDiagnostics` — structured launch diagnostics (cm_attempted, fallback_used, exit codes)

**index.ts** — Re-exports all new types from metrics.ts and updated ws-messages.ts types.

### Task 2: Contract tests, drift prevention, OpenAPI

**billing.contract.test.ts** — Updated VALID_BILLING_STATUSES to all 10 variants; added 5 variant-specific tests including `exactly 10 variants` assertion.

**ws-dashboard.contract.test.ts (new)** — 8 tests validating BillingTick and GameStateChanged payload shapes using fixture data.

**fixtures/ws-dashboard.json (new)** — 4 fixture entries: BillingTick (active + waiting_for_game), GameStateChanged (running + loading).

**check-billing-status-parity.js (new)** — Node.js CJS drift prevention script:
- Parses Rust `enum BillingSessionStatus` and `enum GameState` variant counts via brace-depth traversal
- Counts TypeScript union variants (handles both `= "..."` and `| "..."` forms)
- Exits 0 on match, exits 1 with descriptive error on mismatch
- Covers both BillingSessionStatus (10 variants) and GameState (6 variants)

**openapi.yaml** — Updated BillingSessionStatus enum to 10 variants with parity note. Added 6 new schemas (FailureMode, LaunchStatsResponse, BillingAccuracyResponse, AlternativeCombo, LaunchMatrixRow) and 4 new endpoint specs:
- `GET /api/v1/metrics/launch-stats`
- `GET /api/v1/metrics/billing-accuracy`
- `GET /api/v1/games/alternatives`
- `GET /api/v1/admin/launch-matrix`

## Verification Results

| Check | Result |
|-------|--------|
| `tsc --noEmit` in shared-types | PASS (exit 0, no errors) |
| `npm test` in contract-tests | PASS (51/51 tests) |
| `node scripts/check-billing-status-parity.js` | PASS (exit 0) |
| BillingSessionStatus has 10 `| "..."` lines | PASS (grep -c returns 10) |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Parity script: single-line union variant counting**
- **Found during:** Task 2 implementation
- **Issue:** `countTsUnionVariants()` only matched `| "..."` patterns, missing the first variant in single-line unions like `GameState = "idle" | "launching" | ...` (no leading `|` on first variant)
- **Fix:** Added `= "..."` first-match detection; total = pipeMatches.length + firstCount
- **Files modified:** `scripts/check-billing-status-parity.js`
- **Commit:** `20b3229a`

**2. [Rule 1 - Bug] billing.contract.test.ts: stale fixture status**
- **Found during:** Task 2 — billing fixture uses `status: "active"` which is valid in both old and new type, no change needed.
- **Fix:** No action required — fixture already uses `"active"` which is in the new 10-variant list.

## Self-Check

### Created files exist:
- `packages/shared-types/src/metrics.ts` — FOUND
- `packages/contract-tests/src/ws-dashboard.contract.test.ts` — FOUND
- `packages/contract-tests/src/fixtures/ws-dashboard.json` — FOUND
- `scripts/check-billing-status-parity.js` — FOUND

### Commits exist:
- `70750e4e` — FOUND (Task 1)
- `20b3229a` — FOUND (Task 2)

## Self-Check: PASSED
