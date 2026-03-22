---
phase: 173-api-contracts
plan: "02"
subsystem: shared-types
tags: [typescript, shared-types, kiosk, type-safety]
dependency_graph:
  requires: []
  provides: [packages/shared-types, kiosk-type-alias]
  affects: [kiosk, racingpoint-admin]
tech_stack:
  added: ["@racingpoint/types package (private, path-alias only)"]
  patterns: ["TypeScript path alias for monorepo package sharing"]
key_files:
  created:
    - packages/shared-types/package.json
    - packages/shared-types/tsconfig.json
    - packages/shared-types/src/pod.ts
    - packages/shared-types/src/billing.ts
    - packages/shared-types/src/driver.ts
    - packages/shared-types/src/fleet.ts
    - packages/shared-types/src/index.ts
  modified:
    - kiosk/tsconfig.json
    - kiosk/src/lib/types.ts
    - kiosk/src/app/fleet/page.tsx
decisions:
  - "PricingTier in shared includes is_trial/is_active/sort_order (Rust struct has them; plan spec was incomplete)"
  - "Driver.created_at made optional (kiosk creates partial frontend-only Driver objects)"
  - "Driver.has_used_trial added to shared type (kiosk API computed field used in SetupWizard)"
  - "formatUptime accepts number|null|undefined (shared uses optional, kiosk had null)"
  - "KioskDriver extension type dropped in favour of adding has_used_trial directly to shared Driver"
metrics:
  duration_seconds: 314
  completed_date: "2026-03-23T03:18:30+05:30"
  tasks_completed: 2
  files_created: 7
  files_modified: 3
requirements_met: [CONT-02]
---

# Phase 173 Plan 02: Shared TypeScript Types Package Summary

**One-liner:** `packages/shared-types` package with 4 domain type files (pod/billing/driver/fleet), wired to kiosk via `@racingpoint/types` path alias — type mismatch is now a compile error.

## Tasks Completed

| # | Task | Commit | Files |
|---|------|--------|-------|
| 1 | Create packages/shared-types/ package | 566151e3 | 7 new files in packages/shared-types/ |
| 2 | Wire kiosk tsconfig and update types.ts imports | 3d2e4ab6 | kiosk/tsconfig.json, kiosk/src/lib/types.ts, fleet/page.tsx |

## What Was Built

### packages/shared-types/

A standalone TypeScript package (`@racingpoint/types`) containing 4 domain type files derived from Rust serde structs:

- **src/pod.ts** — `SimType` (8-value union), `PodStatus`, `DrivingState`, `GameState`, `Pod` (maps to Rust PodInfo)
- **src/billing.ts** — `BillingSessionStatus`, `BillingSession`, `PricingTier` (maps to Rust BillingSessionInfo/PricingTier)
- **src/driver.ts** — `Driver` (maps to Rust Driver struct)
- **src/fleet.ts** — `PodFleetStatus`, `FleetHealthResponse` (maps to Rust PodFleetStatus in fleet_health.rs)
- **src/index.ts** — re-exports all 11 types from the 4 domain files

No `any` anywhere. All types derived directly from Rust serde snake_case structs.

### Kiosk Wiring

- `kiosk/tsconfig.json` — added `"@racingpoint/types": ["../packages/shared-types/src/index.ts"]` path alias
- `kiosk/src/lib/types.ts` — replaced inline definitions of Pod, BillingSession, Driver, PodFleetStatus (and related enums) with re-exports from `@racingpoint/types`. Kiosk-specific types (TelemetryFrame, Lap, KioskExperience, BillingStatus, AuthTokenInfo, etc.) remain inline.
- `npx tsc --noEmit` passes with zero errors in kiosk/.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] PricingTier missing Rust struct fields**
- **Found during:** Task 2 (tsc errors on is_trial, is_active, sort_order access)
- **Issue:** Plan spec for shared PricingTier listed only id/name/duration_minutes/price_paise. Rust struct also has is_trial/is_active, and kiosk API returns sort_order. Without these, tsc would error on all PricingTier usages.
- **Fix:** Added `is_trial: boolean`, `is_active: boolean`, `sort_order?: number` to shared PricingTier
- **Files modified:** packages/shared-types/src/billing.ts
- **Commit:** 3d2e4ab6

**2. [Rule 1 - Bug] Driver missing kiosk API fields**
- **Found during:** Task 2 (tsc errors: created_at required, has_used_trial missing)
- **Issue:** Kiosk creates partial Driver objects without `created_at` (frontend-only). Kiosk SetupWizard uses `has_used_trial` computed field returned by API.
- **Fix:** Made `created_at` optional; added optional `has_used_trial` to shared Driver
- **Files modified:** packages/shared-types/src/driver.ts
- **Commit:** 3d2e4ab6

**3. [Rule 1 - Bug] formatUptime type mismatch**
- **Found during:** Task 2 (tsc error: `number | undefined` not assignable to `number | null`)
- **Issue:** Kiosk's `formatUptime(secs: number | null)` vs shared `uptime_secs?: number` (undefined)
- **Fix:** Changed signature to `number | null | undefined`, null check to `== null`
- **Files modified:** kiosk/src/app/fleet/page.tsx
- **Commit:** 3d2e4ab6

## Success Criteria Verification

1. `packages/shared-types/src/` contains pod.ts, billing.ts, driver.ts, fleet.ts, index.ts — PASS
2. All 4 shared shapes defined without `any` — PASS
3. Kiosk tsconfig has `@racingpoint/types` path alias — PASS
4. Kiosk types.ts re-exports shared types instead of defining them inline — PASS
5. `npx tsc --noEmit` in kiosk/ passes with zero errors — PASS (CONT-02 met)

## Self-Check: PASSED

All 5 created files found on disk. Both task commits (566151e3, 3d2e4ab6) verified in git log.
