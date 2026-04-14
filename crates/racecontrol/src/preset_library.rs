//! Phase 298: Game Preset Library — CRUD, reliability scoring, WS push.
//!
//! Endpoints:
//!   GET    /api/v1/presets         — list all presets with reliability scores (public)
//!   POST   /api/v1/presets         — create a new preset (staff JWT)
//!   GET    /api/v1/presets/{id}    — get one preset (public)
//!   PUT    /api/v1/presets/{id}    — update preset (staff JWT)
//!   DELETE /api/v1/presets/{id}    — soft-delete preset (staff JWT, sets enabled=0)
use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::auth::middleware::StaffClaims;
use crate::state::AppState;
use rc_common::protocol::{CoreMessage, CoreToAgentMessage};
use rc_common::types::{GamePreset, GamePresetWithReliability, PresetPushPayload};

// ─── DB helpers ──────────────────────────────────────────────────────────────

/// Fetch all enabled presets and attach aggregated reliability scores.
///
/// Reliability score = AVG(success_rate) from combo_reliability WHERE
///   sim_type = preset.game AND (car = preset.car OR preset.car IS NULL)
///   AND (track = preset.track OR preset.track IS NULL)
///   AND SUM(total_launches) >= 5.
///
/// Aggregates across ALL pods — a preset is unreliable if it fails on any pod.
pub async fn list_presets_with_reliability(
    db: &sqlx::SqlitePool,
    unreliable_threshold: f64,
) -> Result<Vec<GamePresetWithReliability>, sqlx::Error> {
    // Fetch raw rows: game_presets doesn't use sqlx::FromRow on the shared type,
    // so we map columns explicitly.
    let rows = sqlx::query(
        "SELECT id, name, game, car, track, session_type, notes, enabled, created_at
         FROM game_presets WHERE enabled = 1 ORDER BY name ASC",
    )
    .fetch_all(db)
    .await?;

    let mut result = Vec::with_capacity(rows.len());
    for row in rows {
        use sqlx::Row;
        let preset = GamePreset {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            game: row.try_get("game")?,
            car: row.try_get("car")?,
            track: row.try_get("track")?,
            session_type: row.try_get("session_type")?,
            notes: row.try_get("notes")?,
            enabled: {
                let v: i64 = row.try_get("enabled")?;
                v != 0
            },
            created_at: row.try_get("created_at")?,
        };

        // Aggregate reliability across all pods for this (game, car, track) combo.
        let rel_row = sqlx::query(
            "SELECT AVG(cr.success_rate), SUM(cr.total_launches)
             FROM combo_reliability cr
             WHERE cr.sim_type = ?
               AND (? IS NULL OR cr.car = ?)
               AND (? IS NULL OR cr.track = ?)",
        )
        .bind(&preset.game)
        .bind(&preset.car)
        .bind(&preset.car)
        .bind(&preset.track)
        .bind(&preset.track)
        .fetch_optional(db)
        .await?;

        let (reliability_score, total_launches) = match rel_row {
            Some(r) => {
                use sqlx::Row;
                let avg: Option<f64> = r.try_get(0).ok().flatten();
                let total: Option<i64> = r.try_get(1).ok().flatten();
                let total_val = total.unwrap_or(0);
                if total_val >= 5 {
                    (avg, total_val)
                } else {
                    (None, total_val)
                }
            }
            None => (None, 0i64),
        };

        let flagged_unreliable = match reliability_score {
            Some(score) => score < unreliable_threshold,
            None => false, // not enough data to flag
        };

        // Phase 317: Fleet-wide availability from combo_validation_flags
        let fleet_validity = crate::game_inventory::compute_fleet_validity(db, &preset.id)
            .await
            .unwrap_or_else(|_| "unknown".to_string());

        result.push(GamePresetWithReliability {
            preset,
            reliability_score,
            total_launches,
            flagged_unreliable,
            fleet_validity,
        });
    }
    Ok(result)
}

/// Push all presets to a connected pod via its WS sender (PRESET-02).
/// Called on pod WS connect, after push_full_config_to_pod.
pub async fn push_presets_to_pod(
    state: &AppState,
    pod_id: &str,
    cmd_tx: &mpsc::Sender<CoreMessage>,
) -> Result<(), anyhow::Error> {
    let threshold = state.config.presets.unreliable_threshold;
    let presets = list_presets_with_reliability(&state.db, threshold)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to load presets for pod {} push: {}", pod_id, e))?;

    let count = presets.len();
    let payload = PresetPushPayload { presets };
    cmd_tx
        .send(CoreMessage::wrap(CoreToAgentMessage::PresetPush(payload)))
        .await
        .map_err(|e| anyhow::anyhow!("Failed to send PresetPush to pod {}: {}", pod_id, e))?;

    tracing::info!("Pushed {} presets to pod {} on connect", count, pod_id);
    Ok(())
}

// ─── Request types ────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreatePresetRequest {
    pub name: String,
    pub game: String,
    pub car: Option<String>,
    pub track: Option<String>,
    pub session_type: Option<String>,
    pub notes: Option<String>,
    #[serde(default = "bool_true")]
    pub enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePresetRequest {
    pub name: Option<String>,
    pub game: Option<String>,
    pub car: Option<String>,
    pub track: Option<String>,
    pub session_type: Option<String>,
    pub notes: Option<String>,
    pub enabled: Option<bool>,
}

fn bool_true() -> bool { true }

// ─── REST handlers ────────────────────────────────────────────────────────────

/// GET /api/v1/presets (public — pods and kiosk need the list without JWT)
pub async fn list_presets(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<GamePresetWithReliability>>, (StatusCode, Json<Value>)> {
    let threshold = state.config.presets.unreliable_threshold;
    let presets = list_presets_with_reliability(&state.db, threshold)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))))?;
    Ok(Json(presets))
}

/// POST /api/v1/presets (staff JWT required)
pub async fn create_preset(
    State(state): State<Arc<AppState>>,
    Extension(_claims): Extension<StaffClaims>,
    Json(body): Json<CreatePresetRequest>,
) -> Result<Json<GamePreset>, (StatusCode, Json<Value>)> {
    let id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO game_presets (id, name, game, car, track, session_type, notes, enabled)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&body.name)
    .bind(&body.game)
    .bind(&body.car)
    .bind(&body.track)
    .bind(&body.session_type)
    .bind(&body.notes)
    .bind(body.enabled as i64)
    .execute(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))))?;

    let preset = fetch_preset_by_id(&state.db, &id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))))?
        .ok_or_else(|| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": "Failed to retrieve created preset" }))))?;

    tracing::info!("Created preset '{}' (id={}) for game={}", preset.name, preset.id, preset.game);
    Ok(Json(preset))
}

/// GET /api/v1/presets/{id} (public)
pub async fn get_preset(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<GamePreset>, (StatusCode, Json<Value>)> {
    let preset = fetch_preset_by_id(&state.db, &id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))))?;

    preset
        .map(Json)
        .ok_or_else(|| (StatusCode::NOT_FOUND, Json(json!({ "error": "preset not found" }))))
}

/// PUT /api/v1/presets/{id} (staff JWT required)
pub async fn update_preset(
    State(state): State<Arc<AppState>>,
    Extension(_claims): Extension<StaffClaims>,
    Path(id): Path<String>,
    Json(body): Json<UpdatePresetRequest>,
) -> Result<Json<GamePreset>, (StatusCode, Json<Value>)> {
    // Build partial update — only touch provided fields
    if let Some(name) = &body.name {
        sqlx::query("UPDATE game_presets SET name = ? WHERE id = ?")
            .bind(name)
            .bind(&id)
            .execute(&state.db)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))))?;
    }
    if let Some(game) = &body.game {
        sqlx::query("UPDATE game_presets SET game = ? WHERE id = ?")
            .bind(game)
            .bind(&id)
            .execute(&state.db)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))))?;
    }
    if body.car.is_some() {
        sqlx::query("UPDATE game_presets SET car = ? WHERE id = ?")
            .bind(&body.car)
            .bind(&id)
            .execute(&state.db)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))))?;
    }
    if body.track.is_some() {
        sqlx::query("UPDATE game_presets SET track = ? WHERE id = ?")
            .bind(&body.track)
            .bind(&id)
            .execute(&state.db)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))))?;
    }
    if body.session_type.is_some() {
        sqlx::query("UPDATE game_presets SET session_type = ? WHERE id = ?")
            .bind(&body.session_type)
            .bind(&id)
            .execute(&state.db)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))))?;
    }
    if body.notes.is_some() {
        sqlx::query("UPDATE game_presets SET notes = ? WHERE id = ?")
            .bind(&body.notes)
            .bind(&id)
            .execute(&state.db)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))))?;
    }
    if let Some(enabled) = body.enabled {
        sqlx::query("UPDATE game_presets SET enabled = ? WHERE id = ?")
            .bind(enabled as i64)
            .bind(&id)
            .execute(&state.db)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))))?;
    }

    get_preset(State(state), Path(id)).await
}

/// DELETE /api/v1/presets/{id} — soft delete (sets enabled=0, staff JWT required)
pub async fn delete_preset(
    State(state): State<Arc<AppState>>,
    Extension(_claims): Extension<StaffClaims>,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let result = sqlx::query("UPDATE game_presets SET enabled = 0 WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))))?;

    if result.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, Json(json!({ "error": "preset not found" }))));
    }
    Ok(Json(json!({ "deleted": true, "id": id })))
}

// ─── Internal helpers ─────────────────────────────────────────────────────────

async fn fetch_preset_by_id(
    db: &sqlx::SqlitePool,
    id: &str,
) -> Result<Option<GamePreset>, sqlx::Error> {
    use sqlx::Row;
    let row = sqlx::query(
        "SELECT id, name, game, car, track, session_type, notes, enabled, created_at
         FROM game_presets WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(db)
    .await?;

    match row {
        None => Ok(None),
        Some(r) => Ok(Some(GamePreset {
            id: r.try_get("id")?,
            name: r.try_get("name")?,
            game: r.try_get("game")?,
            car: r.try_get("car")?,
            track: r.try_get("track")?,
            session_type: r.try_get("session_type")?,
            notes: r.try_get("notes")?,
            enabled: {
                let v: i64 = r.try_get("enabled")?;
                v != 0
            },
            created_at: r.try_get("created_at")?,
        })),
    }
}

#[cfg(test)]
#[path = "preset_library_tests.rs"]
mod tests;
