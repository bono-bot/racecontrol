---
phase: 173-api-contracts
plan: "03"
subsystem: api-contracts
tags: [typescript, shared-types, openapi, swagger, admin, type-safety]
dependency_graph:
  requires: [173-02]
  provides: [admin-shared-types-wiring, openapi-spec, swagger-ui]
  affects: [racingpoint-admin, web-dashboard]
tech_stack:
  added: ["OpenAPI 3.0 spec (hand-crafted YAML)", "Swagger UI CDN (unpkg swagger-ui-dist@5)"]
  patterns: ["Cross-repo TypeScript path alias", "extends pattern for admin-specific type extension"]
key_files:
  created:
    - docs/openapi.yaml
    - web/public/api-docs/index.html
    - web/public/api-docs/openapi.yaml
  modified:
    - racingpoint-admin/tsconfig.json
    - racingpoint-admin/src/lib/api/fleet.ts
    - racingpoint-admin/src/lib/api/billing.ts
    - racingpoint-admin/src/app/(dashboard)/fleet/page.tsx
decisions:
  - "ActiveSession.pod_number kept as admin-specific extra field (not in shared BillingSession)"
  - "ActiveSession.started_at redeclared as required string (overrides optional from shared BillingSession)"
  - "fleet/page.tsx formatUptime widened to number|null|undefined (shared type uses optional, same fix as kiosk)"
  - "openapi.yaml placed in docs/ (canonical) and copied to web/public/api-docs/ (static serving)"
metrics:
  duration_seconds: 480
  completed_date: "2026-03-23T11:45:00+05:30"
  tasks_completed: 2
  files_created: 3
  files_modified: 4
requirements_met: [CONT-03, CONT-04]
---

# Phase 173 Plan 03: Admin Type Wiring + OpenAPI Spec Summary

**One-liner:** Admin tsconfig wired to `@racingpoint/types` path alias (compile-time type safety CONT-03), hand-crafted OpenAPI 3.0 spec with 66 operations and Swagger UI served at `:3200/api-docs/` (CONT-04).

## Tasks Completed

| # | Task | Commit | Files |
|---|------|--------|-------|
| 1 | Wire admin to shared types package | f587971 | racingpoint-admin tsconfig.json, fleet.ts, billing.ts, fleet/page.tsx |
| 2 | Write openapi.yaml and Swagger UI page | 278b35ce | docs/openapi.yaml, web/public/api-docs/index.html, web/public/api-docs/openapi.yaml |

## What Was Built

### Task 1: Admin Shared Types Wiring

**racingpoint-admin/tsconfig.json** — added `@racingpoint/types` path alias:
```json
"@racingpoint/types": ["../racecontrol/packages/shared-types/src/index.ts"]
```
Points cross-repo to `packages/shared-types/src/index.ts`. No npm install needed — path alias resolved at compile time by TypeScript bundler.

**racingpoint-admin/src/lib/api/fleet.ts** — replaced inline `PodFleetStatus` and `FleetHealthResponse` interface definitions with:
```typescript
import type { PodFleetStatus, FleetHealthResponse } from '@racingpoint/types';
export type { PodFleetStatus, FleetHealthResponse };
```
Admin-specific types (`DeployStatus`, `ExecResult`, `ActivityEntry`) remain inline.

**racingpoint-admin/src/lib/api/billing.ts** — `ActiveSession` now extends shared `BillingSession`:
```typescript
import type { BillingSession, BillingSessionStatus } from '@racingpoint/types';
export interface ActiveSession extends BillingSession {
  pod_number: number;
  price_paise: number;
  started_at: string;   // required override of optional in base
  paused_at: string | null;
  ended_at: string | null;
  staff_id: string | null;
  staff_name: string | null;
}
```
Shared base fields (id, driver_id, driver_name, pod_id, pricing_tier_name, allocated_seconds, driving_seconds, remaining_seconds, status, driving_state, split_count, etc.) are inherited.

`npx tsc --noEmit` exits 0 in racingpoint-admin/. CONT-03 met.

### Task 2: OpenAPI 3.0 Spec + Swagger UI

**docs/openapi.yaml** — complete OpenAPI 3.0.3 spec:
- 66 endpoint operations across 13 tags (health, auth, pods, billing, drivers, pricing, kiosk, games, public, customer, wallet, deploy, staff)
- All key component schemas: `PodFleetStatus` (17 fields), `FleetHealthResponse`, `BillingSession` (16 fields), `PricingTier`, `Driver`, `Pod`, `SimType`, `PodStatus`, `DrivingState`, `GameState`, `BillingSessionStatus`, `OkResponse`, `ErrorResponse`
- Two security schemes: `staffJWT` (Bearer) and `customerJWT` (Bearer)
- Derived from docs/API-BOUNDARIES.md as source of truth

**web/public/api-docs/index.html** — Swagger UI:
- CDN-loaded from unpkg.com/swagger-ui-dist@5 (no build step, no npm install)
- Loads `/api-docs/openapi.yaml` via SwaggerUIBundle
- Racing Point branding: red `#e10600` topbar label
- `filter: true`, `displayRequestDuration: true`, `tryItOutEnabled: false`

**web/public/api-docs/openapi.yaml** — identical copy served as static asset at `:3200/api-docs/openapi.yaml` by Next.js static file server.

Accessible at: `http://192.168.31.23:3200/api-docs/` when web dashboard is running.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] formatUptime signature too narrow in admin fleet page**
- **Found during:** Task 1 (tsc error: `number | undefined` not assignable to `number | null`)
- **Issue:** `racingpoint-admin/src/app/(dashboard)/fleet/page.tsx` had `formatUptime(secs: number | null)`. After switching to shared `PodFleetStatus`, `uptime_secs` became `number | undefined` (optional field) — same issue as kiosk in Plan 02.
- **Fix:** Changed signature to `number | null | undefined`, null check `secs === null || secs === undefined` already present
- **Files modified:** racingpoint-admin/src/app/(dashboard)/fleet/page.tsx
- **Commit:** f587971

## Success Criteria Verification

1. racingpoint-admin tsconfig resolves @racingpoint/types to shared-types — PASS
2. admin fleet.ts imports PodFleetStatus from @racingpoint/types (not inline) — PASS
3. admin billing.ts has ActiveSession extending BillingSession from @racingpoint/types — PASS
4. tsc --noEmit passes in racingpoint-admin/ (CONT-03) — PASS (exit 0)
5. docs/openapi.yaml is valid OpenAPI 3.0 with >= 20 endpoints and key schemas (CONT-04) — PASS (66 ops)
6. web/public/api-docs/index.html serves Swagger UI pointing to openapi.yaml — PASS

## Self-Check: PASSED
