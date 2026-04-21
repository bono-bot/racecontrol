//! Pattern I Part 5 — SessionEnded HTTP fallback.
//!
//! When the WebSocket misses a `CoreToAgentMessage::SessionEnded` frame —
//! because the reconnect loop is silently flapping, because the server's
//! broadcast races a handshake, or because a single frame was dropped — the
//! pod keeps its locally-tracked `active_billing_session_id` forever. The
//! customer-facing symptom: `lock_screen_state=active_session` indefinitely,
//! no "Session Complete" summary card, and (on a pod where the game has
//! already exited) SN-01 blanks the screen within 15 s but never runs the
//! full `apply_session_ended` side effects.
//!
//! This module closes the loop by asking the authoritative server
//! (`GET /api/v1/billing/active`) whether the locally-tracked session is
//! still live. If the server response omits the session id (after filtering
//! by this pod's id), we synthesise a SessionEnded via `apply_session_ended`
//! with `origin=HttpSynth`.
//!
//! Triggers (both fire from `event_loop.rs`):
//! - **T1** — one-shot on every successful WS reconnect, in `event_loop::run`
//!   immediately after `ConnectionState::new()` and before the `select!` loop.
//!   Catches the missed-SessionEnded-during-brief-WS-drop class.
//! - **T2** — 300 s periodic tick as a `select!` arm inside the event loop.
//!   Catches the WS-flap-with-stale-connection class (WS appears live but the
//!   SessionEnded frame was dropped mid-handshake).
//!
//! **Not covered by this module:** silent-loop-death where the reconnect
//! loop wedges entirely (Pod 6 2026-04-20 class). Because the event loop
//! owns `&mut AppState`, T1 + T2 only fire when the event loop is iterating.
//! Silent-loop-death requires a process-exit dead-man's-switch (Part 4,
//! separate MMA audit) that bypasses the mutable-state contention.
//!
//! MMA Step 1 findings addressed here (or deliberately deferred):
//! - C2 (60 s grace) — pure helper `within_grace_window`
//! - C5 (X-Service-Key header) — populated via `mesh_key_cache::get_key_or_env`
//! - C6 (pod_id filter) — pure helper `server_has_my_session`
//! - C7 (shared url helper) — uses `rc_common::url::http_base_from_ws`
//! - D9 (status-code gate) — explicit `.status().is_success()` before parse
//! - D3 (fallback_version telemetry) — emitted on every T1/T2 fire + synth

#![cfg(feature = "http-client")]

use std::sync::OnceLock;
use std::time::{Duration, Instant};

use crate::app_state::AppState;
use crate::event_loop::ConnectionState;
use crate::ws_handler::{apply_session_ended, SessionEndOrigin, SynthReason, WsTx};
use rc_common::types::BillingSessionInfo;
use serde::Deserialize;

const LOG_TARGET: &str = "session-end-fallback";

/// V-B (Step 4 adversarial consensus, kimi V-4 P1 + mistral V-5 P2): reuse a
/// single `reqwest::Client` across every T1 + T2 invocation instead of
/// rebuilding per call. Module-level `OnceLock` so the construction is lazy
/// (deferred until the first HTTP fallback fires) and has no cross-module
/// dependency. TCP connection pool is retained across calls — same
/// (pod → server) pair is reused, no fd exhaustion under long uptime.
fn http_client() -> &'static reqwest::Client {
    static HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    HTTP_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new())
    })
}

/// V-A (Step 4 adversarial consensus, kimi V-1 P0 + mistral V-1 P1): explicit
/// bytes-read timeout separate from client-level timeout. Eliminates the
/// silent-drop class where the client-level timeout fires mid-`.json().await`
/// after the status check already passed, leaving the session stuck forever
/// with only a vague "body shape mismatch?" log.
const BYTES_READ_TIMEOUT: Duration = Duration::from_secs(5);

/// D3 rollback-detection marker. Every T1/T2 fire AND every synth emit this
/// marker. Server-side `stuck_session_candidate` rule (Commit 6) flags pods
/// lacking this marker despite holding an active session id.
pub(crate) const FALLBACK_VERSION: &'static str = "part5_v1";

/// MMA C2 P0 consensus: do NOT fire the HTTP fallback within the first 60 s
/// of a newly-set `active_billing_session_id`. Guards the race where rc-agent
/// has the id but the server hasn't yet inserted into `active_timers` — the
/// HTTP fallback would see the session "missing" and synth prematurely.
pub(crate) const GRACE_WINDOW: Duration = Duration::from_secs(60);

/// MMA C3 consensus: T2 tick cadence. The 5-minute interval matches the
/// existing process_guard + feature_flags periodic-refetch pattern and
/// balances detection latency against HTTP load.
pub(crate) const T2_TICK: Duration = Duration::from_secs(300);

/// Server `/api/v1/billing/active` response envelope.
/// Pinned here; `BillingSessionInfo` itself lives in `rc_common::types`.
#[derive(Debug, Deserialize)]
pub(crate) struct BillingActiveResponse {
    pub sessions: Vec<BillingSessionInfo>,
}

/// MMA C2: returns `true` when the grace window is still active — caller
/// should SKIP the HTTP fallback.
pub(crate) fn within_grace_window(set_at: Option<Instant>, now: Instant) -> bool {
    match set_at {
        Some(ts) => now.saturating_duration_since(ts) < GRACE_WINDOW,
        // No timestamp means either (a) no session tracked (caller should
        // short-circuit before reaching this fn) or (b) a legacy pre-Commit-2
        // code path missed the set_at update. Treating `None` as "grace
        // active" biases toward safety — never synth without the timestamp.
        None => true,
    }
}

/// MMA C6: filter server sessions by this pod's id, then check whether the
/// locally-tracked id is present. Returns `true` if still live — caller
/// should SKIP the synth.
pub(crate) fn server_has_my_session(
    sessions: &[BillingSessionInfo],
    my_pod_id: &str,
    local_id: &str,
) -> bool {
    sessions.iter()
        .filter(|s| s.pod_id == my_pod_id)
        .any(|s| s.id == local_id)
}

/// MMA C6 variant: find the server-returned BillingSessionInfo matching both
/// pod_id and session_id. Used to extract `driver_name` for the synthesised
/// summary card. Returns `None` if not present (caller falls back to cached).
pub(crate) fn find_my_server_session<'a>(
    sessions: &'a [BillingSessionInfo],
    my_pod_id: &str,
    local_id: &str,
) -> Option<&'a BillingSessionInfo> {
    sessions.iter()
        .find(|s| s.pod_id == my_pod_id && s.id == local_id)
}

/// Orchestrator. Called from `event_loop::run` on both T1 (post-reconnect)
/// and T2 (periodic tick) triggers.
///
/// Side effects (in order, all guarded):
/// 1. Heartbeat gate — if `billing_active=false`, return without HTTP call
/// 2. Local session tracking — if no `active_billing_session_id`, return
/// 3. Grace window — if set_at < 60 s ago, return (C2)
/// 4. HTTP GET `/api/v1/billing/active` with `X-Service-Key` (C5)
/// 5. Status gate — if non-2xx, return (D9)
/// 6. JSON parse — if malformed, return
/// 7. pod_id filter + session_id membership check (C6)
/// 8. If server still has my session: emit telemetry marker and return
/// 9. If server does NOT have my session: call `apply_session_ended` with
///    `origin=HttpSynth { fallback_version, synth_reason }` and let the
///    dedup guard + full lifecycle handle the synth
pub(crate) async fn fetch_and_reconcile(
    state: &mut AppState,
    conn: &mut ConnectionState,
    ws_tx: &mut WsTx,
) {
    // Step 1 — heartbeat gate
    if !state.heartbeat_status.billing_active.load(std::sync::atomic::Ordering::Relaxed) {
        return;
    }

    // Step 2 + 3 — snapshot local tracking state AND check grace window
    let (local_id, set_at) = {
        let monitor = state.failure_monitor_tx.borrow();
        (monitor.active_billing_session_id.clone(), monitor.active_billing_session_id_set_at)
    };
    let Some(local_id) = local_id else { return };
    if within_grace_window(set_at, Instant::now()) {
        tracing::debug!(
            target: LOG_TARGET,
            fallback_version = FALLBACK_VERSION,
            billing_session_id = %local_id,
            "skip: within 60s grace window",
        );
        return;
    }

    // Step 4 — HTTP call with service-key header (C5) + shared url helper (C7)
    // V-B: reuse module-level OnceLock client (no per-call rebuild).
    let base = rc_common::url::http_base_from_ws(&state.config.core.url);
    let url = format!("{}/api/v1/billing/active", base);
    let service_key = crate::mesh_key_cache::get_key_or_env(&state.mesh_key_cache)
        .await
        .unwrap_or_default();

    // Step-4-rerun W-2 (kimi P3): an empty service-key guarantees server 401.
    // The HTTP path below would log it at WARN level inside the status gate,
    // but surfacing the root cause here (missing key file vs network issue)
    // speeds operator triage. Mis-deployed pods missing the mesh-key file
    // are otherwise invisible — this elevates them to ERROR.
    if service_key.is_empty() {
        tracing::error!(
            target: LOG_TARGET,
            fallback_version = FALLBACK_VERSION,
            "skip: mesh service key unavailable (empty from mesh_key_cache + env) — fallback cannot authenticate",
        );
        return;
    }

    let my_pod_id = state.config.pod.number.to_string();

    let response = match http_client()
        .get(&url)
        .header("X-Service-Key", &service_key)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            // V-A: distinguish send-stage timeout (network / server down)
            // from other request errors. Both skip synth — never synth on
            // server-unreachable — but operators can grep the WARN-level
            // timeout marker during fleet incidents.
            if e.is_timeout() {
                tracing::warn!(
                    target: LOG_TARGET,
                    fallback_version = FALLBACK_VERSION,
                    error = %e,
                    stage = "send",
                    "skip: HTTP send timed out (network or server-unreachable)",
                );
            } else {
                tracing::debug!(
                    target: LOG_TARGET,
                    fallback_version = FALLBACK_VERSION,
                    error = %e,
                    stage = "send",
                    "skip: HTTP request failed",
                );
            }
            return;
        }
    };

    // Step 5 — status gate (D9) — never synth on 4xx/5xx
    let status = response.status();
    if !status.is_success() {
        tracing::warn!(
            target: LOG_TARGET,
            fallback_version = FALLBACK_VERSION,
            http_status = %status,
            "skip: server returned non-2xx",
        );
        return;
    }

    // Step 6 — V-A: bytes-read with EXPLICIT timeout separate from client-level.
    // Eliminates kimi-v1-P0 / mistral-v1-P1 race where client timeout fires
    // during `.json().await` after status-gate passed — previously logged as
    // "body shape mismatch?" (misleading) with silent-skip-synth behaviour.
    let bytes = match tokio::time::timeout(BYTES_READ_TIMEOUT, response.bytes()).await {
        Ok(Ok(b)) => b,
        Ok(Err(e)) => {
            tracing::warn!(
                target: LOG_TARGET,
                fallback_version = FALLBACK_VERSION,
                error = %e,
                stage = "body_read",
                "skip: body read failed (server closed mid-response?)",
            );
            return;
        }
        Err(_elapsed) => {
            tracing::warn!(
                target: LOG_TARGET,
                fallback_version = FALLBACK_VERSION,
                stage = "body_read",
                timeout_secs = BYTES_READ_TIMEOUT.as_secs(),
                "skip: body read timed out — network stall after 2xx status",
            );
            return;
        }
    };

    // Step 6b — V-A: in-memory JSON decode (no timeout, bytes already owned).
    // A parse failure here is a true schema mismatch, not a network event.
    let body: BillingActiveResponse = match serde_json::from_slice(&bytes) {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!(
                target: LOG_TARGET,
                fallback_version = FALLBACK_VERSION,
                error = %e,
                body_len = bytes.len(),
                stage = "parse",
                "skip: JSON parse failed (schema mismatch — rc_common::BillingSessionInfo out of sync with server?)",
            );
            return;
        }
    };

    // Step 7 — pod_id filter + membership check (C6)
    if let Some(live) = find_my_server_session(&body.sessions, &my_pod_id, &local_id) {
        // V-C + W-2: cache `driving_seconds` AND `driver_name` from every
        // successful live-session poll. When the session later disappears
        // and we synth, we replay the last-known values instead of 0/"".
        // BillingSessionInfo does not carry `total_laps` or `best_lap_ms`
        // — those remain None/0 for synth (tracked as follow-up; requires
        // WS lap-event wiring). Populating the caches here is the cheapest
        // point — we already have the struct in hand.
        let cached_driving = live.driving_seconds;
        let cached_driver = live.driver_name.clone();
        let _ = state.failure_monitor_tx.send_modify(|s| {
            s.session_last_known_driving_seconds = Some(cached_driving);
            s.session_last_known_driver = Some(cached_driver.clone());
        });
        tracing::info!(
            target: LOG_TARGET,
            fallback_version = FALLBACK_VERSION,
            billing_session_id = %local_id,
            cached_driving_seconds = cached_driving,
            cached_driver_name = %cached_driver,
            "no-op: server confirms session still live (driving_seconds + driver_name cached)",
        );
        return;
    }

    // Step 8 — synth via apply_session_ended dedup guard. If the session id
    // was already applied (WsReal arrived first), the dedup guard makes this
    // a debug-log no-op. If not, full lifecycle fires with cached stats
    // where available, zeros/empty where not.
    //
    // V-C + W-2: read cached values. Server no longer has the session (we
    // only reach this branch when `find_my_server_session` returned None
    // above), so the redundant find call from Commit 5 is gone — we read
    // straight from cache, closing mistral-W-1 (redundant-call perf) and
    // kimi/mimo-W-2 (driver_name empty-string class) simultaneously.
    let (cached_driving_seconds, cached_driver_name) = {
        let monitor = state.failure_monitor_tx.borrow();
        (
            monitor.session_last_known_driving_seconds.unwrap_or(0),
            monitor.session_last_known_driver.clone().unwrap_or_default(),
        )
    };

    tracing::info!(
        target: LOG_TARGET,
        fallback_version = FALLBACK_VERSION,
        billing_session_id = %local_id,
        synth_driving_seconds = cached_driving_seconds,
        synth_driver_name = %cached_driver_name,
        driving_cache_hit = cached_driving_seconds > 0,
        driver_cache_hit = !cached_driver_name.is_empty(),
        "synth triggered: server omitted session id",
    );

    apply_session_ended(
        state, conn, ws_tx,
        local_id, cached_driver_name,
        0u32,                   // total_laps — BillingSessionInfo lacks this; follow-up
        None,                   // best_lap_ms — BillingSessionInfo lacks this; follow-up
        cached_driving_seconds, // V-C: cached from last live-session poll (0 if never cached)
        SessionEndOrigin::HttpSynth {
            fallback_version: FALLBACK_VERSION,
            synth_reason: SynthReason::ServerOmittedId,
        },
    ).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use rc_common::types::{BillingSessionStatus, DrivingState};
    use chrono::Utc;

    fn mk_session(id: &str, pod_id: &str) -> BillingSessionInfo {
        BillingSessionInfo {
            id: id.to_string(),
            driver_id: "drv_test".to_string(),
            driver_name: format!("Driver of {}", id),
            pod_id: pod_id.to_string(),
            pricing_tier_name: "test_tier".to_string(),
            allocated_seconds: 1800,
            driving_seconds: 0,
            remaining_seconds: 1800,
            status: BillingSessionStatus::Active,
            driving_state: DrivingState::Idle,
            started_at: Some(Utc::now()),
            split_count: 1,
            split_duration_minutes: None,
            current_split_number: 1,
            elapsed_seconds: None,
            cost_paise: None,
            rate_per_min_paise: None,
            billing_mode: None,
            recovery_pause_seconds: None,
            between_games_idle_seconds: None,
        }
    }

    #[test]
    fn within_grace_window_active_when_set_at_recent() {
        let now = Instant::now();
        let recent = now - Duration::from_secs(30);
        assert!(within_grace_window(Some(recent), now), "30s old must be within 60s grace");
    }

    #[test]
    fn within_grace_window_expired_when_set_at_old() {
        let now = Instant::now();
        let old = now - Duration::from_secs(120);
        assert!(!within_grace_window(Some(old), now), "120s old must be past 60s grace");
    }

    #[test]
    fn within_grace_window_boundary_exactly_60s_is_expired() {
        let now = Instant::now();
        let exactly = now - Duration::from_secs(60);
        assert!(!within_grace_window(Some(exactly), now), "exactly 60s must be NOT within grace (< strict)");
    }

    #[test]
    fn within_grace_window_none_biases_safe() {
        // C2 edge case: if set_at is None but local_id is Some, assume grace is
        // active rather than risk a false synth. Defensive — should not happen
        // in practice post-Commit-2 wiring.
        let now = Instant::now();
        assert!(within_grace_window(None, now), "None set_at must be treated as grace-active (safety bias)");
    }

    #[test]
    fn server_has_my_session_true_when_match_on_both_pod_and_id() {
        let sessions = vec![mk_session("sess_1", "pod-7")];
        assert!(server_has_my_session(&sessions, "pod-7", "sess_1"));
    }

    #[test]
    fn server_has_my_session_false_on_id_mismatch() {
        let sessions = vec![mk_session("sess_1", "pod-7")];
        assert!(!server_has_my_session(&sessions, "pod-7", "sess_different"));
    }

    #[test]
    fn server_has_my_session_false_on_pod_id_mismatch_even_when_id_matches() {
        // C6 P1 invariant: session id alone is not sufficient — must match pod_id
        // too. Guards against ID-recycle / cross-pod test data.
        let sessions = vec![mk_session("sess_shared_id", "pod-3")];
        assert!(!server_has_my_session(&sessions, "pod-7", "sess_shared_id"));
    }

    #[test]
    fn server_has_my_session_finds_in_multi_pod_response() {
        let sessions = vec![
            mk_session("sess_a", "pod-1"),
            mk_session("sess_b", "pod-3"),
            mk_session("sess_c", "pod-7"),
            mk_session("sess_d", "pod-8"),
        ];
        assert!(server_has_my_session(&sessions, "pod-7", "sess_c"));
        assert!(!server_has_my_session(&sessions, "pod-7", "sess_a"));
    }

    #[test]
    fn server_has_my_session_false_on_empty_response() {
        assert!(!server_has_my_session(&[], "pod-7", "any_id"));
    }

    #[test]
    fn find_my_server_session_extracts_driver_name() {
        let sessions = vec![mk_session("sess_target", "pod-7")];
        let found = find_my_server_session(&sessions, "pod-7", "sess_target");
        assert!(found.is_some());
        assert_eq!(found.unwrap().driver_name, "Driver of sess_target");
    }

    #[test]
    fn find_my_server_session_none_on_miss() {
        let sessions = vec![mk_session("sess_x", "pod-7")];
        assert!(find_my_server_session(&sessions, "pod-7", "sess_y").is_none());
        assert!(find_my_server_session(&sessions, "pod-3", "sess_x").is_none());
    }

    /// V-B: `http_client()` uses `OnceLock` — two calls must return the same
    /// underlying client instance (pointer equality via reference compare).
    /// Confirms the OnceLock memoization works and we're not accidentally
    /// rebuilding per call.
    #[test]
    fn http_client_is_memoized() {
        let a: *const reqwest::Client = super::http_client();
        let b: *const reqwest::Client = super::http_client();
        assert_eq!(a, b, "http_client() must return the same OnceLock instance on every call");
    }

    /// V-A: in-memory `serde_json::from_slice` happy path — confirms the
    /// post-refactor decode still accepts the BillingActiveResponse shape
    /// that Commit 5 pinned. This covers the parse branch after bytes-read,
    /// separate from network I/O (which needs HTTP mock for full coverage).
    #[test]
    fn bytes_then_from_slice_accepts_valid_response() {
        let json = br#"{"sessions":[]}"#;
        let decoded: BillingActiveResponse = serde_json::from_slice(json)
            .expect("empty sessions array must decode cleanly");
        assert_eq!(decoded.sessions.len(), 0);
    }

    #[test]
    fn bytes_then_from_slice_rejects_malformed_schema() {
        // V-A: schema mismatch (missing `sessions` field) must fail parse —
        // the WARN log at call site surfaces this as "schema mismatch" rather
        // than the previous "body shape mismatch?" ambiguity.
        let json = br#"{"foo":"bar"}"#;
        let result: Result<BillingActiveResponse, _> = serde_json::from_slice(json);
        assert!(result.is_err(), "missing required `sessions` field must fail decode");
    }

    /// V-C: on a live-session poll (session still present), the cache store
    /// uses `BillingSessionInfo.driving_seconds` directly. This test pins
    /// the field name so a rc_common rename surfaces at compile time.
    #[test]
    fn billing_session_info_exposes_driving_seconds_field() {
        let s = mk_session("sess_cache", "pod-7");
        // If this line stops compiling, rc_common renamed driving_seconds —
        // V-C cache path needs to follow. The mk_session helper above sets
        // driving_seconds: 0 explicitly; confirm we can read it back.
        let _v: u32 = s.driving_seconds;
    }

    /// W-2 (Step-4-rerun, kimi+mimo P2): the cache store path also copies
    /// `BillingSessionInfo.driver_name`. This test pins the field name + type
    /// so a rc_common rename surfaces at compile time (parallel to the
    /// driving_seconds test above).
    #[test]
    fn billing_session_info_exposes_driver_name_field() {
        let s = mk_session("sess_cache", "pod-7");
        // If this line stops compiling, rc_common renamed driver_name —
        // W-2 cache path needs to follow.
        let _v: String = s.driver_name.clone();
        assert_eq!(_v, "Driver of sess_cache");
    }

    /// W-3 (Step-4-rerun, mistral+mimo P3): V-A's explicit `bytes()` timeout
    /// is the critical fix for the kimi-V-1-P0 class. Full coverage requires
    /// an HTTP mock that stalls mid-body (out of scope — integration-only).
    /// This unit test covers the minimum: `tokio::time::timeout` on a
    /// never-completing future returns `Err(Elapsed)`, matching the branch
    /// the V-A fix relies on at `fetch_and_reconcile:249`.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn tokio_timeout_returns_elapsed_on_stalled_future() {
        // Simulate a network body read that never completes within the
        // BYTES_READ_TIMEOUT budget. In production this is `response.bytes()`
        // stalled on a server that sent headers + zero bytes of body.
        let stalled = std::future::pending::<()>();
        let result = tokio::time::timeout(BYTES_READ_TIMEOUT, stalled).await;
        assert!(result.is_err(),
            "tokio::time::timeout MUST return Err on a pending future — this is the mechanism V-A relies on");

        // Also confirm the error type is `tokio::time::error::Elapsed` so
        // pattern-match on `Err(_elapsed)` in fetch_and_reconcile keeps working.
        let elapsed_err = result.err().unwrap();
        let type_name = std::any::type_name_of_val(&elapsed_err);
        assert!(type_name.contains("Elapsed"),
            "timeout error type changed from Elapsed — V-A error-handler pattern match needs update: was {}", type_name);
    }

    /// W-3 companion: cache-hit / cache-miss round-trip for the new W-2 field.
    /// Confirms `FailureMonitorState::session_last_known_driver` survives a
    /// Default construction + assignment cycle, which is the semantic the
    /// live-session branch depends on.
    #[test]
    fn failure_monitor_state_driver_cache_round_trip() {
        use crate::failure_monitor::FailureMonitorState;
        let mut s = FailureMonitorState::default();
        assert_eq!(s.session_last_known_driver, None,
            "new FailureMonitorState must start with no cached driver");
        s.session_last_known_driver = Some("Raikkonen".to_string());
        assert_eq!(s.session_last_known_driver.as_deref(), Some("Raikkonen"));
    }

    /// W-3 (mistral Step-4-rerun, conf 5/5): cache-miss path in synth branch.
    /// When no live-session poll ever cached driver+seconds (e.g. SessionEnded
    /// fires before first /billing/active success), the synth branch must
    /// resolve to "0 seconds + empty driver name" via unwrap_or(0) /
    /// unwrap_or_default(), not panic and not pull stale data.
    #[test]
    fn failure_monitor_state_cache_miss_resolves_to_zero_and_empty() {
        use crate::failure_monitor::FailureMonitorState;
        let s = FailureMonitorState::default();
        let cached_seconds = s.session_last_known_driving_seconds.unwrap_or(0);
        let cached_driver = s.session_last_known_driver.clone().unwrap_or_default();
        assert_eq!(cached_seconds, 0,
            "cache-miss seconds must resolve to 0 (not panic, not sentinel)");
        assert_eq!(cached_driver, "",
            "cache-miss driver must resolve to empty string (not panic, not stale)");
        assert!(cached_driver.is_empty(),
            "driver_cache_hit log field must read false on cache miss");
    }
}
