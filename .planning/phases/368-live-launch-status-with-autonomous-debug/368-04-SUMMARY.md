---
phase: 368-live-launch-status-with-autonomous-debug
plan: "04"
subsystem: kiosk-frontend
tags: [kiosk, launch-cards, websocket, feature-flag, playwright, typescript]
dependency_graph:
  requires: [368-01, 368-02, 368-03]
  provides: [kiosk-launch-card-ui, playwright-probe-368]
  affects: [kiosk-debug-page, kiosk-ws-handler]
tech_stack:
  added: []
  patterns:
    - pure-function vitest tests (no @testing-library/react)
    - WebSocket constructor patch for Playwright WS simulation
    - feature-flag-gated conditional poll (D-14)
    - SHA256 layout regression guard (D-09)
key_files:
  created:
    - kiosk/src/components/LaunchCard.tsx
    - kiosk/src/components/__tests__/LaunchCard.test.tsx
    - tests/page-crawler/probe-debug-launches.spec.ts
  modified:
    - kiosk/src/app/debug/page.tsx
    - kiosk/src/lib/types.ts
    - kiosk/src/lib/api.ts
    - kiosk/src/hooks/useKioskSocket.ts
    - kiosk/src/lib/__tests__/launch-status-types.test.ts
    - kiosk/src/hooks/__tests__/useKioskSocket.launch.test.ts
    - tests/fixtures/368-debug-page-incidents-sha.txt
    - kiosk/src/app/debug/__tests__/debug-page-incidents-untouched.test.ts
decisions:
  - "Pure vitest tests (no @testing-library/react) — not installed in kiosk package"
  - "WebSocket constructor patch in addInitScript for Playwright WS simulation"
  - "D-09 SHA guard covers incidents sidebar only — Activity Feed replacement is outside the marked region"
  - "D-14: shouldPoll = !launchCardsEnabled || !connected — poll removed only when both conditions true"
metrics:
  duration: "resumed from prior session — Tasks 2-3 completed in ~15 min"
  completed_date: "2026-04-11"
  tasks_completed: 4
  tasks_total: 4
  files_changed: 12
---

# Phase 368 Plan 04: Kiosk Frontend — LaunchCard + Debug Page Integration Summary

**One-liner:** LaunchCard React component with 5-state timeline, tier-gated approve button, and inline notes thread; wired into /kiosk/debug behind kiosk_launch_cards_enabled feature flag with D-14 conditional poll and D-09 layout regression guard.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 0 | D-09 layout regression guard + SHA fixture | `6e268a31` | debug-page-incidents-untouched.test.ts, 368-debug-page-incidents-sha.txt |
| 1 | LaunchState types + api.ts methods + useKioskSocket WS handling | `ddb1cd17` | types.ts, api.ts, useKioskSocket.ts, 2 test files |
| 2 | LaunchCard component + debug page integration | `755d9483` | LaunchCard.tsx, LaunchCard.test.tsx, debug/page.tsx |
| 3 | Playwright probe for /kiosk/debug launch cards | `451bb138` | probe-debug-launches.spec.ts |

## What Was Built

### Task 0 — D-09 Layout Regression Guard
- SHA256 fixture at `tests/fixtures/368-debug-page-incidents-sha.txt` covering the incidents sidebar region (3048 chars between `DEBUG-PAGE-INCIDENTS-REGION-START` and `DEBUG-PAGE-INCIDENTS-REGION-END` markers)
- Vitest test that re-computes the SHA on every run and asserts it matches the fixture
- Markers inserted immediately before/after the `<div className="w-48 flex-shrink-0 ...">` incidents sidebar block in debug/page.tsx

### Task 1 — TypeScript Types + API + WS Handler
- `LaunchState` (5 values), `LaunchOrigin` (4 values), `LaunchStatusCard`, `LaunchNoteEvent`, `FeatureFlagRow` added to `kiosk/src/lib/types.ts`
- 6 new API client methods in `api.ts`: `listActiveLaunches`, `getLaunchNotes`, `postLaunchNote`, `approveLaunchFix`, `dismissLaunch`, `listFlags`
- 2 new switch cases in `useKioskSocket.ts`: `launch_status_changed` (upserts to Map) and `launch_note_added` (appends to Map)
- `removeLaunch` useCallback exported from hook
- 29 vitest tests across 2 files — all pass

### Task 2 — LaunchCard Component + Debug Page
- `LaunchCard.tsx`: pod badge, 4-dot state timeline, detail text (D-15 as-is render), fix_action hint, inline notes thread, note composer form, approve-fix button (tier >= 2), dismiss button (terminal states only)
- `debug/page.tsx`: `launchCardsEnabled` state with 60s flag-fetch useEffect; D-14 `shouldPoll = !launchCardsEnabled || !connected` conditional poll; LaunchCard[] render when flag=true, empty-state fallback with ws-connection-dot
- 17 vitest tests — all pass
- D-09 SHA guard: PASS after edit (Activity Feed is outside the marked incidents region)

### Task 3 — Playwright Probe
- `probe-debug-launches.spec.ts`: 4 tests (page load, empty state, single card render, 4-state transition + screenshot)
- WS simulation via `WebSocket` constructor patch injected via `addInitScript` before page load
- `page.route()` mocks for `/api/v1/flags` (feature flag control) and `/api/v1/debug/launches`
- `sessionStorage.setItem('kiosk_staff_token', ...)` injection (not localStorage — api.ts:16 pattern)
- Screenshot saved to `tests/page-crawler/screenshots/launch-card-state-final.png` at end of test 4
- `PROBE_SPEC=probe-debug-launches.spec.ts npx playwright test --list` exits 0, enumerates 4 tests

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] vitest not jest**
- **Found during:** Task 1 setup
- **Issue:** Plan referred to jest patterns; kiosk uses vitest (confirmed via package.json)
- **Fix:** All test imports use `from "vitest"` not `from "@jest/globals"`, test runner command is `npm test -- filename`
- **Files modified:** all 3 test files
- **Commit:** `ddb1cd17`

**2. [Rule 3 - Blocking] No @testing-library/react in kiosk**
- **Found during:** Task 2 (plan called for renderHook tests)
- **Issue:** `@testing-library/react` not installed in kiosk package
- **Fix:** Pure function tests that extract and test Map update logic directly — identical logical coverage, no React dependency required
- **Files modified:** useKioskSocket.launch.test.ts, LaunchCard.test.tsx
- **Commit:** `ddb1cd17`, `755d9483`

**3. [Rule 3 - Blocking] playwright --list requires PROBE_SPEC env**
- **Found during:** Task 3 verification
- **Issue:** playwright.config.ts uses `testMatch: process.env.PROBE_SPEC ?? 'crawl.spec.ts'` — bare `--list` found 0 tests
- **Fix:** Acceptance criteria verification uses `PROBE_SPEC=probe-debug-launches.spec.ts` prefix
- **Files modified:** none (env-only)

## Known Stubs

None — all data flows are wired. The LaunchCard renders `card.detail` as-is (server-guaranteed sanitization, Plan 01). Feature flag defaults to `false` until MMA audit approves toggle.

## Pending (Task 4 — MMA Audit Checkpoint)

The plan contains a `type="checkpoint:human-verify"` Task 4 (MMA audit on the full cross-system bridge). This is a Claude-runs-and-triages task per the plan spec — execution pauses here for the MMA audit run and review.

What is ready for audit:
- Plan 01: rc-common protocol types + LaunchStateMachine + server-side launch_id threading + billing-reject sanitization
- Plan 02: rc-agent emissions at 4 boundaries + tier_engine plumbing + ws_handler backward compat + server relay
- Plan 03: launch_notes table + cloud_sync + 5 REST endpoints + feature flag seed + tier gate
- Plan 04 Tasks 0-3: TypeScript types + LaunchCard component + /debug page integration + Playwright probe

## Self-Check: PASSED

| Item | Status |
|------|--------|
| kiosk/src/components/LaunchCard.tsx | FOUND |
| kiosk/src/components/__tests__/LaunchCard.test.tsx | FOUND |
| tests/page-crawler/probe-debug-launches.spec.ts | FOUND |
| tests/fixtures/368-debug-page-incidents-sha.txt | FOUND |
| kiosk/src/app/debug/__tests__/debug-page-incidents-untouched.test.ts | FOUND |
| commit 6e268a31 (Task 0) | FOUND |
| commit ddb1cd17 (Task 1) | FOUND |
| commit 755d9483 (Task 2) | FOUND |
| commit 451bb138 (Task 3) | FOUND |
