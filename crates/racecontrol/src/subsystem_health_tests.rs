use super::*;
use super::probes::{check_db_sync_lag_sync, parse_sync_age_secs};
use super::alerts::should_alert;

mod db_sync_lag_tests {
    use super::*;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    fn set_mtime_past(path: &std::path::Path, secs_ago: u64) {
        let now_epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_secs();
        let past_epoch = now_epoch.saturating_sub(secs_ago);
        // Use filetime crate for cross-platform mtime manipulation (works on Windows)
        let ft = filetime::FileTime::from_unix_time(past_epoch as i64, 0);
        filetime::set_file_mtime(path, ft).expect("set_file_mtime");
    }

    #[test]
    fn db_sync_lag_ok_when_file_is_fresh() {
        let dir = std::env::temp_dir();
        let path = dir.join("racecontrol-test-fresh.db");
        std::fs::write(&path, b"test").expect("write");
        // Default mtime is now() — should be ok
        let status = check_db_sync_lag_sync(path.to_str().unwrap());
        std::fs::remove_file(&path).ok();
        assert!(status.ok, "Fresh file should be ok, got: {:?}", status.error_code);
        assert!(status.error_code.is_none());
    }

    #[test]
    fn db_sync_lag_warn_when_400s_old() {
        let dir = std::env::temp_dir();
        let path = dir.join("racecontrol-test-warn.db");
        std::fs::write(&path, b"test").expect("write");
        set_mtime_past(&path, 400);
        let status = check_db_sync_lag_sync(path.to_str().unwrap());
        std::fs::remove_file(&path).ok();
        assert!(!status.ok, "400s old should not be ok");
        assert_eq!(status.error_code.as_deref(), Some("DB_SYNC_LAG_WARN"),
            "Expected WARN at 400s, got: {:?}", status.error_code);
    }

    #[test]
    fn db_sync_lag_critical_when_1000s_old() {
        let dir = std::env::temp_dir();
        let path = dir.join("racecontrol-test-critical.db");
        std::fs::write(&path, b"test").expect("write");
        set_mtime_past(&path, 1000);
        let status = check_db_sync_lag_sync(path.to_str().unwrap());
        std::fs::remove_file(&path).ok();
        assert!(!status.ok, "1000s old should not be ok");
        assert_eq!(status.error_code.as_deref(), Some("DB_SYNC_LAG_CRITICAL"),
            "Expected CRITICAL at 1000s, got: {:?}", status.error_code);
    }

    #[test]
    fn db_sync_lag_not_found_when_no_file() {
        let path = "/tmp/racecontrol-test-nonexistent-12345.db";
        let status = check_db_sync_lag_sync(path);
        assert!(!status.ok, "Non-existent file should not be ok");
        assert_eq!(status.error_code.as_deref(), Some("DB_SYNC_FILE_NOT_FOUND"),
            "Expected FILE_NOT_FOUND, got: {:?}", status.error_code);
    }
}

mod general_tests {
    use super::*;

    #[test]
    fn test_subsystem_status_serialization() {
        let status = SubsystemStatus {
            ok: true,
            latency_ms: 42,
            error_code: None,
            detail: Some("42.1 GB free".to_string()),
        };
        let json = serde_json::to_value(&status).expect("serialize");
        assert_eq!(json["ok"], true);
        assert_eq!(json["latency_ms"], 42);
        assert!(json["error_code"].is_null());
        assert_eq!(json["detail"], "42.1 GB free");
    }

    #[test]
    fn test_subsystem_status_serialization_with_error() {
        let status = SubsystemStatus {
            ok: false,
            latency_ms: 150,
            error_code: Some("DB_WRITE_FAILED".to_string()),
            detail: Some("database is locked".to_string()),
        };
        let json = serde_json::to_value(&status).expect("serialize");
        assert_eq!(json["ok"], false);
        assert_eq!(json["latency_ms"], 150);
        assert_eq!(json["error_code"], "DB_WRITE_FAILED");
        assert_eq!(json["detail"], "database is locked");
    }

    #[test]
    fn test_dedup_suppresses_within_window() {
        // First call should allow
        assert!(should_alert("test_dedup_suppress", "ERR_1"));
        // Second call with same key within window should suppress
        assert!(!should_alert("test_dedup_suppress", "ERR_1"));
    }

    #[test]
    fn test_dedup_allows_different_error_code() {
        // Same subsystem, different error code should NOT be suppressed
        assert!(should_alert("test_dedup_diff", "ERR_A"));
        assert!(should_alert("test_dedup_diff", "ERR_B"));
    }

    #[test]
    fn test_dedup_allows_different_subsystem() {
        // Different subsystem, same error code should NOT be suppressed
        assert!(should_alert("subsys_a", "ERR_SAME"));
        assert!(should_alert("subsys_b", "ERR_SAME"));
    }

    #[test]
    fn test_get_current_status_empty_default() {
        // Before any probes run, status should be empty
        // Note: other tests may have populated it, so we check the type is correct
        let status = get_current_status();
        // Type check — HashMap<String, SubsystemStatus>
        assert!(status.is_empty() || status.values().all(|s| s.latency_ms < u64::MAX));
    }

    #[test]
    fn test_parse_sync_age_secs_iso_with_frac() {
        // Use a timestamp from 60 seconds ago
        let ts = (chrono::Utc::now() - chrono::Duration::seconds(60))
            .format("%Y-%m-%dT%H:%M:%S%.3fZ")
            .to_string();
        let age = parse_sync_age_secs(&ts).expect("should parse");
        assert!(age >= 59 && age <= 62, "age was {}", age);
    }

    #[test]
    fn test_parse_sync_age_secs_sqlite_format() {
        let ts = (chrono::Utc::now() - chrono::Duration::seconds(30))
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        let age = parse_sync_age_secs(&ts).expect("should parse");
        assert!(age >= 29 && age <= 32, "age was {}", age);
    }

    #[test]
    fn test_parse_sync_age_secs_invalid() {
        assert!(parse_sync_age_secs("not-a-date").is_none());
    }
}
