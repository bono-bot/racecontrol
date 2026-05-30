//! Heart-V2 session surface — the in-process Rust port of `apps/mock-heart`.
//!
//! Owns the V2 pod-session lifecycle state machine + pod-state projection
//! (`PodStateSnapshot`) + an SSE firehose. The admin proxies POST session ops
//! here (`/heart/sessions/*`) and subscribe to `/heart/pods/state/stream`. In
//! the sandbox these hit `apps/mock-heart` (:8090); in production
//! `RACECONTROL_HEART_URL` points at this crate (`.23:8080`). This module makes
//! the real heart serve the contract the (already-built) TS proxy/billing/panel
//! stack calls — closing the binding gap to first-INR.
//!
//! Scope (first increment): launch / pause / resume / switch-game / end (all
//! idempotent on terminal/no-op states) + pod read + SSE stream + alarm
//! set/clear/ack. NOT a wallet (the proxy owns the wallet + 402 gate). NOT
//! lobby/ac-server (deferred). NOT the rc-agent green-light handshake (billing
//! starts at launch via `green_light_at = now`, mock-heart parity).
//!
//! Discipline carried from the V1-dependent-V2 RCA (`.planning/specs/v2/
//! RCA-heart-v2-session-surface-20260530.md`) + MMA Step-1:
//!  - Never hold a lock across `.await` (snapshot under guard → drop → broadcast).
//!  - `#[serde(deny_unknown_fields)]` on request bodies (no silent serde-drop).
//!  - Idempotent transitions (proxy forwards are at-least-once).
//!  - `let _ = tx.send()` (broadcast send never panics on no-subscribers).
//!  - Heart state is in-memory + V2-isolated (never touches V1 billing tables).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::{
        IntoResponse, Response,
        sse::{Event, KeepAlive, Sse},
    },
    routing::{get, post},
};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::state::AppState;

/// 8 pods per venue layout (canonical PODS_TOTAL substrate; mock-heart parity).
const POD_COUNT: usize = 8;

// ─── Wire types (mirror packages/contracts/openapi/session.yaml, snake_case) ──

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PodLifecycle {
    Empty,
    Occupied,
    Maintenance,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    Preflight,
    Loading,
    Ready,
    Running,
    Paused,
    Ending,
    Ended,
    AutoBilled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlarmPhase {
    PreWarning,
    Active,
    Grace,
    Silenced,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BalanceRunoutAlarm {
    pub phase: AlarmPhase,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub runout_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub grace_started_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub grace_expires_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub grace_expired_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub acknowledged_by_staff_user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub acknowledged_at: Option<String>,
    pub computed_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PodSession {
    pub id: String,
    pub household_id: String,
    pub profile_id: String,
    pub pod_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preset_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lobby_id: Option<String>,
    pub state: SessionState,
    pub tier: String,
    pub game: String,
    pub started_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub green_light_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paused_at: Option<String>,
    pub pause_ms_total: i64,
    pub credits_debited: i64,
}

/// = `PodStateSnapshot` on the SSE wire (the panels' subscription shape).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PodState {
    pub pod_id: String,
    pub lifecycle: PodLifecycle,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_session: Option<PodSession>,
    pub display_message: String,
    pub updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alarm: Option<BalanceRunoutAlarm>,
}

// ─── Request bodies (deny_unknown_fields — no silent serde-drop, RCA §4.2) ────

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LaunchReq {
    pub pod_id: String,
    pub household_id: String,
    pub profile_id: String,
    pub tier: String,
    pub game: String,
    #[serde(default)]
    pub lobby_id: Option<String>,
    #[serde(default)]
    pub preset_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SwitchGameReq {
    pub game: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EndReq {
    pub end_reason: String,
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AckReq {
    pub acknowledged_by_staff_user_id: Option<String>,
}

impl Default for AckReq {
    fn default() -> Self {
        Self { acknowledged_by_staff_user_id: None }
    }
}

// ─── In-memory store (pods + sessions behind ONE lock; mock-heart parity) ─────

/// Outcome of `HeartStore::launch`.
pub enum LaunchOutcome {
    Ok { session: PodSession, snapshot: PodState },
    PodNotFound,
    PodNotEmpty(String),
    Maintenance,
}

#[derive(Default)]
pub struct HeartStore {
    pub pods: HashMap<String, PodState>,
    /// Retains ended sessions so re-delivered end/pause are idempotent (200),
    /// not 404 (mock-heart `state.sessions` semantics).
    pub sessions: HashMap<String, PodSession>,
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

impl HeartStore {
    pub fn new() -> Self {
        let now = now_iso();
        let mut pods = HashMap::new();
        for i in 1..=POD_COUNT {
            let id = format!("pod-{i}");
            pods.insert(
                id.clone(),
                PodState {
                    pod_id: id,
                    lifecycle: PodLifecycle::Empty,
                    current_session: None,
                    display_message: "WELCOME".to_string(),
                    updated_at: now.clone(),
                    alarm: None,
                },
            );
        }
        Self { pods, sessions: HashMap::new() }
    }

    pub fn launch(&mut self, req: LaunchReq) -> LaunchOutcome {
        match self.pods.get(&req.pod_id) {
            None => return LaunchOutcome::PodNotFound,
            Some(pod) => {
                if let Some(s) = &pod.current_session {
                    return LaunchOutcome::PodNotEmpty(s.id.clone());
                }
                if pod.lifecycle == PodLifecycle::Maintenance {
                    return LaunchOutcome::Maintenance;
                }
            }
        }
        let now = now_iso();
        let session = PodSession {
            id: Uuid::new_v4().to_string(),
            household_id: req.household_id,
            profile_id: req.profile_id,
            pod_id: req.pod_id.clone(),
            preset_id: req.preset_id,
            lobby_id: req.lobby_id,
            state: SessionState::Running,
            tier: req.tier,
            game: req.game.clone(),
            started_at: now.clone(),
            green_light_at: Some(now.clone()),
            paused_at: None,
            pause_ms_total: 0,
            credits_debited: 0,
        };
        self.sessions.insert(session.id.clone(), session.clone());
        let pod = self.pods.get_mut(&req.pod_id).expect("pod existence checked above");
        pod.lifecycle = PodLifecycle::Occupied;
        pod.current_session = Some(session.clone());
        pod.display_message = format!("RUNNING · {}", req.game);
        pod.updated_at = now;
        // Stale alarm from a prior session must not survive into a fresh one —
        // kiosk subscribers paint from the snapshot; a leftover active alarm
        // would fire audibly (mock-heart Task #536).
        pod.alarm = None;
        let snapshot = pod.clone();
        LaunchOutcome::Ok { session, snapshot }
    }

    /// Sync the pod's embedded `current_session` copy from the canonical
    /// `sessions` map + stamp a display message. Returns the pod snapshot.
    fn sync_pod_running(&mut self, session: &PodSession, message: String) -> Option<PodState> {
        let pod = self.pods.get_mut(&session.pod_id)?;
        pod.current_session = Some(session.clone());
        pod.display_message = message;
        pod.updated_at = now_iso();
        Some(pod.clone())
    }

    pub fn pause(&mut self, sid: &str) -> Option<(PodSession, Option<PodState>)> {
        let session = self.sessions.get_mut(sid)?;
        if session.state != SessionState::Running {
            return Some((session.clone(), None)); // idempotent no-op
        }
        session.state = SessionState::Paused;
        session.paused_at = Some(now_iso());
        let session = session.clone();
        let snap = self.sync_pod_running(&session, "SESSION PAUSED".to_string());
        Some((session, snap))
    }

    pub fn resume(&mut self, sid: &str) -> Option<(PodSession, Option<PodState>)> {
        let session = self.sessions.get_mut(sid)?;
        if session.state != SessionState::Paused {
            return Some((session.clone(), None)); // idempotent no-op
        }
        if let Some(paused_at) = session.paused_at.take() {
            if let Ok(t) = chrono::DateTime::parse_from_rfc3339(&paused_at) {
                let delta = (chrono::Utc::now() - t.with_timezone(&chrono::Utc)).num_milliseconds();
                if delta > 0 {
                    session.pause_ms_total += delta;
                } else {
                    tracing::warn!(
                        delta,
                        sid = %session.id,
                        "heart resume: non-positive pause delta (clock skew?) — pause time not accumulated"
                    );
                }
            }
        }
        session.state = SessionState::Running;
        let session = session.clone();
        let msg = format!("RUNNING · {}", session.game);
        let snap = self.sync_pod_running(&session, msg);
        Some((session, snap))
    }

    pub fn switch_game(&mut self, sid: &str, new_game: String) -> Option<(PodSession, Option<PodState>)> {
        let session = self.sessions.get_mut(sid)?;
        session.game = new_game.clone();
        session.state = SessionState::Loading;
        let session = session.clone();
        let snap = self.sync_pod_running(&session, format!("LOADING · {new_game}"));
        Some((session, snap))
    }

    /// Delayed loading→running transition (mock-heart 50ms launcher mimic).
    /// Returns a snapshot only if the flip actually happened.
    pub fn complete_switch(&mut self, sid: &str) -> Option<PodState> {
        let session = self.sessions.get_mut(sid)?;
        if session.state != SessionState::Loading {
            return None;
        }
        session.state = SessionState::Running;
        let session = session.clone();
        let msg = format!("RUNNING · {}", session.game);
        self.sync_pod_running(&session, msg)
    }

    pub fn end(&mut self, sid: &str, reason: &str) -> Option<(PodSession, Option<PodState>)> {
        let session = self.sessions.get_mut(sid)?;
        if session.state == SessionState::Ended || session.state == SessionState::AutoBilled {
            return Some((session.clone(), None)); // idempotent
        }
        session.state = if reason == "balance_runout" || reason == "pause_cap_exceeded" {
            SessionState::AutoBilled
        } else {
            SessionState::Ended
        };
        let session = session.clone();
        let snap = if let Some(pod) = self.pods.get_mut(&session.pod_id) {
            pod.current_session = None;
            pod.lifecycle = PodLifecycle::Empty;
            pod.display_message = "THANK YOU".to_string();
            pod.updated_at = now_iso();
            // Auto-end + staff-end both clear the alarm (CR-4: alarm auto-clears
            // on end so the kiosk audible stops + grace banner disappears).
            pod.alarm = None;
            Some(pod.clone())
        } else {
            None
        };
        Some((session, snap))
    }

    /// Project a balance-runout alarm onto the pod. Idempotent: same
    /// phase+runout_at preserves the original grace timestamps (mock-heart).
    pub fn set_alarm(&mut self, pod_id: &str, next: BalanceRunoutAlarm) -> Option<PodState> {
        let pod = self.pods.get_mut(pod_id)?;
        if let Some(prev) = &pod.alarm {
            if prev.phase == next.phase && prev.runout_at == next.runout_at {
                return Some(pod.clone()); // idempotent — keep original timestamps
            }
        }
        pod.alarm = Some(next);
        pod.updated_at = now_iso();
        Some(pod.clone())
    }

    pub fn clear_alarm(&mut self, pod_id: &str) -> Option<PodState> {
        let pod = self.pods.get_mut(pod_id)?;
        if pod.alarm.is_none() {
            return Some(pod.clone());
        }
        pod.alarm = None;
        pod.updated_at = now_iso();
        Some(pod.clone())
    }

    /// Staff ack: active→grace, idempotent on grace, else→silenced. `None`
    /// pod = 404; `Some(None)` = pod exists but has no alarm (404 no_alarm).
    pub fn acknowledge_alarm(&mut self, pod_id: &str, staff: &str) -> Option<Option<PodState>> {
        let pod = self.pods.get_mut(pod_id)?;
        let Some(alarm) = pod.alarm.as_mut() else {
            return Some(None);
        };
        let now = now_iso();
        match alarm.phase {
            AlarmPhase::Active => {
                alarm.phase = AlarmPhase::Grace;
                if alarm.grace_started_at.is_none() {
                    alarm.grace_started_at = Some(now.clone());
                }
                alarm.acknowledged_by_staff_user_id = Some(staff.to_string());
                alarm.acknowledged_at = Some(now.clone());
                alarm.computed_at = now.clone();
            }
            AlarmPhase::Grace => {
                alarm.acknowledged_by_staff_user_id = Some(staff.to_string());
                if alarm.acknowledged_at.is_none() {
                    alarm.acknowledged_at = Some(now.clone());
                }
                alarm.computed_at = now.clone();
            }
            _ => {
                alarm.phase = AlarmPhase::Silenced;
                alarm.computed_at = now.clone();
            }
        }
        pod.updated_at = now;
        Some(Some(pod.clone()))
    }

    pub fn all_pods(&self) -> Vec<PodState> {
        self.pods.values().cloned().collect()
    }

    pub fn get_pod(&self, pod_id: &str) -> Option<PodState> {
        self.pods.get(pod_id).cloned()
    }
}

// ─── HTTP handlers (thin wrappers: lock → mutate → drop → broadcast) ──────────

fn err(status: StatusCode, code: &str, message: String) -> Response {
    (status, Json(json!({ "code": code, "message": message }))).into_response()
}

async fn list_pods(State(state): State<Arc<AppState>>) -> Response {
    let pods = state.heart.read().await.all_pods();
    (StatusCode::OK, Json(json!({ "pods": pods }))).into_response()
}

async fn get_pod(State(state): State<Arc<AppState>>, Path(pod_id): Path<String>) -> Response {
    match state.heart.read().await.get_pod(&pod_id) {
        Some(p) => (StatusCode::OK, Json(p)).into_response(),
        None => err(StatusCode::NOT_FOUND, "not_found", format!("no pod {pod_id}")),
    }
}

async fn launch(State(state): State<Arc<AppState>>, Json(req): Json<LaunchReq>) -> Response {
    let pod_id = req.pod_id.clone();
    let outcome = {
        let mut store = state.heart.write().await;
        store.launch(req)
    };
    match outcome {
        LaunchOutcome::Ok { session, snapshot } => {
            let _ = state.heart_stream_tx.send(snapshot);
            (StatusCode::OK, Json(json!({ "session": session }))).into_response()
        }
        LaunchOutcome::PodNotFound => {
            err(StatusCode::NOT_FOUND, "pod_not_found", format!("unknown pod {pod_id}"))
        }
        LaunchOutcome::PodNotEmpty(sid) => err(
            StatusCode::CONFLICT,
            "pod_not_empty",
            format!("pod {pod_id} already running {sid}"),
        ),
        LaunchOutcome::Maintenance => {
            err(StatusCode::CONFLICT, "pod_in_maintenance", format!("pod {pod_id} is in maintenance"))
        }
    }
}

/// Shared tail for pause/resume/end: broadcast the snapshot, return the session.
fn session_response(result: Option<(PodSession, Option<PodState>)>, state: &Arc<AppState>) -> Response {
    match result {
        Some((session, snap)) => {
            if let Some(s) = snap {
                let _ = state.heart_stream_tx.send(s);
            }
            (StatusCode::OK, Json(session)).into_response()
        }
        None => err(StatusCode::NOT_FOUND, "not_found", "session not found".to_string()),
    }
}

async fn pause(State(state): State<Arc<AppState>>, Path(sid): Path<String>) -> Response {
    let result = state.heart.write().await.pause(&sid);
    session_response(result, &state)
}

async fn resume(State(state): State<Arc<AppState>>, Path(sid): Path<String>) -> Response {
    let result = state.heart.write().await.resume(&sid);
    session_response(result, &state)
}

async fn switch_game(
    State(state): State<Arc<AppState>>,
    Path(sid): Path<String>,
    Json(req): Json<SwitchGameReq>,
) -> Response {
    let result = state.heart.write().await.switch_game(&sid, req.game);
    let resp = session_response(result, &state);
    // Mimic the launcher: flip loading→running after a short delay (mock-heart
    // 50ms). Spawned task re-checks state==loading before flipping.
    if resp.status() == StatusCode::OK {
        let state2 = state.clone();
        let sid2 = sid;
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            let snap = state2.heart.write().await.complete_switch(&sid2);
            if let Some(s) = snap {
                let _ = state2.heart_stream_tx.send(s);
            }
        });
    }
    resp
}

async fn end(State(state): State<Arc<AppState>>, Path(sid): Path<String>, Json(req): Json<EndReq>) -> Response {
    let result = state.heart.write().await.end(&sid, &req.end_reason);
    session_response(result, &state)
}

async fn set_alarm(
    State(state): State<Arc<AppState>>,
    Path(pod_id): Path<String>,
    Json(body): Json<BalanceRunoutAlarm>,
) -> Response {
    let snap = state.heart.write().await.set_alarm(&pod_id, body);
    match snap {
        Some(p) => {
            let _ = state.heart_stream_tx.send(p.clone());
            (StatusCode::OK, Json(p)).into_response()
        }
        None => err(StatusCode::NOT_FOUND, "not_found", format!("no pod {pod_id}")),
    }
}

async fn clear_alarm(State(state): State<Arc<AppState>>, Path(pod_id): Path<String>) -> Response {
    let snap = state.heart.write().await.clear_alarm(&pod_id);
    match snap {
        Some(p) => {
            let _ = state.heart_stream_tx.send(p.clone());
            (StatusCode::OK, Json(p)).into_response()
        }
        None => err(StatusCode::NOT_FOUND, "not_found", format!("no pod {pod_id}")),
    }
}

async fn acknowledge_alarm(
    State(state): State<Arc<AppState>>,
    Path(pod_id): Path<String>,
    Json(req): Json<AckReq>,
) -> Response {
    let staff = req.acknowledged_by_staff_user_id.unwrap_or_else(|| "staff-unknown".to_string());
    let outcome = state.heart.write().await.acknowledge_alarm(&pod_id, &staff);
    match outcome {
        Some(Some(p)) => {
            let _ = state.heart_stream_tx.send(p.clone());
            (StatusCode::OK, Json(p)).into_response()
        }
        Some(None) => err(
            StatusCode::NOT_FOUND,
            "no_active_alarm",
            format!("pod {pod_id} has no active alarm"),
        ),
        None => err(StatusCode::NOT_FOUND, "not_found", format!("no pod {pod_id}")),
    }
}

fn sse_event(p: &PodState) -> Result<Event, std::convert::Infallible> {
    Ok(Event::default().data(serde_json::to_string(p).unwrap_or_else(|_| "{}".to_string())))
}

/// SSE firehose. Emits a cold-boot snapshot of every pod on connect, then a
/// delta on every state mutation, plus a 15s keep-alive comment (CR-10 trip).
async fn pods_state_stream(State(state): State<Arc<AppState>>) -> Response {
    let initial: Vec<Result<Event, std::convert::Infallible>> =
        state.heart.read().await.all_pods().iter().map(sse_event).collect();
    let rx = state.heart_stream_tx.subscribe();
    // On `Lagged` (a slow subscriber overflowed the broadcast ring) resend a
    // full snapshot of every pod so the panel re-syncs to current truth instead
    // of silently skipping the missed deltas (MAOR finding). `pending` buffers
    // those resync frames so the unfold yields them one at a time.
    let live = futures_util::stream::unfold(
        (rx, state.clone(), std::collections::VecDeque::<PodState>::new()),
        |(mut rx, st, mut pending)| async move {
            if let Some(p) = pending.pop_front() {
                return Some((sse_event(&p), (rx, st, pending)));
            }
            loop {
                match rx.recv().await {
                    Ok(p) => return Some((sse_event(&p), (rx, st, pending))),
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                        tracing::warn!(skipped, "heart SSE subscriber lagged — resending full snapshot");
                        let snap = { st.heart.read().await.all_pods() };
                        for p in snap {
                            pending.push_back(p);
                        }
                        if let Some(p) = pending.pop_front() {
                            return Some((sse_event(&p), (rx, st, pending)));
                        }
                        continue;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => return None,
                }
            }
        },
    );
    let stream = futures_util::stream::iter(initial).chain(live);
    Sse::new(stream)
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(15)).text("heartbeat"))
        .into_response()
}

/// Bare `/heart/...` routes — merged at the ROOT in `build_router` (NOT under
/// `/api/v1`) so the proxy's `RACECONTROL_HEART_URL + /heart/...` lands here,
/// mock-heart drop-in parity. Unauthenticated: the heart is LAN-internal and
/// the admin proxy is the auth boundary (mock-heart "internal LAN-only").
pub fn heart_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/heart/pods", get(list_pods))
        .route("/heart/pods/state/stream", get(pods_state_stream))
        .route("/heart/pods/{pod_id}", get(get_pod))
        .route("/heart/pods/{pod_id}/alarm", post(set_alarm).delete(clear_alarm))
        .route("/heart/pods/{pod_id}/alarm/acknowledge", post(acknowledge_alarm))
        .route("/heart/sessions/launch", post(launch))
        .route("/heart/sessions/{sid}/pause", post(pause))
        .route("/heart/sessions/{sid}/resume", post(resume))
        .route("/heart/sessions/{sid}/switch-game", post(switch_game))
        .route("/heart/sessions/{sid}/end", post(end))
}

// ─── Tests (state machine + idempotency; no AppState needed) ──────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn launch_req(pod: &str) -> LaunchReq {
        LaunchReq {
            pod_id: pod.to_string(),
            household_id: "hh-1".to_string(),
            profile_id: "pf-1".to_string(),
            tier: "tier_1_full_skeleton".to_string(),
            game: "ac_sp".to_string(),
            lobby_id: None,
            preset_id: None,
        }
    }

    fn launch_ok(store: &mut HeartStore, pod: &str) -> PodSession {
        match store.launch(launch_req(pod)) {
            LaunchOutcome::Ok { session, .. } => session,
            _ => panic!("expected launch ok"),
        }
    }

    #[test]
    fn seeds_eight_empty_pods() {
        let store = HeartStore::new();
        assert_eq!(store.pods.len(), 8);
        assert!(store.pods.values().all(|p| p.lifecycle == PodLifecycle::Empty));
        assert_eq!(store.get_pod("pod-1").unwrap().display_message, "WELCOME");
    }

    #[test]
    fn launch_sets_running_and_green_light_and_occupies_pod() {
        let mut store = HeartStore::new();
        let s = launch_ok(&mut store, "pod-1");
        assert_eq!(s.state, SessionState::Running);
        assert!(s.green_light_at.is_some(), "billing starts at launch");
        let pod = store.get_pod("pod-1").unwrap();
        assert_eq!(pod.lifecycle, PodLifecycle::Occupied);
        assert_eq!(pod.current_session.unwrap().id, s.id);
    }

    #[test]
    fn launch_on_occupied_pod_is_conflict() {
        let mut store = HeartStore::new();
        launch_ok(&mut store, "pod-1");
        assert!(matches!(store.launch(launch_req("pod-1")), LaunchOutcome::PodNotEmpty(_)));
    }

    #[test]
    fn launch_unknown_pod_is_not_found() {
        let mut store = HeartStore::new();
        assert!(matches!(store.launch(launch_req("pod-99")), LaunchOutcome::PodNotFound));
    }

    #[test]
    fn pause_resume_switch_end_transitions() {
        let mut store = HeartStore::new();
        let s = launch_ok(&mut store, "pod-2");
        assert_eq!(store.pause(&s.id).unwrap().0.state, SessionState::Paused);
        assert_eq!(store.resume(&s.id).unwrap().0.state, SessionState::Running);
        assert_eq!(store.switch_game(&s.id, "f1_25".to_string()).unwrap().0.state, SessionState::Loading);
        assert_eq!(store.complete_switch(&s.id).unwrap().current_session.unwrap().state, SessionState::Running);
        let (ended, snap) = store.end(&s.id, "customer_stop").unwrap();
        assert_eq!(ended.state, SessionState::Ended);
        // pod freed, alarm cleared
        let pod_snap = snap.unwrap();
        assert_eq!(pod_snap.lifecycle, PodLifecycle::Empty);
        assert!(pod_snap.current_session.is_none());
    }

    #[test]
    fn end_is_idempotent_returns_200_not_404() {
        let mut store = HeartStore::new();
        let s = launch_ok(&mut store, "pod-3");
        store.end(&s.id, "customer_stop").unwrap();
        // Second end (proxy at-least-once re-delivery) must still resolve.
        let (again, snap) = store.end(&s.id, "customer_stop").expect("idempotent end");
        assert_eq!(again.state, SessionState::Ended);
        assert!(snap.is_none(), "no-op end does not re-broadcast");
    }

    #[test]
    fn balance_runout_ends_as_auto_billed() {
        let mut store = HeartStore::new();
        let s = launch_ok(&mut store, "pod-4");
        let (ended, _) = store.end(&s.id, "balance_runout").unwrap();
        assert_eq!(ended.state, SessionState::AutoBilled);
    }

    #[test]
    fn pause_on_missing_session_is_404() {
        let mut store = HeartStore::new();
        assert!(store.pause("no-such-session").is_none());
    }

    #[test]
    fn pause_idempotent_on_non_running() {
        let mut store = HeartStore::new();
        let s = launch_ok(&mut store, "pod-5");
        store.pause(&s.id);
        let (again, snap) = store.pause(&s.id).unwrap();
        assert_eq!(again.state, SessionState::Paused);
        assert!(snap.is_none(), "no-op pause does not re-broadcast");
    }

    #[test]
    fn alarm_set_clear_and_launch_clears_stale_alarm() {
        let mut store = HeartStore::new();
        let s = launch_ok(&mut store, "pod-6");
        let alarm = BalanceRunoutAlarm {
            phase: AlarmPhase::Active,
            runout_at: Some(now_iso()),
            grace_started_at: Some(now_iso()),
            grace_expires_at: Some(now_iso()),
            grace_expired_at: None,
            acknowledged_by_staff_user_id: None,
            acknowledged_at: None,
            computed_at: now_iso(),
        };
        assert!(store.set_alarm("pod-6", alarm).unwrap().alarm.is_some());
        // End clears the alarm (CR-4 auto-clear).
        store.end(&s.id, "balance_runout");
        assert!(store.get_pod("pod-6").unwrap().alarm.is_none());
        // A fresh launch on the same pod must not inherit a stale alarm.
        let _ = launch_ok(&mut store, "pod-6");
        assert!(store.get_pod("pod-6").unwrap().alarm.is_none());
    }

    #[test]
    fn launch_req_rejects_unknown_fields() {
        // deny_unknown_fields guards against silent serde-drop (RCA §4.2).
        let extra = r#"{"pod_id":"pod-1","household_id":"h","profile_id":"p","tier":"tier_1_full_skeleton","game":"ac_sp","bogus":true}"#;
        assert!(serde_json::from_str::<LaunchReq>(extra).is_err());
        let ok = r#"{"pod_id":"pod-1","household_id":"h","profile_id":"p","tier":"tier_1_full_skeleton","game":"ac_sp"}"#;
        assert!(serde_json::from_str::<LaunchReq>(ok).is_ok());
    }

    #[test]
    fn pod_state_serializes_snake_case_contract_shape() {
        let mut store = HeartStore::new();
        launch_ok(&mut store, "pod-7");
        let json = serde_json::to_value(store.get_pod("pod-7").unwrap()).unwrap();
        assert_eq!(json["lifecycle"], "occupied");
        assert_eq!(json["current_session"]["state"], "running");
        assert_eq!(json["pod_id"], "pod-7");
    }
}

// ─── HTTP integration tests (real router via oneshot — proves the wire) ───────

#[cfg(test)]
mod http_tests {
    use super::*;
    use axum::body::Body;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    async fn test_app() -> Router {
        let db = sqlx::SqlitePool::connect(":memory:").await.expect("memory db");
        let config = crate::config::Config::default_test();
        let field_cipher = crate::crypto::encryption::test_field_cipher();
        let state = Arc::new(AppState::new_with_test_v2db(config, db, field_cipher));
        heart_routes().with_state(state)
    }

    async fn call(
        app: &Router,
        method: &str,
        uri: &str,
        body: Option<serde_json::Value>,
    ) -> (u16, serde_json::Value) {
        let b = match body {
            Some(v) => Body::from(serde_json::to_vec(&v).unwrap()),
            None => Body::empty(),
        };
        let request = axum::http::Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json")
            .body(b)
            .unwrap();
        let resp = app.clone().oneshot(request).await.unwrap();
        let status = resp.status().as_u16();
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::json!(null));
        (status, json)
    }

    fn launch_body(pod: &str) -> serde_json::Value {
        serde_json::json!({
            "pod_id": pod, "household_id": "hh", "profile_id": "pf",
            "tier": "tier_1_full_skeleton", "game": "ac_sp"
        })
    }

    /// THE binding-gap closure: a real POST to the real Rust router returns a
    /// session, not a 404. (Production previously 404'd every session op.)
    #[tokio::test]
    async fn http_launch_returns_session_not_404() {
        let app = test_app().await;
        let (status, json) = call(&app, "POST", "/heart/sessions/launch", Some(launch_body("pod-1"))).await;
        assert_eq!(status, 200, "launch must NOT 404 — this is the binding-gap closure");
        assert_eq!(json["session"]["state"], "running");
        assert!(json["session"]["id"].as_str().is_some());
        assert!(json["session"]["green_light_at"].as_str().is_some());
    }

    #[tokio::test]
    async fn http_list_pods_returns_eight() {
        let app = test_app().await;
        let (status, json) = call(&app, "GET", "/heart/pods", None).await;
        assert_eq!(status, 200);
        assert_eq!(json["pods"].as_array().unwrap().len(), 8);
    }

    #[tokio::test]
    async fn http_full_lifecycle_launch_pause_end_frees_pod() {
        let app = test_app().await;
        let (_, launched) = call(&app, "POST", "/heart/sessions/launch", Some(launch_body("pod-2"))).await;
        let sid = launched["session"]["id"].as_str().unwrap().to_string();
        let (ps, _) = call(&app, "POST", &format!("/heart/sessions/{sid}/pause"), Some(serde_json::json!({}))).await;
        assert_eq!(ps, 200);
        let (es, ended) = call(&app, "POST", &format!("/heart/sessions/{sid}/end"), Some(serde_json::json!({"end_reason": "customer_stop"}))).await;
        assert_eq!(es, 200);
        assert_eq!(ended["state"], "ended");
        let (_, pod) = call(&app, "GET", "/heart/pods/pod-2", None).await;
        assert_eq!(pod["lifecycle"], "empty", "pod freed on end");
    }

    #[tokio::test]
    async fn http_end_unknown_session_is_404() {
        let app = test_app().await;
        let (status, _) = call(&app, "POST", "/heart/sessions/no-such/end", Some(serde_json::json!({"end_reason": "customer_stop"}))).await;
        assert_eq!(status, 404);
    }

    #[tokio::test]
    async fn http_launch_unknown_field_is_rejected_4xx() {
        let app = test_app().await;
        let bad = serde_json::json!({
            "pod_id": "pod-1", "household_id": "h", "profile_id": "p",
            "tier": "tier_1_full_skeleton", "game": "ac_sp", "bogus": 1
        });
        let (status, _) = call(&app, "POST", "/heart/sessions/launch", Some(bad)).await;
        assert!((400..500).contains(&status), "deny_unknown_fields must reject (4xx), got {status}");
        assert_ne!(status, 200, "unknown field must NOT be silently accepted");
    }

    /// SSE endpoint mounts + returns the right headers. The body is an infinite
    /// stream — we assert status + content-type WITHOUT collecting it.
    #[tokio::test]
    async fn http_sse_stream_mounts_with_event_stream() {
        let app = test_app().await;
        let request = axum::http::Request::builder()
            .method("GET")
            .uri("/heart/pods/state/stream")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(request).await.unwrap();
        assert_eq!(resp.status().as_u16(), 200);
        let ct = resp.headers().get("content-type").and_then(|v| v.to_str().ok()).unwrap_or("");
        assert!(ct.starts_with("text/event-stream"), "expected SSE content-type, got: {ct}");
    }
}
