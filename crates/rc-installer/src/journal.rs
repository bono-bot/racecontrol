//! Crash-safe local install journal (INC-2H, MMA G7).
//!
//! Lets a re-run skip already-completed (especially destructive) steps instead of
//! restarting from scratch. std-only; the atomic write reuses the `.partial` + rename
//! pattern from `artifact_stager.rs`. Pure and tempdir-testable.

use crate::error::{InstallError, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Ordered install phases. Presence in `completed` means the step finished durably.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InstallPhase {
    Redeemed,
    ManifestVerified,
    ArtifactsStaged,
    BackedUp,
    Installed,
    Provisioned,
    Done,
}

/// On-disk resume journal.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct InstallJournal {
    pub schema: u32,
    #[serde(default)]
    pub venue_id: Option<String>,
    #[serde(default)]
    pub completed: Vec<InstallPhase>,
    #[serde(default)]
    pub staged_sha256: Vec<String>,
}

fn partial_path(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(".partial");
    PathBuf::from(s)
}

impl InstallJournal {
    /// A fresh schema-1 journal.
    pub fn new() -> Self {
        Self {
            schema: 1,
            ..Default::default()
        }
    }

    /// Load the journal at `path`; a missing file yields a fresh journal; a corrupt file
    /// is a hard error (never silently discarded — resuming on garbage is dangerous).
    pub fn load_or_new(path: &Path) -> Result<Self> {
        match std::fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| {
                InstallError::Journal(format!("corrupt journal at {}: {e}", path.display()))
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::new()),
            Err(e) => Err(InstallError::Io(e.to_string())),
        }
    }

    /// True iff `phase` is already recorded complete.
    pub fn has(&self, phase: &InstallPhase) -> bool {
        self.completed.contains(phase)
    }

    /// Record a completed phase (idempotent) and atomically persist.
    pub fn record(&mut self, path: &Path, phase: InstallPhase) -> Result<()> {
        if !self.completed.contains(&phase) {
            self.completed.push(phase);
            self.save_atomic(path)?;
        }
        Ok(())
    }

    /// Record a staged-artifact sha (idempotent) and atomically persist.
    pub fn record_staged_sha(&mut self, path: &Path, sha256: &str) -> Result<()> {
        if !self.staged_sha256.iter().any(|s| s == sha256) {
            self.staged_sha256.push(sha256.to_string());
            self.save_atomic(path)?;
        }
        Ok(())
    }

    fn save_atomic(&self, path: &Path) -> Result<()> {
        let bytes =
            serde_json::to_vec_pretty(self).map_err(|e| InstallError::Journal(e.to_string()))?;
        let partial = partial_path(path);
        std::fs::write(&partial, &bytes)?;
        std::fs::rename(&partial, path)?;
        Ok(())
    }
}
