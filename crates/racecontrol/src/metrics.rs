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

/// Query launch_events for dynamic timeout: median + 2*stdev of last 10 successful durations.
/// Returns timeout in seconds. Falls back to default_secs if insufficient history (< 3 samples).
/// Floor: 30 seconds regardless of history (LAUNCH-08).
pub async fn query_dynamic_timeout(
    db: &SqlitePool,
    sim_type: &str,
    car: Option<&str>,
    track: Option<&str>,
    default_secs: u64,
) -> u64 {
    let rows: Vec<(i64,)> = sqlx::query_as(
        "SELECT duration_to_playable_ms FROM launch_events
         WHERE sim_type = ? AND (car = ? OR ? IS NULL) AND (track = ? OR ? IS NULL)
           AND outcome = '\"Success\"'
           AND duration_to_playable_ms IS NOT NULL
         ORDER BY created_at DESC LIMIT 10"
    )
    .bind(sim_type).bind(car).bind(car).bind(track).bind(track)
    .fetch_all(db).await.unwrap_or_default();

    if rows.len() < 3 {
        tracing::info!(
            "dynamic timeout: using default {}s for {}/{:?}/{:?} (insufficient history: {} samples)",
            default_secs, sim_type, car, track, rows.len()
        );
        return default_secs;
    }

    let mut durations_ms: Vec<f64> = rows.iter().map(|(d,)| *d as f64).collect();
    durations_ms.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = durations_ms[durations_ms.len() / 2];
    let mean = durations_ms.iter().sum::<f64>() / durations_ms.len() as f64;
    let variance = durations_ms.iter().map(|d| (d - mean).powi(2)).sum::<f64>() / durations_ms.len() as f64;
    let stdev = variance.sqrt();
    let timeout_ms = median + 2.0 * stdev;
    let timeout_secs = (timeout_ms / 1000.0).ceil() as u64;

    tracing::info!(
        "dynamic timeout: {}s for {}/{:?}/{:?} (median={:.0}ms stdev={:.0}ms samples={})",
        timeout_secs, sim_type, car, track, median, stdev, rows.len()
    );
    timeout_secs.max(30)
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn make_db() -> SqlitePool {
        let db = sqlx::sqlite::SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite");
        let _ = sqlx::query(
            "CREATE TABLE IF NOT EXISTS launch_events (
                id TEXT PRIMARY KEY,
                pod_id TEXT NOT NULL,
                sim_type TEXT NOT NULL,
                car TEXT,
                track TEXT,
                session_type TEXT,
                timestamp TEXT NOT NULL,
                outcome TEXT NOT NULL,
                error_taxonomy TEXT,
                duration_to_playable_ms INTEGER,
                error_details TEXT,
                launch_args_hash TEXT,
                attempt_number INTEGER DEFAULT 1,
                db_fallback INTEGER,
                created_at TEXT DEFAULT (datetime('now'))
            )"
        )
        .execute(&db)
        .await;
        db
    }

    async fn insert_success_row(db: &SqlitePool, sim_type: &str, car: Option<&str>, track: Option<&str>, duration_ms: i64) {
        let id = uuid::Uuid::new_v4().to_string();
        let outcome_str = serde_json::to_string(&LaunchOutcome::Success).unwrap_or_default();
        let _ = sqlx::query(
            "INSERT INTO launch_events (id, pod_id, sim_type, car, track, session_type, timestamp, outcome, duration_to_playable_ms, attempt_number)
             VALUES (?, 'pod_1', ?, ?, ?, NULL, datetime('now'), ?, ?, 1)"
        )
        .bind(&id)
        .bind(sim_type)
        .bind(car)
        .bind(track)
        .bind(&outcome_str)
        .bind(duration_ms)
        .execute(db)
        .await;
    }

    #[tokio::test]
    async fn test_dynamic_timeout_with_sufficient_history() {
        let db = make_db().await;
        for _ in 0..10 {
            insert_success_row(&db, "AssettoCorsa", None, None, 25000).await;
        }
        let timeout = query_dynamic_timeout(&db, "AssettoCorsa", None, None, 120).await;
        // median=25000ms stdev=0 -> timeout=25s -> max(30) = 30
        assert!(timeout >= 30, "timeout floor should be 30s, got {}s", timeout);
        assert!(timeout <= 40, "timeout should not be excessive, got {}s", timeout);
    }

    #[tokio::test]
    async fn test_dynamic_timeout_varied_history() {
        let db = make_db().await;
        let durations = [20000i64, 22000, 23000, 24000, 25000, 25000, 26000, 27000, 28000, 30000];
        for d in durations {
            insert_success_row(&db, "AssettoCorsa", None, None, d).await;
        }
        let timeout = query_dynamic_timeout(&db, "AssettoCorsa", None, None, 120).await;
        assert!(timeout >= 30, "timeout should be at least 30s floor, got {}s", timeout);
        assert!(timeout < 120, "dynamic timeout should be less than default 120s, got {}s", timeout);
    }

    #[tokio::test]
    async fn test_dynamic_timeout_insufficient_history() {
        let db = make_db().await;
        for _ in 0..2 {
            insert_success_row(&db, "AssettoCorsa", None, None, 25000).await;
        }
        let timeout = query_dynamic_timeout(&db, "AssettoCorsa", None, None, 90).await;
        assert_eq!(timeout, 90, "Should return default_secs=90 with only 2 samples");
    }

    #[tokio::test]
    async fn test_dynamic_timeout_empty_history() {
        let db = make_db().await;
        let timeout = query_dynamic_timeout(&db, "AssettoCorsa", None, None, 120).await;
        assert_eq!(timeout, 120, "Should return default_secs=120 with no history");
    }

    #[tokio::test]
    async fn test_dynamic_timeout_floor_30s() {
        let db = make_db().await;
        for _ in 0..10 {
            insert_success_row(&db, "AssettoCorsa", None, None, 1000).await;
        }
        let timeout = query_dynamic_timeout(&db, "AssettoCorsa", None, None, 120).await;
        assert!(timeout >= 30, "timeout floor should be 30s, got {}s", timeout);
    }
}
