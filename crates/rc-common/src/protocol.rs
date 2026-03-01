use serde::{Deserialize, Serialize};

use crate::types::{
    BillingSessionInfo, DrivingState, Leaderboard, LapData, PodInfo, SessionInfo, TelemetryFrame,
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
}
