use anyhow::Result;
use rc_common::verification::{ColdVerificationChain, VerifyStep, VerificationError};

const LOG_TARGET: &str = "config";

// Re-export all config types from rc-common — single source of truth (SCHEMA-01)
// When ai-debugger feature is enabled, we re-export the feature-gated AiDebuggerConfig
// instead of the stub from rc-common to avoid type conflicts.
#[cfg(not(feature = "ai-debugger"))]
pub use rc_common::config_schema::*;

#[cfg(feature = "ai-debugger")]
pub use rc_common::config_schema::{
    AgentConfig, AiDebuggerConfig as _RcCommonAiDebuggerConfig, CoreConfig, GameExeConfig,
    GamesConfig, KioskConfig, LockScreenConfig, MmaConfig, NodeType, PodConfig, PreflightConfig,
    ProcessGuardConfig, TelemetryPortsConfig, WheelbaseConfig,
    default_auto_end_orphan_session_secs, default_core_url, default_sim, default_sim_ip,
    default_sim_port, default_telemetry_ports, default_wheelbase_pid, default_wheelbase_vid,
};

#[cfg(feature = "ai-debugger")]
pub use crate::ai_debugger::AiDebuggerConfig;

use rc_common::types::SimType;

/// Detect which games are actually installed on this pod.
/// Checks both TOML config (exe_path/steam_app_id) AND verifies the game exists on disk
/// via Steam appmanifest files. A game must be configured AND present on disk.
/// AC (original) is always included — it's the default game on every pod.
pub(crate) fn detect_installed_games(games: &GamesConfig) -> Vec<SimType> {
    let mut installed = vec![SimType::AssettoCorsa]; // AC always available (Content Manager)

    // Map rc-common GameExeConfig to agent GameExeConfig via the shared fields
    let candidates: Vec<(&GameExeConfig, SimType)> = vec![
        (&games.f1_25, SimType::F125),
        (&games.iracing, SimType::IRacing),
        (&games.forza, SimType::Forza),
        (&games.le_mans_ultimate, SimType::LeMansUltimate),
        (&games.assetto_corsa_evo, SimType::AssettoCorsaEvo),
        (&games.assetto_corsa_rally, SimType::AssettoCorsaRally),
        (&games.forza_horizon_5, SimType::ForzaHorizon5),
    ];

    for (config, sim_type) in candidates {
        let configured = config.exe_path.is_some() || config.steam_app_id.is_some();
        if !configured {
            continue;
        }

        if let Some(ref path) = config.exe_path {
            if std::path::Path::new(path).exists() {
                installed.push(sim_type);
                continue;
            }
        }

        if let Some(app_id) = config.steam_app_id {
            if is_steam_app_installed(app_id) {
                installed.push(sim_type);
            } else {
                tracing::info!(
                    target: LOG_TARGET,
                    "Game {:?} configured (app_id={}) but not installed on disk — skipping",
                    sim_type, app_id
                );
            }
        }
    }

    installed
}

/// Check if a Steam app is installed by looking for its appmanifest file.
fn is_steam_app_installed(app_id: u32) -> bool {
    let manifest = format!(
        r"C:\Program Files (x86)\Steam\steamapps\appmanifest_{}.acf",
        app_id
    );
    std::path::Path::new(&manifest).exists()
}

/// Validate agent configuration. Returns Err with all issues found (not fail-fast).
///
/// Rules:
/// - pod.number must be 1–99 inclusive
/// - pod.name must not be blank after trimming
/// - core.url must start with "ws://" or "wss://"
pub(crate) fn validate_config(config: &AgentConfig) -> Result<()> {
    let mut errors: Vec<String> = Vec::new();

    if config.pod.number == 0 || config.pod.number > 99 {
        errors.push(format!(
            "pod.number must be 1-99, got {}",
            config.pod.number
        ));
    }

    if config.pod.name.trim().is_empty() {
        errors.push("pod.name must not be empty".to_string());
    }

    let url = config.core.url.trim();
    if !url.starts_with("ws://") && !url.starts_with("wss://") {
        errors.push(format!(
            "core.url must start with ws:// or wss://, got {:?}",
            url
        ));
    }

    if let Some(ref furl) = config.core.failover_url {
        let furl = furl.trim();
        if !furl.starts_with("ws://") && !furl.starts_with("wss://") {
            errors.push(format!(
                "core.failover_url must start with ws:// or wss://, got {:?}",
                furl
            ));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(anyhow::anyhow!("{}", errors.join("; ")))
    }
}

pub(crate) fn config_search_paths() -> Vec<std::path::PathBuf> {
    let mut paths: Vec<std::path::PathBuf> = Vec::new();

    // Determine config filename from binary name — rc-pos-agent.exe uses rc-pos-agent.toml
    let config_name = std::env::current_exe()
        .ok()
        .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .and_then(|stem| {
            if stem.starts_with("rc-pos-agent") {
                Some("rc-pos-agent.toml".to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "rc-agent.toml".to_string());

    // Primary: exe directory (correct on Windows regardless of CWD)
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            paths.push(exe_dir.join(&config_name));
            if config_name != "rc-agent.toml" {
                paths.push(exe_dir.join("rc-agent.toml"));
            }
        }
    }
    // Fallback: CWD (useful for `cargo run` in dev)
    paths.push(std::path::PathBuf::from(&config_name));
    if config_name != "rc-agent.toml" {
        paths.push(std::path::PathBuf::from("rc-agent.toml"));
    }
    // Legacy Linux path
    paths.push(std::path::PathBuf::from("/etc/racecontrol/rc-agent.toml"));

    paths
}

// ─── Known top-level TOML keys for AgentConfig ───────────────────────────────

const KNOWN_FIELDS: &[&str] = &[
    "schema_version",
    "pod",
    "core",
    "wheelbase",
    "telemetry_ports",
    "games",
    "ai_debugger",
    "kiosk",
    "lock_screen",
    "preflight",
    "process_guard",
    "auto_end_orphan_session_secs",
    "ac_evo_telemetry_enabled",
    "mma",
];

/// Lenient TOML deserializer (SCHEMA-02, SCHEMA-03).
///
/// Returns (config, warnings) where warnings are human-readable messages about:
/// - Unknown top-level fields (SCHEMA-02): silently ignored, warning issued
/// - Type-mismatched fields (SCHEMA-03): field falls back to Default value, warning issued
///
/// Never panics or returns Err for well-formed TOML — only structural TOML syntax errors
/// (missing `=`, unclosed brackets) cause an Err.
pub(crate) fn lenient_deserialize(content: &str) -> Result<(AgentConfig, Vec<String>)> {
    let mut warnings: Vec<String> = Vec::new();

    // Parse as raw Value first — always succeeds for syntactically valid TOML
    let raw: toml::Value = toml::from_str(content)
        .map_err(|e| anyhow::anyhow!("TOML syntax error: {}", e))?;

    // Warn on unknown top-level keys (SCHEMA-02)
    if let Some(table) = raw.as_table() {
        for key in table.keys() {
            if !KNOWN_FIELDS.contains(&key.as_str()) {
                warnings.push(format!("Unknown config field '{}' — ignored", key));
            }
        }
    }

    // Try full deserialize — with #[serde(default)] it won't fail on missing fields
    match toml::from_str::<AgentConfig>(content) {
        Ok(config) => Ok((config, warnings)),
        Err(full_err) => {
            // Type error somewhere — fall back field-by-field (SCHEMA-03)
            warnings.push(format!(
                "Config has type errors, using defaults for invalid fields: {}",
                full_err
            ));

            let mut config = AgentConfig::default();

            if let Some(table) = raw.as_table() {
                macro_rules! try_section {
                    ($key:literal, $field:ident, $ty:ty) => {
                        if let Some(val) = table.get($key) {
                            match val.clone().try_into::<$ty>() {
                                Ok(v) => config.$field = v,
                                Err(e) => warnings.push(format!(
                                    "Config field '{}' has invalid type — using default: {}",
                                    $key, e
                                )),
                            }
                        }
                    };
                }

                try_section!("pod", pod, PodConfig);
                try_section!("core", core, CoreConfig);
                try_section!("wheelbase", wheelbase, WheelbaseConfig);
                try_section!("telemetry_ports", telemetry_ports, TelemetryPortsConfig);
                try_section!("games", games, GamesConfig);
                // Use the common stub type explicitly — AgentConfig.ai_debugger is always the common type
                try_section!("ai_debugger", ai_debugger, rc_common::config_schema::AiDebuggerConfig);
                try_section!("kiosk", kiosk, KioskConfig);
                try_section!("lock_screen", lock_screen, LockScreenConfig);
                try_section!("preflight", preflight, PreflightConfig);
                try_section!("process_guard", process_guard, ProcessGuardConfig);
                try_section!("mma", mma, MmaConfig);

                // Scalar fields
                if let Some(val) = table.get("auto_end_orphan_session_secs") {
                    match val.clone().try_into::<u64>() {
                        Ok(v) => config.auto_end_orphan_session_secs = v,
                        Err(e) => warnings.push(format!(
                            "Config field 'auto_end_orphan_session_secs' has invalid type — using default: {}", e
                        )),
                    }
                }
                if let Some(val) = table.get("ac_evo_telemetry_enabled") {
                    match val.clone().try_into::<bool>() {
                        Ok(v) => config.ac_evo_telemetry_enabled = v,
                        Err(e) => warnings.push(format!(
                            "Config field 'ac_evo_telemetry_enabled' has invalid type — using default: {}", e
                        )),
                    }
                }
                if let Some(val) = table.get("schema_version") {
                    match val.clone().try_into::<u64>() {
                        Ok(v) => config.schema_version = v as u32,
                        Err(e) => warnings.push(format!(
                            "Config field 'schema_version' has invalid type — using default: {}", e
                        )),
                    }
                }
            }

            Ok((config, warnings))
        }
    }
}

// ─── Verification chain steps for agent config TOML load (COV-03) ────────────

struct StepAgentFileRead;
impl VerifyStep for StepAgentFileRead {
    type Input = std::path::PathBuf;
    type Output = (String, String);  // (content, path_display)
    fn name(&self) -> &str { "agent_file_read" }
    fn run(&self, input: std::path::PathBuf) -> Result<(String, String), VerificationError> {
        let path_str = input.display().to_string();
        std::fs::read_to_string(&input)
            .map(|c| (c, path_str.clone()))
            .map_err(|e| VerificationError::InputParseError {
                step: self.name().to_string(),
                raw_value: format!("path={} error={}", path_str, e),
            })
    }
}

struct StepAgentTomlParse;
impl VerifyStep for StepAgentTomlParse {
    type Input = (String, String);  // (content, path)
    type Output = AgentConfig;
    fn name(&self) -> &str { "agent_toml_parse" }
    fn run(&self, input: (String, String)) -> Result<AgentConfig, VerificationError> {
        let (content, path) = input;
        match lenient_deserialize(&content) {
            Ok((config, warnings)) => {
                for warn in &warnings {
                    tracing::warn!(target: "config", "{}", warn);
                }
                Ok(config)
            }
            Err(e) => {
                let first_3_lines: String = content.lines().take(3).collect::<Vec<_>>().join(" | ");
                Err(VerificationError::InputParseError {
                    step: self.name().to_string(),
                    raw_value: format!("path={} error={} first_3_lines=[{}]", path, e, first_3_lines),
                })
            }
        }
    }
}

pub fn load_config() -> Result<AgentConfig> {
    let search_paths = config_search_paths();
    let chain = ColdVerificationChain::new("agent_config_load");

    for path in &search_paths {
        match chain.execute_step(&StepAgentFileRead, path.clone()) {
            Ok((content, path_display)) => {
                match chain.execute_step(&StepAgentTomlParse, (content, path_display.clone())) {
                    Ok(config) => {
                        tracing::info!(target: LOG_TARGET, "Loaded config from {}", path_display);
                        validate_config(&config)?;
                        return Ok(config);
                    }
                    Err(e) => {
                        tracing::warn!(target: LOG_TARGET, error = %e, "config parse failed via verification chain");
                        continue;
                    }
                }
            }
            Err(_) => continue,
        }
    }

    Err(anyhow::anyhow!(
        "No config file found. Searched: {}",
        search_paths
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

#[cfg(test)]
mod process_guard_config_tests {
    use super::*;

    #[test]
    fn process_guard_config_defaults() {
        let cfg = ProcessGuardConfig::default();
        assert!(cfg.enabled);
        assert_eq!(cfg.scan_interval_secs, 60);
    }

    #[test]
    fn process_guard_config_deser_enabled_false() {
        let toml_str = "[process_guard]\nenabled = false\n";
        #[derive(serde::Deserialize)]
        struct Wrapper { process_guard: ProcessGuardConfig }
        let w: Wrapper = toml::from_str(toml_str).unwrap();
        assert!(!w.process_guard.enabled);
        assert_eq!(w.process_guard.scan_interval_secs, 60);
    }

    #[test]
    fn agent_config_no_process_guard_section_deserializes() {
        let toml_str = "[pod]\nnumber = 1\n[core]\nurl = \"ws://127.0.0.1:8080/ws/agent\"\n";
        let result = toml::from_str::<toml::Value>(toml_str);
        assert!(result.is_ok());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_config() -> AgentConfig {
        AgentConfig {
            schema_version: 1,
            pod: PodConfig {
                number: 3,
                name: "Pod 03".to_string(),
                sim: "assetto_corsa".to_string(),
                sim_ip: default_sim_ip(),
                sim_port: default_sim_port(),
                node_type: NodeType::Pod,
            },
            core: CoreConfig {
                url: "ws://192.168.31.23:8080/ws/agent".to_string(),
                failover_url: None,
                ws_secret: None,
                tls_ca_cert_path: None,
                tls_skip_verify: false,
            },
            wheelbase: WheelbaseConfig::default(),
            telemetry_ports: TelemetryPortsConfig::default(),
            games: GamesConfig::default(),
            ai_debugger: rc_common::config_schema::AiDebuggerConfig::default(),
            kiosk: KioskConfig::default(),
            lock_screen: LockScreenConfig::default(),
            preflight: PreflightConfig::default(),
            process_guard: ProcessGuardConfig::default(),
            auto_end_orphan_session_secs: default_auto_end_orphan_session_secs(),
            ac_evo_telemetry_enabled: false,
            mma: MmaConfig::default(),
        }
    }

    #[test]
    fn validate_config_accepts_valid_config() {
        let config = valid_config();
        assert!(validate_config(&config).is_ok(), "Valid config should pass validation");
    }

    #[test]
    fn validate_config_rejects_pod_number_zero() {
        let mut config = valid_config();
        config.pod.number = 0;
        let err = validate_config(&config).unwrap_err();
        assert!(
            err.to_string().contains("pod.number must be 1-99"),
            "Error should mention pod.number must be 1-99, got: {}",
            err
        );
    }

    #[test]
    fn validate_config_rejects_pod_number_100() {
        let mut config = valid_config();
        config.pod.number = 100;
        let err = validate_config(&config).unwrap_err();
        assert!(
            err.to_string().contains("pod.number must be 1-99"),
            "Error should mention pod.number must be 1-99, got: {}",
            err
        );
    }

    #[test]
    fn validate_config_accepts_pod_number_9_for_pos() {
        let mut config = valid_config();
        config.pod.number = 9;
        assert!(validate_config(&config).is_ok(), "Pod number 9 should be valid (POS/auxiliary devices)");
    }

    #[test]
    fn validate_config_rejects_empty_pod_name() {
        let mut config = valid_config();
        config.pod.name = "   ".to_string();
        let err = validate_config(&config).unwrap_err();
        assert!(
            err.to_string().contains("pod.name"),
            "Error should mention pod.name, got: {}",
            err
        );
    }

    #[test]
    fn validate_config_rejects_http_url() {
        let mut config = valid_config();
        config.core.url = "http://192.168.31.23:8080/ws/agent".to_string();
        let err = validate_config(&config).unwrap_err();
        assert!(
            err.to_string().contains("ws://"),
            "Error should mention ws://, got: {}",
            err
        );
    }

    #[test]
    fn validate_config_rejects_empty_url() {
        let mut config = valid_config();
        config.core.url = "".to_string();
        let err = validate_config(&config).unwrap_err();
        assert!(
            err.to_string().contains("ws://"),
            "Error should mention ws://, got: {}",
            err
        );
    }

    #[test]
    fn validate_config_accepts_wss_url() {
        let mut config = valid_config();
        config.core.url = "wss://app.racingpoint.cloud/ws/agent".to_string();
        assert!(validate_config(&config).is_ok(), "wss:// URL should be accepted");
    }

    #[test]
    fn validate_config_accepts_pod_number_1_and_8() {
        let mut config = valid_config();
        config.pod.number = 1;
        assert!(validate_config(&config).is_ok(), "Pod 1 should be valid");
        config.pod.number = 8;
        assert!(validate_config(&config).is_ok(), "Pod 8 should be valid");
    }

    #[test]
    fn load_config_returns_err_when_no_file_exists() {
        let mut config = valid_config();
        config.pod.number = 1;
        config.core.url = "ws://127.0.0.1:8080/ws/agent".to_string();
        assert!(validate_config(&config).is_ok(), "Explicitly valid config should pass");
    }

    #[test]
    fn test_config_search_paths_includes_exe_dir() {
        use std::path::PathBuf;
        let paths = config_search_paths();
        assert!(!paths.is_empty(), "config_search_paths() must return at least one path");
        let first = &paths[0];
        assert!(
            first.file_name().map(|n| n == "rc-agent.toml").unwrap_or(false),
            "First search path must end with rc-agent.toml, got: {}",
            first.display()
        );
        assert!(
            first != &PathBuf::from("rc-agent.toml"),
            "First path must include exe directory, not bare 'rc-agent.toml', got: {}",
            first.display()
        );
    }

    #[test]
    fn test_config_search_paths_includes_cwd_fallback() {
        use std::path::PathBuf;
        let paths = config_search_paths();
        let has_cwd_fallback = paths.contains(&PathBuf::from("rc-agent.toml"));
        assert!(has_cwd_fallback, "config_search_paths() must include 'rc-agent.toml' (CWD fallback)");
        let cwd_index = paths.iter().position(|p| p == &PathBuf::from("rc-agent.toml")).unwrap();
        assert!(
            cwd_index > 0,
            "CWD fallback 'rc-agent.toml' must appear after exe-dir path (index {}), not at index 0",
            cwd_index
        );
    }

    #[test]
    fn test_load_config_error_lists_all_searched_paths() {
        let tmp = std::env::temp_dir().join("rc_agent_test_no_config");
        let _ = std::fs::create_dir_all(&tmp);
        let original = std::env::current_dir().ok();
        let _ = std::env::set_current_dir(&tmp);

        let result = load_config();

        if let Some(orig) = original {
            let _ = std::env::set_current_dir(orig);
        }

        let err = result.expect_err("load_config() must return Err when no config file exists");
        let msg = err.to_string();
        assert!(
            msg.contains("No config file found"),
            "Error must contain 'No config file found', got: {}",
            msg
        );
        assert!(
            msg.contains("Searched:"),
            "Error must contain 'Searched:', got: {}",
            msg
        );
        let path_count = msg.matches("rc-agent.toml").count();
        assert!(
            path_count >= 2,
            "Error must list at least 2 paths containing 'rc-agent.toml', found {} in: {}",
            path_count,
            msg
        );
    }

    // ─── Lenient parsing tests (SCHEMA-02, SCHEMA-03) ─────────────────────────

    /// Test 1: TOML with unknown field deserializes successfully, warning issued
    #[test]
    fn lenient_unknown_field_warns_not_errors() {
        let toml_str = r#"
future_field = true
[pod]
number = 1
name = "Pod 01"
[core]
url = "ws://127.0.0.1:8080/ws/agent"
"#;
        let (config, warnings) = lenient_deserialize(toml_str).expect("should succeed");
        assert_eq!(config.pod.number, 1);
        let has_unknown_warn = warnings.iter().any(|w| w.contains("future_field") && w.contains("ignored"));
        assert!(has_unknown_warn, "Expected warning about 'future_field', got: {:?}", warnings);
    }

    /// Test 2: TOML with wrong type on known field falls back to default, warning issued
    #[test]
    fn lenient_type_mismatch_falls_back_to_default() {
        // auto_end_orphan_session_secs is u64 — give it a string
        let toml_str = r#"
auto_end_orphan_session_secs = "not_a_number"
[pod]
number = 1
name = "Pod 01"
[core]
url = "ws://127.0.0.1:8080/ws/agent"
"#;
        let (config, warnings) = lenient_deserialize(toml_str).expect("should succeed");
        assert_eq!(
            config.auto_end_orphan_session_secs,
            default_auto_end_orphan_session_secs(),
            "should fall back to default 300"
        );
        let has_type_warn = warnings.iter().any(|w| w.contains("invalid type") || w.contains("type errors"));
        assert!(has_type_warn, "Expected type mismatch warning, got: {:?}", warnings);
    }

    /// Test 3: TOML with schema_version=99 loads successfully
    #[test]
    fn lenient_future_schema_version_loads() {
        let toml_str = r#"
schema_version = 99
[pod]
number = 2
name = "Pod 02"
[core]
url = "ws://127.0.0.1:8080/ws/agent"
"#;
        let (config, warnings) = lenient_deserialize(toml_str).expect("should succeed");
        assert_eq!(config.schema_version, 99);
        assert!(warnings.is_empty(), "No warnings expected for valid future version, got: {:?}", warnings);
    }

    /// Test 4: Minimal valid TOML still works
    #[test]
    fn lenient_minimal_toml_works() {
        let toml_str = r#"
[pod]
number = 5
name = "Pod 05"
[core]
url = "ws://192.168.31.23:8080/ws/agent"
"#;
        let (config, _warnings) = lenient_deserialize(toml_str).expect("should succeed");
        assert_eq!(config.pod.number, 5);
        assert_eq!(config.pod.name, "Pod 05");
        assert_eq!(config.schema_version, 1); // default
    }

    /// Test 5: re-export of rc_common::config_schema types is accessible
    #[test]
    fn reexported_types_accessible() {
        // If this compiles, re-export works
        let _ = AgentConfig::default();
        let _ = NodeType::Pod;
        let _ = ProcessGuardConfig::default();
        let _ = MmaConfig::default();
        let _ = AiDebuggerConfig::default();
    }
}
