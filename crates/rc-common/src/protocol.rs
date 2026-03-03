use serde::{Deserialize, Serialize};

use crate::types::{
    AcLanSessionConfig, AcPresetSummary, AcServerInfo,
    AiDebugSuggestion, AuthTokenInfo, BillingSessionInfo, DrivingState, GameLaunchInfo,
    Leaderboard, LapData, PodInfo, SessionInfo, SimType, TelemetryFrame,
};

/// Messages sent from Pod Agent → Core Server
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
#[serde(rename_all = "snake_case")]
pub enum AgentMessage {
    /// Agent announces itself on the network
    Register(PodInfo),

    /// Periodic heartbeat with pod status
    Heartbeat(PodInfo),

    /// Real-time telemetry frame from simulator
    Telemetry(TelemetryFrame),

    /// A completed lap
    LapCompleted(LapData),

    /// Session state changed on the pod
    SessionUpdate(SessionInfo),

    /// Driving state change detected by HID/UDP monitoring
    DrivingStateUpdate { pod_id: String, state: DrivingState },

    /// Agent is shutting down
    Disconnect { pod_id: String },

    /// Agent reports game state change (launched, running, stopped, crashed)
    GameStateUpdate(GameLaunchInfo),

    /// Agent sends AI debug suggestion after analyzing a crash/error
    AiDebugResult(AiDebugSuggestion),

    /// Customer entered PIN on lock screen
    PinEntered { pod_id: String, pin: String },
}

/// Messages sent from Core Server → Pod Agent
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
#[serde(rename_all = "snake_case")]
pub enum CoreToAgentMessage {
    /// Acknowledge registration
    Registered { pod_id: String },

    /// Command to start a session on this pod
    StartSession(SessionInfo),

    /// Command to stop the current session
    StopSession { session_id: String },

    /// Update pod configuration
    Configure { config_json: String },

    /// Notify agent that a billing session started
    BillingStarted {
        billing_session_id: String,
        driver_name: String,
        allocated_seconds: u32,
    },

    /// Notify agent that billing session ended
    BillingStopped { billing_session_id: String },

    /// Command to launch a game on this pod
    LaunchGame {
        sim_type: SimType,
        launch_args: Option<String>,
    },

    /// Command to stop the currently running game
    StopGame,

    /// Show PIN entry lock screen on the pod
    ShowPinLockScreen {
        token_id: String,
        driver_name: String,
        pricing_tier_name: String,
        allocated_seconds: u32,
    },

    /// Show QR code lock screen on the pod
    ShowQrLockScreen {
        token_id: String,
        qr_payload: String,
        driver_name: String,
        pricing_tier_name: String,
        allocated_seconds: u32,
    },

    /// Clear/dismiss the lock screen
    ClearLockScreen,

    /// Billing timer tick — sent every second to update pod lock screen countdown
    BillingTick {
        remaining_seconds: u32,
        allocated_seconds: u32,
        driver_name: String,
    },
}

/// Messages sent from Core Server → Web Dashboard
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", content = "data")]
#[serde(rename_all = "snake_case")]
pub enum DashboardEvent {
    /// Full pod state update
    PodUpdate(PodInfo),

    /// All pods status (sent on connect)
    PodList(Vec<PodInfo>),

    /// Live telemetry for a specific pod
    Telemetry(TelemetryFrame),

    /// Updated leaderboard
    LeaderboardUpdate(Leaderboard),

    /// New lap completed
    LapCompleted(LapData),

    /// Session state changed
    SessionUpdate(SessionInfo),

    /// Billing timer tick (sent every 1s for active billing sessions)
    BillingTick(BillingSessionInfo),

    /// Billing session state changed (started, stopped, paused, etc.)
    BillingSessionChanged(BillingSessionInfo),

    /// All active billing sessions (sent on dashboard connect)
    BillingSessionList(Vec<BillingSessionInfo>),

    /// Time warning for a billing session
    BillingWarning {
        billing_session_id: String,
        pod_id: String,
        remaining_seconds: u32,
    },

    /// Game state changed on a pod
    GameStateChanged(GameLaunchInfo),

    /// AI debug suggestion for a game crash/error
    AiDebugSuggestion(AiDebugSuggestion),

    /// All active game sessions (sent on dashboard connect)
    GameSessionList(Vec<GameLaunchInfo>),

    /// AC server state changed (started, running, stopped, error)
    AcServerUpdate(AcServerInfo),

    /// AC preset loaded (response to LoadAcPreset command)
    AcPresetLoaded {
        preset_id: String,
        config: AcLanSessionConfig,
    },

    /// List of saved AC presets (sent on connect or after save/delete)
    AcPresetList(Vec<AcPresetSummary>),

    /// Auth token created (customer assignment pending)
    AuthTokenCreated(AuthTokenInfo),

    /// Auth token consumed (customer verified, billing started)
    AuthTokenConsumed {
        token_id: String,
        pod_id: String,
        billing_session_id: String,
    },

    /// Auth token cleared (expired or cancelled)
    AuthTokenCleared {
        token_id: String,
        pod_id: String,
        reason: String,
    },
}

/// Messages sent from Web Dashboard → Core Server
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "command", content = "data")]
#[serde(rename_all = "snake_case")]
pub enum DashboardCommand {
    /// Start a billing session on a pod
    StartBilling {
        pod_id: String,
        driver_id: String,
        pricing_tier_id: String,
        custom_price_paise: Option<u32>,
        custom_duration_minutes: Option<u32>,
    },

    /// Manually pause billing (staff-initiated)
    PauseBilling { billing_session_id: String },

    /// Resume manually paused billing
    ResumeBilling { billing_session_id: String },

    /// End billing session early
    EndBilling { billing_session_id: String },

    /// Cancel billing session (no charge)
    CancelBilling { billing_session_id: String },

    /// Extend a billing session's time
    ExtendBilling {
        billing_session_id: String,
        additional_seconds: u32,
    },

    /// Launch a game on a specific pod
    LaunchGame {
        pod_id: String,
        sim_type: SimType,
        launch_args: Option<String>,
    },

    /// Stop the game running on a specific pod
    StopGame { pod_id: String },

    /// Start an AC LAN server session and launch pods
    StartAcSession {
        config: AcLanSessionConfig,
        pod_ids: Vec<String>,
    },

    /// Stop the running AC LAN server session
    StopAcSession { session_id: String },

    /// Save an AC preset
    SaveAcPreset {
        name: String,
        config: AcLanSessionConfig,
    },

    /// Delete an AC preset
    DeleteAcPreset { preset_id: String },

    /// Load an AC preset (returns config via AcPresetLoaded event)
    LoadAcPreset { preset_id: String },

    /// Assign customer to a pod with PIN or QR auth
    AssignCustomer {
        pod_id: String,
        driver_id: String,
        pricing_tier_id: String,
        auth_type: String,
        custom_price_paise: Option<u32>,
        custom_duration_minutes: Option<u32>,
    },

    /// Cancel a pending customer assignment
    CancelAssignment { token_id: String },
}
