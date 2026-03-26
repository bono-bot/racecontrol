/// v22.0 Phase 178: In-memory feature flag cache with disk persistence.
///
/// Flags propagate from server over WebSocket (FlagSync / KillSwitch messages),
/// persist to C:\RacingPoint\flags-cache.json for offline operation, and are
/// loaded from disk on startup so pods operate correctly before the first WS sync.
///
/// Kill switches are stored separately and checked FIRST in flag_enabled() — any
/// active kill switch named `kill_<feature>` will disable that feature regardless
/// of the flags map.
use std::collections::HashMap;

use rc_common::types::{FlagSyncPayload, KillSwitchPayload};
use serde::{Deserialize, Serialize};

const LOG_TARGET: &str = "flags";
const CACHE_PATH: &str = r"C:\RacingPoint\flags-cache.json";
const CACHE_PATH_TMP: &str = r"C:\RacingPoint\flags-cache.json.tmp";

/// Serialization format for the on-disk cache.
#[derive(Debug, Serialize, Deserialize)]
struct FlagsCacheFile {
    flags: HashMap<String, bool>,
    kill_switches: HashMap<String, bool>,
    version: u64,
}

/// In-memory feature flag store for rc-agent.
///
/// All public methods are synchronous — callers hold an `Arc<RwLock<FeatureFlags>>`
/// and call `.write().await` / `.read().await` before calling these methods.
#[derive(Debug, Default)]
pub struct FeatureFlags {
    /// Regular feature flags (non-kill-switch).
    flags: HashMap<String, bool>,
    /// Kill switch flags (`kill_*` prefix) stored separately for priority evaluation.
    kill_switches: HashMap<String, bool>,
    /// Monotonically increasing version from server.
    version: u64,
}

impl FeatureFlags {
    /// Create a new empty FeatureFlags with version 0.
    pub fn new() -> Self {
        Self::default()
    }

    /// Load flags from the on-disk cache at `C:\RacingPoint\flags-cache.json`.
    ///
    /// Returns `FeatureFlags::new()` on any error (file missing, parse failure, I/O error).
    /// Missing cache is not an error condition — a fresh pod has no cache yet.
    pub fn load_from_cache() -> Self {
        let data = match std::fs::read_to_string(CACHE_PATH) {
            Ok(d) => d,
            Err(e) => {
                // File not found is expected on first boot — only warn for other errors.
                if e.kind() == std::io::ErrorKind::NotFound {
                    tracing::info!(target: LOG_TARGET, "No flags cache found at {} — starting with defaults (all-true)", CACHE_PATH);
                } else {
                    tracing::warn!(target: LOG_TARGET, "Failed to read flags cache at {}: {} — starting with defaults", CACHE_PATH, e);
                }
                return Self::new();
            }
        };

        match serde_json::from_str::<FlagsCacheFile>(&data) {
            Ok(cache) => {
                tracing::info!(
                    target: LOG_TARGET,
                    "Loaded flags cache: {} flags, {} kill switches, version={}",
                    cache.flags.len(),
                    cache.kill_switches.len(),
                    cache.version
                );
                Self {
                    flags: cache.flags,
                    kill_switches: cache.kill_switches,
                    version: cache.version,
                }
            }
            Err(e) => {
                tracing::warn!(target: LOG_TARGET, "Failed to parse flags cache at {}: {} — starting with defaults", CACHE_PATH, e);
                Self::new()
            }
        }
    }

    /// Check whether a named feature is enabled.
    ///
    /// Evaluation order:
    /// 1. If `kill_<name>` is in kill_switches and is active (`true`), return `false`.
    /// 2. Look up `name` in the flags map.
    /// 3. If not found, default to `true` (fresh pod = features enabled).
    pub fn flag_enabled(&self, name: &str) -> bool {
        // Kill switch check — any active `kill_<name>` disables the feature immediately.
        let kill_key = format!("kill_{}", name);
        if self.kill_switches.get(&kill_key).copied().unwrap_or(false) {
            return false;
        }
        // Regular flag lookup — default true for unknown flags.
        self.flags.get(name).copied().unwrap_or(true)
    }

    /// Apply a FlagSync payload received from the server.
    ///
    /// `kill_*` prefixed keys are routed to `kill_switches`; all others go to `flags`.
    /// Updates `version`, persists the new state to disk, and writes sentry-flags.json
    /// so rc-sentry can consume current flags on its next watchdog poll.
    pub fn apply_sync(&mut self, payload: &FlagSyncPayload) {
        for (key, value) in &payload.flags {
            if key.starts_with("kill_") {
                self.kill_switches.insert(key.clone(), *value);
            } else {
                self.flags.insert(key.clone(), *value);
            }
        }
        self.version = payload.version;
        self.persist_to_disk();
        self.write_sentry_flags();
    }

    /// Write current flags to sentry-flags.json for rc-sentry consumption.
    ///
    /// Called after every FlagSync. rc-sentry reads this file on its 5s watchdog cycle
    /// to gate watchdog behavior (e.g., suppressing restart via kill_watchdog_restart).
    /// Uses atomic tmp-file + rename to avoid partial reads by rc-sentry.
    pub fn write_sentry_flags(&self) {
        let path = std::path::Path::new(r"C:\RacingPoint\sentry-flags.json");
        let tmp_path = path.with_extension("json.tmp");
        let data = serde_json::json!({
            "flags": &self.flags,
            "kill_switches": &self.kill_switches,
            "version": self.version,
        });
        match serde_json::to_string_pretty(&data) {
            Ok(json) => {
                if let Err(e) = std::fs::write(&tmp_path, &json)
                    .and_then(|_| std::fs::rename(&tmp_path, path))
                {
                    tracing::warn!(target: LOG_TARGET, "Failed to write sentry-flags.json: {}", e);
                }
            }
            Err(e) => tracing::warn!(target: LOG_TARGET, "Failed to serialize sentry flags: {}", e),
        }
    }

    /// Apply a KillSwitch payload received from the server.
    ///
    /// Stores the kill switch state and persists to disk.
    /// Logs at warn level — kill switches are exceptional events.
    pub fn apply_kill_switch(&mut self, payload: &KillSwitchPayload) {
        tracing::warn!(
            target: LOG_TARGET,
            "Kill switch: {} = {} (reason: {:?})",
            payload.flag_name,
            payload.active,
            payload.reason
        );
        self.kill_switches.insert(payload.flag_name.clone(), payload.active);
        self.persist_to_disk();
    }

    /// Return the current cache version (sent to server on reconnect via FlagCacheSync).
    pub fn cached_version(&self) -> u64 {
        self.version
    }

    /// Fetch feature flags from the server via HTTP GET and apply them to shared state.
    ///
    /// Used by `spawn_periodic_refetch` for self-healing when WS FlagSync is unavailable.
    /// Fetches from `{base_url}/flags`, parses the response as `Vec<{name, enabled, ...}>`,
    /// builds a `FlagSyncPayload`, and applies via `apply_sync`.
    #[cfg(feature = "http-client")]
    pub async fn fetch_from_server(
        client: &reqwest::Client,
        base_url: &str,
        flags: &std::sync::Arc<tokio::sync::RwLock<FeatureFlags>>,
    ) -> Result<(), String> {
        let url = format!("{}/flags", base_url);
        let resp = client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("HTTP GET {} failed: {}", url, e))?;
        if !resp.status().is_success() {
            return Err(format!("HTTP GET {} returned {}", url, resp.status()));
        }
        let body = resp
            .text()
            .await
            .map_err(|e| format!("read body: {}", e))?;
        // Server returns Vec<FeatureFlagRow> with {name, enabled, ...}
        let rows: Vec<serde_json::Value> =
            serde_json::from_str(&body).map_err(|e| format!("parse flags JSON: {}", e))?;
        let mut flag_map = std::collections::HashMap::new();
        for row in &rows {
            if let (Some(name), Some(enabled)) = (row["name"].as_str(), row["enabled"].as_bool()) {
                flag_map.insert(name.to_string(), enabled);
            }
        }
        let payload = FlagSyncPayload {
            flags: flag_map,
            version: rows
                .first()
                .and_then(|r| r["version"].as_u64())
                .unwrap_or(0),
        };
        let mut ff = flags.write().await;
        ff.apply_sync(&payload);
        tracing::info!(target: LOG_TARGET, count = rows.len(), "feature flags refreshed via HTTP");
        Ok(())
    }

    /// Atomically write flags to `C:\RacingPoint\flags-cache.json`.
    ///
    /// Uses a tmp-file + rename for atomicity so a crash mid-write does not corrupt
    /// the cache. Logs a warning on any I/O error but does NOT panic.
    fn persist_to_disk(&self) {
        let cache = FlagsCacheFile {
            flags: self.flags.clone(),
            kill_switches: self.kill_switches.clone(),
            version: self.version,
        };

        let json = match serde_json::to_string(&cache) {
            Ok(j) => j,
            Err(e) => {
                tracing::warn!(target: LOG_TARGET, "Failed to serialize flags cache: {}", e);
                return;
            }
        };

        if let Err(e) = std::fs::write(CACHE_PATH_TMP, &json) {
            tracing::warn!(target: LOG_TARGET, "Failed to write flags cache tmp file {}: {}", CACHE_PATH_TMP, e);
            return;
        }

        if let Err(e) = std::fs::rename(CACHE_PATH_TMP, CACHE_PATH) {
            tracing::warn!(target: LOG_TARGET, "Failed to rename flags cache tmp -> final {}: {}", CACHE_PATH, e);
        }
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    #[tokio::test]
    async fn fetch_from_server_applies_flags_from_http_response() {
        // Start a mock HTTP server that returns a flags JSON array
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            // Read the full HTTP request before responding
            let mut buf = vec![0u8; 4096];
            let _ = stream.read(&mut buf).await;
            let body = r#"[{"name":"test_flag","enabled":true},{"name":"other_flag","enabled":false}]"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(response.as_bytes()).await.unwrap();
            stream.shutdown().await.unwrap();
        });

        let flags = Arc::new(RwLock::new(FeatureFlags::new()));
        let client = reqwest::Client::new();
        let base_url = format!("http://127.0.0.1:{}/api/v1", addr.port());

        let result = FeatureFlags::fetch_from_server(&client, &base_url, &flags).await;
        assert!(result.is_ok(), "fetch_from_server should succeed, got: {:?}", result);

        let ff = flags.read().await;
        assert!(ff.flag_enabled("test_flag"), "test_flag should be enabled");
        assert!(!ff.flag_enabled("other_flag"), "other_flag should be disabled");
    }

    #[tokio::test]
    async fn fetch_from_server_returns_err_on_unreachable_url() {
        let flags = Arc::new(RwLock::new(FeatureFlags::new()));
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(100))
            .build()
            .unwrap();
        // Use a port that's almost certainly not listening
        let result =
            FeatureFlags::fetch_from_server(&client, "http://127.0.0.1:1/api/v1", &flags).await;
        assert!(result.is_err(), "fetch_from_server should return Err for unreachable URL");
    }

    #[tokio::test]
    async fn apply_http_response_updates_flag_enabled() {
        let flags = Arc::new(RwLock::new(FeatureFlags::new()));
        // Before: test_flag defaults to true (unknown flags default true)
        assert!(flags.read().await.flag_enabled("test_flag"));

        // Apply a sync payload that sets test_flag = false
        {
            let mut ff = flags.write().await;
            let payload = FlagSyncPayload {
                flags: [("test_flag".to_string(), false)].into_iter().collect(),
                version: 1,
            };
            ff.apply_sync(&payload);
        }

        // After: test_flag should be false
        assert!(!flags.read().await.flag_enabled("test_flag"));
    }
}
