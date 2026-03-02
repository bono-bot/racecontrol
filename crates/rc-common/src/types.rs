use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ─── Sim Types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SimType {
    AssettocCorsa,
    IRacing,
    LeMansUltimate,
    F125,
    Forza,
}

impl std::fmt::Display for SimType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SimType::AssettocCorsa => write!(f, "Assetto Corsa"),
            SimType::IRacing => write!(f, "iRacing"),
            SimType::LeMansUltimate => write!(f, "Le Mans Ultimate"),
            SimType::F125 => write!(f, "F1 25"),
            SimType::Forza => write!(f, "Forza Motorsport"),
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodInfo {
    pub id: String,
    pub number: u32,
    pub name: String,
    pub ip_address: String,
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

// ─── Billing ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BillingSessionStatus {
    Pending,
    Active,
    PausedManual,
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
