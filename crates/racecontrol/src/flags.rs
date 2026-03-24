/// v22.0 Phase 177: Feature flag registry — CRUD handlers, broadcast, and audit logging.
///
/// Endpoints:
///   GET  /api/v1/flags          — list all flags from in-memory cache
///   POST /api/v1/flags          — create a new flag (persisted + broadcast)
///   PUT  /api/v1/flags/:name    — update a flag (persisted + broadcast + audit)
use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

use crate::auth::middleware::StaffClaims;
use crate::state::AppState;

/// A single feature flag row, as stored in the `feature_flags` SQLite table.
#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct FeatureFlagRow {
    pub name: String,
    pub enabled: bool,
    pub default_value: bool,
    /// JSON text — parse with serde_json when needed
    pub overrides: String,
    pub version: i64,
    pub updated_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateFlagRequest {
    pub name: String,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub default_value: bool,
    #[serde(default)]
    pub overrides: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateFlagRequest {
    pub enabled: Option<bool>,
    pub default_value: Option<bool>,
    pub overrides: Option<serde_json::Value>,
}

/// Validate a flag name: non-empty, max 64 chars, alphanumeric + underscores only.
fn validate_flag_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("name cannot be empty".to_string());
    }
    if name.len() > 64 {
        return Err("name must be 64 characters or fewer".to_string());
    }
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err("name must contain only alphanumeric characters and underscores".to_string());
    }
    Ok(())
}

/// Validate override keys: each must match `pod_N` where N is a positive integer.
fn validate_overrides(overrides: &serde_json::Value) -> Result<(), String> {
    if let Some(obj) = overrides.as_object() {
        for key in obj.keys() {
            let rest = key.strip_prefix("pod_").ok_or_else(|| {
                format!("override key '{}' must match pod_N pattern", key)
            })?;
            if rest.is_empty() || !rest.chars().all(|c| c.is_ascii_digit()) {
                return Err(format!(
                    "override key '{}' must match pod_N pattern (e.g. pod_1, pod_8)",
                    key
                ));
            }
        }
    }
    Ok(())
}

/// GET /api/v1/flags — list all flags from in-memory cache, sorted by name.
pub async fn list_flags(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<FeatureFlagRow>>, (StatusCode, Json<serde_json::Value>)> {
    let cache = state.feature_flags.read().await;
    let mut flags: Vec<FeatureFlagRow> = cache.values().cloned().collect();
    flags.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(Json(flags))
}

/// POST /api/v1/flags — create a new feature flag.
pub async fn create_flag(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<StaffClaims>,
    Json(body): Json<CreateFlagRequest>,
) -> Result<(StatusCode, Json<FeatureFlagRow>), (StatusCode, Json<serde_json::Value>)> {
    // Validate name
    if let Err(reason) = validate_flag_name(&body.name) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"errors": {"name": reason}})),
        ));
    }

    let overrides_value = body.overrides.unwrap_or(serde_json::Value::Object(Default::default()));

    // Validate override keys
    if let Err(reason) = validate_overrides(&overrides_value) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"errors": {"overrides": reason}})),
        ));
    }

    let overrides_str = overrides_value.to_string();

    // Insert into DB
    let result = sqlx::query(
        "INSERT INTO feature_flags (name, enabled, default_value, overrides, version, updated_at)
         VALUES (?, ?, ?, ?, 1, datetime('now'))",
    )
    .bind(&body.name)
    .bind(body.enabled)
    .bind(body.default_value)
    .bind(&overrides_str)
    .execute(&state.db)
    .await;

    if let Err(e) = result {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        ));
    }

    // Fetch the newly created row from DB
    let row = sqlx::query_as::<_, FeatureFlagRow>(
        "SELECT name, enabled, default_value, overrides, version, updated_at FROM feature_flags WHERE name = ?",
    )
    .bind(&body.name)
    .fetch_one(&state.db)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
    })?;

    // Write audit log entry
    let new_value = serde_json::to_string(&row).unwrap_or_default();
    if let Err(e) = sqlx::query(
        "INSERT INTO config_audit_log (action, entity_type, entity_name, old_value, new_value, pushed_by, pods_acked, created_at)
         VALUES ('create', 'feature_flag', ?, NULL, ?, ?, '[]', datetime('now'))",
    )
    .bind(&row.name)
    .bind(&new_value)
    .bind(&claims.sub)
    .execute(&state.db)
    .await
    {
        tracing::warn!("Failed to write config_audit_log for create {}: {}", row.name, e);
    }

    // Update in-memory cache
    {
        let mut cache = state.feature_flags.write().await;
        cache.insert(row.name.clone(), row.clone());
    }

    // Broadcast FlagSync to all connected pods
    state.broadcast_flag_sync().await;

    Ok((StatusCode::CREATED, Json(row)))
}

/// PUT /api/v1/flags/:name — update an existing feature flag.
pub async fn update_flag(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Extension(claims): Extension<StaffClaims>,
    Json(body): Json<UpdateFlagRequest>,
) -> Result<Json<FeatureFlagRow>, (StatusCode, Json<serde_json::Value>)> {
    // Read current state from cache
    let old_row = {
        let cache = state.feature_flags.read().await;
        cache.get(&name).cloned()
    };

    let old_row = match old_row {
        Some(r) => r,
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({"error": format!("flag '{}' not found", name)})),
            ));
        }
    };

    // Determine new values (only update provided fields)
    let new_enabled = body.enabled.unwrap_or(old_row.enabled);
    let new_default_value = body.default_value.unwrap_or(old_row.default_value);
    let new_overrides = match body.overrides {
        Some(ref v) => {
            // Validate override keys
            if let Err(reason) = validate_overrides(v) {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(json!({"errors": {"overrides": reason}})),
                ));
            }
            v.to_string()
        }
        None => old_row.overrides.clone(),
    };

    // Execute DB update: increment version, update updated_at
    let result = sqlx::query(
        "UPDATE feature_flags
         SET enabled = ?, default_value = ?, overrides = ?, version = version + 1, updated_at = datetime('now')
         WHERE name = ?",
    )
    .bind(new_enabled)
    .bind(new_default_value)
    .bind(&new_overrides)
    .bind(&name)
    .execute(&state.db)
    .await;

    if let Err(e) = result {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        ));
    }

    // Fetch updated row
    let new_row = sqlx::query_as::<_, FeatureFlagRow>(
        "SELECT name, enabled, default_value, overrides, version, updated_at FROM feature_flags WHERE name = ?",
    )
    .bind(&name)
    .fetch_one(&state.db)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
    })?;

    // Write audit log entry
    let old_value = serde_json::to_string(&old_row).unwrap_or_default();
    let new_value = serde_json::to_string(&new_row).unwrap_or_default();
    if let Err(e) = sqlx::query(
        "INSERT INTO config_audit_log (action, entity_type, entity_name, old_value, new_value, pushed_by, pods_acked, created_at)
         VALUES ('update', 'feature_flag', ?, ?, ?, ?, '[]', datetime('now'))",
    )
    .bind(&name)
    .bind(&old_value)
    .bind(&new_value)
    .bind(&claims.sub)
    .execute(&state.db)
    .await
    {
        tracing::warn!("Failed to write config_audit_log for update {}: {}", name, e);
    }

    // Update in-memory cache
    {
        let mut cache = state.feature_flags.write().await;
        cache.insert(new_row.name.clone(), new_row.clone());
    }

    // Broadcast FlagSync to all connected pods
    state.broadcast_flag_sync().await;

    Ok(Json(new_row))
}
