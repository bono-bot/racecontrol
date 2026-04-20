# qwen3-coder (qwen/qwen3-coder)

- elapsed: 13.8s
- prompt_tokens: 8538
- completion_tokens: 1571
- estimated_cost_usd: 0.0044
- finish_reason: stop

---

FINDING #1: HTTP fallback may fire during session launch phase
SEVERITY: P0
AXIS: a
DESCRIPTION: The HTTP fallback can trigger when `active_billing_session_id` is set but server hasn't yet acknowledged it in `/api/v1/billing/active`. This creates a race where rc-agent has locally set the session ID (e.g., during launch) but the server's active session list doesn't include it yet. The fallback would incorrectly conclude the session ended and synth a SessionEnded, causing premature termination. The 60s grace period guard exists but may not be sufficient if server sync is delayed.
EVIDENCE: Design spec section "Failure modes the MMA must enumerate" item 7; `main.rs:2009` sets `active_billing_session_id`
PROPOSED MITIGATION: Extend grace period to 120s and add server-side session state validation before synth

FINDING #2: Duplicate SessionEnded processing when WS frame arrives after HTTP synth
SEVERITY: P1
AXIS: b
DESCRIPTION: If HTTP fallback fires and synthesizes SessionEnded, then the real WS SessionEnded frame arrives later, the deduplication guard may fail if `last_applied_session_end` is not atomically updated. This could cause double overlay transitions, duplicate FFB-zero messages, and duplicate CSV fallback pushes. The race occurs between the HTTP synth task and WS message handler task both calling `apply_session_ended`.
EVIDENCE: `ws_handler.rs:379-415` SessionEnded arm; design spec Idempotency section
PROPOSED MITIGATION: Use atomic compare-and-swap for `last_applied_session_end` and add session ID to all log messages for traceability

FINDING #3: 5-minute tick continues during shutdown causing synth during exit
SEVERITY: P1
AXIS: d
DESCRIPTION: The background 5-minute tokio task that polls for session status does not respect rc-agent shutdown signals. During graceful shutdown, this task could fire and synthesize a SessionEnded while other subsystems are already shutting down, leading to inconsistent state or panic during shutdown sequence. The task should be spawned with a cancellation token that's triggered on shutdown.
EVIDENCE: Design spec "Triggers" T2 section; `main.rs:2214` reconnect success point
PROPOSED MITIGATION: Pass shutdown token to periodic task and check cancellation before HTTP call

FINDING #4: Blank timer reset conflict between WS and HTTP paths
SEVERITY: P2
AXIS: c
DESCRIPTION: Both the real WS SessionEnded handler and the HTTP synth path call `apply_session_ended` which resets `conn.blank_timer`. If the HTTP fallback fires and resets the timer, then the real WS frame arrives and resets it again, there's a race condition where the blank timer could be armed twice with different deadlines. This may cause screen blanking to occur at unexpected times or not occur at all if the timer gets reset during its final countdown.
EVIDENCE: `ws_handler.rs:413` blank_timer reset; design spec Refactor section
PROPOSED MITIGATION: Add atomic flag to track blank timer armed state and prevent double-reset

FINDING #5: Server URL derivation inconsistency between WS and HTTP paths
SEVERITY: P1
AXIS: a
DESCRIPTION: The HTTP fallback uses URL derivation logic that must match the existing WS-to-HTTP conversion pattern at `ws_handler.rs:423-429`. If the fallback implementation uses different logic (e.g., different regex or string manipulation), it could target the wrong server endpoint during failover scenarios. This would cause the HTTP probe to fail or target wrong pod, leading to missed session reconciliation.
EVIDENCE: `ws_handler.rs:423-429` URL derivation; design spec failure mode #8
PROPOSED MITIGATION: Extract URL derivation to shared helper function and use in both paths

FINDING #6: SessionEnded stats synthesis with zero values breaks customer experience
SEVERITY: P2
AXIS: b
DESCRIPTION: When HTTP fallback synthesizes SessionEnded, it may not have access to `total_laps`, `best_lap_ms`, and `driving_seconds`. Synthesizing with zero values creates a misleading summary card for customers who expect real stats. The zeroed stats could also break downstream analytics or billing calculations that assume non-zero values indicate valid sessions.
EVIDENCE: Design spec failure mode #3; `ws_handler.rs:382` SessionEnded stats processing
PROPOSED MITIGATION: Cache last known stats in ConnectionState and use cached values for synth

FINDING #7: HTTP fallback bypasses inactivity monitor reset
SEVERITY: P2
AXIS: c
DESCRIPTION: The real WS SessionEnded handler resets the inactivity monitor (`conn.inactivity_monitor.reset()`), but the HTTP fallback path may not. This creates inconsistency where the inactivity monitor continues running after a synth SessionEnded, potentially triggering false inactivity events or failing to clean up monitor state. The disconnect between monitor state and session state could cause overlay conflicts.
EVIDENCE: `ws_handler.rs:385` inactivity monitor reset; design spec Algorithm section
PROPOSED MITIGATION: Add inactivity monitor reset to `apply_session_ended` function for both paths

FINDING #8: Crash recovery state not reset in HTTP synth path
SEVERITY: P2
AXIS: c
DESCRIPTION: The real WS SessionEnded handler sets `conn.crash_recovery = CrashRecoveryState::Idle`, but the HTTP fallback path through `apply_session_ended` may not reset this state. If crash recovery was active when the HTTP fallback fires, the recovery state would persist incorrectly, potentially causing recovery logic to trigger inappropriately on subsequent sessions or interfering with normal session startup.
EVIDENCE: `ws_handler.rs:392` crash recovery reset; `ws_handler.rs:379-415` SessionEnded arm
PROPOSED MITIGATION: Add crash recovery state reset to `apply_session_ended` function

FINDING #9: Service key authentication mismatch in HTTP fallback
SEVERITY: P1
AXIS: a
DESCRIPTION: The HTTP fallback must include the same `X-Service-Key` header used by other authenticated server calls in `remote_ops.rs`. If the fallback omits this header or uses different authentication, it will be rejected by the server even though `/billing/active` is in `public_routes`. This would cause the fallback to always fail and never synth SessionEnded, defeating the purpose of the feature.
EVIDENCE: Design spec failure mode #9; `remote_ops.rs` authenticated calls pattern
PROPOSED MITIGATION: Use shared HTTP client with service key middleware for fallback requests

FINDING #10: Multi-pod session ID collision causes incorrect synth
SEVERITY: P1
AXIS: b
DESCRIPTION: If two pods somehow get the same session ID (extremely unlikely but possible with UUID collisions), the HTTP fallback could incorrectly synth SessionEnded on one pod when the session is actually active on another pod. The current design trusts session ID uniqueness without filtering by `pod_id`, creating a cross-pod authority confusion where one pod can terminate another pod's session.
EVIDENCE: Design spec failure mode #6; `billing_views.rs:326` server endpoint
PROPOSED MITIGATION: Filter server sessions by `pod_id` match before checking session ID presence
