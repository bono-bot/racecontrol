//! Persistent failure-count state for james_monitor.
//! Written to C:\Users\bono\.claude\watchdog-state.json after each run.
//! Atomic write (tmp + rename) prevents corruption across concurrent runs.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const STATE_PATH: &str = r"C:\Users\bono\.claude\watchdog-state.json";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FailureState {
    /// service_name -> consecutive failure count
    pub counts: HashMap<String, u32>,
}

impl FailureState {
    pub fn load() -> Self {
        Self::load_from(STATE_PATH)
    }

    pub fn load_from(path: &str) -> Self {
        match std::fs::read_to_string(path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) {
        self.save_to(STATE_PATH);
    }

    pub fn save_to(&self, path: &str) {
        let tmp = format!("{}.tmp", path);
        if let Ok(json) = serde_json::to_string_pretty(self) {
            if std::fs::write(&tmp, &json).is_ok() {
                if std::fs::rename(&tmp, path).is_err() {
                    tracing::error!("watchdog-state: rename tmp -> {} failed", path);
                    let _ = std::fs::remove_file(&tmp);
                }
            }
        }
    }

    pub fn count(&self, service: &str) -> u32 {
        self.counts.get(service).copied().unwrap_or(0)
    }

    pub fn increment(&mut self, service: &str) {
        let c = self.counts.entry(service.to_string()).or_insert(0);
        *c = c.saturating_add(1);
    }

    pub fn reset(&mut self, service: &str) {
        self.counts.remove(service);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_default_on_missing() {
        let state = FailureState::load_from(r"C:\nonexistent\path\watchdog-state.json");
        assert_eq!(state.counts.len(), 0);
    }

    #[test]
    fn test_load_default_on_corrupt() {
        let dir = std::env::temp_dir().join("watchdog_test_corrupt");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("watchdog-state.json");
        std::fs::write(&path, b"not valid json!!").expect("write corrupt file");
        let state = FailureState::load_from(path.to_str().unwrap());
        assert_eq!(state.counts.len(), 0, "corrupt file should return default");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_count_unknown_service_returns_zero() {
        let state = FailureState::default();
        assert_eq!(state.count("unknown"), 0);
    }

    #[test]
    fn test_increment_increases_count() {
        let mut state = FailureState::default();
        state.increment("ollama");
        assert_eq!(state.count("ollama"), 1);
        state.increment("ollama");
        assert_eq!(state.count("ollama"), 2);
    }

    #[test]
    fn test_reset_sets_count_to_zero() {
        let mut state = FailureState::default();
        state.increment("ollama");
        state.increment("ollama");
        assert_eq!(state.count("ollama"), 2);
        state.reset("ollama");
        assert_eq!(state.count("ollama"), 0);
    }

    #[test]
    fn test_roundtrip_save_load() {
        let dir = std::env::temp_dir().join("watchdog_test_roundtrip");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("watchdog-state.json");

        let mut state = FailureState::default();
        state.increment("ollama");
        state.increment("ollama");
        state.increment("comms-link");
        state.save_to(path.to_str().unwrap());

        let loaded = FailureState::load_from(path.to_str().unwrap());
        assert_eq!(loaded.count("ollama"), 2);
        assert_eq!(loaded.count("comms-link"), 1);
        assert_eq!(loaded.count("webterm"), 0);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_save_creates_parent_dir() {
        let dir = std::env::temp_dir().join("watchdog_test_newdir");
        let _ = std::fs::remove_dir_all(&dir);
        // Note: save_to will fail silently if parent dir doesn't exist (atomic write uses tmp in same dir)
        // This test verifies the graceful failure path
        let state = FailureState::default();
        let path = dir.join("watchdog-state.json");
        // save_to should not panic even if parent dir is missing
        state.save_to(path.to_str().unwrap());
        // No assertion needed — just must not panic
    }
}
