# Phase 2: Difficulty Tiers - Context

**Gathered:** 2026-03-13
**Status:** Ready for planning

<domain>
## Phase Boundary

Provide 5 racing-themed difficulty tiers (Rookie / Amateur / Semi-Pro / Pro / Alien) that control AI strength via AI_LEVEL, plus a slider for fine-tuning. Assists are NOT bundled with difficulty — they remain independent customer choices. This phase adds the tier system to `AcLaunchParams` and the INI builder; UI/PWA changes are for Phase 8.

</domain>

<decisions>
## Implementation Decisions

### Tier Scope — AI Strength Only
- Difficulty tiers control **AI_LEVEL only** — not assists, not aggression
- Assists (ABS, TC, stability, autoclutch, ideal_line) are **completely independent** parameters
- A customer can pick Alien difficulty (AI_LEVEL=100) with all assists enabled, or Rookie (AI_LEVEL=70) with no assists
- AI_AGGRESSION is **not used** in this phase — uncertain CSP support, not worth the risk

### Named Tiers + Slider
- 5 named tier presets: **Rookie / Amateur / Semi-Pro / Pro / Alien**
- Plus a **slider (0-100)** for fine-tuning AI_LEVEL directly
- Selecting a tier preset sets the slider to that tier's midpoint value
- Customer can freely adjust the slider after selecting a tier

### Slider-Tier Label Behavior
- Slider **always shows the nearest tier name** as the customer drags
- Tier name updates dynamically based on the current AI_LEVEL value
- E.g., dragging to 87 shows "Semi-Pro" since it falls in the 85-89 range

### AI_LEVEL Ranges (Community Standard)
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

</decisions>

<specifics>
## Specific Ideas

- Named tiers use racing terminology to feel immersive: Rookie, Amateur, Semi-Pro, Pro, Alien
- "Alien" is a well-known sim racing term for the fastest drivers — customers will recognize it
- The slider gives advanced users (staff, regular customers) precise control without removing the simple tier buttons for newcomers
- Tier label updating in real-time as slider moves gives immediate feedback on what range they're in

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- `AcLaunchParams.ai_level` (ac_launcher.rs:32): Already exists per AiCarSlot, default 90
- `AcAids` struct (ac_launcher.rs:122-134): Independent assists — abs, tc, stability, autoclutch, ideal_line
- `write_assists_section()` (ac_launcher.rs:400): Already writes assists to INI independently
- `write_assists_ini()` (ac_launcher.rs:681): Writes separate assists.ini file

### Established Patterns
- `default_ai_level()` returns 90 — currently hardcoded, Phase 2 makes this tier-aware
- AI_LEVEL is written per-car in `write_car_sections()` — already supports variable levels
- `effective_ai_cars()` generates AI slots with ai_level from params — tier midpoint feeds in here

### Integration Points
- `AcLaunchParams` receives JSON from WebSocket `LaunchGame` message (main.rs:903-995)
- PWA/kiosk sends `ai_level` as part of launch_args JSON — tier selection in UI maps to this value
- Backend (rc-core) validates and passes through; rc-agent consumes for INI generation

</code_context>

<deferred>
## Deferred Ideas

- AI_AGGRESSION per-tier — defer until CSP support is verified on pods (possibly Phase 2.1 if confirmed working)
- Mid-session difficulty adjustment — Phase 6 covers mid-session controls
- Per-AI-car difficulty variation (e.g., mix of Pro and Amateur AI in same race) — future enhancement

</deferred>

---

*Phase: 02-difficulty-tiers*
*Context gathered: 2026-03-13*
