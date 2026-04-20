# gemini-flash (google/gemini-2.5-flash)

- elapsed: 24.6s
- prompt_tokens: 10273
- completion_tokens: 3691
- estimated_cost_usd: 0.0123
- finish_reason: stop

---

FINDING #1: HTTP Fallback can race with server-side session termination
SEVERITY: P1
AXIS: a
DESCRIPTION: The HTTP fallback mechanism polls the server for active sessions. There's a race condition where the server might be in the process of terminating a session (e.g., `end_billing_session()` has been called, removing it from `active_timers`), but the HTTP request is made *before* the server's database transaction commits or before the server's internal state fully propagates. This could lead to the HTTP fallback prematurely synthesizing a `SessionEnded` event, even if a real `SessionEnded` WS frame was about to be sent or was in flight.
EVIDENCE: DESIGN-SPEC "Algorithm" section: `if !server_ids.contains(local_id): synthesise_session_ended(local_id, best-effort fields)`.
PROPOSED MITIGATION: Introduce a small, configurable delay (e.g., 1-2 seconds) between the HTTP check returning "session gone" and the actual `synthesise_session_ended` call. This allows the server more time to send the WS frame if it's still intended. Log this delay.

FINDING #2: `current_driver_name` can be `None` for `HttpSynth`
SEVERITY: P2
AXIS: c
DESCRIPTION: The `apply_session_ended` function requires a `driver_name`. The existing WS handler sets `conn.current_driver_name = None` at the end of the `SessionEnded` block. If an HTTP-synthesized `SessionEnded` occurs, `conn.current_driver_name` might already be `None` or an outdated value, leading to a blank or incorrect driver name on the summary card. The spec mentions "best-effort fields" for `HttpSynth` but doesn't specify how `driver_name` is sourced.
EVIDENCE: DESIGN-SPEC "Local state" section: `Driver-name cache: conn.current_driver_name (reset to None by the WS handler at line 376 and 413)`. `ws_handler.rs:376` and `ws_handler.rs:413` both set `conn.current_driver_name = None`.
PROPOSED MITIGATION: When synthesizing, if `conn.current_driver_name` is `None`, attempt to retrieve the driver name from the `BillingSessionInfo` returned by the server's `/api/v1/billing/active` endpoint, if available. If still `None`, use a placeholder like "Unknown Driver".

FINDING #3: Server URL derivation inconsistency
SEVERITY: P2
AXIS: c
DESCRIPTION: The spec notes that `ws_handler.rs:423-429` derives the HTTP base URL from the WebSocket URL. The new HTTP fallback path will also need to derive this URL. If a new, separate implementation is used, it could lead to subtle differences in URL parsing or handling of `ws://` vs `wss://` or trailing slashes, causing connection failures or incorrect endpoint targeting.
EVIDENCE: DESIGN-SPEC "Failure modes the MMA must enumerate" item 8: `Server URL derivation: ws_handler.rs:423-429 reuses core.url.replace("ws://", "http://").split("/ws"). New code must use the same pattern or (better) a shared helper — MMA to flag.`
PROPOSED MITIGATION: Create a shared utility function (e.g., `crate::utils::derive_http_base_url`) that encapsulates the logic for converting the WebSocket URL to the HTTP base URL. Both the CSV fallback and the new HTTP fallback should use this single helper.

FINDING #4: Authentication for `/api/v1/billing/active`
SEVERITY: P1
AXIS: b
DESCRIPTION: The spec states `/api/v1/billing/active` is in `public_routes` and "safe for rc-agent to call with its existing service-key HTTP client". However, it also notes that "several `public_routes` still gate on `X-Service-Key` for non-health endpoints." If this specific endpoint *does* require `X-Service-Key`, and the new HTTP client doesn't provide it, the calls will fail with 401/403, preventing the fallback from ever working.
EVIDENCE: DESIGN-SPEC "Server endpoint contract (exists)" and "Failure modes the MMA must enumerate" item 9: `Authentication: the /billing/active route is public. But several public_routes still gate on X-Service-Key for non-health endpoints. Confirm at MMA time and add header if needed — matching the pattern remote_ops.rs uses for its own authenticated server calls.`
PROPOSED MITIGATION: Explicitly confirm with the server team whether `GET /api/v1/billing/active` requires an `X-Service-Key`. If it does, ensure the HTTP client used for the fallback path is configured to include the `X-Service-Key` header, mirroring `remote_ops.rs`.

FINDING #5: `blank_timer` re-arming on duplicate `apply_session_ended`
SEVERITY: P3
AXIS: a
DESCRIPTION: The spec states that `apply_session_ended` is idempotent, and "the re-arming of `blank_timer` is benign — customer was already shown the summary — so a second call is cosmetically harmless". However, re-arming `blank_timer` could potentially extend the blank screen duration or interfere with other blanking logic if it's not truly benign in all scenarios. For example, if the timer is reset to a future time, it might delay a subsequent blanking event that was intended to happen sooner.
EVIDENCE: DESIGN-SPEC "Idempotency + authority" section: `The re-arming of blank_timer is benign — customer was already shown the summary — so a second call is cosmetically harmless, but the duplicate log lines break PoE analysis.` Also `ws_handler.rs:417` `conn.blank_timer.as_mut().reset(...)`.
PROPOSED MITIGATION: The `last_applied_session_end` guard should prevent *all* side effects of `apply_session_ended` if it's a duplicate, not just the summary card. This ensures `blank_timer` is only reset once per session end.

FINDING #6: `tokio::spawn_blocking` in `apply_session_ended`
SEVERITY: P2
AXIS: a
DESCRIPTION: The `apply_session_ended` function (extracted from `ws_handler.rs`) contains `tokio::task::spawn_blocking(|| { ac_launcher::enforce_safe_state(true); });`. If this blocking task takes a significant amount of time, and the HTTP fallback triggers frequently (e.g., due to a misconfigured 5-minute tick or a rapid reconnect loop), it could lead to an accumulation of blocking tasks, potentially starving the Tokio runtime of threads or delaying other critical operations.
EVIDENCE: `ws_handler.rs:377` and `ws_handler.rs:414` both contain `tokio::task::spawn_blocking(...)`.
PROPOSED MITIGATION: Evaluate the execution time of `ac_launcher::enforce_safe_state`. If it's consistently fast, this might be acceptable. If not, consider making `enforce_safe_state` asynchronous or ensuring that only one such blocking task can run at a time (e.g., using an `Arc<AtomicBool>` guard).

FINDING #7: `state.heartbeat_status.billing_active` and `remote_ops::BILLING_ACTIVE` divergence
SEVERITY: P3
AXIS: c
DESCRIPTION: The `apply_session_ended` function updates both `state.heartbeat_status.billing_active` and `crate::remote_ops::BILLING_ACTIVE`. While they are both `AtomicBool` and updated with `Release` ordering, having two separate global flags for essentially the same state (`billing_active`) introduces a potential for future divergence or confusion. If one is updated and the other is missed in a future change, it could lead to inconsistent behavior.
EVIDENCE: `ws_handler.rs:390-391`: `state.heartbeat_status.billing_active.store(false, ...); crate::remote_ops::BILLING_ACTIVE.store(false, ...);`
PROPOSED MITIGATION: Consolidate these two flags into a single source of truth, or clearly document their intended relationship and ensure all state transitions update both consistently. The `state.heartbeat_status.billing_active` seems to be the primary one for short-circuiting the HTTP check.

FINDING #8: Server cold-restart handling
SEVERITY: P1
AXIS: b
DESCRIPTION: The spec correctly identifies "Server cold-restart: mid-request, server returns 503. We must NOT treat unreachable as 'session ended'." The algorithm states `Err(_) | timeout: return # never synth on server-down`. This is good, but it assumes all transient server errors (like 503) will manifest as `Err(_)` or `timeout`. Some HTTP libraries might parse a 5xx response as `Ok(body)` but with an empty or error-indicating body.
EVIDENCE: DESIGN-SPEC "Algorithm" section: `match server_resp: Err(_) | timeout: return`. "Failure modes the MMA must enumerate" item 4.
PROPOSED MITIGATION: Explicitly check the HTTP status code of the response. Only proceed with parsing the body and synthesizing `SessionEnded` if the status code is 200 OK. Treat any non-200 status code (including 5xx) as a non-authoritative response and `return`.

FINDING #9: `BillingSessionInfo` shape and field stability
SEVERITY: P2
AXIS: b
DESCRIPTION: The spec notes that the "Shape of `BillingSessionInfo`: TBD at MMA time — field names for `id`, `pod_id`, `driver_id`, `driver_name`, `started_at`, `allocated_seconds` must be stable enough that rc-agent can identify 'my' session and synthesise a `SessionEnded` frame". If these field names change or are not present, the rc-agent's parsing will fail, preventing the fallback from working. This is a cross-system contract that needs explicit definition.
EVIDENCE: DESIGN-SPEC "Server endpoint contract (exists)" section: `Shape of BillingSessionInfo: TBD at MMA time — field names for id, pod_id, driver_id, driver_name, started_at, allocated_seconds must be stable enough that rc-agent can identify "my" session and synthesise a SessionEnded frame`.
PROPOSED MITIGATION: Define the exact JSON schema for `BillingSessionInfo` in a shared contract (e.g., a Protobuf or OpenAPI schema). Ensure the rc-agent's deserialization logic is robust to missing optional fields (e.g., `driver_name` if it's not always present).

FINDING #10: `tokio::spawn` for 5-minute periodic tick and shutdown
SEVERITY: P1
AXIS: d
DESCRIPTION: The 5-minute periodic tick is launched as a background `tokio::spawn`. The spec mentions "Tokio task cancellation / shutdown race: background 5-minute tick must respect the rc-agent shutdown signal so it doesn't emit the synth during graceful exit." If this task doesn't properly listen for and react to the agent's shutdown signal, it could attempt to perform HTTP requests or modify state during agent shutdown, leading to errors, panics, or incomplete shutdown.
EVIDENCE: DESIGN-SPEC "Failure modes the MMA must enumerate" item 11.
PROPOSED MITIGATION: The `tokio::spawn` task for the 5-minute tick must be given a `CancellationToken` or similar mechanism (e.g., a `watch::Receiver` for a shutdown signal) that it polls before each HTTP request and before synthesizing `SessionEnded`. If the token is cancelled, the task should gracefully exit.

FINDING #11: Log flood from 5-minute tick
SEVERITY: P3
AXIS: d
DESCRIPTION: The spec notes "Log flood: if HTTP fallback fires every 5 min with no active session (gate short-circuits) — only INFO-level log on actual reconciliation event; DEBUG on 'nothing to do'." While the short-circuit gate helps, if there *is* an active session but the server consistently returns "session gone" (e.g., due to a server-side bug or race condition), the `synthesise_session_ended` could be called every 5 minutes, leading to a flood of INFO-level logs for the same event.
EVIDENCE: DESIGN-SPEC "Failure modes the MMA must enumerate" item 12.
PROPOSED MITIGATION: Implement rate-limiting or deduplication for `HttpSynth` `SessionEnded` logs. For example, only log an INFO message the first time a session is synthesized via HTTP, and subsequent attempts for the same session ID within a short period (e.g., 30 minutes) are logged at DEBUG or TRACE level.

FINDING #12: `ws_state` module's `current_phase` for silent-dead
SEVERITY: P3
AXIS: c
DESCRIPTION: The `ws_state` module is designed to diagnose "silent-reconnect-forever failures". However, the `current_phase` only tracks "init" | "connecting" | "connected" | "disconnected". If the WS connection is "connected" but the message processing loop (where `SessionEnded` frames are handled) has silently died or stalled, the `ws_state` will still report "connected", masking the underlying issue that the HTTP fallback is trying to address.
EVIDENCE: `ws_state.rs:27` `current_phase: "connected" means connect_async resolved Ok; it does NOT guarantee the server processed the Register message`.
PROPOSED MITIGATION: Consider adding a `last_message_received_at` timestamp to `WsStateInner` and updating it whenever a message is successfully processed by the WS handler. This would allow for diagnosing silent message processing failures even if the connection itself appears "connected".

FINDING #13: `state.config.pod.number` for `pod_id` filtering
SEVERITY: P2
AXIS: b
DESCRIPTION: The spec asks: "Multi-pod session: `BillingSessionInfo` includes `pod_id`. Do we filter to `pod_id == state.config.pod.number` before comparison, or trust the unique session id? Current design trusts session id — MMA should flag if pod_id mismatch would be a bug." If a session ID is reused across pods (highly unlikely but possible in some systems), or if a pod somehow gets assigned a session ID meant for another pod, trusting only the session ID could lead to a pod ending a session that isn't its own.
EVIDENCE: DESIGN-SPEC "Failure modes the MMA must enumerate" item 6.
PROPOSED MITIGATION: Add a check for `pod_id` from `BillingSessionInfo`. If the server's response includes `pod_id` for a session, verify that it matches `state.config.pod.number` in addition to matching the `billing_session_id`. If they don't match, treat it as if the session is not active for *this* pod.

FINDING #14: `total_laps` and `driving_seconds` for `HttpSynth`
SEVERITY: P2
AXIS: c
DESCRIPTION: The spec mentions that for `HttpSynth`, if `rc-agent doesn't have these locally, synthesise with 0, None, 0`. While this provides a summary card, it might be misleading to the customer. The `ws_handler.rs` code for `SessionEnded` uses `total_laps`, `best_lap_ms`, and `driving_seconds` directly from the server message. The agent does not typically track these locally with high fidelity.
EVIDENCE: DESIGN-SPEC "Failure modes the MMA must enumerate" item 3: `Synthesised driving_seconds / total_laps: if rc-agent doesn't have these locally, synthesise with 0, None, 0. Customer sees a zeroed-out summary card. Better than no card? MMA to decide.`
PROPOSED MITIGATION: If the server's `/api/v1/billing/active` endpoint could optionally return these metrics (e.g., `total_laps`, `driving_seconds`) as part of `BillingSessionInfo`, the `HttpSynth` could provide a more accurate summary. If not, clearly document the limitation of zeroed-out stats for HTTP-synthesized sessions.
