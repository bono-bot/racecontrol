// manager.rs — request_spawn / request_kill entry points (W3 ships real API).
//
// W1 stub: compiling skeleton with no behavior. W3 wires the typed surface
// around `rc_common::spawn_safe::spawn_safe` + `taskkill /IM`.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ManagerError {
    #[error("manager scaffold only — spawn/kill API lands in W3")]
    NotImplemented,
}

#[derive(Debug, Default)]
pub struct ProcessManager {
    _private: (),
}

impl ProcessManager {
    /// W1 stub. Returned error lets callers compile against the surface without
    /// depending on W3 behavior; migration in W4 will introduce real wiring.
    pub fn new() -> Self {
        Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manager_constructs_without_panic() {
        let _m = ProcessManager::new();
    }
}
