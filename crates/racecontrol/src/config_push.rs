/// v22.0 Phase 177: Config push handler — validation, queuing, delivery, and audit logging.
/// Phase 296: Full config push — store/retrieve/push full AgentConfig per pod.
///
/// Endpoints:
///   POST /api/v1/config/push           — validate, queue, and deliver field-level config to pods
///   GET  /api/v1/config/push/queue     — view the per-pod delivery queue
///   GET  /api/v1/config/audit          — view the audit log of all config push events
///   POST /api/v1/config/pod/{pod_id}   — store full AgentConfig for a pod (Phase 296)
///   GET  /api/v1/config/pod/{pod_id}   — retrieve stored AgentConfig for a pod (Phase 296)
use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    Json,
};
use hex;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::auth::middleware::StaffClaims;
use crate::state::AppState;
use rc_common::config_schema::AgentConfig;
use rc_common::protocol::{CoreMessage, CoreToAgentMessage};
use rc_common::types::{ConfigPushPayload, FullConfigPushPayload};

// ─── Request / Response types ────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct PushConfigRequest {
    pub fields: HashMap<String, serde_json::Value>,
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    /// Optional: target specific pods. If empty, push to all connected pods.
    #[serde(default)]
    pub target_pods: Vec<String>,
}

fn default_schema_version() -> u32 {
    1
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ConfigQueueEntry {
    pub id: i64,
    pub pod_id: String,
    pub payload: String,
    pub seq_num: i64,
    pub status: String,
    pub created_at: Option<String>,
    pub acked_at: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct AuditLogEntry {
    pub id: i64,
    pub action: String,
    pub entity_type: String,
    pub entity_name: String,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
    pub pushed_by: String,
    pub pods_acked: String,
    pub seq_num: Option<i64>,
    pub created_at: Option<String>,
}

// ─── Validation ───────────────────────────────────────────────────────────────

/// Validate a config push field map.
/// Returns Ok(()) if all fields are valid, Err(errors_map) with per-field messages otherwise.
pub fn validate_config_push(
    fields: &HashMap<String, serde_json::Value>,
) -> Result<(), HashMap<String, String>> {
    let mut errors: HashMap<String, String> = HashMap::new();

    for (key, value) in fields {
        match key.as_str() {
            "billing_rate" => {
                let ok = value
                    .as_f64()
                    .map(|v| v > 0.0)
                    .unwrap_or(false);
                if !ok {
                    errors.insert(key.clone(), "must be a positive number".to_string());
                }
            }
            "game_limit" => {
                let ok = value
                    .as_i64()
                    .map(|v| (1..=10).contains(&v))
                    .unwrap_or(false);
                if !ok {
                    errors.insert(key.clone(), "must be an integer between 1 and 10".to_string());
                }
            }
            "debug_verbosity" => {
                let valid_levels = ["off", "error", "warn", "info", "debug", "trace"];
                let ok = value
                    .as_str()
                    .map(|s| valid_levels.contains(&s))
                    .unwrap_or(false);
                if !ok {
                    errors.insert(
                        key.clone(),
                        "must be one of [off, error, warn, info, debug, trace]".to_string(),
                    );
                }
            }
            "process_guard_whitelist" => {
                let ok = value
                    .as_array()
                    .map(|arr| {
                        !arr.is_empty() && arr.iter().all(|item| item.as_str().is_some())
                    })
                    .unwrap_or(false);
                if !ok {
                    errors.insert(
                        key.clone(),
                        "must be a non-empty array of strings".to_string(),
                    );
                }
            }
            _ => {
                errors.insert(key.clone(), "unknown config field".to_string());
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

// ─── REST Handlers ────────────────────────────────────────────────────────────

/// POST /api/v1/config/push
/// Validate, queue, and deliver a config push to all or selected pods.
pub async fn push_config(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<StaffClaims>,
    Json(body): Json<PushConfigRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // 1. Validate fields
    if let Err(field_errors) = validate_config_push(&body.fields) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "errors": field_errors })),
        ));
    }

    // 2. Determine target pods
    let target_pods: Vec<String> = if body.target_pods.is_empty() {
        state.pods.read().await.keys().cloned().collect()
    } else {
        body.target_pods.clone()
    };

    // 3. Single seq_num for this push batch
    let seq_num = state.config_push_seq.fetch_add(1, Ordering::SeqCst);

    let payload_json = serde_json::to_string(&body.fields).unwrap_or_else(|_| "{}".to_string());

    let mut queued = 0usize;
    let mut delivered = 0usize;

    // 4. Queue and deliver per pod
    for pod_id in &target_pods {
        // Insert into config_push_queue
        let insert_result = sqlx::query(
            "INSERT INTO config_push_queue (pod_id, payload, seq_num, status) VALUES (?, ?, ?, 'pending')",
        )
        .bind(pod_id)
        .bind(&payload_json)
        .bind(seq_num as i64)
        .execute(&state.db)
        .await;

        if let Err(e) = insert_result {
            tracing::error!("Failed to insert config_push_queue entry for pod {}: {}", pod_id, e);
            continue;
        }
        queued += 1;

        // Deliver if pod is connected
        let sender = state.agent_senders.read().await.get(pod_id).cloned();
        if let Some(tx) = sender {
            let push_payload = ConfigPushPayload {
                fields: body.fields.clone(),
                schema_version: body.schema_version,
                sequence: seq_num,
            };
            match tx.send(CoreMessage::wrap(CoreToAgentMessage::ConfigPush(push_payload))).await {
                Ok(_) => {
                    delivered += 1;
                    // Update status to delivered
                    let _ = sqlx::query(
                        "UPDATE config_push_queue SET status = 'delivered' WHERE pod_id = ? AND seq_num = ?",
                    )
                    .bind(pod_id)
                    .bind(seq_num as i64)
                    .execute(&state.db)
                    .await;
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to send ConfigPush to pod {} (seq={}): {}",
                        pod_id, seq_num, e
                    );
                }
            }
        }
        // If pod not in agent_senders, leave status='pending' (offline pod: CP-02)
    }

    // 5. Write audit log entry
    let field_keys: Vec<&str> = body.fields.keys().map(|s| s.as_str()).collect();
    let entity_name = field_keys.join(",");
    let _ = sqlx::query(
        "INSERT INTO config_audit_log \
         (action, entity_type, entity_name, old_value, new_value, pushed_by, pods_acked, seq_num) \
         VALUES ('config_push', 'config', ?, NULL, ?, ?, '[]', ?)",
    )
    .bind(&entity_name)
    .bind(&payload_json)
    .bind(&claims.sub)
    .bind(seq_num as i64)
    .execute(&state.db)
    .await;

    Ok(Json(json!({
        "queued": queued,
        "delivered": delivered,
        "seq_nums": [seq_num]
    })))
}

/// GET /api/v1/config/push/queue
pub async fn get_queue(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<ConfigQueueEntry>>, (StatusCode, String)> {
    let rows = sqlx::query_as::<_, ConfigQueueEntry>(
        "SELECT id, pod_id, payload, seq_num, status, created_at, acked_at \
         FROM config_push_queue ORDER BY id DESC LIMIT 100",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(rows))
}

/// GET /api/v1/config/audit
pub async fn get_audit_log(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<AuditLogEntry>>, (StatusCode, String)> {
    let rows = sqlx::query_as::<_, AuditLogEntry>(
        "SELECT id, action, entity_type, entity_name, old_value, new_value, \
         pushed_by, pods_acked, seq_num, created_at \
         FROM config_audit_log ORDER BY id DESC LIMIT 100",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(rows))
}

// ─── Phase 296: Full config push — store, retrieve, push ─────────────────────

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

// ─── Phase 296: REST handlers for pod config storage/retrieval ───────────────

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

// ─── Reconnect replay ─────────────────────────────────────────────────────────

/// Replay all unacked config pushes for a pod that just reconnected.
///
/// IMPORTANT: Uses `status != 'acked'` as the filter — NOT a sequence number comparison.
/// FlagCacheSync.cached_version is a FLAG version counter, not a config push sequence number.
/// Using it as last_seq would silently skip valid config pushes.
pub async fn replay_pending_config_pushes(
    state: &AppState,
    pod_id: &str,
    cmd_tx: &mpsc::Sender<CoreMessage>,
) {
    let rows = sqlx::query_as::<_, ConfigQueueEntry>(
        "SELECT id, pod_id, payload, seq_num, status, created_at, acked_at \
         FROM config_push_queue \
         WHERE pod_id = ? AND status != 'acked' \
         ORDER BY seq_num ASC",
    )
    .bind(pod_id)
    .fetch_all(&state.db)
    .await;

    let rows = match rows {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("Failed to query pending config pushes for pod {}: {}", pod_id, e);
            return;
        }
    };

    for entry in rows {
        let fields: HashMap<String, serde_json::Value> = match serde_json::from_str(&entry.payload) {
            Ok(f) => f,
            Err(e) => {
                tracing::warn!(
                    "Skipping malformed config push payload for pod {} seq={}: {}",
                    pod_id, entry.seq_num, e
                );
                continue;
            }
        };

        let push_payload = ConfigPushPayload {
            fields,
            schema_version: 1,
            sequence: entry.seq_num as u64,
        };

        match cmd_tx.send(CoreMessage::wrap(CoreToAgentMessage::ConfigPush(push_payload))).await {
            Ok(_) => {
                tracing::info!(
                    "Replayed config push seq={} to reconnected pod {}",
                    entry.seq_num, pod_id
                );
                let _ = sqlx::query(
                    "UPDATE config_push_queue SET status = 'delivered' WHERE id = ?",
                )
                .bind(entry.id)
                .execute(&state.db)
                .await;
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to replay config push seq={} to pod {}: {}",
                    entry.seq_num, pod_id, e
                );
            }
        }
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod config_push_tests {
    use super::*;
    use rc_common::config_schema::AgentConfig;
    use rc_common::types::FullConfigPushPayload;
    use rc_common::protocol::{CoreMessage, CoreToAgentMessage};

    /// Test 1: FullConfigPushPayload serializes to JSON with config, config_hash, schema_version fields
    #[test]
    fn full_config_push_payload_serializes_expected_fields() {
        let config = AgentConfig::default();
        let config_hash = compute_config_hash(&config);
        let payload = FullConfigPushPayload {
            schema_version: config.schema_version,
            config,
            config_hash: config_hash.clone(),
        };
        let json = serde_json::to_string(&payload).expect("should serialize");
        assert!(json.contains("\"config\""), "JSON must have 'config' field: {json}");
        assert!(json.contains("\"config_hash\""), "JSON must have 'config_hash' field: {json}");
        assert!(json.contains("\"schema_version\""), "JSON must have 'schema_version' field: {json}");
        assert!(json.contains(&config_hash), "JSON must contain the hash: {json}");
    }

    /// Test 2: FullConfigPushPayload round-trips through serde
    #[test]
    fn full_config_push_payload_serde_roundtrip() {
        let config = AgentConfig::default();
        let config_hash = compute_config_hash(&config);
        let payload = FullConfigPushPayload {
            schema_version: config.schema_version,
            config,
            config_hash: config_hash.clone(),
        };
        let json = serde_json::to_string(&payload).expect("should serialize");
        let decoded: FullConfigPushPayload = serde_json::from_str(&json).expect("should deserialize");
        assert_eq!(decoded.config_hash, config_hash);
        assert_eq!(decoded.schema_version, 1);
    }

    /// Test 3: CoreToAgentMessage::FullConfigPush variant serializes with type="full_config_push"
    #[test]
    fn core_to_agent_full_config_push_has_correct_type_tag() {
        let config = AgentConfig::default();
        let config_hash = compute_config_hash(&config);
        let payload = FullConfigPushPayload {
            schema_version: config.schema_version,
            config,
            config_hash,
        };
        let msg = CoreToAgentMessage::FullConfigPush(payload);
        let json = serde_json::to_string(&msg).expect("should serialize");
        assert!(json.contains("\"full_config_push\""), "type tag must be full_config_push: {json}");
    }

    /// Test 4: compute_config_hash returns consistent SHA-256 hex for same AgentConfig
    #[test]
    fn compute_config_hash_is_consistent() {
        let config = AgentConfig::default();
        let hash1 = compute_config_hash(&config);
        let hash2 = compute_config_hash(&config);
        assert_eq!(hash1, hash2, "Same config must produce same hash");
        // SHA-256 hex = 64 chars
        assert_eq!(hash1.len(), 64, "SHA-256 hex should be 64 chars");
    }

    /// Test 5: compute_config_hash returns different hash for different AgentConfig values
    #[test]
    fn compute_config_hash_differs_for_different_configs() {
        let mut config1 = AgentConfig::default();
        let mut config2 = AgentConfig::default();
        config1.pod.number = 1;
        config2.pod.number = 2;
        let hash1 = compute_config_hash(&config1);
        let hash2 = compute_config_hash(&config2);
        assert_ne!(hash1, hash2, "Different configs must produce different hashes");
    }
}
