# Phase 2: Difficulty Tiers - Research

**Researched:** 2026-03-13
**Domain:** Assetto Corsa AI_LEVEL configuration, difficulty tier mapping, Rust enum/struct patterns
**Confidence:** HIGH

## Summary

This phase adds a DifficultyTier system that maps 5 racing-themed names to AI_LEVEL values in AC's race.ini. The user has explicitly decided that tiers control AI_LEVEL **only** -- assists are completely independent, and AI_AGGRESSION is not used. This dramatically simplifies the phase compared to the original REQUIREMENTS.md wording (which mentioned assist bundling).

The existing codebase already has the plumbing: `AiCarSlot.ai_level` (u32, 0-100), `write_race_config_section()` writes `AI_LEVEL=N` to race.ini, and `AcAids` handles assists independently via `write_assists_section()` and `write_assists_ini()`. The primary work is: (1) define a `DifficultyTier` enum with the 5 tiers and their AI_LEVEL ranges, (2) add an `ai_level` field to `AcLaunchParams` (top-level, not just per-AiCarSlot), (3) feed this value into `write_race_config_section()` and `effective_ai_cars()`, and (4) add a `tier_for_level()` function for slider-to-tier-name mapping.

**Primary recommendation:** Add a `DifficultyTier` enum and a top-level `ai_level: u32` to `AcLaunchParams`. The tier is computed from the level (pure function), not stored. The INI builder reads `params.ai_level` instead of `params.ai_cars[0].ai_level`. All AI car slots inherit the session's ai_level. Tests verify tier boundaries, INI output, and JSON backward compatibility.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Difficulty tiers control **AI_LEVEL only** -- not assists, not aggression
- Assists (ABS, TC, stability, autoclutch, ideal_line) are **completely independent** parameters
- A customer can pick Alien difficulty (AI_LEVEL=100) with all assists enabled, or Rookie (AI_LEVEL=70) with no assists
- AI_AGGRESSION is **not used** in this phase -- uncertain CSP support, not worth the risk
- 5 named tier presets: **Rookie / Amateur / Semi-Pro / Pro / Alien**
- Plus a **slider (0-100)** for fine-tuning AI_LEVEL directly
- Selecting a tier preset sets the slider to that tier's midpoint value
- Customer can freely adjust the slider after selecting a tier
- Slider **always shows the nearest tier name** as the customer drags
- Tier name updates dynamically based on the current AI_LEVEL value
- **Rookie:** 70-79 (default midpoint: 75)
- **Amateur:** 80-84 (default midpoint: 82)
- **Semi-Pro:** 85-89 (default midpoint: 87)
- **Pro:** 90-95 (default midpoint: 93)
- **Alien:** 96-100 (default midpoint: 98)
- Values below 70 or above 100 are allowed via slider but show no tier name (or "Custom")

### Claude's Discretion
- Exact implementation of the DifficultyTier enum/struct
- How tier selection flows through AcLaunchParams to the INI builder
- Whether to add a `difficulty_tier` field to AcLaunchParams or compute from ai_level
- Default tier for new sessions (probably Semi-Pro / 87)

### Deferred Ideas (OUT OF SCOPE)
- AI_AGGRESSION per-tier -- defer until CSP support is verified on pods (possibly Phase 2.1 if confirmed working)
- Mid-session difficulty adjustment -- Phase 6 covers mid-session controls
- Per-AI-car difficulty variation (e.g., mix of Pro and Amateur AI in same race) -- future enhancement
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| DIFF-01 | 5 racing-themed difficulty tiers available: Rookie / Amateur / Semi-Pro / Pro / Alien | DifficultyTier enum with 5 variants, `tier_for_level()` mapping function, community-standard AI_LEVEL ranges |
| DIFF-02 | Each tier maps to specific AC parameters (AI_LEVEL, AI_AGGRESSION, assist defaults) | **SUPERSEDED by CONTEXT.md:** Tiers map to AI_LEVEL only. No AI_AGGRESSION. No assist bundling. Assists remain independent via existing AcAids struct. |
| DIFF-03 | Rookie tier auto-enables all assists (ABS, TC, SC, auto-transmission, ideal line) | **SUPERSEDED by CONTEXT.md:** Tiers do NOT control assists. Assists are independent. This requirement is satisfied by the existing AcAids system being available alongside tier selection. |
| DIFF-04 | Alien tier disables all assists (manual everything, no aids) | **SUPERSEDED by CONTEXT.md:** Tiers do NOT control assists. Same as DIFF-03. |
| DIFF-05 | Customer can set custom difficulty via slider (direct AI_LEVEL control) for advanced use | Top-level `ai_level: u32` field on AcLaunchParams (0-100), `tier_for_level()` returns Option<DifficultyTier> for label display, values outside tier ranges return None/"Custom" |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| serde | workspace | Deserialize ai_level from JSON, serialize DifficultyTier | Already used for all AcLaunchParams fields |
| serde_json | workspace | Test fixtures, JSON round-trip validation | Already used in 30+ test cases |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| (none needed) | - | - | This phase requires zero new dependencies |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Enum with ranges | HashMap<String, (u32,u32)> | Enum is compile-time safe, exhaustive match, no runtime lookup cost |
| Computed tier from ai_level | Stored tier field on params | Computing avoids stale data -- ai_level is the source of truth, tier is derived |

**Installation:**
```bash
# No new dependencies required
```

## Architecture Patterns

### Recommended Approach

The tier system is a pure data mapping layer. No new files needed -- all changes go into `ac_launcher.rs` where the existing AI_LEVEL logic lives.

```
crates/rc-agent/src/ac_launcher.rs
  +-- DifficultyTier enum (5 variants)
  +-- tier_for_level(u32) -> Option<DifficultyTier>  (pure function)
  +-- DifficultyTier::midpoint(&self) -> u32          (returns default value)
  +-- DifficultyTier::range(&self) -> (u32, u32)      (returns min..=max)
  +-- AcLaunchParams.ai_level: u32                    (new top-level field)
  +-- write_race_config_section reads params.ai_level  (changed from ai_cars[0].ai_level)
  +-- effective_ai_cars sets each slot's ai_level from params.ai_level
```

### Pattern 1: DifficultyTier Enum with Computed Mapping

**What:** A `DifficultyTier` enum where the tier is computed from `ai_level`, never stored independently. The `ai_level: u32` is the single source of truth.

**When to use:** When the mapping is deterministic and the derived label is for display only.

**Why chosen (Claude's Discretion):** Computing the tier avoids synchronization bugs where a stored tier disagrees with the actual ai_level value. The slider/tier UI is purely frontend concern (Phase 8) -- the backend only needs the numeric ai_level. The tier enum exists for labeling, default selection, and test readability.

**Example:**
```rust
/// Racing-themed difficulty tiers controlling AI_LEVEL only.
/// Assists are completely independent (user decision from CONTEXT.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DifficultyTier {
    Rookie,
    Amateur,
    SemiPro,
    Pro,
    Alien,
}

impl DifficultyTier {
    /// AI_LEVEL range (inclusive) for this tier.
    pub fn range(&self) -> (u32, u32) {
        match self {
            Self::Rookie  => (70, 79),
            Self::Amateur => (80, 84),
            Self::SemiPro => (85, 89),
            Self::Pro     => (90, 95),
            Self::Alien   => (96, 100),
        }
    }

    /// Default AI_LEVEL when this tier is selected.
    pub fn midpoint(&self) -> u32 {
        match self {
            Self::Rookie  => 75,
            Self::Amateur => 82,
            Self::SemiPro => 87,
            Self::Pro     => 93,
            Self::Alien   => 98,
        }
    }

    /// Display name for UI (racing terminology).
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Rookie  => "Rookie",
            Self::Amateur => "Amateur",
            Self::SemiPro => "Semi-Pro",
            Self::Pro     => "Pro",
            Self::Alien   => "Alien",
        }
    }

    /// All tiers in order from easiest to hardest.
    pub fn all() -> &'static [DifficultyTier] {
        &[Self::Rookie, Self::Amateur, Self::SemiPro, Self::Pro, Self::Alien]
    }
}

/// Determine which tier an AI_LEVEL value falls into.
/// Returns None for values outside all tier ranges (0-69 or "Custom").
/// Values above 100 are technically allowed by AC but have no tier.
pub fn tier_for_level(ai_level: u32) -> Option<DifficultyTier> {
    DifficultyTier::all()
        .iter()
        .find(|t| {
            let (min, max) = t.range();
            ai_level >= min && ai_level <= max
        })
        .copied()
}
```

### Pattern 2: Top-Level ai_level on AcLaunchParams

**What:** Add `ai_level: u32` directly to `AcLaunchParams` (the session-wide setting), separate from the per-car `AiCarSlot.ai_level`.

**When to use:** When the difficulty applies uniformly to all AI in the session (which is the case for this venue -- per-car variation is deferred).

**Why chosen (Claude's Discretion):** Currently, `write_race_config_section()` reads `params.ai_cars[0].ai_level` to set the [RACE] AI_LEVEL. This is fragile: if ai_cars is empty (practice, hotlap), it falls back to 90. A top-level field makes the intent explicit and decouples AI difficulty from AI car configuration.

**Example:**
```rust
pub struct AcLaunchParams {
    // ... existing fields ...

    /// AI difficulty level (0-100). Controls AI_LEVEL in race.ini [RACE] section.
    /// Maps to DifficultyTier for display: Rookie(70-79), Amateur(80-84),
    /// Semi-Pro(85-89), Pro(90-95), Alien(96-100).
    /// Default: 87 (Semi-Pro midpoint).
    #[serde(default = "default_ai_level")]
    pub ai_level: u32,
}

fn default_ai_level() -> u32 { 87 } // Semi-Pro midpoint (was 90)
```

**Default tier rationale (Claude's Discretion):** Semi-Pro (87) is the recommended default because:
- Rookie (75) is too easy for most customers, AI will be noticeably slow
- Pro (93) is too hard for casual walk-ins who have never sim-raced
- Semi-Pro (87) provides competitive but beatable AI for the average customer
- The existing default was 90 (Pro range), which is slightly too aggressive for a walk-in venue
- Community consensus: 85-90 is the "fun racing" sweet spot for most AC players

### Pattern 3: AI Level Propagation to AI Car Slots

**What:** When `effective_ai_cars()` builds AI car slots, each slot inherits `params.ai_level` instead of using its own hardcoded value.

**Why:** Ensures the session-wide difficulty setting actually affects all AI opponents, not just the [RACE] section header.

**Example:**
```rust
fn effective_ai_cars(params: &AcLaunchParams) -> Vec<AiCarSlot> {
    if params.session_type == "trackday" && params.ai_cars.is_empty() {
        let count = DEFAULT_TRACKDAY_AI_COUNT.min(MAX_AI_SINGLE_PLAYER);
        generate_trackday_ai(count, params.ai_level) // pass session ai_level
    } else {
        let capped = params.ai_cars.len().min(MAX_AI_SINGLE_PLAYER);
        // Override each slot's ai_level with session-wide value
        params.ai_cars.iter().take(capped).map(|slot| {
            AiCarSlot {
                ai_level: params.ai_level,
                ..slot.clone()
            }
        }).collect()
    }
}
```

### Pattern 4: write_race_config_section Uses params.ai_level

**What:** The [RACE] section AI_LEVEL value comes from `params.ai_level` directly, not from `params.ai_cars[0].ai_level`.

**Example:**
```rust
fn write_race_config_section(ini: &mut String, params: &AcLaunchParams, ai_count: usize) {
    // ... existing track_config logic ...

    let _ = writeln!(ini, "\n[RACE]");
    let _ = writeln!(ini, "AI_LEVEL={}", params.ai_level); // was: ai_cars[0].ai_level
    // ... rest unchanged ...
}
```

### Anti-Patterns to Avoid

- **Storing tier AND ai_level in AcLaunchParams:** Creates synchronization risk. The tier is derivable from the level. Store only ai_level.
- **Bundling assists with tiers:** Explicitly rejected by user. Tiers control AI_LEVEL only. AcAids remains independent.
- **Writing AI_AGGRESSION to race.ini:** Explicitly deferred. CSP support uncertain. Do not add.
- **Per-car ai_level variation:** Deferred. All AI in a session use the same ai_level from params. The per-slot field on AiCarSlot remains for future use but is overridden by session-wide value.
- **Clamping ai_level to 70-100:** The user explicitly allows values 0-100 via slider. Values below 70 just show "Custom" instead of a tier name. Do not clamp.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Tier-to-range mapping | HashMap or config file | Rust enum with match arms | Compile-time exhaustive, zero runtime cost, no file I/O |
| JSON serialization of tier names | Manual string formatting | serde rename_all="snake_case" | Standard, tested, round-trips cleanly |
| Tier label computation | Store tier name alongside ai_level | `tier_for_level()` pure function | Single source of truth, no stale data |

**Key insight:** This feature is fundamentally a data mapping exercise. The enum + pure functions pattern is the correct level of abstraction -- no frameworks, no config files, no external dependencies needed.

## Common Pitfalls

### Pitfall 1: Backward Compatibility -- Old JSON Without ai_level Field

**What goes wrong:** The PWA/kiosk sends `launch_args` JSON that does not include the new `ai_level` field (because Phase 8 has not updated the UI yet). If `ai_level` is not `serde(default)`, deserialization fails and the entire launch breaks.

**Why it happens:** AcLaunchParams is deserialized from JSON sent by rc-core via WebSocket. Until the frontend is updated, the JSON will not contain `ai_level`.

**How to avoid:** Use `#[serde(default = "default_ai_level")]` exactly as the existing `AiCarSlot.ai_level` does. The default function returns 87 (Semi-Pro). Existing JSON with no ai_level field will seamlessly use Semi-Pro.

**Warning signs:** `serde_json::from_str` panics or returns Err in the `LaunchGame` handler, causing the unwrap_or fallback params to be used.

### Pitfall 2: Per-Car ai_level Conflicts With Session-Wide ai_level

**What goes wrong:** The JSON payload includes both `ai_level: 75` (session-wide) and `ai_cars: [{..., ai_level: 90}, ...]` (per-car). Which wins? If `write_race_config_section` reads from `params.ai_level` but the AI car slots still carry their own levels, there is a mismatch between the [RACE] AI_LEVEL header and what AC actually uses per-car.

**Why it happens:** AC uses the [RACE] AI_LEVEL as a global setting that applies to ALL AI cars. The per-car ai_level in AiCarSlot was Phase 1's approach before a session-wide field existed.

**How to avoid:** The session-wide `params.ai_level` is authoritative. `effective_ai_cars()` overrides each slot's ai_level with `params.ai_level`. The [RACE] AI_LEVEL and per-car behavior are consistent. The per-car `AiCarSlot.ai_level` field is retained for future per-car variation (deferred) but is not used for now.

**Warning signs:** AI opponents drive at different speed than expected; AI_LEVEL in race.ini does not match the selected tier.

### Pitfall 3: AI_LEVEL Below 70 Makes AI Unrealistically Slow

**What goes wrong:** The slider allows 0-100. At values below 60-70, AC AI becomes absurdly slow -- crawling around corners, braking extremely early, lapping 30+ seconds off pace. The customer gets bored because there is no competitive racing.

**Why it happens:** AC's AI_LEVEL roughly corresponds to throttle application percentage. Below 70%, the AI is visibly impaired and not fun to race against.

**How to avoid:** Do NOT clamp the slider (user allows 0-100). Instead, when the UI is built (Phase 8), show a warning/label like "Custom (AI may be very slow)" for values below 70. In the backend, the value passes through unchanged -- the warning is a UI concern only.

**Warning signs:** Customer complains AI is "not racing" or "just driving slowly."

### Pitfall 4: Fallback Params in main.rs Must Include ai_level

**What goes wrong:** The `main.rs` WebSocket handler has two manual `AcLaunchParams { ... }` struct literals (lines 913-935, 936-958) for the fallback case when JSON parsing fails. If the new `ai_level` field is added to the struct but not to these two literals, the code will not compile.

**Why it happens:** These are manual struct constructions, not `serde::from_str`. They must include every field.

**How to avoid:** Add `ai_level: 87` to both fallback struct literals in main.rs. The compiler will catch this as an error if forgotten (good).

**Warning signs:** Compilation failure in `main.rs` -- straightforward to fix but must not be overlooked.

## Code Examples

Verified patterns from codebase inspection:

### Current AI_LEVEL Flow (before this phase)
```rust
// ac_launcher.rs line 511-515 -- current behavior
fn write_race_config_section(ini: &mut String, params: &AcLaunchParams, ai_count: usize) {
    let ai_level = if !params.ai_cars.is_empty() {
        params.ai_cars[0].ai_level  // reads from first AI car slot
    } else {
        90 // hardcoded default
    };
    let _ = writeln!(ini, "AI_LEVEL={}", ai_level);
}
```

### After This Phase
```rust
fn write_race_config_section(ini: &mut String, params: &AcLaunchParams, ai_count: usize) {
    let _ = writeln!(ini, "AI_LEVEL={}", params.ai_level); // reads session-wide field
    // ... rest unchanged
}
```

### Test Pattern -- Tier Boundaries
```rust
#[test]
fn test_tier_boundaries() {
    // Exact boundary values
    assert_eq!(tier_for_level(69), None);           // below Rookie
    assert_eq!(tier_for_level(70), Some(DifficultyTier::Rookie));    // Rookie min
    assert_eq!(tier_for_level(79), Some(DifficultyTier::Rookie));    // Rookie max
    assert_eq!(tier_for_level(80), Some(DifficultyTier::Amateur));   // Amateur min
    assert_eq!(tier_for_level(84), Some(DifficultyTier::Amateur));   // Amateur max
    assert_eq!(tier_for_level(85), Some(DifficultyTier::SemiPro));   // Semi-Pro min
    assert_eq!(tier_for_level(89), Some(DifficultyTier::SemiPro));   // Semi-Pro max
    assert_eq!(tier_for_level(90), Some(DifficultyTier::Pro));       // Pro min
    assert_eq!(tier_for_level(95), Some(DifficultyTier::Pro));       // Pro max
    assert_eq!(tier_for_level(96), Some(DifficultyTier::Alien));     // Alien min
    assert_eq!(tier_for_level(100), Some(DifficultyTier::Alien));    // Alien max
    assert_eq!(tier_for_level(101), None);           // above Alien
}

#[test]
fn test_tier_midpoints() {
    assert_eq!(DifficultyTier::Rookie.midpoint(), 75);
    assert_eq!(DifficultyTier::Amateur.midpoint(), 82);
    assert_eq!(DifficultyTier::SemiPro.midpoint(), 87);
    assert_eq!(DifficultyTier::Pro.midpoint(), 93);
    assert_eq!(DifficultyTier::Alien.midpoint(), 98);
}
```

### Test Pattern -- Backward Compatibility
```rust
#[test]
fn test_backward_compat_no_ai_level_field() {
    // Existing JSON from Phase 1 has no ai_level field
    let json = r#"{"car":"ks_ferrari_488","track":"monza","server_ip":"","server_port":0,"server_http_port":0,"server_password":""}"#;
    let params: AcLaunchParams = serde_json::from_str(json)
        .expect("Must deserialize without ai_level field");
    assert_eq!(params.ai_level, 87, "Default must be Semi-Pro midpoint");
}
```

### Test Pattern -- INI Output Uses Session ai_level
```rust
#[test]
fn test_race_ini_uses_session_ai_level() {
    let json = r#"{"car":"ks_ferrari_488","track":"monza","session_type":"race","ai_level":75,"ai_cars":[
        {"model":"ks_ferrari_488_gt3","skin":"","driver_name":"Test","ai_level":90}
    ],"server_ip":"","server_port":0,"server_http_port":0,"server_password":""}"#;
    let params: AcLaunchParams = serde_json::from_str(json).unwrap();
    let ini = build_race_ini_string(&params);
    let sections = parse_ini(&ini);

    let race = sections.get("RACE").expect("RACE must exist");
    // Session-wide ai_level (75) wins over per-car ai_level (90)
    assert_eq!(race.get("AI_LEVEL").map(|s| s.as_str()), Some("75"));
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| AI_LEVEL from first ai_car slot | Session-wide ai_level on AcLaunchParams | This phase | Single source of truth, works for all session types |
| Default AI_LEVEL = 90 | Default AI_LEVEL = 87 (Semi-Pro) | This phase | Better default for walk-in venue customers |
| No tier concept | DifficultyTier enum with 5 tiers | This phase | Named tiers for customer-facing UI (Phase 8) |

**Important AC behavior notes:**
- AI_LEVEL 0-100 is a percentage roughly corresponding to AI throttle application. Community consensus is that 70-100 is the usable range for competitive racing. Below 70, AI is visibly impaired.
- AI_LEVEL is a global [RACE] section setting. AC applies it uniformly to all AI cars in the session.
- AI_AGGRESSION is a separate parameter. The user has deferred it due to uncertain CSP support. Multiple community sources confirm aggression primarily affects AI-vs-player interactions, not AI-vs-AI. CSP 1.80+ has improved AI aggression behavior, but this is CSP-version-dependent and risky for a venue with potentially mixed CSP versions across pods.
- Values above 100 are technically possible (via file editing) but not officially supported. The slider should cap at 100.

## Open Questions

1. **Should the default change from 90 to 87?**
   - What we know: User suggested "probably Semi-Pro / 87" as default. Current code uses 90 (Pro range).
   - What's unclear: Whether changing the default breaks any existing saved launch configs or expectations.
   - Recommendation: Change to 87. The serde default function is the only source of this value. No persistent configs store it. Safe to change.

2. **Should existing test assertions change from ai_level: 90 to 87?**
   - What we know: ~15 existing tests use `"ai_level":90` in JSON fixtures. These explicitly set ai_level, so they will not be affected by the default change.
   - What's unclear: The `test_ai_car_slot_default_ai_level` test asserts `== 90`. This tests `AiCarSlot`'s default, not `AcLaunchParams`'s default.
   - Recommendation: Keep AiCarSlot's default at 90 (it is the per-car field). Only AcLaunchParams gets the new default of 87. No existing tests break.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in test (#[cfg(test)]) + cargo test |
| Config file | crates/rc-agent/Cargo.toml (workspace) |
| Quick run command | `cargo test -p rc-agent --lib ac_launcher -- --nocapture` |
| Full suite command | `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p rc-core` |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| DIFF-01 | 5 tiers with correct names and ranges | unit | `cargo test -p rc-agent -- test_tier_boundaries test_tier_names -x` | Wave 0 |
| DIFF-02 | Tier maps to AI_LEVEL only (no assists, no aggression) | unit | `cargo test -p rc-agent -- test_race_ini_uses_session_ai_level -x` | Wave 0 |
| DIFF-03 | SUPERSEDED: assists independent (existing tests cover) | unit | `cargo test -p rc-agent -- test_write_race_ini_practice_with_aids -x` | Existing |
| DIFF-04 | SUPERSEDED: assists independent (existing tests cover) | unit | `cargo test -p rc-agent -- test_write_race_ini_practice_with_aids -x` | Existing |
| DIFF-05 | Slider sets ai_level 0-100, tier label computed | unit | `cargo test -p rc-agent -- test_tier_for_level_custom test_backward_compat -x` | Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p rc-agent --lib ac_launcher`
- **Per wave merge:** `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p rc-core`
- **Phase gate:** Full suite green before verification

### Wave 0 Gaps
- [ ] `test_tier_boundaries` -- tests all 5 tier boundary values (min, max, midpoint)
- [ ] `test_tier_midpoints` -- verifies midpoint values match CONTEXT.md spec
- [ ] `test_tier_for_level_custom` -- values below 70 and above 100 return None
- [ ] `test_race_ini_uses_session_ai_level` -- session ai_level overrides per-car ai_level in INI
- [ ] `test_backward_compat_no_ai_level_field` -- old JSON without ai_level defaults to 87
- [ ] `test_effective_ai_cars_inherits_session_ai_level` -- all AI car slots get session ai_level
- [ ] `test_trackday_default_ai_inherits_session_ai_level` -- trackday generated AI uses session ai_level, not hardcoded 85

## Sources

### Primary (HIGH confidence)
- Codebase: `crates/rc-agent/src/ac_launcher.rs` -- full AI_LEVEL flow (lines 32, 35, 374, 504-532), AiCarSlot struct, AcAids struct, write_assists_section, write_assists_ini, build_race_ini_string, effective_ai_cars, generate_trackday_ai, all test cases
- Codebase: `crates/rc-agent/src/main.rs` -- LaunchGame handler (lines 903-959), fallback AcLaunchParams literals
- CONTEXT.md -- user decisions on tier scope, ranges, slider behavior, deferred items

### Secondary (MEDIUM confidence)
- [OverTake.gg - AI aggression slider discussion](https://www.overtake.gg/threads/does-the-ai-aggression-slider-works.188874/) -- aggression affects AI-vs-player only, limited AI-vs-AI effect
- [OverTake.gg - Adjusting AI strength discussion](https://www.overtake.gg/threads/adjusting-ai-strength-has-no-effect.208382/) -- AI_LEVEL has built-in per-driver variation, not perfectly linear
- [Steam Community - Difficulty higher than 100%](https://steamcommunity.com/app/244210/discussions/0/1743353798890923701/) -- AI_LEVEL maxes at 100 in UI, file editing can exceed but no official support
- [GTPlanet - AC AI discussion](https://www.gtplanet.net/forum/threads/assetto-corsa-ai.350714/) -- community consensus on 80-100 being the competitive range

### Tertiary (LOW confidence)
- [Toolify.ai - CSP 1.80 AI improvements](https://www.toolify.ai/ai-news/revolutionary-ai-improvements-in-assetto-corsa-with-csp-180-preview-1960061) -- CSP 1.80 improves AI impatience/aggression, but version-dependent; validates decision to defer AI_AGGRESSION

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- no new dependencies, well-understood Rust enum pattern
- Architecture: HIGH -- codebase inspected line-by-line, integration points identified precisely
- Pitfalls: HIGH -- backward compat risk is concrete and testable, fallback literal issue is compiler-caught
- AI_LEVEL behavior: MEDIUM -- community sources confirm 70-100 usable range, but exact throttle-mapping claim not officially documented by Kunos

**Research date:** 2026-03-13
**Valid until:** 2026-04-13 (stable -- AC is a mature game, AI_LEVEL behavior will not change)
