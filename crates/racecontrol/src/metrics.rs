//! Metrics module — launch event recording infrastructure (METRICS-01, METRICS-02, METRICS-07)
//!
//! Provides dual-write storage: SQLite `launch_events` table + JSONL flat file.
//! If the SQLite insert fails, the event is still written to JSONL with `db_fallback = true`.

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;

/// Outcome of a game launch attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LaunchOutcome {
    Success,
    Timeout,
    Crash,
    Error,
    Rejected,
}

/// Structured error classification for launch failures.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ErrorTaxonomy {
    ShaderCompilationFail,
    OutOfMemory,
    AntiCheatKick,
    ConfigCorrupt,
    ProcessCrash { exit_code: i64 },
    LaunchTimeout,
    ContentManagerHang,
    MissingDependency,
    BillingGateRejected,
    FeatureFlagDisabled,
    AgentDisconnected,
    Unknown,
}

/// A single launch event record — written to both SQLite and JSONL.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchEvent {
    pub id: String,
    pub pod_id: String,
    pub sim_type: String,
    pub car: Option<String>,
    pub track: Option<String>,
    pub session_type: Option<String>,
    pub timestamp: String,
    pub outcome: LaunchOutcome,
    pub error_taxonomy: Option<ErrorTaxonomy>,
    pub duration_to_playable_ms: Option<i64>,
    pub error_details: Option<String>,
    pub launch_args_hash: Option<String>,
    pub attempt_number: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub db_fallback: Option<bool>,
}

/// Record a launch event to both SQLite and JSONL.
/// If the DB insert fails, logs the error and writes to JSONL with `db_fallback = true`.
/// Errors are never swallowed silently (METRICS-07).
pub async fn record_launch_event(db: &SqlitePool, event: &LaunchEvent) {
    let outcome_str = serde_json::to_string(&event.outcome).unwrap_or_default();
    let taxonomy_str = event
        .error_taxonomy
        .as_ref()
        .map(|t| serde_json::to_string(t).unwrap_or_default());
    let now = chrono::Utc::now()
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string();

    let db_result = sqlx::query(
        "INSERT INTO launch_events (id, pod_id, sim_type, car, track, session_type, timestamp, outcome, error_taxonomy, duration_to_playable_ms, error_details, launch_args_hash, attempt_number, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&event.id)
    .bind(&event.pod_id)
    .bind(&event.sim_type)
    .bind(&event.car)
    .bind(&event.track)
    .bind(&event.session_type)
    .bind(&event.timestamp)
    .bind(&outcome_str)
    .bind(&taxonomy_str)
    .bind(event.duration_to_playable_ms)
    .bind(&event.error_details)
    .bind(&event.launch_args_hash)
    .bind(event.attempt_number)
    .bind(&now)
    .execute(db)
    .await;

    let mut jsonl_event = event.clone();
    if let Err(e) = &db_result {
        tracing::error!("launch_event insert failed for pod {}: {}", event.pod_id, e);
        jsonl_event.db_fallback = Some(true);
    }

    // Always write to JSONL (dual storage, METRICS-02)
    append_launch_jsonl(&jsonl_event).await;
}

/// Write a launch event only to JSONL (used for DB-failure fallback path).
pub async fn record_launch_event_jsonl_only(event: &LaunchEvent) {
    append_launch_jsonl(event).await;
}

/// Append a single launch event as a JSON line to the JSONL file.
async fn append_launch_jsonl(event: &LaunchEvent) {
    let jsonl_path = launch_jsonl_path();
    if let Some(parent) = jsonl_path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    match serde_json::to_string(event) {
        Ok(line) => {
            let mut file = match tokio::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&jsonl_path)
                .await
            {
                Ok(f) => f,
                Err(e) => {
                    tracing::error!("Failed to open launch-events.jsonl: {e}");
                    return;
                }
            };
            if let Err(e) = file.write_all(format!("{line}\n").as_bytes()).await {
                tracing::error!("Failed to write to launch-events.jsonl: {e}");
            }
        }
        Err(e) => tracing::error!("Failed to serialize launch event to JSONL: {e}"),
    }
}

/// Platform-specific path for the launch events JSONL file.
fn launch_jsonl_path() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        PathBuf::from(r"C:\RacingPoint\data\launch-events.jsonl")
    }
    #[cfg(not(target_os = "windows"))]
    {
        PathBuf::from("data/launch-events.jsonl")
    }
}

/// Compute a simple hash of launch args JSON for dedup/correlation.
/// Uses DefaultHasher — not cryptographic, but cheap and sufficient for dedup.
pub fn hash_launch_args(args_json: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    args_json.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}
