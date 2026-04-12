pub mod assetto_corsa;
pub mod assetto_corsa_evo;
pub mod f1_25;
pub mod iracing;
pub mod lmu;

use anyhow::Result;
use rc_common::types::{AcStatus, SimType, TelemetryFrame, SessionInfo, LapData};

/// Session configuration read back from the running sim.
/// Used by launch_verifier Stage 5 to compare against what the kiosk requested.
#[derive(Debug, Clone, Default)]
pub struct SessionConfig {
    /// Number of cars in session (including player). None if not readable.
    pub num_cars: Option<u32>,
    /// Session type string (e.g. "practice", "race", "hotlap"). None if not readable.
    pub session_type: Option<String>,
    /// Track configuration/layout variant (e.g. "gp", "drift"). None if not readable.
    pub track_config: Option<String>,
    /// Car model identifier. None if not readable.
    pub car_model: Option<String>,
    /// Track name. None if not readable.
    pub track_name: Option<String>,
    /// AI difficulty level (0-100 for AC). None if not readable.
    pub ai_level: Option<u32>,
}

/// A single field mismatch between expected and actual session config.
#[derive(Debug, Clone, PartialEq)]
pub struct ConfigMismatch {
    pub field: &'static str,
    pub expected: String,
    pub actual: String,
}

/// Trait that all sim adapters must implement
pub trait SimAdapter: Send + Sync {
    /// The sim type this adapter handles
    fn sim_type(&self) -> SimType;

    /// Connect to the sim's telemetry source
    fn connect(&mut self) -> Result<()>;

    /// Check if connected
    fn is_connected(&self) -> bool;

    /// Read the latest telemetry frame (non-blocking)
    fn read_telemetry(&mut self) -> Result<Option<TelemetryFrame>>;

    /// Check if a new lap was completed since last call
    fn poll_lap_completed(&mut self) -> Result<Option<LapData>>;

    /// Get current session info
    #[allow(dead_code)]
    fn session_info(&self) -> Result<Option<SessionInfo>>;

    /// Disconnect from telemetry source
    fn disconnect(&mut self);

    /// Car's max RPM (for RPM bar scaling). Default 8000 if unknown.
    fn max_rpm(&self) -> u32 { 8000 }

    /// Read the sim's current AC_STATUS from shared memory. Only meaningful for AC.
    fn read_ac_status(&self) -> Option<AcStatus> { None }

    /// Read current assist state: (abs_level, tc_level, auto_shifter). Only meaningful for AC.
    fn read_assist_state(&self) -> Option<(u8, u8, bool)> { None }

    /// Read whether the player is currently on track. Only meaningful for iRacing.
    /// Returns Some(true) when iRacing's IsOnTrack variable is set.
    /// Default: None (not applicable for other sims).
    fn read_is_on_track(&self) -> Option<bool> { None }

    /// Read the current session configuration from the running sim.
    /// Used by launch_verifier Stage 5 to compare against requested config.
    /// Returns None if the sim doesn't support config readback.
    fn read_session_config(&self) -> Option<SessionConfig> { None }
}

/// Build a sim adapter for the given SimType.
///
/// Called from two places:
/// 1. `main.rs` — initial adapter construction at rc-agent startup (matches
///    `config.pod.sim`, typically AC).
/// 2. `ws_handler.rs` — per-launch rebuild on LaunchGame when sim_type changes.
///
/// Returns `None` when:
/// - The sim type has no adapter implemented (Forza, FH5)
/// - The sim type is gated by a feature flag that is disabled (AC EVO when
///   `ac_evo_telemetry_enabled = false`)
///
/// ADAPTER-SWAP-01 (2026-04-12, James): before this factory existed, rc-agent
/// was sim-locked to the adapter chosen at startup from `config.pod.sim`. All
/// 8 pods have `sim = "assetto_corsa"`, so non-AC launches (F1 25, iRacing,
/// LMU, ACE, ACR) had no UDP/SHM listener. F1 25's UDP packets went into the
/// void, `UdpReachable` never fired, `launch_state` stayed `WaitingForLive`,
/// `AcStatus::Live` was never sent, and the server's 180s `check_launch_timeouts`
/// retry killed every non-AC launch via the pre_launch_checks cleanup cascade.
/// Full trace: `.planning/debug/flow-traces/runs/2026-04-12T05-17-IST-f1_25-launch-fullchain/`
pub(crate) fn build_sim_adapter(
    sim_type: SimType,
    pod_id: &str,
    config: &rc_common::config_schema::AgentConfig,
    signal_tx: &tokio::sync::mpsc::Sender<crate::driving_detector::DetectorSignal>,
) -> Option<Box<dyn SimAdapter>> {
    match sim_type {
        SimType::AssettoCorsa => Some(Box::new(assetto_corsa::AssettoCorsaAdapter::new(
            pod_id.to_string(),
            config.pod.sim_ip.clone(),
            config.pod.sim_port,
        ))),
        SimType::F125 => Some(Box::new(f1_25::F125Adapter::new(
            pod_id.to_string(),
            Some(signal_tx.clone()),
        ))),
        SimType::IRacing => Some(Box::new(iracing::IracingAdapter::new(
            pod_id.to_string(),
        ))),
        SimType::LeMansUltimate => Some(Box::new(lmu::LmuAdapter::new(
            pod_id.to_string(),
        ))),
        SimType::AssettoCorsaEvo => {
            if config.ac_evo_telemetry_enabled {
                Some(Box::new(assetto_corsa_evo::AssettoCorsaEvoAdapter::new(
                    pod_id.to_string(),
                )))
            } else {
                None
            }
        }
        SimType::AssettoCorsaRally => Some(Box::new(
            assetto_corsa_evo::AssettoCorsaEvoAdapter::new_rally(pod_id.to_string()),
        )),
        _ => None,
    }
}

#[cfg(test)]
mod adapter_parity_tests {
    /// REGRESSION: Every shared-memory sim adapter must have a liveness guard
    /// in read_telemetry() that checks the SHM is still valid before reading.
    /// Without this, when a game exits, reading from the stale mapping causes
    /// an access violation that bypasses the Rust panic hook.
    ///
    /// SHM adapters: assetto_corsa, assetto_corsa_evo, iracing, lmu
    /// Non-SHM adapters: f1_25 (UDP) — exempt
    ///
    /// This test reads the actual source code to verify the guard exists.
    /// If a new adapter is added without the guard, or an existing guard is
    /// removed, this test catches it.
    #[test]
    fn test_all_shm_adapters_have_liveness_guard_in_read_telemetry() {
        let sims_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join("sims");

        // Adapters that use shared memory and MUST have a guard
        let shm_adapters = [
            ("assetto_corsa.rs", "verify_shm_alive"),
            ("assetto_corsa_evo.rs", "verify_shm_alive"),
            ("iracing.rs", "verify_shm_alive"),
            ("lmu.rs", "verify_shm_alive"),
        ];

        for (filename, guard_fn) in &shm_adapters {
            let path = sims_dir.join(filename);
            let content = std::fs::read_to_string(&path)
                .unwrap_or_else(|_| panic!("Failed to read {}", filename));

            // Find the read_telemetry function body (NOT read_telemetry_from_plugin).
            // Match "fn read_telemetry(" exactly to avoid hitting _from_plugin variant.
            let in_read_telemetry: String = content
                .lines()
                .skip_while(|l| !(l.contains("fn read_telemetry(") && !l.contains("_from_plugin")))
                .skip(1) // skip the fn line itself
                .take(20) // first 20 lines should contain the guard
                .collect::<Vec<_>>()
                .join("\n");

            assert!(
                in_read_telemetry.contains(guard_fn),
                "REGRESSION: {filename} read_telemetry() is missing {guard_fn}() \
                 liveness guard. When the game exits, reading from stale shared \
                 memory causes an access violation. Every SHM adapter must verify \
                 the mapping is alive before reading. See commit 782ea3ba."
            );
        }
    }

    /// Verify that iRacing's find_var_offset has bounds guards on num_vars.
    /// Without this, corrupted shared memory headers cause unbounded pointer
    /// arithmetic → access violation.
    #[test]
    fn test_iracing_var_offset_has_bounds_guard() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src").join("sims").join("iracing.rs");
        let content = std::fs::read_to_string(&path).expect("read iracing.rs");

        let find_var_fn: String = content
            .lines()
            .skip_while(|l| !l.contains("fn find_var_offset"))
            .take(15)
            .collect::<Vec<_>>()
            .join("\n");

        assert!(
            find_var_fn.contains("num_vars") && find_var_fn.contains("4096"),
            "REGRESSION: iracing.rs find_var_offset() must guard num_vars \
             with an upper bound (4096). Corrupted header values from stale \
             shared memory cause unbounded pointer scan. See commit e388b5af."
        );
    }

    /// Verify non-SHM adapter (F1 25, UDP-based) does NOT need the guard.
    #[test]
    fn test_f1_25_is_udp_not_shm() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src").join("sims").join("f1_25.rs");
        let content = std::fs::read_to_string(&path).expect("read f1_25.rs");

        // F1 25 uses UdpSocket, not OpenFileMappingW
        assert!(
            content.contains("UdpSocket"),
            "f1_25.rs should use UDP, not shared memory"
        );
        assert!(
            !content.contains("OpenFileMappingW"),
            "f1_25.rs should NOT use shared memory (it's UDP-based)"
        );
    }
}
