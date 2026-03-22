---
phase: 173-api-contracts
plan: "04"
subsystem: contract-tests
tags: [contract-tests, vitest, github-actions, ci, typescript]
dependency_graph:
  requires: [173-01, 173-02, 173-03]
  provides: [contract-test-suite, ci-workflow]
  affects: [packages/contract-tests, .github/workflows]
tech_stack:
  added: [vitest@2.1.0]
  patterns: [fixture-based-contract-testing, type-assertion-guards]
key_files:
  created:
    - packages/contract-tests/package.json
    - packages/contract-tests/tsconfig.json
    - packages/contract-tests/vitest.config.ts
    - packages/contract-tests/src/fixtures/fleet-health.json
    - packages/contract-tests/src/fixtures/billing-active.json
    - packages/contract-tests/src/fixtures/pods.json
    - packages/contract-tests/src/fleet.contract.test.ts
    - packages/contract-tests/src/billing.contract.test.ts
    - packages/contract-tests/src/pods.contract.test.ts
    - .github/workflows/contract-tests.yml
  modified: []
decisions:
  - "Vitest chosen over Jest: native ESM support, no transform config, faster cold start"
  - "assertX(data: unknown): asserts data is T pattern used — catches both runtime fixture drift and compile-time type drift"
  - "Fixture IPs use 192.168.31.0 (not a real pod IP) and TEST_ONLY_ prefixed IDs per standing rules"
  - "package-lock.json committed so CI cache-dependency-path works correctly"
metrics:
  duration: "2 minutes"
  completed: "2026-03-23T09:30:56+05:30"
  tasks_completed: 2
  files_created: 10
requirements_satisfied: [CONT-05, CONT-06]
---

# Phase 173 Plan 04: Contract Tests & CI Workflow Summary

Fixture-based Vitest contract tests (11 tests / 3 suites) for fleet, billing, and pods API shapes, wired to GitHub Actions CI on every PR.

## What Was Built

### packages/contract-tests/

A standalone `@racingpoint/contract-tests` package using Vitest 2.1. Tests run against local JSON fixture files — no live server required. Types are imported from `@racingpoint/types` so TypeScript compile-time enforcement catches shape drift.

**Test suites (11 tests total):**

| Suite | Tests | What it validates |
|-------|-------|-------------------|
| `fleet.contract.test.ts` | 3 | `PodFleetStatus` + `FleetHealthResponse` — all 8 required fields |
| `billing.contract.test.ts` | 4 | `BillingSession` — required fields + `BillingSessionStatus` enum + `DrivingState` |
| `pods.contract.test.ts` | 4 | `Pod` — required fields + `SimType` enum + `PodStatus` enum |

**Contract enforcement pattern:**
```typescript
function assertPodFleetStatus(data: unknown): asserts data is PodFleetStatus {
  const d = data as Record<string, unknown>;
  expect(typeof d.pod_number, 'pod_number must be number').toBe('number');
  // ... all required fields
}
```

If a required field is removed from a fixture, the assertion throws with the field name. If the TypeScript interface adds a required field without updating the test assertions, `tsc --noEmit` fails.

**Fixture data:** Uses `TEST_ONLY_` prefixed IDs and `192.168.31.0` (not a real pod IP) per standing rules.

### .github/workflows/contract-tests.yml

Triggers on `push` to main and `pull_request` to main. Runs Node.js 20. Installs `packages/shared-types` then `packages/contract-tests`, then executes `npm test`. Workflow failure blocks PR merge.

## Decisions Made

1. **Vitest over Jest** — native ESM module support, no transform config needed for `.ts` imports, 439ms total test run time.
2. **`assertX(data: unknown): asserts data is T` pattern** — runtime assertion guard doubles as type narrowing. Avoids `any` entirely (standing rule compliance).
3. **`TEST_ONLY_` fixture IDs** — per standing rules, no real-looking identifiers in test data.
4. **`package-lock.json` committed** — required for `cache-dependency-path` in the GitHub Actions `setup-node` step to work correctly.

## Deviations from Plan

None — plan executed exactly as written.

## Self-Check

Files verified:
- `packages/contract-tests/src/fleet.contract.test.ts` — FOUND
- `packages/contract-tests/src/billing.contract.test.ts` — FOUND
- `packages/contract-tests/src/pods.contract.test.ts` — FOUND
- `packages/contract-tests/src/fixtures/fleet-health.json` — FOUND
- `packages/contract-tests/src/fixtures/billing-active.json` — FOUND
- `packages/contract-tests/src/fixtures/pods.json` — FOUND
- `.github/workflows/contract-tests.yml` — FOUND

Commits verified:
- `34886a2d` — feat(173-04): create contract-tests package with Vitest fixture-based tests
- `cd1db67e` — feat(173-04): add GitHub Actions CI workflow for contract tests

`npm test` result: 3 passed / 11 tests / exit 0

## Self-Check: PASSED
