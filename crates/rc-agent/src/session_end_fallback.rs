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

use std::time::{Duration, Instant};

use crate::app_state::AppState;
use crate::event_loop::ConnectionState;
use crate::ws_handler::{apply_session_ended, SessionEndOrigin, SynthReason, WsTx};
use rc_common::types::BillingSessionInfo;
use serde::Deserialize;

const LOG_TARGET: &str = "session-end-fallback";

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
    let base = rc_common::url::http_base_from_ws(&state.config.core.url);
    let url = format!("{}/api/v1/billing/active", base);
    let service_key = crate::mesh_key_cache::get_key_or_env(&state.mesh_key_cache)
        .await
        .unwrap_or_default();

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let my_pod_id = state.config.pod.number.to_string();

    let response = match client
        .get(&url)
        .header("X-Service-Key", &service_key)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::debug!(
                target: LOG_TARGET,
                fallback_version = FALLBACK_VERSION,
                error = %e,
                "skip: HTTP request failed (server unreachable or timeout)",
            );
            return;
        }
    };

    // Step 5 — status gate (D9) — never synth on 4xx/5xx
    if !response.status().is_success() {
        tracing::warn!(
            target: LOG_TARGET,
            fallback_version = FALLBACK_VERSION,
            http_status = %response.status(),
            "skip: server returned non-2xx",
        );
        return;
    }

    // Step 6 — JSON parse (D13: pinned struct in rc_common)
    let body: BillingActiveResponse = match response.json().await {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!(
                target: LOG_TARGET,
                fallback_version = FALLBACK_VERSION,
                error = %e,
                "skip: JSON parse failed (body shape mismatch?)",
            );
            return;
        }
    };

    // Step 7 — pod_id filter + membership check (C6)
    if server_has_my_session(&body.sessions, &my_pod_id, &local_id) {
        tracing::info!(
            target: LOG_TARGET,
            fallback_version = FALLBACK_VERSION,
            billing_session_id = %local_id,
            "no-op: server confirms session still live",
        );
        return;
    }

    // Step 8 — synth via apply_session_ended dedup guard. If the session id
    // was already applied (WsReal arrived first), the dedup guard makes this
    // a debug-log no-op. If not, full lifecycle fires with synthesised zeros.
    let driver_name = find_my_server_session(&body.sessions, &my_pod_id, &local_id)
        .map(|s| s.driver_name.clone())
        .unwrap_or_default();

    tracing::info!(
        target: LOG_TARGET,
        fallback_version = FALLBACK_VERSION,
        billing_session_id = %local_id,
        "synth triggered: server omitted session id (driver_name={:?})",
        driver_name,
    );

    apply_session_ended(
        state, conn, ws_tx,
        local_id, driver_name,
        0u32,           // total_laps — synth has no authoritative count
        None,           // best_lap_ms — synth has no authoritative value
        0u32,           // driving_seconds — synth has no authoritative value
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
}
