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
use rc_common::recovery::RecoveryEvent;

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
    use rc_common::recovery::{RecoveryAction, RecoveryAuthority, RecoveryEvent};

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
