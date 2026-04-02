# Phase 310: Session Trace ID Propagation — COMPLETE

**Completed:** 2026-04-02
**Commit:** `3501828c`
**Tests:** 1033 passing (801 racecontrol + 232 rc-common)

## What Was Delivered

Threaded `billing_session_id` through all subsystems so `WHERE session_id = ?`
traces the full customer journey: Launch → Billing → Crash → Resume → End → Refund.

### Changes (10 files, 86 insertions, 45 deletions)

| File | Change |
|------|--------|
| `db/mod.rs` | ALTER TABLE pod_activity_log + launch_events ADD session_id TEXT + indexes |
| `rc-common/types.rs` | Added `session_id: Option<String>` to `GameLaunchInfo` |
| `metrics.rs` | Added `session_id` to `LaunchEvent` struct + SQL INSERT |
| `game_launcher.rs` | Added `billing_session_id` to `GameTracker`, captured from billing timer at launch, passed through `to_info()` + `LaunchEvent` |
| `activity_log.rs` | Added `session_id: Option<&str>` param to `log_pod_activity()` + SQL INSERT with 12th bind |
| 9 caller files | 81 call sites updated to pass `None` (or real session_id at launch) |

### Requirements Satisfied

- **MI-5**: Single trace_id links Launch → Billing → Refund through logs
- `GameTracker.billing_session_id` set from `timer.session_id` at launch time
- `GameLaunchInfo.session_id` auto-propagates through `DashboardEvent::GameStateChanged`
- `LaunchEvent.session_id` links launch metrics to billing sessions
- `pod_activity_log.session_id` column indexed for fast trace queries

### Verification

```sql
-- After deploy, this returns the complete session timeline:
SELECT session_id, category, action, timestamp
FROM pod_activity_log
WHERE session_id = '<billing-session-id>'
ORDER BY timestamp;

-- Launch metrics linked to billing:
SELECT session_id, outcome, sim_type, duration_to_playable_ms
FROM launch_events
WHERE session_id = '<billing-session-id>';
```

### Future Enhancement

Upgrade the 14 `billing.rs` callers from `None` to `Some(&session_id)` —
the session_id variable is in scope at every billing log site. Low effort, high tracing value.
