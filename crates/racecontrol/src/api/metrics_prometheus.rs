//! Prometheus exposition format endpoint — Phase 288 (PROM-01, PROM-02)
//!
//! GET /api/v1/metrics/prometheus — returns all TSDB metrics in Prometheus text format.

use axum::{
    extract::State,
    http::{header, StatusCode},
    response::IntoResponse,
};
use std::collections::BTreeMap;
use std::fmt::Write;
use std::sync::Arc;

use super::metrics_query;
use super::metrics_query::SnapshotEntry;
use crate::state::AppState;

/// HELP descriptions for known metrics.
fn metric_help(name: &str) -> &'static str {
    match name {
        "cpu_usage" => "CPU usage percentage",
        "gpu_temp" => "GPU temperature celsius",
        "fps" => "Frames per second",
        "billing_revenue" => "Billing revenue paise",
        "ws_connections" => "WebSocket connections",
        "pod_health_score" => "Pod health score 0-100",
        "game_session_count" => "Active game sessions",
        _ => "Metric value",
    }
}

/// Format a slice of SnapshotEntry into Prometheus exposition text.
///
/// Groups by metric name, emits HELP + TYPE + gauge lines with optional pod labels.
pub fn format_prometheus(entries: &[SnapshotEntry]) -> String {
    if entries.is_empty() {
        return String::new();
    }

    // Group entries by metric name (BTreeMap for sorted output)
    let mut grouped: BTreeMap<&str, Vec<&SnapshotEntry>> = BTreeMap::new();
    for entry in entries {
        grouped.entry(entry.name.as_str()).or_default().push(entry);
    }

    let mut buf = String::new();
    let mut first = true;

    for (name, entries) in &grouped {
        if !first {
            let _ = writeln!(buf);
        }
        first = false;

        let full_name = format!("racecontrol_{}", name);
        let _ = writeln!(buf, "# HELP {} {}", full_name, metric_help(name));
        let _ = writeln!(buf, "# TYPE {} gauge", full_name);

        for entry in entries {
            if let Some(pod) = entry.pod {
                let _ = writeln!(buf, "{}{{pod=\"pod-{}\"}} {}", full_name, pod, entry.value);
            } else {
                let _ = writeln!(buf, "{} {}", full_name, entry.value);
            }
        }
    }

    buf
}

/// GET /api/v1/metrics/prometheus
///
/// Returns all current TSDB metrics in Prometheus exposition format.
/// Content-Type: text/plain; version=0.0.4; charset=utf-8
pub async fn prometheus_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let entries = metrics_query::query_snapshot(&state.db, None).await;
    let body = format_prometheus(&entries);

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/plain; version=0.0.4; charset=utf-8")],
        body,
    )
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, pod: Option<u32>, value: f64) -> SnapshotEntry {
        SnapshotEntry {
            name: name.to_string(),
            pod,
            value,
            updated_at: 0,
        }
    }

    #[test]
    fn test_empty_snapshot_returns_empty_string() {
        let result = format_prometheus(&[]);
        assert!(result.is_empty(), "empty snapshot must return empty string");
    }

    #[test]
    fn test_single_metric_no_pod_label() {
        let entries = vec![entry("cpu_usage", None, 45.2)];
        let result = format_prometheus(&entries);
        assert!(result.contains("# HELP racecontrol_cpu_usage CPU usage percentage"));
        assert!(result.contains("# TYPE racecontrol_cpu_usage gauge"));
        assert!(result.contains("racecontrol_cpu_usage 45.2"));
        // Should NOT contain pod label
        assert!(!result.contains('{'));
    }

    #[test]
    fn test_metric_with_pod_label() {
        let entries = vec![entry("cpu_usage", Some(3), 67.8)];
        let result = format_prometheus(&entries);
        assert!(
            result.contains("racecontrol_cpu_usage{pod=\"pod-3\"} 67.8"),
            "got: {}",
            result
        );
    }

    #[test]
    fn test_multiple_metrics_grouped() {
        let entries = vec![
            entry("cpu_usage", Some(1), 10.0),
            entry("cpu_usage", Some(2), 20.0),
            entry("gpu_temp", Some(1), 55.0),
        ];
        let result = format_prometheus(&entries);

        // Both metrics present
        assert!(result.contains("# HELP racecontrol_cpu_usage"));
        assert!(result.contains("# HELP racecontrol_gpu_temp"));

        // cpu_usage lines before gpu_temp (alphabetical)
        let cpu_pos = result.find("racecontrol_cpu_usage{").expect("cpu line");
        let gpu_pos = result.find("racecontrol_gpu_temp{").expect("gpu line");
        assert!(cpu_pos < gpu_pos, "cpu_usage should come before gpu_temp");

        // Blank line separates groups
        assert!(result.contains("\n\n"), "groups must be separated by blank line");
    }

    #[test]
    fn test_metric_names_prefixed() {
        let entries = vec![entry("fps", None, 60.0)];
        let result = format_prometheus(&entries);
        assert!(result.contains("racecontrol_fps "), "must be prefixed with racecontrol_");
    }

    #[test]
    fn test_all_known_metrics_have_help() {
        let known = ["cpu_usage", "gpu_temp", "fps", "billing_revenue",
                      "ws_connections", "pod_health_score", "game_session_count"];
        for name in &known {
            let help = metric_help(name);
            assert_ne!(help, "Metric value", "{} should have specific HELP", name);
        }
    }

    #[test]
    fn test_unknown_metric_gets_default_help() {
        assert_eq!(metric_help("some_random_metric"), "Metric value");
    }
}
