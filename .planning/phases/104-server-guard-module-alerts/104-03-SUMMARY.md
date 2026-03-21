---
phase: 104-server-guard-module-alerts
plan: 03
subsystem: kiosk-ui
tags: [typescript, nextjs, fleet-grid, violation-badge, kiosk]

# Dependency graph
requires:
  - phase: 104-01
    provides: violation_count_24h and last_violation_at fields on fleet/health API response per pod

provides:
  - PodFleetStatus interface in kiosk/src/lib/types.ts with violation_count_24h: number and last_violation_at: string | null
  - Violation badge on kiosk fleet grid pod card when violation_count_24h > 0, Racing Red (#E10600)

affects:
  - kiosk fleet grid UX — staff can see active violations without navigating to logs

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Null-safe badge guard: (pod.violation_count_24h ?? 0) > 0 — handles old binary not yet sending this field"
    - "Inline style backgroundColor: '#E10600' — consistent with existing Maintenance button pattern"
    - "Singular/plural: violation_count_24h === 1 ? 'violation' : 'violations'"

key-files:
  created: []
  modified:
    - kiosk/src/lib/types.ts
    - kiosk/src/app/fleet/page.tsx

key-decisions:
  - "Null-safety via ?? 0 operator — old agents not yet sending violation fields default to no badge rather than TypeScript error"
  - "inline style not Tailwind bg-red-600 — consistent with existing #E10600 usage in Maintenance button (brand color purity)"

requirements-completed: [ALERT-02]

# Metrics
duration: 2min
completed: 2026-03-21
---

# Phase 104 Plan 03: Kiosk Fleet Grid Violation Badge Summary

**PodFleetStatus TypeScript type extended with violation fields; kiosk fleet grid pod cards show a Racing Red (#E10600) violation badge when violation_count_24h > 0.**

## Performance

- **Duration:** 2 min
- **Started:** 2026-03-21T11:09:54Z (IST: 16:39)
- **Completed:** 2026-03-21T11:12:00Z (IST: 16:42)
- **Tasks:** 1 of 1 (checkpoint pending visual verification)
- **Files modified:** 2

## Accomplishments

- `PodFleetStatus` interface gains `violation_count_24h: number` and `last_violation_at: string | null` (after `last_http_check`)
- Violation badge renders on pod cards in fleet grid when `violation_count_24h > 0`
- Badge uses Racing Red `#E10600` inline style (consistent with Maintenance button pattern)
- Badge text: "1 violation" / "N violations" (singular/plural correct)
- Null-safe: `(pod.violation_count_24h ?? 0)` handles pods still running old binary without this field
- Badge positioned below Uptime row, above Crash recovered warning
- TypeScript compiles with zero errors (`npx tsc --noEmit`)

## Task Commits

1. **Task 1: Extend PodFleetStatus + violation badge on fleet page** — `9506d1d` (feat)

## Files Created/Modified

- `kiosk/src/lib/types.ts` — `PodFleetStatus` interface gains `violation_count_24h: number` and `last_violation_at: string | null`
- `kiosk/src/app/fleet/page.tsx` — Violation badge added between Uptime row and crash_recovery conditional

## Decisions Made

- Null-safety via `?? 0` operator — old agents not yet sending violation fields default to no badge rather than TypeScript runtime error
- `inline style` not Tailwind `bg-red-600` — maintains brand color purity (#E10600) consistent with existing Maintenance button

## Deviations from Plan

None — plan executed exactly as written.

## Issues Encountered

None. TypeScript compiled cleanly on first attempt.

## User Setup Required

Visual verification: run kiosk (`npm run dev` in kiosk/) and inspect fleet grid at `/fleet`. Badge renders in Racing Red with violation count.

## Next Phase Readiness

- Kiosk fleet grid now surfaces violation data from the fleet/health API without requiring navigation to logs
- Staff can see at a glance which pods have had process guard violations in the last 24 hours
- Ready for Phase 105 (Port Scan Audit)

---

## Self-Check

- `kiosk/src/lib/types.ts` modified: FOUND (violation_count_24h and last_violation_at present at lines 388-389)
- `kiosk/src/app/fleet/page.tsx` modified: FOUND (badge conditional at line 142, #E10600 at line 145)
- Commit `9506d1d`: FOUND
- TypeScript: zero errors confirmed

## Self-Check: PASSED

---
*Phase: 104-server-guard-module-alerts*
*Completed: 2026-03-21*
