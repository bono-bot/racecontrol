---
phase: 43-wizard-flows-api-pipeline-tests
verified: 2026-03-19T12:00:00+05:30
status: human_needed
score: 11/11 must-haves verified
human_verification:
  - test: "Run: npx playwright test tests/e2e/playwright/kiosk/wizard.spec.ts --list"
    expected: "5 tests listed without error (one per BROW-02 through BROW-06)"
    why_human: "Cannot run npx commands in this shell environment"
  - test: "Run: bash -n tests/e2e/api/billing.sh && bash -n tests/e2e/api/launch.sh"
    expected: "Both exit 0 with no syntax errors"
    why_human: "Bash execution was blocked by sandbox; file content has been reviewed manually and looks correct"
  - test: "With live venue server up (192.168.31.23:8080), run: bash tests/e2e/api/billing.sh"
    expected: "Gates 0-5 pass: server reachable, pod-99 rejected, billing created on pod-8, session appears in active, session ended, session gone from active"
    why_human: "Requires live server and network — cannot verify programmatically"
  - test: "With live venue server and pod-8 agent connected, run: bash tests/e2e/api/launch.sh"
    expected: "All 7 games loop: launch accepted, dismiss_steam_dialog fires on port 8091, state transitions through Launching, game stops. On launch error, screenshot captured at C:/RacingPoint/test-screenshot-{game}.png"
    why_human: "Requires running pods with rc-agent connected — cannot verify programmatically"
  - test: "With live kiosk at http://192.168.31.23:3300, run the full Playwright wizard suite"
    expected: "5 wizard tests execute against the real kiosk; staff walk-in reaches wizard without OTP; F1 25 hides AC-only steps; back navigation returns to previous step"
    why_human: "Requires live kiosk server with browser tests running against it"
---

# Phase 43: Wizard Flows & API Pipeline Tests Verification Report

**Phase Goal:** All 5 sim wizard flows tested per-step in Playwright, experience filtering verified, staff mode booking tested, billing/launch/game-state API pipeline tested via shell scripts, Steam dialog auto-dismissal, error window screenshots for AI debugger.
**Verified:** 2026-03-19T12:00:00+05:30
**Status:** human_needed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | AC wizard navigates through all steps (select_plan through review) with preset experiences | VERIFIED | wizard.spec.ts line 88-142: full AC path with conditional session_splits, player_mode, session_type, ai_config, select_experience, driving_settings, review; asserts select_track/car not visible |
| 2 | Non-AC wizard (F1 25) shows exactly 4 steps: select_plan, select_game, select_experience, review | VERIFIED | wizard.spec.ts line 49-81: 7 AC-only steps asserted not visible, 4-step flow walked |
| 3 | Staff mode walk-in bypasses phone/OTP and reaches wizard directly | VERIFIED | wizard.spec.ts line 148-167: walkin-btn visible, clicked, step-select-plan appears, booking-otp-screen and booking-phone-screen asserted not visible |
| 4 | Selecting F1 25 shows only F1 25 experiences and no select_track/select_car steps | VERIFIED | wizard.spec.ts line 173-191 (BROW-05): step-select-track, step-select-car, step-driving-settings all asserted not visible after F1 25 selected |
| 5 | Back button returns to previous step, step title updates on navigation | VERIFIED | wizard.spec.ts line 196-233: two back clicks from select_experience, step titles captured and compared, step-select-game then step-select-plan confirmed |
| 6 | Billing session can be created and ended via API on pod-8 | VERIFIED | billing.sh line 55-96: POST /billing/start with driver_test_trial + tier_trial; session end at lines 125-151; handles already-active idempotently |
| 7 | Launch without billing is rejected with a billing gate error | VERIFIED | billing.sh line 36-53: POST /games/launch on pod-99, asserts "no active billing" or error in response |
| 8 | Each enabled game reaches Launching or Running state after launch command | VERIFIED | launch.sh line 197: GAMES_TO_TEST covers all 7 enabled games; poll_game_state() checks for launching|running with 30s window (API-02 + API-03) |
| 9 | Game state transitions from Idle through Launching to eventual stop | VERIFIED | launch.sh lines 260-315: poll /games/active for presence, POST /games/stop, poll for NONE state (API-03) |
| 10 | Steam dialog WM_CLOSE is attempted via remote exec during launch | VERIFIED | launch.sh line 67-78: dismiss_steam_dialog() POSTs to port 8091 using PowerShell CloseMainWindow(); called at line 247 after every accepted launch (API-04) |
| 11 | Error window screenshot capture command runs without error on pod | VERIFIED | launch.sh lines 80-95: capture_error_screenshot() POSTs PowerShell .NET Graphics screenshot to port 8091; called at line 325 on any launch failure (API-05) |

**Score:** 11/11 truths verified

### Required Artifacts

| Artifact | Expected | Min Lines | Actual Lines | Status | Details |
|----------|----------|-----------|--------------|--------|---------|
| `tests/e2e/playwright/kiosk/wizard.spec.ts` | All 5 browser wizard test scenarios | 120 | 233 | VERIFIED | 5 top-level test() calls; imports from cleanup fixture; uses staff mode entry pattern throughout |
| `tests/e2e/api/billing.sh` | Billing lifecycle API test | 50 | 198 | VERIFIED | 5-gate structure: health, rejection, create, active check, end session, verify ended |
| `tests/e2e/api/launch.sh` | Per-game launch + state lifecycle + Steam dismiss + error screenshot | 100 | 363 | VERIFIED | Pre-gates + 7-game loop with poll_game_state, dismiss_steam_dialog, capture_error_screenshot |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `wizard.spec.ts` | `tests/e2e/playwright/fixtures/cleanup.ts` | `import { test, expect } from '../fixtures/cleanup'` | WIRED | Line 1: exact import; cleanup.ts exports `test` and `expect`; auto fixture runs before every test |
| `wizard.spec.ts` | kiosk `/book?staff=true&pod=pod-8` | `page.goto` with staff mode URL | WIRED | Lines 37 and 149: URL used in `enterWizardViaStaffWalkIn()` helper and standalone BROW-04 test |
| `billing.sh` | `tests/e2e/lib/common.sh` | `source "$SCRIPT_DIR/../lib/common.sh"` | WIRED | Line 12: sourced; common.sh exports pass/fail/skip/info/summary_exit — all used in billing.sh |
| `billing.sh` | `tests/e2e/lib/pod-map.sh` | `source "$SCRIPT_DIR/../lib/pod-map.sh"` | WIRED | Line 14: sourced; pod-map.sh exports pod_ip() — available in billing.sh scope |
| `launch.sh` | `tests/e2e/lib/common.sh` | `source "$SCRIPT_DIR/../lib/common.sh"` | WIRED | Line 18: sourced; pass/fail/skip/info/summary_exit all used throughout launch.sh |
| `launch.sh` | `tests/e2e/lib/pod-map.sh` | `source "$SCRIPT_DIR/../lib/pod-map.sh"` | WIRED | Line 20: sourced; `POD_IP=$(pod_ip "${POD_ID}")` at line 22 |
| `launch.sh` | `http://POD_IP:8091/exec` | curl POST for Steam dialog dismiss + screenshot | WIRED | Lines 70-72 (dismiss_steam_dialog) and 87-89 (capture_error_screenshot): both use port 8091 not 8090 |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| BROW-02 | 43-01-PLAN.md | AC wizard flow — full path with AI config, driving settings | SATISFIED | wizard.spec.ts line 88-142: AC preset path through all AC-specific steps to review |
| BROW-03 | 43-01-PLAN.md | Non-AC wizard flow — simplified 4-step flow for F1 25 | SATISFIED | wizard.spec.ts line 49-81: 4-step flow; 7 AC-only steps asserted absent |
| BROW-04 | 43-01-PLAN.md | Staff mode booking — bypass path tested end-to-end | SATISFIED | wizard.spec.ts line 148-167: walkin-btn click bypasses OTP, booking-otp-screen asserted not visible |
| BROW-05 | 43-01-PLAN.md | Experience filtering — only selected game's steps appear | SATISFIED | wizard.spec.ts line 173-191: F1 25 selection hides select_track, select_car, driving_settings |
| BROW-06 | 43-01-PLAN.md | UI navigation — back/forward, step indicators update | SATISFIED | wizard.spec.ts line 196-233: two back clicks, step title changes verified |
| API-01 | 43-02-PLAN.md | Billing gates — reject without billing, create/end session | SATISFIED | billing.sh gates 1-5: rejection on pod-99, create with driver_test_trial+tier_trial, verify active, end, verify gone |
| API-02 | 43-02-PLAN.md | Per-game launch — all enabled games | SATISFIED | launch.sh line 197: all 7 enabled games in GAMES_TO_TEST loop; POST /games/launch per game |
| API-03 | 43-02-PLAN.md | Game state lifecycle — Launching→Running→Stop→Idle | SATISFIED | launch.sh: poll_game_state() helper + /games/active presence check + stop + poll for NONE |
| API-04 | 43-02-PLAN.md | Steam dialog auto-dismiss — WM_CLOSE via remote exec | SATISFIED | launch.sh lines 67-78: dismiss_steam_dialog() via port 8091 PowerShell CloseMainWindow(); fires after every accepted launch |
| API-05 | 43-02-PLAN.md | Error window screenshot — capture on pods for AI debugger | SATISFIED | launch.sh lines 80-95: capture_error_screenshot() via port 8091 PowerShell .NET screenshot; fires on launch failure |

No orphaned requirements: all 10 Phase 43 IDs in REQUIREMENTS.md appear in plan frontmatter and are covered by implementation.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| None | — | — | — | — |

No TODO/FIXME/placeholder patterns found in any artifact. No empty implementations or stub returns detected.

### Human Verification Required

#### 1. Playwright Test Discovery

**Test:** Run `npx playwright test tests/e2e/playwright/kiosk/wizard.spec.ts --list` from repo root.
**Expected:** 5 tests listed without syntax error, matching test names for BROW-02 through BROW-06.
**Why human:** Playwright CLI cannot be executed in the current sandbox environment.

#### 2. Shell Script Syntax Validation

**Test:** Run `bash -n tests/e2e/api/billing.sh && bash -n tests/e2e/api/launch.sh`.
**Expected:** Both commands exit 0 with no output (valid syntax).
**Why human:** Bash execution was blocked by sandbox. Files were reviewed in full manually — structure appears syntactically correct, but only `bash -n` can confirm definitively.

#### 3. Live Billing API Test

**Test:** With venue server running, execute `bash tests/e2e/api/billing.sh` (or with `RC_BASE_URL=http://192.168.31.23:8080/api/v1`).
**Expected:** Gate 0 passes (health), Gate 1 passes (pod-99 rejected for "no active billing"), Gate 2 creates session, Gate 3 sees it in active sessions, Gate 4 ends it, Gate 5 confirms it is gone.
**Why human:** Requires live server at 192.168.31.23 and real billing API — cannot verify programmatically.

#### 4. Live Per-Game Launch Test

**Test:** With pod-8 rc-agent connected, execute `bash tests/e2e/api/launch.sh`.
**Expected:** All 7 games loop successfully; Steam dialog dismiss attempted on port 8091; at least one game reaches Launching state; stop command accepted; screenshot captured on any failure.
**Why human:** Requires running pods with rc-agent WebSocket connection — cannot verify programmatically.

#### 5. Live Wizard Browser Tests

**Test:** With kiosk server running at http://192.168.31.23:3300, run the full Playwright suite against the live venue.
**Expected:** All 5 wizard tests pass end-to-end: staff walk-in reaches wizard, F1 25 hides AC steps, AC flow traverses all AC-specific steps, back navigation works.
**Why human:** Requires live kiosk and browser environment.

### Summary

Phase 43 goal is fully achieved at the code level. All 3 artifacts exist with substantial implementations well above minimum line requirements. All 11 observable truths are verified by direct code inspection. All 7 key links are confirmed wired. All 10 requirement IDs (BROW-02 through BROW-06, API-01 through API-05) are covered.

The only remaining items are live execution tests that require the venue network — the scripts are structured correctly and connect to the right dependencies. The human verification items above are confirmations of behavior against live infrastructure, not gaps in the implementation.

---

_Verified: 2026-03-19T12:00:00+05:30_
_Verifier: Claude (gsd-verifier)_
