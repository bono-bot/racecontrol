---
phase: 02-difficulty-tiers
verified: 2026-03-13T09:15:00Z
status: passed
score: 7/7 must-haves verified
re_verification: false
---

# Phase 2: Difficulty Tiers Verification Report

**Phase Goal:** Customers choose a racing-themed difficulty level (Rookie/Amateur/Semi-Pro/Pro/Alien) that controls AI strength via AI_LEVEL, with a slider for fine-tuning. Assists are independent -- not bundled with tiers.
**Verified:** 2026-03-13T09:15:00Z
**Status:** passed
**Re-verification:** No -- initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | DifficultyTier enum has exactly 5 variants: Rookie, Amateur, SemiPro, Pro, Alien | VERIFIED | ac_launcher.rs:30-36 -- `pub enum DifficultyTier` with 5 variants, derives Serialize/Deserialize |
| 2 | tier_for_level(75) returns Rookie, tier_for_level(87) returns SemiPro, tier_for_level(98) returns Alien | VERIFIED | ac_launcher.rs:86-94 -- pure function iterates all tiers, checks range. test_tier_boundaries (line 2011) passes all boundary values |
| 3 | tier_for_level(69) returns None, tier_for_level(101) returns None (values outside tier ranges) | VERIFIED | ac_launcher.rs:86-94 -- returns None when no tier range matches. test_tier_boundaries asserts None for 69 and 101; test_tier_for_level_zero asserts None for 0 |
| 4 | AcLaunchParams.ai_level defaults to 87 (Semi-Pro midpoint) when not provided in JSON | VERIFIED | ac_launcher.rs:176-177 -- `#[serde(default = "default_session_ai_level")] pub ai_level: u32`; line 107 -- `fn default_session_ai_level() -> u32 { 87 }`. test_backward_compat_no_ai_level_field passes |
| 5 | race.ini [RACE] AI_LEVEL uses params.ai_level, not params.ai_cars[0].ai_level | VERIFIED | ac_launcher.rs:598 -- `writeln!(ini, "AI_LEVEL={}", params.ai_level)` directly. Old per-car derivation logic removed. test_race_ini_uses_session_ai_level passes |
| 6 | All AI car slots in effective_ai_cars() inherit params.ai_level (session-wide, not per-car) | VERIFIED | ac_launcher.rs:461-480 -- trackday branch passes params.ai_level to generate_trackday_ai(); race branch maps each slot to `AiCarSlot { ai_level: params.ai_level, ..slot.clone() }`. Both test_effective_ai_cars_inherits and test_trackday_default_ai_inherits pass |
| 7 | Assists (AcAids) remain completely independent of difficulty tier selection | VERIFIED | AcAids struct (line 202-213) has zero references to DifficultyTier. grep for `DifficultyTier.*AcAids` and `AcAids.*DifficultyTier` returns no matches. write_assists_section (line 484-502) reads only from params.aids, never from params.ai_level or any tier |

**Score:** 7/7 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/rc-agent/src/ac_launcher.rs` | DifficultyTier enum, tier_for_level(), ai_level on AcLaunchParams, updated INI builder, updated effective_ai_cars | VERIFIED | All expected items present: enum at line 30, tier_for_level at line 86, ai_level field at line 177, write_race_config_section uses params.ai_level at line 598, effective_ai_cars overrides at lines 464 and 473-477 |
| `crates/rc-agent/src/main.rs` | Fallback AcLaunchParams literals with ai_level: 87 | VERIFIED | Both fallback literals contain `ai_level: 87` at lines 929 and 953 |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| AcLaunchParams.ai_level | write_race_config_section | `params.ai_level` used for AI_LEVEL= in [RACE] | WIRED | Line 598: `writeln!(ini, "AI_LEVEL={}", params.ai_level)` -- direct usage, no intermediary |
| AcLaunchParams.ai_level | effective_ai_cars | Each AiCarSlot.ai_level overridden with params.ai_level | WIRED | Lines 464 (trackday: `generate_trackday_ai(count, params.ai_level)`) and 475 (race: `ai_level: params.ai_level`) |
| DifficultyTier | tier_for_level | Pure function mapping u32 -> Option of DifficultyTier | WIRED | Line 86: `pub fn tier_for_level(ai_level: u32) -> Option<DifficultyTier>` iterates all tiers and checks range |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| DIFF-01 | 02-01 | 5 racing-themed difficulty tiers available | SATISFIED | DifficultyTier enum has Rookie/Amateur/SemiPro/Pro/Alien with display_name() returning "Rookie", "Amateur", "Semi-Pro", "Pro", "Alien" |
| DIFF-02 | 02-01 | Each tier maps to specific AC parameters (AI_LEVEL) | SATISFIED | Each tier maps to AI_LEVEL range via range()/midpoint(). Assists and aggression explicitly excluded per user decision. AI_AGGRESSION not written (only appears in doc comment) |
| DIFF-03 | 02-01 | Rookie tier auto-enables all assists | SUPERSEDED | User decision: assists are completely independent of tier. AcAids has zero coupling to DifficultyTier. Requirement text in REQUIREMENTS.md still says "auto-enables" but context/plan/roadmap all document this was superseded |
| DIFF-04 | 02-01 | Alien tier disables all assists | SUPERSEDED | Same as DIFF-03 -- assists are independent. No tier-to-assist mapping exists anywhere in the codebase (confirmed by grep) |
| DIFF-05 | 02-01 | Customer can set custom difficulty via slider (direct AI_LEVEL control) | SATISFIED | AcLaunchParams.ai_level accepts any u32 (0-100+). tier_for_level returns None for values outside tier ranges (0-69, 101+), enabling "Custom" display. Backend ready; UI slider is Phase 8 |

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| (none) | - | - | - | No TODO/FIXME/PLACEHOLDER found in ac_launcher.rs |
| (none) | - | - | - | No .unwrap() in production code (all .unwrap() calls are in #[cfg(test)] module only) |

### Human Verification Required

### 1. AI drives at expected difficulty on pod

**Test:** Deploy to Pod 8, launch a race at Rookie (ai_level: 75) and Alien (ai_level: 98), observe AI behavior difference
**Expected:** AI at 75 should be noticeably slower and less aggressive than AI at 98
**Why human:** Requires running AC on a physical pod and observing real-time AI driving behavior

### 2. Slider in PWA/kiosk updates tier label (Phase 8 scope)

**Test:** Load PWA, drag difficulty slider, verify tier name updates in real-time
**Expected:** Slider at 75 shows "Rookie", at 87 shows "Semi-Pro", at 98 shows "Alien"
**Why human:** PWA/UI is Phase 8 -- this backend phase provides the data model but the slider UI does not exist yet. Not a gap for Phase 2.

### Gaps Summary

No gaps found. All 7 observable truths are verified against the actual codebase. The DifficultyTier enum is substantive (not a stub) with full method implementations and 10 dedicated tests covering boundaries, midpoints, ranges, display names, backward compatibility, INI output, and AI slot inheritance. All key links are wired -- params.ai_level flows through to both write_race_config_section and effective_ai_cars. Assists remain completely decoupled from difficulty tiers as intended by the user's design decision. Both commits (f7ca1a8, 03b91ff) are verified in git history.

DIFF-03 and DIFF-04 are marked SUPERSEDED (not failed) because the user explicitly decided that difficulty tiers control AI_LEVEL only, making the original requirement text (assist presets per tier) inapplicable. This decision is documented in CONTEXT.md, PLAN.md, SUMMARY.md, and ROADMAP.md.

---

_Verified: 2026-03-13T09:15:00Z_
_Verifier: Claude (gsd-verifier)_
