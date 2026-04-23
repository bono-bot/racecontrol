// registry.rs — TOML-backed process declaration store.
//
// W1 stub. W2 populates `ManagedProcess` enum + loader + round-trip test.
// W5 adds bidirectional CI gate (every enum variant ⇄ TOML entry).

use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("registry scaffold only — loader lands in W2")]
    NotImplemented,
}

/// Placeholder container for the W2 entry set. Concrete shape intentionally
/// unspecified in W1 to avoid locking the schema before the migration survey.
#[derive(Debug, Default)]
pub struct Registry {
    _private: (),
}

impl Registry {
    /// W1 stub. W2 will return the parsed registry.
    pub fn load(_path: &Path) -> Result<Self, RegistryError> {
        Err(RegistryError::NotImplemented)
    }

    /// Mirror of `load` for tests that want to verify the unimplemented shape.
    pub fn load_default() -> Result<Self, RegistryError> {
        Err(RegistryError::NotImplemented)
    }
}
