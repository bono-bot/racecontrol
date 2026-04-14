/// Phase 296: Full config push — store, retrieve, push full AgentConfig per pod.
///
/// Endpoints:
///   POST /api/v1/config/pod/{pod_id}   — store full AgentConfig for a pod
///   GET  /api/v1/config/pod/{pod_id}   — retrieve stored AgentConfig for a pod

use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    Json,
};
use hex;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::auth::middleware::StaffClaims;
use crate::state::AppState;
use rc_common::config_schema::AgentConfig;
use rc_common::protocol::{CoreMessage, CoreToAgentMessage};
use rc_common::types::FullConfigPushPayload;

/// Compute a deterministic SHA-256 hash of an AgentConfig.
///
/// Serializes to canonical JSON (deterministic field ordering via serde_json),
/// then hashes. Same config always produces the same hash.
/// Agent uses this to skip processing if config hasn't changed (PUSH-06).
pub fn compute_config_hash(config: &AgentConfig) -> String {
    let json = serde_json::to_string(config).unwrap_or_else(|_| "{}".to_string());
    let mut hasher = Sha256::new();
    hasher.update(json.as_bytes());
    let result = hasher.finalize();
    hex::encode(result)
}

/// Store (or replace) the full AgentConfig for a pod in the pod_configs table.
///
/// Returns the SHA-256 hash of the stored config.
pub async fn store_pod_config(
    db: &sqlx::SqlitePool,
    pod_id: &str,
    config: &AgentConfig,
    updated_by: &str,
) -> Result<String, sqlx::Error> {
    let config_json = serde_json::to_string(config).unwrap_or_else(|_| "{}".to_string());
    let config_hash = compute_config_hash(config);
    let schema_version = config.schema_version as i64;

    sqlx::query(
        "INSERT INTO pod_configs (pod_id, config_json, config_hash, schema_version, last_modified, updated_by) \
         VALUES (?, ?, ?, ?, datetime('now'), ?) \
         ON CONFLICT(pod_id) DO UPDATE SET \
           config_json = excluded.config_json, \
           config_hash = excluded.config_hash, \
           schema_version = excluded.schema_version, \
           last_modified = datetime('now'), \
           updated_by = excluded.updated_by",
    )
    .bind(pod_id)
    .bind(&config_json)
    .bind(&config_hash)
    .bind(schema_version)
    .bind(updated_by)
    .execute(db)
    .await?;

    Ok(config_hash)
}

/// Retrieve stored AgentConfig + hash for a pod. Returns None if no config stored.
pub async fn get_pod_config(
    db: &sqlx::SqlitePool,
    pod_id: &str,
) -> Result<Option<(AgentConfig, String)>, sqlx::Error> {
    let row = sqlx::query_as::<_, (String, String)>(
        "SELECT config_json, config_hash FROM pod_configs WHERE pod_id = ?",
    )
    .bind(pod_id)
    .fetch_optional(db)
    .await?;

    match row {
        None => Ok(None),
        Some((config_json, config_hash)) => {
            match serde_json::from_str::<AgentConfig>(&config_json) {
                Ok(config) => Ok(Some((config, config_hash))),
                Err(e) => {
                    tracing::error!(
                        "Failed to deserialize stored config for pod {}: {}",
                        pod_id, e
                    );
                    // Return DB error equivalent — data corruption
                    Err(sqlx::Error::Protocol(format!(
                        "Stored config JSON is corrupt for pod {}: {}",
                        pod_id, e
                    )))
                }
            }
        }
    }
}

/// Push the stored full AgentConfig to a connected pod via its WS sender.
///
/// If no config is stored for the pod, logs info and returns Ok (not an error).
/// If pod config exists, builds FullConfigPushPayload and sends.
pub async fn push_full_config_to_pod(
    state: &AppState,
    pod_id: &str,
    cmd_tx: &mpsc::Sender<CoreMessage>,
) -> Result<(), anyhow::Error> {
    match get_pod_config(&state.db, pod_id).await {
        Err(e) => {
            tracing::warn!("Failed to retrieve config for pod {} push: {}", pod_id, e);
            Err(anyhow::anyhow!("DB error retrieving config for pod {}: {}", pod_id, e))
        }
        Ok(None) => {
            tracing::info!("No stored config for pod {} — skipping full config push", pod_id);
            Ok(())
        }
        Ok(Some((config, config_hash))) => {
            let schema_version = config.schema_version;
            let payload = FullConfigPushPayload {
                config,
                config_hash,
                schema_version,
            };
            cmd_tx
                .send(CoreMessage::wrap(CoreToAgentMessage::FullConfigPush(payload)))
                .await
                .map_err(|e| anyhow::anyhow!("Failed to send FullConfigPush to pod {}: {}", pod_id, e))
        }
    }
}

// ─── REST handlers for pod config storage/retrieval ─────────────────────────

/// POST /api/v1/config/pod/{pod_id}
/// Store (or update) the full AgentConfig for a pod.
/// If the pod is currently connected, pushes the new config immediately.
pub async fn set_pod_config(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<StaffClaims>,
    Path(pod_id): Path<String>,
    Json(config): Json<AgentConfig>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Normalize pod_id to canonical form
    let canonical_id = rc_common::pod_id::normalize_pod_id(&pod_id)
        .unwrap_or_else(|_| pod_id.clone());

    // Store config in DB
    let config_hash = store_pod_config(&state.db, &canonical_id, &config, &claims.sub)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to store config: {}", e) })),
            )
        })?;

    // Check if pod is currently connected and push immediately
    // Clone sender in tight scope — never hold lock across .await (standing rule)
    let maybe_sender = {
        let senders = state.agent_senders.read().await;
        senders.get(&canonical_id).cloned()
    };

    let pushed = if let Some(tx) = maybe_sender {
        match push_full_config_to_pod(&state, &canonical_id, &tx).await {
            Ok(_) => true,
            Err(e) => {
                tracing::warn!(
                    "Stored config for pod {} but push failed (pod may have disconnected): {}",
                    canonical_id, e
                );
                false
            }
        }
    } else {
        false
    };

    // Write audit log entry
    let _ = sqlx::query(
        "INSERT INTO config_audit_log \
         (action, entity_type, entity_name, old_value, new_value, pushed_by, pods_acked) \
         VALUES ('full_config_set', 'pod_config', ?, NULL, ?, ?, '[]')",
    )
    .bind(&canonical_id)
    .bind(&config_hash)
    .bind(&claims.sub)
    .execute(&state.db)
    .await;

    Ok(Json(json!({
        "stored": true,
        "pushed": pushed,
        "config_hash": config_hash,
        "pod_id": canonical_id
    })))
}

/// GET /api/v1/config/pod/{pod_id}
/// Retrieve stored AgentConfig for a pod (returns 404 if none stored).
pub async fn get_pod_config_handler(
    State(state): State<Arc<AppState>>,
    Path(pod_id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let canonical_id = rc_common::pod_id::normalize_pod_id(&pod_id)
        .unwrap_or_else(|_| pod_id.clone());

    // Query with last_modified for the response
    #[derive(sqlx::FromRow)]
    struct PodConfigRow {
        config_json: String,
        config_hash: String,
        last_modified: Option<String>,
    }

    let row = sqlx::query_as::<_, PodConfigRow>(
        "SELECT config_json, config_hash, last_modified FROM pod_configs WHERE pod_id = ?",
    )
    .bind(&canonical_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("DB error: {}", e) })),
        )
    })?;

    match row {
        None => Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": format!("No config stored for pod {}", canonical_id) })),
        )),
        Some(row) => {
            let config_value: serde_json::Value = serde_json::from_str(&row.config_json)
                .unwrap_or(serde_json::Value::Null);
            Ok(Json(json!({
                "pod_id": canonical_id,
                "config": config_value,
                "config_hash": row.config_hash,
                "last_modified": row.last_modified
            })))
        }
    }
}
