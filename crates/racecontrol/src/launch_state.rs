// Phase 368 Plan 01 — In-memory LaunchStateMachine
//
// Tracks the lifecycle of every game launch attempt from /games/launch entry
// to "playable" (GameState::Running + tracker.playable_at set). State is
// in-memory only per D-03; durable history is in launch_timeline_spans.events_json.
//
// CLAUDE.md invariants enforced:
//   - No unwrap() in production code — all Option handling via ? or match
//   - NO lock held across .await — every guard is declared in its own { } block
//     that drops BEFORE any async send or DB call.
//
// Default impl has explicit comment per CLAUDE.md standing rule:
//   "Every ::default() must be reviewed — is this the real state?"
// Here Default is intentional: an empty initial map is the correct starting state.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::Utc;
use rc_common::protocol::{LaunchStatusCard, LaunchState, LaunchOrigin};

const MAX_ACTIVE_LAUNCHES: usize = 100;
/// Cards older than this (in seconds) in a terminal state are pruned.
const STALE_AFTER_SECS: i64 = 600; // 10 minutes

/// In-memory store for active launch status cards.
///
/// Thread-safety: all access is via tokio::sync::RwLock. Guards are always dropped
/// inside their own `{ }` block before any .await call to comply with the
/// CLAUDE.md "never hold a lock across .await" standing rule.
pub struct LaunchStateMachine {
    /// Key = launch_id (server-minted UUID v4 per D-01)
    states: RwLock<HashMap<String, LaunchStatusCard>>,
}

impl LaunchStateMachine {
    /// Intentional default: empty initial state — correct starting point for a new server.
    pub fn new() -> Self {
        Self {
            states: RwLock::new(HashMap::new()),
        }
    }

    /// Create a new launch card in LaunchStarted state and store it.
    /// Returns the created card (caller broadcasts it as DashboardEvent::LaunchStatusChanged).
    ///
    /// Lock-and-drop: guard released before returning to caller — no .await inside.
    pub async fn start_launch(
        &self,
        launch_id: String,
        pod_id: String,
        sim_type: String,
        origin: LaunchOrigin,
    ) -> LaunchStatusCard {
        let card = LaunchStatusCard {
            launch_id: launch_id.clone(),
            pod_id,
            sim_type,
            state: LaunchState::LaunchStarted,
            detail: None,
            timestamp: Utc::now(),
            origin,
            ai_tier: None,
            fix_action: None,
        };
        // CLAUDE.md rule: lock acquired and dropped in tight block — no .await inside.
        {
            let mut guard = self.states.write().await;
            guard.insert(launch_id, card.clone());
        }
        card
    }

    /// Attempt a state transition for the given launch_id.
    ///
    /// Returns `Some(updated_card)` if the transition was valid and applied.
    /// Returns `None` if the launch_id is unknown OR the transition is invalid
    /// (monotonic order violation or terminal-state rejection).
    ///
    /// Lock-and-drop: guard released before returning — safe to .await after this call.
    pub async fn transition(
        &self,
        launch_id: &str,
        new_state: LaunchState,
        detail: Option<String>,
        ai_tier: Option<u8>,
        fix_action: Option<String>,
    ) -> Option<LaunchStatusCard> {
        // Acquire write lock in tight block — drop before any downstream .await.
        let updated = {
            let mut guard = self.states.write().await;
            let existing = guard.get_mut(launch_id)?;
            if !Self::is_valid_transition(existing.state, new_state) {
                return None;
            }
            existing.state = new_state;
            if detail.is_some() {
                existing.detail = detail;
            }
            if ai_tier.is_some() {
                existing.ai_tier = ai_tier;
            }
            if fix_action.is_some() {
                existing.fix_action = fix_action;
            }
            existing.timestamp = Utc::now();
            existing.clone()
        }; // guard dropped here — safe to .await after this point
        Some(updated)
    }

    /// Validate whether a state transition is allowed.
    ///
    /// Rules:
    ///   1. Terminal states (IssueFixed, NeedsManualIntervention) block ALL further transitions.
    ///   2. Only forward transitions in the defined order are allowed.
    ///   3. LaunchStarted → NeedsManualIntervention is allowed (billing-reject path — see Plan 01 Task 3 Step C).
    ///   4. LaunchStarted → IssueFixed is allowed (happy-path skip: no retry needed).
    fn is_valid_transition(from: LaunchState, to: LaunchState) -> bool {
        use LaunchState::*;
        // Terminal states: no further transitions allowed (idempotency guard — P2-05)
        if matches!(from, IssueFixed | NeedsManualIntervention) {
            return false;
        }
        match (from, to) {
            // Normal forward progression
            (LaunchStarted, AiAnalysisRequested) => true,
            // Happy-path skip: game launched and reached playable without any retry
            (LaunchStarted, IssueFixed) => true,
            // billing-reject path — see Plan 01 Task 3 Step C
            (LaunchStarted, NeedsManualIntervention) => true,
            (AiAnalysisRequested, IssueBeingFixed) => true,
            (AiAnalysisRequested, NeedsManualIntervention) => true,
            (IssueBeingFixed, IssueFixed) => true,
            (IssueBeingFixed, NeedsManualIntervention) => true,
            // All other transitions (backward, same-state, etc.) rejected
            _ => false,
        }
    }

    /// Remove a launch card by launch_id.
    /// Returns `true` if the card existed and was removed, `false` if unknown.
    ///
    /// Called by:
    ///   - D-11 auto-dismiss 5 min after IssueFixed
    ///   - POST /debug/launches/{id}/dismiss (staff manual dismiss of NeedsManualIntervention)
    pub async fn dismiss(&self, launch_id: &str) -> bool {
        let mut guard = self.states.write().await;
        guard.remove(launch_id).is_some()
    }

    /// Return all active cards sorted newest-first.
    ///
    /// Lock-and-drop: read lock is held only to clone the snapshot — dropped before returning.
    pub async fn get_active(&self) -> Vec<LaunchStatusCard> {
        let snapshot: Vec<LaunchStatusCard> = {
            let guard = self.states.read().await;
            guard.values().cloned().collect()
        };
        let mut cards = snapshot;
        cards.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        cards
    }

    /// Prune stale cards and enforce the MAX_ACTIVE_LAUNCHES cap.
    ///
    /// Cards are stale when they are older than STALE_AFTER_SECS regardless of state.
    /// After age-prune, if still over cap, oldest entries are dropped first.
    ///
    /// Lock-and-drop: write lock acquired and released in one block — no .await inside.
    pub async fn prune(&self) {
        let now = Utc::now();
        let mut guard = self.states.write().await;

        // Age-based eviction
        guard.retain(|_, card| {
            let age_secs = (now - card.timestamp).num_seconds();
            age_secs < STALE_AFTER_SECS
        });

        // Count-based cap: drop oldest entries first
        if guard.len() > MAX_ACTIVE_LAUNCHES {
            let mut entries: Vec<(String, chrono::DateTime<chrono::Utc>)> = guard
                .iter()
                .map(|(k, v)| (k.clone(), v.timestamp))
                .collect();
            // Oldest first
            entries.sort_by(|a, b| a.1.cmp(&b.1));
            let drop_count = guard.len() - MAX_ACTIVE_LAUNCHES;
            for (k, _) in entries.into_iter().take(drop_count) {
                guard.remove(&k);
            }
        }
    }
}

impl Default for LaunchStateMachine {
    // Intentional default: empty initial state — correct for a freshly-started server.
    fn default() -> Self {
        Self::new()
    }
}

// Make LaunchStateMachine cheaply shareable across async tasks via Arc
impl LaunchStateMachine {
    pub fn new_arc() -> Arc<Self> {
        Arc::new(Self::new())
    }
}
