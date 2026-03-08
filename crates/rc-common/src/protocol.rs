use serde::{Deserialize, Serialize};

use crate::types::{
    AcLanSessionConfig, AcPresetSummary, AcServerInfo,
    AiDebugSuggestion, AuthTokenInfo, BillingSessionInfo, DrivingState, GameLaunchInfo,
    GroupSessionInfo, Leaderboard, LapData, PodInfo, SessionInfo, SimType, TelemetryFrame,
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

    /// Session ended — agent should stop game, show summary, then return to idle
    SessionEnded {
        billing_session_id: String,
        driver_name: String,
        total_laps: u32,
        best_lap_ms: Option<u32>,
        driving_seconds: u32,
    },

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

    /// Blank the screen (show black screen)
    BlankScreen,

    /// Billing timer tick — sent every second to update pod lock screen countdown
    BillingTick {
        remaining_seconds: u32,
        allocated_seconds: u32,
        driver_name: String,
    },

    /// Sub-session ended (billing expired but pod has active reservation — multi-session)
    SubSessionEnded {
        billing_session_id: String,
        driver_name: String,
        total_laps: u32,
        best_lap_ms: Option<u32>,
        driving_seconds: u32,
        wallet_balance_paise: i64,
    },

    /// Show assistance screen (for games without auto-spawn, e.g. F1 25)
    ShowAssistanceScreen {
        driver_name: String,
        message: String,
    },

    /// Enter debug/maintenance mode (employee access — allow Content Manager, no billing)
    EnterDebugMode {
        employee_name: String,
    },

    /// Change transmission (auto/manual) mid-session — rewrites race.ini
    SetTransmission {
        transmission: String,
    },

    /// Kiosk settings updated — broadcast to all agents
    SettingsUpdated {
        settings: std::collections::HashMap<String, String>,
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

    /// Pod reservation state changed
    PodReservationChanged {
        reservation_id: String,
        driver_id: String,
        pod_id: String,
        status: String,
    },

    /// A pod needs staff assistance (F1 25 or non-auto-spawn games)
    AssistanceNeeded {
        pod_id: String,
        driver_name: String,
        game: String,
        reason: String,
    },

    /// Camera focus recommendation from automated camera controller
    CameraFocusUpdate {
        pod_id: String,
        driver_name: String,
        reason: String,
    },

    /// AI-to-AI message (Bono ↔ James) — visible in admin dashboard
    AiMessage {
        id: String,
        sender: String,
        recipient: String,
        content: String,
        message_type: String,
        created_at: String,
    },

    /// Multiplayer group session created
    GroupSessionCreated(GroupSessionInfo),

    /// Group session member status changed
    GroupMemberUpdate {
        group_session_id: String,
        driver_id: String,
        status: String,
        pod_id: Option<String>,
    },

    /// All group members validated — AC LAN starting
    GroupSessionAllValidated {
        group_session_id: String,
        ac_session_id: String,
        pod_ids: Vec<String>,
    },
}

/// Messages on the AI ↔ AI WebSocket channel (Bono ↔ James)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
#[serde(rename_all = "snake_case")]
pub enum AiChannelMessage {
    /// Authenticate on connect
    Auth { secret: String, identity: String },

    /// Server acknowledges auth
    AuthOk { identity: String },

    /// Auth failed
    AuthFailed { reason: String },

    /// A message from one AI to another
    Message {
        id: String,
        sender: String,
        content: String,
        message_type: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        metadata: Option<serde_json::Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        in_reply_to: Option<String>,
        created_at: String,
    },

    /// Acknowledge receipt of a message
    Ack { message_id: String },

    /// Mark message as read
    MarkRead { message_id: String },

    /// Keepalive ping
    Ping,

    /// Keepalive pong
    Pong,
}

/// Actions pushed from cloud → local rc-core via action queue.
/// Cloud inserts these; rc-core polls and processes them every 3 seconds.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action_type", content = "payload")]
#[serde(rename_all = "snake_case")]
pub enum CloudAction {
    /// Customer booked a session via PWA
    BookingCreated {
        booking_id: String,
        driver_id: String,
        pricing_tier_id: String,
        experience_id: Option<String>,
        pod_id: Option<String>,
    },
    /// Customer topped up wallet via PWA
    WalletTopUp {
        driver_id: String,
        amount_paise: i64,
        transaction_id: String,
    },
    /// Customer cancelled a booking via PWA
    BookingCancelled {
        booking_id: String,
    },
    /// Customer confirmed QR code on PWA
    QrConfirmed {
        token_id: String,
        driver_id: String,
    },
    /// Admin changed a setting via cloud dashboard
    SettingsChanged {
        key: String,
        value: String,
    },
    /// Push notification to staff/pod
    Notification {
        title: String,
        body: String,
        target: String,
    },
}

/// Wrapper for a pending cloud action with its ID and timestamp.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingCloudAction {
    pub id: String,
    pub action: CloudAction,
    pub created_at: String,
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
        staff_id: Option<String>,
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

    /// Acknowledge staff assistance notification
    AcknowledgeAssistance { pod_id: String },

    /// Set camera control mode
    SetCameraMode {
        mode: String,
        enabled: Option<bool>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_cloud_action_booking_roundtrip() {
        let action = CloudAction::BookingCreated {
            booking_id: "book-123".to_string(),
            driver_id: "drv-456".to_string(),
            pricing_tier_id: "tier-30min".to_string(),
            experience_id: Some("exp-nurburgring".to_string()),
            pod_id: Some("pod_3".to_string()),
        };
        let json = serde_json::to_string(&action).unwrap();
        assert!(json.contains("booking_created"));
        let parsed: CloudAction = serde_json::from_str(&json).unwrap();
        if let CloudAction::BookingCreated { booking_id, .. } = parsed {
            assert_eq!(booking_id, "book-123");
        } else {
            panic!("Wrong variant");
        }
    }

    #[test]
    fn test_cloud_action_wallet_roundtrip() {
        let action = CloudAction::WalletTopUp {
            driver_id: "drv-1".to_string(),
            amount_paise: 90000,
            transaction_id: "txn-abc".to_string(),
        };
        let json = serde_json::to_string(&action).unwrap();
        let parsed: CloudAction = serde_json::from_str(&json).unwrap();
        if let CloudAction::WalletTopUp { amount_paise, .. } = parsed {
            assert_eq!(amount_paise, 90000);
        } else {
            panic!("Wrong variant");
        }
    }

    #[test]
    fn test_cloud_action_notification() {
        let action = CloudAction::Notification {
            title: "New Booking".to_string(),
            body: "Customer booked Pod 3".to_string(),
            target: "dashboard".to_string(),
        };
        let json = serde_json::to_string(&action).unwrap();
        assert!(json.contains("notification"));
        let parsed: CloudAction = serde_json::from_str(&json).unwrap();
        if let CloudAction::Notification { title, .. } = parsed {
            assert_eq!(title, "New Booking");
        } else {
            panic!("Wrong variant");
        }
    }

    #[test]
    fn test_pending_cloud_action_serde() {
        let pending = PendingCloudAction {
            id: "act-1".to_string(),
            action: CloudAction::BookingCancelled {
                booking_id: "book-999".to_string(),
            },
            created_at: "2026-03-07T12:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&pending).unwrap();
        let parsed: PendingCloudAction = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, "act-1");
        if let CloudAction::BookingCancelled { booking_id } = parsed.action {
            assert_eq!(booking_id, "book-999");
        } else {
            panic!("Wrong variant");
        }
    }

    #[test]
    fn test_agent_message_roundtrip() {
        let msg = AgentMessage::PinEntered {
            pod_id: "pod_1".to_string(),
            pin: "1234".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("pin_entered"));
        let parsed: AgentMessage = serde_json::from_str(&json).unwrap();
        if let AgentMessage::PinEntered { pod_id, pin } = parsed {
            assert_eq!(pod_id, "pod_1");
            assert_eq!(pin, "1234");
        } else {
            panic!("Wrong variant");
        }
    }

    #[test]
    fn test_core_to_agent_billing_tick() {
        let msg = CoreToAgentMessage::BillingTick {
            remaining_seconds: 1500,
            allocated_seconds: 1800,
            driver_name: "Test Driver".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("billing_tick"));
        let parsed: CoreToAgentMessage = serde_json::from_str(&json).unwrap();
        if let CoreToAgentMessage::BillingTick { remaining_seconds, .. } = parsed {
            assert_eq!(remaining_seconds, 1500);
        } else {
            panic!("Wrong variant");
        }
    }

    #[test]
    fn test_cloud_action_qr_confirmed() {
        let action = CloudAction::QrConfirmed {
            token_id: "tok-abc".to_string(),
            driver_id: "drv-xyz".to_string(),
        };
        let json = serde_json::to_string(&action).unwrap();
        assert!(json.contains("qr_confirmed"));
        let parsed: CloudAction = serde_json::from_str(&json).unwrap();
        if let CloudAction::QrConfirmed { token_id, driver_id } = parsed {
            assert_eq!(token_id, "tok-abc");
            assert_eq!(driver_id, "drv-xyz");
        } else {
            panic!("Wrong variant");
        }
    }

    #[test]
    fn test_cloud_action_settings_changed() {
        let action = CloudAction::SettingsChanged {
            key: "screen_blanking_enabled".to_string(),
            value: "true".to_string(),
        };
        let json = serde_json::to_string(&action).unwrap();
        let parsed: CloudAction = serde_json::from_str(&json).unwrap();
        if let CloudAction::SettingsChanged { key, value } = parsed {
            assert_eq!(key, "screen_blanking_enabled");
            assert_eq!(value, "true");
        } else {
            panic!("Wrong variant");
        }
    }
}
