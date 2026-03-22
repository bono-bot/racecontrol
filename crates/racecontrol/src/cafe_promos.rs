use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::state::AppState;

// ─── Types ───────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct CafePromo {
    pub id: String,
    pub name: String,
    pub promo_type: String,
    pub config: String,
    pub is_active: bool,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub stacking_group: Option<String>,
    pub created_at: String,
    pub updated_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateCafePromoRequest {
    pub name: String,
    pub promo_type: String,
    pub config: serde_json::Value,
    pub is_active: Option<bool>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub stacking_group: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateCafePromoRequest {
    pub name: Option<String>,
    pub config: Option<serde_json::Value>,
    pub is_active: Option<bool>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub stacking_group: Option<String>,
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn validate_promo_type(promo_type: &str) -> bool {
    matches!(promo_type, "combo" | "happy_hour" | "gaming_bundle")
}

// ─── Handlers ────────────────────────────────────────────────────────────────

pub async fn list_cafe_promos(
    State(state): State<Arc<AppState>>,
) -> Result<(StatusCode, Json<Vec<CafePromo>>), (StatusCode, Json<serde_json::Value>)> {
    let promos = sqlx::query_as::<_, CafePromo>(
        "SELECT id, name, promo_type, config, is_active, start_time, end_time, stacking_group, created_at, updated_at
         FROM cafe_promos
         ORDER BY created_at DESC",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
    })?;

    Ok((StatusCode::OK, Json(promos)))
}

pub async fn create_cafe_promo(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateCafePromoRequest>,
) -> Result<(StatusCode, Json<CafePromo>), (StatusCode, Json<serde_json::Value>)> {
    if !validate_promo_type(&req.promo_type) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "invalid promo_type; must be 'combo', 'happy_hour', or 'gaming_bundle'"})),
        ));
    }

    let id = Uuid::new_v4().to_string();
    let config_str = serde_json::to_string(&req.config).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
    })?;
    let is_active = req.is_active.unwrap_or(false);

    sqlx::query(
        "INSERT INTO cafe_promos (id, name, promo_type, config, is_active, start_time, end_time, stacking_group)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&req.name)
    .bind(&req.promo_type)
    .bind(&config_str)
    .bind(is_active)
    .bind(&req.start_time)
    .bind(&req.end_time)
    .bind(&req.stacking_group)
    .execute(&state.db)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
    })?;

    let promo = sqlx::query_as::<_, CafePromo>(
        "SELECT id, name, promo_type, config, is_active, start_time, end_time, stacking_group, created_at, updated_at
         FROM cafe_promos WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(&state.db)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
    })?;

    Ok((StatusCode::CREATED, Json(promo)))
}

pub async fn update_cafe_promo(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<UpdateCafePromoRequest>,
) -> Result<(StatusCode, Json<CafePromo>), (StatusCode, Json<serde_json::Value>)> {
    let mut set_clauses: Vec<String> = Vec::new();
    let mut has_fields = false;

    if req.name.is_some() {
        set_clauses.push("name = ?".to_string());
        has_fields = true;
    }
    if req.config.is_some() {
        set_clauses.push("config = ?".to_string());
        has_fields = true;
    }
    if req.is_active.is_some() {
        set_clauses.push("is_active = ?".to_string());
        has_fields = true;
    }
    if req.start_time.is_some() {
        set_clauses.push("start_time = ?".to_string());
        has_fields = true;
    }
    if req.end_time.is_some() {
        set_clauses.push("end_time = ?".to_string());
        has_fields = true;
    }
    if req.stacking_group.is_some() {
        set_clauses.push("stacking_group = ?".to_string());
        has_fields = true;
    }

    if !has_fields {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "no fields to update"})),
        ));
    }

    set_clauses.push("updated_at = datetime('now')".to_string());

    let sql = format!(
        "UPDATE cafe_promos SET {} WHERE id = ?",
        set_clauses.join(", ")
    );

    let mut query = sqlx::query(&sql);

    if let Some(ref name) = req.name {
        query = query.bind(name);
    }
    if let Some(ref config) = req.config {
        let config_str = serde_json::to_string(config).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
        })?;
        query = query.bind(config_str);
    }
    if let Some(is_active) = req.is_active {
        query = query.bind(is_active);
    }
    if let Some(ref start_time) = req.start_time {
        query = query.bind(start_time);
    }
    if let Some(ref end_time) = req.end_time {
        query = query.bind(end_time);
    }
    if let Some(ref stacking_group) = req.stacking_group {
        query = query.bind(stacking_group);
    }

    let result = query
        .bind(&id)
        .execute(&state.db)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
        })?;

    if result.rows_affected() == 0 {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "promo not found"})),
        ));
    }

    let promo = sqlx::query_as::<_, CafePromo>(
        "SELECT id, name, promo_type, config, is_active, start_time, end_time, stacking_group, created_at, updated_at
         FROM cafe_promos WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(&state.db)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
    })?;

    Ok((StatusCode::OK, Json(promo)))
}

pub async fn delete_cafe_promo(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    let result = sqlx::query("DELETE FROM cafe_promos WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
        })?;

    if result.rows_affected() == 0 {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "promo not found"})),
        ));
    }

    Ok(StatusCode::NO_CONTENT)
}

pub async fn toggle_cafe_promo(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<(StatusCode, Json<CafePromo>), (StatusCode, Json<serde_json::Value>)> {
    let result = sqlx::query(
        "UPDATE cafe_promos SET is_active = NOT is_active, updated_at = datetime('now') WHERE id = ?",
    )
    .bind(&id)
    .execute(&state.db)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
    })?;

    if result.rows_affected() == 0 {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "promo not found"})),
        ));
    }

    let promo = sqlx::query_as::<_, CafePromo>(
        "SELECT id, name, promo_type, config, is_active, start_time, end_time, stacking_group, created_at, updated_at
         FROM cafe_promos WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(&state.db)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
    })?;

    Ok((StatusCode::OK, Json(promo)))
}
