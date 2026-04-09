//! VMS PATTERN: Local CSV lap fallback when WebSocket is disconnected.
//!
//! VMS writes lap data to local CSV when the network is down, then syncs
//! when connectivity returns. We do the same — laps are never lost.
//!
//! File: C:\RacingPoint\laps-offline.csv
//! Format: timestamp,driver,car,track,lap_time_ms,session_type,lap_number,valid
//!
//! GLD-C-03: Session-end auto-sync via push_csv_fallback.
//! rc-agent calls push_csv_fallback at SessionEnded — reads CSV into memory,
//! POSTs to /api/v1/sessions/{id}/telemetry-fallback with X-Service-Key,
//! clears the file ONLY after confirmed HTTP 200. Retries with exponential
//! backoff. Never clears before confirmed success (pitfall 5 prevention).

use std::io::Write;
use std::path::Path;

const CSV_PATH: &str = r"C:\RacingPoint\laps-offline.csv";
const LOG_TARGET: &str = "csv-lap-fallback";

/// Minimum byte threshold to consider the CSV file non-empty.
/// A file smaller than this is assumed to be header-only or empty.
const MIN_CONTENT_BYTES: u64 = 10;

/// Append a lap to the offline CSV file.
/// Called when WS send fails (disconnected/queue full).
pub fn save_lap_to_csv(lap: &rc_common::types::LapData) {
    let path = Path::new(CSV_PATH);

    let write_header = !path.exists();

    match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        Ok(mut file) => {
            if write_header {
                let _ = writeln!(
                    file,
                    "timestamp,driver_id,car,track,lap_time_ms,session_type,lap_number,valid,id,pod_id"
                );
            }
            let ts = chrono::Utc::now().to_rfc3339();
            let session_type = format!("{:?}", lap.session_type);
            let _ = writeln!(
                file,
                "{},{},{},{},{},{},{},{},{},{}",
                ts,
                csv_escape(&lap.driver_id),
                csv_escape(&lap.car),
                csv_escape(&lap.track),
                lap.lap_time_ms,
                csv_escape(&session_type),
                lap.lap_number,
                if lap.valid { 1 } else { 0 },
                csv_escape(&lap.id),
                csv_escape(&lap.pod_id),
            );
            tracing::info!(
                target: LOG_TARGET,
                "Saved lap to CSV (WS offline): driver={} lap_time={}ms track={}",
                lap.driver_id, lap.lap_time_ms, lap.track
            );
        }
        Err(e) => {
            tracing::error!(target: LOG_TARGET, "Failed to write CSV fallback: {}", e);
        }
    }
}

/// Count pending offline laps (for debug endpoint / sync status).
pub fn pending_csv_lap_count() -> usize {
    let path = Path::new(CSV_PATH);
    if !path.exists() {
        return 0;
    }
    match std::fs::read_to_string(path) {
        Ok(content) => content.lines().count().saturating_sub(1), // -1 for header
        Err(_) => 0,
    }
}

/// Clear the offline CSV after successful sync.
pub fn clear_csv_laps() {
    let path = Path::new(CSV_PATH);
    if path.exists() {
        if let Err(e) = std::fs::remove_file(path) {
            tracing::warn!(target: LOG_TARGET, "Failed to clear CSV fallback: {}", e);
        } else {
            tracing::info!(target: LOG_TARGET, "Cleared offline lap CSV after sync");
        }
    }
}

// ─── GLD-C-03: Session-end CSV fallback push ─────────────────────────────────

/// Production backoff schedule in seconds: 2, 4, 8, 16, 32, 64, 128.
/// Total retry envelope: 254s (~4 min), well within the 10-min budget.
const PRODUCTION_BACKOFFS: &[u64] = &[2, 4, 8, 16, 32, 64, 128];

/// GLD-C-03: POST the local CSV fallback file to the server after session end.
///
/// Read-then-clear ordering (D-07 pitfall 5):
/// 1. Read file into memory FIRST
/// 2. POST to server
/// 3. Call clear_csv_laps() ONLY on confirmed HTTP 200
/// 4. Never clear on error — file remains for the next retry opportunity
///
/// Retries with exponential backoff (PRODUCTION_BACKOFFS). Returns Err if all
/// retries exhausted. Spawned fire-and-forget from SessionEnded handler.
#[cfg(feature = "http-client")]
pub async fn push_csv_fallback(
    server_url: String,
    service_key: String,
    session_id: String,
) -> anyhow::Result<()> {
    push_csv_fallback_inner(
        Path::new(CSV_PATH),
        &server_url,
        &service_key,
        &session_id,
        PRODUCTION_BACKOFFS,
    ).await
}

/// Testable inner function — parameterised over CSV path and backoff schedule.
/// Production code calls push_csv_fallback which injects the real path and backoffs.
/// Tests call this directly with a tempdir path and `&[0u64]` (no sleeps).
#[cfg(feature = "http-client")]
pub(crate) async fn push_csv_fallback_inner(
    csv_path: &Path,
    server_url: &str,
    service_key: &str,
    session_id: &str,
    backoffs: &[u64],
) -> anyhow::Result<()> {
    use std::time::Duration;

    // Skip if file missing or too small (header-only / empty)
    let meta = match tokio::fs::metadata(csv_path).await {
        Ok(m) => m,
        Err(_) => {
            tracing::debug!(target: LOG_TARGET, path = ?csv_path, "csv fallback absent — skipping push");
            return Ok(());
        }
    };
    if meta.len() < MIN_CONTENT_BYTES {
        tracing::debug!(target: LOG_TARGET, bytes = meta.len(), "csv fallback empty — skipping push");
        return Ok(());
    }

    // Read INTO MEMORY before any network activity.
    // Pitfall 5: never clear the file before a confirmed 200.
    let body_bytes = tokio::fs::read(csv_path).await
        .map_err(|e| anyhow::anyhow!("csv fallback read failed: {}", e))?;

    let url = format!(
        "{}/api/v1/sessions/{}/telemetry-fallback",
        server_url.trim_end_matches('/'),
        session_id
    );
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| anyhow::anyhow!("reqwest build failed: {}", e))?;

    // First attempt (delay=0) then backoff retries
    let attempts = std::iter::once(0u64).chain(backoffs.iter().copied());
    let mut last_err: Option<String> = None;

    for (attempt_idx, delay) in attempts.enumerate() {
        if delay > 0 {
            tokio::time::sleep(Duration::from_secs(delay)).await;
        }

        let form = reqwest::multipart::Form::new()
            .part(
                "csv",
                reqwest::multipart::Part::bytes(body_bytes.clone())
                    .file_name(format!("{session_id}.csv"))
                    .mime_str("text/csv")
                    .map_err(|e| anyhow::anyhow!("mime_str error: {}", e))?,
            );

        let res = client
            .post(&url)
            .header("X-Service-Key", service_key)
            .multipart(form)
            .send()
            .await;

        match res {
            Ok(r) if r.status().is_success() => {
                tracing::info!(
                    target: LOG_TARGET,
                    session_id = %session_id,
                    bytes = body_bytes.len(),
                    attempt = attempt_idx + 1,
                    "csv fallback pushed successfully"
                );
                // ONLY NOW clear the local file (read-before-clear guarantee).
                // Use the parameterised path so tests can point to a tempdir.
                if let Err(e) = tokio::fs::remove_file(csv_path).await {
                    tracing::warn!(target: LOG_TARGET, error = %e, "clear_csv_laps failed after successful push");
                } else {
                    tracing::info!(target: LOG_TARGET, "Cleared offline lap CSV after sync");
                }
                return Ok(());
            }
            Ok(r) => {
                let status = r.status();
                last_err = Some(format!("attempt {}: HTTP {}", attempt_idx + 1, status));
                tracing::warn!(
                    target: LOG_TARGET,
                    session_id = %session_id,
                    status = %status,
                    attempt = attempt_idx + 1,
                    "csv fallback push returned non-2xx — will retry"
                );
            }
            Err(e) => {
                last_err = Some(format!("attempt {}: {}", attempt_idx + 1, e));
                tracing::warn!(
                    target: LOG_TARGET,
                    session_id = %session_id,
                    error = %e,
                    attempt = attempt_idx + 1,
                    "csv fallback push network error — will retry"
                );
            }
        }
    }

    Err(anyhow::anyhow!(
        "csv fallback push failed after {} attempts: {}",
        backoffs.len() + 1,
        last_err.unwrap_or_else(|| "unknown".into())
    ))
}

fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_csv_escape_plain() {
        assert_eq!(csv_escape("hello"), "hello");
    }

    #[test]
    fn test_csv_escape_comma() {
        assert_eq!(csv_escape("hello,world"), "\"hello,world\"");
    }

    #[test]
    fn test_csv_escape_quote() {
        assert_eq!(csv_escape("he said \"hi\""), "\"he said \"\"hi\"\"\"");
    }
}

// ─── GLD-C-03: push_csv_fallback tests ───────────────────────────────────────

#[cfg(all(test, feature = "http-client"))]
mod csv_fallback_tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tempfile::TempDir;
    use tokio::net::TcpListener;

    /// Spin up a minimal axum server on an ephemeral port that always returns
    /// `status_code` for POST requests to `/api/v1/sessions/*/telemetry-fallback`.
    /// Returns (base_url, hit_counter).
    async fn mock_server(status_code: u16) -> (String, Arc<AtomicUsize>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let hits = Arc::new(AtomicUsize::new(0));
        let hits_clone = hits.clone();

        tokio::spawn(async move {
            let app = axum::Router::new()
                .route(
                    "/api/v1/sessions/{id}/telemetry-fallback",
                    axum::routing::post(move || {
                        let h = hits_clone.clone();
                        async move {
                            h.fetch_add(1, Ordering::SeqCst);
                            axum::http::StatusCode::from_u16(status_code)
                                .unwrap_or(axum::http::StatusCode::INTERNAL_SERVER_ERROR)
                        }
                    }),
                );
            axum::serve(listener, app).await.unwrap();
        });

        (format!("http://{}", addr), hits)
    }

    /// Write CSV content to a tempdir file and return (TempDir, path).
    fn make_csv_file(dir: &TempDir, content: &str) -> std::path::PathBuf {
        let path = dir.path().join("laps-offline.csv");
        std::fs::write(&path, content).unwrap();
        path
    }

    /// Test 1: mock server returns 200 → file cleared after push.
    #[tokio::test]
    async fn test_push_on_session_end() {
        let (server_url, hits) = mock_server(200).await;
        let dir = TempDir::new().unwrap();
        let csv_path = make_csv_file(&dir, "ts,driver\n2026-01-01,TestDriver\n");

        let result = push_csv_fallback_inner(
            &csv_path,
            &server_url,
            "test-service-key",
            "session-001",
            &[0u64], // no actual sleep
        ).await;

        assert!(result.is_ok(), "push should succeed with 200: {:?}", result.err());
        assert_eq!(hits.load(Ordering::SeqCst), 1, "server should be hit exactly once");
        assert!(!csv_path.exists(), "csv file must be cleared after confirmed 200");
    }

    /// Test 2: CSV file is empty (< MIN_CONTENT_BYTES) → no network call made.
    #[tokio::test]
    async fn test_no_push_when_empty() {
        let (server_url, hits) = mock_server(200).await;
        let dir = TempDir::new().unwrap();
        let csv_path = make_csv_file(&dir, "abc"); // 3 bytes < MIN_CONTENT_BYTES=10

        let result = push_csv_fallback_inner(
            &csv_path,
            &server_url,
            "test-service-key",
            "session-002",
            &[0u64],
        ).await;

        assert!(result.is_ok(), "empty file should return Ok without pushing");
        assert_eq!(hits.load(Ordering::SeqCst), 0, "server must NOT be called for empty csv");
        assert!(csv_path.exists(), "empty csv file must NOT be cleared");
    }

    /// Test 3: CSV file does not exist → Ok(()) returned, no network call.
    #[tokio::test]
    async fn test_no_push_when_missing() {
        let (server_url, hits) = mock_server(200).await;
        let dir = TempDir::new().unwrap();
        let csv_path = dir.path().join("does-not-exist.csv"); // never created

        let result = push_csv_fallback_inner(
            &csv_path,
            &server_url,
            "test-service-key",
            "session-003",
            &[0u64],
        ).await;

        assert!(result.is_ok(), "missing file should return Ok without pushing");
        assert_eq!(hits.load(Ordering::SeqCst), 0, "server must NOT be called when file absent");
    }

    /// Test 4: mock server always returns 500 → Err returned, file NOT cleared.
    /// Uses `backoffs = &[0u64]` so retries are immediate (no actual sleep).
    #[tokio::test]
    async fn test_no_clear_on_failure() {
        let (server_url, hits) = mock_server(500).await;
        let dir = TempDir::new().unwrap();
        let csv_path = make_csv_file(&dir, "ts,driver\n2026-01-01,TestDriver\n");

        let result = push_csv_fallback_inner(
            &csv_path,
            &server_url,
            "test-service-key",
            "session-004",
            &[0u64, 0u64], // 3 total attempts (1 initial + 2 retries), immediate
        ).await;

        assert!(result.is_err(), "push should fail when server always returns 500");
        assert!(hits.load(Ordering::SeqCst) >= 1, "server should be hit at least once");
        assert!(csv_path.exists(), "csv file must NOT be cleared after failed push");
    }
}
