---
phase: 258-staff-controls-deployment-safety
plan: 02
subsystem: api
tags: [rust, axum, deploy, shift-handoff, ota, billing, weekend-lock]

# Dependency graph
requires:
  - phase: 258-staff-controls-deployment-safety-01
    provides: STAFF-01/02/03/04 discount approval, daily override report, cash drawer reconciliation
  - phase: 258-staff-controls-deployment-safety-03
    provides: DEPLOY-02/04/05 graceful shutdown, session recovery, WS dedup
provides:
  - STAFF-05 shift handoff workflow (POST /staff/shift-handoff + GET /staff/shift-briefing)
  - DEPLOY-01 session drain verified and documented with DEPLOY-01 comments
  - DEPLOY-03 weekend peak-hour deploy window lock (is_deploy_window_locked)
affects:
  - any future staff workflow endpoints consuming shift-handoff audit trail
  - any deploy tooling calling deploy_rolling() directly

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "is_deploy_window_locked(force, actor) -> Result<(), String>: standalone fn for weekend gate"
    - "chrono IST offset: Utc::now() + Duration::hours(5) + Duration::minutes(30)"
    - "shift handoff via log_admin_action with action_type=shift_handoff for audit trail"

key-files:
  created: []
  modified:
    - crates/racecontrol/src/api/routes.rs
    - crates/racecontrol/src/deploy.rs
    - crates/racecontrol/src/ws/mod.rs

key-decisions:
  - "Shift handoff returns 400 (not 403) when active sessions exist without incoming_staff_id — it is a validation error, not auth error"
  - "shift_briefing queries audit_log by action_type='shift_handoff' — reuses log_admin_action path from 258-01"
  - "is_deploy_window_locked is a standalone synchronous fn — no AppState needed, easy to unit test"
  - "deploy_rolling signature adds force+actor params (not AppState config) — keeps hot path unblocked when no window conflict"
  - "OTA deploy uses query param ?force=true (not JSON body) because OTA handler takes raw TOML body"
  - "deploy_single_pod also gated by window lock — consistent safety across all deploy entry points"
  - "Dashboard WS deploy path passes force=false, actor='dashboard' — no force override from dashboard"
  - "DEPLOY-01 verified: 3 call sites in billing.rs+billing_fsm.rs all call check_and_trigger_pending_deploy after active_timers removal"

patterns-established:
  - "Force override pattern: fn(force: bool, actor: &str) -> Result<(), String> with WARN log on override"
  - "IST conversion: UTC + 5h30m using chrono::Duration addition (not FixedOffset) for simplicity"

requirements-completed: [STAFF-05, DEPLOY-01, DEPLOY-03]

# Metrics
duration: 25min
completed: 2026-03-29
---

# Phase 258 Plan 02: Shift Handoff & Deploy Window Lock Summary

**Shift handoff API with active-session acknowledgment gate, DEPLOY-01 session drain verified across 3 billing hook points, DEPLOY-03 weekend 18:00-23:00 IST deploy lock with force override across all deploy entry points**

## Performance

- **Duration:** 25 min
- **Started:** 2026-03-29T10:00:00Z
- **Completed:** 2026-03-29T10:25:00Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- STAFF-05: POST /staff/shift-handoff validates active sessions require incoming_staff_id; inserts audit_log entry via log_admin_action
- STAFF-05: GET /staff/shift-briefing returns active session snapshot + last handoff notes from audit_log
- DEPLOY-01: Verified all 3 session-end hook points in billing.rs (tick loop, stop_billing path) and billing_fsm.rs (FSM cancel) correctly call check_and_trigger_pending_deploy; added documentation comments
- DEPLOY-03: is_deploy_window_locked(force, actor) blocks weekend 18:00-22:59 IST deploys; returns 423 Locked to callers
- All three deploy handlers gated: deploy_rolling_handler (body JSON), deploy_single_pod (body JSON), ota_deploy_handler (query param)
- ws/mod.rs dashboard deploy path updated to match new deploy_rolling signature (force=false, actor='dashboard')
- 3 unit tests for is_deploy_window_locked

## Task Commits

1. **Task 1: Shift handoff workflow** - `1b038a5f` (feat)
2. **Task 2: Deploy window lock and DEPLOY-01 verification** - `a46f7c49` (feat)

## Files Created/Modified
- `crates/racecontrol/src/api/routes.rs` - shift_handoff_handler + shift_briefing_handler + route registrations + force field on DeployRequest + ota_deploy_handler query param + deploy_single_pod window check
- `crates/racecontrol/src/deploy.rs` - is_deploy_window_locked(), DEPLOY-01 doc comment on check_and_trigger_pending_deploy, force+actor params on deploy_rolling(), DEPLOY-03 comment on rolling fn
- `crates/racecontrol/src/ws/mod.rs` - updated deploy_rolling call to pass force=false, actor='dashboard'

## Decisions Made
- Shift handoff uses `log_admin_action` with `action_type='shift_handoff'` — reuses the same audit path as STAFF-01/02/03/04 from 258-01. Consistent with audit query patterns.
- `is_deploy_window_locked` returns `Result<(), String>` (not a bool + separate message) so callers can return the message directly as the 423 response body. Clean single-fn protocol.
- OTA handler takes raw TOML body, no JSON, so `force` goes in a query parameter `?force=true` rather than body JSON. Consistent with REST conventions for TOML uploads.
- Peak hours defined as `(18..=22).contains(&hour)` — this covers 18:00 through 22:59 (23:00 is excluded per "6 PM to 11 PM" venue closing time).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] deploy_single_pod missing window check**
- **Found during:** Task 2 (reading deploy handlers to add window check)
- **Issue:** Plan specified window check for deploy_rolling_handler and ota_deploy_handler, but deploy_single_pod is a third deploy entry point that bypasses the check. Leaving it unprotected would be a correctness gap.
- **Fix:** Added is_deploy_window_locked call at the top of deploy_single_pod, same pattern as other handlers.
- **Files modified:** crates/racecontrol/src/api/routes.rs
- **Committed in:** a46f7c49 (Task 2 commit)

**2. [Rule 3 - Blocking] ws/mod.rs had stale deploy_rolling call**
- **Found during:** Task 2 (after adding force+actor params to deploy_rolling signature)
- **Issue:** ws/mod.rs had a direct call to deploy_rolling with the old 2-arg signature; broke compilation.
- **Fix:** Updated to pass force=false, actor="dashboard" — dashboard-initiated deploys do not override the window.
- **Files modified:** crates/racecontrol/src/ws/mod.rs
- **Committed in:** a46f7c49 (Task 2 commit)

---

**Total deviations:** 2 auto-fixed (1 missing critical path, 1 blocking compilation)
**Impact on plan:** Both fixes were essential for correctness and compilation. No scope creep.

## Issues Encountered
- Integration test suite (cargo test --test integration) failed with 3 failures due to pre-existing compile errors in `fleet_alert.rs` and `telemetry_store.rs` from another session's uncommitted work. These errors existed at the base commit before this plan's changes and are confirmed out of scope (deviation rules: fix only issues caused by current task changes). Documented in deferred-items.

## Next Phase Readiness
- STAFF-05, DEPLOY-01, DEPLOY-03 all complete
- All requirements for phase 258 are now complete: STAFF-01–05, DEPLOY-01–05
- Phase 258 is fully done pending final push and ROADMAP update

---
*Phase: 258-staff-controls-deployment-safety*
*Completed: 2026-03-29*
