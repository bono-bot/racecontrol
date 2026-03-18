use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ─── Sim Types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SimType {
    AssettoCorsa,
    AssettoCorsaEvo,
    AssettoCorsaRally,
    #[serde(rename = "iracing")]
    IRacing,
    LeMansUltimate,
    #[serde(rename = "f1_25")]
    F125,
    Forza,
    #[serde(rename = "forza_horizon_5")]
    ForzaHorizon5,
}

impl std::fmt::Display for SimType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SimType::AssettoCorsa => write!(f, "Assetto Corsa"),
            SimType::AssettoCorsaEvo => write!(f, "Assetto Corsa Evo"),
            SimType::AssettoCorsaRally => write!(f, "Assetto Corsa Rally"),
            SimType::IRacing => write!(f, "iRacing"),
            SimType::LeMansUltimate => write!(f, "Le Mans Ultimate"),
            SimType::F125 => write!(f, "F1 25"),
            SimType::Forza => write!(f, "Forza Motorsport"),
            SimType::ForzaHorizon5 => write!(f, "Forza Horizon 5"),
        }
    }
}

// ─── Pod ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PodStatus {
    Offline,
    Idle,
    InSession,
    Error,
    /// Admin has intentionally disabled this pod — skip all auto-recovery
    Disabled,
}

/// Classification of pod failure causes — shared taxonomy for bot detection (Phase 24+)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PodFailureReason {
    // Crash/hang class
    GameFrozen,
    ProcessHung,
    // Game launch class
    ContentManagerHang,
    LaunchTimeout,
    // USB/hardware class
    WheelbaseDisconnected,
    FfbFault,
    // Billing class
    SessionStuckWaitingForGame,
    IdleBillingDrift,
    CreditSyncFailed,
    // Telemetry class
    UdpDataMissing,
    TelemetryInvalid,
    // Multiplayer class
    MultiplayerDesync,
    MultiplayerServerDisconnect,
    // PIN class
    PinValidationFailed,
    StaffUnlockNeeded,
    // Lap class
    LapCut,
    LapInvalidSpeed,
    LapSpin,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodInfo {
    pub id: String,
    pub number: u32,
    pub name: String,
    pub ip_address: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mac_address: Option<String>,
    pub sim_type: SimType,
    pub status: PodStatus,
    pub current_driver: Option<String>,
    pub current_session_id: Option<String>,
    pub last_seen: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub driving_state: Option<DrivingState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub billing_session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub game_state: Option<GameState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_game: Option<SimType>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub installed_games: Vec<SimType>,
    /// Whether the pod screen is currently blanked (black screen between sessions).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screen_blanked: Option<bool>,
    /// Current FFB preset: "light", "medium", or "strong".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ffb_preset: Option<String>,
}

// ─── Driver ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Driver {
    pub id: String,
    pub name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub steam_guid: Option<String>,
    pub iracing_id: Option<String>,
    pub total_laps: u64,
    pub total_time_ms: u64,
    pub created_at: DateTime<Utc>,
}

// ─── Session ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionType {
    Practice,
    Qualifying,
    Race,
    Hotlap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Pending,
    Active,
    Finished,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: String,
    pub session_type: SessionType,
    pub sim_type: SimType,
    pub track: String,
    pub car_class: Option<String>,
    pub status: SessionStatus,
    pub max_drivers: Option<u32>,
    pub laps_or_minutes: Option<u32>,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
}

// ─── Telemetry ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryFrame {
    pub pod_id: String,
    pub timestamp: DateTime<Utc>,
    pub driver_name: String,
    pub car: String,
    pub track: String,
    pub lap_number: u32,
    pub lap_time_ms: u32,
    pub sector: u8,
    pub speed_kmh: f32,
    pub throttle: f32,
    pub brake: f32,
    pub steering: f32,
    pub gear: i8,
    pub rpm: u32,
    pub position: Option<Position3D>,
    pub session_time_ms: u32,

    // F1-specific telemetry (optional — only populated by F1 adapter)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drs_active: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drs_available: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ers_deploy_mode: Option<u8>, // 0=none, 1=medium, 2=hotlap, 3=overtake
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ers_store_percent: Option<f32>, // 0.0–100.0
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub best_lap_ms: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_lap_invalid: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sector1_ms: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sector2_ms: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sector3_ms: Option<u32>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Position3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

// ─── Lap ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LapData {
    pub id: String,
    pub session_id: String,
    pub driver_id: String,
    pub pod_id: String,
    pub sim_type: SimType,
    pub track: String,
    pub car: String,
    pub lap_number: u32,
    pub lap_time_ms: u32,
    pub sector1_ms: Option<u32>,
    pub sector2_ms: Option<u32>,
    pub sector3_ms: Option<u32>,
    pub valid: bool,
    pub session_type: SessionType,
    pub created_at: DateTime<Utc>,
}

// ─── Leaderboard ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderboardEntry {
    pub position: u32,
    pub driver_name: String,
    pub driver_id: String,
    pub car: String,
    pub best_lap_ms: u32,
    pub last_lap_ms: Option<u32>,
    pub total_laps: u32,
    pub gap_to_leader_ms: Option<i64>,
    pub is_personal_best: bool,
    pub is_track_record: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Leaderboard {
    pub session_id: String,
    pub track: String,
    pub session_type: SessionType,
    pub entries: Vec<LeaderboardEntry>,
    pub updated_at: DateTime<Utc>,
}

// ─── Events ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    SingleRace,
    Tournament,
    Championship,
    HotlapCompetition,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: String,
    pub name: String,
    pub event_type: EventType,
    pub sim_type: Option<SimType>,
    pub track: Option<String>,
    pub car_class: Option<String>,
    pub max_entries: Option<u32>,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

// ─── Booking ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Booking {
    pub id: String,
    pub driver_id: String,
    pub pod_id: Option<String>,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub status: String,
    pub payment_status: String,
    pub created_at: DateTime<Utc>,
}

// ─── Driving State ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DrivingState {
    /// Customer is actively driving (pedal/wheel inputs or game telemetry detected)
    Active,
    /// No driving inputs detected (menu, loading, pit stationary)
    Idle,
    /// No detection source available (no HID device, no UDP data)
    NoDevice,
}

// ─── AC Status ─────────────────────────────────────────────────────────────

/// Assetto Corsa shared memory STATUS field values.
/// Maps to graphics::STATUS: 0=Off, 1=Replay, 2=Live, 3=Pause.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcStatus {
    /// AC not running or in menu (STATUS=0)
    Off,
    /// Watching replay (STATUS=1)
    Replay,
    /// Car is on track, driving (STATUS=2) -- billing trigger
    Live,
    /// Game paused via ESC menu (STATUS=3) -- billing pauses
    Pause,
}

// ─── Billing ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BillingSessionStatus {
    Pending,
    /// Game launched, waiting for AC STATUS=LIVE before billing starts
    WaitingForGame,
    Active,
    PausedManual,
    PausedDisconnect,
    /// Billing paused because AC STATUS=PAUSE (customer hit ESC)
    PausedGamePause,
    Completed,
    EndedEarly,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillingSessionInfo {
    pub id: String,
    pub driver_id: String,
    pub driver_name: String,
    pub pod_id: String,
    pub pricing_tier_name: String,
    pub allocated_seconds: u32,
    pub driving_seconds: u32,
    pub remaining_seconds: u32,
    pub status: BillingSessionStatus,
    pub driving_state: DrivingState,
    pub started_at: Option<DateTime<Utc>>,
    /// Number of sub-sessions (e.g. 3 for 3×10min). Default 1 = no split.
    pub split_count: u32,
    /// Duration of each sub-session in minutes (e.g. 10 for 3×10min). None = no split.
    pub split_duration_minutes: Option<u32>,
    /// Which sub-session is currently running (1-indexed). 1 = first or only session.
    pub current_split_number: u32,
    /// Elapsed driving seconds (count-up model). None for legacy countdown sessions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elapsed_seconds: Option<u32>,
    /// Running cost in paise. None for legacy countdown sessions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_paise: Option<i64>,
    /// Current rate per minute in paise (2330 standard, 1500 value). None for legacy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate_per_min_paise: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricingTier {
    pub id: String,
    pub name: String,
    pub duration_minutes: u32,
    pub price_paise: u32,
    pub is_trial: bool,
    pub is_active: bool,
}

// ─── Game Launcher ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameState {
    /// No game running on this pod
    Idle,
    /// Game executable is being launched
    Launching,
    /// Game process is running (PID tracked)
    Running,
    /// Game is being stopped/killed
    Stopping,
    /// Game crashed or failed to launch
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameLaunchInfo {
    pub pod_id: String,
    pub sim_type: SimType,
    pub game_state: GameState,
    pub pid: Option<u32>,
    pub launched_at: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagnostics: Option<LaunchDiagnostics>,
}

/// Structured diagnostics from a game launch attempt.
/// Populated by rc-agent, forwarded to racecontrol for dashboard display.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LaunchDiagnostics {
    /// Whether Content Manager was attempted (multiplayer only)
    pub cm_attempted: bool,
    /// CM process exit code (None if CM wasn't used or is still running)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cm_exit_code: Option<i32>,
    /// CM log error excerpts (None if no errors found)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cm_log_errors: Option<String>,
    /// Whether direct acs.exe fallback was used after CM failure
    #[serde(default)]
    pub fallback_used: bool,
    /// acs.exe exit code if it exited immediately (None if still running)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direct_exit_code: Option<i32>,
}

// ─── AC Dedicated Server ──────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcServerStatus {
    Starting,
    Running,
    Stopping,
    Stopped,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcSessionBlock {
    pub name: String,
    pub session_type: SessionType,
    pub duration_minutes: u32,
    pub laps: u32,
    pub wait_time_secs: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcWeatherConfig {
    pub graphics: String,
    pub base_temperature_ambient: u32,
    pub base_temperature_road: u32,
    pub variation_ambient: u32,
    pub variation_road: u32,
    pub wind_base_speed_min: u32,
    pub wind_base_speed_max: u32,
    pub wind_base_direction: u32,
    pub wind_variation_direction: u32,
}

impl Default for AcWeatherConfig {
    fn default() -> Self {
        Self {
            graphics: "3_clear".to_string(),
            base_temperature_ambient: 22,
            base_temperature_road: 28,
            variation_ambient: 2,
            variation_road: 2,
            wind_base_speed_min: 0,
            wind_base_speed_max: 5,
            wind_base_direction: 0,
            wind_variation_direction: 15,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcDynamicTrackConfig {
    pub session_start: u32,
    pub randomness: u32,
    pub session_transfer: u32,
    pub lap_gain: u32,
}

impl Default for AcDynamicTrackConfig {
    fn default() -> Self {
        Self {
            session_start: 100,  // Fully rubbered-in from the start
            randomness: 0,       // No random grip variation
            session_transfer: 100, // Full grip transfer between sessions
            lap_gain: 0,         // No grip change from driving — consistent conditions
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcEntrySlot {
    pub car_model: String,
    pub skin: String,
    pub driver_name: String,
    pub guid: String,
    pub ballast: u32,
    pub restrictor: u32,
    pub pod_id: Option<String>,
    /// None for human entries, Some("fixed") for AI entries (AssettoServer format)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcLanSessionConfig {
    pub name: String,
    pub track: String,
    pub track_config: String,
    pub cars: Vec<String>,
    pub max_clients: u32,
    pub password: String,
    pub sessions: Vec<AcSessionBlock>,
    pub entries: Vec<AcEntrySlot>,
    pub weather: Vec<AcWeatherConfig>,
    pub dynamic_track: AcDynamicTrackConfig,
    pub pickup_mode: bool,
    pub udp_port: u16,
    pub tcp_port: u16,
    pub http_port: u16,
    pub min_csp_version: u32,
    pub csp_extra_options: Option<String>,
    pub abs_allowed: u32,
    pub tc_allowed: u32,
    pub autoclutch_allowed: bool,
    pub tyre_blankets_allowed: bool,
    pub stability_allowed: bool,
    pub force_virtual_mirror: bool,
    pub damage_multiplier: u32,
    pub fuel_rate: u32,
    pub tyre_wear_rate: u32,
}

impl Default for AcLanSessionConfig {
    fn default() -> Self {
        Self {
            name: "RacingPoint LAN Race".to_string(),
            track: "monza".to_string(),
            track_config: String::new(),
            cars: vec!["ks_ferrari_488_gt3".to_string()],
            max_clients: 16,
            password: String::new(),
            sessions: vec![
                AcSessionBlock {
                    name: "Practice".to_string(),
                    session_type: SessionType::Practice,
                    duration_minutes: 10,
                    laps: 0,
                    wait_time_secs: 30,
                },
                AcSessionBlock {
                    name: "Qualifying".to_string(),
                    session_type: SessionType::Qualifying,
                    duration_minutes: 10,
                    laps: 0,
                    wait_time_secs: 60,
                },
                AcSessionBlock {
                    name: "Race".to_string(),
                    session_type: SessionType::Race,
                    duration_minutes: 0,
                    laps: 10,
                    wait_time_secs: 60,
                },
            ],
            entries: Vec::new(),
            weather: vec![AcWeatherConfig::default()],
            dynamic_track: AcDynamicTrackConfig::default(),
            pickup_mode: true,
            udp_port: 9600,
            tcp_port: 9600,
            http_port: 8081,
            min_csp_version: 2144, // Enforce CSP — fixes audio restart on session start
            csp_extra_options: None,
            abs_allowed: 1,
            tc_allowed: 1,
            autoclutch_allowed: true,
            tyre_blankets_allowed: true,
            stability_allowed: false,
            force_virtual_mirror: false,
            damage_multiplier: 100,
            fuel_rate: 100,
            tyre_wear_rate: 100,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcServerInfo {
    pub session_id: String,
    pub config: AcLanSessionConfig,
    pub status: AcServerStatus,
    pub pid: Option<u32>,
    pub started_at: Option<DateTime<Utc>>,
    pub join_url: String,
    pub connected_pods: Vec<String>,
    pub error_message: Option<String>,
    #[serde(default)]
    pub continuous_mode: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcPresetSummary {
    pub id: String,
    pub name: String,
    pub track: String,
    pub track_config: String,
    pub cars: Vec<String>,
    pub max_clients: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

// ─── Pod Activity Log ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodActivityEntry {
    pub id: String,
    pub pod_id: String,
    pub pod_number: u32,
    pub timestamp: String,
    pub category: String,  // system | game | billing | auth | race_engineer
    pub action: String,
    pub details: String,
    pub source: String,    // agent | core | race_engineer | staff
}

// ─── Auth Token ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthTokenInfo {
    pub id: String,
    pub pod_id: String,
    pub driver_id: String,
    pub driver_name: String,
    pub pricing_tier_id: String,
    pub pricing_tier_name: String,
    pub auth_type: String,
    pub token: String,
    pub status: String,
    pub allocated_seconds: u32,
    pub custom_price_paise: Option<u32>,
    pub custom_duration_minutes: Option<u32>,
    pub created_at: String,
    pub expires_at: String,
}

// ─── Wallet ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletInfo {
    pub driver_id: String,
    pub balance_paise: i64,
    pub total_credited_paise: i64,
    pub total_debited_paise: i64,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletTransaction {
    pub id: String,
    pub driver_id: String,
    pub amount_paise: i64,
    pub balance_after_paise: i64,
    pub txn_type: String,
    pub reference_id: Option<String>,
    pub notes: Option<String>,
    pub staff_id: Option<String>,
    pub created_at: String,
}

// ─── Pod Reservation ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodReservationInfo {
    pub id: String,
    pub driver_id: String,
    pub pod_id: String,
    pub status: String,
    pub created_at: String,
    pub ended_at: Option<String>,
    pub last_activity_at: Option<String>,
}

// ─── AI Debugger ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiDebugSuggestion {
    pub pod_id: String,
    pub sim_type: SimType,
    pub error_context: String,
    pub suggestion: String,
    pub model: String,
    pub created_at: DateTime<Utc>,
}

// ─── Deploy Types ────────────────────────────────────────────────────────────

/// Deploy lifecycle state for a single pod.
/// Tracks progress through the kill->verify->download->start->verify sequence.
/// Used by the deploy executor (racecontrol) and displayed in the kiosk dashboard.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", content = "detail")]
#[serde(rename_all = "snake_case")]
pub enum DeployState {
    /// No deploy in progress
    Idle,
    /// Sending taskkill to rc-agent.exe
    Killing,
    /// Polling for process to exit after kill signal
    WaitingDead,
    /// Downloading new binary from HTTP server
    Downloading {
        progress_pct: u8,
    },
    /// Verifying downloaded binary size meets minimum threshold
    SizeCheck,
    /// Starting new rc-agent.exe process
    Starting,
    /// Waiting for process alive + WebSocket + lock screen health
    VerifyingHealth,
    /// Deploy completed successfully
    Complete,
    /// Deploy failed at some step
    Failed {
        reason: String,
    },
    /// Pod has an active billing session — deploy is queued until session ends
    WaitingSession,
    /// Rolling back to rc-agent-prev.exe after health verification failure
    RollingBack,
}

impl Default for DeployState {
    fn default() -> Self {
        DeployState::Idle
    }
}

impl DeployState {
    /// Returns true if a deploy is actively in progress (not idle, complete, failed, or waiting).
    pub fn is_active(&self) -> bool {
        !matches!(
            self,
            DeployState::Idle
                | DeployState::Complete
                | DeployState::Failed { .. }
                | DeployState::WaitingSession
        )
    }
}

// ─── Friends & Multiplayer Types ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FriendInfo {
    pub driver_id: String,
    pub name: String,
    pub customer_id: Option<String>,
    pub is_online: bool,
    pub total_laps: i64,
    pub total_time_ms: i64,
    pub session_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FriendRequestInfo {
    pub id: String,
    pub driver_id: String,
    pub driver_name: String,
    pub customer_id: Option<String>,
    pub direction: String, // "incoming" or "outgoing"
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupSessionInfo {
    pub id: String,
    pub host_driver_id: String,
    pub host_name: String,
    pub experience_name: String,
    pub pricing_tier_name: String,
    pub shared_pin: String,
    pub status: String,
    pub members: Vec<GroupMemberInfo>,
    pub created_at: String,
    /// Track ID for lobby display (e.g. "monza")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub track: Option<String>,
    /// Car model for lobby display (e.g. "ks_ferrari_488_gt3")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub car: Option<String>,
    /// Number of AI opponents filling the grid
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_count: Option<u32>,
    /// Difficulty tier name (e.g. "semi_pro")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub difficulty_tier: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupMemberInfo {
    pub driver_id: String,
    pub driver_name: String,
    pub customer_id: Option<String>,
    pub role: String,   // "host" or "invitee"
    pub status: String, // pending/accepted/declined/validated/completed/cancelled
    pub pod_id: Option<String>,
    pub pod_number: Option<u32>,
}

// ─── Watchdog ──────────────────────────────────────────────────────────────

/// Crash report sent by rc-watchdog to racecontrol after restarting rc-agent.
/// Fire-and-forget HTTP POST — delivery failure is non-fatal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WatchdogCrashReport {
    pub pod_id: String,
    pub exit_code: Option<i32>,
    /// ISO 8601 UTC timestamp of the detected crash (Utc::now().to_rfc3339())
    pub crash_time: String,
    pub restart_count: u32,
    pub watchdog_version: String,
}

// -- Content Manifest (Phase 05) --

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentManifest {
    pub cars: Vec<CarManifestEntry>,
    pub tracks: Vec<TrackManifestEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CarManifestEntry {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackManifestEntry {
    pub id: String,
    pub configs: Vec<TrackConfigManifest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackConfigManifest {
    pub config: String,       // "" for default layout
    pub has_ai: bool,
    pub pit_count: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_sim_type_serde_roundtrip() {
        let variants = vec![
            SimType::AssettoCorsa,
            SimType::AssettoCorsaEvo,
            SimType::AssettoCorsaRally,
            SimType::IRacing,
            SimType::LeMansUltimate,
            SimType::F125,
            SimType::Forza,
            SimType::ForzaHorizon5,
        ];
        for variant in variants {
            let json = serde_json::to_string(&variant).unwrap();
            let parsed: SimType = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, parsed, "Roundtrip failed for {:?}", variant);
        }
    }

    #[test]
    fn test_sim_type_json_values() {
        assert_eq!(serde_json::to_string(&SimType::AssettoCorsa).unwrap(), "\"assetto_corsa\"");
        assert_eq!(serde_json::to_string(&SimType::AssettoCorsaEvo).unwrap(), "\"assetto_corsa_evo\"");
        assert_eq!(serde_json::to_string(&SimType::AssettoCorsaRally).unwrap(), "\"assetto_corsa_rally\"");
        assert_eq!(serde_json::to_string(&SimType::IRacing).unwrap(), "\"iracing\"");
        assert_eq!(serde_json::to_string(&SimType::LeMansUltimate).unwrap(), "\"le_mans_ultimate\"");
        assert_eq!(serde_json::to_string(&SimType::F125).unwrap(), "\"f1_25\"");
        assert_eq!(serde_json::to_string(&SimType::Forza).unwrap(), "\"forza\"");
        assert_eq!(serde_json::to_string(&SimType::ForzaHorizon5).unwrap(), "\"forza_horizon_5\"");
    }

    #[test]
    fn test_sim_type_display() {
        assert_eq!(SimType::AssettoCorsa.to_string(), "Assetto Corsa");
        assert_eq!(SimType::AssettoCorsaEvo.to_string(), "Assetto Corsa Evo");
        assert_eq!(SimType::AssettoCorsaRally.to_string(), "Assetto Corsa Rally");
        assert_eq!(SimType::IRacing.to_string(), "iRacing");
        assert_eq!(SimType::LeMansUltimate.to_string(), "Le Mans Ultimate");
        assert_eq!(SimType::F125.to_string(), "F1 25");
        assert_eq!(SimType::Forza.to_string(), "Forza Motorsport");
        assert_eq!(SimType::ForzaHorizon5.to_string(), "Forza Horizon 5");
    }

    #[test]
    fn test_sim_type_launch_game_message_roundtrip() {
        use crate::protocol::CoreToAgentMessage;

        // Test with new variant AssettoCorsaRally
        let msg = CoreToAgentMessage::LaunchGame {
            sim_type: SimType::AssettoCorsaRally,
            launch_args: Some("{\"track\":\"rally_stage_1\"}".to_string()),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("assetto_corsa_rally"));
        let parsed: CoreToAgentMessage = serde_json::from_str(&json).unwrap();
        if let CoreToAgentMessage::LaunchGame { sim_type, launch_args } = parsed {
            assert_eq!(sim_type, SimType::AssettoCorsaRally);
            assert_eq!(launch_args, Some("{\"track\":\"rally_stage_1\"}".to_string()));
        } else {
            panic!("Wrong variant after deserialize");
        }

        // Test with new variant ForzaHorizon5
        let msg2 = CoreToAgentMessage::LaunchGame {
            sim_type: SimType::ForzaHorizon5,
            launch_args: None,
        };
        let json2 = serde_json::to_string(&msg2).unwrap();
        assert!(json2.contains("forza_horizon_5"));
        let parsed2: CoreToAgentMessage = serde_json::from_str(&json2).unwrap();
        if let CoreToAgentMessage::LaunchGame { sim_type, .. } = parsed2 {
            assert_eq!(sim_type, SimType::ForzaHorizon5);
        } else {
            panic!("Wrong variant after deserialize");
        }
    }

    #[test]
    fn deploy_state_idle_roundtrip() {
        let state = DeployState::Idle;
        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains("idle"));
        let parsed: DeployState = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, DeployState::Idle);
    }

    #[test]
    fn deploy_state_killing_roundtrip() {
        let state = DeployState::Killing;
        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains("killing"));
        let parsed: DeployState = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, DeployState::Killing);
    }

    #[test]
    fn deploy_state_waiting_dead_roundtrip() {
        let state = DeployState::WaitingDead;
        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains("waiting_dead"));
        let parsed: DeployState = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, DeployState::WaitingDead);
    }

    #[test]
    fn deploy_state_downloading_roundtrip() {
        let state = DeployState::Downloading { progress_pct: 50 };
        let json = serde_json::to_string(&state).unwrap();
        let parsed: DeployState = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, DeployState::Downloading { progress_pct: 50 });
    }

    #[test]
    fn deploy_state_size_check_roundtrip() {
        let state = DeployState::SizeCheck;
        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains("size_check"));
        let parsed: DeployState = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, DeployState::SizeCheck);
    }

    #[test]
    fn deploy_state_starting_roundtrip() {
        let state = DeployState::Starting;
        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains("starting"));
        let parsed: DeployState = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, DeployState::Starting);
    }

    #[test]
    fn deploy_state_verifying_health_roundtrip() {
        let state = DeployState::VerifyingHealth;
        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains("verifying_health"));
        let parsed: DeployState = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, DeployState::VerifyingHealth);
    }

    #[test]
    fn deploy_state_complete_roundtrip() {
        let state = DeployState::Complete;
        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains("complete"));
        let parsed: DeployState = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, DeployState::Complete);
    }

    #[test]
    fn deploy_state_failed_roundtrip() {
        let state = DeployState::Failed { reason: "binary too small".to_string() };
        let json = serde_json::to_string(&state).unwrap();
        let parsed: DeployState = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, DeployState::Failed { reason: "binary too small".to_string() });
    }

    #[test]
    fn deploy_state_default_is_idle() {
        let state = DeployState::default();
        assert_eq!(state, DeployState::Idle);
    }

    #[test]
    fn deploy_state_is_active_false_for_idle_complete_failed() {
        assert!(!DeployState::Idle.is_active());
        assert!(!DeployState::Complete.is_active());
        assert!(!DeployState::Failed { reason: "x".into() }.is_active());
    }

    #[test]
    fn deploy_state_is_active_true_for_in_progress_states() {
        assert!(DeployState::Killing.is_active());
        assert!(DeployState::WaitingDead.is_active());
        assert!(DeployState::Downloading { progress_pct: 0 }.is_active());
        assert!(DeployState::SizeCheck.is_active());
        assert!(DeployState::Starting.is_active());
        assert!(DeployState::VerifyingHealth.is_active());
    }

    // ── AcStatus tests (Phase 03 Plan 01) ─────────────────────────────────

    #[test]
    fn ac_status_serde_roundtrip_all_variants() {
        let variants = vec![
            (AcStatus::Off, "\"off\""),
            (AcStatus::Replay, "\"replay\""),
            (AcStatus::Live, "\"live\""),
            (AcStatus::Pause, "\"pause\""),
        ];
        for (variant, expected_json) in variants {
            let json = serde_json::to_string(&variant).unwrap();
            assert_eq!(json, expected_json, "Serialize failed for {:?}", variant);
            let parsed: AcStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, parsed, "Roundtrip failed for {:?}", variant);
        }
    }

    #[test]
    fn billing_session_status_paused_game_pause_roundtrip() {
        let status = BillingSessionStatus::PausedGamePause;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"paused_game_pause\"");
        let parsed: BillingSessionStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, BillingSessionStatus::PausedGamePause);
    }

    #[test]
    fn billing_session_status_waiting_for_game_roundtrip() {
        let status = BillingSessionStatus::WaitingForGame;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"waiting_for_game\"");
        let parsed: BillingSessionStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, BillingSessionStatus::WaitingForGame);
    }

    #[test]
    fn billing_session_info_with_new_optional_fields_roundtrip() {
        let info = BillingSessionInfo {
            id: "sess-1".to_string(),
            driver_id: "drv-1".to_string(),
            driver_name: "Test Driver".to_string(),
            pod_id: "pod_1".to_string(),
            pricing_tier_name: "per-minute".to_string(),
            allocated_seconds: 10800,
            driving_seconds: 900,
            remaining_seconds: 9900,
            status: BillingSessionStatus::Active,
            driving_state: DrivingState::Active,
            started_at: None,
            split_count: 1,
            split_duration_minutes: None,
            current_split_number: 1,
            elapsed_seconds: Some(900),
            cost_paise: Some(34950),
            rate_per_min_paise: Some(2330),
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"elapsed_seconds\":900"));
        assert!(json.contains("\"cost_paise\":34950"));
        assert!(json.contains("\"rate_per_min_paise\":2330"));
        let parsed: BillingSessionInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.elapsed_seconds, Some(900));
        assert_eq!(parsed.cost_paise, Some(34950));
        assert_eq!(parsed.rate_per_min_paise, Some(2330));
    }

    #[test]
    fn billing_session_info_without_optional_fields_backward_compat() {
        // Old-format BillingSessionInfo without new fields should deserialize with None
        let json = r#"{
            "id": "sess-1",
            "driver_id": "drv-1",
            "driver_name": "Test",
            "pod_id": "pod_1",
            "pricing_tier_name": "30 Minutes",
            "allocated_seconds": 1800,
            "driving_seconds": 100,
            "remaining_seconds": 1700,
            "status": "active",
            "driving_state": "active",
            "started_at": null,
            "split_count": 1,
            "split_duration_minutes": null,
            "current_split_number": 1
        }"#;
        let parsed: BillingSessionInfo = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.elapsed_seconds, None);
        assert_eq!(parsed.cost_paise, None);
        assert_eq!(parsed.rate_per_min_paise, None);
    }

    // ── WaitingSession tests (Task 1 - 04-03) ───────────────────────────────

    #[test]
    fn deploy_state_waiting_session_serializes_to_waiting_session() {
        let state = DeployState::WaitingSession;
        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains("waiting_session"), "Expected 'waiting_session' in JSON: {}", json);
        let parsed: DeployState = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, DeployState::WaitingSession);
    }

    #[test]
    fn deploy_state_waiting_session_is_not_active() {
        // WaitingSession is a queued state — not "active" in the deploy sense
        // (it doesn't block watchdog; it just defers until session ends)
        assert!(!DeployState::WaitingSession.is_active());
    }

    // ── RollingBack tests (Phase 20-deploy-resilience Plan 01) ──────────────

    #[test]
    fn deploy_state_rolling_back_serde() {
        let state = DeployState::RollingBack;
        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains("rolling_back"), "Expected 'rolling_back' in JSON: {}", json);
        let parsed: DeployState = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, DeployState::RollingBack);
    }

    #[test]
    fn rolling_back_is_active() {
        // RollingBack is an active deploy phase — prevents second deploy from starting
        assert!(DeployState::RollingBack.is_active());
    }

    // ── Phase 09 Plan 01: AcEntrySlot ai_mode + GroupSessionInfo enrichment ─

    #[test]
    fn ac_entry_slot_without_ai_mode_backward_compat() {
        // AcEntrySlot with ai_mode None should serialize without ai_mode field
        let entry = AcEntrySlot {
            car_model: "ks_ferrari_488_gt3".to_string(),
            skin: String::new(),
            driver_name: "Test Driver".to_string(),
            guid: "steam_123".to_string(),
            ballast: 0,
            restrictor: 0,
            pod_id: Some("pod_1".to_string()),
            ai_mode: None,
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(!json.contains("ai_mode"), "ai_mode None must not appear in JSON: {}", json);
    }

    #[test]
    fn ac_entry_slot_with_ai_mode_fixed() {
        let entry = AcEntrySlot {
            car_model: "ks_ferrari_488_gt3".to_string(),
            skin: String::new(),
            driver_name: "Marco Rossi".to_string(),
            guid: String::new(),
            ballast: 0,
            restrictor: 0,
            pod_id: None,
            ai_mode: Some("fixed".to_string()),
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"ai_mode\":\"fixed\""), "ai_mode fixed must appear in JSON: {}", json);
    }

    #[test]
    fn ac_entry_slot_deserialize_without_ai_mode_backward_compat() {
        // Old-format AcEntrySlot without ai_mode should deserialize with None
        let json = r#"{
            "car_model": "ks_bmw_m3_gt2",
            "skin": "",
            "driver_name": "Old Driver",
            "guid": "steam_456",
            "ballast": 0,
            "restrictor": 0,
            "pod_id": "pod_2"
        }"#;
        let entry: AcEntrySlot = serde_json::from_str(json).unwrap();
        assert_eq!(entry.ai_mode, None, "Missing ai_mode must default to None");
    }

    #[test]
    fn group_session_info_with_track_car_ai_count() {
        let info = GroupSessionInfo {
            id: "gs-1".to_string(),
            host_driver_id: "drv-1".to_string(),
            host_name: "Host".to_string(),
            experience_name: "GT3 Race".to_string(),
            pricing_tier_name: "per-minute".to_string(),
            shared_pin: "1234".to_string(),
            status: "active".to_string(),
            members: vec![],
            created_at: "2026-01-01".to_string(),
            track: Some("monza".to_string()),
            car: Some("ks_ferrari_488_gt3".to_string()),
            ai_count: Some(15),
            difficulty_tier: Some("semi_pro".to_string()),
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"track\":\"monza\""), "track must appear: {}", json);
        assert!(json.contains("\"car\":\"ks_ferrari_488_gt3\""), "car must appear: {}", json);
        assert!(json.contains("\"ai_count\":15"), "ai_count must appear: {}", json);
        assert!(json.contains("\"difficulty_tier\":\"semi_pro\""), "difficulty_tier must appear: {}", json);
    }

    // ── WatchdogCrashReport tests (Phase 19 Plan 01) ──────────────────────

    #[test]
    fn test_watchdog_crash_report_serde_roundtrip() {
        let report = WatchdogCrashReport {
            pod_id: "pod_3".to_string(),
            exit_code: Some(-1073741819), // STATUS_ACCESS_VIOLATION
            crash_time: "2026-03-15T10:00:00+00:00".to_string(),
            restart_count: 5,
            watchdog_version: "0.1.0".to_string(),
        };
        let json = serde_json::to_string(&report).unwrap();
        let parsed: WatchdogCrashReport = serde_json::from_str(&json).unwrap();
        assert_eq!(report, parsed);
    }

    #[test]
    fn test_watchdog_crash_report_exit_code_none() {
        let report = WatchdogCrashReport {
            pod_id: "pod_1".to_string(),
            exit_code: None,
            crash_time: "2026-03-15T10:00:00+00:00".to_string(),
            restart_count: 1,
            watchdog_version: "0.1.0".to_string(),
        };
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"exit_code\":null"));
        let parsed: WatchdogCrashReport = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.exit_code, None);
    }

    #[test]
    fn test_watchdog_crash_report_json_fields() {
        let report = WatchdogCrashReport {
            pod_id: "pod_8".to_string(),
            exit_code: Some(1),
            crash_time: "2026-03-15T12:30:00+00:00".to_string(),
            restart_count: 42,
            watchdog_version: "0.1.0".to_string(),
        };
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"pod_id\":\"pod_8\""));
        assert!(json.contains("\"exit_code\":1"));
        assert!(json.contains("\"restart_count\":42"));
        assert!(json.contains("\"watchdog_version\":\"0.1.0\""));
    }

    #[test]
    fn group_session_info_without_new_fields_backward_compat() {
        // Old-format GroupSessionInfo without new fields should deserialize with None
        let json = r#"{
            "id": "gs-1",
            "host_driver_id": "drv-1",
            "host_name": "Host",
            "experience_name": "GT3",
            "pricing_tier_name": "30 Minutes",
            "shared_pin": "1234",
            "status": "forming",
            "members": [],
            "created_at": "2026-01-01"
        }"#;
        let info: GroupSessionInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.track, None);
        assert_eq!(info.car, None);
        assert_eq!(info.ai_count, None);
        assert_eq!(info.difficulty_tier, None);
    }

    // ── Phase 23 Plan 01: PodFailureReason serde tests ────────────────────

    #[test]
    fn test_pod_failure_reason_serde_roundtrip() {
        // 1. Serde roundtrip: GameFrozen → json → back
        let reason = PodFailureReason::GameFrozen;
        let json = serde_json::to_string(&reason).unwrap();
        assert!(json.contains("game_frozen"), "Expected 'game_frozen' in JSON, got: {}", json);
        let parsed: PodFailureReason = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, PodFailureReason::GameFrozen);

        // 2. All 9 failure class groups have at least one variant
        // Crash/hang class
        let _ = PodFailureReason::GameFrozen;
        let _ = PodFailureReason::ProcessHung;
        // Game launch class
        let _ = PodFailureReason::ContentManagerHang;
        let _ = PodFailureReason::LaunchTimeout;
        // USB/hardware class
        let _ = PodFailureReason::WheelbaseDisconnected;
        let _ = PodFailureReason::FfbFault;
        // Billing class
        let _ = PodFailureReason::SessionStuckWaitingForGame;
        let _ = PodFailureReason::IdleBillingDrift;
        let _ = PodFailureReason::CreditSyncFailed;
        // Telemetry class
        let _ = PodFailureReason::UdpDataMissing;
        let _ = PodFailureReason::TelemetryInvalid;
        // Multiplayer class
        let _ = PodFailureReason::MultiplayerDesync;
        let _ = PodFailureReason::MultiplayerServerDisconnect;
        // PIN class
        let _ = PodFailureReason::PinValidationFailed;
        let _ = PodFailureReason::StaffUnlockNeeded;
        // Lap class
        let _ = PodFailureReason::LapCut;
        let _ = PodFailureReason::LapInvalidSpeed;
        let _ = PodFailureReason::LapSpin;

        // 3. Copy trait works — assign to variable twice without move error
        let r = PodFailureReason::WheelbaseDisconnected;
        let r2 = r; // Copy — no move
        assert_eq!(r, r2);
    }
}
