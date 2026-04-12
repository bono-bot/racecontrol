//! Game Launch Contract — shared types between server and agent.
//!
//! This file is the AGREEMENT between racecontrol (server) and rc-agent (pod).
//! Neither side changes this without the other updating their consumer.
//! Changes require a [CONTRACT CHANGE] tagged commit and comms-link notification.
//!
//! Design principles (from VMS architecture analysis, 2026-04-13):
//! - Launch is SIMPLE: validate funds → write config → spawn process → done
//! - Launcher EXITS after spawning — no monitoring, no healing in the launch path
//! - Monitoring is a SEPARATE concern (telemetry reader, not launcher)
//! - Each sim implements the SimLauncher trait — no copy-paste between sims
//! - Two launch methods (Staff Kiosk + PWA PIN) share ONLY the final launch step

use serde::{Deserialize, Serialize};

// ─── Game Types ──────────────────────────────────────────────────────────────

/// Every supported game in the RaceControl ecosystem.
/// Adding a new game = add variant here → compiler shows every handler that needs updating.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GameType {
    AssettCorsa,
    F125,
    IRacing,
    LeManUltimate,
}

impl GameType {
    /// The executable name VMS Connect uses for this sim.
    /// Reference: VMS Connect ini files (EXAMPLE assettocorsa.ini, lmu.ini, iracing.ini)
    pub fn exe_name(&self) -> &'static str {
        match self {
            Self::AssettCorsa => "acs.exe",
            Self::F125 => "F1_25.exe",
            Self::IRacing => "iRacingSim64DX11.exe",
            Self::LeManUltimate => "Le Mans Ultimate.exe",
        }
    }

    /// Steam App ID. VMS uses 480 (Spacewar) for native C++ apps.
    /// AC needs 244210 (the real Steam ID) because acs.exe goes through Steam DRM.
    pub fn steam_app_id(&self) -> Option<u32> {
        match self {
            Self::AssettCorsa => Some(244210),
            Self::F125 => None, // F1 25 uses its own launcher
            Self::IRacing => None, // iRacing uses its own launcher
            Self::LeManUltimate => None, // LMU uses its own launcher
        }
    }
}

// ─── Launch Request (Server → Agent) ─────────────────────────────────────────

/// What the server sends to the agent to launch a game.
/// This is the ONLY message that crosses the server↔agent boundary for launching.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchRequest {
    /// Which game to launch.
    pub game: GameType,
    /// Which pod this is for (agent validates it matches its own pod_id).
    pub pod_id: u8,
    /// Customer driver ID (for telemetry attribution).
    pub driver_id: i64,
    /// Customer display name (written into game config).
    pub driver_name: String,
    /// Launch method — determines how the request arrived.
    pub method: LaunchMethod,
    /// Game-specific configuration.
    pub config: GameConfig,
}

/// How the launch was initiated. Staff and PWA are separate code paths
/// that converge here — the agent doesn't care which path was taken.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LaunchMethod {
    /// Staff clicked launch on the kiosk (Method 1).
    StaffKiosk { staff_id: String },
    /// Customer entered a PIN from PWA (Method 2).
    PwaPin { pin: String },
}

/// Game-specific configuration. Each variant carries only what that sim needs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GameConfig {
    AssettCorsa {
        track: String,
        car: String,
        session_type: AcSessionType,
        ai_count: u8,
        ai_level: u8,
        /// Multiplayer: connect to local AC server.
        server: Option<AcServerConfig>,
    },
    F125 {
        /// F1 25 uses its own session config — minimal params from our side.
        track: Option<String>,
    },
    /// iRacing manages its own sessions — we just launch the exe.
    IRacing {},
    LeManUltimate {
        /// LMU uses rFactor2 engine — similar to AC server architecture.
        track: Option<String>,
        car: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AcSessionType {
    Practice,
    Race,
    Hotlap,
    TimeAttack,
    Drift,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcServerConfig {
    pub ip: String,
    pub http_port: u16,
    pub password: Option<String>,
}

// ─── Launch Result (Agent → Server) ──────────────────────────────────────────

/// What the agent sends back after attempting to launch.
/// This is immediate — the agent does NOT wait for the game to fully load.
/// Game liveness is monitored separately (telemetry reader, not launcher).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchResult {
    pub pod_id: u8,
    pub game: GameType,
    pub outcome: LaunchOutcome,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LaunchOutcome {
    /// Game process was spawned successfully.
    /// PID is provided if available (not all platforms report it immediately).
    Spawned { pid: Option<u32> },
    /// Launch failed — game process could not be started.
    Failed { reason: String },
}

// ─── Telemetry Events (Agent → Server, separate from launch) ─────────────────

/// Lap completed event — sent by the telemetry reader (NOT the launcher).
/// VMS pattern: the AC plugin writes to shared memory, VMS Connect reads it.
/// Our pattern: rc-agent reads shared memory / UDP, sends this to server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LapCompleted {
    pub pod_id: u8,
    pub driver_id: i64,
    pub game: GameType,
    pub track: String,
    pub car: String,
    pub lap_time_ms: u32,
    pub lap_number: u32,
    pub is_valid: bool,
    pub sector_times_ms: Vec<u32>,
}

/// Real-time telemetry snapshot — sent periodically by the telemetry reader.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetrySnapshot {
    pub pod_id: u8,
    pub driver_id: i64,
    pub game: GameType,
    pub speed_kmh: f32,
    pub gear: i32,
    pub throttle: f32,
    pub brake: f32,
    pub rpm: i32,
    pub lap_time_ms: u32,
    pub position: f32, // normalized spline position 0.0-1.0
}

/// Game state change — sent when the game starts, crashes, or ends.
/// These events drive billing (subscribe to GameStarted to start billing).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GameEvent {
    /// Game process is running and producing telemetry.
    Started { pod_id: u8, game: GameType, pid: u32 },
    /// Game process crashed or was terminated unexpectedly.
    Crashed { pod_id: u8, game: GameType, reason: String },
    /// Game ended normally (customer exited or session timer expired).
    Ended { pod_id: u8, game: GameType, duration_secs: u64 },
}
