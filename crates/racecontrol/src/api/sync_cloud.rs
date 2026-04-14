#![allow(unused_imports)]
use axum::{
    Json,
    extract::State,
};
use serde_json::{Value, json};
use std::sync::Arc;

use crate::cloud_sync;
use crate::state::{AppState, VenueConfigSnapshot};

// Re-export handlers from submodules so `use super::sync_cloud::*` in routes.rs still works
#[path = "sync_cloud_pull.rs"]
mod sync_cloud_pull;
#[path = "sync_cloud_push.rs"]
mod sync_cloud_push;

pub(crate) use sync_cloud_pull::*;
pub(crate) use sync_cloud_push::*;

// ─── Shared helpers + sync_health ──────────────────────────────────────────

/// Parse a config_snapshot JSON value into a VenueConfigSnapshot.
/// Extracted for testability -- used by sync_push handler.
pub(crate) fn parse_config_snapshot(config_snap: &serde_json::Value) -> VenueConfigSnapshot {
    VenueConfigSnapshot {
        venue_name: config_snap.pointer("/venue/name")
            .and_then(|v| v.as_str()).unwrap_or("RacingPoint").to_string(),
        venue_location: config_snap.pointer("/venue/location")
            .and_then(|v| v.as_str()).unwrap_or_default().to_string(),
        venue_timezone: config_snap.pointer("/venue/timezone")
            .and_then(|v| v.as_str()).unwrap_or("Asia/Kolkata").to_string(),
        pod_count: config_snap.pointer("/pods/count")
            .and_then(|v| v.as_u64()).unwrap_or(0),
        pod_discovery: config_snap.pointer("/pods/discovery")
            .and_then(|v| v.as_bool()).unwrap_or(false),
        pod_healer_enabled: config_snap.pointer("/pods/healer_enabled")
            .and_then(|v| v.as_bool()).unwrap_or(false),
        pod_healer_interval_secs: config_snap.pointer("/pods/healer_interval_secs")
            .and_then(|v| v.as_u64()).unwrap_or(120),
        branding_primary_color: config_snap.pointer("/branding/primary_color")
            .and_then(|v| v.as_str()).unwrap_or_default().to_string(),
        branding_theme: config_snap.pointer("/branding/theme")
            .and_then(|v| v.as_str()).unwrap_or_default().to_string(),
        source: config_snap.pointer("/_meta/source")
            .and_then(|v| v.as_str()).unwrap_or("unknown").to_string(),
        pushed_at: config_snap.pointer("/_meta/pushed_at")
            .and_then(|v| v.as_u64()).unwrap_or(0),
        config_hash: config_snap.pointer("/_meta/hash")
            .and_then(|v| v.as_str()).unwrap_or_default().to_string(),
        received_at: chrono::Utc::now(),
    }
}

pub(crate) async fn sync_health(State(state): State<Arc<AppState>>) -> Json<Value> {
    let driver_count = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM drivers")
        .fetch_one(&state.db)
        .await
        .map(|r| r.0)
        .unwrap_or(0);

    let sync_states = sqlx::query_as::<_, (String, String, i64, String, i64)>(
        "SELECT table_name, last_synced_at, last_sync_count,
                COALESCE(updated_at, last_synced_at),
                COALESCE(conflict_count, 0)
         FROM sync_state ORDER BY table_name",
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let now = chrono::Utc::now();

    let sync_info: Vec<Value> = sync_states
        .iter()
        .map(|(table, last, count, updated, conflicts)| {
            // Compute per-table staleness
            let table_lag = chrono::NaiveDateTime::parse_from_str(updated, "%Y-%m-%d %H:%M:%S")
                .or_else(|_| chrono::NaiveDateTime::parse_from_str(updated, "%Y-%m-%dT%H:%M:%S"))
                .map(|dt| (now - dt.and_utc()).num_seconds())
                .unwrap_or(-1);
            json!({
                "table": table,
                "last_synced_at": last,
                "last_sync_count": count,
                "staleness_seconds": table_lag,
                "conflict_count": conflicts,
            })
        })
        .collect();

    // Compute overall lag from most recent sync activity
    let last_activity = sqlx::query_as::<_, (String,)>(
        "SELECT MAX(COALESCE(updated_at, last_synced_at)) FROM sync_state",
    )
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let lag_seconds: i64 = match last_activity {
        Some((ts,)) => {
            chrono::NaiveDateTime::parse_from_str(&ts, "%Y-%m-%d %H:%M:%S")
                .or_else(|_| chrono::NaiveDateTime::parse_from_str(&ts, "%Y-%m-%dT%H:%M:%S"))
                .map(|dt| (now - dt.and_utc()).num_seconds())
                .unwrap_or(-1)
        }
        None => -1,
    };

    let health_status = if lag_seconds < 0 {
        "unknown"
    } else if lag_seconds <= 60 {
        "healthy"
    } else if lag_seconds <= 300 {
        "degraded"
    } else {
        "critical"
    };

    // Relay status: check if comms-link relay is configured and reachable
    let relay_configured = state.config.cloud.comms_link_url.is_some();
    let relay_available = if relay_configured {
        cloud_sync::is_relay_available(&state).await
    } else {
        false
    };

    // Determine current sync mode
    let sync_mode = if !state.config.cloud.enabled {
        "disabled"
    } else if relay_configured && relay_available {
        "relay"
    } else {
        "http"
    };

    Json(json!({
        "status": health_status,
        "lag_seconds": lag_seconds,
        "drivers": driver_count,
        "cloud_sync_enabled": state.config.cloud.enabled,
        "cloud_api_url": state.config.cloud.api_url,
        "relay_configured": relay_configured,
        "relay_available": relay_available,
        "sync_mode": sync_mode,
        "comms_link_url": state.config.cloud.comms_link_url,
        "sync_state": sync_info,
    }))
}
