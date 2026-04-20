# grok-code (x-ai/grok-code-fast-1)

- elapsed: 12.6s
- prompt_tokens: 8445
- completion_tokens: 1983
- estimated_cost_usd: 0.0047
- finish_reason: stop

---

FINDING #1: Race condition between HTTP synth and WS SessionEnded on same session ID
SEVERITY: P1
AXIS: a
DESCRIPTION: If HTTP fetch_and_reconcile detects session ended and calls apply_session_ended at t=0, but a real WS SessionEnded frame arrives at t=1 with the same ID, the guard on last_applied_session_end blocks the WS handler, preventing real stats from updating the summary card, leaving customer with zeroed-out synth data.
EVIDENCE: Spec section "Idempotency + authority" describes guard, but ws_handler.rs:379-415 shows WS arm relies on real frame data; race window exists post-synth.
PROPOSED MITIGATION: Modify guard to allow WS real frame if origin differs, or log and skip synth if WS pending.

FINDING #2: Timing-sensitive false positive synth during session launch grace period
SEVERITY: P2
AXIS: b
DESCRIPTION: If active_billing_session_id is set locally at t=0 but server hasn't inserted into active_timers yet (e.g., due to DB lag), HTTP probe at t=30s sees "gone" and synths ended, despite session being active, causing premature overlay deactivation and game stop.
EVIDENCE: Spec seed #7 mentions 60s grace; main.rs:2009 sets active_billing_session_id via failure_monitor_tx; server endpoint billing_views.rs:326 reads active_timers.
PROPOSED MITIGATION: Enforce 60s minimum age check on active_billing_session_id before probing, using tokio::time::Instant.

FINDING #3: Race with reconnect loop resetting ws_disconnected_at on success
SEVERITY: a
AXIS: c
DESCRIPTION: T1 fires post-reconnect at main.rs:2214, but if WS reconnect succeeds just after HTTP probe starts, ws_disconnected_at resets to None, potentially masking a prior disconnect that should have triggered synth earlier.
EVIDENCE: main.rs:2060-2230 shows ws_disconnected_at reset on success; T1 injection at "Connected and registered".
PROPOSED MITIGATION: Capture ws_disconnected_at timestamp before T1 firing and include in synth decision if >30s ago.

FINDING #4: Ordering bug where synth apply_session_ended runs before WS handler clears conn fields
SEVERITY: a
AXIS: b
DESCRIPTION: HTTP synth calls apply_session_ended, setting conn.current_driver_name = None at ws_handler.rs:413, but if a delayed WS SessionEnded arrives, it overwrites with real driver_name, causing inconsistent state in conn struct.
EVIDENCE: ws_handler.rs:379-415 sets conn fields in order; spec refactor extracts to apply_session_ended but doesn't specify ordering guard.
PROPOSED MITIGATION: Add atomic flag on ConnectionState to prevent concurrent apply_session_ended calls.

FINDING #5: Integration failure with blank_timer reset on synth vs real WS
SEVERITY: P1
AXIS: c
DESCRIPTION: Synth apply_session_ended resets blank_timer at ws_handler.rs:413, but real WS does too; if synth fires first, real WS resets again harmlessly, but if failure_monitor_tx send_modify races, it could leave blank_timer in wrong state, delaying screen blank.
EVIDENCE: ws_handler.rs:413 resets blank_timer; failure_monitor_tx at ws_handler.rs:390 modifies state; spec notes re-arming benign but races unaddressed.
PROPOSED MITIGATION: Use tokio::sync::Mutex on blank_timer operations in apply_session_ended.

FINDING #6: Authority mismatch if server returns stale active_timers due to read lock delay
SEVERITY: P2
AXIS: b
DESCRIPTION: Server endpoint billing_views.rs:326 acquires read locks on rate_tiers and active_timers; if a concurrent end_billing_session is writing, HTTP probe might see outdated data, missing a just-ended session and failing to synth.
EVIDENCE: billing_views.rs:326 uses read().await; spec assumes server authoritative but doesn't account for lock contention timing.
PROPOSED MITIGATION: Add server-side timestamp to BillingSessionInfo and client-side check for staleness >5s.

FINDING #7: Variable-level bug where conn.current_driver_name cache is None during synth
SEVERITY: P2
AXIS: a
DESCRIPTION: Synth apply_session_ended uses conn.current_driver_name for summary, but if WS flap reset it to None at ws_handler.rs:376 before synth, summary shows empty driver name, confusing customer.
EVIDENCE: ws_handler.rs:376 sets conn.current_driver_name = None; spec "Driver-name cache" relies on it; synth path lacks fallback.
PROPOSED MITIGATION: Store driver_name in active_billing_session_id struct alongside ID, fetched from WS launch.

FINDING #8: Idempotency violation if apply_session_ended called twice rapidly without guard
SEVERITY: P1
AXIS: b
DESCRIPTION: Without last_applied_session_end guard, two T2 ticks at t=0 and t=300s both synth for same ID, causing duplicate overlay deactivation, double FFB-zero messages, and log floods.
EVIDENCE: Spec "Idempotency of apply_session_ended" claims idempotent, but ws_handler.rs:379-415 has side effects like ws_tx.send; guard prevents double application.
PROPOSED MITIGATION: Ensure last_applied_session_end is checked atomically in apply_session_ended entry.

FINDING #9: Cross-system ordering issue with CSV fallback push on synth
SEVERITY: P2
AXIS: c
DESCRIPTION: Synth apply_session_ended spawns CSV push at ws_handler.rs:423-429, but if real WS arrives later, it spawns again, potentially double-posting laps-offline.csv to server, corrupting session data.
EVIDENCE: ws_handler.rs:423-429 spawns on SessionEnded; spec origin=HttpSynth but doesn't gate CSV; idempotent claim may not hold for detached tasks.
PROPOSED MITIGATION: Add origin check in spawned task to skip CSV push if origin=HttpSynth.

FINDING #10: Deployment observability gap in ws_state.rs for synth events
SEVERITY: P3
AXIS: d
DESCRIPTION: Synth apply_session_ended doesn't update ws_state.rs diagnostics, so /debug/ws-state shows no indication of HTTP fallback firing, obscuring synth vs WS-ended in post-deploy monitoring.
EVIDENCE: ws_state.rs tracks WS phases; spec deploy verification relies on fleet/health but not ws_state for synth.
PROPOSED MITIGATION: Add synth counter to WsStateInner, incremented in apply_session_ended.

FINDING #11: Rollback bug where old binary lacks synth, causing stuck sessions post-rollback
SEVERITY: P0
AXIS: d
DESCRIPTION: If patch deployed and rollback to prior binary occurs, rc-agent loses HTTP fallback, reverting to original stuck-session behavior on WS misses, with no alert in logs.
EVIDENCE: Spec deploy "Binary swap" via rc-sentry; no rollback guard; ws_handler.rs lacks synth in provided source.
PROPOSED MITIGATION: Add version check in startup_log to warn if synth code absent after prior deployment.

FINDING #12: Integration with failure_monitor_tx race on billing_active flag
SEVERITY: P1
AXIS: c
DESCRIPTION: Synth sets billing_active=false via failure_monitor_tx at ws_handler.rs:390, but if reconnect loop concurrently modifies state, it could overwrite to true, leaving heartbeat_status out of sync.
EVIDENCE: ws_handler.rs:390 sends modify; main.rs reconnect modifies state; spec short-circuit on heartbeat_status.
PROPOSED MITIGATION: Use atomic compare_exchange in failure_monitor_tx for billing_active updates.
