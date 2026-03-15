//! Phased startup log -- records each startup phase to disk for post-mortem analysis.
//!
//! On each rc-agent startup, write_phase() records timestamped phases to
//! `C:\RacingPoint\rc-agent-startup.log`. The first call per process truncates the file
//! (fresh log per startup). Subsequent calls append.
//!
//! detect_crash_recovery() reads the previous startup log to check if the last run
//! completed successfully. Must be called BEFORE the first write_phase() call.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

/// Default path for the startup log on production pods.
const LOG_PATH: &str = r"C:\RacingPoint\rc-agent-startup.log";

/// Tracks whether this process has already written the first phase (truncate vs append).
static LOG_INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Write a startup phase to the default log path.
///
/// First call truncates the file (new startup = fresh log). Subsequent calls append.
/// Never panics -- errors are logged to stderr.
pub fn write_phase(phase: &str, details: &str) {
    write_phase_to(Path::new(LOG_PATH), phase, details, &LOG_INITIALIZED);
}

/// Write a startup phase to an explicit path (for testing).
///
/// `initialized` tracks whether this is the first write (truncate) or subsequent (append).
pub fn write_phase_to(path: &Path, phase: &str, details: &str, initialized: &AtomicBool) {
    let timestamp = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

    let line = if details.is_empty() {
        format!("{} phase={}\n", timestamp, phase)
    } else {
        format!("{} phase={} {}\n", timestamp, phase, details)
    };

    let is_first = !initialized.swap(true, Ordering::SeqCst);

    let write_result = if is_first {
        // First call this process: truncate (fresh log per startup)
        fs::write(path, &line)
    } else {
        // Subsequent calls: append
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .and_then(|mut f| f.write_all(line.as_bytes()))
    };

    if let Err(err) = write_result {
        eprintln!("[startup-log] Failed to write: {}", err);
    }
}

/// Check if the previous rc-agent startup crashed (did not reach "phase=complete").
///
/// Returns `true` if the last non-empty line does NOT contain "phase=complete".
/// Returns `false` if the log file is missing (no evidence of crash).
pub fn detect_crash_recovery() -> bool {
    detect_crash_recovery_from(Path::new(LOG_PATH))
}

/// Testable version of detect_crash_recovery with explicit path.
pub fn detect_crash_recovery_from(path: &Path) -> bool {
    match fs::read_to_string(path) {
        Ok(content) => {
            let last_line = content
                .lines()
                .rev()
                .find(|line| !line.trim().is_empty());
            match last_line {
                Some(line) => !line.contains("phase=complete"),
                None => false, // empty file = not a crash
            }
        }
        Err(_) => false, // file missing = not a crash recovery
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;
    use tempfile::tempdir;

    #[test]
    fn test_write_phase() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("startup.log");
        let initialized = AtomicBool::new(false);

        write_phase_to(&path, "init", "", &initialized);

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("phase=init"), "Log must contain phase=init");
    }

    #[test]
    fn test_write_phase_with_details() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("startup.log");
        let initialized = AtomicBool::new(false);

        write_phase_to(&path, "config_loaded", "pod=3", &initialized);

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("phase=config_loaded"), "Log must contain phase");
        assert!(content.contains("pod=3"), "Log must contain details");
    }

    #[test]
    fn test_write_phase_appends() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("startup.log");
        let initialized = AtomicBool::new(false);

        write_phase_to(&path, "init", "", &initialized);
        write_phase_to(&path, "config_loaded", "", &initialized);

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("phase=init"), "Must contain first phase");
        assert!(
            content.contains("phase=config_loaded"),
            "Must contain second phase"
        );
        // Verify two lines
        let lines: Vec<&str> = content.lines().filter(|l| !l.is_empty()).collect();
        assert_eq!(lines.len(), 2, "Must have exactly 2 non-empty lines");
    }

    #[test]
    fn test_write_phase_truncates_on_first_call() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("startup.log");

        // Pre-populate with old data
        fs::write(&path, "old data from previous run\n").unwrap();

        // Simulate a fresh startup (new AtomicBool = false)
        let initialized = AtomicBool::new(false);
        write_phase_to(&path, "init", "", &initialized);

        let content = fs::read_to_string(&path).unwrap();
        assert!(
            !content.contains("old data"),
            "First write_phase must truncate old data"
        );
        assert!(content.contains("phase=init"));
    }

    #[test]
    fn test_detect_crash_incomplete() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("startup.log");
        fs::write(
            &path,
            "2026-03-15T00:00:00Z phase=init\n2026-03-15T00:00:01Z phase=firewall\n",
        )
        .unwrap();

        assert!(
            detect_crash_recovery_from(&path),
            "Should detect crash: last phase is firewall, not complete"
        );
    }

    #[test]
    fn test_detect_crash_complete() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("startup.log");
        fs::write(
            &path,
            "2026-03-15T00:00:00Z phase=init\n2026-03-15T00:00:05Z phase=complete\n",
        )
        .unwrap();

        assert!(
            !detect_crash_recovery_from(&path),
            "Should NOT detect crash: last phase is complete"
        );
    }

    #[test]
    fn test_detect_crash_no_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nonexistent.log");

        assert!(
            !detect_crash_recovery_from(&path),
            "No log file means no crash to recover from"
        );
    }
}
