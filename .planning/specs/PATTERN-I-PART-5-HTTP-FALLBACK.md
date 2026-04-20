# Pattern I Part 5 — SessionEnded HTTP Fallback (DESIGN-SPEC)

**Status:** Post-MMA-Step-1 (updated 2026-04-20 20:05 IST with 10 consensus improvements from 5-model audit at `mma-part5/FINDINGS-STEP1.md`). Not coded. Not deployed.
**Owner:** James session 2026-04-20.
**Scope:** rc-agent + one shared helper in rc_common (server endpoint already exists + fleet/health server-side rule for D3 rollback-detection).
**MMA requirement:** Step 1 DIAGNOSE done ($0.04, 5 models, 9 consensus findings). Step 2 PLAN next.

## Goal

When WebSocket misses a `CoreToAgentMessage::SessionEnded` — because the reconnect loop went silent-dead (Pod 6 2026-04-20 class), because a WS flap dropped the frame, or because the server's broadcast races a handshake — rc-agent currently keeps a local `active_session_id` forever. Customer sees:

- `lock_screen_state=active_session` indefinitely
- game process potentially still running past time-out
- no "Session Complete" summary card
- SN-01 (a13942f2) blanks the screen within 15 s **only after the game process is gone** — it does not synthesise the summary card and does not run `end_billing_session()` side effects

This part closes that class: rc-agent periodically asks the server "is session X still live?" and if the authoritative answer is "no", rc-agent replays the same side effects it would have done on a real `SessionEnded` frame.

## Server endpoint contract (exists)

- **Route:** `GET /api/v1/billing/active`
- **Handler:** `api/billing_views.rs:326 active_billing_sessions`
- **Current response:** `{"sessions": [BillingSessionInfo]}` — all active timers across all pods
- **Auth:** registered in `public_routes` at `api/routes.rs:499` (no staff JWT required) — safe for rc-agent to call with its existing service-key HTTP client
- **Shape of `BillingSessionInfo`:** TBD at MMA time — field names for `id`, `pod_id`, `driver_id`, `driver_name`, `started_at`, `allocated_seconds` must be stable enough that rc-agent can identify "my" session and synthesise a `SessionEnded` frame

## Local state

- **Source of truth for "my active session":** `shutdown_monitor.borrow().active_billing_session_id` (`main.rs:2009`) — already populated by `failure_monitor_tx.send_modify` in the real SessionEnded path
- **Heartbeat gate:** `state.heartbeat_status.billing_active.load(Relaxed)` — short-circuit the HTTP check when rc-agent knows no session is active
- **Driver-name cache:** `conn.current_driver_name` (reset to `None` by the WS handler at line 376 and 413) — needed for the synthesised summary

## Triggers

**T1 — post-reconnect-success fire.** Injection point: `main.rs:2214` right after `"Connected and registered as Pod #{}"`. Fires once per successful WS re-registration.

**T2 — 5-minute periodic tick.** Background `tokio::spawn` launched at boot, polls every 300 s regardless of WS state. Rationale: T1 alone does not cover the silent-loop-death class because the loop itself never reconnects — a disjoint timer is required.

**T3 — WS state-change fire (deferred — not in this patch).** When `silent_reconnect_suspected` flips false→true locally, could trigger an immediate probe. Out of scope this round; T1+T2 covers the two observed failure classes.

## Algorithm (MMA-Step-1-updated)

```
fetch_and_reconcile():
  // (1) Short-circuit when rc-agent knows no session is active
  if !state.heartbeat_status.billing_active.load():  return

  // (2) Load local session id + set-time (C2 60s grace)
  let monitor = state.failure_monitor_tx.borrow();
  let local_id = monitor.active_billing_session_id.clone();
  let set_at = monitor.active_billing_session_id_set_at;  // NEW — Instant
  if local_id.is_none():  return
  if set_at.elapsed() < 60s:  return                                      // C2: grace window

  // (3) HTTP call — with service key (C5) + shared url helper (C7)
  let base = rc_common::url::http_base_from_ws(&state.config.core.url)    // C7
  let resp = http_client
      .get(format!("{base}/api/v1/billing/active"))
      .header("X-Service-Key", state.config.core.service_key())           // C5
      .timeout(5s)
      .send().await;

  // (4) Status gate BEFORE JSON parse (D9)
  let body = match resp:
    Ok(r) if r.status().is_success() => r.json::<BillingActiveResponse>().await,
    Ok(r) => { emit(fallback_http_status=r.status()); return; }           // 401/5xx → no synth
    Err(_) | timeout => return;                                           // never synth on server-down

  // (5) pod_id filter (C6) — only consider sessions for THIS pod
  let my_pod = state.config.pod.number.to_string();
  let server_my_sessions: Vec<&BillingSessionInfo> =
      body.sessions.iter().filter(|s| s.pod_id == my_pod).collect();

  // (6) Membership check on session_id
  let still_present = server_my_sessions.iter().any(|s| s.id == local_id);
  if still_present:
      emit(INFO fallback_version=part5_v1 result=live); return            // D3 marker
  emit(INFO fallback_version=part5_v1 result=synth_triggered);            // D3 marker

  // (7) Dedup guard on AppState — fires only once per session id (D6)
  {
      let mut guard = state.last_applied_session_end.write().await;       // AppState, not ConnectionState
      if guard.as_deref() == Some(local_id): return
      *guard = Some(local_id.clone());
  }

  // (8) Use server-carried driver_name + cached last-known stats (C4 + C8)
  let driver = server_my_sessions
      .iter()
      .find(|s| s.id == local_id)  // may be None post-filter — will fallback to cache
      .map(|s| s.driver_name.clone())
      .or_else(|| monitor.session_last_known_driver.clone())
      .unwrap_or_default();
  let last_known = monitor.session_last_known_stats.clone();   // (laps, best_ms, driving_s)

  apply_session_ended(
      state, conn, ws_tx,
      local_id, driver,
      last_known.total_laps, last_known.best_lap_ms, last_known.driving_seconds,
      SessionEndOrigin::HttpSynth {                                       // structured log tag
          fallback_version: "part5_v1",
          synth_reason: SynthReason::ServerOmittedId,
      },
  ).await;
```

### T2 periodic tick with cancellation (C3)

```rust
pub fn spawn_session_end_fallback_tick(
    state: Arc<AppState>, cancel: CancellationToken,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(300));
        ticker.tick().await;  // burn first immediate tick
        loop {
            tokio::select! {
                _ = cancel.cancelled() => { tracing::info!("part5 T2 tick cancelled"); break; }
                _ = ticker.tick()      => { fetch_and_reconcile(&state, &mut conn, &mut ws_tx).await; }
            }
        }
    })
}
```

T1 (post-reconnect) fires the same `fetch_and_reconcile` inline at `main.rs:2214` — no separate task needed there.

## Refactor (preparatory — one commit before the T1/T2 wiring)

Extract body of `ws_handler.rs:379-445` (the entire `SessionEnded` arm PLUS the CSV push and its URL derivation) into:

```rust
pub(crate) enum SessionEndOrigin {
    WsReal,
    HttpSynth { fallback_version: &'static str, synth_reason: SynthReason },
}

pub(crate) async fn apply_session_ended(
    state: &Arc<AppState>,
    conn: &mut ConnectionState,
    ws_tx: &mut WsTxSink,
    billing_session_id: String,
    driver_name: String,
    total_laps: i64,
    best_lap_ms: Option<i64>,
    driving_seconds: i64,
    origin: SessionEndOrigin,
) -> Result<(), ApplyError>
```

**D11 invariant:** the extracted body MUST reset — with char-for-char equivalence to the inlined arm:
- `conn.inactivity_monitor = None` + `reset()` before drop
- `state.lock_screen.dismiss_countdown_warning()`
- `state.heartbeat_status.billing_active.store(false, Release)`
- `crate::remote_ops::BILLING_ACTIVE.store(false, Release)`
- `conn.crash_recovery = CrashRecoveryState::Idle`  ← D11 flagged this
- `state.overlay.deactivate()`
- `state.last_ac_status = None`
- `state.ac_status_stable_since = None`
- `conn.launch_state = LaunchState::Idle`
- `failure_monitor_tx.send_modify` all 5 fields
- `ffb_controller::safe_session_end(&state.ffb).await`
- `show_session_summary(...)`
- `game.stop()`, `adapter.disconnect()`, FfbZeroed emit, `enforce_safe_state(true)`
- `conn.current_driver_name = None`
- `blank_timer.reset()` + `blank_timer_armed = true` (only on first-apply; see C9 guard below)
- CSV push (unchanged logic; origin-agnostic)

Characterisation test `test_apply_session_ended_char_for_char` diffs observable state before/after inlined arm vs extracted fn.

### C1 / C9 dedup + upgrade path

```rust
// Dedup guard lives on AppState (D6), not ConnectionState
pub struct AppState { ..., pub last_applied_session_end: RwLock<Option<String>>, ... }

// Inside apply_session_ended, immediately after argument validation:
let is_first_apply = {
    let mut g = state.last_applied_session_end.write().await;
    if g.as_deref() == Some(&billing_session_id) { false }
    else { *g = Some(billing_session_id.clone()); true }
};

if !is_first_apply {
    // Second application arriving — C1 says: don't re-run lifecycle,
    // but DO refresh the summary card if origin=WsReal (real stats supersede zeros)
    match origin {
        SessionEndOrigin::WsReal => {
            refresh_summary_card(state, driver_name, total_laps, best_lap_ms, driving_seconds,
                conn.session_max_speed_kmh, conn.session_race_position).await;
            tracing::info!("session_end refresh with real stats after synth");
        }
        SessionEndOrigin::HttpSynth { .. } => {
            tracing::debug!("session_end synth dedup'd — already applied");
        }
    }
    return Ok(());  // C9: skip blank_timer reset on second-apply
}
// First-apply path continues below — full side effects including blank_timer reset
```

`refresh_summary_card` is a thin helper that ONLY calls `state.lock_screen.show_session_summary(...)` with the new stats — no blank_timer touch, no FFB action, no game.stop (all already idempotent-true).

## Idempotency + authority (MMA-updated)

- **Server is authoritative on "session is over".** If the server's `/billing/active` response omits a matching `(pod_id, session_id)` pair, that session is ended from the database's perspective (it was removed from `state.billing.active_timers` when `end_billing_session()` ran server-side).
- **Never synth on server-error / server-unreachable.** Explicit status-code gate at (4) in the algorithm (D9) — 401/5xx returns early with no synth, no dedup-guard mutation.
- **Idempotency of `apply_session_ended`:** first-apply does full side effects; second-apply with `origin=WsReal` calls `refresh_summary_card` only; second-apply with `origin=HttpSynth` is debug-logged no-op. See refactor section.
- **Dedup guard lives on `AppState.last_applied_session_end: RwLock<Option<String>>`** — NOT on ConnectionState (D6). ConnectionState is rebuilt on every reconnect loop iteration; placing the guard there would wipe it and allow duplicate applies after a silent reconnect.
- **60 s grace window (C2):** `active_billing_session_id` might be populated before the server inserts into `active_timers` (race between billing/start handler and its own DB write). During the window the HTTP fallback would see the session "missing" and synth prematurely. Guard: `active_billing_session_id_set_at: Instant` on FailureMonitorState, populated in the same `send_modify` call that sets the id. Fallback skips if `elapsed() < 60s`.
- **Pod-id filter (C6):** server response may carry sessions for other pods. Filter by `pod_id == state.config.pod.number.to_string()` before running the membership check. Trusting bare session_id is unsafe against any future ID-recycle scheme or cross-pod test scenarios.
- **Rollback detection (D3):** every T1 fire AND every synth emits `fallback_version=part5_v1` structured log. Server-side rule in `/fleet/health` composite check: any pod holding `active_session_id` without recent SessionEnded > 15 min AND without `fallback_version` log observed in last 30 min → flag "stuck-session candidate, pre-patch binary suspected". Fires during a botched rollback.

## Failure modes the MMA must enumerate

Seed list (non-exhaustive — MMA should expand):

1. **Clock-skew:** what if server's `started_at` for a session is in the future due to NTP drift? (Local session tracking uses server-supplied IDs, not timestamps — should be fine. Confirm.)
2. **Race with real SessionEnded:** HTTP probe concludes "session gone" 200ms before the WS frame arrives. Guard above handles.
3. **Synthesised `driving_seconds` / `total_laps`:** if rc-agent doesn't have these locally, synthesise with `0, None, 0`. Customer sees a zeroed-out summary card. Better than no card? MMA to decide.
4. **Server cold-restart:** mid-request, server returns `503`. We must NOT treat unreachable as "session ended".
5. **Server returns stale/cached 304:** unlikely (handler always fresh-reads), but MMA should confirm.
6. **Multi-pod session:** `BillingSessionInfo` includes `pod_id`. Do we filter to `pod_id == state.config.pod.number` before comparison, or trust the unique session id? Current design trusts session id — MMA should flag if pod_id mismatch would be a bug.
7. **Session id is synthesised locally before server acks:** rc-agent could have a `launching` or `pre-active` state where `active_billing_session_id` is set but the server hasn't yet inserted into `active_timers`. HTTP probe would say "gone" → false-positive synth. Guard: only fire the HTTP fallback if local session id has been `active_billing_session_id` for ≥ 60 s.
8. **Server URL derivation:** `ws_handler.rs:423-429` reuses `core.url.replace("ws://", "http://").split("/ws")`. New code must use the same pattern or (better) a shared helper — MMA to flag.
9. **Authentication:** the `/billing/active` route is public. But several `public_routes` still gate on `X-Service-Key` for non-health endpoints. Confirm at MMA time and add header if needed — matching the pattern `remote_ops.rs` uses for its own authenticated server calls.
10. **Partial server response:** JSON parses but `sessions` field missing. Treat as "unknown" (not "empty") and skip synth.
11. **Tokio task cancellation / shutdown race:** background 5-minute tick must respect the rc-agent shutdown signal so it doesn't emit the synth during graceful exit.
12. **Log flood:** if HTTP fallback fires every 5 min with no active session (gate short-circuits) — only INFO-level log on actual reconciliation event; DEBUG on "nothing to do".

## Tests (MMA-updated)

Characterisation first (D11):
- `test_apply_session_ended_char_for_char` — before extraction, snapshot observable state deltas from the inlined arm (overlay state, BILLING_ACTIVE, blank_timer deadline, LaunchState, inactivity_monitor, crash_recovery, FfbZeroed emitted, current_driver_name, failure_monitor fields). After extraction, same snapshot via `apply_session_ended(..WsReal)`. Assert char-for-char equivalence.

Dedup + upgrade (C1, C9, D6):
- `test_first_apply_runs_full_side_effects` — fresh AppState, verify all 16 state mutations from refactor invariant
- `test_second_apply_wsreal_refreshes_summary_only` — after HttpSynth at t=0 with zeros, WsReal at t=5 with real stats. Verify: `show_session_summary` called again (with real stats), blank_timer NOT re-armed, BILLING_ACTIVE still false (idempotent), no second FfbZeroed emit
- `test_second_apply_httpsynth_debug_noop` — HttpSynth at t=0, HttpSynth at t=5. Verify debug log only, no side effects
- `test_dedup_guard_survives_reconnect` — apply session-X, rebuild ConnectionState, apply session-X again. Guard still blocks (D6: lives on AppState)

Trigger gating (C2, C3, C5, C6, D9):
- `test_http_synth_skips_on_server_unreachable` — mock HTTP timeout, verify no dedup-guard mutation
- `test_http_synth_skips_on_server_5xx` — mock 503, verify no synth (D9 status gate)
- `test_http_synth_skips_on_server_401` — mock 401, verify no synth + structured log with `fallback_http_status=401` (C5 diag)
- `test_http_synth_skips_when_no_local_session` — no `active_billing_session_id`, verify zero HTTP calls
- `test_http_synth_honours_60s_grace` — session id set 30 s ago, server omits it. Verify skip.
- `test_http_synth_fires_when_server_omits_session` — session id set 120 s ago, server response omits. Verify apply_session_ended called with origin=HttpSynth.
- `test_http_synth_filters_by_pod_id` — server returns my-session-id on different pod_id. Verify NO synth (C6: filter takes effect).
- `test_t2_tick_respects_cancellation` — spawn T2 task, cancel token. Verify loop exits within 1s without firing another tick (C3).

Shared helper (C7):
- `test_http_base_from_ws_strips_path` — 4 variants: `ws://h:p/ws`, `wss://h:p/ws?jwt=x`, `ws://h:p`, `wss://h:p/ws/pods/N`. All strip to `http(s)://h:p`.

Rollback observability (D3):
- `test_t1_emits_fallback_version_marker` — mock WS reconnect-success, verify structured log contains `fallback_version="part5_v1"`
- `test_synth_emits_fallback_version_marker` — trigger synth, verify marker present

## Deploy scope (MMA-updated)

- **Build targets (2):**
  - `rc-agent` binary (primary change — T1, T2, apply_session_ended refactor)
  - `racecontrol` binary server-side (D3 rollback-detection rule in `/fleet/health` — tiny: one `stuck_session_candidate: bool` field + composite rule)
- **Shared types (1):** `BillingSessionInfo` struct pinned in `rc_common` (D13) — both crates depend on it for deploy-safe wire-format stability
- **Shared helper (1):** `rc_common::url::http_base_from_ws(&str) -> String` (C7)
- **Pre-merge probe:** before merging the fallback PR, curl `/api/v1/billing/active` from an anonymous source (no X-Service-Key, no staff JWT) to confirm route is genuinely public. If it returns 401, fallback would silently fail → block merge until route auth is either (a) explicitly public, or (b) fallback is updated to include service-key header (C5)
- **Binary swap order:**
  1. racecontrol server first (rollback-detection rule needs to see the new `fallback_version` log to work)
  2. Pod 8 canary rc-agent — observe 15+ min on live customer sessions
  3. Remaining pods 1-7 in one fleet wave via rc-sentry `/exec` atomic rename per standing rule
- **No config change**
- **No DB migration**
- **No frontend rebuild**
- **Cloud parity:** rc-agent has no cloud presence. `racecontrol` server-side D3 rule MUST deploy to Bono VPS too (per "Fix one system? Fix ALL systems" rule) — cloud-side server also runs the fleet/health composite check.
- **Verification target:** organic WS-drop event on any pod where `active_billing_session_id.is_some()`. Cannot be synthetically triggered without a real billing session + real WS drop. Runbook: monitor `fleet/health` for `silent_reconnect_suspected=true AND active_session_id set` after deploy, confirm within next ~24 h that no Pod 6-class stuck-overlay recurs. Simultaneously verify the D3 rollback-detection rule never flags a post-deploy pod (absence of flag = pods carry the fallback_version marker).

### Deploy-safety rollback plan

- rc-agent-prev.exe preserved 72 h per OTA rule
- racecontrol-prev.exe preserved 72 h
- Rollback path: if D3 rule starts flagging pods (meaning pre-patch binary is back in service) OR if customer reports re-emergence of stuck-session class, revert both binaries via the prev-swap pattern
- **Known rollback risk:** rolling back racecontrol while rc-agents still emit `fallback_version=part5_v1` is benign (server ignores unknown field). Rolling back rc-agents while racecontrol still has the D3 rule is also benign (rule emits a diagnostic flag but does nothing destructive — just shows up on fleet/health).

## MMA audit trail

- **Step 1 DIAGNOSE — DONE 2026-04-20 19:57 IST.** 5 models (deepseek-r1, qwen3-coder, nemotron-super, gemini-flash, grok-code). $0.04 / $3 budget. 9 consensus findings (≥3/5) across 4 axes + 1 dissenting P0 (D3 rollback regression). Full findings at [`mma-part5/FINDINGS-STEP1.md`](mma-part5/FINDINGS-STEP1.md). All 10 must-include updates folded into this spec above.
- **Step 2 PLAN — pending.** 5 models design fix plans against this updated spec. JSON format: `actions / risk / rollback` per spec section. Budget: $0.50-1.00.
- **Step 3 EXECUTE — pending.** Writer picks best plan + implements smallest reversible change. Coder role: deepseek-v3 or qwen3-coder or grok-code (tie-broken on availability).
- **Step 4 VERIFY — pending.** Deterministic checks (cargo test + the new test list above) THEN 3-model adversarial (different models from Steps 1-3). Score ≥4.0 = PASS per UNIFIED-MMA-PROTOCOL.md.
