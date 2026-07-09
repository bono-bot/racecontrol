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
        // State-allowlist guard (MAOR F1 CRITICAL / F2 IMPORTANT; supplemental RCA
        // heart-loading-complete-route-20260601): `promote_to_running` is now
        // reachable as an EXTERNAL, retryable rc-agent callback
        // (POST /heart/sessions/{sid}/loading-complete), not only the internal
        // launch_real caller. A delayed / duplicate / out-of-order callback MUST
        // NOT resurrect a terminal or paused session — doing so would re-grant
        // green_light_at (= billing RESTART on a session the customer already left)
        // and re-link the pod via sync_pod_running (clobbering a newer session that
        // took the pod). Only Loading (the normal pre-condition) and
        // Running-without-green (post-restart rehydration) may be promoted.
        match session.state {
            // Idempotent re-deliver of a callback that already succeeded — no-op.
            SessionState::Running if session.green_light_at.is_some() => {
                return Some((session.clone(), None));
            }
            // Valid promote pre-conditions.
            SessionState::Loading | SessionState::Running => {}
            // Preflight / Ready / Paused / Ending / Ended / AutoBilled: a callback
            // for a session that is not loading is a no-op — never grant green-light
            // or re-link the pod. 200 (idempotent-safe for a retryable callback).
            other => {
                tracing::warn!(sid, ?other, "loading-complete on non-Loading session — ignored: no green-light, no pod relink (prevents billing resurrection)");
                return Some((session.clone(), None));
            }
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

    /// I3 bridge (DIAGNOSE D3, unanimous): the rc-agent reported `Error` (crash)
    /// for a live session. Billing-NEUTRAL — keep the session + `green_light_at`
    /// intact (the proxy owns billing; a transient crash must NOT stop billing or
    /// free a billed pod), surface the interruption on the pod display so staff
    /// can act. Idempotent: terminal sessions + already-marked displays no-op
    /// (no double-broadcast). NEVER frees the pod, NEVER touches `green_light_at`.
    pub fn mark_crashed(&mut self, sid: &str) -> Option<(PodSession, Option<PodState>)> {
        let session = match self.sessions.get(sid) {
            Some(s) if !matches!(s.state, SessionState::Ended | SessionState::AutoBilled) => s.clone(),
            Some(s) => return Some((s.clone(), None)), // terminal — idempotent no-op
            None => return None,
        };
        let msg = format!("INTERRUPTED · {}", session.game);
        let snap = match self.pods.get_mut(&session.pod_id) {
            Some(pod) if pod.display_message != msg => {
                pod.display_message = msg;
                pod.updated_at = now_iso();
                Some(pod.clone())
            }
            _ => None, // pod gone or already marked — idempotent (no re-broadcast)
        };
        Some((session, snap))
    }

    /// All live (non-terminal) sessions for the reconciler diff: `(normalized_pod,
    /// sid, state, has_green_light)`. The pod id is normalized to canonical `pod_N`
    /// form so the caller can match against the rc-agent's `active_games`, which is
    /// keyed canonically (both `handle_game_state_update` and
    /// `dispatch_launch_to_agent` call `normalize_pod_id`). The heart keys pods as
    /// `pod-N` (hyphen) — WITHOUT this normalization the diff (and the legacy
    /// green-light reconcile) silently never match (hyphen vs underscore).
    pub fn live_sessions_normalized(
        &self,
    ) -> Vec<(String, String, SessionState, bool, String)> {
        self.pods
            .values()
            .filter_map(|p| {
                p.current_session.as_ref().map(|s| {
                    let norm = rc_common::pod_id::normalize_pod_id(&p.pod_id)
                        .unwrap_or_else(|_| p.pod_id.clone());
                    // started_at (RFC3339) added for #11: the reconciler derives a
                    // Loading session's age from it for the stale-Loading reaper.
                    (
                        norm,
                        s.id.clone(),
                        s.state,
                        s.green_light_at.is_some(),
                        s.started_at.clone(),
                    )
                })
            })
            .collect()
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
/// scope: the proxy wallet HOLD+402 is cluster-2. I2 (Gap-2, Captain auth
/// 2026-06-03): `launch_args` now carries real car/track (`build_launch_args`,
/// A-then-B) so the agent boots real content instead of an empty launch — closing
/// the delivery-integrity break (bill moving with no real game). Full
/// preset→car/track resolution (A) remains forward-work (see `build_launch_args`).
async fn launch_real(state: Arc<AppState>, req: LaunchReq) -> Response {
    use crate::game_launcher_ops::{AgentDispatchCtx, default_verify_timeout, dispatch_launch_to_agent};
    let pod_id = req.pod_id.clone();
    let game = req.game.clone();
    // I2: build the real launch content (car/track) + duration BEFORE `req` is
    // moved into launch_loading below. `None` here was the delivery-integrity
    // break — the agent booted with no car/track, so the bill could move with no
    // real game on the rig.
    let launch_args = build_launch_args(&req);
    let duration_minutes = tier_to_duration(&req.tier);
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
        duration_minutes,         // I2: from the V2 tier (None = wallet-bounded; 402-gate + autobill tick bound spend)
        origin: rc_common::protocol::LaunchOrigin::Customer,
        verify_timeout: default_verify_timeout(sim_type),
    };
    let dispatch = dispatch_launch_to_agent(&state, &pod_id, sim_type, launch_args, ctx).await;
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
                // MAOR IMPROVEMENT-2: this is the same free-play gap as the
                // failure arm — the game runs with no green-lit session to bill
                // it. Stop it (best-effort) so the customer doesn't play unbilled
                // until the reconciler's sustained-absence sweep.
                tracing::error!(sid = %session.id, pod = %pod_id, "real-launch: session missing at promote — state corruption");
                stop_game_best_effort(&state, &pod_id, "promote-None state corruption").await;
                err(StatusCode::INTERNAL_SERVER_ERROR, "promote_failed", format!("pod {pod_id} launch verified but session state lost"))
            }
        }
        result => {
            let reason = match result {
                Ok(o) => format!("game not confirmed running (state {:?})", o.final_state),
                Err(e) => e,
            };
            // Cluster-A #1/#2: abandonment is TWO acts that must happen together
            // — StopGame to the agent AND fail_launch in the heart. The rc-agent
            // may have started the game AFTER our verify_timeout (slow Steam
            // cold-start); fail_launch alone frees the pod in the heart but
            // leaves that late game running unbilled (free-play) with an orphaned
            // active_games tracker that bricks the next launch. abandon_launch
            // closes both.
            let failed = abandon_launch(&state, &pod_id, &session.id, &reason).await;
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

/// Best-effort StopGame to a pod's agent (cluster-A #1/#2). The rc-agent may
/// have started the game AFTER our verify_timeout (slow Steam cold-start);
/// without StopGame that late game runs unbilled (free-play, #1) and leaves an
/// orphaned active_games tracker that bricks the next launch (#2). The agent's
/// StopGame handler zeroes FFB, kills the game, and clears the tracker. Safe in
/// EVERY case — StopGame on an idle pod is a no-op (FFB-zero is always safe).
///
/// NON-BLOCKING (`try_send`, MAOR BLOCK-1): this runs on the HTTP launch path,
/// so a wedged/slow agent consumer (full channel) must NEVER hang the request.
/// On a full or closed channel the StopGame is dropped + logged; the
/// reconciler's sustained-absence path (#3/#4) is the backstop.
///
/// `StopGame` is pod-wide + unkeyed (no launch_id). This is safe here because
/// the caller sends it BEFORE `fail_launch` frees the pod: the pod stays
/// `Loading` (so a concurrent `launch_loading` gets `PodNotEmpty`) until then,
/// and the per-pod mpsc is FIFO — so any later session's `LaunchGame` is
/// enqueued AFTER this StopGame and the agent processes it second. The
/// StopGame-before-fail_launch order is therefore load-bearing. (Defense-in-
/// depth follow-on: bind StopGame to a launch_id so the agent only stops the
/// matching game — a protocol change, §S-146-gated, tracked separately.)
async fn stop_game_best_effort(state: &Arc<AppState>, pod_id: &str, why: &str) {
    use rc_common::protocol::{CoreMessage, CoreToAgentMessage};
    use tokio::sync::mpsc::error::TrySendError;
    // Clone the sender out of the read guard (the guard drops at the block end).
    let sender = { state.agent_senders.read().await.get(pod_id).cloned() };
    let Some(tx) = sender else {
        tracing::warn!(pod = %pod_id, "StopGame skipped — agent not connected ({why}); reconciler backstops");
        return;
    };
    match tx.try_send(CoreMessage::wrap(CoreToAgentMessage::StopGame)) {
        Ok(()) => tracing::info!(pod = %pod_id, "StopGame sent ({why}) — closes late-start free-play + orphan tracker"),
        Err(TrySendError::Full(_)) => tracing::warn!(pod = %pod_id, "StopGame dropped — agent channel full ({why}); reconciler backstops"),
        Err(TrySendError::Closed(_)) => tracing::warn!(pod = %pod_id, "StopGame not sent — agent disconnected ({why}); reconciler backstops"),
    }
}

/// Abandon an in-flight launch that never confirmed Running (cluster-A #1/#2,
/// RCA-HEART-V2-LAUNCH-RECONCILER-RESTART-SAFETY §5(ii)). Two parts that MUST
/// happen together, IN THIS ORDER:
///   (a) StopGame to the agent (see `stop_game_best_effort` — the ordering
///       safety argument lives there); THEN
///   (b) fail_launch in the heart — end the session WITHOUT green-light + free
///       the pod (no money harm; the proxy never billed — green-light never
///       granted).
async fn abandon_launch(
    state: &Arc<AppState>,
    pod_id: &str,
    session_id: &str,
    reason: &str,
) -> Option<(PodSession, Option<PodState>)> {
    stop_game_best_effort(state, pod_id, "real-launch abandon").await; // (a) BEFORE (b)
    state.heart.write().await.fail_launch(session_id, reason) // (b)
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
        // NOTE (cluster-D): this alias resolves to CLASSIC AssettoCorsa only and
        // CANNOT distinguish Rally/Evo — a non-canonical Rally/Evo alias that misses
        // the serde-first parse above would mis-route here and (per build_launch_args)
        // wrongly receive AC1 {car,track}. Canonical snake_case strings
        // (`assetto_corsa_rally`/`_evo`) parse correctly above, so first-INR (classic
        // AC) is safe; harden this with a real V2 game catalog before MP/Rally/Evo.
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

/// FIRST-INR default AC single-player content (path B). MUST match a car/track
/// folder actually provisioned on the pods (operator-confirmed) — the rc-agent
/// content check (`rc-agent::steam_checks::check_ac_content`) fails the launch if
/// the dir is missing. These are base-AC free-content ids; override per venue via
/// `RC_FIRST_INR_AC_CAR` / `RC_FIRST_INR_AC_TRACK`.
const DEFAULT_AC_CAR: &str = "abarth500";
const DEFAULT_AC_TRACK: &str = "magione";

/// I2 (Gap-2, Captain auth 2026-06-03): build the rc-agent `launch_args` JSON for
/// a real launch. The agent consumes the `{"car","track"}` string fields
/// (`rc-agent::steam_checks::check_ac_content` + the race-config builder);
/// `AcLauncher::validate_args` only checks valid-JSON, so the agent fields are the
/// binding contract — emit ONLY fields the agent reads (serde drops unknowns).
///
/// A-then-B (Captain 2026-06-03):
///  - (A) preset-resolved car/track — FORWARD-SEAM, not yet wired: the heart has
///    no preset registry and `LaunchReq` carries no car/track, so full A needs the
///    proxy to resolve `preset_id`→car/track into `LaunchReq`, or an agent-side
///    preset lookup. A bare `preset_id` would be silently dropped by the agent.
///  - (B) operator-configured first-INR default (env), falling back to base-AC.
/// MP/lobby launch_args = V2.1-FROZEN → `None`.
///
/// CONTENT CONTRACT (bugs #13/#14, §S-146 cluster-D fix 2026-06-03): emit
/// `{car,track}` ONLY for **classic** AssettoCorsa. The `DEFAULT_AC_CAR/TRACK`
/// (`abarth500`/`magione`) are AC1-classic content ids that the classic agent
/// launcher writes into `race.ini` (`MODEL=`/`TRACK=`). They are WRONG for the
/// other AC engines and must NOT be emitted there:
///  - **AssettoCorsaRally** never applies `{car,track}` (the Rally launch path has
///    no config-write) → the JSON is silently ignored → wrong/blank content (#13).
///  - **AssettoCorsaEvo** lives in an Unreal content namespace where `abarth500`/
///    `magione` do not exist → launch fails / wrong content (#14).
/// Per-sim content maps for Rally/Evo are V2.1-FROZEN. Until then they get `None`
/// → the agent boots its own configured default content (correct, not broken).
fn build_launch_args(req: &LaunchReq) -> Option<String> {
    use rc_common::types::SimType;
    match heart_game_to_sim_type(&req.game) {
        SimType::AssettoCorsa => {
            let car = std::env::var("RC_FIRST_INR_AC_CAR")
                .unwrap_or_else(|_| DEFAULT_AC_CAR.to_string());
            let track = std::env::var("RC_FIRST_INR_AC_TRACK")
                .unwrap_or_else(|_| DEFAULT_AC_TRACK.to_string());
            serde_json::to_string(&serde_json::json!({ "car": car, "track": track })).ok()
        }
        // AC Rally / AC Evo (wrong-namespace for AC1 ids) + non-AC / multiplayer
        // first-INR content = V2.1-frozen → None (agent uses its own default).
        _ => None,
    }
}

/// I2: derive the agent session duration cap from the V2 tier. V2.0 → `None`
/// (no agent-side cap): the wallet 402 launch-gate + the per-minute autobill tick
/// already bound spend; an agent cap shorter than the wallet would end a paid
/// session early. Explicit seam for a future tier-based cap (V2.1).
fn tier_to_duration(_tier: &str) -> Option<u32> {
    None
}

/// Cluster-A (#3/#4, §S-146 reconciler restart-safety) two-timer constants. The
/// heart must NEVER end a billed session from the ABSENCE of an agent signal —
/// only from a positive agent report or a bounded sustained-absence. (MMA Step-1
/// DIAGNOSE: two-timer model; RCA-HEART-V2-LAUNCH-RECONCILER-RESTART-SAFETY.)
const RECONCILE_STARTUP_GRACE: std::time::Duration = std::time::Duration::from_secs(120);
const RECONCILE_SUSTAINED_ABSENCE: std::time::Duration = std::time::Duration::from_secs(900); // 15 min

/// Gap #11: a `Loading` session (pre-green-light, pre-billing) whose launch task
/// died between `launch_loading` and `promote`/`abandon` (a dropped/dead in-request
/// task — NOT the heart-restart case, which rehydrate forces `Loading`→`Running`)
/// strands the pod `Occupied` forever (future `launch_loading` ⇒ `PodNotEmpty`).
/// Reap it after this TTL. MUST exceed the max legitimate `launch_loading`→`promote`
/// window: dispatch verify (`default_verify_timeout` 30s AC / 60s other,
/// `game_launcher_ops.rs`) + semaphore/retry/ACK + AC/Steam cold-start (30-120s).
/// 600s ≈ 5× margin over the realistic worst case, and strictly BELOW
/// `RECONCILE_SUSTAINED_ABSENCE` (900s) so the Loading arm stays the more-specific
/// case. Reaping is BILLING-NEUTRAL (no green-light ⇒ the proxy never billed).
const LOADING_STALE_TTL: std::time::Duration = std::time::Duration::from_secs(600); // 10 min

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReconcileAct {
    Promote,
    Crash,
    Exit,
    Abandon,
    /// #11: reap a stale `Loading` session (age ≥ `LOADING_STALE_TTL`) via the
    /// in-request abandon path (StopGame-before-fail_launch). Billing-neutral.
    AbandonStale,
}

/// Pure per-session reconcile decision — unit-tested exhaustively so the
/// restart-safety invariant is verifiable without a live `AppState` (#3/#4).
/// - `agent_state`: the rc-agent `active_games` view for this pod (`None` = no tracker).
/// - `agent_connected`: a live WS sender exists for this pod on THIS heart now.
/// - `heart_uptime`: this heart instance's uptime (startup grace).
/// - `since_disconnect`: time since this pod's agent last disconnected from THIS
///   heart (`None` = never connected this instance → absent the whole uptime).
#[allow(clippy::too_many_arguments)]
fn reconcile_act(
    agent_state: Option<rc_common::types::GameState>,
    agent_connected: bool,
    heart_uptime: std::time::Duration,
    since_disconnect: Option<std::time::Duration>,
    real_launch: bool,
    session_state: SessionState,
    has_green_light: bool,
    // #11: age of a `Loading` session (since `started_at`). `None` = unknown
    // (parse failure) => never reaped. Only consulted for the stale-Loading arm.
    loading_age: Option<std::time::Duration>,
) -> Option<ReconcileAct> {
    use rc_common::types::GameState;
    match agent_state {
        // Agent confirms Running but the session has no green-light → promote (closes
        // the post-restart free-play window). NOT flag-gated; non-destructive.
        Some(GameState::Running)
            if !has_green_light
                && matches!(session_state, SessionState::Running | SessionState::Loading) =>
        {
            Some(ReconcileAct::Promote)
        }
        // Agent reports a crash (flag-gated) → billing-NEUTRAL mark (safe path).
        Some(GameState::Error) if real_launch && session_state == SessionState::Running => {
            Some(ReconcileAct::Crash)
        }
        // Agent tracker GONE for a Running billed session. #3/#4: an absent agent_view
        // is NOT proof the game exited — apply the two-timer guard before ending.
        None if real_launch && session_state == SessionState::Running => {
            // (1) Startup grace: right after a heart restart the agents have not
            // reconnected yet, so active_games is empty for EVERY pod — never end a
            // billed session in this window (#3 fires on any heart redeploy).
            if heart_uptime < RECONCILE_STARTUP_GRACE {
                return None;
            }
            // A RECENT disconnect stamp means the agent's WS just flapped (or is
            // mid-reconnect). Even if the `agent_senders` snapshot shows it connected,
            // that snapshot is read in a SEPARATE lock from the disconnect stamp and may
            // be stale; and a just-reconnected agent has not re-reported its game yet.
            // Either way, do NOT end a billed session until the connection settles —
            // this closes the TOCTOU between the two snapshots (MAOR ISSUE-1).
            let recently_flapped = since_disconnect.is_some_and(|d| d < RECONCILE_STARTUP_GRACE);
            if agent_connected && !recently_flapped {
                // Agent is connected AND stable, and reports no game → genuine exit.
                Some(ReconcileAct::Exit)
            } else if !agent_connected {
                // Agent not connected. Distinguish a transient WS blip (hold the billed
                // session) from a sustained absence (pod powered off / dead → end as
                // abandoned so the pod isn't stranded; per-tick billing is already
                // wallet-bounded by the proxy 402-gate). `since_disconnect == None`
                // means the agent never connected to THIS heart instance → it has been
                // absent for the whole heart uptime.
                let absent_for = since_disconnect.unwrap_or(heart_uptime);
                if absent_for >= RECONCILE_SUSTAINED_ABSENCE {
                    Some(ReconcileAct::Abandon)
                } else {
                    None // transient / not-yet-sustained absence → hold
                }
            } else {
                // Connected but recently flapped → hold until the connection settles.
                None
            }
        }
        // #11: a Loading session stuck past LOADING_STALE_TTL (dropped/dead launch
        // task — the agent never reported Running, and the in-request abandon never
        // ran). Reap it (billing-neutral: no green-light). Placed AFTER the
        // Some(Running)=>Promote arm, so a slow launch the agent has CONFIRMED Running
        // is promoted, not reaped. real_launch-gated (launch_loading is real-launch
        // only; belt-and-suspenders preserves the flag-OFF ⇒ no-destructive invariant).
        _ if real_launch
            && session_state == SessionState::Loading
            && loading_age.is_some_and(|age| age >= LOADING_STALE_TTL) =>
        {
            Some(ReconcileAct::AbandonStale)
        }
        _ => None, // healthy / Launching / loading-in-flight (within TTL)
    }
}

/// R2 (bridge RCA §7): one reconcile pass — for every pod the rc-agent reports
/// Running, grant green-light to a live heart session that has none (closes the
/// post-restart free-play window; also resolves the L3-1 stuck-Occupied
/// residual once the agent reconnects). #3/#4: NEVER ends a billed session from an
/// absent agent_view during the startup grace or a transient WS blip — only on a
/// connected-agent's no-game report or a bounded sustained absence. Persists +
/// broadcasts each repair. Called at boot (after rehydrate) + periodically.
pub async fn reconcile_heart_green_light_once(state: &Arc<AppState>) {
    use rc_common::pod_id::normalize_pod_id;
    use rc_common::types::GameState;
    use std::collections::HashMap;
    // Snapshot the rc-agent's runtime view ONCE, keyed by CANONICAL pod id
    // (`pod_N`). active_games is already canonical (handle_game_state_update +
    // dispatch_launch_to_agent both normalize); normalize again defensively and, on
    // any duplicate key (pod-id format drift), prefer the most-live state. The
    // active_games lock is taken + dropped here and is NEVER held together with the
    // heart lock, so the lock order (active_games → heart) can never invert
    // (DIAGNOSE D1 #1 CRITICAL: lock-order deadlock).
    let agent_view: HashMap<String, GameState> = {
        let games = state.game_launcher.active_games.read().await;
        let mut m: HashMap<String, GameState> = HashMap::new();
        for (pod, t) in games.iter() {
            let k = normalize_pod_id(pod).unwrap_or_else(|_| pod.clone());
            m.entry(k)
                .and_modify(|g| {
                    if t.game_state == GameState::Running {
                        *g = GameState::Running;
                    }
                })
                .or_insert(t.game_state);
        }
        m
    };

    let real_launch = state
        .feature_flags
        .read()
        .await
        .get("heart_v2_real_launch")
        .map(|f| f.enabled)
        .unwrap_or(false);

    // #3/#4 restart-safety inputs: this heart instance's uptime (startup grace) +
    // per-pod agent presence (a live, non-closed WS sender on THIS heart) + the age
    // of each pod's last disconnect. Snapshot each lock once, keyed CANONICAL.
    let heart_uptime = state.started_at.elapsed();
    let connected: std::collections::HashSet<String> = {
        let senders = state.agent_senders.read().await;
        senders
            .iter()
            .filter(|(_, s)| !s.is_closed())
            .map(|(k, _)| normalize_pod_id(k).unwrap_or_else(|_| k.clone()))
            .collect()
    };
    let disconnects: HashMap<String, std::time::Duration> = {
        let d = state.last_agent_disconnect.read().await;
        d.iter()
            .map(|(k, i)| (normalize_pod_id(k).unwrap_or_else(|_| k.clone()), i.elapsed()))
            .collect()
    };

    // Diff the heart's live sessions (normalized pod) vs the agent view; decide every
    // transition via the pure `reconcile_act` WITHOUT holding the heart lock. Operate
    // by `sid` (a UUID) so pod-id key format is irrelevant past the match (DIAGNOSE D3).
    let acts: Vec<(String, String, ReconcileAct)> = {
        let live = state.heart.read().await.live_sessions_normalized();
        live.into_iter()
            .filter_map(|(norm_pod, sid, st, has_gl, started_at)| {
                // #11: a Loading session's age, for the stale-Loading reaper arm.
                // started_at is RFC3339 (now_iso); parse + diff vs now. A parse
                // failure or a future timestamp (clock skew) ⇒ None ⇒ the arm holds
                // (never reaps on bad data).
                let loading_age = chrono::DateTime::parse_from_rfc3339(&started_at)
                    .ok()
                    .and_then(|t| {
                        (chrono::Utc::now() - t.with_timezone(&chrono::Utc))
                            .to_std()
                            .ok()
                    });
                reconcile_act(
                    agent_view.get(&norm_pod).copied(),
                    connected.contains(&norm_pod),
                    heart_uptime,
                    disconnects.get(&norm_pod).copied(),
                    real_launch,
                    st,
                    has_gl,
                    loading_age,
                )
                .map(|act| (sid, norm_pod, act))
            })
            .collect()
    };
    if acts.is_empty() {
        return;
    }
    // Apply. The synchronous heart mutations (Promote/Crash/Exit/Abandon) run under
    // ONE write lock; mutators are idempotent so a TOCTOU between the read snapshot
    // and this write (e.g. a concurrent proxy /end) is safe. AbandonStale is the ONE
    // act that must run OUTSIDE the lock (abandon_launch is async + needs &state —
    // never hold heart.write() across .await), so it is collected here and applied in
    // a SECOND pass below.
    let mut stale: Vec<(String, String)> = Vec::new(); // (sid, pod_id)
    let results: Vec<(PodSession, Option<PodState>)> = {
        let mut heart = state.heart.write().await;
        acts.into_iter()
            .filter_map(|(sid, pod_id, act)| match act {
                ReconcileAct::Promote => heart.promote_to_running(&sid),
                ReconcileAct::Crash => heart.mark_crashed(&sid),
                ReconcileAct::Exit => heart.end(&sid, "game_exit"),
                ReconcileAct::Abandon => heart.end(&sid, "agent_abandoned"),
                ReconcileAct::AbandonStale => {
                    stale.push((sid, pod_id));
                    None
                }
            })
            .collect()
    };
    for (sess, snap) in results {
        persist_session(&state.v2db, &sess).await;
        if let Some(s) = snap {
            let _ = state.heart_stream_tx.send(s);
        }
    }
    // #11 second pass (out of the heart write lock): reap stale Loading sessions via
    // the in-request abandon path (StopGame-before-fail_launch; billing-neutral — the
    // session never had a green-light). abandon_launch persists + frees the pod.
    for (sid, pod_id) in stale {
        if let Some((sess, snap)) =
            abandon_launch(state, &pod_id, &sid, "loading_stale_reaped").await
        {
            persist_session(&state.v2db, &sess).await;
            if let Some(s) = snap {
                let _ = state.heart_stream_tx.send(s);
            }
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

/// rc-agent loading-complete callback (contract `reportLoadingComplete` / #4b,
/// session.yaml): the agent confirms the game window is up → promote the Loading
/// session to Running + grant `green_light_at`. **Billing starts HERE**, not at
/// launch — confirm-before-bill (Captain decision G-DUP-C4 2026-05-31; bridge RCA
/// §5; supplemental RCA heart-loading-complete-route-20260601). Idempotent: a
/// re-delivered callback on an already-green-lit session no-ops (same 200, via
/// `promote_to_running`'s already-Running guard). Unknown session → 404. The heart
/// route is unauthenticated (LAN-internal); the F6SystemJWT/pod-subject-match auth
/// is enforced at the admin proxy, the auth boundary for all `/heart/*` routes.
async fn loading_complete(State(state): State<Arc<AppState>>, Path(sid): Path<String>) -> Response {
    let result = state.heart.write().await.promote_to_running(&sid);
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
        // confirm-before-bill (G-NEW-9): rc-agent reports the game window is up →
        // promote Loading→Running + grant green_light_at. Contract #4b.
        .route("/heart/sessions/{sid}/loading-complete", post(loading_complete))
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

    /// Cluster-D (#13/#14): launch_args `{car,track}` is emitted ONLY for classic
    /// AssettoCorsa. AC Rally (never applies them) + AC Evo (wrong content
    /// namespace) + non-AC games get `None` so the agent uses its own default
    /// content instead of the wrong AC1 ids.
    #[test]
    fn build_launch_args_only_classic_ac_gets_car_track() {
        let with_game = |g: &str| {
            let mut r = launch_req("pod-1");
            r.game = g.to_string();
            build_launch_args(&r)
        };
        // classic AC (canonical snake_case name + the `ac_sp` alias) → Some {car,track}
        let args = with_game("assetto_corsa").expect("classic AC must emit launch_args");
        assert!(args.contains("\"car\"") && args.contains("\"track\""), "classic AC carries car+track: {args}");
        assert!(with_game("ac_sp").is_some(), "ac_sp alias resolves to classic AC");
        // AC Rally → None (#13: the Rally launch path never applies car/track)
        assert_eq!(with_game("assetto_corsa_rally"), None, "Rally must NOT get AC1 launch_args (#13)");
        // AC Evo → None (#14: abarth500/magione are not in Evo's Unreal namespace)
        assert_eq!(with_game("assetto_corsa_evo"), None, "Evo must NOT get AC1-namespace launch_args (#14)");
        // non-AC → None
        assert_eq!(with_game("f1_25"), None, "non-AC games get no launch_args");
    }

    /// Cluster-A (#3/#4): exhaustive restart-safety matrix for the pure reconcile
    /// decision. The heart must NEVER end a billed (Running) session from an absent
    /// agent_view during the startup grace or a transient WS blip.
    #[test]
    fn reconcile_act_restart_safety_matrix() {
        use rc_common::types::GameState;
        let r = SessionState::Running;
        let s = std::time::Duration::from_secs;
        let grace = RECONCILE_STARTUP_GRACE;
        let absent = RECONCILE_SUSTAINED_ABSENCE;

        // NOTE: every call gains a trailing `loading_age` arg (#11). For the Running
        // cases below it is `None` (inert — the stale-Loading arm only fires on Loading).
        // #3 — heart restart: empty agent_view, agent not yet reconnected, WITHIN the
        // startup grace → MUST hold the billed session (the mass-end-on-redeploy bug).
        assert_eq!(reconcile_act(None, false, s(10), None, true, r, true, None), None,
            "#3: must NOT end a billed session during the startup grace");
        // post-grace, agent connected, reports no game → genuine exit.
        assert_eq!(reconcile_act(None, true, s(200), None, true, r, true, None), Some(ReconcileAct::Exit),
            "connected agent with no game = real exit");
        // #4 — post-grace, agent NOT connected, recently disconnected (transient blip) → hold.
        assert_eq!(reconcile_act(None, false, s(3600), Some(s(10)), true, r, true, None), None,
            "#4: a transient WS blip must NOT end a billed session");
        // post-grace, agent NOT connected, disconnected ≥ sustained-absence → abandon.
        assert_eq!(reconcile_act(None, false, s(3600), Some(absent), true, r, true, None), Some(ReconcileAct::Abandon),
            "sustained absence (dead pod) → abandon");
        // post-grace, never-connected pod, heart up ≥ sustained-absence → abandon.
        assert_eq!(reconcile_act(None, false, absent, None, true, r, true, None), Some(ReconcileAct::Abandon),
            "never-seen pod after long uptime → abandon");
        // FLAG OFF: an absent agent_view is ALWAYS a no-op (no destructive action).
        assert_eq!(reconcile_act(None, false, s(3600), Some(absent), false, r, true, None), None,
            "flag OFF: reconciler never ends a session");
        // grace boundary is exclusive-below: exactly at grace + connected + settled → exit.
        assert_eq!(reconcile_act(None, true, grace, None, true, r, true, None), Some(ReconcileAct::Exit));
        // MAOR ISSUE-1: connected snapshot but a RECENT disconnect stamp (WS flap /
        // stale snapshot) → HOLD, never end a billed session on a flapped agent.
        assert_eq!(reconcile_act(None, true, s(3600), Some(s(10)), true, r, true, None), None,
            "ISSUE-1: connected-but-recently-flapped must hold, not Exit");
        // connected + the flap is old (reconnected & stable past the settle window) → exit.
        assert_eq!(reconcile_act(None, true, s(3600), Some(grace), true, r, true, None), Some(ReconcileAct::Exit),
            "a settled reconnected agent reporting no game = exit");

        // Non-destructive arms unchanged:
        // agent reports Running without green-light → promote (NOT flag-gated).
        assert_eq!(reconcile_act(Some(GameState::Running), true, s(3600), None, false, r, false, None), Some(ReconcileAct::Promote));
        // agent reports Error (flag ON) → crash (billing-neutral mark).
        assert_eq!(reconcile_act(Some(GameState::Error), true, s(3600), None, true, r, true, None), Some(ReconcileAct::Crash));
        // agent reports Running + already green-lit → no-op.
        assert_eq!(reconcile_act(Some(GameState::Running), true, s(3600), None, true, r, true, None), None);
        // a Loading session with NO age info (parse failure) → no-op (hold; never reap on bad data).
        assert_eq!(reconcile_act(None, false, s(3600), Some(absent), true, SessionState::Loading, false, None), None,
            "Loading with unknown age must NOT be reaped");

        // #11 — stale-Loading reaper (the dropped/dead launch-task case). Loading is
        // pre-green-light/pre-billing, so AbandonStale is billing-neutral.
        let stale = LOADING_STALE_TTL;
        // Loading aged ≥ TTL, flag ON → reap.
        assert_eq!(reconcile_act(None, false, s(3600), Some(absent), true, SessionState::Loading, false, Some(stale)),
            Some(ReconcileAct::AbandonStale),
            "#11: a Loading session past LOADING_STALE_TTL with no promote must be reaped (billing-neutral)");
        // Flag OFF → never reap (preserves flag-OFF ⇒ no-destructive invariant).
        assert_eq!(reconcile_act(None, false, s(3600), Some(absent), false, SessionState::Loading, false, Some(stale)), None,
            "#11: flag OFF — reconciler never reaps a Loading session");
        // Slow-but-VALID launch UNDER the TTL → must NOT be reaped (no false reap of cold-start).
        assert_eq!(reconcile_act(None, false, s(3600), None, true, SessionState::Loading, false, Some(stale - s(1))), None,
            "#11: a slow launch under the TTL must NOT be reaped");
        // TTL boundary is inclusive (≥).
        assert_eq!(reconcile_act(None, false, s(3600), None, true, SessionState::Loading, false, Some(stale)),
            Some(ReconcileAct::AbandonStale), "#11: TTL boundary is inclusive (≥)");
        // A slow Loading launch the agent has CONFIRMED Running → PROMOTE, not reaped
        // (the Promote arm wins; this is the load-bearing safety check).
        assert_eq!(reconcile_act(Some(GameState::Running), true, s(3600), None, true, SessionState::Loading, false, Some(stale)),
            Some(ReconcileAct::Promote),
            "#11: a slow launch the agent confirmed Running is PROMOTED, never reaped");
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

    /// G-NEW-9 confirm-before-bill: the rc-agent loading-complete callback flips a
    /// Loading session (no green-light) → Running + grants `green_light_at`.
    /// Billing starts HERE, not at launch. Idempotent; unknown session → 404.
    #[tokio::test]
    async fn http_loading_complete_promotes_loading_and_grants_green_light() {
        let (app, state) = test_app_with_migrated_v2db().await;
        // Seed a Loading session directly: the flag-OFF HTTP launch path goes
        // straight to Running; confirm-before-bill reserves Loading first (the
        // real-launch path the rc-agent drives), so the callback has work to do.
        let sid = {
            let mut store = state.heart.write().await;
            match store.launch_loading(LaunchReq {
                pod_id: "pod-4".to_string(),
                household_id: "hh".to_string(),
                profile_id: "pf".to_string(),
                tier: "tier_1_full_skeleton".to_string(),
                game: "ac_sp".to_string(),
                lobby_id: None,
                preset_id: None,
            }) {
                LaunchOutcome::Ok { session, .. } => {
                    assert_eq!(session.state, SessionState::Loading);
                    assert!(session.green_light_at.is_none(), "no green-light before loading-complete");
                    session.id
                }
                _ => panic!("launch_loading must succeed on an empty pod"),
            }
        };
        // rc-agent callback: game window up → grant green-light (billing starts).
        let (status, json) =
            call(&app, "POST", &format!("/heart/sessions/{sid}/loading-complete"), Some(serde_json::json!({}))).await;
        assert_eq!(status, 200, "loading-complete must NOT 404 — contract #4b route");
        assert_eq!(json["state"], "running");
        assert!(json["green_light_at"].as_str().is_some(), "loading-complete grants green_light_at = billing start");
        // Idempotent: a re-delivered callback no-ops (still 200, still green-lit).
        let (status2, json2) =
            call(&app, "POST", &format!("/heart/sessions/{sid}/loading-complete"), Some(serde_json::json!({}))).await;
        assert_eq!(status2, 200, "re-delivered loading-complete is idempotent");
        assert!(json2["green_light_at"].as_str().is_some());
        // Unknown session → 404 (not a silent 200).
        let (s404, _) =
            call(&app, "POST", "/heart/sessions/no-such/loading-complete", Some(serde_json::json!({}))).await;
        assert_eq!(s404, 404, "unknown session must 404, not silently succeed");
    }

    /// MAOR F1 (CRITICAL) regression guard: a late/duplicate loading-complete that
    /// arrives AFTER the session ended must NOT resurrect it — no Running, no fresh
    /// green_light_at, pod stays free. Prevents a billing restart on a session the
    /// customer already left + pod-link clobber of a newer session.
    #[tokio::test]
    async fn http_loading_complete_does_not_resurrect_ended_session() {
        let (app, state) = test_app_with_migrated_v2db().await;
        let sid = {
            let mut store = state.heart.write().await;
            match store.launch_loading(LaunchReq {
                pod_id: "pod-5".to_string(),
                household_id: "hh".to_string(),
                profile_id: "pf".to_string(),
                tier: "tier_1_full_skeleton".to_string(),
                game: "ac_sp".to_string(),
                lobby_id: None,
                preset_id: None,
            }) {
                LaunchOutcome::Ok { session, .. } => session.id,
                _ => panic!("launch_loading must succeed on an empty pod"),
            }
        };
        // Normal path: callback → Running + green-light.
        let (s1, _) = call(&app, "POST", &format!("/heart/sessions/{sid}/loading-complete"), Some(serde_json::json!({}))).await;
        assert_eq!(s1, 200);
        // Customer's session ends.
        let (s2, ended) = call(&app, "POST", &format!("/heart/sessions/{sid}/end"), Some(serde_json::json!({"end_reason": "customer_stop"}))).await;
        assert_eq!(s2, 200);
        assert_eq!(ended["state"], "ended");
        // Late/duplicate callback AFTER end — must be a no-op, NOT a resurrection.
        let (s3, json) = call(&app, "POST", &format!("/heart/sessions/{sid}/loading-complete"), Some(serde_json::json!({}))).await;
        assert_eq!(s3, 200, "late callback is idempotent-safe (200), not an error the agent retry-storms on");
        assert_eq!(json["state"], "ended", "ENDED session must NOT be resurrected to running");
        // The pod must stay free — not re-linked to the dead session.
        let (_, pod) = call(&app, "GET", "/heart/pods/pod-5", None).await;
        assert_eq!(pod["lifecycle"], "empty", "ended session's pod must NOT be re-linked by a late callback");
    }

    /// MAOR F2 (IMPORTANT) regression guard: loading-complete on a PAUSED session
    /// must NOT silently resume it — resume is an explicit action; a stray callback
    /// must not restart the billing clock.
    #[tokio::test]
    async fn http_loading_complete_does_not_resume_paused_session() {
        let (app, state) = test_app_with_migrated_v2db().await;
        let sid = {
            let mut store = state.heart.write().await;
            match store.launch_loading(LaunchReq {
                pod_id: "pod-6".to_string(),
                household_id: "hh".to_string(),
                profile_id: "pf".to_string(),
                tier: "tier_1_full_skeleton".to_string(),
                game: "ac_sp".to_string(),
                lobby_id: None,
                preset_id: None,
            }) {
                LaunchOutcome::Ok { session, .. } => session.id,
                _ => panic!("launch_loading must succeed on an empty pod"),
            }
        };
        let (s1, _) = call(&app, "POST", &format!("/heart/sessions/{sid}/loading-complete"), Some(serde_json::json!({}))).await;
        assert_eq!(s1, 200);
        let (sp, _) = call(&app, "POST", &format!("/heart/sessions/{sid}/pause"), Some(serde_json::json!({}))).await;
        assert_eq!(sp, 200);
        // Stray loading-complete on a paused session — no-op, stays paused.
        let (s3, json) = call(&app, "POST", &format!("/heart/sessions/{sid}/loading-complete"), Some(serde_json::json!({}))).await;
        assert_eq!(s3, 200);
        assert_eq!(json["state"], "paused", "paused session must NOT be flipped to running by loading-complete");
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
        build_state_aged(0).await
    }
    /// Build a test heart with `secs_ago` of simulated uptime so the reconciler's
    /// startup-grace timer (cluster-A #3/#4) can be exercised deterministically.
    async fn build_state_aged(secs_ago: u64) -> Arc<AppState> {
        let db = sqlx::SqlitePool::connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite");
        let config = crate::config::Config::default_test();
        let field_cipher = crate::crypto::encryption::test_field_cipher();
        let mut st = AppState::new_with_test_v2db(config, db, field_cipher);
        if secs_ago > 0 {
            st.started_at = std::time::Instant::now()
                .checked_sub(std::time::Duration::from_secs(secs_ago))
                .expect("monotonic clock is past the requested age");
        }
        Arc::new(st)
    }
    /// Mark a pod's agent as connected (a live, non-closed WS sender). The returned
    /// receiver MUST be held by the caller — dropping it closes the sender and the
    /// reconciler would see the agent as disconnected.
    async fn connect_agent(state: &Arc<AppState>, pod: &str) -> tokio::sync::mpsc::Receiver<CoreMessage> {
        let (tx, rx) = tokio::sync::mpsc::channel::<CoreMessage>(4);
        state.agent_senders.write().await.insert(pod.to_string(), tx);
        rx
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
    // I2 (Gap-2): build_launch_args carries real AC car/track so the agent boots
    // real content (delivery integrity) — and the JSON satisfies the agent's
    // binding contract (AcLauncher::validate_args + the {"car","track"} fields).
    #[test]
    fn build_launch_args_ac_emits_valid_car_track_json() {
        use crate::game_launcher::{AcLauncher, GameLauncherImpl};
        let r = LaunchReq {
            pod_id: "pod-1".into(), household_id: "h".into(), profile_id: "p".into(),
            tier: "tier_1_full_skeleton".into(), game: "ac_sp".into(),
            lobby_id: None, preset_id: None,
        };
        let args = build_launch_args(&r).expect("AC launch must carry launch_args, never None");
        AcLauncher.validate_args(Some(&args)).expect("agent validate_args must accept built launch_args");
        let v: serde_json::Value = serde_json::from_str(&args).expect("valid JSON");
        assert!(v.get("car").and_then(|c| c.as_str()).map(|s| !s.is_empty()).unwrap_or(false), "car field required by check_ac_content");
        assert!(v.get("track").and_then(|t| t.as_str()).map(|s| !s.is_empty()).unwrap_or(false), "track field required by check_ac_content");
    }

    // Non-AC / multiplayer launch content is V2.1-frozen → None (no invented fields).
    #[test]
    fn build_launch_args_non_ac_is_frozen_none() {
        let r = LaunchReq {
            pod_id: "pod-1".into(), household_id: "h".into(), profile_id: "p".into(),
            tier: "tier_1_full_skeleton".into(), game: "f1_25".into(),
            lobby_id: None, preset_id: None,
        };
        assert!(build_launch_args(&r).is_none(), "non-AC first-INR content is V2.1-frozen");
    }

    // V2.0 duration is wallet-bounded (None): the 402-gate + autobill tick bound spend.
    #[test]
    fn tier_to_duration_v2_0_is_wallet_bounded_none() {
        assert_eq!(tier_to_duration("tier_1_full_skeleton"), None);
        assert_eq!(tier_to_duration("tier_2_desktop_workaround"), None);
    }

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

    // Cluster-A #1/#2 (slow-launch StopGame contract): abandoning an in-flight
    // launch must (a) tell the agent to StopGame — so a game the rc-agent
    // started AFTER our verify_timeout is killed (no free-play, #1) and its
    // active_games tracker cleared (no pod-brick, #2) — AND (b) free the pod in
    // the heart. The pre-fix failure arm did only (b), leaving a late game
    // running unbilled. This must FAIL pre-fix (no StopGame was ever sent).
    #[tokio::test]
    async fn abandon_launch_sends_stopgame_and_frees_pod() {
        let state = build_state().await;
        let session = match state.heart.write().await.launch_loading(req("pod-1")) {
            LaunchOutcome::Ok { session, .. } => session,
            other => panic!("expected Ok, got {:?}", std::mem::discriminant(&other)),
        };
        // The agent is connected and may have a late game running on it.
        let mut rx = connect_agent(&state, "pod-1").await;

        let failed =
            abandon_launch(&state, "pod-1", &session.id, "game not confirmed running").await;

        // (a) the agent was told to stop — closes free-play + orphan tracker.
        let msg = rx.try_recv().expect("abandon must send a command to the agent");
        assert!(
            matches!(msg.inner, rc_common::protocol::CoreToAgentMessage::StopGame),
            "abandon must send StopGame, got {:?}",
            msg.inner
        );
        // (b) the pod is freed in the heart, no green-light.
        let (sess, snap) = failed.expect("fail_launch result");
        assert_eq!(sess.state, SessionState::Ended);
        assert!(sess.green_light_at.is_none(), "abandoned launch must never have green-light");
        let pod = snap.expect("pod snapshot");
        assert_eq!(pod.lifecycle, PodLifecycle::Empty, "abandon must free the pod");
        assert!(pod.current_session.is_none());
    }

    // Backstop: when the agent is NOT connected, abandon must still free the pod
    // (StopGame is best-effort; the reconciler's sustained-absence path is the
    // safety net). Must not panic or hang.
    #[tokio::test]
    async fn abandon_launch_frees_pod_even_when_agent_disconnected() {
        let state = build_state().await;
        let session = match state.heart.write().await.launch_loading(req("pod-2")) {
            LaunchOutcome::Ok { session, .. } => session,
            other => panic!("expected Ok, got {:?}", std::mem::discriminant(&other)),
        };
        // No connect_agent → agent_senders has no entry for pod-2.
        let failed = abandon_launch(&state, "pod-2", &session.id, "agent did not ACK").await;
        let (sess, snap) = failed.expect("fail_launch result");
        assert_eq!(sess.state, SessionState::Ended);
        assert_eq!(snap.expect("pod snapshot").lifecycle, PodLifecycle::Empty);
    }

    // MAOR BLOCK-1 regression: abandon_launch runs on the HTTP launch path, so a
    // FULL agent channel (slow/wedged consumer) must NOT hang it. try_send drops
    // the StopGame and the pod is still freed. With the old blocking
    // `send().await` this test would hang and time out.
    #[tokio::test]
    async fn abandon_launch_does_not_hang_on_full_agent_channel() {
        use rc_common::protocol::{CoreMessage, CoreToAgentMessage};
        let state = build_state().await;
        let session = match state.heart.write().await.launch_loading(req("pod-1")) {
            LaunchOutcome::Ok { session, .. } => session,
            other => panic!("expected Ok, got {:?}", std::mem::discriminant(&other)),
        };
        // connect_agent uses a capacity-4 channel; hold the rx (do NOT drain) and
        // fill it so a blocking send would wait forever.
        let _rx = connect_agent(&state, "pod-1").await;
        let tx = state.agent_senders.read().await.get("pod-1").cloned().unwrap();
        for _ in 0..4 {
            tx.try_send(CoreMessage::wrap(CoreToAgentMessage::StopGame)).expect("prefill the channel");
        }
        // Must return promptly despite the full channel.
        let failed = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            abandon_launch(&state, "pod-1", &session.id, "verify timeout"),
        )
        .await
        .expect("abandon_launch must NOT hang on a full agent channel (MAOR BLOCK-1)");
        // The pod is still freed even though the StopGame was dropped.
        let (sess, snap) = failed.expect("fail_launch result");
        assert_eq!(sess.state, SessionState::Ended);
        assert_eq!(snap.expect("pod snapshot").lifecycle, PodLifecycle::Empty);
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

    // ───────────────────────── I3 bridge: reconciler crash/exit ──────────────
    // Seam B (MMA DIAGNOSE 2026-06-02): crash/exit detection lives in the
    // reconciler, matched on the NORMALIZED pod key (heart "pod-1" ↔ agent "pod_1").

    async fn set_real_launch(state: &Arc<AppState>, on: bool) {
        state.feature_flags.write().await.insert(
            "heart_v2_real_launch".to_string(),
            crate::flags::FeatureFlagRow {
                name: "heart_v2_real_launch".to_string(),
                enabled: on,
                default_value: false,
                overrides: "{}".to_string(),
                version: 1,
                updated_at: None,
            },
        );
    }

    /// Seed a Running, green-lit heart session on `pod-1`; return its sid.
    async fn seed_running(state: &Arc<AppState>) -> String {
        let mut h = state.heart.write().await;
        let s = match h.launch_loading(req("pod-1")) {
            LaunchOutcome::Ok { session, .. } => session,
            _ => panic!("launch_loading failed"),
        };
        h.promote_to_running(&s.id).expect("promote");
        s.id
    }

    // DIAGNOSE D3/D4: a real crash (agent Error) marks the pod INTERRUPTED but is
    // BILLING-NEUTRAL — session stays Running, green-light preserved, pod NOT freed.
    #[tokio::test]
    async fn reconcile_crash_marks_interrupted_billing_neutral() {
        let state = build_state().await;
        set_real_launch(&state, true).await;
        let sid = seed_running(&state).await;
        let green_before = state
            .heart
            .read()
            .await
            .get_pod("pod-1")
            .unwrap()
            .current_session
            .unwrap()
            .green_light_at
            .clone();
        let mut t = running_tracker("pod_1");
        t.game_state = GameState::Error;
        state.game_launcher.active_games.write().await.insert("pod_1".to_string(), t);

        reconcile_heart_green_light_once(&state).await;

        let pod = state.heart.read().await.get_pod("pod-1").unwrap();
        assert!(pod.display_message.starts_with("INTERRUPTED"), "crash must surface INTERRUPTED, got {}", pod.display_message);
        let sess = pod.current_session.as_ref().expect("crash must NOT free a billed pod");
        assert_eq!(sess.id, sid);
        assert_eq!(sess.state, SessionState::Running, "crash is billing-neutral — session stays Running");
        assert_eq!(sess.green_light_at, green_before, "crash must NEVER revoke green-light");
        assert_eq!(pod.lifecycle, PodLifecycle::Occupied);
    }

    // DIAGNOSE D4: a clean exit (agent Idle removed the tracker → absent) frees the
    // pod so staff can re-launch.
    #[tokio::test]
    async fn reconcile_exit_frees_pod() {
        // Post-grace + the pod's agent connected-and-reporting-no-game = a GENUINE
        // exit → free the pod. (The within-grace / transient-absence cases are held;
        // see reconcile_within_grace_holds_billed_pod + the pure decision matrix.)
        let state = build_state_aged(RECONCILE_STARTUP_GRACE.as_secs() + 60).await;
        set_real_launch(&state, true).await;
        let _sid = seed_running(&state).await;
        let _rx = connect_agent(&state, "pod_1").await; // agent present, active_games empty
        reconcile_heart_green_light_once(&state).await;

        let pod = state.heart.read().await.get_pod("pod-1").unwrap();
        assert!(pod.current_session.is_none(), "a connected agent reporting no game = clean exit must free the pod");
        assert_eq!(pod.lifecycle, PodLifecycle::Empty);
    }

    /// Cluster-A #3 (integration): a heart restart (uptime within the startup grace)
    /// with an empty agent_view must NOT end/free a Running billed session — the
    /// reconciler holds it until agents reconnect. This is the mass-end-on-redeploy fix.
    #[tokio::test]
    async fn reconcile_within_grace_holds_billed_pod() {
        let state = build_state().await; // uptime ~0 < startup grace
        set_real_launch(&state, true).await;
        let _sid = seed_running(&state).await;
        // active_games empty + no agent connected = post-restart, agents not back yet.
        reconcile_heart_green_light_once(&state).await;

        let pod = state.heart.read().await.get_pod("pod-1").unwrap();
        assert!(pod.current_session.is_some(), "#3: must NOT free a billed pod during the startup grace");
        assert_eq!(pod.lifecycle, PodLifecycle::Occupied);
        assert_eq!(pod.current_session.unwrap().state, SessionState::Running,
            "billed session preserved across the restart window");
    }

    /// Cluster-A (integration): a billed pod whose agent has been disconnected longer
    /// than the sustained-absence bound is ended as abandoned, so the pod isn't
    /// stranded (per-tick billing is already wallet-bounded by the proxy 402-gate).
    #[tokio::test]
    async fn reconcile_sustained_absence_abandons_pod() {
        let state = build_state_aged(RECONCILE_STARTUP_GRACE.as_secs() + 60).await;
        set_real_launch(&state, true).await;
        let _sid = seed_running(&state).await;
        // agent NOT connected; stamp a disconnect older than the sustained-absence bound.
        let old = std::time::Instant::now()
            .checked_sub(RECONCILE_SUSTAINED_ABSENCE + std::time::Duration::from_secs(60))
            .expect("monotonic past");
        state.last_agent_disconnect.write().await.insert("pod_1".to_string(), old);
        reconcile_heart_green_light_once(&state).await;

        let pod = state.heart.read().await.get_pod("pod-1").unwrap();
        assert!(pod.current_session.is_none(), "sustained absence (dead pod) → abandoned, pod freed");
    }

    // The pod-id normalization fix: heart keys "pod-1", active_games keys "pod_1".
    // A Loading heart session + agent Running on "pod_1" must promote ACROSS the
    // hyphen/underscore boundary (otherwise the reconcile silently never fires).
    #[tokio::test]
    async fn reconcile_promote_matches_across_hyphen_underscore_keys() {
        let state = build_state().await;
        set_real_launch(&state, true).await;
        let sid = {
            let mut h = state.heart.write().await;
            match h.launch_loading(req("pod-1")) {
                LaunchOutcome::Ok { session, .. } => session.id,
                _ => panic!(),
            }
        };
        state.game_launcher.active_games.write().await.insert("pod_1".to_string(), running_tracker("pod_1"));

        reconcile_heart_green_light_once(&state).await;

        let sess = state.heart.read().await.get_pod("pod-1").unwrap().current_session.unwrap();
        assert_eq!(sess.id, sid);
        assert_eq!(sess.state, SessionState::Running, "agent-Running across the key boundary must promote");
        assert!(sess.green_light_at.is_some(), "promote must grant green-light (closes the free-play window)");
    }

    // Flag gating: with heart_v2_real_launch OFF, the reconciler must NOT apply
    // crash/exit (the proxy /end owns session lifecycle in the mock path).
    #[tokio::test]
    async fn reconcile_flag_off_ignores_crash_exit() {
        let state = build_state().await;
        set_real_launch(&state, false).await;
        let sid = seed_running(&state).await;
        let mut t = running_tracker("pod_1");
        t.game_state = GameState::Error;
        state.game_launcher.active_games.write().await.insert("pod_1".to_string(), t);

        reconcile_heart_green_light_once(&state).await;

        let pod = state.heart.read().await.get_pod("pod-1").unwrap();
        let sess = pod.current_session.as_ref().expect("flag OFF: pod must be untouched");
        assert_eq!(sess.id, sid);
        assert_eq!(sess.state, SessionState::Running);
        assert!(!pod.display_message.starts_with("INTERRUPTED"), "flag OFF must not mark crashed");
    }
}
