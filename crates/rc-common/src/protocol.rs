use serde::{Deserialize, Serialize};

use crate::types::{
    AcLanSessionConfig, AcPresetSummary, AcServerInfo, AcStatus,
    AiDebugSuggestion, AuthTokenInfo, BillingSessionInfo, ContentManifest, DeployState, DrivingState,
    GameLaunchInfo, GroupSessionInfo, Leaderboard, LapData, PodActivityEntry, PodInfo, SessionInfo,
    SimType, TelemetryFrame,
};

/// Summary of deploy state for a single pod — used in DeployStatusList
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployPodStatus {
    pub pod_id: String,
    pub state: DeployState,
    pub last_updated: String,
}

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

    /// Response to CoreToAgentMessage::Ping — carries same id back for round-trip measurement
    Pong { id: u64 },

    /// Agent reports AC shared memory STATUS change (Off/Replay/Live/Pause)
    GameStatusUpdate { pod_id: String, ac_status: AcStatus },

    /// Agent reports FFB safety action completed (zeroed wheelbase torque)
    FfbZeroed { pod_id: String },

    /// Agent reports game crash detected (process disappeared unexpectedly)
    GameCrashed { pod_id: String, billing_active: bool },

    /// Pod reports installed AC content at startup/reconnect
    ContentManifest(ContentManifest),

    /// Assist change confirmed (response to SetAssist)
    AssistChanged {
        pod_id: String,
        assist_type: String,
        enabled: bool,
        confirmed: bool,
    },

    /// FFB gain change confirmed (response to SetFfbGain)
    FfbGainChanged {
        pod_id: String,
        percent: u8,
    },

    /// Full assist state (response to QueryAssistState)
    AssistState {
        pod_id: String,
        abs: u8,
        tc: u8,
        auto_shifter: bool,
        ffb_percent: u8,
    },

    /// Result of a WebSocket exec command (response to CoreToAgentMessage::Exec)
    ExecResult {
        request_id: String,
        success: bool,
        exit_code: Option<i32>,
        stdout: String,
        stderr: String,
    },
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

    /// Billing timer tick — sent every second to update pod overlay
    BillingTick {
        remaining_seconds: u32,
        allocated_seconds: u32,
        driver_name: String,
        /// Elapsed driving seconds (count-up model). None for legacy countdown.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        elapsed_seconds: Option<u32>,
        /// Running cost in paise. None for legacy countdown.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cost_paise: Option<i64>,
        /// Current rate per minute in paise. None for legacy countdown.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rate_per_min_paise: Option<i64>,
        /// Whether billing is currently paused (game pause). None for legacy.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        paused: Option<bool>,
        /// Minutes remaining until next pricing tier kicks in. None if on final tier.
        #[serde(default, skip_serializing_if = "Option::is_none", alias = "minutes_to_value_tier")]
        minutes_to_next_tier: Option<u32>,
        /// Name of the current pricing tier (e.g. "Standard", "Extended"). None for legacy.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tier_name: Option<String>,
    },

    /// Sub-session ended (billing expired but pod has active reservation — multi-session)
    SubSessionEnded {
        billing_session_id: String,
        driver_name: String,
        total_laps: u32,
        best_lap_ms: Option<u32>,
        driving_seconds: u32,
        wallet_balance_paise: i64,
        current_split_number: u32,
        total_splits: u32,
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

    /// Change FFB strength preset (light/medium/strong) — rewrites controls.ini
    SetFfb {
        preset: String,
    },

    /// PIN validation failed — agent should show error on lock screen
    PinFailed {
        reason: String,
    },

    /// Kiosk settings updated — broadcast to all agents
    SettingsUpdated {
        settings: std::collections::HashMap<String, String>,
    },

    /// Show pause overlay on kiosk (disconnect detected, billing paused)
    ShowPauseOverlay {
        session_id: String,
        remaining_seconds: u32,
        pause_count: u32,
    },

    /// Hide pause overlay on kiosk (session resumed or ended)
    HidePauseOverlay {
        session_id: String,
    },

    /// Application-level ping for round-trip latency measurement — agent must respond with AgentMessage::Pong { id }
    Ping { id: u64 },

    /// Toggle a driving assist mid-session via SendInput (Phase 6)
    SetAssist {
        assist_type: String,
        enabled: bool,
    },

    /// Set FFB gain as percentage (10-100) via HID (Phase 6)
    SetFfbGain {
        percent: u8,
    },

    /// Query current assist state from agent (Phase 6)
    QueryAssistState,

    /// Run a shell command on this pod (remote exec via WebSocket)
    Exec {
        request_id: String,
        cmd: String,
        #[serde(default = "default_exec_timeout_ms")]
        timeout_ms: u64,
    },
}

fn default_exec_timeout_ms() -> u64 {
    10_000
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

    /// Billing session paused due to disconnect
    SessionPaused {
        pod_id: String,
        session_id: String,
        reason: String,
        pause_count: u32,
    },

    /// Billing session resumed from disconnect pause
    SessionResumed {
        pod_id: String,
        session_id: String,
    },

    /// Single pod activity entry (real-time, as it happens)
    PodActivity(PodActivityEntry),

    /// Batch of recent activity entries (sent on dashboard connect)
    PodActivityList(Vec<PodActivityEntry>),

    /// Watchdog initiated a restart for this pod
    PodRestarting {
        pod_id: String,
        attempt: u32,
        max_attempts: u32,
        backoff_label: String,
    },

    /// Watchdog is verifying restart success for this pod
    PodVerifying {
        pod_id: String,
        attempt: u32,
    },

    /// All restart attempts exhausted — manual intervention required
    PodRecoveryFailed {
        pod_id: String,
        attempt: u32,
        reason: String,
    },

    /// Deploy progress update for a pod (streamed during deploy sequence)
    DeployProgress {
        pod_id: String,
        state: DeployState,
        message: String,
        timestamp: String,
    },

    /// All pod deploy states (sent on dashboard connect, like PodList)
    DeployStatusList(Vec<DeployPodStatus>),
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
        split_count: Option<u32>,
        split_duration_minutes: Option<u32>,
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
        #[serde(default)]
        ai_level: Option<u32>,
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

    /// Deploy rc-agent binary to a single pod
    DeployPod {
        pod_id: String,
        binary_url: String,
    },

    /// Rolling deploy to all pods (Pod 8 canary first)
    DeployRolling {
        binary_url: String,
    },

    /// Cancel an in-progress deploy for a pod
    CancelDeploy {
        pod_id: String,
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
            elapsed_seconds: None,
            cost_paise: None,
            rate_per_min_paise: None,
            paused: None,
            minutes_to_next_tier: None,
            tier_name: None,
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

    #[test]
    fn test_core_to_agent_pause_overlay() {
        let msg = CoreToAgentMessage::ShowPauseOverlay {
            session_id: "sess-1".to_string(),
            remaining_seconds: 600,
            pause_count: 2,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("show_pause_overlay"));
        let parsed: CoreToAgentMessage = serde_json::from_str(&json).unwrap();
        if let CoreToAgentMessage::ShowPauseOverlay { session_id, remaining_seconds, pause_count } = parsed {
            assert_eq!(session_id, "sess-1");
            assert_eq!(remaining_seconds, 600);
            assert_eq!(pause_count, 2);
        } else {
            panic!("Wrong variant");
        }
    }

    #[test]
    fn test_dashboard_event_pod_restarting_roundtrip() {
        let event = DashboardEvent::PodRestarting {
            pod_id: "pod_1".to_string(),
            attempt: 2,
            max_attempts: 4,
            backoff_label: "2m".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("pod_restarting"), "Expected 'pod_restarting' in: {}", json);
        let parsed: DashboardEvent = serde_json::from_str(&json).unwrap();
        if let DashboardEvent::PodRestarting { pod_id, attempt, max_attempts, backoff_label } = parsed {
            assert_eq!(pod_id, "pod_1");
            assert_eq!(attempt, 2);
            assert_eq!(max_attempts, 4);
            assert_eq!(backoff_label, "2m");
        } else {
            panic!("Wrong variant after roundtrip");
        }
    }

    #[test]
    fn test_dashboard_event_pod_verifying_roundtrip() {
        let event = DashboardEvent::PodVerifying {
            pod_id: "pod_3".to_string(),
            attempt: 1,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("pod_verifying"), "Expected 'pod_verifying' in: {}", json);
        let parsed: DashboardEvent = serde_json::from_str(&json).unwrap();
        if let DashboardEvent::PodVerifying { pod_id, attempt } = parsed {
            assert_eq!(pod_id, "pod_3");
            assert_eq!(attempt, 1);
        } else {
            panic!("Wrong variant after roundtrip");
        }
    }

    #[test]
    fn test_dashboard_event_pod_recovery_failed_roundtrip() {
        let event = DashboardEvent::PodRecoveryFailed {
            pod_id: "pod_8".to_string(),
            attempt: 4,
            reason: "All restart attempts exhausted".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("pod_recovery_failed"), "Expected 'pod_recovery_failed' in: {}", json);
        let parsed: DashboardEvent = serde_json::from_str(&json).unwrap();
        if let DashboardEvent::PodRecoveryFailed { pod_id, attempt, reason } = parsed {
            assert_eq!(pod_id, "pod_8");
            assert_eq!(attempt, 4);
            assert_eq!(reason, "All restart attempts exhausted");
        } else {
            panic!("Wrong variant after roundtrip");
        }
    }

    #[test]
    fn test_core_to_agent_ping_roundtrip() {
        let msg = CoreToAgentMessage::Ping { id: 42 };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("ping"), "Expected 'ping' in: {}", json);
        let parsed: CoreToAgentMessage = serde_json::from_str(&json).unwrap();
        if let CoreToAgentMessage::Ping { id } = parsed {
            assert_eq!(id, 42);
        } else {
            panic!("Wrong variant after roundtrip: expected Ping");
        }
    }

    #[test]
    fn test_agent_pong_roundtrip() {
        let msg = AgentMessage::Pong { id: 99 };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("pong"), "Expected 'pong' in: {}", json);
        let parsed: AgentMessage = serde_json::from_str(&json).unwrap();
        if let AgentMessage::Pong { id } = parsed {
            assert_eq!(id, 99);
        } else {
            panic!("Wrong variant after roundtrip: expected Pong");
        }
    }

    #[test]
    fn test_dashboard_event_session_paused() {
        let event = DashboardEvent::SessionPaused {
            pod_id: "pod_1".to_string(),
            session_id: "sess-1".to_string(),
            reason: "disconnect".to_string(),
            pause_count: 1,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("session_paused"));
        let parsed: DashboardEvent = serde_json::from_str(&json).unwrap();
        if let DashboardEvent::SessionPaused { pod_id, session_id, reason, pause_count } = parsed {
            assert_eq!(pod_id, "pod_1");
            assert_eq!(session_id, "sess-1");
            assert_eq!(reason, "disconnect");
            assert_eq!(pause_count, 1);
        } else {
            panic!("Wrong variant");
        }
    }

    // ── Phase 03 Plan 01 tests ───────────────────────────────────────────

    #[test]
    fn test_game_status_update_roundtrip() {
        use crate::types::AcStatus;
        let msg = AgentMessage::GameStatusUpdate {
            pod_id: "pod_3".to_string(),
            ac_status: AcStatus::Live,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("game_status_update"), "Expected 'game_status_update' in: {}", json);
        assert!(json.contains("\"ac_status\":\"live\""));
        let parsed: AgentMessage = serde_json::from_str(&json).unwrap();
        if let AgentMessage::GameStatusUpdate { pod_id, ac_status } = parsed {
            assert_eq!(pod_id, "pod_3");
            assert_eq!(ac_status, AcStatus::Live);
        } else {
            panic!("Wrong variant after roundtrip");
        }
    }

    #[test]
    fn test_game_status_update_all_ac_statuses() {
        use crate::types::AcStatus;
        for status in [AcStatus::Off, AcStatus::Replay, AcStatus::Live, AcStatus::Pause] {
            let msg = AgentMessage::GameStatusUpdate {
                pod_id: "pod_1".to_string(),
                ac_status: status,
            };
            let json = serde_json::to_string(&msg).unwrap();
            let parsed: AgentMessage = serde_json::from_str(&json).unwrap();
            if let AgentMessage::GameStatusUpdate { ac_status, .. } = parsed {
                assert_eq!(ac_status, status, "Roundtrip failed for {:?}", status);
            } else {
                panic!("Wrong variant");
            }
        }
    }

    #[test]
    fn test_billing_tick_with_new_optional_fields() {
        let msg = CoreToAgentMessage::BillingTick {
            remaining_seconds: 9900,
            allocated_seconds: 10800,
            driver_name: "Test Driver".to_string(),
            elapsed_seconds: Some(900),
            cost_paise: Some(34950),
            rate_per_min_paise: Some(2500),
            paused: Some(false),
            minutes_to_next_tier: Some(15),
            tier_name: Some("Standard".to_string()),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"elapsed_seconds\":900"));
        assert!(json.contains("\"cost_paise\":34950"));
        assert!(json.contains("\"rate_per_min_paise\":2500"));
        assert!(json.contains("\"paused\":false"));
        assert!(json.contains("\"minutes_to_next_tier\":15"));
        assert!(json.contains("\"tier_name\":\"Standard\""));
        let parsed: CoreToAgentMessage = serde_json::from_str(&json).unwrap();
        if let CoreToAgentMessage::BillingTick {
            remaining_seconds, elapsed_seconds, cost_paise, rate_per_min_paise, paused, minutes_to_next_tier, tier_name, ..
        } = parsed {
            assert_eq!(remaining_seconds, 9900);
            assert_eq!(elapsed_seconds, Some(900));
            assert_eq!(cost_paise, Some(34950));
            assert_eq!(rate_per_min_paise, Some(2500));
            assert_eq!(paused, Some(false));
            assert_eq!(minutes_to_next_tier, Some(15));
            assert_eq!(tier_name, Some("Standard".to_string()));
        } else {
            panic!("Wrong variant");
        }
    }

    #[test]
    fn test_billing_tick_backward_compat_old_format() {
        // Old-format BillingTick without new fields should still deserialize
        let json = r#"{"type":"billing_tick","data":{"remaining_seconds":1500,"allocated_seconds":1800,"driver_name":"Test"}}"#;
        let parsed: CoreToAgentMessage = serde_json::from_str(json).unwrap();
        if let CoreToAgentMessage::BillingTick {
            remaining_seconds, allocated_seconds, driver_name,
            elapsed_seconds, cost_paise, rate_per_min_paise, paused, minutes_to_next_tier, tier_name,
        } = parsed {
            assert_eq!(remaining_seconds, 1500);
            assert_eq!(allocated_seconds, 1800);
            assert_eq!(driver_name, "Test");
            assert_eq!(elapsed_seconds, None);
            assert_eq!(cost_paise, None);
            assert_eq!(rate_per_min_paise, None);
            assert_eq!(paused, None);
            assert_eq!(minutes_to_next_tier, None);
            assert_eq!(tier_name, None);
        } else {
            panic!("Wrong variant");
        }
    }

    #[test]
    fn test_dashboard_event_deploy_progress_killing_roundtrip() {
        let event = DashboardEvent::DeployProgress {
            pod_id: "pod_8".to_string(),
            state: crate::types::DeployState::Killing,
            message: "Sending taskkill to rc-agent.exe".to_string(),
            timestamp: "2026-03-13T10:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("deploy_progress"), "Expected 'deploy_progress' in: {}", json);
        let parsed: DashboardEvent = serde_json::from_str(&json).unwrap();
        if let DashboardEvent::DeployProgress { pod_id, state, message, .. } = parsed {
            assert_eq!(pod_id, "pod_8");
            assert_eq!(state, crate::types::DeployState::Killing);
            assert_eq!(message, "Sending taskkill to rc-agent.exe");
        } else {
            panic!("Wrong variant after roundtrip");
        }
    }

    #[test]
    fn test_dashboard_event_deploy_progress_failed_roundtrip() {
        let event = DashboardEvent::DeployProgress {
            pod_id: "pod_3".to_string(),
            state: crate::types::DeployState::Failed { reason: "binary too small (1024 bytes)".to_string() },
            message: "Deploy failed: binary size check".to_string(),
            timestamp: "2026-03-13T10:05:00Z".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: DashboardEvent = serde_json::from_str(&json).unwrap();
        if let DashboardEvent::DeployProgress { state, .. } = parsed {
            assert!(matches!(state, crate::types::DeployState::Failed { .. }));
        } else {
            panic!("Wrong variant");
        }
    }

    #[test]
    fn test_dashboard_event_deploy_status_list_roundtrip() {
        let list = DashboardEvent::DeployStatusList(vec![
            DeployPodStatus {
                pod_id: "pod_1".to_string(),
                state: crate::types::DeployState::Idle,
                last_updated: "2026-03-13T10:00:00Z".to_string(),
            },
            DeployPodStatus {
                pod_id: "pod_8".to_string(),
                state: crate::types::DeployState::Complete,
                last_updated: "2026-03-13T10:01:00Z".to_string(),
            },
        ]);
        let json = serde_json::to_string(&list).unwrap();
        assert!(json.contains("deploy_status_list"), "Expected 'deploy_status_list' in: {}", json);
        let parsed: DashboardEvent = serde_json::from_str(&json).unwrap();
        if let DashboardEvent::DeployStatusList(statuses) = parsed {
            assert_eq!(statuses.len(), 2);
            assert_eq!(statuses[0].pod_id, "pod_1");
            assert_eq!(statuses[1].pod_id, "pod_8");
        } else {
            panic!("Wrong variant");
        }
    }

    #[test]
    fn test_dashboard_command_deploy_pod_roundtrip() {
        let cmd = DashboardCommand::DeployPod {
            pod_id: "pod_8".to_string(),
            binary_url: "http://192.168.31.27:9998/rc-agent.exe".to_string(),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains("deploy_pod"));
        let parsed: DashboardCommand = serde_json::from_str(&json).unwrap();
        if let DashboardCommand::DeployPod { pod_id, binary_url } = parsed {
            assert_eq!(pod_id, "pod_8");
            assert_eq!(binary_url, "http://192.168.31.27:9998/rc-agent.exe");
        } else {
            panic!("Wrong variant");
        }
    }

    #[test]
    fn test_dashboard_command_deploy_rolling_roundtrip() {
        let cmd = DashboardCommand::DeployRolling {
            binary_url: "http://192.168.31.27:9998/rc-agent.exe".to_string(),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains("deploy_rolling"));
        let parsed: DashboardCommand = serde_json::from_str(&json).unwrap();
        if let DashboardCommand::DeployRolling { binary_url } = parsed {
            assert_eq!(binary_url, "http://192.168.31.27:9998/rc-agent.exe");
        } else {
            panic!("Wrong variant");
        }
    }

    #[test]
    fn test_dashboard_command_cancel_deploy_roundtrip() {
        let cmd = DashboardCommand::CancelDeploy {
            pod_id: "pod_5".to_string(),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains("cancel_deploy"));
        let parsed: DashboardCommand = serde_json::from_str(&json).unwrap();
        if let DashboardCommand::CancelDeploy { pod_id } = parsed {
            assert_eq!(pod_id, "pod_5");
        } else {
            panic!("Wrong variant");
        }
    }

    // ── Phase 04 Plan 01: Safety enforcement protocol messages ──────────

    #[test]
    fn test_ffb_zeroed_roundtrip() {
        let msg = AgentMessage::FfbZeroed { pod_id: "pod-1".into() };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("ffb_zeroed"), "Expected 'ffb_zeroed' in: {}", json);
        let back: AgentMessage = serde_json::from_str(&json).unwrap();
        if let AgentMessage::FfbZeroed { pod_id } = back {
            assert_eq!(pod_id, "pod-1");
        } else {
            panic!("Wrong variant after roundtrip: expected FfbZeroed");
        }
    }

    #[test]
    fn test_game_crashed_roundtrip() {
        let msg = AgentMessage::GameCrashed { pod_id: "pod-1".into(), billing_active: true };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("game_crashed"), "Expected 'game_crashed' in: {}", json);
        let back: AgentMessage = serde_json::from_str(&json).unwrap();
        if let AgentMessage::GameCrashed { pod_id, billing_active } = back {
            assert_eq!(pod_id, "pod-1");
            assert!(billing_active);
        } else {
            panic!("Wrong variant after roundtrip: expected GameCrashed");
        }
    }

    // ── Phase 05 Plan 01: ContentManifest serde tests ────────────────────

    #[test]
    fn test_content_manifest_roundtrip() {
        use crate::types::{ContentManifest, CarManifestEntry, TrackManifestEntry, TrackConfigManifest};

        let manifest = ContentManifest {
            cars: vec![
                CarManifestEntry { id: "ks_ferrari_488_gt3".to_string() },
                CarManifestEntry { id: "ks_porsche_911_gt3_r".to_string() },
            ],
            tracks: vec![
                TrackManifestEntry {
                    id: "monza".to_string(),
                    configs: vec![TrackConfigManifest {
                        config: "".to_string(),
                        has_ai: true,
                        pit_count: Some(29),
                    }],
                },
                TrackManifestEntry {
                    id: "spa".to_string(),
                    configs: vec![
                        TrackConfigManifest {
                            config: "gp".to_string(),
                            has_ai: true,
                            pit_count: Some(40),
                        },
                        TrackConfigManifest {
                            config: "drift".to_string(),
                            has_ai: false,
                            pit_count: None,
                        },
                    ],
                },
            ],
        };
        let json = serde_json::to_string(&manifest).unwrap();
        let parsed: ContentManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.cars.len(), 2);
        assert_eq!(parsed.cars[0].id, "ks_ferrari_488_gt3");
        assert_eq!(parsed.cars[1].id, "ks_porsche_911_gt3_r");
        assert_eq!(parsed.tracks.len(), 2);
        assert_eq!(parsed.tracks[0].id, "monza");
        assert_eq!(parsed.tracks[0].configs.len(), 1);
        assert_eq!(parsed.tracks[0].configs[0].config, "");
        assert!(parsed.tracks[0].configs[0].has_ai);
        assert_eq!(parsed.tracks[0].configs[0].pit_count, Some(29));
        assert_eq!(parsed.tracks[1].id, "spa");
        assert_eq!(parsed.tracks[1].configs.len(), 2);
    }

    #[test]
    fn test_content_manifest_agent_message_wire_format() {
        use crate::types::{ContentManifest, CarManifestEntry, TrackManifestEntry, TrackConfigManifest};

        let manifest = ContentManifest {
            cars: vec![CarManifestEntry { id: "bmw_z4_gt3".to_string() }],
            tracks: vec![TrackManifestEntry {
                id: "imola".to_string(),
                configs: vec![TrackConfigManifest {
                    config: "".to_string(),
                    has_ai: true,
                    pit_count: Some(24),
                }],
            }],
        };
        let msg = AgentMessage::ContentManifest(manifest);
        let json = serde_json::to_string(&msg).unwrap();
        // Verify wire format: {"type":"content_manifest","data":{...}}
        assert!(json.contains("\"type\":\"content_manifest\""), "Expected type=content_manifest in: {}", json);
        assert!(json.contains("\"data\":{"), "Expected data field in: {}", json);
        // Roundtrip
        let parsed: AgentMessage = serde_json::from_str(&json).unwrap();
        if let AgentMessage::ContentManifest(m) = parsed {
            assert_eq!(m.cars.len(), 1);
            assert_eq!(m.cars[0].id, "bmw_z4_gt3");
            assert_eq!(m.tracks.len(), 1);
            assert_eq!(m.tracks[0].id, "imola");
            assert!(m.tracks[0].configs[0].has_ai);
            assert_eq!(m.tracks[0].configs[0].pit_count, Some(24));
        } else {
            panic!("Wrong variant after roundtrip: expected ContentManifest");
        }
    }

    #[test]
    fn test_content_manifest_agent_message_roundtrip() {
        use crate::types::{ContentManifest, CarManifestEntry, TrackManifestEntry, TrackConfigManifest};

        let manifest = ContentManifest {
            cars: vec![
                CarManifestEntry { id: "ks_audi_r8_lms".to_string() },
                CarManifestEntry { id: "ks_lamborghini_huracan_gt3".to_string() },
                CarManifestEntry { id: "ks_mclaren_650s_gt3".to_string() },
            ],
            tracks: vec![
                TrackManifestEntry {
                    id: "nurburgring".to_string(),
                    configs: vec![
                        TrackConfigManifest { config: "gp".to_string(), has_ai: true, pit_count: Some(30) },
                        TrackConfigManifest { config: "nordschleife".to_string(), has_ai: true, pit_count: Some(30) },
                        TrackConfigManifest { config: "sprint".to_string(), has_ai: false, pit_count: None },
                    ],
                },
            ],
        };
        let msg = AgentMessage::ContentManifest(manifest);
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: AgentMessage = serde_json::from_str(&json).unwrap();
        if let AgentMessage::ContentManifest(m) = parsed {
            assert_eq!(m.cars.len(), 3);
            assert_eq!(m.tracks.len(), 1);
            assert_eq!(m.tracks[0].configs.len(), 3);
            assert_eq!(m.tracks[0].configs[0].config, "gp");
            assert!(m.tracks[0].configs[0].has_ai);
            assert_eq!(m.tracks[0].configs[0].pit_count, Some(30));
            assert_eq!(m.tracks[0].configs[2].config, "sprint");
            assert!(!m.tracks[0].configs[2].has_ai);
            assert_eq!(m.tracks[0].configs[2].pit_count, None);
        } else {
            panic!("Wrong variant after roundtrip");
        }
    }

    #[test]
    fn test_content_manifest_track_config_with_ai_and_pits_roundtrip() {
        use crate::types::TrackConfigManifest;

        let config = TrackConfigManifest {
            config: "gp".to_string(),
            has_ai: true,
            pit_count: Some(15),
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: TrackConfigManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.config, "gp");
        assert!(parsed.has_ai);
        assert_eq!(parsed.pit_count, Some(15));
    }

    #[test]
    fn test_content_manifest_track_config_no_ai_no_pits_roundtrip() {
        use crate::types::TrackConfigManifest;

        let config = TrackConfigManifest {
            config: "drift".to_string(),
            has_ai: false,
            pit_count: None,
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: TrackConfigManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.config, "drift");
        assert!(!parsed.has_ai);
        assert_eq!(parsed.pit_count, None);
    }

    #[test]
    fn test_content_manifest_empty_roundtrip() {
        use crate::types::ContentManifest;

        let manifest = ContentManifest {
            cars: vec![],
            tracks: vec![],
        };
        let msg = AgentMessage::ContentManifest(manifest);
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: AgentMessage = serde_json::from_str(&json).unwrap();
        if let AgentMessage::ContentManifest(m) = parsed {
            assert!(m.cars.is_empty());
            assert!(m.tracks.is_empty());
        } else {
            panic!("Wrong variant after roundtrip: expected ContentManifest");
        }
    }

    // ── Phase 06 Plan 01: Mid-session control protocol messages ─────────

    #[test]
    fn test_mid_session_set_assist_roundtrip() {
        let msg = CoreToAgentMessage::SetAssist {
            assist_type: "abs".to_string(),
            enabled: true,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("set_assist"), "Expected 'set_assist' in: {}", json);
        let parsed: CoreToAgentMessage = serde_json::from_str(&json).unwrap();
        if let CoreToAgentMessage::SetAssist { assist_type, enabled } = parsed {
            assert_eq!(assist_type, "abs");
            assert!(enabled);
        } else {
            panic!("Wrong variant after roundtrip: expected SetAssist");
        }
    }

    #[test]
    fn test_mid_session_set_assist_tc_off() {
        let msg = CoreToAgentMessage::SetAssist {
            assist_type: "tc".to_string(),
            enabled: false,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: CoreToAgentMessage = serde_json::from_str(&json).unwrap();
        if let CoreToAgentMessage::SetAssist { assist_type, enabled } = parsed {
            assert_eq!(assist_type, "tc");
            assert!(!enabled);
        } else {
            panic!("Wrong variant");
        }
    }

    #[test]
    fn test_mid_session_set_assist_transmission() {
        let msg = CoreToAgentMessage::SetAssist {
            assist_type: "transmission".to_string(),
            enabled: true,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: CoreToAgentMessage = serde_json::from_str(&json).unwrap();
        if let CoreToAgentMessage::SetAssist { assist_type, enabled } = parsed {
            assert_eq!(assist_type, "transmission");
            assert!(enabled);
        } else {
            panic!("Wrong variant");
        }
    }

    #[test]
    fn test_mid_session_set_ffb_gain_roundtrip() {
        let msg = CoreToAgentMessage::SetFfbGain { percent: 85 };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("set_ffb_gain"), "Expected 'set_ffb_gain' in: {}", json);
        let parsed: CoreToAgentMessage = serde_json::from_str(&json).unwrap();
        if let CoreToAgentMessage::SetFfbGain { percent } = parsed {
            assert_eq!(percent, 85);
        } else {
            panic!("Wrong variant after roundtrip: expected SetFfbGain");
        }
    }

    #[test]
    fn test_mid_session_query_assist_state_roundtrip() {
        let msg = CoreToAgentMessage::QueryAssistState;
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("query_assist_state"), "Expected 'query_assist_state' in: {}", json);
        let parsed: CoreToAgentMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, CoreToAgentMessage::QueryAssistState));
    }

    #[test]
    fn test_mid_session_assist_changed_roundtrip() {
        let msg = AgentMessage::AssistChanged {
            pod_id: "pod_1".to_string(),
            assist_type: "abs".to_string(),
            enabled: false,
            confirmed: true,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("assist_changed"), "Expected 'assist_changed' in: {}", json);
        let parsed: AgentMessage = serde_json::from_str(&json).unwrap();
        if let AgentMessage::AssistChanged { pod_id, assist_type, enabled, confirmed } = parsed {
            assert_eq!(pod_id, "pod_1");
            assert_eq!(assist_type, "abs");
            assert!(!enabled);
            assert!(confirmed);
        } else {
            panic!("Wrong variant after roundtrip: expected AssistChanged");
        }
    }

    #[test]
    fn test_mid_session_ffb_gain_changed_roundtrip() {
        let msg = AgentMessage::FfbGainChanged {
            pod_id: "pod_3".to_string(),
            percent: 70,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("ffb_gain_changed"), "Expected 'ffb_gain_changed' in: {}", json);
        let parsed: AgentMessage = serde_json::from_str(&json).unwrap();
        if let AgentMessage::FfbGainChanged { pod_id, percent } = parsed {
            assert_eq!(pod_id, "pod_3");
            assert_eq!(percent, 70);
        } else {
            panic!("Wrong variant after roundtrip: expected FfbGainChanged");
        }
    }

    #[test]
    fn test_mid_session_assist_state_roundtrip() {
        let msg = AgentMessage::AssistState {
            pod_id: "pod_5".to_string(),
            abs: 2,
            tc: 0,
            auto_shifter: true,
            ffb_percent: 85,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("assist_state"), "Expected 'assist_state' in: {}", json);
        let parsed: AgentMessage = serde_json::from_str(&json).unwrap();
        if let AgentMessage::AssistState { pod_id, abs, tc, auto_shifter, ffb_percent } = parsed {
            assert_eq!(pod_id, "pod_5");
            assert_eq!(abs, 2);
            assert_eq!(tc, 0);
            assert!(auto_shifter);
            assert_eq!(ffb_percent, 85);
        } else {
            panic!("Wrong variant after roundtrip: expected AssistState");
        }
    }

    #[test]
    fn test_mid_session_set_ffb_gain_boundary_values() {
        // Test min (10%) and max (100%) gain values
        for percent in [10u8, 50, 100] {
            let msg = CoreToAgentMessage::SetFfbGain { percent };
            let json = serde_json::to_string(&msg).unwrap();
            let parsed: CoreToAgentMessage = serde_json::from_str(&json).unwrap();
            if let CoreToAgentMessage::SetFfbGain { percent: p } = parsed {
                assert_eq!(p, percent);
            } else {
                panic!("Wrong variant for percent={}", percent);
            }
        }
    }
}
