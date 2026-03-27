/// v22.0 Phase 177: Config push handler — validation, queuing, delivery, and audit logging.
///
/// Endpoints:
///   POST /api/v1/config/push       — validate, queue, and deliver config to pods
///   GET  /api/v1/config/push/queue — view the per-pod delivery queue
///   GET  /api/v1/config/audit      — view the audit log of all config push events
use axum::{
    extract::{Extension, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::auth::middleware::StaffClaims;
use crate::state::AppState;
use rc_common::protocol::CoreToAgentMessage;
use rc_common::types::ConfigPushPayload;

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
            match tx.send(CoreToAgentMessage::ConfigPush(push_payload)).await {
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

// ─── Reconnect replay ─────────────────────────────────────────────────────────

/// Replay all unacked config pushes for a pod that just reconnected.
///
/// IMPORTANT: Uses `status != 'acked'` as the filter — NOT a sequence number comparison.
/// FlagCacheSync.cached_version is a FLAG version counter, not a config push sequence number.
/// Using it as last_seq would silently skip valid config pushes.
pub async fn replay_pending_config_pushes(
    state: &AppState,
    pod_id: &str,
    cmd_tx: &mpsc::Sender<CoreToAgentMessage>,
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

        match cmd_tx.send(CoreToAgentMessage::ConfigPush(push_payload)).await {
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
