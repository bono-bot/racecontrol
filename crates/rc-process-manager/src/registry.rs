// registry.rs — TOML-backed process declaration store (Wave 2).
//
// Loads `process_registry.toml`, validates schema_version, rejects duplicate
// entries, and returns a typed `Registry` keyed by `ManagedProcess`. Wave 3
// reads the registry to enforce `max_instances`; Wave 5 CI gate iterates
// `ManagedProcess::all()` against `Registry::entries` to detect drift; Wave 8
// daily-reset reads `survives_daily_reset`.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde::Deserialize;
use thiserror::Error;

use crate::process::ManagedProcess;

/// Current registry schema version. Bump when the on-disk shape of
/// `Entry` / `RespawnPolicy` changes in a way that breaks older TOML files.
pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("registry file read error: {0}")]
    Io(#[from] std::io::Error),

    #[error("registry TOML parse error: {0}")]
    Parse(#[from] toml::de::Error),

    #[error("schema_version mismatch: expected {expected}, got {found}")]
    SchemaVersion { expected: u32, found: u32 },

    #[error("duplicate entry for {0:?}")]
    DuplicateEntry(ManagedProcess),

    #[error("no registry entry for {0:?}")]
    MissingEntry(ManagedProcess),
}

/// Respawn enforcement mode consulted by the Wave 7 scan loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RespawnPolicy {
    /// At most `max_instances`; scan loop auto-kills extras.
    Singleton,
    /// Scan loop logs violations; does not auto-kill.
    Managed,
    /// Manager treats the process as externally owned; never auto-spawn or
    /// auto-kill. Used for rc-agent (owned by rc-sentry) and rc-sentry (owned
    /// by the Windows service rc-watchdog).
    OwnerManaged,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Entry {
    pub process: ManagedProcess,
    pub owner_crate: String,
    pub max_instances: u32,
    pub respawn_policy: RespawnPolicy,
    pub survives_daily_reset: bool,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OnDiskRegistry {
    schema_version: u32,
    #[serde(rename = "entry", default)]
    entries: Vec<Entry>,
}

/// In-memory registry. Keyed by `ManagedProcess` for O(1) lookup from the
/// manager and scan loop.
#[derive(Debug, Default)]
pub struct Registry {
    schema_version: u32,
    entries: HashMap<ManagedProcess, Entry>,
}

impl Registry {
    /// Parse a TOML document string into a validated `Registry`. Separated
    /// from `load_from_path` so tests can exercise the full loader without
    /// touching the filesystem.
    pub fn from_str(content: &str) -> Result<Self, RegistryError> {
        let on_disk: OnDiskRegistry = toml::from_str(content)?;
        if on_disk.schema_version != SCHEMA_VERSION {
            return Err(RegistryError::SchemaVersion {
                expected: SCHEMA_VERSION,
                found: on_disk.schema_version,
            });
        }
        let mut entries = HashMap::with_capacity(on_disk.entries.len());
        for entry in on_disk.entries {
            let process = entry.process;
            if entries.insert(process, entry).is_some() {
                return Err(RegistryError::DuplicateEntry(process));
            }
        }
        Ok(Self {
            schema_version: on_disk.schema_version,
            entries,
        })
    }

    /// Load the registry from a TOML file on disk.
    pub fn load_from_path(path: &Path) -> Result<Self, RegistryError> {
        let content = fs::read_to_string(path)?;
        Self::from_str(&content)
    }

    pub fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Look up the registry entry for a managed process variant.
    pub fn entry(&self, process: ManagedProcess) -> Result<&Entry, RegistryError> {
        self.entries
            .get(&process)
            .ok_or(RegistryError::MissingEntry(process))
    }

    /// Iterate all declared entries (order not guaranteed).
    pub fn entries(&self) -> impl Iterator<Item = &Entry> {
        self.entries.values()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal well-formed registry covering one variant — used by the
    /// missing-entry test to keep the fixture small.
    const SINGLE_ENTRY: &str = r#"
schema_version = 1

[[entry]]
process = "Steam"
owner_crate = "steam_launcher"
max_instances = 1
respawn_policy = "singleton"
survives_daily_reset = true
"#;

    const DUPLICATE_ENTRY: &str = r#"
schema_version = 1

[[entry]]
process = "Steam"
owner_crate = "a"
max_instances = 1
respawn_policy = "singleton"
survives_daily_reset = true

[[entry]]
process = "Steam"
owner_crate = "b"
max_instances = 1
respawn_policy = "singleton"
survives_daily_reset = true
"#;

    const WRONG_SCHEMA: &str = r#"
schema_version = 999

[[entry]]
process = "Steam"
owner_crate = "steam_launcher"
max_instances = 1
respawn_policy = "singleton"
survives_daily_reset = true
"#;

    #[test]
    fn loader_parses_shipped_registry_toml() {
        // The real registry lives next to Cargo.toml. Path resolution uses
        // CARGO_MANIFEST_DIR so the test is CWD-independent.
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let path = std::path::Path::new(manifest_dir).join("process_registry.toml");
        let reg = Registry::load_from_path(&path)
            .expect("shipped process_registry.toml must load cleanly");
        assert_eq!(reg.schema_version(), SCHEMA_VERSION);
        // Every enum variant must have a registry entry — this is the W5 CI
        // gate's condition, checked here at unit-test time as a smoke test.
        for variant in ManagedProcess::all() {
            reg.entry(*variant)
                .unwrap_or_else(|e| panic!("missing entry for {:?}: {}", variant, e));
        }
        assert_eq!(reg.len(), ManagedProcess::all().len());
    }

    #[test]
    fn missing_entry_returns_error() {
        let reg = Registry::from_str(SINGLE_ENTRY).expect("single-entry fixture must parse");
        // Steam is present.
        assert!(reg.entry(ManagedProcess::Steam).is_ok());
        // AssettoCorsa2 is NOT present in the fixture.
        match reg.entry(ManagedProcess::AssettoCorsa2) {
            Err(RegistryError::MissingEntry(ManagedProcess::AssettoCorsa2)) => {}
            other => panic!("expected MissingEntry(AssettoCorsa2), got {:?}", other),
        }
    }

    #[test]
    fn duplicate_entry_is_rejected() {
        match Registry::from_str(DUPLICATE_ENTRY) {
            Err(RegistryError::DuplicateEntry(ManagedProcess::Steam)) => {}
            other => panic!("expected DuplicateEntry(Steam), got {:?}", other),
        }
    }

    #[test]
    fn schema_version_mismatch_is_rejected() {
        match Registry::from_str(WRONG_SCHEMA) {
            Err(RegistryError::SchemaVersion { expected: 1, found: 999 }) => {}
            other => panic!("expected SchemaVersion 1/999, got {:?}", other),
        }
    }

    #[test]
    fn entry_fields_round_trip_through_toml() {
        let reg = Registry::from_str(SINGLE_ENTRY).unwrap();
        let steam = reg.entry(ManagedProcess::Steam).unwrap();
        assert_eq!(steam.owner_crate, "steam_launcher");
        assert_eq!(steam.max_instances, 1);
        assert_eq!(steam.respawn_policy, RespawnPolicy::Singleton);
        assert!(steam.survives_daily_reset);
        assert!(steam.notes.is_none());
    }
}
