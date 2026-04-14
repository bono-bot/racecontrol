/// Config push request/response types and validation logic.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
