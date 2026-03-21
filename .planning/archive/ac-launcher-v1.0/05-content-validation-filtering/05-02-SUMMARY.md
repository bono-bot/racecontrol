---
phase: 05-content-validation-filtering
plan: 02
subsystem: api, game-launcher, catalog
tags: [content-filtering, launch-validation, websocket, manifest-cache, catalog-api]

# Dependency graph
requires:
  - phase: 05-content-validation-filtering/plan-01
    provides: ContentManifest types, content_scanner::scan_ac_content(), AgentMessage::ContentManifest variant
provides:
  - pod_manifests cache in AppState (per-pod ContentManifest storage)
  - get_filtered_catalog() returning pod-specific catalog with session types and max_ai
  - validate_launch_combo() rejecting invalid car/track/session combos
  - Launch validation gates at game_launcher::launch_game() and routes::customer_book_session()
  - customer_ac_catalog API with optional pod_id query param
  - Agent-side manifest sending after Register on connect/reconnect
affects: [06-assist-settings, 07-server-side-pinning, 09-multiplayer]

# Tech tracking
tech-stack:
  added: []
  patterns: [manifest-cache-pattern, launch-validation-gate, enriched-catalog-response]

key-files:
  created: []
  modified:
    - crates/rc-core/src/state.rs
    - crates/rc-core/src/ws/mod.rs
    - crates/rc-core/src/catalog.rs
    - crates/rc-core/src/api/routes.rs
    - crates/rc-core/src/game_launcher.rs
    - crates/rc-agent/src/main.rs

key-decisions:
  - "Fallback mode: None manifest returns full static catalog and allows any launch combo"
  - "max_ai capped at 19 (AC 20-slot limit minus player) with saturating_sub for pit_count"
  - "Auth/billing retry paths intentionally ungated -- they re-use already-validated args"
  - "customer_book_session passes empty session_type to validate_launch_combo -- kiosk experiences are pre-configured"
  - "Empty car/track IDs skip validation (supports non-AC game launches)"

patterns-established:
  - "Manifest cache pattern: agent sends ContentManifest after Register, core caches in RwLock<HashMap>"
  - "Launch validation gate pattern: validate_launch_combo() called before CoreToAgentMessage::LaunchGame at each entry point"
  - "Enriched catalog response: track entries include available_session_types, max_ai, and configs arrays"

requirements-completed: [CONT-01, CONT-02, CONT-04, SESS-07]

# Metrics
duration: 10min
completed: 2026-03-14
---

# Phase 5 Plan 02: Content Validation & Filtering Integration Summary

**Per-pod manifest cache in AppState, filtered catalog API with session type gating and AI slider caps, launch validation at both game_launcher and customer_book_session entry points, and agent manifest sending on connect/reconnect**

## Performance

- **Duration:** 10 min
- **Started:** 2026-03-13T22:36:38Z
- **Completed:** 2026-03-13T22:47:32Z
- **Tasks:** 2
- **Files modified:** 6

## Accomplishments
- AppState caches per-pod ContentManifest via WebSocket handler, enabling content-aware catalog and launch decisions
- get_filtered_catalog() filters cars/tracks to pod manifest and enriches track entries with available_session_types (practice/hotlap always; race/trackday/race_weekend only with AI lines) and max_ai (derived from pit_count, capped at 19)
- validate_launch_combo() rejects invalid car/track/session combos with descriptive error messages; allows anything when no manifest cached (graceful fallback)
- Launch validation gates at both game_launcher::launch_game() (covers admin/split/dashboard paths) and routes::customer_book_session() (covers direct kiosk booking path)
- customer_ac_catalog API accepts optional pod_id query param for per-pod filtering; backward compatible without it
- rc-agent sends ContentManifest immediately after Register on every connect/reconnect
- 13 TDD tests covering filtering, session type gating, pit_count defaults, and validation edge cases

## Task Commits

Each task was committed atomically:

1. **Task 1: AppState pod_manifests + WS handler + get_filtered_catalog + validate_launch_combo + tests** - `b871d58` (feat, TDD)
2. **Task 2: API integration + launch validation gates + agent manifest sending** - `dd638fd` (feat)

## Files Created/Modified
- `crates/rc-core/src/state.rs` - Added pod_manifests: RwLock<HashMap<String, ContentManifest>> to AppState
- `crates/rc-core/src/ws/mod.rs` - Added AgentMessage::ContentManifest handler that stores manifest per pod
- `crates/rc-core/src/catalog.rs` - Added get_filtered_catalog(), validate_launch_combo(), enrich_track_entry(), and 13 tests
- `crates/rc-core/src/api/routes.rs` - Updated customer_ac_catalog with pod_id query param; added validation gate in customer_book_session
- `crates/rc-core/src/game_launcher.rs` - Added launch validation gate before double-launch check in launch_game()
- `crates/rc-agent/src/main.rs` - Added ContentManifest sending after Register on connect/reconnect

## Decisions Made
- **Fallback mode:** When no manifest is cached for a pod, get_filtered_catalog returns the full static catalog and validate_launch_combo allows any combo. This ensures backward compatibility during rolling deploys.
- **max_ai calculation:** max(pit_count across configs) - 1, capped at 19 (AC 20-slot limit). pit_count=None defaults to 19.
- **Intentionally ungated paths:** Auth auto-spawn, billing launch timeout retry, and game_launcher internal retry all re-use previously validated args. Gating them would require threading AppState through auth with no benefit.
- **Empty session_type from kiosk:** customer_book_session passes "" since kiosk experiences are pre-configured combos with known-valid session types.
- **Empty car/track skip:** validate_launch_combo skips car/track validation if the ID is empty, supporting non-AC game launches.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- Content validation and filtering is fully wired end-to-end
- Phase 5 Plan 03 (PWA integration) can consume the enriched catalog API at /customer/ac/catalog?pod_id=pod_X
- max_ai field enables AI slider cap in PWA without additional API calls
- available_session_types enables PWA to hide unavailable session modes per track

## Self-Check: PASSED

- All 6 modified files exist on disk
- Commit b871d58 (Task 1) verified in git log
- Commit dd638fd (Task 2) verified in git log
- 159 rc-core unit tests + 13 integration tests pass
- 76 rc-common tests pass
- rc-agent and rc-core both compile without errors

---
*Phase: 05-content-validation-filtering*
*Completed: 2026-03-14*
