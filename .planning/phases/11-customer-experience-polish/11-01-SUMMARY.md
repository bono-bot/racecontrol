---
phase: 11-customer-experience-polish
plan: "01"
subsystem: ui
tags: [rust, lock-screen, svg, branding, telemetry, session-summary, html, tdd]

# Dependency graph
requires:
  - phase: 08-pod-lock-screen-hardening
    provides: lock_screen.rs infrastructure, LockScreenState enum, LockScreenManager, serve_lock_screen HTTP server
provides:
  - RP_LOGO_SVG inline SVG constant in lock_screen.rs
  - page_shell_with_bg() for wallpaper-aware HTML rendering
  - Extended SessionSummary state with top_speed_kmh + race_position
  - render_session_summary_page_full() public test helper
  - session_max_speed_kmh accumulator in main.rs event loop
  - Persistent session results (no auto-reload, no blank_timer on SessionEnded)
  - lock_screen_wallpaper_url from SettingsUpdated propagated to LockScreenManager
affects: [phase-12, future-branding, customer-experience]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - TDD with stub functions: compile stubs first so all code compiles, new tests fail at runtime (proper RED)
    - Wallpaper injection via page_shell_with_bg() — keeps page_shell() unchanged for existing callers
    - SVG raw string with r##"..."## to handle # characters in attribute values
    - Telemetry accumulation pattern: local mut variables reset on BillingStarted, passed to summary on SessionEnded

key-files:
  created: []
  modified:
    - crates/rc-agent/src/lock_screen.rs
    - crates/rc-agent/src/main.rs

key-decisions:
  - "SVG raw string uses r##..## delimiter (not r#..#) because fill='#E10600' contains the # terminator"
  - "render_page() accepts wallpaper_url: Option<&str> parameter; render_page_public() passes None (debug server has no wallpaper state)"
  - "Top speed display uses as u32 truncation not {:.0} rounding — 245.5 shows as 245 not 246"
  - "ScreenBlanked state never receives wallpaper URL — hardcoded via render_blank_page() using page_shell() directly"
  - "session_race_position stays None for now — TelemetryFrame does not carry race position, plumbed when AC shared memory position is added"

patterns-established:
  - "SVG branding: use RP_LOGO_SVG constant, embed via {{RP_LOGO_SVG}} placeholder in PAGE_SHELL template"
  - "Wallpaper: page_shell_with_bg(title, content, wallpaper_url) for all customer-facing states, NOT ScreenBlanked"
  - "Session stats: accumulate in main.rs event loop, reset on BillingStarted, pass to show_session_summary on SessionEnded"

requirements-completed: [BRAND-01, BRAND-02, BRAND-03, SESS-01, SESS-02, SESS-03]

# Metrics
duration: 10min
completed: 2026-03-14
---

# Phase 11 Plan 01: Customer Experience Polish Summary

**Inline SVG Racing Point logo in all lock screens, wallpaper URL support via SettingsUpdated, session summary with top speed + race position stats, and persistent results screen (no 15s auto-reload)**

## Performance

- **Duration:** 10 min
- **Started:** 2026-03-14T03:34:10Z
- **Completed:** 2026-03-14T03:44:31Z
- **Tasks:** 2 (TDD: RED + GREEN)
- **Files modified:** 2

## Accomplishments

- Added `RP_LOGO_SVG` constant — checkered flag accent + RACING POINT wordmark in #E10600 red, embedded inline in `PAGE_SHELL` and `render_launch_splash_page`
- Implemented `page_shell_with_bg()` that injects CSS `background-image` when `lock_screen_wallpaper_url` setting is present; `ScreenBlanked` remains pure black
- Extended `SessionSummary` state and `show_session_summary()` with `top_speed_kmh` + `race_position` fields; stat cards render conditionally (hidden when None/zero)
- Removed `setTimeout(location.reload, 15000)` from `SESSION_SUMMARY_PAGE` and `blank_timer_armed` from `SessionEnded` handler — results persist until core sends next state
- Added `session_max_speed_kmh` accumulator in main.rs telemetry loop, reset on `BillingStarted`, passed to summary on `SessionEnded`

## Task Commits

Each task was committed atomically:

1. **Task 1: TDD tests for all 6 requirements in lock_screen.rs** - `f9660ea` (test)
2. **Task 2: Implement all 6 requirements — logo, wallpaper, session stats, SESS-03** - `aecbf65` (feat)

_TDD pattern: RED commit (10 failing tests + compile stubs) → GREEN commit (full implementation, all 26 pass)_

## Files Created/Modified

- `crates/rc-agent/src/lock_screen.rs` — RP_LOGO_SVG constant, PAGE_SHELL logo embed, page_shell_with_bg(), wallpaper_url field on LockScreenManager, extended SessionSummary, render_session_summary_page_full(), SESSION_SUMMARY_PAGE without reload script, 10 new tests
- `crates/rc-agent/src/main.rs` — session_max_speed_kmh + session_race_position accumulators, telemetry accumulation, BillingStarted reset, SessionEnded pass-through, SettingsUpdated wallpaper handler, blank_timer removed from SessionEnded

## Decisions Made

- SVG raw string uses `r##"..."##` delimiter because `fill="#E10600"` contains `#` which would terminate `r#"..."#`
- `render_page_public()` passes `None` as wallpaper_url since debug server doesn't have wallpaper state — clean separation
- Top speed display truncates with `as u32` (245.5 → 245) not `{:.0}` rounding (245.5 → 246) for consistent UX
- `session_race_position` stays `None` — `TelemetryFrame` doesn't carry race position yet; AC shared memory `NORMALIZED_CAR_POSITION` constant exists but isn't plumbed through

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Plumbed wallpaper_url through HTTP server**
- **Found during:** Task 2 (implementation)
- **Issue:** Plan specified `set_wallpaper_url()` on LockScreenManager but `serve_lock_screen()` had no access to it — wallpaper would be set but never rendered
- **Fix:** Added `wallpaper_url: Arc<Mutex<Option<String>>>` field to `LockScreenManager`, passed as parameter to `serve_lock_screen()`, cloned per connection, passed to `render_page()` which then passes to render functions
- **Files modified:** `crates/rc-agent/src/lock_screen.rs`
- **Verification:** `wallpaper_url_renders_in_css` test passes

**2. [Rule 1 - Bug] Fixed `r#"..."#` raw string termination by SVG color attributes**
- **Found during:** Task 2 (first build attempt)
- **Issue:** `const RP_LOGO_SVG: &str = r#"..fill="#E10600"..."#` — the `"#` inside fill attribute terminated the raw string early, causing 6 compile errors
- **Fix:** Changed delimiter to `r##"..."##` which requires `"##` to terminate, avoiding false terminations
- **Files modified:** `crates/rc-agent/src/lock_screen.rs`
- **Verification:** Build succeeds, SVG renders correctly

**3. [Rule 1 - Bug] Test struct initializations needed wallpaper_url field**
- **Found during:** Task 2 (test compilation after adding field to struct)
- **Issue:** Existing tests that construct `LockScreenManager` directly (not via `::new()`) failed to compile after adding `wallpaper_url` field
- **Fix:** Added `wallpaper_url: std::sync::Arc::new(std::sync::Mutex::new(None))` to 3 test struct literals
- **Files modified:** `crates/rc-agent/src/lock_screen.rs` (test section)
- **Verification:** All 26 tests pass

**4. [Rule 1 - Bug] `{:.0}` rounding vs truncation for top speed display**
- **Found during:** Task 2 (single test still failing after implementation)
- **Issue:** `{:.0}` formats 245.5f32 as "246" (rounds up), but test expects "245"
- **Fix:** Changed to `spd as u32` truncation so 245.5 → 245
- **Files modified:** `crates/rc-agent/src/lock_screen.rs`
- **Verification:** `session_summary_shows_top_speed` test passes

---

**Total deviations:** 4 auto-fixed (1 missing plumbing, 3 bugs)
**Impact on plan:** All auto-fixes necessary for correctness. No scope creep.

## Issues Encountered

None of significance — all issues resolved via auto-fix rules during Task 2.

## User Setup Required

None - no external service configuration required. Wallpaper URL is configured via the `lock_screen_wallpaper_url` key in the `SettingsUpdated` core message.

## Next Phase Readiness

- Phase 11 Plan 01 complete — Racing Point branding consistent across all lock screen states
- Session summary shows top speed (when telemetry records > 0 km/h) and race position (when `Some`)
- Wallpaper URL can be set from rc-core SettingsUpdated message using key `lock_screen_wallpaper_url`
- Race position accumulator wired but stays `None` until `TelemetryFrame` carries position data from AC shared memory

---
*Phase: 11-customer-experience-polish*
*Completed: 2026-03-14*
