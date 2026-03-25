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

/// A billing accuracy event — records timing relationship between launch command,
/// playable signal, and billing start (METRICS-03).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillingAccuracyEvent {
    pub id: String,
    pub session_id: String,
    pub pod_id: String,
    pub sim_type: Option<String>,
    /// One of: "start", "pause", "resume", "end", "discrepancy"
    pub event_type: String,
    pub launch_command_at: Option<String>,
    pub playable_signal_at: Option<String>,
    pub billing_start_at: Option<String>,
    pub delta_ms: Option<i64>,
    pub details: Option<String>,
}

/// Record a billing accuracy event to SQLite.
/// Errors are logged but never swallowed (METRICS-07).
pub async fn record_billing_accuracy_event(db: &SqlitePool, event: &BillingAccuracyEvent) {
    let now = chrono::Utc::now()
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string();
    let result = sqlx::query(
        "INSERT INTO billing_accuracy_events (id, session_id, pod_id, sim_type, event_type, launch_command_at, playable_signal_at, billing_start_at, delta_ms, details, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&event.id)
    .bind(&event.session_id)
    .bind(&event.pod_id)
    .bind(&event.sim_type)
    .bind(&event.event_type)
    .bind(&event.launch_command_at)
    .bind(&event.playable_signal_at)
    .bind(&event.billing_start_at)
    .bind(event.delta_ms)
    .bind(&event.details)
    .bind(&now)
    .execute(db)
    .await;

    if let Err(e) = result {
        tracing::error!(
            "billing_accuracy_event insert failed for session {}: {e}",
            event.session_id
        );
    }
}

/// Outcome of a crash recovery attempt (METRICS-04).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecoveryOutcome {
    Success,
    Failed,
    PartialSuccess,
}

/// A crash recovery event — records what happened when Race Engineer tried to
/// recover a crashed game (METRICS-04). Feeds Phase 199 history-informed recovery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryEvent {
    pub id: String,
    pub pod_id: String,
    pub sim_type: Option<String>,
    pub car: Option<String>,
    pub track: Option<String>,
    /// ErrorTaxonomy serialized or free text (e.g. "game_crash")
    pub failure_mode: String,
    /// e.g. "auto_relaunch_attempt_1", "auto_relaunch_exhausted"
    pub recovery_action_tried: String,
    pub recovery_outcome: RecoveryOutcome,
    pub recovery_duration_ms: Option<i64>,
    pub error_details: Option<String>,
}

/// Record a crash recovery event to SQLite.
/// Errors are logged but never swallowed (METRICS-07).
pub async fn record_recovery_event(db: &SqlitePool, event: &RecoveryEvent) {
    let now = chrono::Utc::now()
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string();
    let outcome_str = serde_json::to_string(&event.recovery_outcome).unwrap_or_default();

    let result = sqlx::query(
        "INSERT INTO recovery_events (id, pod_id, sim_type, car, track, failure_mode, recovery_action_tried, recovery_outcome, recovery_duration_ms, error_details, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&event.id)
    .bind(&event.pod_id)
    .bind(&event.sim_type)
    .bind(&event.car)
    .bind(&event.track)
    .bind(&event.failure_mode)
    .bind(&event.recovery_action_tried)
    .bind(&outcome_str)
    .bind(event.recovery_duration_ms)
    .bind(&event.error_details)
    .bind(&now)
    .execute(db)
    .await;

    if let Err(e) = result {
        tracing::error!(
            "recovery_event insert failed for pod {}: {e}",
            event.pod_id
        );
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
