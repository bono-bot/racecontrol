//! VMS PATTERN: Local CSV lap fallback when WebSocket is disconnected.
//!
//! VMS writes lap data to local CSV when the network is down, then syncs
//! when connectivity returns. We do the same — laps are never lost.
//!
//! File: C:\RacingPoint\laps-offline.csv
//! Format: timestamp,driver,car,track,lap_time_ms,session_type,lap_number,valid

use std::io::Write;
use std::path::Path;

const CSV_PATH: &str = r"C:\RacingPoint\laps-offline.csv";
const LOG_TARGET: &str = "csv-lap-fallback";

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
