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
    /// Updates `version` and persists the new state to disk.
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
