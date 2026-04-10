---
phase: 343-staff-pin-hardening
plan: 04
status: complete
commit: 4074bb0d
---

# 343-04 Summary: E2E Staff PIN Lifecycle Regression Spec

## What shipped

Playwright E2E spec at `e2e-regression/tests/10-auth/staff-pin-lifecycle.spec.ts` that codifies the exact Vishal 2026-04-09 failure mode into a permanent regression test.

## Test flow (7 tests, serial)

1. Create test staff with PIN 9999 on cloud
2. Validate PIN 9999 on cloud
3. Wait 35s for cloud→venue sync, validate PIN 9999 on venue
4. Change PIN 9999→8888 via PUT /staff/{id} on cloud (expects `verified:true` + `correlation_id` from 343-02)
5. Immediately (t+2s) validate PIN 8888 on both cloud + venue
6. **Wait 70s (2x sync cycle + safety)**, re-validate PIN 8888 — THIS catches silent revert
7. Verify old PIN 9999 rejected on both endpoints

afterAll: soft-delete test staff, verify deletion propagated.

## Adaptation from plan

Plan referenced `POST /admin/staff/{id}/change-pin` (343-03 superseded endpoint). Adapted to use existing `PUT /staff/{id}` with `{pin: "8888"}` which now returns `verified:true` from 343-02.

## Fixtures used

- `getAdminToken()` from `fixtures/auth.ts` (already existed)
- `API_BASE` from `fixtures/test-data.ts` as fallback
- Custom `apiPost/apiPut/apiDelete` helpers for dual-endpoint (cloud + venue) testing

## Runtime

~120s per run. Requires env vars: `CLOUD_API_URL`, `VENUE_API_URL`, `ADMIN_PIN`.

```bash
cd C:/Users/bono/racingpoint/racecontrol/e2e-regression
CLOUD_API_URL=http://100.70.177.44:8080/api/v1 \
VENUE_API_URL=http://192.168.31.23:8080/api/v1 \
ADMIN_PIN=8141 \
npx playwright test tests/10-auth/staff-pin-lifecycle.spec.ts
```

## Success criteria

- SC-8: spec exists, compiles, includes 70s sync-wait assertion ✅
- Creates/changes/waits/revalidates/cleans up throwaway staff record ✅
- Tests both cloud and venue endpoints ✅
- Uses existing fixture patterns ✅

## Deferred

- Live runtime execution (requires deployed 343-01 + 343-02 on both cloud + venue)
- Nightly CI integration
