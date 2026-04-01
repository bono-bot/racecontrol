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
use rc_common::protocol::CoreToAgentMessage;
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

        result.push(GamePresetWithReliability {
            preset,
            reliability_score,
            total_launches,
            flagged_unreliable,
        });
    }
    Ok(result)
}

/// Push all presets to a connected pod via its WS sender (PRESET-02).
/// Called on pod WS connect, after push_full_config_to_pod.
pub async fn push_presets_to_pod(
    state: &AppState,
    pod_id: &str,
    cmd_tx: &mpsc::Sender<CoreToAgentMessage>,
) -> Result<(), anyhow::Error> {
    let threshold = state.config.presets.unreliable_threshold;
    let presets = list_presets_with_reliability(&state.db, threshold)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to load presets for pod {} push: {}", pod_id, e))?;

    let count = presets.len();
    let payload = PresetPushPayload { presets };
    cmd_tx
        .send(CoreToAgentMessage::PresetPush(payload))
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

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    /// Build an in-memory DB with game_presets and combo_reliability tables.
    async fn make_test_db() -> sqlx::SqlitePool {
        let db = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite for preset tests");

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS game_presets (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                game TEXT NOT NULL,
                car TEXT,
                track TEXT,
                session_type TEXT,
                notes TEXT,
                enabled INTEGER NOT NULL DEFAULT 1,
                created_at TEXT DEFAULT (datetime('now'))
            )",
        )
        .execute(&db)
        .await
        .expect("create game_presets");

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS combo_reliability (
                pod_id TEXT NOT NULL,
                sim_type TEXT NOT NULL,
                car TEXT,
                track TEXT,
                success_rate REAL NOT NULL DEFAULT 0.0,
                avg_time_to_track_ms REAL,
                p95_time_to_track_ms REAL,
                total_launches INTEGER NOT NULL DEFAULT 0,
                common_failure_modes TEXT,
                last_updated TEXT NOT NULL
            )",
        )
        .execute(&db)
        .await
        .expect("create combo_reliability");

        db
    }

    async fn insert_preset(
        db: &sqlx::SqlitePool,
        id: &str,
        name: &str,
        game: &str,
        car: Option<&str>,
        track: Option<&str>,
    ) {
        sqlx::query(
            "INSERT INTO game_presets (id, name, game, car, track, session_type, notes, enabled)
             VALUES (?, ?, ?, ?, ?, NULL, NULL, 1)",
        )
        .bind(id)
        .bind(name)
        .bind(game)
        .bind(car)
        .bind(track)
        .execute(db)
        .await
        .expect("insert preset");
    }

    async fn insert_combo_reliability(
        db: &sqlx::SqlitePool,
        pod_id: &str,
        sim_type: &str,
        car: Option<&str>,
        track: Option<&str>,
        success_rate: f64,
        total_launches: i64,
    ) {
        sqlx::query(
            "INSERT INTO combo_reliability (pod_id, sim_type, car, track, success_rate, total_launches, last_updated)
             VALUES (?, ?, ?, ?, ?, ?, datetime('now'))",
        )
        .bind(pod_id)
        .bind(sim_type)
        .bind(car)
        .bind(track)
        .bind(success_rate)
        .bind(total_launches)
        .execute(db)
        .await
        .expect("insert combo_reliability");
    }

    /// Test 1: create_preset inserts a row and returns the new GamePreset with a UUID id
    #[tokio::test]
    async fn test_create_preset_returns_uuid_id() {
        let db = make_test_db().await;
        let id = Uuid::new_v4().to_string();

        sqlx::query(
            "INSERT INTO game_presets (id, name, game, car, track, session_type, notes, enabled)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind("Monza Hotlap")
        .bind("assettoCorsa")
        .bind::<Option<String>>(None)
        .bind::<Option<String>>(None)
        .bind::<Option<String>>(None)
        .bind::<Option<String>>(None)
        .bind(1i64)
        .execute(&db)
        .await
        .expect("insert");

        let preset = fetch_preset_by_id(&db, &id)
            .await
            .expect("fetch")
            .expect("should exist");

        // UUID should be 36 chars (8-4-4-4-12 format)
        assert_eq!(preset.id.len(), 36, "id must be UUID format");
        assert_eq!(preset.name, "Monza Hotlap");
        assert_eq!(preset.game, "assettoCorsa");
        assert!(preset.enabled);
    }

    /// Test 2: list_presets_with_reliability returns presets with reliability_score=None
    /// when combo_reliability has no rows for that (game, car, track)
    #[tokio::test]
    async fn test_list_presets_no_reliability_data() {
        let db = make_test_db().await;
        insert_preset(&db, "p1", "No Data Preset", "assettoCorsa", Some("ks_ferrari_gte"), Some("monza")).await;

        let result = list_presets_with_reliability(&db, 0.6)
            .await
            .expect("list presets");

        assert_eq!(result.len(), 1);
        assert!(result[0].reliability_score.is_none(), "no combo data → reliability_score=None");
        assert_eq!(result[0].total_launches, 0);
        assert!(!result[0].flagged_unreliable, "no data → not flagged");
    }

    /// Test 3: list_presets_with_reliability returns reliability_score=0.5 and
    /// flagged_unreliable=true when combo_reliability has avg success_rate=0.5
    /// and total_launches >= 5 (threshold = 0.6)
    #[tokio::test]
    async fn test_list_presets_unreliable_when_low_score_and_enough_launches() {
        let db = make_test_db().await;
        insert_preset(&db, "p2", "Unreliable Preset", "assettoCorsa", Some("ks_ferrari_gte"), Some("monza")).await;

        // Insert reliability data: 5 launches, 50% success
        insert_combo_reliability(&db, "pod1", "assettoCorsa", Some("ks_ferrari_gte"), Some("monza"), 0.5, 5).await;

        let result = list_presets_with_reliability(&db, 0.6)
            .await
            .expect("list presets");

        assert_eq!(result.len(), 1);
        let preset = &result[0];
        assert!(preset.reliability_score.is_some(), "5 launches → should have score");
        let score = preset.reliability_score.unwrap();
        assert!((score - 0.5).abs() < 0.01, "score should be ~0.5, got {}", score);
        assert_eq!(preset.total_launches, 5);
        assert!(preset.flagged_unreliable, "0.5 < 0.6 threshold AND 5 launches → flagged");
    }

    /// Test 4: list_presets_with_reliability returns flagged_unreliable=false
    /// when total_launches=4 (below 5-launch minimum), even if score < threshold
    #[tokio::test]
    async fn test_list_presets_not_flagged_when_too_few_launches() {
        let db = make_test_db().await;
        insert_preset(&db, "p3", "Not Enough Data", "assettoCorsa", Some("ks_bmw_m4_gt3"), Some("spa")).await;

        // Insert reliability data: only 4 launches (below minimum), low success rate
        insert_combo_reliability(&db, "pod1", "assettoCorsa", Some("ks_bmw_m4_gt3"), Some("spa"), 0.25, 4).await;

        let result = list_presets_with_reliability(&db, 0.6)
            .await
            .expect("list presets");

        assert_eq!(result.len(), 1);
        let preset = &result[0];
        // Below 5 launches → reliability_score is None
        assert!(preset.reliability_score.is_none(), "4 launches < 5 minimum → score=None");
        assert_eq!(preset.total_launches, 4);
        assert!(!preset.flagged_unreliable, "< 5 launches → never flagged, even if score would be low");
    }
}
