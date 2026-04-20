# nemotron-super (nvidia/nemotron-3-super-120b-a12b)

- elapsed: 37.4s
- prompt_tokens: 9439
- completion_tokens: 8682
- estimated_cost_usd: 0.0053
- finish_reason: stop

---

FINDING #1: Race condition between T1 and T2 triggers causing concurrent HTTP reconciliation
SEVERITY: P1
AXIS: a
DESCRIPTION: If T1 (post-reconnect) and T2 (5-minute tick) fire close together, two concurrent `fetch_and_reconcile` tasks may both read `last_applied_session_end` as None before either updates it, causing duplicate session end synthesis despite the idempotency guard. This could trigger duplicate overlay transitions, FFB zeroing, and billing cleanup.
EVIDENCE: Design spec lacks synchronization for T1/T2 concurrency; T1 fires per reconnect (unbounded frequency), T2 every 300s. No mutex or atomic guard around the reconciliation logic.
PROPOSED MITIGATION: Add a per-session `reconciliation_in_progress: AtomicBool` flag in `shutdown_monitor` to serialize HTTP fallback attempts for the same session ID.

FINDING #2: Missing 60-second grace period check enables false-positive session end during session start
SEVERITY: P0
AXIS: b
DESCRIPTION: The design specifies a 60-second grace period guard (only synthesize if session ID active ≥60s) but the algorithm omits this check. If server hasn't yet registered a new session (e.g., during Pod boot), HTTP fallback may see local ID missing from `/billing/active` and falsely synthesize SessionEnded, terminating a valid session.
EVIDENCE: Design spec failure mode #7 describes the guard but algorithm in "Triggers" section shows no age check; only verifies `local_id.is_none()` and server omission.
PROPOSED MITIGATION: Store `active_billing_session_id_set_at: Instant` in `shutdown_monitor` and skip synthesis if `elapsed() < Duration::from_secs(60)`.

FINDING #3: 5s HTTP timeout too aggressive causes unnecessary retry delays in lossy networks
SEVERITY: P2
AXIS: c
DESCRIPTION: A 5s timeout may trigger on transient network blanks or slow server responses, skipping synthesis unnecessarily. Recovery then waits until next T1/T2 trigger (up to 5min), delaying session end detection and prolonging orphaned game processes/overlays.
EVIDENCE: Design spec: `let server_resp = http_get("/api/v1/billing/active", 5s_timeout)`
PROPOSED MITIGATION: Increase timeout to 15s (matching typical WS reconnect intervals) and treat timeouts as "unknown" (skip synthesis but retry next tick) rather than failing silently.

FINDING #4: Zeroed synthesis fields create misleading session summary when local stats unavailable
SEVERITY: P2
AXIS: c
DESCRIPTION: If HTTP fallback fires after WS disconnect (e.g., silent-loop-death), rc-agent may lack fresh `driving_seconds`, `total_laps`, etc. Synthesizing zeros shows inaccurate "0 laps, 0s" summary card, confusing customers and corrupting offline lap metrics.
EVIDENCE: Design spec failure mode #3: "Synthesised `driving_seconds` / `total_laps`: if rc-agent doesn't have these locally, synthesise with `0, None, 0`."
PROPOSED MITIGATION: Use last-known values from `conn` (e.g., `conn.session_driving_seconds`, `conn.session_total_laps`) if available; only fall back to zero if truly unset (indicating session never started).

FINDING #5: ConnectionState reset during reconnect erases idempotency guard enabling double synthesis
SEVERITY: P1
AXIS: b
DESCRIPTION: If `ConnectionState::last_applied_session_end` is reset during WS reconnect (e.g., when clearing `current_driver_name`), the guard is lost. A subsequent HTTP fallback could re-apply SessionEnded for an already-ended session, causing duplicate summary cards and redundant cleanup.
EVIDENCE: Design spec relies on `last_applied_session_end` in `ConnectionState` but WS handler only resets `conn.current_driver_name = None` (line 413 in ws_handler.rs), not the guard. No evidence guard persists across reconnects.
PROPOSED MITIGATION: Initialize `last_applied_session_end` to None on ConnectionState creation and never reset it; persist guard across reconnects by storing it in `AppState` instead.

FINDING #6: Background T2 tick ignores shutdown signal causing spurious synth during exit
SEVERITY: P1
AXIS: d
DESCRIPTION: The 5-minute periodic tick (T2) uses `tokio::spawn` at boot but doesn't respect rc-agent's shutdown signal. During graceful exit, it may fire and synthesize SessionEnded after state teardown begins, causing panics (e.g., sending WS messages on closed connection) or incomplete cleanup.
EVIDENCE: Design spec failure mode #11: "Tokio task cancellation / shutdown race: background 5-minute tick must respect the rc-agent shutdown signal"
PROPOSED MITIGATE: Bind T2 tick to shutdown token via `tokio::select! { _ = shutdown_token.cancelled() => break, _ = tick.sleep() => ... }`.

FINDING #7: Server 304 Not Modified response treated as empty session list causing false synthesis
SEVERITY: P2
AXIS: c
DESCRIPTION: If server returns HTTP 304 (Not Modified) due to caching, the response body may be empty. Current logic treats this as successful fetch but empty `sessions` array, incorrectly concluding local session ended and triggering synthesis when session is actually active.
EVIDENCE: Design spec failure mode #5: "Server returns stale/cached 304: unlikely (handler always fresh-reads), but MMA should confirm." No handling for 304 in algorithm.
PROPOSED MITIGATION: Treat 304 as "unchanged from last known state" – skip synthesis if prior response contained the session ID; require explicit absence in 200 response to trigger fallback.

FINDING #8: Missing pod_id validation enables cross-pod session confusion
SEVERITY: P1
AXIS: b
DESCRIPTION: The fallback compares only session ID against `/billing/active` list without verifying `pod_id`. If session IDs are not globally unique (e.g., recycled across pods), rc-agent may mistake another pod's active session as its own ended session (or vice versa), causing incorrect session termination or missed cleanup.
EVIDENCE: Design spec failure mode #6: "Multi-pod session: `BillingSessionInfo` includes `pod_id`. Do we filter to `pod_id == state.config.pod.number` before comparison, or trust the unique session id?"
PROPOSED MITIGATION: Filter server response to `sessions.iter().filter(|s| s.pod_id == state.config.pod.number)` before checking ID presence. Requires `BillingSessionInfo` to include `pod_id`.

FINDING #9: Missing service key header causes 401/403 preventing fallback activation
SEVERITY: P1
AXIS: c
DESCRIPTION: Though `/billing/active` is marked public, other `public_routes` require `X-Service-Key`. If this endpoint actually mandates the key (e.g., via middleware), rc-agent's unauthenticated request gets 4xx, treated as server error and skipped – leaving session stranded indefinitely.
EVIDENCE: Design spec failure mode #9: "Authentication: the `/billing/active` route is public. But several `public_routes` still gate on `X-Service-Key` for non-health endpoints. Confirm at MMA time and add header if needed."
PROPOSED MITIGATION: Add `X-Service-Key` header using same mechanism as `remote_ops.rs`: `state.mesh_key_cache.get_key_or_env().await.unwrap_or_default()`.

FINDING #10: Partial JSON response missing `sessions` field causes silent skip delaying session end
SEVERITY: P2
AXIS: c
DESCRIPTION: If server returns valid JSON but omits the `sessions` field (e.g., due to partial failure), current logic treats it as "unknown" and skips synthesis. This delays session end detection until next T1/T2 trigger, prolonging orphaned states by up to 5 minutes.
EVIDENCE: Design spec failure mode #10: "Partial server response: JSON parses but `sessions` field missing. Treat as 'unknown' (not 'empty') and skip synth."
PROPOSED MITIGATION: Treat missing `sessions` field as server error (skip synthesis but log WARNING) – avoids false synthesis while making issue visible for investigation. Do not treat as empty list.
