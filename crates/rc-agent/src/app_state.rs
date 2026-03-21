use std::sync::Arc;
use tokio::sync::{mpsc, watch, RwLock};
use crate::config::AgentConfig;
use rc_common::types::MachineWhitelist;
use crate::driving_detector::{DetectorSignal, DrivingDetector};
use crate::ffb_controller::FfbController;
use crate::kiosk::KioskManager;
use crate::lock_screen::{LockScreenEvent, LockScreenManager};
use crate::overlay::OverlayManager;
use crate::debug_server;
use crate::failure_monitor;
use crate::game_process;
use crate::self_heal::SelfHealResult;
use crate::udp_heartbeat;
use rc_common::protocol::AgentMessage;
use rc_common::types::{AcStatus, AiDebugSuggestion, PodInfo, SimType};
use crate::sims::SimAdapter;

/// All long-lived agent state that survives WebSocket reconnections.
///
/// Variables initialized in main() before the reconnect loop are bundled here.
/// This enables event_loop::run() (Plan 74-04) to receive state as a single
/// parameter instead of 25+ separate variables.
pub struct AppState {
    pub(crate) pod_id: String,
    pub(crate) pod_info: PodInfo,
    pub(crate) config: AgentConfig,
    pub(crate) sim_type: SimType,
    pub(crate) installed_games: Vec<SimType>,
    pub(crate) ffb: Arc<FfbController>,
    pub(crate) detector: DrivingDetector,
    pub(crate) adapter: Option<Box<dyn SimAdapter>>,
    pub(crate) hid_detected: bool,
    pub(crate) kiosk: KioskManager,
    pub(crate) kiosk_enabled: bool,
    pub(crate) lock_screen: LockScreenManager,
    pub(crate) overlay: OverlayManager,
    pub(crate) signal_rx: mpsc::Receiver<DetectorSignal>,
    pub(crate) lock_event_rx: mpsc::Receiver<LockScreenEvent>,
    pub(crate) heartbeat_event_rx: mpsc::Receiver<udp_heartbeat::HeartbeatEvent>,
    pub(crate) ai_result_rx: mpsc::Receiver<AiDebugSuggestion>,
    pub(crate) ai_result_tx: mpsc::Sender<AiDebugSuggestion>,
    pub(crate) ws_exec_result_rx: mpsc::Receiver<AgentMessage>,
    pub(crate) ws_exec_result_tx: mpsc::Sender<AgentMessage>,
    /// Process guard shared whitelist — fetched on WS connect, read each scan cycle.
    /// Defaults to MachineWhitelist::default() (report_only, empty lists) until fetched.
    pub(crate) guard_whitelist: Arc<RwLock<MachineWhitelist>>,
    /// Sender half — process_guard module sends AgentMessage::ProcessViolation here.
    pub(crate) guard_violation_tx: mpsc::Sender<AgentMessage>,
    /// Receiver half — event_loop.rs drains this and forwards to WebSocket.
    pub(crate) guard_violation_rx: mpsc::Receiver<AgentMessage>,
    pub(crate) failure_monitor_tx: watch::Sender<failure_monitor::FailureMonitorState>,
    pub(crate) heartbeat_status: Arc<udp_heartbeat::HeartbeatStatus>,
    pub(crate) last_launch_error: debug_server::LastLaunchError,
    pub(crate) agent_start_time: std::time::Instant,
    pub(crate) exe_dir: std::path::PathBuf,
    pub(crate) heal_result: SelfHealResult,
    pub(crate) crash_recovery_startup: bool,
    pub(crate) startup_self_test_verdict: Option<String>,
    pub(crate) startup_probe_failures: u8,
    pub(crate) lock_screen_bound: bool,
    pub(crate) remote_ops_bound: bool,
    pub(crate) game_process: Option<game_process::GameProcess>,
    pub(crate) last_ac_status: Option<AcStatus>,
    pub(crate) ac_status_stable_since: Option<std::time::Instant>,
    pub(crate) in_maintenance: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// STAFF-04: Tracks when the last PreFlightFailed WS alert was sent.
    /// None = never alerted. Alerts are suppressed within a 60s cooldown window.
    pub(crate) last_preflight_alert: Option<std::time::Instant>,
}
