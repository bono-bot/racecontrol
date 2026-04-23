// process.rs — `ManagedProcess` enum (W2 of plan_process_manager_crate_20260423.md).
//
// Every process class the fleet legally spawns or kills is a variant here. Add
// a variant IFF the same commit also adds the matching entry to
// `process_registry.toml`; Wave 5 ships the bidirectional CI gate that rejects
// drift in either direction.

use serde::{Deserialize, Serialize};

/// Declarative identifier for a process class. The variant set is the single
/// source of truth consulted by `ProcessManager::request_spawn` /
/// `request_kill` (W3) and the W7 background scan loop.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize,
)]
pub enum ManagedProcess {
    Steam,
    AssettoCorsa2,
    RcAgent,
    RcSentry,
    ConspitLink,
    Kiosk,
    AcManager,
    Msedge,
    WindowsTerminal,
}

impl ManagedProcess {
    /// All variants in a stable order. W5 CI gate iterates this slice when
    /// checking registry parity so new variants automatically enter the check.
    pub fn all() -> &'static [ManagedProcess] {
        &[
            Self::Steam,
            Self::AssettoCorsa2,
            Self::RcAgent,
            Self::RcSentry,
            Self::ConspitLink,
            Self::Kiosk,
            Self::AcManager,
            Self::Msedge,
            Self::WindowsTerminal,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_slice_matches_variant_count() {
        assert_eq!(ManagedProcess::all().len(), 9);
    }

    #[test]
    fn all_slice_contains_unique_variants() {
        let mut seen = std::collections::HashSet::new();
        for v in ManagedProcess::all() {
            assert!(seen.insert(*v), "duplicate variant in ManagedProcess::all(): {:?}", v);
        }
    }
}
