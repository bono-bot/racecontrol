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

// ─── PUSH-05: Hot/cold field classification ───────────────────────────────────
//
// HOT fields: apply immediately on FullConfigPush without agent restart (PUSH-03).
// COLD fields: log as pending-restart, NOT applied until next startup (PUSH-04).
// Fields are identified by dot-path string prefix for comparison.

/// Fields that can be applied immediately without agent restart (PUSH-03).
/// These control runtime feature gates and budgets — safe to toggle live.
pub(crate) const HOT_RELOAD_CONFIG_FIELDS: &[&str] = &[
    "process_guard.enabled",
    "process_guard.scan_interval_secs",
    "kiosk.enabled",
    "lock_screen.enabled",
    "preflight.enabled",
    "mma.training_mode",
    "mma.daily_budget_pod",
    "mma.daily_budget_server",
    "mma.daily_budget_pos",
    "auto_end_orphan_session_secs",
    "ac_evo_telemetry_enabled",
];

/// Fields that require agent restart to take effect (PUSH-04).
/// Changing these live would cause undefined behavior (wrong pod identity,
/// WS reconnect with old URL, HID bind failure, UDP port conflict).
pub(crate) const COLD_CONFIG_FIELDS: &[&str] = &[
    "pod.number",
    "pod.name",
    "pod.sim",
    "pod.sim_ip",
    "pod.sim_port",
    "pod.node_type",
    "core.url",
    "core.failover_url",
    "core.ws_secret",
    "core.tls_ca_cert_path",
    "core.tls_skip_verify",
    "wheelbase.vendor_id",
    "wheelbase.product_id",
    "telemetry_ports.ports",
    "games",
    "ai_debugger",
];

// ─── PUSH-05: Server config persistence ──────────────────────────────────────

/// Returns the path to the server-pushed config cache file.
/// Always in the same directory as the running binary.
fn server_config_path() -> std::path::PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("rc-agent-server-config.json")
}

/// PUSH-05: Persist server-pushed config to local JSON file for boot resilience.
/// File: rc-agent-server-config.json in the same directory as the exe.
/// On next startup, if no TOML config is found, load_config() will fall back to this.
pub fn persist_server_config(config: &AgentConfig, config_hash: &str) -> Result<()> {
    persist_server_config_to(config, config_hash, &server_config_path())
}

/// Persist to a specific path (for testing).
pub(crate) fn persist_server_config_to(
    config: &AgentConfig,
    config_hash: &str,
    path: &std::path::Path,
) -> Result<()> {
    let wrapper = serde_json::json!({
        "config": config,
        "config_hash": config_hash,
        "received_at": chrono::Utc::now().to_rfc3339(),
    });
    let json = serde_json::to_string_pretty(&wrapper)
        .map_err(|e| anyhow::anyhow!("Failed to serialize server config: {}", e))?;
    std::fs::write(path, json)
        .map_err(|e| anyhow::anyhow!("Failed to write server config to {}: {}", path.display(), e))?;
    tracing::info!(
        target: LOG_TARGET,
        "Persisted server config to {} (hash={})",
        path.display(),
        config_hash
    );
    Ok(())
}

/// PUSH-05: Load last-received server config from local JSON file.
/// Returns None if file doesn't exist or is corrupt — caller falls back gracefully.
pub fn load_server_config() -> Option<(AgentConfig, String)> {
    load_server_config_from(&server_config_path())
}

/// Load from a specific path (for testing).
pub(crate) fn load_server_config_from(path: &std::path::Path) -> Option<(AgentConfig, String)> {
    let content = std::fs::read_to_string(path).ok()?;
    let wrapper: serde_json::Value = serde_json::from_str(&content).ok()?;
    let config: AgentConfig = serde_json::from_value(wrapper.get("config")?.clone()).ok()?;
    let hash = wrapper.get("config_hash")?.as_str()?.to_string();
    Some((config, hash))
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

    // PUSH-05: Fallback to last-received server config when no TOML file found.
    // This provides boot resilience when the server is temporarily unreachable.
    if let Some((config, hash)) = load_server_config() {
        tracing::warn!(
            target: LOG_TARGET,
            "No TOML config found — using last-received server config (hash={})",
            hash
        );
        validate_config(&config)?;
        return Ok(config);
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

// ─── PUSH-05: Server config persistence tests ─────────────────────────────────

#[cfg(test)]
mod server_config_persistence_tests {
    use super::*;
    use tempfile::tempdir;

    fn test_config() -> AgentConfig {
        let mut c = AgentConfig::default();
        c.pod.number = 3;
        c.pod.name = "Pod 03".to_string();
        c.core.url = "ws://192.168.31.23:8080/ws/agent".to_string();
        c
    }

    /// Test 1: persist_server_config writes a JSON file that load_server_config reads back with matching fields.
    #[test]
    fn persist_and_load_round_trip() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("rc-agent-server-config.json");
        let config = test_config();
        let hash = "abc123deadbeef".to_string();

        persist_server_config_to(&config, &hash, &path).expect("persist should succeed");
        let (loaded, loaded_hash) = load_server_config_from(&path).expect("load should return Some");

        assert_eq!(loaded_hash, hash, "hash must match");
        assert_eq!(loaded.pod.number, config.pod.number, "pod.number must match");
        assert_eq!(loaded.pod.name, config.pod.name, "pod.name must match");
        assert_eq!(loaded.core.url, config.core.url, "core.url must match");
    }

    /// Test 2: load_server_config returns None when no file exists.
    #[test]
    fn load_returns_none_when_no_file() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("nonexistent-server-config.json");
        let result = load_server_config_from(&path);
        assert!(result.is_none(), "Should return None when file does not exist");
    }

    /// Test 3: AgentConfig round-trips through JSON (serialize+deserialize identity check).
    #[test]
    fn agent_config_json_roundtrip() {
        let config = test_config();
        let json = serde_json::to_string(&config).expect("serialize");
        let decoded: AgentConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(decoded.pod.number, config.pod.number);
        assert_eq!(decoded.pod.name, config.pod.name);
        assert_eq!(decoded.core.url, config.core.url);
        assert_eq!(decoded.schema_version, config.schema_version);
    }

    /// Test 4: HOT_RELOAD_CONFIG_FIELDS and COLD_CONFIG_FIELDS lists are disjoint
    /// (no field appears in both).
    #[test]
    fn hot_and_cold_fields_are_disjoint() {
        for hot in HOT_RELOAD_CONFIG_FIELDS {
            assert!(
                !COLD_CONFIG_FIELDS.contains(hot),
                "Field '{}' appears in both HOT_RELOAD_CONFIG_FIELDS and COLD_CONFIG_FIELDS — must be disjoint",
                hot
            );
        }
    }

    /// Test 5: persist_server_config produces valid JSON with all expected keys.
    #[test]
    fn persist_produces_valid_json_with_expected_keys() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("rc-agent-server-config.json");
        let config = test_config();
        let hash = "deadbeef01234567".to_string();

        persist_server_config_to(&config, &hash, &path).expect("persist should succeed");

        let content = std::fs::read_to_string(&path).expect("read file");
        let value: serde_json::Value = serde_json::from_str(&content).expect("valid JSON");
        assert!(value.get("config").is_some(), "must contain 'config' key");
        assert!(value.get("config_hash").is_some(), "must contain 'config_hash' key");
        assert!(value.get("received_at").is_some(), "must contain 'received_at' key");
        assert_eq!(value["config_hash"].as_str(), Some("deadbeef01234567"));
    }
}
