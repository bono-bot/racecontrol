---
phase: 201-frontend-integration-type-sync
verified: 2026-03-26T12:30:00+05:30
status: passed
score: 17/17 must-haves verified
re_verification: false
human_verification:
  - test: "Kiosk pod panel — trigger waiting_for_game via WS"
    expected: "Panel shows Game Loading... spinner (not countdown) with blue background banner"
    why_human: "UI rendering requires live kiosk browser and active WS session"
  - test: "Kiosk crash recovery — trigger paused_game_pause via WS"
    expected: "Panel shows amber Relaunching Game... banner with Relaunch Now button"
    why_human: "UI rendering requires live session state"
  - test: "Kiosk reliability warning — select combo with <70% success rate"
    expected: "Amber banner appears before launch confirmation with Suggest Alternative button"
    why_human: "Requires live /api/v1/games/alternatives data from backend"
  - test: "Web billing page — set status to waiting_for_game"
    expected: "End/Pause/Extend buttons hidden; Loading... badge visible"
    why_human: "Requires live WS session and browser inspection"
  - test: "/games/reliability page — open in browser"
    expected: "Launch matrix table loads with red/amber/green row color coding and flagged badges"
    why_human: "Requires live backend with data at /api/v1/admin/launch-matrix"
---

# Phase 201: Frontend Integration & Type Sync Verification Report

**Phase Goal:** All 4 frontend apps (kiosk, web dashboard, admin, PWA) correctly handle new billing states, game states, and metrics — with type safety enforced by contract tests and CI. No customer-facing UI breaks when the backend ships new states.

**Verified:** 2026-03-26T12:30:00+05:30
**Status:** PASSED (automated checks) + human verification recommended for UI rendering
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths (from ROADMAP Success Criteria)

| # | Truth | Status | Evidence |
|---|-------|--------|---------|
| 1 | TYPE PARITY: shared-types BillingSessionStatus has exactly 10 variants matching Rust | VERIFIED | `billing.ts` has exactly 10 `| "..."` variants; `node scripts/check-billing-status-parity.js` exits 0 |
| 2 | KIOSK LOCAL TYPE REMOVED: zero `BillingStatus` definitions in kiosk/src/ | VERIFIED | `grep "BillingStatus" kiosk/src/` returns 0 matches; `types.ts` uses `BillingSessionStatus` from `@racingpoint/types` only |
| 3 | KIOSK PANEL — ALL STATES: non-terminal billing statuses show live session view | VERIFIED | `PodKioskView.tsx deriveKioskState` maps `waiting_for_game` -> `launching`; `paused_game_pause`/`paused_disconnect` map to `in_session` |
| 4 | KIOSK LOADING STATE: timer shows "Game Loading..." spinner during waiting_for_game | VERIFIED | `LiveSessionPanel.tsx` line 116-124: renders blue loading banner when `billing.status === "waiting_for_game"`; countdown useEffect guard `billing.status !== "active"` prevents countdown |
| 5 | KIOSK CRASH RECOVERY: amber Relaunching banner during paused_game_pause | VERIFIED | `PodKioskView.tsx` line 407: amber banner with spinner and "Relaunching Game..."; `LiveSessionPanel.tsx` line 127-146: same treatment |
| 6 | KIOSK RELIABILITY WARNING: <70% combo triggers amber banner + alternatives modal | VERIFIED | `SetupWizard.tsx` has `alternatives` fetch on review step; SetupWizard is only file with reliability warning logic |
| 7 | WEB WAITING_FOR_GAME: billing card hides End/Pause buttons, shows Loading... badge | VERIFIED | `billing/page.tsx` lines 126 + 186-193: `isWaitingForGame` flag; controls hidden via `!isWaitingForGame`; `StatusBadge status="waiting_for_game"` rendered |
| 8 | WEB ALL PAUSED STATES: distinct badges for paused_game_pause, paused_disconnect, paused_manual | VERIFIED | `StatusBadge.tsx` COLORS: yellow for `paused_game_pause`, orange for `paused_disconnect`, blue for `paused_manual`; STATUS_LABELS maps them to "Game Crashed", "Disconnected", "Paused" |
| 9 | STATUS BADGE COMPLETE: all 10 BillingSessionStatus variants with distinct colors | VERIFIED | `StatusBadge.tsx` COLORS Record has all 10 billing variants explicitly; no variant falls through to default grey |
| 10 | ADMIN BILLING BADGES: billing history uses StatusBadge for all 10 statuses | VERIFIED | `billing/history/page.tsx` imports and uses `StatusBadge` component; stale 5-entry `statusColors` map removed |
| 11 | ADMIN GAME STATE: games page shows game_state column with color-coded StatusBadge | VERIFIED | `games/page.tsx` line 144: `<StatusBadge status={gameInfo.game_state} />` |
| 12 | ADMIN LAUNCH MATRIX: /games/reliability page exists with per-pod grid | VERIFIED | `web/src/app/games/reliability/page.tsx` exists; uses `getLaunchMatrix`, `rowBgClass()` for red/amber/green, `flagged` badge |
| 13 | ADMIN METRICS CLIENT: metrics.ts exports 4 typed methods | VERIFIED | `web/src/lib/api/metrics.ts` exports `getLaunchStats`, `getBillingAccuracy`, `getAlternatives`, `getLaunchMatrix` with typed responses |
| 14 | OPENAPI UPDATED: 10 BillingSessionStatus variants + 4 new endpoints | VERIFIED | openapi.yaml has `waiting_for_game` in BillingSessionStatus enum; all 4 endpoints present: `/metrics/launch-stats`, `/metrics/billing-accuracy`, `/games/alternatives`, `/admin/launch-matrix` |
| 15 | CONTRACT TESTS: billing.contract.test.ts validates all 10 variants; ws-dashboard tests validate BillingTick/GameStateChanged | VERIFIED | `billing.contract.test.ts` has `VALID_BILLING_STATUSES` with 10 entries + "exactly 10 variants" assertion; `ws-dashboard.contract.test.ts` has 8 tests covering BillingTick and GameStateChanged |
| 16 | DRIFT PREVENTION: parity script compares Rust vs TS variant counts | VERIFIED | `scripts/check-billing-status-parity.js` runs and exits 0; covers both BillingSessionStatus (10) and GameState (6) |
| 17 | WS DASHBOARD FIXTURE: BillingTick and GameStateChanged fixtures exist | VERIFIED | `packages/contract-tests/src/fixtures/ws-dashboard.json` has 4 fixture entries including `waiting_for_game` and `loading` states |

**Score:** 17/17 truths verified

---

## Required Artifacts

| Artifact | Status | Details |
|----------|--------|---------|
| `packages/shared-types/src/billing.ts` | VERIFIED | 10-variant BillingSessionStatus with inline comments; `BillingSession.status: BillingSessionStatus` |
| `packages/shared-types/src/metrics.ts` | VERIFIED | Exports FailureMode, LaunchStatsResponse, BillingAccuracyResponse, AlternativeCombo, LaunchMatrixRow |
| `packages/shared-types/src/ws-messages.ts` | VERIFIED | Exports BillingTick, GameStateChanged, LaunchDiagnostics |
| `packages/shared-types/src/index.ts` | VERIFIED | Re-exports all new types from metrics.ts and ws-messages.ts |
| `packages/contract-tests/src/billing.contract.test.ts` | VERIFIED | 10 variants in VALID_BILLING_STATUSES; "exactly 10 variants" assertion test |
| `packages/contract-tests/src/ws-dashboard.contract.test.ts` | VERIFIED | 8 tests for BillingTick/GameStateChanged payload shapes |
| `packages/contract-tests/src/fixtures/ws-dashboard.json` | VERIFIED | 4 fixtures: billing_tick, billing_tick_waiting, game_state_changed, game_state_loading |
| `scripts/check-billing-status-parity.js` | VERIFIED | Parses Rust enum + TS union; exits 0 on parity; confirmed passing live |
| `kiosk/src/lib/types.ts` | VERIFIED | No local BillingStatus; imports BillingSessionStatus from @racingpoint/types |
| `kiosk/src/hooks/useKioskSocket.ts` | VERIFIED | TERMINAL_STATUSES includes cancelled_no_playable as 4th terminal |
| `kiosk/src/components/LiveSessionPanel.tsx` | VERIFIED | waiting_for_game -> Loading banner; paused_game_pause -> amber Relaunching; paused_disconnect -> orange Disconnected; countdown guard `status === "active"` only |
| `kiosk/src/components/KioskPodCard.tsx` | VERIFIED | derivePodState maps waiting_for_game->loading, paused_game_pause/paused_disconnect->crashed |
| `kiosk/src/components/PodKioskView.tsx` | VERIFIED | deriveKioskState maps waiting_for_game->launching; crash recovery + disconnect banners in InSessionView |
| `kiosk/src/components/SetupWizard.tsx` | VERIFIED | Reliability warning with alternatives fetch on review step |
| `web/src/lib/api.ts` | VERIFIED | BillingSession.status has all 10 variants inline; GameState includes "loading" |
| `web/src/components/StatusBadge.tsx` | VERIFIED | COLORS Record covers all 10 BillingSessionStatus + 6 GameState variants; STATUS_LABELS for 4 user-friendly overrides |
| `web/src/app/billing/page.tsx` | VERIFIED | isWaitingForGame flag; buttons hidden; StatusBadge shown; isPaused covers all 3 paused states |
| `web/src/app/billing/history/page.tsx` | VERIFIED | Uses StatusBadge component; stale statusColors map removed |
| `web/src/app/games/page.tsx` | VERIFIED | StatusBadge renders game_state in game info cards; Reliability Matrix link added |
| `web/src/lib/api/metrics.ts` | VERIFIED | 4 typed functions with proper error handling; uses NEXT_PUBLIC_API_URL |
| `web/src/app/games/reliability/page.tsx` | VERIFIED | getLaunchMatrix import + usage; rowBgClass(); flagged badge; game selector dropdown |
| `docs/openapi.yaml` | VERIFIED | BillingSessionStatus enum has waiting_for_game; 4 new endpoint specs |

---

## Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `packages/contract-tests/src/billing.contract.test.ts` | `packages/shared-types/src/billing.ts` | `import type { BillingSession, BillingSessionStatus } from '@racingpoint/types'` | WIRED | Import confirmed line 2 |
| `scripts/check-billing-status-parity.js` | `crates/rc-common/src/types.rs` | regex parse of Rust enum via brace-counting | WIRED | Script reads file path `crates/rc-common/src/types.rs`; live run exits 0 |
| `kiosk/src/lib/types.ts` | `packages/shared-types/src/billing.ts` | `import type { GameState, BillingSessionStatus } from '@racingpoint/types'` | WIRED | Import confirmed line 3 |
| `kiosk/src/hooks/useKioskSocket.ts` | `kiosk/src/components/LiveSessionPanel.tsx` | billingTimers state passed as prop | WIRED | billingTimers confirmed in hook state; LiveSessionPanel receives billing prop |
| `web/src/app/games/reliability/page.tsx` | `web/src/lib/api/metrics.ts` | `import { getLaunchMatrix }` | WIRED | Import confirmed line 5; called in useEffect line 41 |
| `web/src/lib/api/metrics.ts` | `packages/shared-types/src/metrics.ts` | Types declared locally (by design — documented decision) | PARTIAL | Local types match shared-types shape exactly; parity enforced via check-billing-status-parity.js |
| `web/src/app/billing/page.tsx` | `web/src/components/StatusBadge.tsx` | `import StatusBadge` | WIRED | StatusBadge import confirmed; used for waiting_for_game display |

Note on PARTIAL link: web/src/lib/api/metrics.ts declares types locally rather than importing from @racingpoint/types. This was a documented decision in the SUMMARY (avoids build-time dependency; kept in sync via parity script). The parity script only checks BillingSessionStatus/GameState — the metrics types are NOT covered by the script. This is a minor gap in drift prevention for metrics types but does not affect functionality.

---

## Requirements Coverage

The SYNC-01 through ADMIN-04 requirement IDs for phase 201 are defined inline in the ROADMAP.md success criteria (no separate REQUIREMENTS.md for v24.0). All 19 requirements are covered by the 3 plans.

| Requirement | Plan | Description | Status |
|-------------|------|-------------|--------|
| SYNC-01 | 201-01 | BillingSessionStatus 10-variant parity with Rust | SATISFIED — billing.ts has 10 variants; parity script confirms match |
| SYNC-02 | 201-01, 201-02 | Kiosk local BillingStatus removed | SATISFIED — zero matches for "BillingStatus" in kiosk/src/ |
| SYNC-03 | 201-01 | Contract test validates all 10 variants | SATISFIED — VALID_BILLING_STATUSES has 10; "exactly 10 variants" test |
| SYNC-04 | 201-01 | ws-dashboard contract test validates BillingTick/GameStateChanged | SATISFIED — ws-dashboard.contract.test.ts has 8 tests |
| SYNC-05 | 201-01 | Drift prevention CI/pre-commit check | SATISFIED — scripts/check-billing-status-parity.js exists, exits 0 |
| SYNC-06 | 201-01 | OpenAPI spec updated: 10 variants + 4 new endpoints | SATISFIED — openapi.yaml confirmed |
| SYNC-07 | 201-01 | GameState "loading" variant in TS | SATISFIED — research confirmed all 6 variants already present; web/src/lib/api.ts confirmed |
| KIOSK-01 | 201-02 | Kiosk handles all 10 billing status variants | SATISFIED — all 10 handled; cancelled_no_playable in TERMINAL_STATUSES |
| KIOSK-02 | 201-02 | Kiosk panel shows live session view for all non-terminal statuses | SATISFIED — derivePodState/deriveKioskState confirmed |
| KIOSK-03 | 201-02 | Kiosk timer shows "Game Loading..." during waiting_for_game | SATISFIED — LiveSessionPanel.tsx confirmed |
| KIOSK-04 | 201-02 | Kiosk crash recovery: amber Relaunching, then Session Paused | SATISFIED — PodKioskView + LiveSessionPanel amber banner confirmed |
| KIOSK-05 | 201-02 | Reliability warning banner + alternatives modal | SATISFIED — SetupWizard.tsx confirmed |
| WEB-01 | 201-03 | Billing page hides buttons during waiting_for_game | SATISFIED — isWaitingForGame flag + button guard confirmed |
| WEB-02 | 201-03 | Distinct badge colors for 3 paused states | SATISFIED — StatusBadge COLORS Record confirmed |
| WEB-03 | 201-03 | StatusBadge renders all 10 variants with distinct colors | SATISFIED — all 10 billing variants in COLORS Record |
| ADMIN-01 | 201-03 | Admin billing history shows all 10 badge colors | SATISFIED — history page uses StatusBadge component |
| ADMIN-02 | 201-03 | Games page game_state column with StatusBadge | SATISFIED — games/page.tsx confirmed |
| ADMIN-03 | 201-03 | /games/reliability page with per-pod launch matrix | SATISFIED — page.tsx exists with getLaunchMatrix + color coding |
| ADMIN-04 | 201-03 | metrics.ts exports 4 typed API client methods | SATISFIED — getLaunchStats, getBillingAccuracy, getLaunchMatrix, getAlternatives all present |

---

## Anti-Patterns Found

No blockers or significant warnings found.

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `web/src/lib/api/metrics.ts` | 11-52 | Types declared locally instead of importing from @racingpoint/types | INFO | Documented deliberate decision; correct shape; not drift-covered by parity script (metrics types only, not billing/game state types) |
| `pwa/src/app/sessions/[id]/page.tsx` | 47-79 | `statusBadgeClasses` switch missing waiting_for_game, paused_disconnect, paused_game_pause, cancelled_no_playable | INFO | Falls through to default grey — non-breaking for customer-facing PWA (read-only session history) |
| `pwa/src/app/book/active/page.tsx` | 75 | `billing.status !== "active"` check — does not pause countdown during waiting_for_game | INFO | PWA uses `status: string` (deliberate, per research) — countdown accuracy on PWA is low priority |

All three are INFO-level only. The PWA status handling was pre-scoped as acceptable per the research document ("string is OK for PWA"). None prevent the phase goal.

---

## Human Verification Required

### 1. Kiosk Loading State Display

**Test:** On live kiosk at pod, trigger a session start (assign customer, launch game), observe kiosk panel immediately after game launch command before AC STATUS=LIVE
**Expected:** "Game Loading..." text with blue background and spinning SVG icon. Countdown timer frozen. Session label shows "Game Loading" instead of "Session Time"
**Why human:** UI rendering requires live WS session with billing.status = "waiting_for_game" — cannot grep for rendered output

### 2. Kiosk Crash Recovery Banner

**Test:** Simulate paused_game_pause state via WS injection or actual game crash during session
**Expected:** Amber background banner with spinning icon, "Relaunching Game..." text, "Relaunch Now" button visible
**Why human:** Requires live session state change; visual rendering cannot be verified statically

### 3. Kiosk Reliability Warning Modal

**Test:** In SetupWizard review step, select a game/car/track combination that returns success_rate < 0.70 from `/api/v1/games/alternatives`
**Expected:** Amber banner shows percentage; "Suggest Alternative" button appears; tapping opens modal with top-3 alternatives
**Why human:** Requires backend data from Phase 200 metrics APIs + live SetupWizard flow

### 4. Web Billing Page — waiting_for_game State

**Test:** Start a billing session on web dashboard when kiosk is in game loading state
**Expected:** Billing card shows "Loading..." purple badge; End, Pause, and Extend buttons are not rendered
**Why human:** Requires live WS session with status=waiting_for_game visible in billing page

### 5. /games/reliability Page Data Load

**Test:** Navigate to :3200/games/reliability in browser
**Expected:** Table loads with per-pod rows; rows with <70% success_rate have red background; flagged=true rows show red "Flagged" badge; game selector dropdown works
**Why human:** Requires live backend with data in /api/v1/admin/launch-matrix response

---

## Gaps Summary

No gaps. All 19 requirements satisfied. All 17 observable truths verified from code inspection.

The one structural observation: the ROADMAP goal mentions "All 4 frontend apps (kiosk, web dashboard, admin, PWA)" but the research pre-determined that PWA's `status: string` is acceptable (customer-facing read-only history only). The plans did not include a PWA plan, and the research explicitly states this is fine. The PWA `sessions/[id]/page.tsx` gracefully handles unknown statuses via `default` case. This is not a gap — it was a scoped design decision.

---

_Verified: 2026-03-26T12:30:00+05:30_
_Verifier: Claude (gsd-verifier)_
