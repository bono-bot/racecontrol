//! Metrics module — launch event recording infrastructure (METRICS-01, METRICS-02, METRICS-07)
//!
//! Provides dual-write storage: SQLite `launch_events` table + JSONL flat file.
//! If the SQLite insert fails, the event is still written to JSONL with `db_fallback = true`.

#[path = "metrics_combo.rs"]
mod combo;
pub use combo::{ComboReliability, FailureMode, query_combo_reliability, update_combo_reliability};

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
    /// Phase 310: Billing session ID for trace correlation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

/// Record a launch event to both SQLite and JSONL.
/// If the DB insert fails, logs the error and writes to JSONL with `db_fallback = true`.
/// Errors are never swallowed silently (METRICS-07).
pub async fn record_launch_event(db: &SqlitePool, event: &LaunchEvent, venue_id: &str) {
    let outcome_str = serde_json::to_string(&event.outcome).unwrap_or_default();
    let taxonomy_str = event
        .error_taxonomy
        .as_ref()
        .map(|t| serde_json::to_string(t).unwrap_or_default());
    let now = chrono::Utc::now()
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string();

    let db_result = sqlx::query(
        "INSERT INTO launch_events (id, pod_id, sim_type, car, track, session_type, timestamp, outcome, error_taxonomy, duration_to_playable_ms, error_details, launch_args_hash, attempt_number, created_at, venue_id, session_id)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
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
    .bind(venue_id)
    .bind(&event.session_id)
    .execute(db)
    .await;

    let mut jsonl_event = event.clone();
    if let Err(e) = &db_result {
        tracing::error!("launch_event insert failed for pod {}: {}", event.pod_id, e);
        jsonl_event.db_fallback = Some(true);
    }

    // Always write to JSONL (dual storage, METRICS-02)
    append_launch_jsonl(&jsonl_event).await;

    // INTEL-01: Update combo_reliability after every launch event (including crash recovery relaunches).
    // Called after both SQLite insert and JSONL write so all code paths update reliability scores.
    update_combo_reliability(
        db,
        &event.pod_id,
        &event.sim_type,
        event.car.as_deref(),
        event.track.as_deref(),
    )
    .await;
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
pub async fn record_billing_accuracy_event(db: &SqlitePool, event: &BillingAccuracyEvent, venue_id: &str) {
    let now = chrono::Utc::now()
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string();
    let result = sqlx::query(
        "INSERT INTO billing_accuracy_events (id, session_id, pod_id, sim_type, event_type, launch_command_at, playable_signal_at, billing_start_at, delta_ms, details, created_at, venue_id)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
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
    .bind(venue_id)
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
    Attempted,
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
pub async fn record_recovery_event(db: &SqlitePool, event: &RecoveryEvent, venue_id: &str) {
    let now = chrono::Utc::now()
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string();
    let outcome_str = serde_json::to_string(&event.recovery_outcome).unwrap_or_default();

    let result = sqlx::query(
        "INSERT INTO recovery_events (id, pod_id, sim_type, car, track, failure_mode, recovery_action_tried, recovery_outcome, recovery_duration_ms, error_details, created_at, venue_id)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
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
    .bind(venue_id)
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

/// Query recovery_events for the highest-success-rate recovery action over the last 30 days.
/// Requires minimum 3 samples — below that, returns default `("kill_clean_relaunch", 0.0)`.
/// Returns (action_name, success_rate_0_to_1).
pub async fn query_best_recovery_action(
    db: &SqlitePool,
    pod_id: &str,
    sim_type: &str,
    failure_mode: &str,
) -> (String, f64) {
    let rows: Vec<(String, i64, i64)> = sqlx::query_as(
        "SELECT recovery_action_tried,
                COUNT(*) as total,
                SUM(CASE WHEN recovery_outcome='\"Success\"' THEN 1 ELSE 0 END) as successes
         FROM recovery_events
         WHERE pod_id = ? AND sim_type = ? AND failure_mode = ?
           AND created_at > datetime('now', '-30 days')
         GROUP BY recovery_action_tried
         ORDER BY (SUM(CASE WHEN recovery_outcome='\"Success\"' THEN 1 ELSE 0 END) * 1.0 / COUNT(*)) DESC
         LIMIT 1",
    )
    .bind(pod_id)
    .bind(sim_type)
    .bind(failure_mode)
    .fetch_all(db)
    .await
    .unwrap_or_default();

    match rows.first() {
        Some((action, total, successes)) if *total >= 3 => {
            let rate = *successes as f64 / *total as f64;
            (action.clone(), rate)
        }
        _ => ("kill_clean_relaunch".to_string(), 0.0),
    }
}

/// Query launch_events for dynamic timeout: median + 2*stdev of last 10 successful durations.
/// Returns timeout in seconds. Falls back to default_secs if insufficient history (< 3 samples).
///
/// Floor: `default_secs` (per-sim default) regardless of history (Pattern G, 2026-04-18).
/// Supersedes the earlier LAUNCH-08 30s floor, which allowed a dynamic value below the
/// per-sim default when historical launches happened to be unusually fast. Pod 4 F1 25
/// 2026-04-18 17:19:43 IST: dynamic returned 39s, launch needed > 39s, server killed it,
/// BILL-14 retry cascade followed. The dynamic timeout can raise a slow-game floor, but
/// must never lower it below the per-sim safe default.
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
    let computed_secs = (timeout_ms / 1000.0).ceil() as u64;
    let floored_secs = computed_secs.max(default_secs);

    if floored_secs > computed_secs {
        tracing::info!(
            "dynamic timeout: {}s for {}/{:?}/{:?} (computed {}s raised to per-sim default {}s; median={:.0}ms stdev={:.0}ms samples={})",
            floored_secs, sim_type, car, track, computed_secs, default_secs, median, stdev, rows.len()
        );
    } else {
        tracing::info!(
            "dynamic timeout: {}s for {}/{:?}/{:?} (above per-sim default {}s; median={:.0}ms stdev={:.0}ms samples={})",
            floored_secs, sim_type, car, track, default_secs, median, stdev, rows.len()
        );
    }
    floored_secs
}

#[cfg(test)]
#[path = "metrics_tests.rs"]
mod tests;
