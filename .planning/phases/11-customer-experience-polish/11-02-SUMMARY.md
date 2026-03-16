---
phase: 11-customer-experience-polish
plan: "02"
subsystem: ui
tags: [next.js, kiosk, settings, branding, wallpaper]

# Dependency graph
requires:
  - phase: 11-01
    provides: lock_screen_wallpaper_url support in rc-agent (page_shell_with_bg, SettingsUpdated handler)
provides:
  - Wallpaper URL input in kiosk settings page (Pod Display section)
  - Staff UI to configure lock_screen_wallpaper_url via kiosk dashboard
affects: [BRAND-02, customer-experience]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - Reuse handleSettingChange(key, value) pattern for arbitrary KioskSettings keys
    - KioskSettings index signature [key: string]: string enables new keys without type changes

key-files:
  created: []
  modified:
    - kiosk/src/app/settings/page.tsx

key-decisions:
  - "No API, type, or backend changes needed — KioskSettings index signature + updateSettings already handle arbitrary keys"
  - "Pod Display section positioned between Spectator Display and Experiences for logical grouping"
  - "Input type=url provides browser validation hint while still allowing blank values"

requirements-completed: [BRAND-02]

# Metrics
duration: 1min
completed: 2026-03-14
---

# Phase 11 Plan 02: Wallpaper URL Staff UI Summary

**Staff-facing wallpaper URL input added to kiosk settings page — completes BRAND-02 end-to-end chain from dashboard to pod lock screen**

## Performance

- **Duration:** 1 min
- **Started:** 2026-03-14T03:47:14Z
- **Completed:** 2026-03-14T03:47:57Z
- **Tasks:** 1
- **Files modified:** 1

## Accomplishments

- Added "Pod Display" section to `kiosk/src/app/settings/page.tsx` positioned between Spectator Display and Experiences
- Section contains a labeled text input (type=url) for `lock_screen_wallpaper_url` with placeholder and helper text explaining 10-second propagation delay
- onChange calls `handleSettingChange("lock_screen_wallpaper_url", e.target.value)` — identical pattern to all other settings inputs
- No changes needed to `api.ts`, `types.ts`, or any Rust backend — existing generic key-value infrastructure handles it all
- Completes BRAND-02 end-to-end: staff enters URL in settings -> `PUT /kiosk/settings` -> racecontrol saves + broadcasts `SettingsUpdated` -> rc-agent sets `wallpaper_url` -> `page_shell_with_bg()` renders CSS `background-image` on pod lock screens

## Task Commits

1. **Task 1: Add Pod Display section with wallpaper URL input** - `8c86f5b` (feat)

## Files Created/Modified

- `kiosk/src/app/settings/page.tsx` — added "Pod Display" section (21 lines) with wallpaper URL input between Spectator Display and Experiences sections

## Decisions Made

- No API/type/backend changes required — `KioskSettings` has `[key: string]: string` index signature and `updateSettings` accepts `Partial<KioskSettings>`, so `lock_screen_wallpaper_url` works out of the box
- `Pod Display` section positioned logically between Spectator Display (other display settings) and Experiences (content settings)
- Helper text explicitly mentions "10 seconds" propagation time and "default Racing Point gradient" fallback — both relevant for staff operating the venue

## Deviations from Plan

None — plan executed exactly as written. Single file change, zero backend modifications, TypeScript compilation clean.

## Issues Encountered

None.

## User Setup Required

None — the feature is fully operational. Staff can immediately use the wallpaper URL input in the kiosk Settings page. The end-to-end chain (Plan 01 + Plan 02) is now complete.

## Next Phase Readiness

- BRAND-02 fully complete — wallpaper URL configurable from staff dashboard, rendered on pod lock screens
- Phase 11 all plans complete — customer experience polish done
- No open items from this plan

---
*Phase: 11-customer-experience-polish*
*Completed: 2026-03-14*

## Self-Check: PASSED

- FOUND: `kiosk/src/app/settings/page.tsx`
- FOUND: `.planning/phases/11-customer-experience-polish/11-02-SUMMARY.md`
- FOUND: commit `8c86f5b`
