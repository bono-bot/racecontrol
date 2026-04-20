# deepseek-r1 (deepseek/deepseek-r1-0528)

- elapsed: 198.6s
- prompt_tokens: 9493
- completion_tokens: 4680
- estimated_cost_usd: 0.0149
- finish_reason: stop

---

Here are 12 findings covering the requested axes:

FINDING #1: Race between HTTP synth and real SessionEnded during WS reconnect  
SEVERITY: P1  
AXIS: a  
DESCRIPTION: During WS reconnect (T1 trigger), a real SessionEnded frame may arrive concurrently with the HTTP reconciliation check. Non-atomic access to `last_applied_session_end` could allow double-application of session end logic.  
EVIDENCE: Spec section "Idempotency + authority" describes non-atomic guard using `last_applied_session_end`  
PROPOSED MITIGATION: Use atomic compare-and-swap for `last_applied_session_end` in ConnectionState with ordering constraints (AcqRel).  

FINDING #2: Stale driver_name in synthesized SessionEnded  
SEVERITY: P2  
AXIS: c  
DESCRIPTION: HTTP fallback uses `conn.current_driver_name` which gets cleared by WS handlers (line 376). If WS reconnected after session start but before end, driver_name may be None during synth, causing blank summary card.  
EVIDENCE: ws_handler.rs:376 clears current_driver_name on non-SessionEnded messages  
PROPOSED MITIGATION: Cache driver_name separately when session becomes active and retain until session end.  

FINDING #3: HTTP timeout blocks tokio executor during reconciliation  
SEVERITY: P2  
AXIS: a  
DESCRIPTION: 5s HTTP timeout in reconciliation loop (T1/T2) uses blocking wait. If server hangs, it stalls the tokio executor thread, delaying critical operations like game process monitoring.  
EVIDENCE: Algorithm section specifies "5s_timeout" for HTTP call  
PROPOSED MITIGATION: Wrap HTTP call in `tokio::time::timeout` with spawn_blocking or use async reqwest client.  

FINDING #4: Reconciliation fires during pod registration race  
SEVERITY: P1  
AXIS: c  
DESCRIPTION: T1 trigger fires immediately after pod registration (main.rs:2214). If server-side session activation races behind WS registration, HTTP check may see "no session" during grace period.  
EVIDENCE: main.rs:2214 injects T1 "right after Connected and registered"  
PROPOSED MITIGATION: Add 10s delay after registration before first HTTP reconciliation attempt.  

FINDING #5: Blank timer reset by synth causes premature screen blank  
SEVERITY: P3  
AXIS: c  
DESCRIPTION: HTTP-synthesized SessionEnded resets blank_timer to 30s (ws_handler.rs:450). If customer was mid-session when synth fired, this forces blanking during active use.  
EVIDENCE: ws_handler.rs:450 shows blank_timer reset in SessionEnded handler  
PROPOSED MITIGATION: Only reset blank_timer when origin=WsReal or when session summary is actually displayed.  

FINDING #6: Server cold restart causes false-negative reconciliation  
SEVERITY: P2  
AXIS: b  
DESCRIPTION: After server restart, /billing/active returns empty list until timers repopulate. HTTP fallback may incorrectly synthesize SessionEnded for valid sessions.  
EVIDENCE: Failure modes #4 acknowledges cold restart risk but only handles 503  
PROPOSED MITIGATION: Check server generation ID or uptime in response; skip reconciliation if server recently booted.  

FINDING #7: Periodic task leaks during graceful shutdown  
SEVERITY: P2  
AXIS: d  
DESCRIPTION: Background tokio task for T2 isn't tied to main shutdown monitor. During graceful exit, it may attempt state access after cleanup.  
EVIDENCE: Triggers section describes T2 as "Background tokio::spawn launched at boot"  
PROPOSED MITIGATION: Pass shutdown_rx to periodic task and break on shutdown signal.  

FINDING #8: JWT-based URLs break HTTP endpoint derivation  
SEVERITY: P1  
AXIS: c  
DESCRIPTION: HTTP reconciliation uses core.url but WS connection may use JWT-modified URL (main.rs:2060-2230). Derived HTTP URL could be invalid when JWT is active.  
EVIDENCE: main.rs:2060-2230 shows JWT URL modification  
PROPOSED MITIGATION: Reuse the same connect_url logic from WS loop for HTTP base URL.  

FINDING #9: Double FFB-zero emission on synth+real sequence  
SEVERITY: P3  
AXIS: c  
DESCRIPTION: apply_session_ended emits FfbZeroed message (ws_handler.rs:383). When HTTP synth precedes real WS frame, two zero messages are sent despite idempotency guard.  
EVIDENCE: ws_handler.rs:383 shows FfbZeroed emission in SessionEnded handler  
PROPOSED MITIGATION: Move FfbZeroed emission to WS-specific path, not shared apply_session_ended.  

FINDING #10: CSV fallback push duplicates data on synth+real  
SEVERITY: P2  
AXIS: b  
DESCRIPTION: HTTP-synthesized SessionEnded triggers CSV push (ws_handler.rs:413). Real SessionEnded later would retrigger, causing duplicate server records.  
EVIDENCE: ws_handler.rs:413 shows detached CSV push task  
PROPOSED MITIGATION: Skip CSV push when origin=HttpSynth or add idempotency key to push request.  

FINDING #11: Heartbeat_status race with reconciliation check  
SEVERITY: P1  
AXIS: a  
DESCRIPTION: heartbeat_status.billing_active is checked with Relaxed ordering, but may be concurrently updated by failure_monitor_tx during session transitions.  
EVIDENCE: Algorithm uses Relaxed load for billing_active gate  
PROPOSED MITIGATION: Use Acquire ordering for load and synchronize with failure_monitor_tx updates.  

FINDING #12: Untestable silent-reconnect-death scenario  
SEVERITY: P3  
AXIS: d  
DESCRIPTION: Verification requires organic WS-drop with active session. No synthetic trigger exists, making pre-deploy validation impossible for primary failure mode.  
EVIDENCE: Deploy scope: "Cannot be synthetically triggered without real billing session + real WS drop"  
PROPOSED MITIGATION: Add debug endpoint to force silent_reconnect_suspected state for testing.
