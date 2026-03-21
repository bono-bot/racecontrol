---
phase: 06-mid-session-controls
plan: 02
subsystem: api
tags: [axum, api-routes, websocket, caching, assists, ffb, mid-session]

# Dependency graph
requires:
  - phase: 06-mid-session-controls
    provides: Plan 01 protocol messages (SetAssist, SetFfbGain, QueryAssistState, AssistChanged, FfbGainChanged, AssistState)
provides:
  - POST /pods/{pod_id}/assists endpoint for unified assist toggle (abs, tc, transmission)
  - Updated POST /pods/{pod_id}/ffb accepting numeric percent alongside legacy preset
  - GET /pods/{pod_id}/assist-state returning cached CachedAssistState values with background refresh
  - WebSocket handlers for AssistChanged, FfbGainChanged, AssistState updating AppState assist_cache
  - CachedAssistState struct with Default (abs=0, tc=0, auto_shifter=true, ffb_percent=70)
affects:
  - 06-03 (PWA bottom sheet controls will call these API endpoints)
  - 08-staff-pwa-integration (staff kiosk may use assist-state endpoint)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Async assist cache: tokio::sync::RwLock<HashMap<String, CachedAssistState>> on AppState"
    - "Background refresh pattern: GET endpoint reads cache immediately AND sends QueryAssistState to agent"
    - "Dual-mode FFB endpoint: numeric percent sends SetFfbGain, string preset sends legacy SetFfb"

key-files:
  created: []
  modified:
    - crates/rc-core/src/state.rs
    - crates/rc-core/src/api/routes.rs
    - crates/rc-core/src/ws/mod.rs

key-decisions:
  - "assist_cache uses tokio::sync::RwLock (not std::sync::RwLock) for consistency with all other AppState fields"
  - "GET /assist-state returns cached values immediately AND triggers background QueryAssistState -- bridges async gap between PWA request and agent response"
  - "Default CachedAssistState: abs=0, tc=0, auto_shifter=true, ffb_percent=70 (matches agent defaults)"
  - "No stability control endpoint -- AC has no runtime mechanism, excluded by design per user decision"
  - "Backward compatible FFB endpoint: percent field takes priority, falls back to legacy preset field"

patterns-established:
  - "Cache-then-refresh: return cached state immediately, fire background query for freshness"
  - "Dual-mode endpoint: check for new field first, fall back to legacy field for backward compatibility"

requirements-completed: [DIFF-06, DIFF-07, DIFF-08, DIFF-10]

# Metrics
duration: 6min
completed: 2026-03-14
---

# Phase 6 Plan 02: Core API Routes and Assist State Cache Summary

**POST /assists and GET /assist-state API routes with per-pod CachedAssistState in AppState, WebSocket handlers populating cache from agent confirmations, and backward-compatible FFB percent on existing /ffb endpoint**

## Performance

- **Duration:** 6 min
- **Started:** 2026-03-14T00:22:24Z
- **Completed:** 2026-03-14T00:28:17Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- Three new API capabilities: POST /assists for unified assist toggle, GET /assist-state for cached pod state, and numeric FFB percent on existing /ffb endpoint
- CachedAssistState struct and assist_cache field added to AppState for real-time assist value tracking
- WebSocket handlers for all 3 new AgentMessage variants (AssistChanged, FfbGainChanged, AssistState) updating the assist cache on every state change
- GET /assist-state returns concrete values (abs, tc, auto_shifter, ffb_percent), never a bare acknowledgment -- satisfying the locked decision "Drawer shows actual pod state when opened"

## Task Commits

Each task was committed atomically:

1. **Task 1: Add API routes for assist changes, FFB gain, and assist state query with cached response** - `9a3fade` (feat)
2. **Task 2: Handle new AgentMessage variants in WebSocket handler -- update assist cache** - `b299e2e` (feat)

## Files Created/Modified
- `crates/rc-core/src/state.rs` - CachedAssistState struct with Default impl, assist_cache field on AppState, initialized in constructor
- `crates/rc-core/src/api/routes.rs` - New routes (POST /assists, GET /assist-state), updated set_pod_ffb for numeric percent, set_pod_assists and get_pod_assist_state handlers
- `crates/rc-core/src/ws/mod.rs` - Match arms for AssistChanged, FfbGainChanged, AssistState with cache updates, logging, and activity tracking

## Decisions Made
- **tokio::sync::RwLock for assist_cache:** All other AppState fields use tokio::sync::RwLock, so using the same for consistency and avoiding mixed sync/async lock patterns.
- **Background refresh on GET:** GET /assist-state reads cache immediately for fast response AND sends QueryAssistState to agent so cache gets refreshed in the background. Next drawer open will have even fresher data.
- **Backward compatible FFB:** The /ffb endpoint checks for `percent` field first (new mid-session path), then falls back to `preset` field (existing behavior). No breaking change for existing callers.
- **No stability control:** Intentionally excluded from the valid assist_type list -- AC has no runtime mechanism for stability control.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All core API routes and WebSocket handlers are in place for Plan 03 (PWA bottom sheet controls)
- PWA can call POST /pods/{pod_id}/assists, POST /pods/{pod_id}/ffb, and GET /pods/{pod_id}/assist-state
- 172 rc-core tests + 85 rc-common tests all passing (257 total)
- No stability control endpoint exists (as designed)

---
*Phase: 06-mid-session-controls*
*Completed: 2026-03-14*
