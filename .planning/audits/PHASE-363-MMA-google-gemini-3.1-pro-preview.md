## Phase 363-03 MMA Audit — Findings

### P0 blockers (must fix before deploy)
1. **Concurrency / FSM:** Tick loop grace window re-entry race and infinite loop
   - **file:line:** `crates/racecontrol/src/billing.rs:1455-1460`
   - **description:** When a grace window expires, the tick loop clears `lap_reject_grace_until` and `pending_end_status` synchronously, then drops the lock and calls `end_billing_session`. If `end_billing_session` takes longer than the 1s tick interval, or if it fails (e.g., CAS mismatch) and leaves the timer in `active_timers`, the next tick will see the timer as expired with NO grace window. It will then re-evaluate it as expired and *re-enter* the grace window, setting a new 5s deadline. This causes an infinite loop of delayed finalizes and DB writes.
   - **why:** Reverting the FSM state to `None` while yielding to async operations allows concurrent ticks to misinterpret the state as "needs a new grace window".
   - **fix:** Do not clear the grace fields. Instead, remove the timer from `active_timers` entirely using `timers.remove(pod_id)` before dropping the lock, and pass the necessary owned data to the deferred finalize task.
   - **confidence:** 10/10
