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

// `Deserialize` (+ `default` on skip-serialized Options) added for L3-1: the
// durable store round-trips a PodSession as a JSON blob. Additive — does not
// change the serialized (wire) shape.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PodSession {
    pub id: String,
    pub household_id: String,
    pub profile_id: String,
    pub pod_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lobby_id: Option<String>,
    pub state: SessionState,
    pub tier: String,
    pub game: String,
    pub started_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub green_light_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
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

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AckReq {
    pub acknowledged_by_staff_user_id: Option<String>,
}

// ─── In-memory store (pods + sessions behind ONE lock; mock-heart parity) ─────

/// Outcome of `HeartStore::launch`.
#[allow(clippy::large_enum_variant)]
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

    // ─── DELTA-A real-launch mutators (flag heart_v2_real_launch) ────────────
    // bridge RCA §5/§7: green-light is granted AFTER the rc-agent confirms the
    // game is Running (confirm-before-bill), never at request time.

    /// Reserve the pod with a LOADING session and NO green-light. Mirrors
    /// [`launch`] except `state=Loading` + `green_light_at=None`.
    pub fn launch_loading(&mut self, req: LaunchReq) -> LaunchOutcome {
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
            state: SessionState::Loading,
            tier: req.tier,
            game: req.game.clone(),
            started_at: now.clone(),
            green_light_at: None, // ← NOT granted until the agent confirms Running
            paused_at: None,
            pause_ms_total: 0,
            credits_debited: 0,
        };
        self.sessions.insert(session.id.clone(), session.clone());
        let pod = self.pods.get_mut(&req.pod_id).expect("pod existence checked above");
        pod.lifecycle = PodLifecycle::Occupied;
        pod.current_session = Some(session.clone());
        pod.display_message = format!("LOADING · {}", req.game);
        pod.updated_at = now;
        pod.alarm = None;
        let snapshot = pod.clone();
        LaunchOutcome::Ok { session, snapshot }
    }

    /// Promote a Loading session to Running + grant green-light. Called ONLY
    /// after the rc-agent closed-loop verify confirms the game is running.
    /// Idempotent: a re-delivered promote on an already-green-lit session no-ops.
    pub fn promote_to_running(&mut self, sid: &str) -> Option<(PodSession, Option<PodState>)> {
        let session = self.sessions.get_mut(sid)?;
        if session.state == SessionState::Running && session.green_light_at.is_some() {
            return Some((session.clone(), None));
        }
        session.state = SessionState::Running;
        if session.green_light_at.is_none() {
            session.green_light_at = Some(now_iso());
        }
        let session = session.clone();
        let msg = format!("RUNNING · {}", session.game);
        let snap = self.sync_pod_running(&session, msg);
        Some((session, snap))
    }

    /// Fail an in-flight launch: end the session WITHOUT green-light + free the
    /// pod. No money harm — the proxy never billed (green-light never granted).
    pub fn fail_launch(&mut self, sid: &str, reason: &str) -> Option<(PodSession, Option<PodState>)> {
        let session = self.sessions.get_mut(sid)?;
        if matches!(session.state, SessionState::Ended | SessionState::AutoBilled) {
            return Some((session.clone(), None));
        }
        tracing::warn!(sid, reason, "heart real-launch failed — ending session without green-light + freeing pod");
        session.state = SessionState::Ended;
        let session = session.clone();
        let snap = if let Some(pod) = self.pods.get_mut(&session.pod_id) {
            pod.current_session = None;
            pod.lifecycle = PodLifecycle::Empty;
            pod.display_message = "WELCOME".to_string();
            pod.updated_at = now_iso();
            pod.alarm = None;
            Some(pod.clone())
        } else {
            None
        };
        Some((session, snap))
    }

    /// R2 reconciler (bridge RCA §7): close the confirm-before-bill window. For
    /// each pod the rc-agent reports Running (`running_pods`), if the heart's
    /// live session has no green-light — the heart crashed post-Running,
    /// pre-green-light, so the customer would play FREE — grant it now. Returns
    /// the repaired sessions (caller persists + broadcasts). Idempotent: a pod
    /// whose session already has green-light is skipped.
    pub fn reconcile_green_light(&mut self, running_pods: &[String]) -> Vec<(PodSession, Option<PodState>)> {
        let mut repaired = Vec::new();
        for pod_id in running_pods {
            let sid = match self.pods.get(pod_id).and_then(|p| p.current_session.as_ref()) {
                Some(s) if matches!(s.state, SessionState::Running | SessionState::Loading)
                    && s.green_light_at.is_none() => s.id.clone(),
                _ => continue,
            };
            if let Some(s) = self.sessions.get_mut(&sid) {
                s.state = SessionState::Running;
                s.green_light_at = Some(now_iso());
                let s2 = s.clone();
                tracing::warn!(sid = %sid, pod = %pod_id, "R2 reconcile: agent reports Running but session had no green-light — granting now (closes free-play window)");
                let msg = format!("RUNNING · {}", s2.game);
                let snap = self.sync_pod_running(&s2, msg);
                repaired.push((s2, snap));
            }
        }
        repaired
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

    /// Rehydrate sessions from the durable store at boot (L3-1). Retains ENDED
    /// sessions in the map (cross-restart idempotency: a re-delivered end/pause
    /// must still find the session and return 200, not 404 — MMA nvidia/deepseek)
    /// but relinks `pod.current_session` ONLY for live (non-ended) sessions.
    /// MMA A2.RELINK edge handling:
    ///  - two live sessions for one pod → newest `started_at` wins (logged);
    ///  - session referencing an unknown pod → kept in map, pod not relinked (logged);
    ///  - ended + newer running on a pod → the running session wins (ended never relinks).
    pub fn apply_loaded_sessions(&mut self, loaded: Vec<PodSession>) {
        for mut s in loaded {
            // MMA-review fix A (CRITICAL): a session persisted as Loading means the
            // heart crashed inside the 50ms switch_game→complete_switch window; the
            // spawned completion task is gone and never re-fires. complete_switch
            // ALWAYS lands Running, so recover deterministically (idempotent across
            // repeated restarts) — otherwise the session hangs in Loading forever.
            if s.state == SessionState::Loading {
                tracing::warn!(sid = %s.id, "heart rehydrate: session was mid switch-game (Loading) at restart — recovering to Running");
                s.state = SessionState::Running;
            }
            self.sessions.insert(s.id.clone(), s.clone());
            let live = !matches!(s.state, SessionState::Ended | SessionState::AutoBilled);
            if !live {
                continue;
            }
            let Some(pod) = self.pods.get_mut(&s.pod_id) else {
                tracing::warn!(sid = %s.id, pod = %s.pod_id, "heart rehydrate: live session references unknown pod — kept in session map, pod not relinked");
                continue;
            };
            // started_at is RFC3339 UTC-millis from now_iso() (always 'Z'), so
            // lexicographic == chronological ordering for the newest-wins tie-break.
            if let Some(existing) = &pod.current_session {
                if existing.started_at >= s.started_at {
                    tracing::warn!(pod = %s.pod_id, kept = %existing.id, dropped = %s.id, "heart rehydrate: multiple live sessions for one pod — kept newest started_at");
                    continue;
                }
                tracing::warn!(pod = %s.pod_id, replaced = %existing.id, with = %s.id, "heart rehydrate: multiple live sessions for one pod — replaced with newer started_at");
            }
            pod.lifecycle = PodLifecycle::Occupied;
            // MMA-review fix B (IMPORTANT): derive the display from the rehydrated
            // state (was hardcoded "RUNNING · …" → a Paused session showed RUNNING
            // on the kiosk after a restart).
            pod.display_message = match s.state {
                SessionState::Paused => "SESSION PAUSED".to_string(),
                SessionState::Ending => "ENDING...".to_string(),
                _ => format!("RUNNING · {}", s.game),
            };
            pod.updated_at = now_iso();
            pod.current_session = Some(s);
        }
    }
}

// ─── Durable persistence (L3-1: survive heart restart) — MMA Step-1 consensus ──
// Per-mutation SQLite UPSERT write-through with memory-then-DB ordering;
// last-writer-wins on `updated_at`; boot rehydrates ALL sessions (incl. ended)
// so a re-delivered end/pause after a restart stays idempotent (200, not 404).
// No age-prune (would break that idempotency). V2-isolated: v2db only, no FK
// into V1 (heart-V2 RCA §1). Best-effort: a DB error is logged, never fails the
// live request — in-memory is authoritative and no money is held in the heart.

fn session_state_str(state: &SessionState) -> String {
    serde_json::to_value(state)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_else(|| "unknown".to_string())
}

/// Write-through one session to the durable store (UPSERT, last-writer-wins on
/// `updated_at`). Best-effort; logs on failure (in-memory remains authoritative).
pub async fn persist_session(v2db: &v2_db::DbPool, session: &PodSession) {
    let data = match serde_json::to_string(session) {
        Ok(d) => d,
        Err(e) => {
            tracing::error!(sid = %session.id, error = %e, "heart persist: serialize failed — session not durably stored");
            return;
        }
    };
    let updated_at = chrono::Utc::now().timestamp_millis();
    let state = session_state_str(&session.state);
    let res = sqlx::query(
        "INSERT INTO heart_v2_sessions (id, pod_id, state, updated_at, data) \
         VALUES (?1, ?2, ?3, ?4, ?5) \
         ON CONFLICT(id) DO UPDATE SET \
             pod_id = excluded.pod_id, state = excluded.state, \
             updated_at = excluded.updated_at, data = excluded.data \
         WHERE excluded.updated_at >= heart_v2_sessions.updated_at",
    )
    .bind(&session.id)
    .bind(&session.pod_id)
    .bind(&state)
    .bind(updated_at)
    .bind(&data)
    .execute(v2db)
    .await;
    if let Err(e) = res {
        tracing::error!(sid = %session.id, error = %e, "heart persist: write-through UPSERT failed — durability degraded (in-memory authoritative)");
    }
}

/// Load all persisted sessions (running + ended) for boot rehydration.
pub async fn load_sessions(v2db: &v2_db::DbPool) -> Vec<PodSession> {
    let rows: Vec<String> = match sqlx::query_scalar(
        "SELECT data FROM heart_v2_sessions ORDER BY updated_at ASC",
    )
    .fetch_all(v2db)
    .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(error = %e, "heart load: query failed — starting with empty sessions");
            return Vec::new();
        }
    };
    rows.into_iter()
        .filter_map(|data| match serde_json::from_str::<PodSession>(&data) {
            Ok(s) => Some(s),
            Err(e) => {
                tracing::error!(error = %e, "heart load: skipping un-deserializable session row");
                None
            }
        })
        .collect()
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
    // DELTA-A (bridge RCA §7): when the `heart_v2_real_launch` flag is ON, the
    // heart dispatches to the real rc-agent and grants green-light only after
    // confirmed-Running. Default OFF → unchanged sandbox behavior (mock
    // green-light at launch; keeps the existing sandbox/test suite green).
    let real_launch = state
        .feature_flags
        .read()
        .await
        .get("heart_v2_real_launch")
        .map(|f| f.enabled)
        .unwrap_or(false);
    if real_launch {
        return launch_real(state, req).await;
    }
    let pod_id = req.pod_id.clone();
    let outcome = {
        let mut store = state.heart.write().await;
        store.launch(req)
    };
    match outcome {
        LaunchOutcome::Ok { session, snapshot } => {
            persist_session(&state.v2db, &session).await;
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

/// DELTA-A real launch (flag `heart_v2_real_launch`): reserve a Loading session
/// (NO green-light) → dispatch to the rc-agent + closed-loop verify with the
/// heart lock DROPPED → promote to Running + green-light only on confirmed
/// Running, else fail (no green-light, pod freed). V2-isolated:
/// `billing_session_id=None` → creates no V1 billing row. Mock-green-light
/// scope: the proxy wallet HOLD+402 is cluster-2; full preset→launch_args is a
/// follow-up (dispatched with `None` args this increment — the PATH is proven,
/// the car/track content is deferred).
async fn launch_real(state: Arc<AppState>, req: LaunchReq) -> Response {
    use crate::game_launcher_ops::{AgentDispatchCtx, default_verify_timeout, dispatch_launch_to_agent};
    let pod_id = req.pod_id.clone();
    let game = req.game.clone();
    // 1) Reserve the pod with a Loading session (no green-light yet).
    let session = match state.heart.write().await.launch_loading(req) {
        LaunchOutcome::Ok { session, snapshot } => {
            persist_session(&state.v2db, &session).await;
            let _ = state.heart_stream_tx.send(snapshot);
            session
        }
        LaunchOutcome::PodNotFound => {
            return err(StatusCode::NOT_FOUND, "pod_not_found", format!("unknown pod {pod_id}"));
        }
        LaunchOutcome::PodNotEmpty(sid) => {
            return err(StatusCode::CONFLICT, "pod_not_empty", format!("pod {pod_id} already running {sid}"));
        }
        LaunchOutcome::Maintenance => {
            return err(StatusCode::CONFLICT, "pod_in_maintenance", format!("pod {pod_id} is in maintenance"));
        }
    };
    // 2) Dispatch to the rc-agent + closed-loop verify (heart lock NOT held).
    let sim_type = heart_game_to_sim_type(&game);
    let ctx = AgentDispatchCtx {
        launch_id: Uuid::new_v4().to_string(),
        billing_session_id: None, // V2-isolated — no V1 billing row
        duration_minutes: None,   // TODO follow-up: derive from the V2 tier
        origin: rc_common::protocol::LaunchOrigin::Customer,
        verify_timeout: default_verify_timeout(sim_type),
    };
    let dispatch = dispatch_launch_to_agent(&state, &pod_id, sim_type, None, ctx).await;
    // 3) Promote (confirmed Running → green-light) or fail (no green-light).
    match dispatch {
        Ok(o) if o.verified_running => {
            let promoted = { state.heart.write().await.promote_to_running(&session.id) };
            if let Some((sess, snap)) = promoted {
                persist_session(&state.v2db, &sess).await;
                if let Some(s) = snap {
                    let _ = state.heart_stream_tx.send(s);
                }
                (StatusCode::OK, Json(json!({ "session": sess }))).into_response()
            } else {
                // MAOR (google IMPORTANT): promote returning None means the
                // just-created session vanished from the store (state
                // corruption). The game IS running but the heart has no
                // green-lit session — do NOT report success; surface a 500.
                tracing::error!(sid = %session.id, pod = %pod_id, "real-launch: session missing at promote — state corruption");
                err(StatusCode::INTERNAL_SERVER_ERROR, "promote_failed", format!("pod {pod_id} launch verified but session state lost"))
            }
        }
        result => {
            let reason = match result {
                Ok(o) => format!("game not confirmed running (state {:?})", o.final_state),
                Err(e) => e,
            };
            let failed = { state.heart.write().await.fail_launch(&session.id, &reason) };
            if let Some((sess, snap)) = failed {
                persist_session(&state.v2db, &sess).await;
                if let Some(s) = snap {
                    let _ = state.heart_stream_tx.send(s);
                }
            }
            err(StatusCode::BAD_GATEWAY, "launch_failed", format!("pod {pod_id} launch not confirmed: {reason}"))
        }
    }
}

/// Map the heart session `game` string → rc-agent `SimType`. Best-effort:
/// serde first, then common aliases, else AssettoCorsa (documented fallback).
/// FOLLOW-UP: a real V2 game catalog (overlaps the preset surface).
fn heart_game_to_sim_type(game: &str) -> rc_common::types::SimType {
    use rc_common::types::SimType;
    if let Ok(st) = serde_json::from_value::<SimType>(serde_json::Value::String(game.to_string())) {
        return st;
    }
    match game.to_ascii_lowercase().as_str() {
        g if g.starts_with("ac") || g.contains("assetto") => SimType::AssettoCorsa,
        g if g.contains("f1") => SimType::F125,
        g if g.contains("iracing") => SimType::IRacing,
        g if g.contains("lmu") || g.contains("lemans") => SimType::LeMansUltimate,
        other => {
            tracing::warn!(game = other, "heart real-launch: unknown game string — defaulting to AssettoCorsa (follow-up: V2 game catalog)");
            SimType::AssettoCorsa
        }
    }
}

/// R2 (bridge RCA §7): one reconcile pass — for every pod the rc-agent reports
/// Running, grant green-light to a live heart session that has none (closes the
/// post-restart free-play window; also resolves the L3-1 stuck-Occupied
/// residual once the agent reconnects). Persists + broadcasts each repair.
/// Called at boot (after rehydrate) + periodically from `main.rs`.
pub async fn reconcile_heart_green_light_once(state: &Arc<AppState>) {
    use rc_common::types::GameState;
    let running_pods: Vec<String> = {
        let games = state.game_launcher.active_games.read().await;
        games
            .iter()
            // MAOR consistency: only a confirmed Running pod justifies granting
            // green-light (matches the dispatch verify tightening) — not a
            // transient Loading.
            .filter(|(_, t)| matches!(t.game_state, GameState::Running))
            .map(|(pod, _)| pod.clone())
            .collect()
    };
    if running_pods.is_empty() {
        return;
    }
    let repaired = { state.heart.write().await.reconcile_green_light(&running_pods) };
    for (sess, snap) in repaired {
        persist_session(&state.v2db, &sess).await;
        if let Some(s) = snap {
            let _ = state.heart_stream_tx.send(s);
        }
    }
}

/// Shared tail for pause/resume/switch-game/end: persist the session durably
/// (write-through BEFORE the SSE delta becomes observable — MMA A1 ordering, so
/// a subscriber never sees a state the store hasn't committed), then broadcast
/// the snapshot + return the session. `None` = unknown session (404). Persist is
/// best-effort (logged on failure; in-memory remains authoritative).
async fn persist_and_respond(
    result: Option<(PodSession, Option<PodState>)>,
    state: &Arc<AppState>,
) -> Response {
    match result {
        Some((session, snap)) => {
            persist_session(&state.v2db, &session).await;
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
    persist_and_respond(result, &state).await
}

async fn resume(State(state): State<Arc<AppState>>, Path(sid): Path<String>) -> Response {
    let result = state.heart.write().await.resume(&sid);
    persist_and_respond(result, &state).await
}

async fn switch_game(
    State(state): State<Arc<AppState>>,
    Path(sid): Path<String>,
    Json(req): Json<SwitchGameReq>,
) -> Response {
    let result = state.heart.write().await.switch_game(&sid, req.game);
    let resp = persist_and_respond(result, &state).await;
    // Mimic the launcher: flip loading→running after a short delay (mock-heart
    // 50ms). Spawned task re-checks state==loading before flipping.
    if resp.status() == StatusCode::OK {
        let state2 = state.clone();
        let sid2 = sid;
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            let snap = state2.heart.write().await.complete_switch(&sid2);
            if let Some(s) = snap {
                // Persist the post-flip Running state before broadcasting it.
                if let Some(sess) = s.current_session.clone() {
                    persist_session(&state2.v2db, &sess).await;
                }
                let _ = state2.heart_stream_tx.send(s);
            }
        });
    }
    resp
}

async fn end(State(state): State<Arc<AppState>>, Path(sid): Path<String>, Json(req): Json<EndReq>) -> Response {
    let result = state.heart.write().await.end(&sid, &req.end_reason);
    persist_and_respond(result, &state).await
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

    /// Like test_app, but with a MIGRATED file-based v2db (so heart_v2_sessions
    /// exists + a single shared DB across pool connections). Returns the AppState
    /// so a test can read the durable rows the handlers wrote. (new_with_test_v2db
    /// builds an unmigrated lazy in-memory pool — handler persists no-op there.)
    async fn test_app_with_migrated_v2db() -> (Router, Arc<AppState>) {
        let db = sqlx::SqlitePool::connect(":memory:").await.expect("memory db");
        let path = std::env::temp_dir().join(format!("heart_v2_http_{}.db", Uuid::new_v4()));
        let v2db = v2_db::open(path.to_str().unwrap()).await.expect("open v2db");
        v2_db::migrate(&v2db).await.expect("migrate v2db");
        let config = crate::config::Config::default_test();
        let field_cipher = crate::crypto::encryption::test_field_cipher();
        let state = Arc::new(AppState::new(config, db, v2db, field_cipher));
        (heart_routes().with_state(state.clone()), state)
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

    /// MMA-review fix D (CRITICAL test-validity): the unit-level persistence tests
    /// call persist_session directly, bypassing the axum handlers — removing the
    /// persist from persist_and_respond/launch would still pass all of them. This
    /// drives the PRODUCTION handler path over HTTP, then reads the durable row
    /// back to prove the handler wired the write-through.
    #[tokio::test]
    async fn http_handler_write_through_persists_to_db() {
        let (app, state) = test_app_with_migrated_v2db().await;
        let (status, json) = call(&app, "POST", "/heart/sessions/launch", Some(launch_body("pod-1"))).await;
        assert_eq!(status, 200);
        let sid = json["session"]["id"].as_str().unwrap().to_string();
        let row: Option<(String,)> = sqlx::query_as("SELECT state FROM heart_v2_sessions WHERE id = ?")
            .bind(&sid)
            .fetch_optional(&state.v2db)
            .await
            .unwrap();
        assert_eq!(row.map(|r| r.0), Some("running".to_string()), "launch handler must write the session through to the durable store");
        let (es, _) = call(&app, "POST", &format!("/heart/sessions/{sid}/end"), Some(serde_json::json!({"end_reason": "customer_stop"}))).await;
        assert_eq!(es, 200);
        let row2: Option<(String,)> = sqlx::query_as("SELECT state FROM heart_v2_sessions WHERE id = ?")
            .bind(&sid)
            .fetch_optional(&state.v2db)
            .await
            .unwrap();
        assert_eq!(row2.map(|r| r.0), Some("ended".to_string()), "end handler must write the ended state through");
    }
}

// ─── Persistence / restart-survival tests (L3-1) ──────────────────────────────
// Each test simulates a heart restart: write a session through to a real v2db,
// then rehydrate a FRESH HeartStore from it and assert the live state survived.
#[cfg(test)]
mod persistence_tests {
    use super::*;

    /// A real on-disk v2db (WAL) with all migrations applied (incl. the L3-1
    /// heart_v2_sessions table). Unique temp path per test so they don't collide.
    async fn mem_v2db() -> v2_db::DbPool {
        let path = std::env::temp_dir().join(format!("heart_v2_persist_{}.db", Uuid::new_v4()));
        let pool = v2_db::open(path.to_str().unwrap()).await.expect("open v2db");
        v2_db::migrate(&pool).await.expect("migrate v2db");
        pool
    }

    fn req(pod: &str) -> LaunchReq {
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

    fn launch(store: &mut HeartStore, pod: &str) -> PodSession {
        match store.launch(req(pod)) {
            LaunchOutcome::Ok { session, .. } => session,
            _ => panic!("expected launch ok"),
        }
    }

    /// THE L3-1 closure: a running session written through survives a heart
    /// restart — recovered into a fresh HeartStore with the pod relinked.
    #[tokio::test]
    async fn running_session_survives_restart() {
        let v2db = mem_v2db().await;
        let session = {
            let mut store = HeartStore::new();
            let s = launch(&mut store, "pod-1");
            persist_session(&v2db, &s).await;
            s
        };
        // Restart: a brand-new HeartStore (empty sessions, the pre-fix state)…
        let mut restarted = HeartStore::new();
        assert!(restarted.sessions.is_empty(), "fresh store starts empty — that was the gap");
        // …rehydrates from the durable store.
        restarted.apply_loaded_sessions(load_sessions(&v2db).await);
        let recovered = restarted.sessions.get(&session.id).expect("session recovered after restart");
        assert_eq!(recovered.state, SessionState::Running);
        assert_eq!(recovered.green_light_at, session.green_light_at, "billing signal preserved");
        let pod = restarted.get_pod("pod-1").unwrap();
        assert_eq!(pod.lifecycle, PodLifecycle::Occupied, "pod relinked to recovered session");
        assert_eq!(pod.current_session.unwrap().id, session.id);
    }

    /// Paused state + accumulated pause_ms survive a restart.
    #[tokio::test]
    async fn paused_state_survives_restart() {
        let v2db = mem_v2db().await;
        let sid = {
            let mut store = HeartStore::new();
            let s = launch(&mut store, "pod-2");
            persist_session(&v2db, &s).await;
            let (paused, _) = store.pause(&s.id).unwrap();
            persist_session(&v2db, &paused).await;
            s.id
        };
        let mut restarted = HeartStore::new();
        restarted.apply_loaded_sessions(load_sessions(&v2db).await);
        assert_eq!(restarted.sessions.get(&sid).unwrap().state, SessionState::Paused);
        // MMA-review fix B: pod display must reflect Paused, not "RUNNING · …".
        assert_eq!(restarted.get_pod("pod-2").unwrap().display_message, "SESSION PAUSED");
    }

    /// MMA idempotency catch (nvidia/deepseek): an ENDED session is retained
    /// across restart so a re-delivered end returns 200 (found), not 404 — and
    /// the pod is NOT relinked to the ended session.
    #[tokio::test]
    async fn ended_session_retained_and_pod_not_relinked() {
        let v2db = mem_v2db().await;
        let sid = {
            let mut store = HeartStore::new();
            let s = launch(&mut store, "pod-3");
            persist_session(&v2db, &s).await;
            let (ended, _) = store.end(&s.id, "customer_stop").unwrap();
            persist_session(&v2db, &ended).await;
            s.id
        };
        let mut restarted = HeartStore::new();
        restarted.apply_loaded_sessions(load_sessions(&v2db).await);
        let (again, snap) = restarted
            .end(&sid, "customer_stop")
            .expect("ended session retained across restart → re-delivered end is 200 not 404");
        assert_eq!(again.state, SessionState::Ended);
        assert!(snap.is_none(), "no-op re-end does not re-broadcast");
        assert_eq!(restarted.get_pod("pod-3").unwrap().lifecycle, PodLifecycle::Empty, "ended session must not relink the pod");
    }

    /// MMA A2.RELINK(c): an ended + a newer running session on the same pod →
    /// the pod relinks to the RUNNING one, never the ended one.
    #[tokio::test]
    async fn running_wins_over_ended_on_same_pod() {
        let v2db = mem_v2db().await;
        {
            let mut store = HeartStore::new();
            let s1 = launch(&mut store, "pod-4");
            let (e1, _) = store.end(&s1.id, "customer_stop").unwrap();
            persist_session(&v2db, &e1).await;
            let s2 = launch(&mut store, "pod-4");
            persist_session(&v2db, &s2).await;
        }
        let mut restarted = HeartStore::new();
        restarted.apply_loaded_sessions(load_sessions(&v2db).await);
        let pod = restarted.get_pod("pod-4").unwrap();
        assert_eq!(pod.lifecycle, PodLifecycle::Occupied);
        assert_eq!(
            pod.current_session.unwrap().state,
            SessionState::Running,
            "the running session wins relink, not the ended one"
        );
    }

    /// Empty store with no rows rehydrates cleanly (no panic, no spurious sessions).
    #[tokio::test]
    async fn empty_store_rehydrates_to_clean_state() {
        let v2db = mem_v2db().await;
        let mut restarted = HeartStore::new();
        restarted.apply_loaded_sessions(load_sessions(&v2db).await);
        assert!(restarted.sessions.is_empty());
        assert!(restarted.pods.values().all(|p| p.lifecycle == PodLifecycle::Empty));
    }

    /// MMA-review fix A (CRITICAL): a session persisted mid switch-game (Loading)
    /// — the heart crashed in the 50ms complete_switch window — must recover to
    /// Running on rehydration (the spawned completion task is gone forever), not
    /// hang in Loading. Pod display must read RUNNING, not LOADING.
    #[tokio::test]
    async fn loading_session_recovers_to_running_on_restart() {
        let v2db = mem_v2db().await;
        let sid = {
            let mut store = HeartStore::new();
            let s = launch(&mut store, "pod-5");
            persist_session(&v2db, &s).await;
            let (loading, _) = store.switch_game(&s.id, "f1_25".to_string()).unwrap();
            assert_eq!(loading.state, SessionState::Loading);
            // Persist Loading, then "crash" BEFORE complete_switch persists Running.
            persist_session(&v2db, &loading).await;
            s.id
        };
        let mut restarted = HeartStore::new();
        restarted.apply_loaded_sessions(load_sessions(&v2db).await);
        let rec = restarted.sessions.get(&sid).unwrap();
        assert_eq!(rec.state, SessionState::Running, "Loading must recover to Running, not hang");
        let pod = restarted.get_pod("pod-5").unwrap();
        assert_eq!(pod.lifecycle, PodLifecycle::Occupied);
        assert_eq!(pod.display_message, "RUNNING · f1_25");
    }

    /// MMA-review fix E (MINOR): two live (Running) sessions on one pod →
    /// rehydration relinks the NEWER started_at. (The launch guard prevents this
    /// in practice; rehydration must still be robust to a duplicated DB state.)
    #[tokio::test]
    async fn two_running_sessions_same_pod_newest_wins() {
        let v2db = mem_v2db().await;
        let (older, newer) = {
            let mut store = HeartStore::new();
            let s1 = launch(&mut store, "pod-6");
            let mut s2 = s1.clone();
            s2.id = format!("{}-newer", s1.id);
            s2.started_at = "2099-01-01T00:00:00.000Z".to_string(); // strictly later
            persist_session(&v2db, &s1).await;
            persist_session(&v2db, &s2).await;
            (s1.id, s2.id)
        };
        let mut restarted = HeartStore::new();
        restarted.apply_loaded_sessions(load_sessions(&v2db).await);
        let linked = restarted.get_pod("pod-6").unwrap().current_session.unwrap().id;
        assert_eq!(linked, newer, "pod must relink the newer started_at session");
        assert_ne!(linked, older);
    }
}

// ─── DELTA-A bridge tests (heart-V2 ↔ game_launch) — the 6 must-fix items ─────
// (bridge RCA §7 MUST-FIX-BEFORE-MERGE list.) Self-contained: exercises the real
// dispatch core + the heart real-launch mutators without the HTTP router.
#[cfg(test)]
mod bridge_tests {
    use super::*;
    use crate::game_launcher::GameTracker;
    use crate::game_launcher_ops::{AgentDispatchCtx, dispatch_launch_to_agent};
    use crate::state::AppState;
    use crate::state::CommandAckResult;
    use rc_common::protocol::{CoreMessage, LaunchOrigin};
    use rc_common::types::{GameState, SimType};
    use std::sync::Arc;
    use std::time::Duration;

    /// Minimal AppState for dispatch tests. `dispatch_launch_to_agent` never
    /// touches `state.db`, so a bare in-memory pool is sufficient.
    async fn build_state() -> Arc<AppState> {
        let db = sqlx::SqlitePool::connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite");
        let config = crate::config::Config::default_test();
        let field_cipher = crate::crypto::encryption::test_field_cipher();
        Arc::new(AppState::new_with_test_v2db(config, db, field_cipher))
    }

    fn req(pod: &str) -> LaunchReq {
        LaunchReq {
            pod_id: pod.to_string(),
            household_id: "hh".to_string(),
            profile_id: "pf".to_string(),
            tier: "tier_1_full_skeleton".to_string(),
            game: "ac_sp".to_string(),
            lobby_id: None,
            preset_id: None,
        }
    }

    fn ctx(timeout_ms: u64) -> AgentDispatchCtx {
        AgentDispatchCtx {
            launch_id: "L-test".to_string(),
            billing_session_id: None,
            duration_minutes: None,
            origin: LaunchOrigin::Customer,
            verify_timeout: Duration::from_millis(timeout_ms),
        }
    }

    fn running_tracker(pod: &str) -> GameTracker {
        GameTracker {
            pod_id: pod.to_string(),
            sim_type: SimType::AssettoCorsa,
            game_state: GameState::Running,
            pid: None,
            launched_at: Some(chrono::Utc::now()),
            error_message: None,
            launch_args: None,
            auto_relaunch_count: 0,
            externally_tracked: false,
            dynamic_timeout_secs: None,
            exit_codes: Vec::new(),
            max_auto_relaunch: 2,
            playable_at: None,
            ready_delay_ms: None,
            billing_session_id: None,
            launch_id: "v1-existing".to_string(),
        }
    }

    /// Register a mock pod agent on `pod` (normalized form, e.g. "pod_1"): drain
    /// the launch command, ACK it, and optionally flip the tracker to Running
    /// (what `handle_game_state_update` does when the real agent reports Live).
    async fn mock_agent(state: &Arc<AppState>, pod: &str, set_running: bool) -> tokio::task::JoinHandle<()> {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<CoreMessage>(4);
        state.agent_senders.write().await.insert(pod.to_string(), tx);
        let st = state.clone();
        let pod = pod.to_string();
        tokio::spawn(async move {
            if let Some(msg) = rx.recv().await {
                let cid = msg.command_id.clone().unwrap_or_default();
                if let Some(ack) = st.pending_command_acks.write().await.remove(&cid) {
                    let _ = ack.send(CommandAckResult { success: true, error: None });
                }
                if set_running {
                    if let Some(t) = st.game_launcher.active_games.write().await.get_mut(&pod) {
                        t.game_state = GameState::Running;
                    }
                }
            }
        })
    }

    // MUST-FIX #2 (closed-loop, happy path) + #1 (V1 isolation): ACK + Running
    // verifies, and the V2 dispatch creates NO V1 billing state.
    #[tokio::test]
    async fn dispatch_verifies_running_and_creates_no_v1_billing_state() {
        let state = build_state().await;
        let agent = mock_agent(&state, "pod_1", true).await;
        let out = dispatch_launch_to_agent(&state, "pod_1", SimType::AssettoCorsa, None, ctx(3000))
            .await
            .expect("dispatch ok");
        assert!(out.verified_running, "ACK + Running status must verify");
        // V1 isolation: the V2 launch must not create any V1 billing state.
        assert!(state.billing.active_timers.read().await.is_empty(), "V2 launch created a V1 active_timer");
        assert!(state.billing.waiting_for_game.read().await.is_empty(), "V2 launch created a V1 waiting_for_game entry");
        let bsid = state.game_launcher.active_games.read().await.get("pod_1").and_then(|t| t.billing_session_id.clone());
        assert!(bsid.is_none(), "V2 launch tracker must carry no V1 billing_session_id");
        let _ = agent.await;
    }

    // MUST-FIX #2/#6 (closed-loop / no false-positive): ACK but the game never
    // reaches Running within the verify budget → NOT verified (caller grants no
    // green-light → no billing). This is the anti-stale-verify guarantee (Q5).
    #[tokio::test]
    async fn dispatch_not_verified_when_game_never_reaches_running() {
        let state = build_state().await;
        let agent = mock_agent(&state, "pod_1", false).await; // ACK only, never Running
        let out = dispatch_launch_to_agent(&state, "pod_1", SimType::AssettoCorsa, None, ctx(800))
            .await
            .expect("dispatch ok");
        assert!(!out.verified_running, "ACK without a Running status must NOT verify");
        let _ = agent.await;
    }

    // MAOR (google CRITICAL) regression lock: a transient Loading state must NOT
    // count as verified — only Running grants green-light (anti-stale-verify).
    #[tokio::test]
    async fn dispatch_not_verified_on_loading_alone() {
        let state = build_state().await;
        let (tx, mut rx) = tokio::sync::mpsc::channel::<CoreMessage>(4);
        state.agent_senders.write().await.insert("pod_1".to_string(), tx);
        let st = state.clone();
        let agent = tokio::spawn(async move {
            if let Some(msg) = rx.recv().await {
                let cid = msg.command_id.clone().unwrap_or_default();
                if let Some(ack) = st.pending_command_acks.write().await.remove(&cid) {
                    let _ = ack.send(CommandAckResult { success: true, error: None });
                }
                if let Some(t) = st.game_launcher.active_games.write().await.get_mut("pod_1") {
                    t.game_state = GameState::Loading; // stuck Loading, never Running
                }
            }
        });
        let out = dispatch_launch_to_agent(&state, "pod_1", SimType::AssettoCorsa, None, ctx(800))
            .await
            .expect("dispatch ok");
        assert!(!out.verified_running, "Loading alone must NOT verify — only Running confirms");
        let _ = agent.await;
    }

    // MUST-FIX #4 (concurrency / split-brain): a pod already Running (e.g. a V1
    // kiosk launch) must reject a V2 dispatch — the SINGLE shared active_games
    // tracker is the one launch authority (Q1).
    #[tokio::test]
    async fn dispatch_rejected_when_pod_already_active() {
        let state = build_state().await;
        state.game_launcher.active_games.write().await.insert("pod_1".to_string(), running_tracker("pod_1"));
        let res = dispatch_launch_to_agent(&state, "pod_1", SimType::AssettoCorsa, None, ctx(200)).await;
        assert!(res.is_err(), "a V2 launch on a pod already active (V1) must be rejected");
        assert!(res.unwrap_err().contains("already has a game active"));
    }

    // MUST-FIX #6 (no agent → not confirmed → no green-light): the ORDERING fix.
    // launch_loading grants NO green-light; only a confirmed launch promotes.
    #[test]
    fn launch_loading_withholds_green_light_until_promote() {
        let mut store = HeartStore::new();
        let session = match store.launch_loading(req("pod-1")) {
            LaunchOutcome::Ok { session, .. } => session,
            other => panic!("expected Ok, got {:?}", std::mem::discriminant(&other)),
        };
        assert_eq!(session.state, SessionState::Loading, "reserved session must be Loading, not Running");
        assert!(session.green_light_at.is_none(), "green-light must NOT be granted at reserve time (confirm-before-bill)");
        // confirmed-Running → promote grants green-light.
        let (promoted, _) = store.promote_to_running(&session.id).expect("promote");
        assert_eq!(promoted.state, SessionState::Running);
        assert!(promoted.green_light_at.is_some(), "promote must grant green-light");
    }

    // MUST-FIX #6 (failed launch leaves no money trail): fail_launch ends the
    // session WITHOUT green-light and frees the pod.
    #[test]
    fn fail_launch_frees_pod_and_grants_no_green_light() {
        let mut store = HeartStore::new();
        let session = match store.launch_loading(req("pod-1")) {
            LaunchOutcome::Ok { session, .. } => session,
            _ => panic!("expected Ok"),
        };
        let (failed, snap) = store.fail_launch(&session.id, "agent did not ACK").expect("fail_launch");
        assert_eq!(failed.state, SessionState::Ended);
        assert!(failed.green_light_at.is_none(), "failed launch must never have green-light");
        let pod = snap.expect("pod snapshot");
        assert_eq!(pod.lifecycle, PodLifecycle::Empty, "failed launch must free the pod");
        assert!(pod.current_session.is_none());
    }

    // MUST-FIX #5 (restart/reconciliation): after a restart, if the agent
    // reports a pod Running but the rehydrated session has no green-light (heart
    // crashed post-Running, pre-green-light → free play), reconcile grants it.
    #[test]
    fn reconcile_grants_green_light_when_agent_running_but_session_has_none() {
        let mut store = HeartStore::new();
        // A real-launch reserved a Loading session (no green-light); the heart
        // then "restarted" (state rehydrated as-is); the agent now reports Running.
        let _ = store.launch_loading(req("pod-1"));
        let repaired = store.reconcile_green_light(&["pod-1".to_string()]);
        assert_eq!(repaired.len(), 1, "the session with no green-light must be repaired");
        let (sess, _) = &repaired[0];
        assert!(sess.green_light_at.is_some(), "reconcile must grant green-light");
        assert_eq!(sess.state, SessionState::Running);
        // Idempotent: a second pass repairs nothing.
        assert!(store.reconcile_green_light(&["pod-1".to_string()]).is_empty(), "reconcile must be idempotent once green-light is granted");
    }
}
