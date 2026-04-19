use std::sync::Arc;
use tokio::sync::{mpsc, watch, Mutex, RwLock};
use crate::safe_mode;
use crate::config::AgentConfig;
use crate::feature_flags::FeatureFlags;
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
use crate::diagnostic_engine;
use crate::diagnostic_log;
use crate::tier_engine;
use crate::udp_heartbeat;
use rc_common::protocol::AgentMessage;
use rc_common::types::{AcStatus, AiDebugSuggestion, PodInfo, SimType};
use crate::off_track_blanking::OffTrackBlanking;
use crate::off_track_detector::OffTrackDetector;
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
    #[allow(dead_code)]
    pub(crate) sim_type: SimType,
    #[allow(dead_code)]
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
    /// ADAPTER-SWAP-01 (2026-04-12, James): Kept as a Sender so ws_handler can
    /// clone it into a rebuilt sim adapter on every LaunchGame (per-launch
    /// adapter rebuild — see sims::build_sim_adapter). Before this field
    /// existed, signal_tx was consumed at startup and non-AC launches had no
    /// way to receive a freshly-cloned channel on adapter rebuild.
    pub(crate) signal_tx: mpsc::Sender<DetectorSignal>,
    pub(crate) lock_event_rx: mpsc::Receiver<LockScreenEvent>,
    pub(crate) heartbeat_event_rx: mpsc::Receiver<udp_heartbeat::HeartbeatEvent>,
    pub(crate) ai_result_rx: mpsc::Receiver<AiDebugSuggestion>,
    pub(crate) ai_result_tx: mpsc::Sender<AiDebugSuggestion>,
    pub(crate) ws_exec_result_rx: mpsc::Receiver<AgentMessage>,
    pub(crate) ws_exec_result_tx: mpsc::Sender<AgentMessage>,
    /// v22.0 Phase 178: In-memory feature flags — loaded from disk cache on startup,
    /// updated via FlagSync / KillSwitch WS messages, persisted on every update.
    pub(crate) flags: Arc<RwLock<FeatureFlags>>,
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
    /// Safe mode state machine — gates risky subsystems during protected game sessions.
    /// Lives in AppState (not ConnectionState) to survive WebSocket reconnections.
    pub(crate) safe_mode: safe_mode::SafeMode,
    /// Shadow flag for cross-task safe mode checks (process_guard reads this).
    /// Must be kept in sync with safe_mode.active on every state transition.
    pub(crate) safe_mode_active: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// WMI process start event receiver — None if WMI watcher failed to start.
    pub(crate) wmi_rx: Option<std::sync::mpsc::Receiver<String>>,
    /// Cooldown timer — fires 30s after protected game exits.
    pub(crate) safe_mode_cooldown_timer: std::pin::Pin<Box<tokio::time::Sleep>>,
    /// Whether the cooldown timer is armed (should be polled in select!).
    pub(crate) safe_mode_cooldown_armed: bool,
    /// STAFF-04: Tracks when the last PreFlightFailed WS alert was sent.
    /// None = never alerted. Alerts are suppressed within a 60s cooldown window.
    pub(crate) last_preflight_alert: Option<std::time::Instant>,
    /// Diagnostic event channel sender — pre-flight failures are emitted here
    /// so the tier engine can attempt autonomous healing via Meshed Intelligence.
    pub(crate) diagnostic_event_tx: mpsc::Sender<diagnostic_engine::DiagnosticEvent>,
    /// v27.0: Shared diagnostic event log — ring buffer of recent tier engine results
    pub(crate) diagnostic_log: diagnostic_log::DiagnosticLog,
    /// v27.0: Staff diagnostic request channel — WS handler injects requests for tier engine
    pub(crate) staff_diagnostic_tx: mpsc::Sender<tier_engine::StaffDiagnosticRequest>,
    /// BOOT-04: Operator confirmation that process guard allowlist is correct.
    /// When false, process guard stays in report_only even if configured for kill_and_report.
    /// Set to true via GUARD_CONFIRMED fleet exec command.
    pub(crate) guard_confirmed: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// SEC-10: Mutex serializing LaunchGame + clean_state_reset.
    /// Ensures clean_state_reset (5+ second blocking operation) completes before
    /// a second LaunchGame command can proceed. Prevents race: game launched while
    /// old processes are still being killed, leaving two game instances competing.
    pub(crate) game_launch_mutex: Arc<Mutex<()>>,
    pub(crate) off_track_detector: OffTrackDetector,
    pub(crate) off_track_blanking: OffTrackBlanking,
    /// LAUNCH-FIX-3: Signal from GAME-07 async task to hide lock screen when
    /// Steam URL game window is confirmed. The tokio::spawn'd task cannot borrow
    /// state directly, so it sends () here and event_loop calls close_browser().
    pub(crate) lock_screen_hide_tx: mpsc::Sender<()>,
    pub(crate) lock_screen_hide_rx: mpsc::Receiver<()>,
    /// Phase 306: JWT token received from server after PSK bootstrap.
    /// Used for subsequent WS reconnections. Cleared on 401 rejection.
    pub(crate) current_jwt: Option<String>,
    /// Phase 306: JWT expiry timestamp (Unix seconds). Reconnect loop
    /// compares against Utc::now() to decide PSK vs JWT URL.
    pub(crate) jwt_expires_at: Option<i64>,
    /// Phase 413 Plan 04: shared Option Z mesh service-key cache.
    ///
    /// Populated by `rc_common::boot_resilience::spawn_periodic_refetch` (see
    /// main.rs Plan 03 wire-up). Consumed by:
    ///   - `ai_debugger::check_audit_known_issues` (Tier 0 mesh oracle via `analyze_crash`)
    ///   - `remote_ops::require_service_key` (pod /exec middleware, sub-router state)
    ///   - `ws_handler` csv_lap_fallback push (session-end CSV upload)
    ///
    /// When empty / unpopulated, `get_key_or_env` falls back to the legacy
    /// `RCAGENT_SERVICE_KEY` env var (test + first-boot compatibility).
    #[cfg(feature = "http-client")]
    pub(crate) mesh_key_cache: crate::mesh_key_cache::MeshKeyCache,
    /// SAFETY-NET-01 (2026-04-19): timestamp when we first observed
    /// `state=ActiveSession` with no game process alive and no crash-recovery
    /// pause. Moved from `ConnectionState` to `AppState` so the stuck-detection
    /// timer survives WS disconnects. Pod 4 observed 2026-04-19: stuck in
    /// ActiveSession for 3h+ while WS was silent-reconnecting. The original
    /// in-WS tick never fired because `ConnectionState` is dropped on
    /// disconnect and the counter was reset on every reconnect. With this
    /// field on AppState, both the in-WS tick AND the reconnect-loop tick
    /// share state.
    pub(crate) stuck_active_session_since: Option<std::time::Instant>,
    /// SAFETY-NET-02 (2026-04-20): timestamp when rc-agent entered
    /// `LockScreenState::SessionSummary`. The old mechanism armed a 30s
    /// `tokio::time::Sleep` on `ConnectionState.blank_timer`; if the WS
    /// reconnected inside that 30s window the timer was dropped and the
    /// native SessionSummary window stayed visible forever (customer-reported
    /// symptom: "blanking does not re-apply after session ends"). Moving the
    /// threshold check to AppState + running it under the same WS-independent
    /// safety-net infrastructure makes the SessionSummary → ScreenBlanked
    /// transition resilient to WS flaps. Set to `Some(Instant::now())` by
    /// `ws_handler::SessionEnded`; lazily initialised by the tick if it finds
    /// lock_screen in SessionSummary with no timestamp (defence in depth for
    /// state changes made via remote_ops or debug tooling).
    pub(crate) session_summary_since: Option<std::time::Instant>,
}

impl AppState {
    #[allow(dead_code)]
    pub fn set_safe_mode_active(&mut self, active: bool) {
        self.safe_mode.active = active;
        self.safe_mode_active.store(active, std::sync::atomic::Ordering::SeqCst);
    }
}
