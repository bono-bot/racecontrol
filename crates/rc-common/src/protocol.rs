use serde::{Deserialize, Serialize};

use crate::types::{
    AcLanSessionConfig, AcPresetSummary, AcServerInfo, AcStatus,
    AiDebugSuggestion, AuthTokenInfo, BillingSessionInfo, ContentManifest, DeployState, DrivingState,
    GameLaunchInfo, GroupSessionInfo, Leaderboard, LapData, MachineWhitelist, PodActivityEntry,
    PodInfo, PodFailureReason, ProcessViolation, SessionInfo, SimType, TelemetryFrame,
    FlagSyncPayload, ConfigPushPayload, OtaDownloadPayload, OtaAckPayload, ConfigAckPayload,
    KillSwitchPayload, FlagCacheSyncPayload,
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

    /// Response to CoreToAgentMessage::Ping — carries same id back for round-trip measurement.
    /// `agent_delay_us` reports how long the agent's event loop took to process the Ping
    /// (measured on the pod side via Instant). High values indicate a blocked async runtime.
    Pong { id: u64, #[serde(default)] agent_delay_us: Option<u64> },

    /// Agent reports AC shared memory STATUS change (Off/Replay/Live/Pause)
    GameStatusUpdate {
        pod_id: String,
        ac_status: AcStatus,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sim_type: Option<SimType>,
    },

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

    /// Agent reports startup status after connecting (HEAL-02)
    StartupReport {
        pod_id: String,
        version: String,
        uptime_secs: u64,
        config_hash: String,
        crash_recovery: bool,
        repairs: Vec<String>,
        /// Phase 46: lock screen HTTP server (:18923) bound successfully
        #[serde(default)]
        lock_screen_port_bound: bool,
        /// Phase 46: remote ops HTTP server (:8090) bound successfully
        #[serde(default)]
        remote_ops_port_bound: bool,
        /// Phase 46: OpenFFBoard HID device detected at startup
        #[serde(default)]
        hid_detected: bool,
        /// Phase 46: UDP telemetry ports successfully bound
        #[serde(default)]
        udp_ports_bound: Vec<u16>,
        /// Phase 50: Startup self-test verdict (HEALTHY/DEGRADED/CRITICAL). None if not yet implemented.
        #[serde(default)]
        startup_self_test_verdict: Option<String>,
        /// Phase 50: Number of failed probes at startup (0 = all pass)
        #[serde(default)]
        startup_probe_failures: u8,
    },

    /// Agent detected a hardware failure (USB disconnect, FFB fault)
    HardwareFailure {
        pod_id: String,
        reason: PodFailureReason,
        detail: String,
    },

    /// Agent detected telemetry gap (no UDP data for N seconds while billing active)
    TelemetryGap {
        pod_id: String,
        sim_type: SimType,
        gap_seconds: u32,
    },

    /// Agent detected billing anomaly (stuck session, idle drift, game dead + billing alive)
    BillingAnomaly {
        pod_id: String,
        billing_session_id: String,
        reason: PodFailureReason,
        detail: String,
    },

    /// Agent flagged an invalid lap at capture time
    LapFlagged {
        pod_id: String,
        lap_id: String,
        reason: PodFailureReason,
        detail: String,
    },

    /// Agent detected multiplayer session failure (desync or server disconnect)
    MultiplayerFailure {
        pod_id: String,
        reason: PodFailureReason,
        session_id: Option<String>,
    },

    /// Agent detected a persistent unknown process and temporarily allowed it.
    /// Server should approve or reject within the TTL window.
    ProcessApprovalRequest {
        pod_id: String,
        process_name: String,
        exe_path: String,
        sighting_count: u32,
    },

    /// Agent locked down the kiosk after a process was rejected or approval timed out.
    KioskLockdown {
        pod_id: String,
        reason: String,
    },

    /// Agent auto-ended an orphaned billing session (SESSION-01).
    /// Fired when billing_active=true + game_pid=None for >= auto_end_orphan_session_secs.
    SessionAutoEnded {
        pod_id: String,
        billing_session_id: String,
        /// "orphan_timeout" or "crash_limit"
        reason: String,
    },

    /// Agent paused billing during crash recovery (SESSION-03).
    BillingPaused {
        pod_id: String,
        billing_session_id: String,
    },

    /// Agent resumed billing after successful game relaunch (SESSION-03).
    BillingResumed {
        pod_id: String,
        billing_session_id: String,
    },

    /// Phase 50: Agent returns self-test probe results (response to RunSelfTest)
    SelfTestResult {
        pod_id: String,
        request_id: String,
        /// Serialized SelfTestReport — avoids protocol crate needing self_test types
        report: serde_json::Value,
    },

    /// Phase 97: Pre-flight checks passed before session start.
    PreFlightPassed {
        pod_id: String,
    },

    /// Phase 97: Pre-flight checks failed after auto-fix attempt.
    PreFlightFailed {
        pod_id: String,
        failures: Vec<String>,
        timestamp: String,
    },

    /// Phase 138: Idle health checks failed after N consecutive ticks (no billing session active).
    /// Sent after IDLE_HEALTH_HYSTERESIS_THRESHOLD (3) consecutive failures.
    /// Checks: lock_screen_http (port 18923) + window_rect (Edge covering >=90% screen).
    IdleHealthFailed {
        pod_id: String,
        /// Names of checks that failed on the most recent tick (e.g. "lock_screen_http", "window_rect").
        failures: Vec<String>,
        /// How many consecutive ticks have failed (always >= 3 when this message is sent).
        consecutive_count: u32,
        /// ISO-8601 UTC timestamp of this failure event.
        timestamp: String,
    },

    /// Phase 101: Pod reports a process/port/autostart whitelist violation.
    /// Sent immediately on detection (report-only mode) or after enforcement action (kill mode).
    /// `consecutive_count` = 1 on first sighting, increments each scan cycle.
    ProcessViolation(ProcessViolation),

    /// Phase 101: Pod sends periodic guard health summary (once per scan cycle).
    /// Allows server to detect if guard has stopped running on a pod.
    ProcessGuardStatus {
        pod_id: String,
        /// Total scans completed since rc-agent started.
        scan_count: u64,
        /// Total violations detected (including report-only) since rc-agent started.
        violation_count_total: u64,
        /// Violations in the last scan cycle (0 = clean).
        violation_count_last_scan: u32,
        /// ISO 8601 UTC timestamp of the most recent completed scan.
        last_scan_at: String,
        /// Guard is active and scanning (false if disabled in config).
        guard_active: bool,
    },

    /// Freedom mode process monitoring report — sent periodically while freedom mode is active.
    /// Lists all non-system processes running on the pod for audit trail.
    FreedomModeReport {
        pod_id: String,
        /// Non-system processes: vec of (process_name, exe_path)
        processes: Vec<(String, String)>,
        /// Detected game executables currently running
        games_detected: Vec<String>,
    },

    /// v22.0: Agent acknowledges OTA download completion or failure
    OtaAck(OtaAckPayload),

    /// v22.0: Agent acknowledges config push receipt
    ConfigAck(ConfigAckPayload),

    /// v22.0: Agent requests full flag state from server (sent on reconnect with cached version)
    FlagCacheSync(FlagCacheSyncPayload),

    /// Forward-compatibility: catch-all for message types added in newer server versions.
    /// Older agents silently ignore these instead of crashing on deserialization.
    #[serde(other)]
    Unknown,
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
        /// Session-scoped kiosk unlock token (SESS-04)
        #[serde(default)]
        session_token: Option<String>,
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

    /// Enter freedom mode (unrestricted pod access)
    EnterFreedomMode,

    /// Exit freedom mode (return to normal restrictions)
    ExitFreedomMode,

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

    /// Approve a temporarily-allowed process (add to permanent learned allowlist)
    ApproveProcess {
        process_name: String,
    },

    /// Reject a temporarily-allowed process (kill it + lock kiosk with staff message)
    RejectProcess {
        process_name: String,
    },

    /// Phase 50: Command agent to run all self-test probes and return results via SelfTestResult
    RunSelfTest {
        request_id: String,
    },

    /// Phase 68: Command agent to switch its WebSocket target URL at runtime.
    /// Agent reconnects to target_url on the next reconnect iteration without restarting.
    /// self_monitor will suppress WS-dead relaunch for 60s after receiving this.
    SwitchController {
        target_url: String,
    },

    /// Phase 97: Server clears MaintenanceRequired state on a pod (handler in Phase 98).
    ClearMaintenance,

    /// Phase 139: Server instructs agent to close all Edge browser processes and relaunch
    /// the lock screen browser. Sent by pod_healer when HTTP check fails but WS is alive.
    /// Agent must gate this on billing_active — do NOT relaunch during an active session.
    ForceRelaunchBrowser { pod_id: String },

    /// Phase 101: Server pushes an updated whitelist to all connected pods.
    /// Agent replaces its in-memory whitelist on receipt — no reconnect needed.
    UpdateProcessWhitelist {
        whitelist: MachineWhitelist,
    },

    /// v22.0: Server pushes feature flag state to agent
    FlagSync(FlagSyncPayload),

    /// v22.0: Server pushes config changes to agent
    ConfigPush(ConfigPushPayload),

    /// v22.0: Server instructs agent to download new binary via OTA
    OtaDownload(OtaDownloadPayload),

    /// v22.0: Server pushes kill switch activation/deactivation
    KillSwitch(KillSwitchPayload),

    /// Forward-compatibility: catch-all for message types added in newer server versions.
    /// Older agents silently ignore these instead of crashing on deserialization.
    #[serde(other)]
    Unknown,
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

    /// Fleet-wide rolling deploy completed -- summary of per-pod outcomes
    FleetDeploySummary {
        succeeded: Vec<String>,
        failed: Vec<String>,
        waiting: Vec<String>,
        timestamp: String,
    },

    /// Customer requested a game launch from PWA -- staff must confirm before launch
    GameLaunchRequested {
        pod_id: String,
        sim_type: SimType,
        driver_name: String,
        request_id: String,
    },

    /// Personal best achieved during a session (broadcast for real-time PWA notification)
    PbAchieved {
        driver_id: String,
        session_id: String,
        track: String,
        car: String,
        lap_time_ms: i64,
        lap_id: String,
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

/// Actions pushed from cloud → local racecontrol via action queue.
/// Cloud inserts these; racecontrol polls and processes them every 3 seconds.
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
        let msg = AgentMessage::Pong { id: 99, agent_delay_us: Some(1234) };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("pong"), "Expected 'pong' in: {}", json);
        let parsed: AgentMessage = serde_json::from_str(&json).unwrap();
        if let AgentMessage::Pong { id, agent_delay_us } = parsed {
            assert_eq!(id, 99);
            assert_eq!(agent_delay_us, Some(1234));
        } else {
            panic!("Wrong variant after roundtrip: expected Pong");
        }
    }

    #[test]
    fn test_agent_pong_backwards_compat() {
        // Old agents send Pong without agent_delay_us — must still parse
        let json = r#"{"type":"pong","data":{"id":42}}"#;
        let parsed: AgentMessage = serde_json::from_str(json).unwrap();
        if let AgentMessage::Pong { id, agent_delay_us } = parsed {
            assert_eq!(id, 42);
            assert_eq!(agent_delay_us, None);
        } else {
            panic!("Wrong variant: expected Pong");
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
            sim_type: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("game_status_update"), "Expected 'game_status_update' in: {}", json);
        assert!(json.contains("\"ac_status\":\"live\""));
        let parsed: AgentMessage = serde_json::from_str(&json).unwrap();
        if let AgentMessage::GameStatusUpdate { pod_id, ac_status, .. } = parsed {
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
                sim_type: None,
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
    fn test_billing_tick_old_field_alias() {
        // PROTOC-01: Old rc-agent versions send "minutes_to_value_tier" — the serde alias
        // must accept this key and populate minutes_to_next_tier.
        let json = r#"{"type":"billing_tick","data":{"remaining_seconds":1200,"allocated_seconds":1800,"driver_name":"AliasTest","minutes_to_value_tier":15,"tier_name":"Standard"}}"#;
        let parsed: CoreToAgentMessage = serde_json::from_str(json).unwrap();
        if let CoreToAgentMessage::BillingTick {
            remaining_seconds,
            minutes_to_next_tier,
            tier_name,
            ..
        } = parsed
        {
            assert_eq!(remaining_seconds, 1200);
            // KEY: "minutes_to_value_tier" in JSON maps to minutes_to_next_tier in struct via alias
            assert_eq!(
                minutes_to_next_tier,
                Some(15),
                "serde alias must map minutes_to_value_tier -> minutes_to_next_tier"
            );
            assert_eq!(tier_name, Some("Standard".to_string()));

            // Verify re-serialization uses the canonical field name, not the alias
            let reserialized = serde_json::to_string(&CoreToAgentMessage::BillingTick {
                remaining_seconds: 1200,
                allocated_seconds: 1800,
                driver_name: "AliasTest".to_string(),
                elapsed_seconds: None,
                cost_paise: None,
                rate_per_min_paise: None,
                paused: None,
                minutes_to_next_tier: Some(15),
                tier_name: Some("Standard".to_string()),
            })
            .unwrap();
            assert!(
                reserialized.contains("\"minutes_to_next_tier\":15"),
                "serialized output must use canonical field name, got: {}",
                reserialized
            );
            assert!(
                !reserialized.contains("minutes_to_value_tier"),
                "serialized output must NOT contain old alias name, got: {}",
                reserialized
            );
        } else {
            panic!("Expected BillingTick variant after alias deserialization");
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
    fn fleet_deploy_summary_serde_roundtrip() {
        let event = DashboardEvent::FleetDeploySummary {
            succeeded: vec!["pod_1".to_string(), "pod_2".to_string()],
            failed: vec!["pod_3".to_string()],
            waiting: vec!["pod_5".to_string()],
            timestamp: "2026-03-15T12:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("fleet_deploy_summary"), "Expected 'fleet_deploy_summary' tag in JSON: {}", json);
        let parsed: DashboardEvent = serde_json::from_str(&json).unwrap();
        if let DashboardEvent::FleetDeploySummary { succeeded, failed, waiting, .. } = parsed {
            assert_eq!(succeeded.len(), 2);
            assert_eq!(failed.len(), 1);
            assert_eq!(waiting.len(), 1);
            assert_eq!(failed[0], "pod_3");
        } else {
            panic!("Expected FleetDeploySummary variant");
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

    // ── Phase 17 Plan 01: WebSocket exec protocol tests ──────────────────

    #[test]
    fn test_exec_roundtrip() {
        let msg = CoreToAgentMessage::Exec {
            request_id: "req-abc-123".to_string(),
            cmd: "whoami".to_string(),
            timeout_ms: 5000,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: CoreToAgentMessage = serde_json::from_str(&json).unwrap();
        if let CoreToAgentMessage::Exec { request_id, cmd, timeout_ms } = deserialized {
            assert_eq!(request_id, "req-abc-123");
            assert_eq!(cmd, "whoami");
            assert_eq!(timeout_ms, 5000);
        } else {
            panic!("Wrong variant");
        }
    }

    #[test]
    fn test_exec_wire_format() {
        let msg = CoreToAgentMessage::Exec {
            request_id: "r1".to_string(),
            cmd: "echo hi".to_string(),
            timeout_ms: 10_000,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["type"], "exec");
        assert_eq!(v["data"]["request_id"], "r1");
        assert_eq!(v["data"]["cmd"], "echo hi");
        assert_eq!(v["data"]["timeout_ms"], 10_000);
    }

    #[test]
    fn test_exec_default_timeout() {
        // When timeout_ms is missing from JSON, default_exec_timeout_ms() should provide 10_000
        let json = r#"{"type":"exec","data":{"request_id":"r2","cmd":"dir"}}"#;
        let msg: CoreToAgentMessage = serde_json::from_str(json).unwrap();
        if let CoreToAgentMessage::Exec { timeout_ms, .. } = msg {
            assert_eq!(timeout_ms, 10_000);
        } else {
            panic!("Wrong variant");
        }
    }

    #[test]
    fn test_exec_result_roundtrip() {
        let msg = AgentMessage::ExecResult {
            request_id: "req-abc-123".to_string(),
            success: true,
            exit_code: Some(0),
            stdout: "bono\\user".to_string(),
            stderr: String::new(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: AgentMessage = serde_json::from_str(&json).unwrap();
        if let AgentMessage::ExecResult { request_id, success, exit_code, stdout, stderr } = deserialized {
            assert_eq!(request_id, "req-abc-123");
            assert!(success);
            assert_eq!(exit_code, Some(0));
            assert_eq!(stdout, "bono\\user");
            assert!(stderr.is_empty());
        } else {
            panic!("Wrong variant");
        }
    }

    #[test]
    fn test_exec_result_success_and_error() {
        // Success case
        let success = AgentMessage::ExecResult {
            request_id: "s1".to_string(),
            success: true,
            exit_code: Some(0),
            stdout: "ok".to_string(),
            stderr: String::new(),
        };
        let json = serde_json::to_string(&success).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["type"], "exec_result");
        assert_eq!(v["data"]["success"], true);

        // Error case (timeout)
        let err = AgentMessage::ExecResult {
            request_id: "e1".to_string(),
            success: false,
            exit_code: Some(124),
            stdout: String::new(),
            stderr: "Command timed out after 10000ms".to_string(),
        };
        let json = serde_json::to_string(&err).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["type"], "exec_result");
        assert_eq!(v["data"]["success"], false);
        assert_eq!(v["data"]["exit_code"], 124);

        // Error case (no exit code — semaphore exhausted)
        let sem_err = AgentMessage::ExecResult {
            request_id: "e2".to_string(),
            success: false,
            exit_code: None,
            stdout: String::new(),
            stderr: "WS slots exhausted (4 max)".to_string(),
        };
        let json = serde_json::to_string(&sem_err).unwrap();
        let deserialized: AgentMessage = serde_json::from_str(&json).unwrap();
        if let AgentMessage::ExecResult { exit_code, .. } = deserialized {
            assert_eq!(exit_code, None);
        } else {
            panic!("Wrong variant");
        }
    }

    // ── Phase 18 Plan 02: StartupReport serde tests ─────────────────────

    #[test]
    fn test_startup_report_roundtrip() {
        let msg = AgentMessage::StartupReport {
            pod_id: "pod_3".to_string(),
            version: "0.6.0".to_string(),
            uptime_secs: 5,
            config_hash: "abc123".to_string(),
            crash_recovery: false,
            repairs: vec!["config".to_string()],
            lock_screen_port_bound: false,
            remote_ops_port_bound: false,
            hid_detected: false,
            udp_ports_bound: vec![],
            startup_self_test_verdict: None,
            startup_probe_failures: 0,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("startup_report"), "JSON must contain 'startup_report', got: {}", json);
        assert!(json.contains("pod_3"));
        assert!(json.contains("0.6.0"));
        let parsed: AgentMessage = serde_json::from_str(&json).unwrap();
        if let AgentMessage::StartupReport { pod_id, version, uptime_secs, crash_recovery, repairs, .. } = parsed {
            assert_eq!(pod_id, "pod_3");
            assert_eq!(version, "0.6.0");
            assert_eq!(uptime_secs, 5);
            assert!(!crash_recovery);
            assert_eq!(repairs, vec!["config"]);
        } else {
            panic!("Wrong variant after roundtrip");
        }
    }

    #[test]
    fn test_startup_report_crash_recovery() {
        let msg = AgentMessage::StartupReport {
            pod_id: "pod_8".to_string(),
            version: "0.6.0".to_string(),
            uptime_secs: 0,
            config_hash: "def456".to_string(),
            crash_recovery: true,
            repairs: vec![],
            lock_screen_port_bound: false,
            remote_ops_port_bound: false,
            hid_detected: false,
            udp_ports_bound: vec![],
            startup_self_test_verdict: None,
            startup_probe_failures: 0,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: AgentMessage = serde_json::from_str(&json).unwrap();
        if let AgentMessage::StartupReport { crash_recovery, repairs, .. } = parsed {
            assert!(crash_recovery);
            assert!(repairs.is_empty());
        } else {
            panic!("Wrong variant");
        }
    }

    #[test]
    fn test_startup_report_boot_verification_roundtrip() {
        let msg = AgentMessage::StartupReport {
            pod_id: "pod_8".to_string(),
            version: "0.6.0".to_string(),
            uptime_secs: 12,
            config_hash: "abc123".to_string(),
            crash_recovery: false,
            repairs: vec![],
            lock_screen_port_bound: true,
            remote_ops_port_bound: true,
            hid_detected: true,
            udp_ports_bound: vec![9996, 20777, 5300],
            startup_self_test_verdict: None,
            startup_probe_failures: 0,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("lock_screen_port_bound"));
        assert!(json.contains("hid_detected"));
        let parsed: AgentMessage = serde_json::from_str(&json).unwrap();
        if let AgentMessage::StartupReport {
            lock_screen_port_bound,
            remote_ops_port_bound,
            hid_detected,
            udp_ports_bound,
            ..
        } = parsed {
            assert!(lock_screen_port_bound);
            assert!(remote_ops_port_bound);
            assert!(hid_detected);
            assert_eq!(udp_ports_bound, vec![9996, 20777, 5300]);
        } else {
            panic!("Wrong variant after roundtrip");
        }
    }

    #[test]
    fn test_startup_report_backward_compat_missing_new_fields() {
        // Simulate old agent sending StartupReport without Phase 46 fields
        // Format: {"type":"startup_report","data":{...}} (adjacently tagged)
        let old_json = r#"{"type":"startup_report","data":{"pod_id":"pod_3","version":"0.5.2","uptime_secs":5,"config_hash":"abc","crash_recovery":false,"repairs":[]}}"#;
        let parsed: AgentMessage = serde_json::from_str(old_json).unwrap();
        if let AgentMessage::StartupReport {
            lock_screen_port_bound,
            remote_ops_port_bound,
            hid_detected,
            udp_ports_bound,
            ..
        } = parsed {
            assert!(!lock_screen_port_bound, "missing field should default to false");
            assert!(!remote_ops_port_bound, "missing field should default to false");
            assert!(!hid_detected, "missing field should default to false");
            assert!(udp_ports_bound.is_empty(), "missing field should default to empty vec");
        } else {
            panic!("Wrong variant");
        }
    }

    // ── Phase 23 Plan 01: New bot failure AgentMessage variant tests ──────

    #[test]
    fn test_hardware_failure_roundtrip() {
        let msg = AgentMessage::HardwareFailure {
            pod_id: "pod_3".to_string(),
            reason: crate::types::PodFailureReason::WheelbaseDisconnected,
            detail: "USB disconnect detected".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("hardware_failure"), "Expected 'hardware_failure' in JSON, got: {}", json);
        let parsed: AgentMessage = serde_json::from_str(&json).unwrap();
        if let AgentMessage::HardwareFailure { pod_id, reason, detail } = parsed {
            assert_eq!(pod_id, "pod_3");
            assert_eq!(reason, crate::types::PodFailureReason::WheelbaseDisconnected);
            assert_eq!(detail, "USB disconnect detected");
        } else {
            panic!("Wrong variant");
        }
    }

    #[test]
    fn test_telemetry_gap_roundtrip() {
        let msg = AgentMessage::TelemetryGap {
            pod_id: "pod_5".to_string(),
            sim_type: crate::types::SimType::AssettoCorsa,
            gap_seconds: 42u32,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("telemetry_gap"), "Expected 'telemetry_gap' in JSON, got: {}", json);
        let parsed: AgentMessage = serde_json::from_str(&json).unwrap();
        if let AgentMessage::TelemetryGap { pod_id, sim_type, gap_seconds } = parsed {
            assert_eq!(pod_id, "pod_5");
            assert_eq!(sim_type, crate::types::SimType::AssettoCorsa);
            assert_eq!(gap_seconds, 42u32);
        } else {
            panic!("Wrong variant");
        }
    }

    #[test]
    fn test_billing_anomaly_roundtrip() {
        let msg = AgentMessage::BillingAnomaly {
            pod_id: "pod_2".to_string(),
            billing_session_id: "bsess-abc123".to_string(),
            reason: crate::types::PodFailureReason::SessionStuckWaitingForGame,
            detail: "game never launched".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("billing_anomaly"), "Expected 'billing_anomaly' in JSON, got: {}", json);
        let parsed: AgentMessage = serde_json::from_str(&json).unwrap();
        if let AgentMessage::BillingAnomaly { pod_id, billing_session_id, reason, detail } = parsed {
            assert_eq!(pod_id, "pod_2");
            assert_eq!(billing_session_id, "bsess-abc123");
            assert_eq!(reason, crate::types::PodFailureReason::SessionStuckWaitingForGame);
            assert_eq!(detail, "game never launched");
        } else {
            panic!("Wrong variant");
        }
    }

    #[test]
    fn test_lap_flagged_roundtrip() {
        let msg = AgentMessage::LapFlagged {
            pod_id: "pod_7".to_string(),
            lap_id: "lap-xyz-789".to_string(),
            reason: crate::types::PodFailureReason::LapCut,
            detail: "sector 2 shortcut".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("lap_flagged"), "Expected 'lap_flagged' in JSON, got: {}", json);
        let parsed: AgentMessage = serde_json::from_str(&json).unwrap();
        if let AgentMessage::LapFlagged { pod_id, lap_id, reason, detail } = parsed {
            assert_eq!(pod_id, "pod_7");
            assert_eq!(lap_id, "lap-xyz-789");
            assert_eq!(reason, crate::types::PodFailureReason::LapCut);
            assert_eq!(detail, "sector 2 shortcut");
        } else {
            panic!("Wrong variant");
        }
    }

    #[test]
    fn test_multiplayer_failure_roundtrip() {
        // session_id = None
        let msg_none = AgentMessage::MultiplayerFailure {
            pod_id: "pod_1".to_string(),
            reason: crate::types::PodFailureReason::MultiplayerDesync,
            session_id: None,
        };
        let json_none = serde_json::to_string(&msg_none).unwrap();
        assert!(json_none.contains("multiplayer_failure"), "Expected 'multiplayer_failure' in JSON, got: {}", json_none);
        let parsed_none: AgentMessage = serde_json::from_str(&json_none).unwrap();
        if let AgentMessage::MultiplayerFailure { pod_id, reason, session_id } = parsed_none {
            assert_eq!(pod_id, "pod_1");
            assert_eq!(reason, crate::types::PodFailureReason::MultiplayerDesync);
            assert_eq!(session_id, None);
        } else {
            panic!("Wrong variant for session_id=None");
        }

        // session_id = Some(...)
        let msg_some = AgentMessage::MultiplayerFailure {
            pod_id: "pod_1".to_string(),
            reason: crate::types::PodFailureReason::MultiplayerServerDisconnect,
            session_id: Some("sess-mp-456".to_string()),
        };
        let json_some = serde_json::to_string(&msg_some).unwrap();
        let parsed_some: AgentMessage = serde_json::from_str(&json_some).unwrap();
        if let AgentMessage::MultiplayerFailure { session_id, .. } = parsed_some {
            assert_eq!(session_id, Some("sess-mp-456".to_string()));
        } else {
            panic!("Wrong variant for session_id=Some");
        }
    }

    // ── Phase 49 Plan 01: Session lifecycle autonomy protocol tests ────────

    #[test]
    fn test_session_auto_ended_roundtrip() {
        let msg = AgentMessage::SessionAutoEnded {
            pod_id: "pod_1".to_string(),
            billing_session_id: "sess-abc".to_string(),
            reason: "orphan_timeout".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        // Verify serde rename_all = "snake_case" applies
        assert!(json.contains("\"type\":\"session_auto_ended\""),
            "Expected 'session_auto_ended' type tag in: {}", json);
        assert!(json.contains("\"pod_id\":\"pod_1\""));
        assert!(json.contains("\"billing_session_id\":\"sess-abc\""));
        assert!(json.contains("\"reason\":\"orphan_timeout\""));
        let parsed: AgentMessage = serde_json::from_str(&json).unwrap();
        if let AgentMessage::SessionAutoEnded { pod_id, billing_session_id, reason } = parsed {
            assert_eq!(pod_id, "pod_1");
            assert_eq!(billing_session_id, "sess-abc");
            assert_eq!(reason, "orphan_timeout");
        } else {
            panic!("Wrong variant after roundtrip: expected SessionAutoEnded");
        }
    }

    #[test]
    fn test_billing_paused_roundtrip() {
        let msg = AgentMessage::BillingPaused {
            pod_id: "pod_1".to_string(),
            billing_session_id: "sess-abc".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"billing_paused\""),
            "Expected 'billing_paused' type tag in: {}", json);
        assert!(json.contains("\"pod_id\":\"pod_1\""));
        assert!(json.contains("\"billing_session_id\":\"sess-abc\""));
        let parsed: AgentMessage = serde_json::from_str(&json).unwrap();
        if let AgentMessage::BillingPaused { pod_id, billing_session_id } = parsed {
            assert_eq!(pod_id, "pod_1");
            assert_eq!(billing_session_id, "sess-abc");
        } else {
            panic!("Wrong variant after roundtrip: expected BillingPaused");
        }
    }

    #[test]
    fn test_billing_resumed_roundtrip() {
        let msg = AgentMessage::BillingResumed {
            pod_id: "pod_1".to_string(),
            billing_session_id: "sess-abc".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"billing_resumed\""),
            "Expected 'billing_resumed' type tag in: {}", json);
        assert!(json.contains("\"pod_id\":\"pod_1\""));
        assert!(json.contains("\"billing_session_id\":\"sess-abc\""));
        let parsed: AgentMessage = serde_json::from_str(&json).unwrap();
        if let AgentMessage::BillingResumed { pod_id, billing_session_id } = parsed {
            assert_eq!(pod_id, "pod_1");
            assert_eq!(billing_session_id, "sess-abc");
        } else {
            panic!("Wrong variant after roundtrip: expected BillingResumed");
        }
    }

    // ── Phase 50 Plan 01: Self-test protocol roundtrip tests ──────────────

    #[test]
    fn test_run_self_test_roundtrip() {
        let msg = CoreToAgentMessage::RunSelfTest {
            request_id: "st-001".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(
            json.contains("run_self_test"),
            "Expected 'run_self_test' type tag in: {}",
            json
        );
        let parsed: CoreToAgentMessage = serde_json::from_str(&json).unwrap();
        if let CoreToAgentMessage::RunSelfTest { request_id } = parsed {
            assert_eq!(request_id, "st-001");
        } else {
            panic!("Wrong variant after roundtrip: expected RunSelfTest");
        }
    }

    #[test]
    fn test_self_test_result_roundtrip() {
        let msg = AgentMessage::SelfTestResult {
            pod_id: "pod-8".to_string(),
            request_id: "st-001".to_string(),
            report: serde_json::json!({
                "probes": [],
                "verdict": null,
                "timestamp": "2026-03-19T00:00:00Z"
            }),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(
            json.contains("self_test_result"),
            "Expected 'self_test_result' type tag in: {}",
            json
        );
        let parsed: AgentMessage = serde_json::from_str(&json).unwrap();
        if let AgentMessage::SelfTestResult { pod_id, request_id, report } = parsed {
            assert_eq!(pod_id, "pod-8");
            assert_eq!(request_id, "st-001");
            assert_eq!(report["timestamp"], "2026-03-19T00:00:00Z");
        } else {
            panic!("Wrong variant after roundtrip: expected SelfTestResult");
        }
    }

    #[test]
    fn test_startup_report_phase50_backward_compat() {
        // Old JSON without Phase 50 fields must deserialize with None/0 defaults
        let old_json = r#"{"type":"startup_report","data":{"pod_id":"pod_3","version":"0.5.2","uptime_secs":5,"config_hash":"abc","crash_recovery":false,"repairs":[]}}"#;
        let parsed: AgentMessage = serde_json::from_str(old_json).unwrap();
        if let AgentMessage::StartupReport {
            startup_self_test_verdict,
            startup_probe_failures,
            pod_id,
            ..
        } = parsed {
            assert_eq!(pod_id, "pod_3");
            assert!(startup_self_test_verdict.is_none(), "Expected None for missing field");
            assert_eq!(startup_probe_failures, 0, "Expected 0 for missing field");
        } else {
            panic!("Wrong variant after roundtrip: expected StartupReport");
        }
    }

    #[test]
    fn test_startup_report_phase50_with_verdict() {
        let msg = AgentMessage::StartupReport {
            pod_id: "pod_8".to_string(),
            version: "0.5.3".to_string(),
            uptime_secs: 10,
            config_hash: "deadbeef".to_string(),
            crash_recovery: false,
            repairs: vec![],
            lock_screen_port_bound: true,
            remote_ops_port_bound: true,
            hid_detected: true,
            udp_ports_bound: vec![9996],
            startup_self_test_verdict: Some("HEALTHY".to_string()),
            startup_probe_failures: 0,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: AgentMessage = serde_json::from_str(&json).unwrap();
        if let AgentMessage::StartupReport {
            startup_self_test_verdict,
            startup_probe_failures,
            ..
        } = parsed {
            assert_eq!(startup_self_test_verdict, Some("HEALTHY".to_string()));
            assert_eq!(startup_probe_failures, 0);
        } else {
            panic!("Wrong variant after roundtrip: expected StartupReport");
        }
    }

    #[test]
    fn switch_controller_serde_round_trip() {
        let msg = CoreToAgentMessage::SwitchController {
            target_url: "ws://100.70.177.44:8080/ws/agent".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("switch_controller"), "JSON: {}", json);
        assert!(json.contains("target_url"), "JSON: {}", json);
        let deserialized: CoreToAgentMessage = serde_json::from_str(&json).unwrap();
        match deserialized {
            CoreToAgentMessage::SwitchController { target_url } => {
                assert_eq!(target_url, "ws://100.70.177.44:8080/ws/agent");
            }
            other => panic!("Expected SwitchController, got {:?}", other),
        }
    }

    #[test]
    fn test_pre_flight_variants_round_trip() {
        let passed = AgentMessage::PreFlightPassed {
            pod_id: "pod-8".to_string(),
        };
        let json = serde_json::to_string(&passed).unwrap();
        let parsed: AgentMessage = serde_json::from_str(&json).unwrap();
        if let AgentMessage::PreFlightPassed { pod_id } = parsed {
            assert_eq!(pod_id, "pod-8");
        } else {
            panic!("Wrong variant after roundtrip: expected PreFlightPassed");
        }

        let failed = AgentMessage::PreFlightFailed {
            pod_id: "pod-3".to_string(),
            failures: vec!["HID wheelbase not connected".to_string()],
            timestamp: "2026-03-21T10:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&failed).unwrap();
        let parsed: AgentMessage = serde_json::from_str(&json).unwrap();
        if let AgentMessage::PreFlightFailed { pod_id, failures, timestamp } = parsed {
            assert_eq!(pod_id, "pod-3");
            assert_eq!(failures.len(), 1);
            assert_eq!(timestamp, "2026-03-21T10:00:00Z");
        } else {
            panic!("Wrong variant after roundtrip: expected PreFlightFailed");
        }
    }

    #[test]
    fn test_idle_health_failed_roundtrip() {
        let msg = AgentMessage::IdleHealthFailed {
            pod_id: "pod-1".to_string(),
            failures: vec!["lock_screen_http".to_string(), "window_rect".to_string()],
            consecutive_count: 3,
            timestamp: "2026-01-01T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("idle_health_failed"), "serde tag must be idle_health_failed");
        let parsed: AgentMessage = serde_json::from_str(&json).unwrap();
        if let AgentMessage::IdleHealthFailed { pod_id, failures, consecutive_count, timestamp } = parsed {
            assert_eq!(pod_id, "pod-1");
            assert_eq!(failures.len(), 2);
            assert_eq!(consecutive_count, 3);
            assert!(!timestamp.is_empty());
        } else {
            panic!("Wrong variant after roundtrip: expected IdleHealthFailed");
        }
    }

    #[test]
    fn test_clear_maintenance_round_trip() {
        let msg = CoreToAgentMessage::ClearMaintenance;
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: CoreToAgentMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, CoreToAgentMessage::ClearMaintenance));
    }

    #[test]
    fn test_force_relaunch_browser_roundtrip() {
        let msg = CoreToAgentMessage::ForceRelaunchBrowser { pod_id: "pod-1".to_string() };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("force_relaunch_browser"), "type tag must be snake_case");
        assert!(json.contains("pod-1"), "pod_id must appear in JSON");
        let roundtrip: CoreToAgentMessage = serde_json::from_str(&json).unwrap();
        if let CoreToAgentMessage::ForceRelaunchBrowser { pod_id } = roundtrip {
            assert_eq!(pod_id, "pod-1");
        } else {
            panic!("roundtrip produced wrong variant");
        }
    }
}

#[cfg(test)]
mod process_guard_protocol_tests {
    use super::*;
    use crate::types::{MachineWhitelist, ProcessViolation, ViolationType};

    #[test]
    fn agent_message_process_violation_has_correct_type_tag() {
        let v = ProcessViolation {
            machine_id: "pod-8".to_string(),
            violation_type: ViolationType::Process,
            name: "steam.exe".to_string(),
            exe_path: None,
            action_taken: "reported".to_string(),
            timestamp: "2026-03-21T12:00:00Z".to_string(),
            consecutive_count: 1,
        };
        let msg = AgentMessage::ProcessViolation(v);
        let json = serde_json::to_string(&msg).unwrap();
        assert!(
            json.contains(r#""type":"process_violation""#),
            "Expected process_violation type tag, got: {json}"
        );
        assert!(json.contains(r#""steam.exe""#));
    }

    #[test]
    fn agent_message_process_guard_status_has_correct_type_tag() {
        let msg = AgentMessage::ProcessGuardStatus {
            pod_id: "pod-1".to_string(),
            scan_count: 42,
            violation_count_total: 3,
            violation_count_last_scan: 0,
            last_scan_at: "2026-03-21T12:00:00Z".to_string(),
            guard_active: true,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(
            json.contains(r#""type":"process_guard_status""#),
            "Expected process_guard_status type tag, got: {json}"
        );
        assert!(json.contains(r#""scan_count":42"#));
    }

    #[test]
    fn core_to_agent_update_whitelist_round_trips() {
        let wl = MachineWhitelist {
            machine_id: "pod-8".to_string(),
            processes: vec!["rc-agent.exe".to_string(), "svchost.exe".to_string()],
            ports: vec![8090],
            autostart_keys: vec!["RCAgent".to_string()],
            violation_action: "report_only".to_string(),
            warn_before_kill: true,
        };
        let msg = CoreToAgentMessage::UpdateProcessWhitelist { whitelist: wl };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(
            json.contains(r#""type":"update_process_whitelist""#),
            "Expected update_process_whitelist type tag, got: {json}"
        );
        let msg2: CoreToAgentMessage = serde_json::from_str(&json).unwrap();
        match msg2 {
            CoreToAgentMessage::UpdateProcessWhitelist { whitelist } => {
                assert_eq!(whitelist.machine_id, "pod-8");
                assert_eq!(whitelist.ports, vec![8090]);
            }
            other => panic!("Wrong variant deserialized: {other:?}"),
        }
    }

    #[test]
    fn existing_agent_message_heartbeat_still_deserializes() {
        // Verify backward compat — adding new variants must not break existing consumers
        let json = r#"{"type":"disconnect","data":{"pod_id":"pod-3"}}"#;
        let msg: AgentMessage = serde_json::from_str(json).unwrap();
        match msg {
            AgentMessage::Disconnect { pod_id } => assert_eq!(pod_id, "pod-3"),
            other => panic!("Wrong variant: {other:?}"),
        }
    }

    #[test]
    fn existing_core_to_agent_clear_maintenance_still_deserializes() {
        let json = r#"{"type":"clear_maintenance","data":null}"#;
        let msg: CoreToAgentMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(msg, CoreToAgentMessage::ClearMaintenance));
    }

    // ── Phase 176 Plan 01: Forward-compatibility + new variant tests ──────────

    #[test]
    fn test_agent_message_unknown_variant_forward_compat() {
        // Simulate a message type that doesn't exist yet
        let json = r#"{"type":"totally_unknown_type","data":null}"#;
        let parsed: AgentMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(parsed, AgentMessage::Unknown));
    }

    #[test]
    fn test_core_to_agent_unknown_variant_forward_compat() {
        let json = r#"{"type":"totally_unknown_type","data":null}"#;
        let parsed: CoreToAgentMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(parsed, CoreToAgentMessage::Unknown));
    }

    #[test]
    fn test_agent_message_unknown_with_null_data() {
        // Unknown type with null data payload should deserialize to Unknown.
        // Note: serde adjacently-tagged (#[serde(tag, content)]) + #[serde(other)] discards
        // content only when data is null. Non-null data with unknown type is not supported by
        // serde without a custom deserializer — forward-compat protocol requires data:null for
        // sentinel/notification-style future messages.
        let json = r#"{"type":"future_feature_xyz","data":null}"#;
        let parsed: AgentMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(parsed, AgentMessage::Unknown));
    }

    #[test]
    fn test_flag_sync_roundtrip() {
        use std::collections::HashMap;
        let mut flags = HashMap::new();
        flags.insert("ai-debugger".to_string(), true);
        flags.insert("process-guard".to_string(), false);
        let msg = CoreToAgentMessage::FlagSync(FlagSyncPayload { flags: flags.clone(), version: 42 });
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("flag_sync"));
        let parsed: CoreToAgentMessage = serde_json::from_str(&json).unwrap();
        if let CoreToAgentMessage::FlagSync(payload) = parsed {
            assert_eq!(payload.flags.len(), 2);
            assert_eq!(payload.version, 42);
        } else {
            panic!("Expected FlagSync variant");
        }
    }

    #[test]
    fn test_config_push_roundtrip() {
        use std::collections::HashMap;
        let mut fields = HashMap::new();
        fields.insert("billing_rate".to_string(), serde_json::json!(900));
        let msg = CoreToAgentMessage::ConfigPush(ConfigPushPayload {
            fields,
            schema_version: 1,
            sequence: 100,
        });
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("config_push"));
        let parsed: CoreToAgentMessage = serde_json::from_str(&json).unwrap();
        if let CoreToAgentMessage::ConfigPush(payload) = parsed {
            assert_eq!(payload.schema_version, 1);
            assert_eq!(payload.sequence, 100);
        } else {
            panic!("Expected ConfigPush variant");
        }
    }

    #[test]
    fn test_ota_download_roundtrip() {
        let msg = CoreToAgentMessage::OtaDownload(OtaDownloadPayload {
            manifest_url: "http://192.168.31.27:9998/manifest.toml".to_string(),
            binary_sha256: "abcdef1234567890".to_string(),
            version: "1.0.0".to_string(),
        });
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("ota_download"));
        let parsed: CoreToAgentMessage = serde_json::from_str(&json).unwrap();
        if let CoreToAgentMessage::OtaDownload(payload) = parsed {
            assert_eq!(payload.binary_sha256, "abcdef1234567890");
        } else {
            panic!("Expected OtaDownload variant");
        }
    }

    #[test]
    fn test_kill_switch_roundtrip() {
        let msg = CoreToAgentMessage::KillSwitch(KillSwitchPayload {
            flag_name: "kill_billing".to_string(),
            active: true,
            reason: Some("Emergency maintenance".to_string()),
        });
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("kill_switch"));
        let parsed: CoreToAgentMessage = serde_json::from_str(&json).unwrap();
        if let CoreToAgentMessage::KillSwitch(payload) = parsed {
            assert_eq!(payload.flag_name, "kill_billing");
            assert!(payload.active);
        } else {
            panic!("Expected KillSwitch variant");
        }
    }

    #[test]
    fn test_ota_ack_roundtrip() {
        let msg = AgentMessage::OtaAck(OtaAckPayload {
            pod_id: "pod_8".to_string(),
            version: "1.0.0".to_string(),
            success: true,
            error: None,
        });
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("ota_ack"));
        let parsed: AgentMessage = serde_json::from_str(&json).unwrap();
        if let AgentMessage::OtaAck(payload) = parsed {
            assert_eq!(payload.pod_id, "pod_8");
            assert!(payload.success);
        } else {
            panic!("Expected OtaAck variant");
        }
    }

    #[test]
    fn test_config_ack_roundtrip() {
        let msg = AgentMessage::ConfigAck(ConfigAckPayload {
            pod_id: "pod_1".to_string(),
            sequence: 42,
            accepted: true,
        });
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("config_ack"));
        let parsed: AgentMessage = serde_json::from_str(&json).unwrap();
        if let AgentMessage::ConfigAck(payload) = parsed {
            assert_eq!(payload.sequence, 42);
        } else {
            panic!("Expected ConfigAck variant");
        }
    }

    #[test]
    fn test_flag_cache_sync_roundtrip() {
        let msg = AgentMessage::FlagCacheSync(FlagCacheSyncPayload {
            pod_id: "pod_3".to_string(),
            cached_version: 5,
        });
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("flag_cache_sync"));
        let parsed: AgentMessage = serde_json::from_str(&json).unwrap();
        if let AgentMessage::FlagCacheSync(payload) = parsed {
            assert_eq!(payload.pod_id, "pod_3");
            assert_eq!(payload.cached_version, 5);
        } else {
            panic!("Expected FlagCacheSync variant");
        }
    }
}
