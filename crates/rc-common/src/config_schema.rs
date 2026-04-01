//! Shared configuration schema for rc-agent and racecontrol.
//!
//! All AgentConfig struct definitions live here. rc-agent re-exports them.
//! This ensures a single source of truth for pod config (SCHEMA-01).

use serde::{Deserialize, Serialize};

// ─── Schema Version ───────────────────────────────────────────────────────────

fn default_schema_version() -> u32 { 1 }

// ─── NodeType ─────────────────────────────────────────────────────────────────

/// Node type within the Racing Point fleet.
/// Determines which subsystems are initialized at startup.
/// - Pod: Full gaming pod (FFB, HID, overlay, lock screen, game launching)
/// - POS: Point-of-sale terminal (billing, kiosk, mesh intelligence — no game hardware)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeType {
    Pod,
    Pos,
}

impl Default for NodeType {
    fn default() -> Self {
        NodeType::Pod
    }
}

impl std::fmt::Display for NodeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeType::Pod => write!(f, "pod"),
            NodeType::Pos => write!(f, "pos"),
        }
    }
}

// ─── Default helpers ──────────────────────────────────────────────────────────

pub fn default_sim() -> String { "none".to_string() }
pub fn default_sim_ip() -> String { "127.0.0.1".to_string() }
pub fn default_sim_port() -> u16 { 9996 }
pub fn default_core_url() -> String { "ws://127.0.0.1:8080/ws/agent".to_string() }
pub fn default_wheelbase_vid() -> u16 { 0x1209 }
pub fn default_wheelbase_pid() -> u16 { 0xFFB0 }
pub fn default_telemetry_ports() -> Vec<u16> { vec![9996, 20777, 5300, 6789, 5555] }
pub fn default_auto_end_orphan_session_secs() -> u64 { 300 }

fn default_true() -> bool { true }
fn default_scan_interval() -> u64 { 60 }
fn default_daily_budget_pod() -> f64 { 10.0 }
fn default_daily_budget_server() -> f64 { 20.0 }
fn default_daily_budget_pos() -> f64 { 5.0 }

// ─── Sub-config structs ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PodConfig {
    #[serde(default)]
    pub number: u32,
    #[serde(default)]
    pub name: String,
    /// Sim type — required for gaming pods, ignored for POS nodes.
    #[serde(default = "default_sim")]
    pub sim: String,
    #[serde(default = "default_sim_ip")]
    pub sim_ip: String,
    #[serde(default = "default_sim_port")]
    pub sim_port: u16,
    /// Node type: "pod" (default) or "pos". Determines which subsystems start.
    #[serde(default)]
    pub node_type: NodeType,
}

impl Default for PodConfig {
    fn default() -> Self {
        Self {
            number: 0,
            name: String::new(),
            sim: default_sim(),
            sim_ip: default_sim_ip(),
            sim_port: default_sim_port(),
            node_type: NodeType::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CoreConfig {
    #[serde(default = "default_core_url")]
    pub url: String,
    #[serde(default)]
    pub failover_url: Option<String>,
    /// Shared secret for WebSocket authentication.
    #[serde(default)]
    pub ws_secret: Option<String>,
    /// SEC-07: Path to a custom CA certificate file (PEM format) for wss:// connections.
    #[serde(default)]
    pub tls_ca_cert_path: Option<String>,
    /// SEC-07: Skip TLS certificate verification for wss:// connections.
    /// DANGEROUS: only use for LAN development/testing with self-signed certs.
    #[serde(default)]
    pub tls_skip_verify: bool,
}

impl Default for CoreConfig {
    fn default() -> Self {
        Self {
            url: default_core_url(),
            failover_url: None,
            ws_secret: None,
            tls_ca_cert_path: None,
            tls_skip_verify: false,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WheelbaseConfig {
    #[serde(default = "default_wheelbase_vid")]
    pub vendor_id: u16,
    #[serde(default = "default_wheelbase_pid")]
    pub product_id: u16,
}

impl Default for WheelbaseConfig {
    fn default() -> Self {
        Self {
            vendor_id: default_wheelbase_vid(),
            product_id: default_wheelbase_pid(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TelemetryPortsConfig {
    #[serde(default = "default_telemetry_ports")]
    pub ports: Vec<u16>,
}

impl Default for TelemetryPortsConfig {
    fn default() -> Self {
        Self {
            ports: default_telemetry_ports(),
        }
    }
}

/// Per-game executable configuration.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct GameExeConfig {
    /// Path to game executable
    pub exe_path: Option<String>,
    /// Working directory (defaults to exe parent dir)
    pub working_dir: Option<String>,
    /// Launch arguments
    pub args: Option<String>,
    /// Steam app ID (for Steam launch method)
    pub steam_app_id: Option<u32>,
    /// Whether to use Steam launch (steam://rungameid/{id})
    #[serde(default)]
    pub use_steam: bool,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct GamesConfig {
    #[serde(default)]
    pub assetto_corsa: GameExeConfig,
    #[serde(default)]
    pub assetto_corsa_evo: GameExeConfig,
    #[serde(default)]
    pub assetto_corsa_rally: GameExeConfig,
    #[serde(default)]
    pub iracing: GameExeConfig,
    #[serde(default)]
    pub f1_25: GameExeConfig,
    #[serde(default)]
    pub le_mans_ultimate: GameExeConfig,
    #[serde(default)]
    pub forza: GameExeConfig,
    #[serde(default)]
    pub forza_horizon_5: GameExeConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KioskConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl Default for KioskConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// POS-01: Lock screen browser configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LockScreenConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl Default for LockScreenConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PreflightConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl Default for PreflightConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProcessGuardConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_scan_interval")]
    pub scan_interval_secs: u64,
}

impl Default for ProcessGuardConfig {
    fn default() -> Self {
        Self { enabled: true, scan_interval_secs: 60 }
    }
}

/// AI debugger configuration (non-feature-gated stub).
/// When ai-debugger feature is ON in rc-agent, a richer version overrides this.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct AiDebuggerConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub ollama_url: String,
    #[serde(default)]
    pub ollama_model: String,
}

/// MMA-First Protocol configuration.
/// During the 30-day training period, MMA 5-model diagnosis is Tier 1
/// for all unresolved issues, rapidly populating the fleet KB.
/// After training_end, the system auto-flips to production mode (KB-first).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MmaConfig {
    /// When true AND today is within [training_start, training_end], MMA is Tier 1.
    #[serde(default)]
    pub training_mode: bool,
    /// ISO 8601 date when training period began (e.g. "2026-03-30").
    #[serde(default)]
    pub training_start: Option<String>,
    /// ISO 8601 date when training period ends (e.g. "2026-04-29").
    #[serde(default)]
    pub training_end: Option<String>,
    /// Daily budget per pod during training (default $15, production $10).
    #[serde(default = "default_daily_budget_pod")]
    pub daily_budget_pod: f64,
    /// Daily budget for server node during training (default $25, production $20).
    #[serde(default = "default_daily_budget_server")]
    pub daily_budget_server: f64,
    /// Daily budget for POS terminal during training (default $8, production $5).
    #[serde(default = "default_daily_budget_pos")]
    pub daily_budget_pos: f64,
}

impl Default for MmaConfig {
    fn default() -> Self {
        Self {
            training_mode: false,
            training_start: None,
            training_end: None,
            daily_budget_pod: default_daily_budget_pod(),
            daily_budget_server: default_daily_budget_server(),
            daily_budget_pos: default_daily_budget_pos(),
        }
    }
}

impl MmaConfig {
    /// Returns true if the training period is currently active.
    pub fn is_training_active(&self) -> bool {
        if !self.training_mode {
            return false;
        }

        let today = chrono::Utc::now().date_naive();

        if let Some(ref end_str) = self.training_end {
            if let Ok(end_date) = chrono::NaiveDate::parse_from_str(end_str, "%Y-%m-%d") {
                if today > end_date {
                    return false;
                }
            }
        }

        if let Some(ref start_str) = self.training_start {
            if let Ok(start_date) = chrono::NaiveDate::parse_from_str(start_str, "%Y-%m-%d") {
                if today < start_date {
                    return false;
                }
            }
        }

        true
    }

    /// Returns the appropriate daily budget based on node type and training status.
    pub fn daily_budget_for_node(&self, node_type: &NodeType) -> f64 {
        if self.is_training_active() {
            match node_type {
                NodeType::Pod => self.daily_budget_pod,
                NodeType::Pos => self.daily_budget_pos,
            }
        } else {
            match node_type {
                NodeType::Pod => 10.0,
                NodeType::Pos => 5.0,
            }
        }
    }
}

// ─── Top-level AgentConfig ────────────────────────────────────────────────────

/// Top-level configuration for rc-agent and related binaries.
/// Defined once in rc-common; re-exported by rc-agent.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AgentConfig {
    /// Schema version for forward compatibility (SCHEMA-04).
    /// Old agents silently ignore a higher version — they just use the fields they know.
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub pod: PodConfig,
    #[serde(default)]
    pub core: CoreConfig,
    #[serde(default)]
    pub wheelbase: WheelbaseConfig,
    #[serde(default)]
    pub telemetry_ports: TelemetryPortsConfig,
    #[serde(default)]
    pub games: GamesConfig,
    #[serde(default)]
    pub ai_debugger: AiDebuggerConfig,
    #[serde(default)]
    pub kiosk: KioskConfig,
    #[serde(default)]
    pub lock_screen: LockScreenConfig,
    #[serde(default)]
    pub preflight: PreflightConfig,
    #[serde(default)]
    pub process_guard: ProcessGuardConfig,
    /// Orphan billing auto-end timeout in seconds (SESSION-01).
    #[serde(default = "default_auto_end_orphan_session_secs")]
    pub auto_end_orphan_session_secs: u64,
    /// AC EVO shared memory telemetry feature flag (HARD-05).
    #[serde(default)]
    pub ac_evo_telemetry_enabled: bool,
    /// MMA-First Protocol config (v31.0).
    #[serde(default)]
    pub mma: MmaConfig,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            schema_version: 1,
            pod: PodConfig::default(),
            core: CoreConfig::default(),
            wheelbase: WheelbaseConfig::default(),
            telemetry_ports: TelemetryPortsConfig::default(),
            games: GamesConfig::default(),
            ai_debugger: AiDebuggerConfig::default(),
            kiosk: KioskConfig::default(),
            lock_screen: LockScreenConfig::default(),
            preflight: PreflightConfig::default(),
            process_guard: ProcessGuardConfig::default(),
            auto_end_orphan_session_secs: default_auto_end_orphan_session_secs(),
            ac_evo_telemetry_enabled: false,
            mma: MmaConfig::default(),
        }
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod config_schema_tests {
    use super::*;

    /// Test 1: AgentConfig with schema_version=1 deserializes from TOML, schema_version accessible
    #[test]
    fn schema_version_explicit_1() {
        let toml_str = r#"
schema_version = 1
[pod]
number = 1
name = "Pod 01"
[core]
url = "ws://127.0.0.1:8080/ws/agent"
"#;
        let config: AgentConfig = toml::from_str(toml_str).expect("should parse");
        assert_eq!(config.schema_version, 1);
    }

    /// Test 2: AgentConfig with schema_version omitted defaults to 1
    #[test]
    fn schema_version_defaults_to_1() {
        let toml_str = r#"
[pod]
number = 2
name = "Pod 02"
[core]
url = "ws://127.0.0.1:8080/ws/agent"
"#;
        let config: AgentConfig = toml::from_str(toml_str).expect("should parse");
        assert_eq!(config.schema_version, 1, "missing schema_version should default to 1");
    }

    /// Test 3: AgentConfig with schema_version=99 (future) still deserializes
    #[test]
    fn schema_version_future_accepted() {
        let toml_str = r#"
schema_version = 99
[pod]
number = 3
name = "Pod 03"
[core]
url = "ws://127.0.0.1:8080/ws/agent"
"#;
        let config: AgentConfig = toml::from_str(toml_str).expect("future schema_version should be accepted");
        assert_eq!(config.schema_version, 99);
    }

    /// Test 4: All sub-configs derive Clone + Serialize + Deserialize
    #[test]
    fn all_sub_configs_are_clone_serialize() {
        let config = AgentConfig::default();
        let cloned = config.clone();
        // Serialize round-trip
        let serialized = toml::to_string(&cloned).expect("should serialize");
        let deserialized: AgentConfig = toml::from_str(&serialized).expect("should deserialize");
        assert_eq!(deserialized.schema_version, 1);

        // Sub-configs individually
        let _ = config.pod.clone();
        let _ = config.core.clone();
        let _ = config.wheelbase.clone();
        let _ = config.telemetry_ports.clone();
        let _ = config.games.clone();
        let _ = config.kiosk.clone();
        let _ = config.lock_screen.clone();
        let _ = config.preflight.clone();
        let _ = config.process_guard.clone();
        let _ = config.mma.clone();
        let _ = config.ai_debugger.clone();
    }

    /// Test 5: Default AgentConfig has schema_version=1
    #[test]
    fn default_agent_config_schema_version_is_1() {
        let config = AgentConfig::default();
        assert_eq!(config.schema_version, 1);
    }

    #[test]
    fn node_type_default_is_pod() {
        assert_eq!(NodeType::default(), NodeType::Pod);
    }

    #[test]
    fn mma_config_is_training_active_false_by_default() {
        let mma = MmaConfig::default();
        assert!(!mma.is_training_active());
    }

    #[test]
    fn mma_daily_budget_for_node() {
        let mma = MmaConfig::default();
        assert_eq!(mma.daily_budget_for_node(&NodeType::Pod), 10.0);
        assert_eq!(mma.daily_budget_for_node(&NodeType::Pos), 5.0);
    }
}
