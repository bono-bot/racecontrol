# Layer 3 — Server Grace Window — Expiry Flow Research

> Produced 2026-04-16 by James (autonomous session). Research only — no code changes.

## Billing Timer Expiry → StopGame Chain

| Step | File | Lines | What happens |
|------|------|-------|--------------|
| 1. Tick loop | `billing_timer.rs` | 28, 208, 244 | `tick_all_timers()` every 1s. `timer.tick()` decrements remaining. Returns `expired = true` when 0. |
| 2. Grace window entry | `billing_timer.rs` | 249-261 | Sets `lap_reject_grace_until = now + 5s`, `pending_end_status = Completed`. Session NOT finalized yet. |
| 3. Grace window check | `billing_timer.rs` | 67-75, 281-283 | Next tick after grace elapses → timer collected into `deferred_finalizes`, removed from `active_timers`. |
| 4. Deferred finalize | `billing_timer.rs` | 302-313 | After dropping write lock, calls `end_billing_session(state, &sid, end_status)`. |
| 5. End session + StopGame | `billing_session_end.rs` | 37, 284 | Looks up pod, computes refunds, updates DB, sends `CoreToAgentMessage::StopGame` via WS. |
| 6. Agent receives StopGame | `ws_handler.rs` (rc-agent) | 1241 | Kills game process, zeroes FFB, sends ACK. |

**Alternate path** (old, pre-grace): `billing_timer_expiry.rs:17` → `handle_expired_sessions()` → StopGame at line 44.

## Existing Grace Window (GLD-C-04)

A 5-second grace window **already exists** at `billing_timer.rs:249-263`. Purpose: allow late `LapRejected` messages to arrive before finalizing. It does NOT check whether a lap is in progress — it's a fixed 5s delay.

## Extension Point for Lap-Aware Grace

Replace or augment the block at `billing_timer.rs:249` (inside `if expired`). Instead of unconditionally setting 5s grace, query whether a lap is in progress and set a longer grace (30-60s) if so.

## Data Available at Extension Point

At `expired = true` (line 244), the code holds:
- Write lock on `active_timers`, read lock on `pods`
- `timer.session_id`, `timer.driving_seconds`, `timer.elapsed_seconds`, `timer.allocated_seconds`
- `timer.driver_id`, `timer.driver_name`
- Pod connection state via `pods.get(pod_id)`

## Key Complication: No "Lap In Progress" Signal

The server receives `LapCompleted` events from agents (`ws/agent_game.rs:44-67`) stored in `laps` table. But there is **no "lap in progress" message** from agent → server. The `current_lap_invalid` field in `TelemetryFrame` (`rc-common types.rs:219`) is per-tick telemetry, not a session-level flag.

### Options to Solve

1. **New agent message** (preferred): Agent sends `LapInProgress { lap_number, estimated_remaining_ms }` periodically or on sector crossings. Server uses this to decide grace duration.
2. **Telemetry inference** (fragile): Compare `last LapCompleted timestamp` vs `now` vs `average lap duration`. If `now - last_lap_completed < avg_lap_time`, assume lap in progress. Breaks for first lap or highly variable tracks.
3. **Fixed generous grace** (simplest): Always extend by average-fastest-lap-time for the track (from reference data). Doesn't need agent changes but wastes time if no lap is happening.

## Struct References

| Struct | File | Key fields |
|--------|------|------------|
| `BillingTimer` | `billing.rs:27, 97-103` | `lap_reject_grace_until`, `pending_end_status` |
| `TelemetryFrame` | `rc-common/types.rs:219` | `current_lap_invalid` |
| `CoreToAgentMessage::StopGame` | `rc-common/protocol.rs` | The stop command |
