---
phase: 05-content-validation-filtering
plan: 01
subsystem: protocol, agent
tags: [serde, filesystem, content-scanning, ac-modding, manifest]

# Dependency graph
requires:
  - phase: 04-safety-enforcement
    provides: AgentMessage enum pattern with serde tagged variants
provides:
  - ContentManifest, CarManifestEntry, TrackManifestEntry, TrackConfigManifest structs
  - AgentMessage::ContentManifest protocol variant
  - scan_ac_content() filesystem scanner for AC cars and tracks
  - Per-track-config AI line detection and pit stall count parsing
affects: [05-02 catalog filtering, 05-03 launch validation, 07 curated presets]

# Tech tracking
tech-stack:
  added: [tempfile (dev-dependency for rc-agent tests)]
  patterns: [filesystem scanner with testable path injection, track config detection heuristic]

key-files:
  created:
    - crates/rc-agent/src/content_scanner.rs
  modified:
    - crates/rc-common/src/types.rs
    - crates/rc-common/src/protocol.rs
    - crates/rc-agent/src/main.rs
    - crates/rc-agent/Cargo.toml

key-decisions:
  - "scan_ac_content_at() takes arbitrary Path for testability; scan_ac_content() wraps with hardcoded AC path"
  - "Config detection heuristic: subfolder must contain data/ or ai/ or models.ini to qualify as track config"
  - "NON_CONFIG_DIRS constant (skins, sfx, extension, ui, data, ai) prevents false config detection"
  - "Empty ai/ folder correctly reports has_ai=false -- requires at least one file"
  - "Default layout tracks (no config subfolders) produce config entry with empty string"

patterns-established:
  - "Filesystem scanner pattern: sync function with Path parameter for testability, tempfile for test fixtures"
  - "Track config detection: check for data/ or ai/ or models.ini in subfolders"

requirements-completed: [CONT-07, CONT-05, CONT-06]

# Metrics
duration: 5min
completed: 2026-03-14
---

# Phase 5 Plan 1: Content Manifest Types + Scanner Summary

**ContentManifest types with serde wire format and filesystem scanner for AC cars, tracks, AI lines, and pit counts**

## Performance

- **Duration:** 5 min
- **Started:** 2026-03-13T22:28:08Z
- **Completed:** 2026-03-13T22:33:18Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- ContentManifest/CarManifestEntry/TrackManifestEntry/TrackConfigManifest structs in rc-common with full serde support
- AgentMessage::ContentManifest variant following existing tagged-enum wire format pattern
- content_scanner.rs module with scan_ac_content() that walks AC content directories
- Correct handling of all edge cases: empty AI folders, missing ui_track.json, non-config subfolders, default layout tracks, multi-config tracks, pitboxes string parsing

## Task Commits

Each task was committed atomically:

1. **Task 1: ContentManifest types + AgentMessage variant + serde tests** - `25a6f79` (feat)
2. **Task 2: Content scanner module with filesystem scanning + unit tests** - `3b25d6f` (feat)

## Files Created/Modified
- `crates/rc-common/src/types.rs` - Added ContentManifest, CarManifestEntry, TrackManifestEntry, TrackConfigManifest structs
- `crates/rc-common/src/protocol.rs` - Added ContentManifest import and AgentMessage::ContentManifest variant + 6 serde roundtrip tests
- `crates/rc-agent/src/content_scanner.rs` - New module: scan_ac_content(), scan_cars(), scan_tracks(), detect_track_configs(), check_has_ai(), parse_pit_count() + 15 unit tests
- `crates/rc-agent/src/main.rs` - Added mod content_scanner declaration
- `crates/rc-agent/Cargo.toml` - Added tempfile dev-dependency for test fixtures

## Decisions Made
- scan_ac_content_at(path) exposed for testability alongside scan_ac_content() with hardcoded path
- Config detection uses heuristic: subfolder must contain data/ or ai/ or models.ini (from RESEARCH.md Pitfall 1)
- NON_CONFIG_DIRS constant prevents skins/sfx/extension/ui/data/ai from being treated as configs
- Empty ai/ folder reports has_ai=false (checks for at least one file, not just directory existence)
- Default layout tracks detected by root-level data/ directory when no valid config subfolders found

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- ContentManifest types ready for rc-core to cache per-pod manifests (Plan 05-02)
- scan_ac_content() ready to be called after Register message in rc-agent's WebSocket flow
- Wire format verified: {"type":"content_manifest","data":{"cars":[...],"tracks":[...]}}
- All 91 tests green across rc-common (76) and rc-agent content_scanner (15)

## Self-Check: PASSED

- All 5 files verified present on disk
- Both commits (25a6f79, 3b25d6f) verified in git history
- 76 rc-common tests + 15 rc-agent content_scanner tests all green

---
*Phase: 05-content-validation-filtering*
*Completed: 2026-03-14*
