---
phase: 08-staff-pwa-integration
verified: 2026-03-14T04:15:00Z
status: passed
score: 8/8 must-haves verified
re_verification: false
---

# Phase 8: Staff & PWA Integration Verification Report

**Phase Goal:** Both customer (PWA/QR) and staff (kiosk) launch paths work end-to-end with the new session system. All 5 session types from Phase 1 (Practice, Hotlap, Race vs AI, Track Day, Race Weekend) are wired to both frontends. Session type selection uses Phase 5's AI-line filtering. Both launch paths converge on the same backend validation.
**Verified:** 2026-03-14T04:15:00Z
**Status:** passed
**Re-verification:** No -- initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | SessionType union in kiosk types.ts includes all 5 values | VERIFIED | Line 282: `"practice" \| "hotlap" \| "race" \| "trackday" \| "race_weekend"` |
| 2 | CustomBookingPayload in PWA api.ts includes session_type field | VERIFIED | Line 307: `session_type?: string;` in CustomBookingPayload interface |
| 3 | CustomBookingOptions in routes.rs includes session_type field | VERIFIED | Lines 4838-4839: `#[serde(default)] session_type: Option<String>` |
| 4 | build_custom_launch_args includes session_type in output JSON | VERIFIED | Lines 722-738 in catalog.rs: accepts `session_type: &str` param, outputs `"session_type": session_type` in json! macro |
| 5 | customer_book_session passes session_type through to launch_args | VERIFIED | Lines 5000-5002 in routes.rs: passes `c.session_type.as_deref().unwrap_or("practice")` to build_custom_launch_args; line 5012: double-writes session_type in post-processing block |
| 6 | validate_launch_combo rejects race_weekend on tracks without AI lines | VERIFIED | Line 482 in catalog.rs: `matches!(session_type, "race" \| "trackday" \| "race_weekend")` checks AI lines; test `validate_launch_combo_rejects_race_weekend_without_ai` at line 1048 confirms |
| 7 | Kiosk SetupWizard shows 5 session types and GameConfigurator has session_type step with launch_args wiring | VERIFIED | SetupWizard.tsx lines 461-467: 5 session type cards (practice, hotlap, race, trackday, race_weekend). GameConfigurator.tsx: ConfigStep type includes "session_type" (line 15), handleLaunch includes `session_type: sessionType` (line 136), selectPreset sets sessionType (line 121) |
| 8 | PWA Mode step replaced with Session Type step showing 5 types plus separate multiplayer card | VERIFIED | SessionTypeStep function at line 652 of book/page.tsx: 5 types in array (lines 661-667), "Race with Friends" multiplayer card (line 686-699) with dashed blue border; handleBook includes `session_type: sessionType` (line 245) |

**Score:** 8/8 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `kiosk/src/lib/types.ts` | Updated SessionType union (5 values) | VERIFIED | Line 282: exact 5-value union, "qualification" removed |
| `pwa/src/lib/api.ts` | CustomBookingPayload with session_type | VERIFIED | Line 307: optional session_type field present |
| `crates/rc-core/src/catalog.rs` | build_custom_launch_args with session_type parameter | VERIFIED | Line 722: session_type param added, line 738: included in JSON output |
| `crates/rc-core/src/api/routes.rs` | CustomBookingOptions with session_type field | VERIFIED | Lines 4838-4839: Option<String> with serde(default) |
| `kiosk/src/components/SetupWizard.tsx` | 5 session type options in session_type step | VERIFIED | Lines 461-467: all 5 types rendered as cards with labels and descriptions |
| `kiosk/src/components/GameConfigurator.tsx` | session_type ConfigStep + launch_args wiring | VERIFIED | Full 577-line component with session_type step (lines 297-325), handleLaunch JSON (line 136), selectPreset (line 121), track filtering (lines 84-91) |
| `pwa/src/app/book/page.tsx` | SessionTypeStep replacing ModeStep | VERIFIED | SessionTypeStep component (lines 652-702) with 5 types + multiplayer card, handleBook wiring (line 245), preset pre-fill (line 219) |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `routes.rs` (customer_book_session) | `catalog.rs` (build_custom_launch_args) | session_type param pass-through | WIRED | Line 5002: `c.session_type.as_deref().unwrap_or("practice")` passed as 7th arg |
| `pwa/src/lib/api.ts` (CustomBookingPayload) | `routes.rs` (CustomBookingOptions) | session_type field match | WIRED | PWA sends `session_type` in JSON body; routes.rs deserializes via `session_type: Option<String>` |
| `game_launcher.rs` | `catalog.rs` (validate_launch_combo) | session_type parsed from launch_args | WIRED | Line 83: `args.get("session_type")`, line 85: `validate_launch_combo(manifest.as_ref(), car, track, session_type)` |
| `GameConfigurator.tsx` (handleLaunch) | `game_launcher.rs` | session_type in launch_args JSON | WIRED | Line 136: `session_type: sessionType` in JSON.stringify; flows through /games/launch -> launch_game() -> game_launcher |
| `pwa/book/page.tsx` (handleBook) | `pwa/src/lib/api.ts` | session_type in CustomBookingPayload | WIRED | Line 245: `session_type: sessionType` in custom object; api.bookCustom sends it |
| `SetupWizard.tsx` | `useSetupWizard.ts` | handleSelectSessionType sets sessionType | WIRED | SetupWizard line 223: `handleSelectSessionType(type)` calls `setField("sessionType", type)`; useSetupWizard line 176: `session_type: state.sessionType` in buildLaunchArgs |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| SESS-06 | 08-01, 08-02 | Staff can configure any session type from kiosk for a pod | SATISFIED | GameConfigurator has session_type step with all 5 types, handleLaunch sends session_type in launch_args, flows through to validate_launch_combo. SetupWizard also has all 5 types with buildLaunchArgs including session_type. |
| CONT-03 | 08-01, 08-02 | Staff can configure car/track/session from kiosk | SATISFIED | GameConfigurator provides full wizard: game -> session_type -> track -> car -> settings -> review -> launch. Track filtering by session type implemented. |

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `GameConfigurator.tsx` | 290 | "Coming Soon" text for disabled games | Info | Acceptable -- disabled games (Forza) show "Coming Soon", not a placeholder for Phase 8 functionality |
| `pwa/book/page.tsx` | 643 | "Coming soon" text | Info | In a different section (not Phase 8 related), likely for a feature not yet implemented |

No blocker or warning-level anti-patterns found in Phase 8 files. No TODO/FIXME/PLACEHOLDER comments in any modified files related to session type functionality.

### Human Verification Required

### 1. Kiosk SetupWizard Session Type Visual

**Test:** Open kiosk, start a new booking, navigate to the session_type step
**Expected:** 5 session type cards displayed (Practice, Hotlap, Race vs AI, Track Day, Race Weekend) with icons, labels, and descriptions. No "qualification" option.
**Why human:** Visual rendering and icon correctness cannot be verified programmatically

### 2. Kiosk GameConfigurator Full Flow

**Test:** Click a pod card in kiosk, select Custom Setup, pick a game, then select each of the 5 session types and verify track filtering works for AI-requiring types
**Expected:** After selecting "Race vs AI" or "Track Day" or "Race Weekend", the track list should only show tracks with AI capability (if available_session_types data is present from the API)
**Why human:** Requires live API data and visual confirmation of track list filtering

### 3. PWA SessionTypeStep Rendering

**Test:** Log in to PWA, start booking, navigate past Duration and Game steps to reach Session Type step
**Expected:** 5 session type cards plus a visually distinct "Race with Friends" multiplayer card with dashed blue border. Selecting a session type advances to Track step.
**Why human:** Visual rendering, border styling, and interaction flow require manual testing

### 4. End-to-End Launch Verification

**Test:** Complete full booking from kiosk (staff path) and PWA (customer path) with "Race vs AI" session type on a track with AI lines
**Expected:** Both paths converge on the same backend validation. Game launches with correct session_type parameter. validate_launch_combo accepts the combo.
**Why human:** Requires running backend, pod agent, and actual game launch to verify full flow

### Gaps Summary

No gaps found. All 8 must-haves from Plans 01 and 02 are verified in the codebase:

1. TypeScript types updated with all 5 session types in both kiosk and PWA
2. Rust backend structs include session_type with backward-compatible defaults
3. build_custom_launch_args accepts and outputs session_type in JSON
4. customer_book_session passes session_type through to launch_args (with double-write for consistency)
5. validate_launch_combo correctly rejects race_weekend on tracks without AI (bug fix included)
6. Staff launch path (game_launcher) already reads session_type from launch_args -- pass-through confirmed working
7. All 3 frontends (SetupWizard, GameConfigurator, PWA booking) show 5 session types with track filtering by AI capability
8. Tests cover session_type output in launch args and race_weekend AI validation

Both requirements SESS-06 and CONT-03 are satisfied. No orphaned requirements found.

---

_Verified: 2026-03-14T04:15:00Z_
_Verifier: Claude (gsd-verifier)_
