//! Metrics Query API — Phase 286 (QAPI-01..05)
//!
//! Endpoints:
//!   GET /api/v1/metrics/query     — time-series query with auto-resolution
//!   GET /api/v1/metrics/names     — distinct metric names from both tables
//!   GET /api/v1/metrics/snapshot  — latest value per metric+pod combination

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use std::sync::Arc;
use crate::state::AppState;

// ─── Response types (D-01, D-02, D-03) ────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct TimePoint {
    pub ts: i64,
    pub value: f64,
}

#[derive(Debug, Serialize)]
pub struct QueryResponse {
    pub metric: String,
    pub pod: Option<u32>,
    pub resolution: String,
    pub points: Vec<TimePoint>,
}

#[derive(Debug, Serialize)]
pub struct SnapshotEntry {
    pub name: String,
    pub pod: Option<u32>,
    pub value: f64,
    pub updated_at: i64,
}

#[derive(Debug, Serialize)]
pub struct SnapshotResponse {
    pub metrics: Vec<SnapshotEntry>,
}

#[derive(Debug, Serialize)]
pub struct NamesResponse {
    pub names: Vec<String>,
}

// ─── Query params ──────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct MetricsQueryParams {
    pub metric: String,
    pub from: i64,
    pub to: i64,
    pub pod: Option<u32>,
    pub resolution: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PodFilterParams {
    pub pod: Option<u32>,
}

// ─── Auto-resolution (D-07) ───────────────────────────────────────────────────

pub fn select_resolution(from: i64, to: i64, override_res: Option<&str>) -> &'static str {
    if let Some(r) = override_res {
        match r {
            "raw" => return "raw",
            "hourly" => return "hourly",
            "daily" => return "daily",
            _ => {}
        }
    }
    let range = to - from;
    if range < 86_400 {
        "raw"
    } else if range < 7 * 86_400 {
        "hourly"
    } else {
        "daily"
    }
}

// ─── Inner query functions (testable without AppState) ────────────────────────

/// Query time-series points. Uses metrics_samples for "raw", metrics_rollups for others.
///
/// CRITICAL: CAST(strftime('%s', col) AS INTEGER) — strftime returns TEXT, must CAST.
/// CRITICAL: COALESCE in snapshot join — NULL = NULL is false in SQLite.
pub async fn query_time_series(
    db: &SqlitePool,
    metric: &str,
    from: i64,
    to: i64,
    pod: Option<u32>,
    resolution: &str,
) -> Vec<TimePoint> {
    let pod_str = pod.map(|n| format!("pod-{}", n));

    if resolution == "raw" {
        let rows = if let Some(ref pod_id) = pod_str {
            sqlx::query(
                "SELECT CAST(strftime('%s', recorded_at) AS INTEGER) AS ts, value
                 FROM metrics_samples
                 WHERE metric_name = ?
                   AND recorded_at >= datetime(?, 'unixepoch')
                   AND recorded_at < datetime(?, 'unixepoch')
                   AND pod_id = ?
                 ORDER BY recorded_at",
            )
            .bind(metric)
            .bind(from)
            .bind(to)
            .bind(pod_id)
            .fetch_all(db)
            .await
            .unwrap_or_default()
        } else {
            sqlx::query(
                "SELECT CAST(strftime('%s', recorded_at) AS INTEGER) AS ts, value
                 FROM metrics_samples
                 WHERE metric_name = ?
                   AND recorded_at >= datetime(?, 'unixepoch')
                   AND recorded_at < datetime(?, 'unixepoch')
                 ORDER BY recorded_at",
            )
            .bind(metric)
            .bind(from)
            .bind(to)
            .fetch_all(db)
            .await
            .unwrap_or_default()
        };

        rows.into_iter()
            .map(|r| TimePoint {
                ts: r.try_get::<i64, _>("ts").unwrap_or_default(),
                value: r.try_get::<f64, _>("value").unwrap_or_default(),
            })
            .collect()
    } else {
        // hourly or daily — use metrics_rollups, avg_value as value
        let rows = if let Some(ref pod_id) = pod_str {
            sqlx::query(
                "SELECT CAST(strftime('%s', period_start) AS INTEGER) AS ts, avg_value AS value
                 FROM metrics_rollups
                 WHERE resolution = ?
                   AND metric_name = ?
                   AND period_start >= datetime(?, 'unixepoch')
                   AND period_start < datetime(?, 'unixepoch')
                   AND pod_id = ?
                 ORDER BY period_start",
            )
            .bind(resolution)
            .bind(metric)
            .bind(from)
            .bind(to)
            .bind(pod_id)
            .fetch_all(db)
            .await
            .unwrap_or_default()
        } else {
            sqlx::query(
                "SELECT CAST(strftime('%s', period_start) AS INTEGER) AS ts, avg_value AS value
                 FROM metrics_rollups
                 WHERE resolution = ?
                   AND metric_name = ?
                   AND period_start >= datetime(?, 'unixepoch')
                   AND period_start < datetime(?, 'unixepoch')
                 ORDER BY period_start",
            )
            .bind(resolution)
            .bind(metric)
            .bind(from)
            .bind(to)
            .fetch_all(db)
            .await
            .unwrap_or_default()
        };

        rows.into_iter()
            .map(|r| TimePoint {
                ts: r.try_get::<i64, _>("ts").unwrap_or_default(),
                value: r.try_get::<f64, _>("value").unwrap_or_default(),
            })
            .collect()
    }
}

/// Query all distinct metric names from both tables, sorted.
pub async fn query_metric_names(db: &SqlitePool) -> Vec<String> {
    let rows = sqlx::query(
        "SELECT metric_name FROM (
            SELECT DISTINCT metric_name FROM metrics_samples
            UNION
            SELECT DISTINCT metric_name FROM metrics_rollups
        ) ORDER BY metric_name",
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();

    rows.into_iter()
        .filter_map(|r| r.try_get::<String, _>("metric_name").ok())
        .collect()
}

/// Query latest value per metric+pod combination.
///
/// Uses self-join pattern. COALESCE handles NULL pod_id correctly (NULL = NULL is false in SQLite).
/// CAST(strftime('%s', ...) AS INTEGER) — strftime returns TEXT, must cast to i64.
pub async fn query_snapshot(db: &SqlitePool, pod: Option<u32>) -> Vec<SnapshotEntry> {
    let pod_str = pod.map(|n| format!("pod-{}", n));

    let rows = if let Some(ref pod_id) = pod_str {
        sqlx::query(
            "SELECT s.metric_name, s.pod_id,
                    s.value,
                    CAST(strftime('%s', s.recorded_at) AS INTEGER) AS updated_at
             FROM metrics_samples s
             INNER JOIN (
                 SELECT metric_name, pod_id, MAX(recorded_at) AS max_ts
                 FROM metrics_samples GROUP BY metric_name, pod_id
             ) latest
                 ON s.metric_name = latest.metric_name
                AND COALESCE(s.pod_id, '') = COALESCE(latest.pod_id, '')
                AND s.recorded_at = latest.max_ts
             WHERE s.pod_id = ?
             ORDER BY s.metric_name, s.pod_id",
        )
        .bind(pod_id)
        .fetch_all(db)
        .await
        .unwrap_or_default()
    } else {
        sqlx::query(
            "SELECT s.metric_name, s.pod_id,
                    s.value,
                    CAST(strftime('%s', s.recorded_at) AS INTEGER) AS updated_at
             FROM metrics_samples s
             INNER JOIN (
                 SELECT metric_name, pod_id, MAX(recorded_at) AS max_ts
                 FROM metrics_samples GROUP BY metric_name, pod_id
             ) latest
                 ON s.metric_name = latest.metric_name
                AND COALESCE(s.pod_id, '') = COALESCE(latest.pod_id, '')
                AND s.recorded_at = latest.max_ts
             ORDER BY s.metric_name, s.pod_id",
        )
        .fetch_all(db)
        .await
        .unwrap_or_default()
    };

    rows.into_iter()
        .map(|r| {
            let pod_id: Option<String> = r.try_get("pod_id").ok();
            SnapshotEntry {
                pod: pod_id.as_deref()
                    .and_then(|s| s.strip_prefix("pod-"))
                    .and_then(|n| n.parse().ok()),
                name: r.try_get::<String, _>("metric_name").unwrap_or_default(),
                value: r.try_get::<f64, _>("value").unwrap_or_default(),
                updated_at: r.try_get::<i64, _>("updated_at").unwrap_or_default(),
            }
        })
        .collect()
}

// ─── Handlers ─────────────────────────────────────────────────────────────────

/// GET /api/v1/metrics/query?metric=cpu_usage&from=T1&to=T2[&pod=N][&resolution=raw|hourly|daily]
pub async fn query_handler(
    State(state): State<Arc<AppState>>,
    Query(params): Query<MetricsQueryParams>,
) -> impl IntoResponse {
    // D-05: validate range
    if params.from >= params.to {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "from must be less than to"})),
        )
            .into_response();
    }

    let resolution = select_resolution(params.from, params.to, params.resolution.as_deref());
    let points = query_time_series(
        &state.db,
        &params.metric,
        params.from,
        params.to,
        params.pod,
        resolution,
    )
    .await;

    (
        StatusCode::OK,
        Json(QueryResponse {
            metric: params.metric,
            pod: params.pod,
            resolution: resolution.to_string(),
            points,
        }),
    )
        .into_response()
}

/// GET /api/v1/metrics/names — all distinct metric names from both tables
pub async fn names_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let names = query_metric_names(&state.db).await;
    Json(NamesResponse { names })
}

/// GET /api/v1/metrics/snapshot[?pod=N] — latest value per metric+pod combination
pub async fn snapshot_handler(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PodFilterParams>,
) -> impl IntoResponse {
    let metrics = query_snapshot(&state.db, params.pod).await;
    Json(SnapshotResponse { metrics })
}

// Tests moved to metrics_query_tests.rs for <500 line compliance
