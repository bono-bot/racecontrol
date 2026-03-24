//! Recovery events API (COORD-04).
//!
//! Provides:
//! - `RecoveryEventStore`: in-memory ring buffer (200 events max)
//! - `post_recovery_event`: POST /api/v1/recovery/events -- accepts event, returns 201
//! - `get_recovery_events`: GET /api/v1/recovery/events?pod_id=X&since_secs=N -- filtered query

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::Utc;
use serde::Deserialize;
use std::collections::VecDeque;
use std::sync::Arc;

use crate::state::AppState;
use rc_common::recovery::{RecoveryEvent, RecoveryIntent};

const MAX_EVENTS: usize = 200;

/// In-memory ring buffer for recovery events. FIFO eviction at MAX_EVENTS.
#[derive(Debug, Default)]
pub struct RecoveryEventStore {
    events: VecDeque<RecoveryEvent>,
}

impl RecoveryEventStore {
    pub fn new() -> Self {
        Self { events: VecDeque::with_capacity(MAX_EVENTS) }
    }

    pub fn push(&mut self, event: RecoveryEvent) {
        if self.events.len() >= MAX_EVENTS {
            self.events.pop_front();
        }
        self.events.push_back(event);
    }

    pub fn query(&self, pod_id: Option<&str>, since_secs: Option<u64>) -> Vec<RecoveryEvent> {
        let cutoff = since_secs.map(|s| Utc::now() - chrono::Duration::seconds(s as i64));
        self.events.iter()
            .filter(|e| {
                if let Some(ref pid) = pod_id {
                    if e.pod_id != *pid { return false; }
                }
                if let Some(ref cut) = cutoff {
                    if e.timestamp < *cut { return false; }
                }
                true
            })
            .cloned()
            .collect()
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }
}

/// In-memory store for recovery intents with TTL-based deconfliction (COORD-02).
///
/// Before any recovery authority acts on a pod+process, it registers an intent here.
/// Other authorities must check `has_active_intent` and back off if one is found.
/// Intents expire automatically after 2 minutes (TTL enforced by RecoveryIntent::is_expired).
#[derive(Debug, Default)]
pub struct RecoveryIntentStore {
    intents: Vec<RecoveryIntent>,
}

impl RecoveryIntentStore {
    pub fn new() -> Self {
        Self { intents: Vec::new() }
    }

    /// Register a new intent. Cleans up expired entries first.
    pub fn register(&mut self, intent: RecoveryIntent) {
        self.cleanup_expired();
        self.intents.push(intent);
    }

    /// Returns the first active (non-expired) intent for this pod_id + process, if any.
    pub fn has_active_intent(&self, pod_id: &str, process: &str) -> Option<&RecoveryIntent> {
        self.intents.iter().find(|i| {
            i.pod_id == pod_id && i.process == process && !i.is_expired()
        })
    }

    /// Remove all expired intents. Called automatically on register.
    pub fn cleanup_expired(&mut self) {
        self.intents.retain(|i| !i.is_expired());
    }

    /// Number of active (non-expired) intents in the store.
    pub fn active_len(&self) -> usize {
        self.intents.iter().filter(|i| !i.is_expired()).count()
    }
}

#[derive(Deserialize)]
pub struct RecoveryEventsQuery {
    pub pod_id: Option<String>,
    pub since_secs: Option<u64>,
}

/// POST /api/v1/recovery/events -- report a recovery event.
pub async fn post_recovery_event(
    State(state): State<Arc<AppState>>,
    Json(mut event): Json<RecoveryEvent>,
) -> StatusCode {
    // Server stamps the timestamp to prevent clock drift from pods
    event.timestamp = Utc::now();

    tracing::info!(
        target: "recovery",
        pod_id = %event.pod_id,
        process = %event.process,
        authority = %event.authority,
        action = %event.action,
        spawn_verified = ?event.spawn_verified,
        "recovery event received"
    );

    let mut store = state.recovery_events.lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    store.push(event);

    StatusCode::CREATED
}

/// GET /api/v1/recovery/events -- query recovery events with optional filters.
pub async fn get_recovery_events(
    State(state): State<Arc<AppState>>,
    Query(params): Query<RecoveryEventsQuery>,
) -> Json<Vec<RecoveryEvent>> {
    let store = state.recovery_events.lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let events = store.query(params.pod_id.as_deref(), params.since_secs);
    Json(events)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use rc_common::recovery::{RecoveryAction, RecoveryAuthority, RecoveryEvent, RecoveryIntent};

    fn make_event(pod_id: &str) -> RecoveryEvent {
        RecoveryEvent {
            pod_id: pod_id.to_string(),
            process: "rc-agent.exe".to_string(),
            authority: RecoveryAuthority::RcSentry,
            action: RecoveryAction::Restart,
            spawn_verified: Some(true),
            server_reachable: Some(true),
            reason: "heartbeat_timeout_60s".to_string(),
            context: String::new(),
            timestamp: Utc::now(),
        }
    }

    #[test]
    fn test_intent_store_register_and_query() {
        let mut store = RecoveryIntentStore::new();
        let intent = RecoveryIntent::new(
            "pod-3",
            "rc-agent.exe",
            RecoveryAuthority::PodHealer,
            "graduated_tier1_restart",
        );
        store.register(intent);
        let found = store.has_active_intent("pod-3", "rc-agent.exe");
        assert!(found.is_some(), "registered intent must be retrievable");
        assert_eq!(found.unwrap().pod_id, "pod-3");
        assert_eq!(found.unwrap().process, "rc-agent.exe");
    }

    #[test]
    fn test_intent_store_expired_not_returned() {
        let mut store = RecoveryIntentStore::new();
        // Insert an intent with a timestamp from 3 minutes ago (past the 2-min TTL)
        let mut old_intent = RecoveryIntent::new(
            "pod-4",
            "rc-agent.exe",
            RecoveryAuthority::RcSentry,
            "old_reason",
        );
        old_intent.created_at = Utc::now() - chrono::Duration::minutes(3);
        store.intents.push(old_intent);
        // Should not find it since it's expired
        let found = store.has_active_intent("pod-4", "rc-agent.exe");
        assert!(found.is_none(), "expired intent must not be returned");
    }

    #[test]
    fn test_intent_store_different_pod_not_returned() {
        let mut store = RecoveryIntentStore::new();
        store.register(RecoveryIntent::new(
            "pod-1",
            "rc-agent.exe",
            RecoveryAuthority::PodHealer,
            "test",
        ));
        let found = store.has_active_intent("pod-2", "rc-agent.exe");
        assert!(found.is_none(), "intent for pod-1 must not match pod-2 query");
    }

    #[test]
    fn test_intent_store_cleanup_removes_expired() {
        let mut store = RecoveryIntentStore::new();
        let mut old_intent = RecoveryIntent::new(
            "pod-5",
            "rc-agent.exe",
            RecoveryAuthority::RcSentry,
            "expired",
        );
        old_intent.created_at = Utc::now() - chrono::Duration::minutes(5);
        store.intents.push(old_intent);
        store.register(RecoveryIntent::new(
            "pod-6",
            "rc-agent.exe",
            RecoveryAuthority::PodHealer,
            "fresh",
        ));
        // cleanup_expired is called internally by register; only pod-6 should remain
        assert_eq!(store.active_len(), 1, "only 1 active (non-expired) intent should remain");
    }

    #[test]
    fn test_recovery_event_serde_roundtrip() {
        let event = make_event("pod-1");
        let json = serde_json::to_string(&event).expect("serialize");
        let restored: RecoveryEvent = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.pod_id, "pod-1");
        assert_eq!(restored.process, "rc-agent.exe");
        assert!(matches!(restored.authority, RecoveryAuthority::RcSentry));
        assert!(matches!(restored.action, RecoveryAction::Restart));
        assert_eq!(restored.spawn_verified, Some(true));
        assert_eq!(restored.server_reachable, Some(true));
        assert_eq!(restored.reason, "heartbeat_timeout_60s");
    }

    #[test]
    fn test_store_push_and_len() {
        let mut store = RecoveryEventStore::new();
        store.push(make_event("pod-1"));
        store.push(make_event("pod-2"));
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn test_store_eviction_at_cap() {
        let mut store = RecoveryEventStore::new();
        // Push 250 events -- first 50 (pod-0 through pod-49) should be evicted
        for i in 0..250u32 {
            let mut event = make_event(&format!("pod-{}", i));
            event.reason = format!("event-{}", i);
            store.push(event);
        }
        assert_eq!(store.len(), 200, "store must cap at 200");
        // First remaining event should be #50 (0-indexed)
        let events = store.query(None, None);
        assert_eq!(events[0].reason, "event-50", "oldest remaining should be event-50");
    }

    #[test]
    fn test_query_by_pod_id() {
        let mut store = RecoveryEventStore::new();
        store.push(make_event("pod-1"));
        store.push(make_event("pod-2"));
        store.push(make_event("pod-1"));

        let results = store.query(Some("pod-1"), None);
        assert_eq!(results.len(), 2, "should return 2 events for pod-1");
        for r in &results {
            assert_eq!(r.pod_id, "pod-1");
        }
    }

    #[test]
    fn test_query_by_since_secs() {
        let mut store = RecoveryEventStore::new();
        // Event with old timestamp (5 minutes ago)
        let mut old_event = make_event("pod-3");
        old_event.timestamp = Utc::now() - chrono::Duration::minutes(5);
        store.push(old_event);
        // Event with recent timestamp
        store.push(make_event("pod-3"));

        let results = store.query(None, Some(120)); // last 120 seconds
        assert_eq!(results.len(), 1, "should only return events within 120 seconds");
        assert_eq!(results[0].pod_id, "pod-3");
    }

    #[test]
    fn test_query_by_both_filters() {
        let mut store = RecoveryEventStore::new();
        let mut old_event = make_event("pod-4");
        old_event.timestamp = Utc::now() - chrono::Duration::minutes(5);
        store.push(old_event);
        store.push(make_event("pod-4"));
        store.push(make_event("pod-5"));

        let results = store.query(Some("pod-4"), Some(120));
        assert_eq!(results.len(), 1, "should return only recent pod-4 events");
        assert_eq!(results[0].pod_id, "pod-4");
    }

    #[test]
    fn test_empty_store_query_returns_empty() {
        let store = RecoveryEventStore::new();
        let results = store.query(None, None);
        assert!(results.is_empty(), "empty store should return empty vec");
    }
}
