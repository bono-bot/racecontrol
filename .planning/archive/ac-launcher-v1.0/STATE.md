---
gsd_state_version: 1.0
milestone: v1.0
milestone_name: milestone
status: complete
stopped_at: v1.0 milestone complete — all 9 phases, 20 plans executed
last_updated: "2026-03-14T05:15:00.000Z"
last_activity: 2026-03-14 -- Phase 9 complete + gap fix (ai_level wiring). v1.0 DONE.
progress:
  total_phases: 9
  completed_phases: 9
  total_plans: 20
  completed_plans: 20
---

---
gsd_state_version: 1.0
milestone: v1.0
milestone_name: milestone
status: executing
stopped_at: Completed 09-03-PLAN.md
last_updated: "2026-03-14T05:01:01Z"
last_activity: 2026-03-14 -- Plan 09-03 complete (PWA lobby enrichment with track/car/AI info)
progress:
  total_phases: 9
  completed_phases: 9
  total_plans: 21
  completed_plans: 21
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-03-13)

**Core value:** When a customer selects a session and hits go, the game launches with exactly the settings they chose, billing starts only when they're actually driving, and they never see an option that doesn't work.
**Current focus:** v1.0 Milestone COMPLETE. All 9 phases executed successfully.

## Current Position

Phase: 9 of 9 (Multiplayer Enhancement)
Plan: 3 of 3 complete in phase 9
Status: v1.0 MILESTONE COMPLETE
Last activity: 2026-03-14 -- Phase 9 complete + gap fix. All 9 phases done.

Progress: [##########] 100%

## Performance Metrics

**Velocity:**
- Total plans completed: 21
- Average duration: 7.8min
- Total execution time: 2.37 hours

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 01-session-types-race-mode | 2 | 16min | 8min |
| 02-difficulty-tiers | 1 | 8min | 8min |
| 03-billing-synchronization | 3 | 36min | 12min |
| 04-safety-enforcement | 2 | 20min | 10min |
| 05-content-validation-filtering | 2 | 15min | 7.5min |
| 06-mid-session-controls | 3 | 27min | 9min |
| 07-curated-presets | 2/2 | 11min | 5.5min |
| 08-staff-pwa-integration | 2/2 | 11min | 5.5min |
| 09-multiplayer-enhancement | 3/3 | 23min | 7.7min |

**Recent Trend:**
- Last 5 plans: 08-01 (5min), 08-02 (6min), 09-01, 09-02, 09-03 (2min)
- Trend: Steady -- plan complexity varies

*Updated after each plan completion*

## Accumulated Context

### Decisions

Decisions are logged in PROJECT.md Key Decisions table.
Recent decisions affecting current work:

- Research: ~70% infrastructure exists; work is gap-filling not greenfield
- Research: AC_STATUS shared memory (value 2 = LIVE) is the billing trigger
- Research: Direct acs.exe launch for single-player, acmanager:// URI for multiplayer
- Research: AI_LEVEL tuning values need real-pod testing (Phase 2)
- Research: Multiplayer AI fillers may need CSP server plugins (Phase 9 risk)
- Plan 01-01: Used String session_type (not SessionType enum) for forward compatibility with trackday/weekend
- Plan 01-01: Composable INI builder uses writeln! into String buffer for maintainability
- Plan 01-01: effective_ai_cars() caps all modes at 19 AI (AC 20-slot limit)
- Plan 01-01: 60-name international AI driver pool shuffled per session
- Plan 01-02: Track Day defaults to 12 mixed GT3/supercar AI from TRACKDAY_CAR_POOL
- Plan 01-02: Race Weekend time allocation uses saturating_sub + max(1) for minimum 1 minute race
- Plan 01-02: AI SKIN= left empty for AC to pick random installed skin
- Plan 01-02: effective_ai_cars() centralizes trackday defaults + 19-cap clamping for all modes
- Plan 02-01: DifficultyTier controls AI_LEVEL only -- assists remain independent (user decision)
- Plan 02-01: AI_AGGRESSION deferred -- uncertain CSP support
- Plan 02-01: Default ai_level is 87 (Semi-Pro midpoint) for backward compat
- Plan 02-01: Session-wide ai_level overrides per-car values in all modes
- Plan 02-01: DIFF-03/DIFF-04 (assist presets per tier) superseded by user decision
- Plan 03-01: All new BillingTick/BillingSessionInfo fields are Option<T> for rolling deploy backward compat
- Plan 03-01: minutes_to_next_tier uses integer division (floor) -- at 29:59 shows "1 minute" remaining
- Plan 03-01: compute_session_cost() is a pure function (no DB/state) -- testable and fast per-tick
- Plan 03-01: PausedGamePause has separate 10-min timeout, independent of disconnect pause logic
- Plan 03-01: elapsed_seconds mirrors driving_seconds for backward compat with existing billing code
- Plan 03-03: Deferred billing uses placeholder ID (deferred-UUID) returned to kiosk/PWA; real session on Live
- Plan 03-03: Reservation linking deferred from auth-time to actual billing start in start_billing_session()
- Plan 03-03: AcStatus::Replay treated same as Pause for billing (customer not driving)
- Plan 03-03: AcStatus::Off ends billing session as EndedEarly (game exit = session end)
- Plan 03-03: Launch timeout retry sends LaunchGame to agent; agent-side retry handled by LaunchState machine
- Plan 03-03: check_launch_timeouts_from_manager() helper enables unit testing without full AppState
- Plan 03-02: Taxi meter detects new mode via elapsed_seconds > 0 || waiting_for_game || paused; falls back to countdown
- Plan 03-02: BillingStarted uses allocated_seconds >= 10800 threshold for open-ended billing v2
- Plan 03-02: AC STATUS debounce at 1 second prevents pause/unpause flapping (RESEARCH.md Pitfall 3)
- Plan 03-02: STATUS polling guarded by game_process.is_some() prevents stale shared memory reads (Pitfall 1)
- Plan 03-02: format_cost uses floor division (paise / 100) for customer-friendly rounding
- Plan 04-01: DAMAGE=0 hardcoded in all three INI paths (race.ini, assists.ini, server_cfg.ini) -- params.conditions.damage ignored
- Plan 04-01: verify_safety_content() is testable string-based function; verify_safety_settings() wraps it with file I/O
- Plan 04-01: FfbZeroed/GameCrashed are log-only on core side for now; Plan 02 will add FFB zeroing logic on agent side
- Plan 04-02: FFB zero is awaited via spawn_blocking().await before any game.stop() -- not fire-and-forget
- Plan 04-02: 500ms delay between FFB zero and game kill gives HID USB command time to reach wheelbase
- Plan 04-02: enforce_safe_state() called separately after FFB zero (no longer bundled in same spawn_blocking)
- Plan 04-02: Crash during billing zeros FFB immediately then arms 30s recovery timer (was: timer only)
- Plan 04-02: Physical Pod 8 verification deferred until full project completion (code audit approved)
- Plan 05-01: scan_ac_content_at() takes arbitrary Path for testability; scan_ac_content() wraps with hardcoded AC path
- Plan 05-01: Config detection heuristic: subfolder must contain data/ or ai/ or models.ini to qualify as track config
- Plan 05-01: NON_CONFIG_DIRS constant prevents false config detection (skins, sfx, extension, ui, data, ai)
- Plan 05-01: Empty ai/ folder reports has_ai=false -- requires at least one file
- Plan 05-01: Default layout tracks produce config entry with empty string when no valid config subfolders found
- Plan 05-02: Fallback mode: None manifest returns full static catalog and allows any launch combo
- Plan 05-02: max_ai = min(max_pit_count - 1, 19) with saturating_sub; pit_count=None defaults to 19
- Plan 05-02: Auth/billing retry paths intentionally ungated -- re-use already-validated args
- Plan 05-02: customer_book_session passes empty session_type to validate_launch_combo (kiosk pre-configured)
- Plan 05-02: Empty car/track IDs skip validation (supports non-AC game launches)
- Plan 06-01: SendInput helpers in ac_launcher::mid_session submodule (colocated with AC launch code)
- Plan 06-01: set_gain() uses send_vendor_cmd_to_class() with CLASS_AXIS parameter (backward-compat with existing send_vendor_cmd)
- Plan 06-01: read_assist_state() implemented as SimAdapter trait default method for dyn dispatch
- Plan 06-01: Stability control excluded -- AC has no keyboard shortcut (user decision DIFF-09)
- Plan 06-01: SetFfb handler: numeric values use HID gain, non-numeric presets fall back to legacy INI
- Plan 06-01: last_ffb_percent cached in main.rs scope (default 70%) -- FFB has no shared memory readback
- Plan 06-02: assist_cache uses tokio::sync::RwLock (not std) for consistency with all other AppState fields
- Plan 06-02: GET /assist-state returns cached values immediately AND triggers background QueryAssistState refresh
- Plan 06-02: Default CachedAssistState: abs=0, tc=0, auto_shifter=true, ffb_percent=70 (matches agent defaults)
- Plan 06-02: No stability control endpoint -- AC has no runtime mechanism, excluded by design
- Plan 06-02: FFB endpoint backward compatible -- percent field takes priority, falls back to legacy preset field
- Plan 06-03: No stability control toggle in PWA -- AC has no runtime mechanism (per locked decision DIFF-09)
- Plan 06-03: Toggles send POST immediately on tap -- no Apply button (per locked user decision)
- Plan 06-03: FFB slider visual update instant, API call debounced 500ms (per locked user decision)
- Plan 06-03: Sheet fetches actual pod state on open via getAssistState (not cached last-sent values)
- Plan 06-03: Optimistic toggle UI with revert-on-API-failure for responsive feel
- Plan 07-01: PresetEntry named distinctly from AcPresetSummary (multiplayer server presets) to avoid collision
- Plan 07-01: 4 featured presets (2 Race, 1 Casual, 1 Challenge) for Staff Picks hero section
- Plan 07-01: TypeScript presets field optional (?) for backward compat during rolling deploy
- Plan 07-01: Race/trackday presets excluded when track has_ai=false (same pattern as validate_launch_combo)
- Plan 07-02: showPresets boolean state gates preset screen vs wizard -- avoids shifting step indices
- Plan 07-02: Catalog loaded eagerly in PWA (moved from lazy step-4 load) so preset cards display immediately
- Plan 07-02: Category gradients: Race=red, Casual=blue, Challenge=purple -- consistent across PWA and kiosk
- Plan 07-02: Kiosk uses "presets" ConfigStep as new initial step instead of "game"
- Plan 07-02: Visual verification deferred to next on-site test (TypeScript compilation verified)
- Plan 08-01: qualification renamed to hotlap in SessionType union and kiosk UI (per locked decision)
- Plan 08-01: session_type: Option<String> with serde(default) for backward compat -- old clients default to practice
- Plan 08-01: Double-write session_type in routes.rs post-processing block (harmless, ensures consistency)
- Plan 08-01: Staff launch path (game_launcher) already reads session_type -- no changes needed
- Plan 08-02: GameConfigurator session_type step replaces mode step entirely -- multiplayer stays disabled
- Plan 08-02: PWA SessionTypeStep shows 5 types + visually distinct "Race with Friends" multiplayer card (dashed blue border)
- Plan 08-02: Track filtering uses graceful fallback: undefined available_session_types shows all tracks
- Plan 08-02: Session type display uses capitalize + underscore-to-space for readable labels
- Plan 09-01: AI names moved to rc-common (shared crate) for single source of truth between agent and core
- Plan 09-01: AcEntrySlot.ai_mode uses serde(default, skip_serializing_if) for backward compat
- Plan 09-01: GroupSessionInfo track/car/ai_count/difficulty_tier all Option<T> for rolling deploy
- Plan 09-01: AI filler count = pit_count - human_count, capped at 19 (AC 20-slot limit)
- Plan 09-01: AI_LEVEL mapped from difficulty_tier via Phase 2 midpoints, default SemiPro (87)
- Plan 09-01: extra_cfg.yml written to server_dir root (AssettoServer reads from working directory)
- Plan 09-01: LaunchGame sends JSON with game_mode "multi" instead of raw acmanager:// URI
- Plan 09-01: track/car/ai_count stored on group_sessions table for lobby enrichment
- Plan 09-02: MultiplayerBillingWait uses HashSet for expected_pods/live_pods (O(1) membership checks)
- Plan 09-02: timeout_spawned flag prevents duplicate 60s timeout spawns per group
- Plan 09-02: Auth callers query group_session_members with graceful .ok().flatten() fallback
- Plan 09-02: AcStatus::Off cleans up multiplayer_waiting for pods that crash during loading
- Plan 09-03: All new GroupSessionInfo TS fields optional (?) for backward compat with old API responses
- Plan 09-03: Info cards hidden entirely when track field absent (graceful degradation)
- Plan 09-03: Status count uses validated filter to show who still needs to check in

### Pending Todos

None yet.

### Blockers/Concerns

- AI_AGGRESSION support uncertain across CSP versions (deferred from Phase 2)
- Mid-session assist changes via assists.ini RESOLVED -- Plan 06-01 uses SendInput (Ctrl+A/T/G) instead of INI writes
- Multiplayer AI on dedicated server may require CSP plugins (affects Phase 9)

## Session Continuity

Last session: 2026-03-14T05:01:01.192Z
Stopped at: Completed 09-03-PLAN.md
Resume file: None
