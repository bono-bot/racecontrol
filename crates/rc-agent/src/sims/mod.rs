pub mod assetto_corsa;
pub mod f1_25;

use anyhow::Result;
use rc_common::types::{AcStatus, SimType, TelemetryFrame, SessionInfo, LapData};

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
    fn session_info(&self) -> Result<Option<SessionInfo>>;

    /// Disconnect from telemetry source
    fn disconnect(&mut self);

    /// Car's max RPM (for RPM bar scaling). Default 8000 if unknown.
    fn max_rpm(&self) -> u32 { 8000 }

    /// Read the sim's current AC_STATUS from shared memory. Only meaningful for AC.
    fn read_ac_status(&self) -> Option<AcStatus> { None }
}
