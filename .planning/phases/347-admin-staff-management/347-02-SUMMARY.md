---
phase: 347-admin-staff-management
plan: "02"
subsystem: admin-frontend
tags: [staff, pin-management, feature-flag, modal, admin-dashboard]
dependency_graph:
  requires: [347-01]
  provides: [STAFF-01, STAFF-03, STAFF-04, STAFF-08, STAFF-09, STAFF-10, DEP-01, DEP-02]
  affects: [racingpoint-admin, staff-manage-page]
tech_stack:
  added: []
  patterns: [feature-flag-conditional-render, staged-progress-stepper, partial-success-error-banner]
key_files:
  created: []
  modified:
    - racingpoint-admin/src/lib/api/staff.ts
    - racingpoint-admin/src/app/(dashboard)/staff/manage/page.tsx
decisions:
  - "Feature flag NEXT_PUBLIC_FEATURE_STAFF_PIN_UI=on activates modal path; off preserves legacy inline PIN"
  - "React hooks declared unconditionally (visiblePins/togglePin preserved) per React rules; JSX rendering is conditional"
  - "Pin field excluded from edit form when flag ON — Change PIN modal is the only PIN change path"
  - "staffApi.update PIN path retained for flag-OFF fallback (D-16)"
metrics:
  duration: "~15 minutes"
  completed: "2026-04-10"
  tasks_completed: 2
  tasks_total: 2
  files_modified: 2
---

# Phase 347 Plan 02: Staff PIN UI — Change PIN Modal Summary

## One-liner

Feature-flagged Change PIN modal with staged 4-step progress stepper and partial-success error banner, replacing inline PIN display when `NEXT_PUBLIC_FEATURE_STAFF_PIN_UI=on`.

## What Was Built

### Task 1 — staffApi.changePin + typed interfaces (commit `481c826`)

Added to `racingpoint-admin/src/lib/api/staff.ts`:
- `ChangePinParams` interface: `{ new_pin: string }`
- `ChangePinResponse` interface: `{ status, cloud_verified, venue_verified, latency_ms, correlation_id }`
- `staffApi.changePin(id, data)` — POSTs to `/admin/staff/{id}/change-pin` via catch-all proxy, returns `Promise<ChangePinResponse>`
- No `any` types. Existing `StaffMember.pin` field unchanged (list endpoint still returns it).

### Task 2 — Enhanced manage/page.tsx (commit `980902b`)

Added to `racingpoint-admin/src/app/(dashboard)/staff/manage/page.tsx`:

**Feature flag constant:**
```typescript
const STAFF_PIN_UI_ENABLED = process.env.NEXT_PUBLIC_FEATURE_STAFF_PIN_UI === 'on';
```

**When flag ON:**
- PIN column hidden from table header (`!STAFF_PIN_UI_ENABLED` guard)
- PIN toggle cell hidden from view-mode rows
- PIN field hidden from edit-mode rows; `editForm.pin` not populated from `s.pin`
- `staffApi.update` PIN path skipped (only name/phone/role changes go through update)
- "Change PIN" button per active staff row triggers modal
- Change PIN modal with dual password inputs, 4+ numeric digit validation, matching confirm check

**When flag OFF (D-16 fallback — behavior identical to pre-Phase-347):**
- `visiblePins` state and `togglePin` function preserved (declared unconditionally per React rules)
- PIN column, PIN toggle cell, PIN edit field all rendered
- `staffApi.update({pin})` path still active

**Change PIN modal features:**
- Staged progress stepper: Writing cloud → Syncing venue → Verifying cloud → Verifying venue (4 steps)
- Timeout-based step advancement during API call (600ms/1400ms/2200ms intervals)
- All-done checkmarks on `status: ok` + both verified
- Error banner on partial success: `"PIN changed on cloud but X failed - contact James (ref: {correlation_id})"`
- Cancel button closes modal; submit disabled until `isPinValid` (4+ digits, numeric, matching)

**Preserved unchanged regardless of flag (D-05):**
- Create form with PIN field (always required for new staff)
- Edit name/phone/role via `staffApi.update`
- Deactivate/reactivate flows

## Verification Results

All plan acceptance criteria passed via grep:

| Check | Result |
|-------|--------|
| `STAFF_PIN_UI_ENABLED` matches (7 in page.tsx) | PASS |
| `NEXT_PUBLIC_FEATURE_STAFF_PIN_UI` (1 match) | PASS |
| `visiblePins` preserved (5 matches) | PASS |
| `togglePin` preserved (3 matches) | PASS |
| `!STAFF_PIN_UI_ENABLED` guards (PIN column + cell + edit field) | PASS |
| `staffApi.update` retained (4 matches) | PASS |
| `changePinTarget` (state + button + modal) | PASS |
| `staffApi.changePin` (1 match in handler) | PASS |
| `contact James` (partial success error) | PASS |
| `Writing cloud` stepper label | PASS |
| `Verifying venue` stepper label | PASS |
| `pinSubmitting` (6 matches) | PASS |
| `any` type count | 0 |
| `ChangePinResponse` in staff.ts (interface + Promise cast) | PASS |
| `ChangePinParams` in staff.ts (interface + param type) | PASS |

## Commits

| Repo | Hash | Message |
|------|------|---------|
| racingpoint-admin | `481c826` | feat(347-02): add changePin method and ChangePinResponse type to staffApi |
| racingpoint-admin | `980902b` | feat(347-02): enhance staff manage page with Change PIN modal and feature flag |

Pushed to `github.com:bono-bot/racingpoint-admin.git` main branch.

## Deviations from Plan

None — plan executed exactly as written.

## Known Stubs

None. The modal calls `staffApi.changePin` which hits the real `/admin/staff/{id}/change-pin` endpoint implemented in Plan 347-01. No hardcoded responses or mock data.

## Deploy Notes

Per plan `deploy:` section:
- `frontend_rebuild: [admin]` — admin dashboard must be rebuilt with `NEXT_PUBLIC_FEATURE_STAFF_PIN_UI=on` in `.env.production.local` to activate the Change PIN UI
- `cloud_parity: [frontend]` — same rebuild required on cloud (Bono VPS)
- `targets: [server, cloud]`
- No Rust binary changes, no DB migrations, no config changes

The feature flag defaults to OFF (env var not set = legacy PIN display), making this deploy safe without a rebuild.

## Self-Check: PASSED

- `racingpoint-admin/src/lib/api/staff.ts` — modified, commit `481c826` exists
- `racingpoint-admin/src/app/(dashboard)/staff/manage/page.tsx` — modified, commit `980902b` exists
- Both commits pushed to remote `04cf340..980902b`
