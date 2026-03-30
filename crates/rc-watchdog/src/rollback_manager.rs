//! SW-03 / SW-04 / SW-13: Binary rollback with depth tracking and WhatsApp alert.
//!
//! When rc-agent fails validation or enters a restart loop, this module:
//! 1. Kills rc-agent.exe (if running)
//! 2. Renames rc-agent.exe → rc-agent-failed.exe
//! 3. Renames rc-agent-prev.exe → rc-agent.exe
//! 4. Tracks rollback depth (max 3 before giving up)
//! 5. Sends WhatsApp alert via Bono comms-link
//!
//! All file operations are synchronous — runs in the service poll loop.
//! On Windows, rename-while-running is allowed (Windows locks delete but not rename).

use std::path::Path;

use crate::bono_alert;

/// Maximum rollback depth before giving up and entering maintenance mode.
const MAX_ROLLBACK_DEPTH: u32 = 3;

/// State file for tracking rollback depth across restarts.
const ROLLBACK_STATE_FILE: &str = r"C:\RacingPoint\rollback-state.json";

/// Sentinel file written when maintenance mode is entered.
const MAINTENANCE_MODE_FILE: &str = r"C:\RacingPoint\MAINTENANCE_MODE";

/// Persistent rollback state — survives process restarts.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RollbackState {
    pub depth: u32,
    pub last_rollback_reason: String,
    pub last_rollback_time: String,
    /// Track which binaries we've rolled back from
    pub rolled_back_hashes: Vec<String>,
}

impl Default for RollbackState {
    fn default() -> Self {
        Self {
            depth: 0,
            last_rollback_reason: String::new(),
            last_rollback_time: String::new(),
            rolled_back_hashes: Vec::new(),
        }
    }
}

impl RollbackState {
    /// Load state from disk, returning default if missing or corrupt.
    pub fn load() -> Self {
        Self::load_from(ROLLBACK_STATE_FILE)
    }

    pub fn load_from(path: &str) -> Self {
        match std::fs::read_to_string(path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Save state to disk (atomic write via tmp + rename).
    pub fn save(&self) {
        self.save_to(ROLLBACK_STATE_FILE);
    }

    pub fn save_to(&self, path: &str) {
        let tmp = format!("{}.tmp", path);
        if let Ok(json) = serde_json::to_string_pretty(self) {
            if std::fs::write(&tmp, &json).is_ok() {
                if std::fs::rename(&tmp, path).is_err() {
                    tracing::error!("rollback-state: rename tmp -> {} failed", path);
                    let _ = std::fs::remove_file(&tmp);
                }
            }
        }
    }

    /// Reset depth to 0 (called when rc-agent starts successfully after rollback).
    pub fn reset(&mut self) {
        self.depth = 0;
        self.rolled_back_hashes.clear();
    }

    /// Check if we've exceeded max rollback depth.
    pub fn exhausted(&self) -> bool {
        self.depth >= MAX_ROLLBACK_DEPTH
    }
}

/// Outcome of a rollback attempt.
#[derive(Debug)]
pub enum RollbackOutcome {
    /// Successfully rolled back — rc-agent-prev.exe is now rc-agent.exe
    Success { depth: u32 },
    /// No previous binary available to roll back to
    NoPreviousBinary,
    /// Rollback depth exceeded — entering maintenance mode
    DepthExhausted { depth: u32 },
    /// File operation failed
    FileError(String),
}

/// Attempt to roll back rc-agent.exe to rc-agent-prev.exe.
///
/// Steps:
/// 1. Check rollback depth (SW-04)
/// 2. Kill rc-agent.exe process
/// 3. Rename current → failed, prev → current
/// 4. Increment depth and save state
/// 5. Alert Bono via WhatsApp (SW-13)
pub fn perform_rollback(
    install_dir: &Path,
    reason: &str,
    current_hash: &str,
) -> RollbackOutcome {
    let agent_exe = install_dir.join("rc-agent.exe");
    let prev_exe = install_dir.join("rc-agent-prev.exe");
    let failed_exe = install_dir.join("rc-agent-failed.exe");

    // Load current state
    let mut state = RollbackState::load();

    // SW-04: Check depth
    if state.exhausted() {
        tracing::error!(
            "Rollback depth exhausted ({}/{}) — entering MAINTENANCE_MODE",
            state.depth, MAX_ROLLBACK_DEPTH
        );
        enter_maintenance_mode(reason);
        let alert_msg = format!(
            "[CRITICAL] Pod rollback depth exhausted ({}/{}) — MAINTENANCE_MODE entered. Reason: {}",
            state.depth, MAX_ROLLBACK_DEPTH, reason
        );
        let _ = bono_alert::alert_bono(&alert_msg);
        return RollbackOutcome::DepthExhausted { depth: state.depth };
    }

    // Check previous binary exists
    if !prev_exe.is_file() {
        tracing::error!("No rc-agent-prev.exe found — cannot roll back");
        let alert_msg = format!(
            "[CRITICAL] Rollback failed — no rc-agent-prev.exe. Reason: {}",
            reason
        );
        let _ = bono_alert::alert_bono(&alert_msg);
        return RollbackOutcome::NoPreviousBinary;
    }

    // Kill rc-agent if running
    if let Err(e) = kill_rc_agent() {
        tracing::warn!("Failed to kill rc-agent before rollback: {}", e);
        // Continue anyway — file might not be locked
    }

    // Wait briefly for process to die
    std::thread::sleep(std::time::Duration::from_secs(2));

    // Step 1: Remove old failed binary if present
    if failed_exe.is_file() {
        if let Err(e) = std::fs::remove_file(&failed_exe) {
            tracing::warn!("Could not remove old rc-agent-failed.exe: {}", e);
        }
    }

    // Step 2: Rename current → failed (Windows allows rename of running exe)
    if agent_exe.is_file() {
        if let Err(e) = std::fs::rename(&agent_exe, &failed_exe) {
            let msg = format!("Failed to rename rc-agent.exe → rc-agent-failed.exe: {}", e);
            tracing::error!("{}", msg);
            return RollbackOutcome::FileError(msg);
        }
        tracing::info!("Renamed rc-agent.exe → rc-agent-failed.exe");
    }

    // Step 3: Rename prev → current
    if let Err(e) = std::fs::rename(&prev_exe, &agent_exe) {
        let msg = format!("Failed to rename rc-agent-prev.exe → rc-agent.exe: {}", e);
        tracing::error!("{}", msg);
        // Try to recover: rename failed back to agent
        if failed_exe.is_file() {
            let _ = std::fs::rename(&failed_exe, &agent_exe);
        }
        return RollbackOutcome::FileError(msg);
    }
    tracing::info!("Renamed rc-agent-prev.exe → rc-agent.exe (rollback complete)");

    // Update state
    state.depth = state.depth.saturating_add(1);
    state.last_rollback_reason = reason.to_string();
    state.last_rollback_time = chrono::Utc::now().to_rfc3339();
    if !current_hash.is_empty() {
        state.rolled_back_hashes.push(current_hash.to_string());
    }
    state.save();

    // SW-13: WhatsApp alert
    let alert_msg = format!(
        "[WARN] rc-agent rolled back (depth {}/{}). Reason: {}",
        state.depth, MAX_ROLLBACK_DEPTH, reason
    );
    let _ = bono_alert::alert_bono(&alert_msg);

    tracing::info!(
        "Rollback complete: depth={}/{}, reason={}",
        state.depth, MAX_ROLLBACK_DEPTH, reason
    );

    RollbackOutcome::Success { depth: state.depth }
}

/// Kill rc-agent.exe via taskkill.
fn kill_rc_agent() -> anyhow::Result<()> {
    let mut cmd = std::process::Command::new("taskkill");
    cmd.args(["/F", "/IM", "rc-agent.exe"]);

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }

    let output = cmd.output()
        .map_err(|e| anyhow::anyhow!("taskkill spawn failed: {}", e))?;

    if output.status.success() {
        tracing::info!("Killed rc-agent.exe before rollback");
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::debug!("taskkill returned non-zero (may not be running): {}", stderr.trim());
    }
    Ok(())
}

/// Enter maintenance mode by writing the sentinel file.
fn enter_maintenance_mode(reason: &str) {
    let content = format!(
        "MAINTENANCE_MODE entered by rc-watchdog\nReason: {}\nTime: {}\n",
        reason,
        chrono::Utc::now().to_rfc3339()
    );
    if let Err(e) = std::fs::write(MAINTENANCE_MODE_FILE, content) {
        tracing::error!("Failed to write MAINTENANCE_MODE sentinel: {}", e);
    }
}

/// Check if MAINTENANCE_MODE sentinel exists.
pub fn is_maintenance_mode() -> bool {
    Path::new(MAINTENANCE_MODE_FILE).is_file()
}

/// SW-07: Auto-clear MAINTENANCE_MODE if it has been active for too long.
/// Returns true if it was cleared.
pub fn auto_clear_maintenance_mode(max_age_secs: u64) -> bool {
    let path = Path::new(MAINTENANCE_MODE_FILE);
    if !path.is_file() {
        return false;
    }

    let metadata = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(_) => return false,
    };

    let modified = match metadata.modified() {
        Ok(t) => t,
        Err(_) => return false,
    };

    let age = match modified.elapsed() {
        Ok(d) => d,
        Err(_) => return false,
    };

    if age.as_secs() >= max_age_secs {
        tracing::info!(
            "Auto-clearing MAINTENANCE_MODE (age: {}s, max: {}s)",
            age.as_secs(),
            max_age_secs
        );
        if let Err(e) = std::fs::remove_file(path) {
            tracing::error!("Failed to remove MAINTENANCE_MODE: {}", e);
            return false;
        }
        // Also reset rollback state
        let mut state = RollbackState::load();
        state.reset();
        state.save();
        return true;
    }

    false
}

/// Reset rollback state on confirmed healthy agent.
/// Call this when health poll succeeds after a rollback.
pub fn confirm_healthy() {
    let mut state = RollbackState::load();
    if state.depth > 0 {
        tracing::info!(
            "Agent confirmed healthy — resetting rollback depth from {} to 0",
            state.depth
        );
        state.reset();
        state.save();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rollback_state_default() {
        let state = RollbackState::default();
        assert_eq!(state.depth, 0);
        assert!(!state.exhausted());
    }

    #[test]
    fn test_rollback_state_exhausted_at_max() {
        let state = RollbackState {
            depth: MAX_ROLLBACK_DEPTH,
            ..Default::default()
        };
        assert!(state.exhausted());
    }

    #[test]
    fn test_rollback_state_not_exhausted_below_max() {
        let state = RollbackState {
            depth: MAX_ROLLBACK_DEPTH - 1,
            ..Default::default()
        };
        assert!(!state.exhausted());
    }

    #[test]
    fn test_rollback_state_reset() {
        let mut state = RollbackState {
            depth: 2,
            rolled_back_hashes: vec!["abc".to_string()],
            ..Default::default()
        };
        state.reset();
        assert_eq!(state.depth, 0);
        assert!(state.rolled_back_hashes.is_empty());
    }

    #[test]
    fn test_rollback_state_roundtrip() {
        let dir = std::env::temp_dir().join("rc_watchdog_test_rollback");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("rollback-state.json");

        let state = RollbackState {
            depth: 2,
            last_rollback_reason: "hash mismatch".to_string(),
            last_rollback_time: "2026-03-31T12:00:00Z".to_string(),
            rolled_back_hashes: vec!["abc123".to_string(), "def456".to_string()],
        };
        state.save_to(path.to_str().expect("valid path"));

        let loaded = RollbackState::load_from(path.to_str().expect("valid path"));
        assert_eq!(loaded.depth, 2);
        assert_eq!(loaded.last_rollback_reason, "hash mismatch");
        assert_eq!(loaded.rolled_back_hashes.len(), 2);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_rollback_state_load_missing_file() {
        let state = RollbackState::load_from(r"C:\nonexistent\rollback-state.json");
        assert_eq!(state.depth, 0);
    }

    #[test]
    fn test_rollback_state_load_corrupt_file() {
        let dir = std::env::temp_dir().join("rc_watchdog_test_rollback_corrupt");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("rollback-state.json");
        std::fs::write(&path, "not json!!!").ok();
        let state = RollbackState::load_from(path.to_str().expect("valid path"));
        assert_eq!(state.depth, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
