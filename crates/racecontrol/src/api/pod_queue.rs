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

// ─── UX-08: Virtual queue handlers ───────────────────────────────────────────

#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct QueueJoinBody {
    driver_name: String,
    phone: Option<String>,
    party_size: Option<i64>,
    driver_id: Option<String>,
}

/// POST /queue/join — walk-in joins the virtual queue (no auth required)
pub(crate) async fn queue_join_handler(
    State(state): State<Arc<AppState>>,
    Json(body): Json<QueueJoinBody>,
) -> Json<Value> {
    let id = uuid::Uuid::new_v4().to_string();
    let party_size = body.party_size.unwrap_or(1).max(1);

    // Insert the new queue entry
    if let Err(e) = sqlx::query(
        "INSERT INTO virtual_queue (id, driver_id, driver_name, phone, party_size, status, venue_id)
         VALUES (?, ?, ?, ?, ?, 'waiting', ?)",
    )
    .bind(&id)
    .bind(&body.driver_id)
    .bind(&body.driver_name)
    .bind(&body.phone)
    .bind(party_size)
    .bind(&state.config.venue.venue_id)
    .execute(&state.db)
    .await
    {
        return Json(json!({ "error": e.to_string() }));
    }

    // Compute position: count waiting entries joined before this one + 1
    let position: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) + 1 FROM virtual_queue WHERE status = 'waiting' AND id != ? AND joined_at <= (SELECT joined_at FROM virtual_queue WHERE id = ?)",
    )
    .bind(&id)
    .bind(&id)
    .fetch_one(&state.db)
    .await
    .unwrap_or(1);

    // Estimate wait: position * avg_session_minutes (30 min default) / 8 pods
    let estimated_wait_minutes = (position * 30 + 7) / 8; // ceiling division

    // Update with computed position and ETA
    let _ = sqlx::query(
        "UPDATE virtual_queue SET position = ?, estimated_wait_minutes = ? WHERE id = ?",
    )
    .bind(position)
    .bind(estimated_wait_minutes)
    .bind(&id)
    .execute(&state.db)
    .await;

    Json(json!({
        "queue_id": id,
        "position": position,
        "estimated_wait_minutes": estimated_wait_minutes,
    }))
}

/// GET /queue/status/{id} — customer checks their queue position and status (no auth)
pub(crate) async fn queue_status_handler(
    State(state): State<Arc<AppState>>,
    Path(queue_id): Path<String>,
) -> Json<Value> {
    let row = sqlx::query_as::<_, (String, String, i64, Option<String>, Option<String>)>(
        "SELECT id, status, party_size, joined_at, called_at FROM virtual_queue WHERE id = ?",
    )
    .bind(&queue_id)
    .fetch_optional(&state.db)
    .await;

    let row = match row {
        Ok(Some(r)) => r,
        Ok(None) => return Json(json!({ "error": "Queue entry not found" })),
        Err(e) => return Json(json!({ "error": e.to_string() })),
    };

    let (id, status, party_size, joined_at, called_at) = row;

    // Recompute live position from DB
    let position: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) + 1 FROM virtual_queue WHERE status = 'waiting' AND id != ? AND joined_at <= (SELECT joined_at FROM virtual_queue WHERE id = ?)",
    )
    .bind(&id)
    .bind(&id)
    .fetch_one(&state.db)
    .await
    .unwrap_or(1);

    let estimated_wait_minutes = if status == "waiting" {
        Some((position * 30 + 7) / 8)
    } else {
        None
    };

    Json(json!({
        "queue_id": id,
        "status": status,
        "party_size": party_size,
        "position": if status == "waiting" { Some(position) } else { None },
        "estimated_wait_minutes": estimated_wait_minutes,
        "joined_at": joined_at,
        "called_at": called_at,
    }))
}

/// POST /queue/{id}/leave — customer removes themselves from queue (no auth)
pub(crate) async fn queue_leave_handler(
    State(state): State<Arc<AppState>>,
    Path(queue_id): Path<String>,
) -> Json<Value> {
    match sqlx::query(
        "UPDATE virtual_queue SET status = 'left', updated_at = datetime('now') WHERE id = ? AND status IN ('waiting', 'called')",
    )
    .bind(&queue_id)
    .execute(&state.db)
    .await
    {
        Ok(r) if r.rows_affected() > 0 => Json(json!({ "ok": true })),
        Ok(_) => Json(json!({ "error": "Queue entry not found or already left/seated" })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

/// v29.0 Phase 34: GET /api/v1/pods/:id/availability
/// Returns pod availability from the self-healing availability map (updated by anomaly scanner)
/// with DB fallback for unresolved Critical maintenance events.
/// Used by kiosk maintenance gate to prevent booking degraded/unavailable pods.
pub(crate) async fn pod_availability_handler(
    State(state): State<Arc<AppState>>,
    Path(pod_id): Path<String>,
) -> Json<Value> {
    // Accept both "pod_1" (canonical) and "1" (legacy integer). Strip "pod_" prefix if present.
    // Other /pods/{id}/* endpoints use the string form, so this one is kept consistent.
    let pod_num: u8 = pod_id
        .strip_prefix("pod_")
        .unwrap_or(&pod_id)
        .parse()
        .unwrap_or(0);

    if pod_num == 0 {
        return Json(json!({
            "error": format!("Invalid pod_id: expected 'pod_N' or 'N', got '{}'", pod_id),
        }));
    }

    // Check in-memory availability map (updated by anomaly scanner → self-healing)
    {
        let avail_map = state.pod_availability.read().await;
        if let Some(avail) = avail_map.get(&pod_num) {
            match avail {
                crate::self_healing::PodAvailability::Degraded { reason } => {
                    return Json(json!({
                        "pod_id": pod_id,
                        "state": "Degraded",
                        "reason": reason,
                        "hold_until": null
                    }));
                }
                crate::self_healing::PodAvailability::Unavailable { reason } => {
                    return Json(json!({
                        "pod_id": pod_id,
                        "state": "Unavailable",
                        "reason": reason,
                        "hold_until": null
                    }));
                }
                crate::self_healing::PodAvailability::MaintenanceHold { until, reason } => {
                    return Json(json!({
                        "pod_id": pod_id,
                        "state": "MaintenanceHold",
                        "reason": reason,
                        "hold_until": until
                    }));
                }
                crate::self_healing::PodAvailability::Available => {
                    // Fall through to DB check below
                }
            }
        }
    }

    // DB fallback: check for unresolved Critical events in last hour
    // maintenance_events.pod_id stores the canonical "pod_N" string — use that form.
    let pod_id_canonical = format!("pod_{}", pod_num);
    let critical_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM maintenance_events \
         WHERE pod_id = ?1 AND severity = 'Critical' AND resolved_at IS NULL \
         AND detected_at > datetime('now', '-1 hour')"
    )
    .bind(&pod_id_canonical)
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    if critical_count > 0 {
        Json(json!({
            "pod_id": pod_id,
            "state": "Unavailable",
            "reason": "Critical maintenance alert",
            "hold_until": null
        }))
    } else {
        Json(json!({
            "pod_id": pod_id,
            "state": "Available",
            "reason": null,
            "hold_until": null
        }))
    }
}

/// GET /queue — staff sees all waiting + called entries ordered by join time
pub(crate) async fn queue_list_handler(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, Option<String>, String, Option<String>, i64, String, Option<String>, Option<String>, Option<String>)>(
        "SELECT id, driver_id, driver_name, phone, party_size, status, joined_at, called_at, seated_at
         FROM virtual_queue WHERE status IN ('waiting', 'called') ORDER BY joined_at ASC",
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let entries: Vec<Value> = rows
        .into_iter()
        .map(|(id, driver_id, driver_name, phone, party_size, status, joined_at, called_at, seated_at)| {
            json!({
                "queue_id": id,
                "driver_id": driver_id,
                "driver_name": driver_name,
                "phone": phone,
                "party_size": party_size,
                "status": status,
                "joined_at": joined_at,
                "called_at": called_at,
                "seated_at": seated_at,
            })
        })
        .collect();

    Json(json!({ "queue": entries, "count": entries.len() }))
}

/// POST /queue/{id}/call — staff calls the next customer (status: waiting → called)
pub(crate) async fn queue_call_handler(
    State(state): State<Arc<AppState>>,
    Path(queue_id): Path<String>,
) -> Json<Value> {
    match sqlx::query(
        "UPDATE virtual_queue SET status = 'called', called_at = datetime('now'), updated_at = datetime('now')
         WHERE id = ? AND status = 'waiting'",
    )
    .bind(&queue_id)
    .execute(&state.db)
    .await
    {
        Ok(r) if r.rows_affected() > 0 => Json(json!({ "ok": true })),
        Ok(_) => Json(json!({ "error": "Queue entry not found or not in waiting status" })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

/// POST /queue/{id}/seat — staff marks customer as seated/gaming (status: called → seated)
pub(crate) async fn queue_seat_handler(
    State(state): State<Arc<AppState>>,
    Path(queue_id): Path<String>,
) -> Json<Value> {
    match sqlx::query(
        "UPDATE virtual_queue SET status = 'seated', seated_at = datetime('now'), updated_at = datetime('now')
         WHERE id = ? AND status = 'called'",
    )
    .bind(&queue_id)
    .execute(&state.db)
    .await
    {
        Ok(r) if r.rows_affected() > 0 => Json(json!({ "ok": true })),
        Ok(_) => Json(json!({ "error": "Queue entry not found or not in called status" })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

/// Background task: expire 'called' entries that have been waiting > 10 minutes.
/// Runs every 5 minutes. Spawned at server startup.
pub async fn queue_expire_task(db: sqlx::SqlitePool) {
    tracing::info!("QUEUE: expire task started");
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(300));
    loop {
        interval.tick().await;
        match sqlx::query(
            "UPDATE virtual_queue SET status = 'expired', updated_at = datetime('now')
             WHERE status = 'called' AND called_at < datetime('now', '-10 minutes')",
        )
        .execute(&db)
        .await
        {
            Ok(r) if r.rows_affected() > 0 => {
                tracing::info!("QUEUE: expired {} stale 'called' entries", r.rows_affected());
            }
            Ok(_) => {}
            Err(e) => {
                tracing::warn!("QUEUE: expire task error: {}", e);
            }
        }
    }
}

pub(crate) fn debug_sim_type_to_snake(s: &str) -> Option<&str> {
    match s {
        "AssettoCorsa" => Some("assetto_corsa"),
        "AssettoCorsaEvo" => Some("assetto_corsa_evo"),
        "AssettoCorsaRally" => Some("assetto_corsa_rally"),
        "Iracing" => Some("iracing"),
        "LeManUltimate" | "LeMansUltimate" => Some("le_mans_ultimate"),
        "F125" | "F1_25" => Some("f1_25"),
        "Forza" => Some("forza"),
        "ForzaHorizon5" => Some("forza_horizon_5"),
        _ => None,
    }
}

/// GET /api/v1/fleet/pod-inventory/{pod_id}
///
/// Returns per-pod installed games (sim_types) and per-preset validity for AC experiences.
/// Public endpoint — kiosk fetches without auth (no JWT required).
/// Unknown pod_id returns 200 with empty response (backward compat).
pub async fn pod_inventory_handler(
    State(state): State<Arc<AppState>>,
    Path(pod_id): Path<String>,
) -> Json<Value> {
    // 1. Query pod_game_inventory for launchable games on this pod
    let game_rows: Vec<(Option<String>,)> = sqlx::query_as(
        "SELECT sim_type FROM pod_game_inventory WHERE pod_id = ? AND launchable = 1"
    )
    .bind(&pod_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    // 2. Build installed_sim_types — convert Debug format to snake_case, filter unknowns
    let installed_sim_types: Vec<&str> = game_rows
        .iter()
        .filter_map(|(st,)| st.as_deref().and_then(debug_sim_type_to_snake))
        .collect();

    // 3. Query combo_validation_flags for per-preset validity
    let combo_rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT preset_id, status FROM combo_validation_flags WHERE pod_id = ?"
    )
    .bind(&pod_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    // 4. Build preset_validity: "Available" → "valid", anything else → "invalid"
    let preset_validity: serde_json::Map<String, Value> = combo_rows
        .into_iter()
        .map(|(id, status)| {
            let v = if status == "Available" { "valid" } else { "invalid" };
            (id, Value::String(v.to_string()))
        })
        .collect();

    Json(json!({
        "pod_id": pod_id,
        "installed_sim_types": installed_sim_types,
        "preset_validity": preset_validity,
    }))
}

pub async fn game_matrix_handler(State(state): State<Arc<AppState>>) -> Json<Value> {
    // Fetch distinct games ordered by display_name
    let game_rows: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT DISTINCT game_id, display_name, COALESCE(sim_type, '') FROM pod_game_inventory ORDER BY display_name"
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let mut games: Vec<Value> = Vec::with_capacity(game_rows.len());

    for (game_id, display_name, sim_type) in game_rows {
        // For each game, find which pods have it installed.
        // Clone db ref — never hold lock across await.
        let pod_rows: Vec<(String, i64, String)> = sqlx::query_as(
            "SELECT pod_id, launchable, scanned_at FROM pod_game_inventory WHERE game_id = ?"
        )
        .bind(&game_id)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

        let mut pods_map = serde_json::Map::new();
        for (pod_id, launchable, scanned_at) in pod_rows {
            pods_map.insert(pod_id, json!({
                "installed": true,
                "launchable": launchable != 0,
                "scanned_at": scanned_at,
            }));
        }

        games.push(json!({
            "game_id": game_id,
            "display_name": display_name,
            "sim_type": sim_type,
            "pods": pods_map,
        }));
    }

    Json(json!({ "games": games }))
}
