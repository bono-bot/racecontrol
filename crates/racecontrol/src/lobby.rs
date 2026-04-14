//! Synchronized multiplayer lobby manager (VMS-inspired Feature 2).
//!
//! Orchestrates group game launches: all pods load the game, hold in lobby
//! until all report ready, then start simultaneously.

use std::collections::{HashMap, HashSet};
use tokio::sync::RwLock;

use rc_common::types::{LobbyPhase, LobbyStatus};

const LOG_TARGET: &str = "lobby";

/// Timeout after which the lobby proceeds even if some pods aren't ready.
const LOBBY_TIMEOUT_SECS: u64 = 120;

/// State for a single multiplayer lobby instance.
struct LobbyInstance {
    group_session_id: String,
    assigned_pods: Vec<String>,
    ready_pods: HashSet<String>,
    phase: LobbyPhase,
    created_at: chrono::DateTime<chrono::Utc>,
}

impl LobbyInstance {
    fn to_status(&self) -> LobbyStatus {
        LobbyStatus {
            group_session_id: self.group_session_id.clone(),
            phase: self.phase.clone(),
            total_pods: self.assigned_pods.len() as u32,
            ready_pods: self.ready_pods.iter().cloned().collect(),
            created_at: self.created_at.to_rfc3339(),
        }
    }

    fn all_ready(&self) -> bool {
        self.assigned_pods.iter().all(|p| self.ready_pods.contains(p))
    }
}

/// Manages all active lobbies across multiplayer sessions.
pub struct LobbyManager {
    lobbies: RwLock<HashMap<String, LobbyInstance>>,
}

impl LobbyManager {
    pub fn new() -> Self {
        Self {
            lobbies: RwLock::new(HashMap::new()),
        }
    }

    /// Create a new lobby for a group session.
    /// Returns the LobbyStatus for initial broadcast.
    pub async fn create_lobby(
        &self,
        group_session_id: String,
        assigned_pods: Vec<String>,
    ) -> LobbyStatus {
        let instance = LobbyInstance {
            group_session_id: group_session_id.clone(),
            assigned_pods,
            ready_pods: HashSet::new(),
            phase: LobbyPhase::Forming,
            created_at: chrono::Utc::now(),
        };
        let status = instance.to_status();
        self.lobbies.write().await.insert(group_session_id, instance);
        status
    }

    /// Record a pod as ready. Returns updated status and whether all pods are now ready.
    pub async fn mark_pod_ready(
        &self,
        group_session_id: &str,
        pod_id: &str,
    ) -> Option<(LobbyStatus, bool)> {
        let mut lobbies = self.lobbies.write().await;
        let instance = lobbies.get_mut(group_session_id)?;

        if instance.phase != LobbyPhase::Forming {
            tracing::warn!(
                target: LOG_TARGET,
                "Pod {} reported ready for lobby {} but phase is {:?} — ignoring",
                pod_id, group_session_id, instance.phase
            );
            return None;
        }

        instance.ready_pods.insert(pod_id.to_string());
        tracing::info!(
            target: LOG_TARGET,
            "Lobby {}: pod {} ready ({}/{})",
            group_session_id, pod_id,
            instance.ready_pods.len(), instance.assigned_pods.len()
        );

        let all_ready = instance.all_ready();
        if all_ready {
            instance.phase = LobbyPhase::AllReady;
        }

        Some((instance.to_status(), all_ready))
    }

    /// Transition lobby to Starting phase (called when LobbyGo is about to be sent).
    pub async fn mark_starting(&self, group_session_id: &str) {
        let mut lobbies = self.lobbies.write().await;
        if let Some(instance) = lobbies.get_mut(group_session_id) {
            instance.phase = LobbyPhase::Starting;
        }
    }

    /// Remove a completed or timed-out lobby.
    pub async fn remove_lobby(&self, group_session_id: &str) {
        self.lobbies.write().await.remove(group_session_id);
    }

    /// Get lobbies that have exceeded the timeout and should proceed anyway.
    pub async fn get_timed_out_lobbies(&self) -> Vec<String> {
        let lobbies = self.lobbies.read().await;
        let now = chrono::Utc::now();
        lobbies
            .iter()
            .filter(|(_, inst)| {
                inst.phase == LobbyPhase::Forming
                    && (now - inst.created_at).num_seconds() > LOBBY_TIMEOUT_SECS as i64
            })
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Get current status for a lobby.
    pub async fn get_status(&self, group_session_id: &str) -> Option<LobbyStatus> {
        let lobbies = self.lobbies.read().await;
        lobbies.get(group_session_id).map(|i| i.to_status())
    }
}
