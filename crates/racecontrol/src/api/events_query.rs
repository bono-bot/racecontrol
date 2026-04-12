#![allow(unused_imports)]
use rand::Rng;
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post, put},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

use crate::ac_server;
use crate::accounting;
use crate::fleet_alert;
use crate::recovery;
use crate::cafe;
use crate::config_push;
use crate::flags;
use crate::policy_engine;
use crate::preset_library;
use crate::cafe_alerts;
use crate::cafe_marketing;
use crate::cafe_promos;
use crate::auth;
use crate::whatsapp_alerter;
use crate::psychology;
use crate::auth::middleware::{require_staff_jwt, require_role_manager, require_role_superadmin};
use crate::network_source::require_non_pod_source;
use crate::billing;
use crate::catalog;
use crate::cloud_sync;
use crate::fleet_health;
use crate::fleet_intelligence;
use crate::process_guard;
use crate::friends;
use crate::game_launcher;
use crate::multiplayer;
use crate::pod_reservation;
use crate::reservation;
use crate::scheduler;
use crate::wallet;
use crate::weekend;
use crate::maintenance_store;
use crate::state::{AppState, VenueConfigSnapshot};
use crate::venue_shutdown;
use crate::wol;
use rc_common::pod_id::normalize_pod_id;
use rc_common::types::*;
use rc_common::protocol::{CloudAction, CoreMessage, CoreToAgentMessage, DashboardEvent};

// ─── Phase 302: Event archive query API ──────────────────────────────────────

#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct EventsQuery {
    event_type: Option<String>,
    pod: Option<String>,
    from: Option<String>, // YYYY-MM-DD
    to: Option<String>,   // YYYY-MM-DD
    limit: Option<i64>,
}

/// GET /api/v1/events — Query system_events with optional filters.
///
/// All filter inputs are validated with character allowlists before SQL
/// interpolation (no raw user data in query strings — standing rule compliance).
/// Payload TEXT column is parsed back to JSON Value to avoid double-encoding.
pub(crate) async fn get_events(
    State(state): State<Arc<AppState>>,
    Query(q): Query<EventsQuery>,
) -> Json<Value> {
    let mut query = String::from(
        "SELECT id, event_type, source, pod, timestamp, payload FROM system_events WHERE 1=1",
    );
    let mut bind_values: Vec<String> = Vec::new();

    if let Some(ref et) = q.event_type {
        // Allow alphanumeric + underscore + dot (e.g. "billing.session_started")
        if et.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.') {
            query.push_str(" AND event_type = ?");
            bind_values.push(et.clone());
        }
    }
    if let Some(ref pod) = q.pod {
        // Allow alphanumeric + hyphen + underscore (e.g. "pod_4", "pod-4")
        if pod.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
            query.push_str(" AND pod = ?");
            bind_values.push(pod.clone());
        }
    }
    if let Some(ref from) = q.from {
        // Validate YYYY-MM-DD: exactly 10 chars, digits + hyphens only
        if from.len() == 10 && from.chars().all(|c| c.is_ascii_digit() || c == '-') {
            query.push_str(" AND date(timestamp) >= ?");
            bind_values.push(from.clone());
        }
    }
    if let Some(ref to) = q.to {
        // Validate YYYY-MM-DD: exactly 10 chars, digits + hyphens only
        if to.len() == 10 && to.chars().all(|c| c.is_ascii_digit() || c == '-') {
            query.push_str(" AND date(timestamp) <= ?");
            bind_values.push(to.clone());
        }
    }

    let limit = q.limit.unwrap_or(200).min(1000);
    query.push_str(" ORDER BY timestamp DESC LIMIT ?");

    let mut qb = sqlx::query_as::<_, (String, String, String, Option<String>, String, String)>(&query);
    for val in &bind_values {
        qb = qb.bind(val);
    }
    qb = qb.bind(limit);

    match qb.fetch_all(&state.db).await {
        Ok(rows) => {
            let events: Vec<Value> = rows
                .into_iter()
                .map(|(id, event_type, source, pod, timestamp, payload)| {
                    // Parse payload TEXT back to JSON Value — avoids double-encoding (Pitfall 5)
                    let payload_val: Value = serde_json::from_str(&payload)
                        .unwrap_or_else(|_| Value::String(payload));
                    json!({
                        "id": id,
                        "event_type": event_type,
                        "source": source,
                        "pod": pod,
                        "timestamp": timestamp,
                        "payload": payload_val,
                    })
                })
                .collect();
            let count = events.len();
            Json(json!({ "events": events, "count": count }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}
