---
status: partial
phase: 347-admin-staff-management
source: [347-VERIFICATION.md]
started: 2026-04-10T18:30:00Z
updated: 2026-04-10T18:30:00Z
---

## Current Test

[awaiting human testing — blocked on Phase 343 live deploy + admin rebuild]

## Tests

### 1. Change PIN UI with flag on
expected: Rebuild admin with NEXT_PUBLIC_FEATURE_STAFF_PIN_UI=on. Open /admin/staff, confirm no PIN values in table, click Change PIN, enter 4-digit PIN, watch all 4 stepper stages go green, confirm toast appears.
result: [pending]

### 2. Kiosk propagation within 10 seconds
expected: After green success on Change PIN, try old PIN on a pod kiosk — rejected. Try new PIN — accepted within 10 seconds.
result: [pending]

### 3. Flag-off fallback (D-16 regression)
expected: With NEXT_PUBLIC_FEATURE_STAFF_PIN_UI absent or off, confirm PIN toggle column still visible and inline staffApi.update PIN save still works.
result: [pending]

### 4. DEP-02 Node version check
expected: Run node --version on server .23 and confirm v22.x. Currently no code artifact (.nvmrc or engines field) confirms this.
result: [pending]

## Summary

total: 4
passed: 0
issues: 0
pending: 4
skipped: 0
blocked: 0

## Gaps
