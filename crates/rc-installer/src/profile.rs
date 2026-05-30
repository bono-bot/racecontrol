//! Installer profile (spec §5 step 2, D-INSTALLER-1).
//!
//! The ONLY operator-facing decision is Server vs Pod. There is deliberately
//! no online/offline/stub/flavor variant (spec §4.1) — distribution mode is
//! not an operator choice.

/// The single installer profile choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    Server,
    Pod,
}

impl Profile {
    /// Stable lowercase label. Invariant (asserted by `tests/d_installer_1.rs`):
    /// MUST NOT contain `online`/`offline`/`flavor`/`stub`/`isolated`.
    pub fn as_str(self) -> &'static str {
        match self {
            Profile::Server => "server",
            Profile::Pod => "pod",
        }
    }
}
