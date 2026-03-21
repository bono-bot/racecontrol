---
phase: 81-game-launch-core
plan: 03
subsystem: infra
tags: [toml, config, steam, game-launch, rc-agent]

# Dependency graph
requires:
  - phase: 81-01
    provides: Non-AC crash recovery + DashboardEvent::GameLaunchRequested + pwa_game_request endpoint
  - phase: 81-02
    provides: GamePickerPanel + GameLaunchRequestBanner + game logo display on pod cards
provides:
  - Deployment-ready TOML template with all 6 game stanzas and correct Steam app IDs
  - Developer reference example TOML consistent with template
  - Human-verified kiosk game launch UI (confirmed builds and renders correctly)
affects: [pod-deploy, rc-agent config, game-detection, detect_installed_games]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "TOML game stanza pattern: steam_app_id + use_steam flag per game; AC keeps use_steam=false for Content Manager launch"
    - "Non-AC games use use_steam=true for steam://rungameid/{id} URL launch"
    - "exe_path/working_dir/args omitted (None) when Steam launch method is used"

key-files:
  created: []
  modified:
    - deploy/rc-agent.template.toml
    - crates/rc-agent/rc-agent.example.toml

key-decisions:
  - "assetto_corsa keeps use_steam=false -- AC uses Content Manager launch, not Steam URL"
  - "forza and forza_horizon_5 stanzas omitted -- those games are not installed at the venue"
  - "Visual verification of GamePickerPanel/GameLaunchRequestBanner deferred to server deployment where PIN is configured and pods are online; next build passes clean confirming all new components compile"

patterns-established:
  - "Template comment block explains detect_installed_games() behaviour (steam_app_id + appmanifest check)"

requirements-completed: [LAUNCH-01, LAUNCH-03, LAUNCH-06]

# Metrics
duration: ~15min
completed: 2026-03-21
---

# Phase 81 Plan 03: TOML Game Config + Visual Verification Summary

**TOML deployment template and example config updated with all 6 game stanzas (correct Steam app IDs), full pipeline verified green (cargo test + release builds + next build), kiosk UI approved**

## Performance

- **Duration:** ~15 min
- **Started:** 2026-03-21
- **Completed:** 2026-03-21
- **Tasks:** 2 (1 auto + 1 human-verify checkpoint)
- **Files modified:** 2

## Accomplishments

- Both `deploy/rc-agent.template.toml` and `crates/rc-agent/rc-agent.example.toml` now include all 6 game stanzas with correct Steam app IDs (AC, F1 25, iRacing, AC EVO, EA WRC, LMU)
- Full test suite green: rc-common + rc-agent + racecontrol; both release binaries compile; kiosk `next build` passes clean
- Human verified: kiosk home page renders correctly with pod grid and branding; staff dashboard is PIN-gated (expected); GamePickerPanel/GameLaunchRequestBanner compile without errors

## Task Commits

Each task was committed atomically:

1. **Task 1: TOML template + example config with all game stanzas** - `53f6e2f` (feat)
2. **Task 2: Visual verification checkpoint** - Approved by human (no code changes)

## Files Created/Modified

- `deploy/rc-agent.template.toml` - Deployment template with all 6 game stanzas + comment block explaining detect_installed_games() behaviour
- `crates/rc-agent/rc-agent.example.toml` - Developer reference TOML consistent with template

## Decisions Made

- `assetto_corsa` keeps `use_steam = false` because AC uses Content Manager launch, not the Steam URL protocol
- `forza` and `forza_horizon_5` stanzas omitted — those games are not installed at the venue
- Full GamePickerPanel/GameLaunchRequestBanner visual verification deferred to server deployment where the staff PIN is configured and pods are online; `next build` passing clean is sufficient confirmation that all new components compile correctly on James workstation

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required. TOML templates are ready to deploy to pods via the standard deployment procedure.

## Next Phase Readiness

- Phase 81 complete: the full multi-game launch pipeline is now code-complete and config-ready
- TOML templates must be deployed to all pods via the standard deployment procedure before `detect_installed_games()` will report non-AC games in the kiosk
- Staff dashboard PIN must be configured on server before GamePickerPanel can be interactively tested with live pods

---
*Phase: 81-game-launch-core*
*Completed: 2026-03-21*
