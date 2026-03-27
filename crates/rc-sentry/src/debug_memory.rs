//! Pattern memory for crash diagnostics.
//!
//! Reads/writes debug-memory.json (shared with rc-agent's ai_debugger).
//! Stores crash patterns keyed by log content for instant replay of known fixes.
//! Atomic write (tmp + rename) prevents corruption.

use serde::{Deserialize, Serialize};

const LOG_TARGET: &str = "debug-memory";
const MEMORY_PATH: &str = r"C:\RacingPoint\debug-memory-sentry.json";
const MAX_INCIDENTS: usize = 50;

// ─── Types ───────────────────────────────────────────────────────────────────

/// A resolved crash incident stored in pattern memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrashIncident {
    /// Pattern key (derived from crash context)
    pub pattern_key: String,
    /// Fix type that resolved this crash
    pub fix_type: String,
    /// Detailed description of the fix
    pub detail: String,
    /// Number of times this pattern has been seen
    pub hit_count: u32,
    /// Last seen timestamp (ISO 8601)
    pub last_seen: String,
}

/// Pattern memory — learns from resolved crashes for instant replay.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DebugMemory {
    pub incidents: Vec<CrashIncident>,
}

impl DebugMemory {
    /// Load memory from disk. Returns empty memory if file doesn't exist or is corrupt.
    pub fn load() -> Self {
        Self::load_from(MEMORY_PATH)
    }

    /// Load from a specific path (for testing).
    pub fn load_from(path: &str) -> Self {
        match std::fs::read_to_string(path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Save memory to disk with atomic write (tmp + rename).
    pub fn save(&self) {
        self.save_to(MEMORY_PATH);
    }

    /// Save to a specific path (for testing).
    pub fn save_to(&self, path: &str) {
        let tmp = format!("{}.tmp", path);
        match serde_json::to_string_pretty(self) {
            Ok(json) => {
                if std::fs::write(&tmp, &json).is_ok() {
                    if std::fs::rename(&tmp, path).is_err() {
                        tracing::error!(target: LOG_TARGET, "failed to rename tmp -> {}", path);
                        let _ = std::fs::remove_file(&tmp);
                    }
                } else {
                    tracing::error!(target: LOG_TARGET, "failed to write tmp file {}", tmp);
                }
            }
            Err(e) => tracing::error!(target: LOG_TARGET, "failed to serialize: {}", e),
        }
    }

    /// Look up a known fix for a crash pattern.
    pub fn instant_fix(&self, pattern_key: &str) -> Option<&CrashIncident> {
        self.incidents.iter().find(|i| i.pattern_key == pattern_key)
    }

    /// Record a resolved crash, incrementing hit_count if pattern exists.
    pub fn record(&mut self, pattern_key: String, fix_type: String, detail: String) {
        let now = chrono_now();

        if let Some(existing) = self.incidents.iter_mut().find(|i| i.pattern_key == pattern_key) {
            existing.hit_count += 1;
            existing.last_seen = now;
            existing.fix_type = fix_type;
            existing.detail = detail;
        } else {
            self.incidents.push(CrashIncident {
                pattern_key,
                fix_type,
                detail,
                hit_count: 1,
                last_seen: now,
            });
        }

        // Prune to MAX_INCIDENTS, keeping highest hit_count
        if self.incidents.len() > MAX_INCIDENTS {
            self.incidents.sort_by(|a, b| b.hit_count.cmp(&a.hit_count));
            self.incidents.truncate(MAX_INCIDENTS);
        }
    }
}

// ─── Pattern Key Derivation ──────────────────────────────────────────────────

/// Derive a stable pattern key from crash context.
/// Extracts the most distinctive part of the crash (panic message > exit code > last phase).
pub fn derive_pattern_key(
    panic_message: Option<&str>,
    exit_code: Option<i32>,
    last_phase: Option<&str>,
) -> String {
    // Build pattern key from most specific to least specific signal.
    // Combine multiple signals when available for better differentiation.
    let mut parts = Vec::new();

    if let Some(panic) = panic_message {
        // Normalize: strip line numbers, addresses, specific values
        let normalized = panic
            .replace(|c: char| c.is_ascii_digit(), "#")
            .replace("##", "#");
        parts.push(format!("panic:{}", normalized.chars().take(100).collect::<String>()));
    }

    if let Some(code) = exit_code {
        parts.push(format!("exit:{}", code));
    }

    if let Some(phase) = last_phase {
        parts.push(format!("phase:{}", phase.chars().take(80).collect::<String>()));
    }

    if parts.is_empty() {
        "unknown".to_string()
    } else {
        parts.join("+")
    }
}

/// Get current timestamp as ISO 8601 string without chrono dependency.
fn chrono_now() -> String {
    // Use SystemTime for a simple ISO-ish timestamp
    let dur = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}s-since-epoch", dur.as_secs())
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_memory_returns_none() {
        let mem = DebugMemory::default();
        assert!(mem.instant_fix("anything").is_none());
    }

    #[test]
    fn record_and_lookup() {
        let mut mem = DebugMemory::default();
        mem.record("panic:test".to_string(), "zombie_kill".to_string(), "killed".to_string());
        let fix = mem.instant_fix("panic:test");
        assert!(fix.is_some());
        assert_eq!(fix.unwrap().fix_type, "zombie_kill");
        assert_eq!(fix.unwrap().hit_count, 1);
    }

    #[test]
    fn record_increments_hit_count() {
        let mut mem = DebugMemory::default();
        mem.record("panic:test".to_string(), "fix_a".to_string(), "detail".to_string());
        mem.record("panic:test".to_string(), "fix_b".to_string(), "detail2".to_string());
        let fix = mem.instant_fix("panic:test").unwrap();
        assert_eq!(fix.hit_count, 2);
        assert_eq!(fix.fix_type, "fix_b"); // updated to latest
    }

    #[test]
    fn prunes_to_max_incidents() {
        let mut mem = DebugMemory::default();
        for i in 0..60 {
            mem.record(format!("pattern:{}", i), "fix".to_string(), "d".to_string());
        }
        assert!(mem.incidents.len() <= MAX_INCIDENTS);
    }

    #[test]
    fn derive_key_prefers_panic() {
        let key = derive_pattern_key(Some("panicked at 'overflow'"), Some(101), Some("phase:bind"));
        assert!(key.starts_with("panic:"));
    }

    #[test]
    fn derive_key_combines_exit_code_and_phase() {
        let key = derive_pattern_key(None, Some(101), Some("phase:bind"));
        assert_eq!(key, "exit:101+phase:phase:bind");
    }

    #[test]
    fn derive_key_falls_back_to_phase() {
        let key = derive_pattern_key(None, None, Some("[STARTUP] phase: ws_connect"));
        assert!(key.starts_with("phase:"));
    }

    #[test]
    fn derive_key_unknown_fallback() {
        let key = derive_pattern_key(None, None, None);
        assert_eq!(key, "unknown");
    }

    #[test]
    fn save_load_roundtrip() {
        let dir = std::env::temp_dir();
        let path = dir.join("test-debug-memory.json");
        let path_str = path.to_str().unwrap();

        let mut mem = DebugMemory::default();
        mem.record("test:key".to_string(), "test_fix".to_string(), "test detail".to_string());
        mem.save_to(path_str);

        let loaded = DebugMemory::load_from(path_str);
        assert_eq!(loaded.incidents.len(), 1);
        assert_eq!(loaded.incidents[0].pattern_key, "test:key");
        assert_eq!(loaded.incidents[0].fix_type, "test_fix");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn load_returns_default_on_missing_file() {
        let mem = DebugMemory::load_from("/nonexistent/path.json");
        assert!(mem.incidents.is_empty());
    }
}
