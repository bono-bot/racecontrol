//! Configuration module for RaceControl.
//!
//! Split into submodules by domain:
//! - `infra` — venue, server, database, cloud, pods, branding, MMA
//! - `services` — auth, billing, watchdog, monitoring, integrations, process guard, backup

use serde::Deserialize;
use rc_common::verification::{ColdVerificationChain, VerifyStep, VerificationError};

pub mod infra;
pub mod ops;
pub mod services;

// Re-export all public types so `use crate::config::Config` etc. keep working.
pub use infra::{
    AlertCondition, MetricAlertRule, MmaConfig, GmailConfig,
    VenueConfig, ServerConfig, MtlsConfig, DatabaseConfig,
    CloudConfig, PodsConfig, StaticPodConfig, BrandingConfig,
};
pub use services::{
    IntegrationsConfig, DiscordConfig, WhatsAppConfig,
    AiDebuggerConfig, AcServerConfig, AuthConfig,
    WhatsAppCategory, EvolutionCredentials,
    WatchdogConfig, MonitoringConfig, AlertingConfig,
    CafeConfig, BillingConfig,
    AllowedProcess, ProcessGuardOverride, ProcessGuardConfig,
    BonoConfig,
};
pub use ops::{
    BackupConfig, EventArchiveConfig, PresetsConfig,
};

// ─── Shared default helpers (used by submodules via `super::`) ──────────────

pub(crate) fn default_true() -> bool { true }
pub(crate) fn default_false() -> bool { false }

// ─── Main Config struct ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub venue: VenueConfig,
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    #[serde(default)]
    pub cloud: CloudConfig,
    #[serde(default)]
    pub pods: PodsConfig,
    #[serde(default)]
    pub branding: BrandingConfig,
    #[serde(default)]
    pub integrations: IntegrationsConfig,
    #[serde(default)]
    pub ai_debugger: AiDebuggerConfig,
    #[serde(default)]
    pub ac_server: AcServerConfig,
    #[serde(default)]
    pub auth: AuthConfig,
    #[serde(default)]
    pub watchdog: WatchdogConfig,
    #[serde(default)]
    pub bono: BonoConfig,
    #[serde(default)]
    pub gmail: GmailConfig,
    #[serde(default)]
    pub monitoring: MonitoringConfig,
    #[serde(default)]
    pub alerting: AlertingConfig,
    #[serde(default)]
    pub process_guard: ProcessGuardConfig,
    #[serde(default)]
    pub cafe: CafeConfig,
    #[serde(default)]
    pub billing: BillingConfig,
    #[serde(default)]
    pub mma: MmaConfig,
    #[serde(default)]
    pub alert_rules: Vec<MetricAlertRule>,
    #[serde(default)]
    pub backup: BackupConfig,
    /// Phase 302: Structured event archive pipeline config.
    #[serde(default)]
    pub event_archive: EventArchiveConfig,
    /// Phase 298 PRESET-04: Preset reliability scoring config.
    #[serde(default)]
    pub presets: PresetsConfig,
}

impl Config {
    /// Resolve Evolution API credentials by message category.
    /// Marketing messages use `integrations.whatsapp.marketing_*` if configured,
    /// falling back to `auth.evolution_*`. This ensures marketing can be routed
    /// through Bono VPS while operational messages stay on the venue tunnel.
    pub fn evolution_for(&self, category: WhatsAppCategory) -> Option<EvolutionCredentials> {
        match category {
            WhatsAppCategory::Operational => {
                Some(EvolutionCredentials {
                    url: self.auth.evolution_url.clone()?,
                    api_key: self.auth.evolution_api_key.clone()?,
                    instance: self.auth.evolution_instance.clone()?,
                })
            }
            WhatsAppCategory::Marketing => {
                let wa = &self.integrations.whatsapp;
                // Use dedicated marketing config if set, otherwise fall back to operational
                let url = wa.marketing_url.as_ref()
                    .or(self.auth.evolution_url.as_ref())?.clone();
                let key = wa.marketing_api_key.as_ref()
                    .or(self.auth.evolution_api_key.as_ref())?.clone();
                let inst = wa.marketing_instance.as_ref()
                    .or(self.auth.evolution_instance.as_ref())?.clone();
                Some(EvolutionCredentials { url, api_key: key, instance: inst })
            }
        }
    }
}

/// Phase 343: Returns true if this racecontrol instance IS the cloud (authoritative writer).
/// Cloud does NOT reject staff mutations — it's the source of truth.
///
/// Fix 2026-04-18: the prior heuristic tested `api_url.contains(":{self_port}")`, which
/// matched venue instances too whenever venue + cloud shared a common port (both on :8080).
/// Consequence: BOTH instances self-identified as cloud → BOTH rejected venue-authoritative
/// writes → kiosk_settings became unwritable via API on the entire deployment. Observed
/// during James PoE 2026-04-18 while trying to flip kiosk_lockdown_enabled=false.
/// New heuristic requires an explicit loopback/localhost host+port match. Instances
/// that need to be treated as cloud must either (a) match a loopback api_url or
/// (b) set RC_IS_CLOUD=1 in their environment.
pub fn this_instance_is_cloud(config: &Config) -> bool {
    if std::env::var("RC_IS_CLOUD").as_deref() == Ok("1") {
        return true;
    }
    // Heuristic: if cloud api_url points at our own loopback:port, we ARE the cloud.
    // Port-only substring is INSUFFICIENT — venue + cloud commonly share the same port.
    if let Some(ref api_url) = config.cloud.api_url {
        let loopback_v4 = format!("127.0.0.1:{}", config.server.port);
        let loopback_v6 = format!("[::1]:{}", config.server.port);
        let localhost = format!("localhost:{}", config.server.port);
        if api_url.contains(&loopback_v4) || api_url.contains(&loopback_v6) || api_url.contains(&localhost) {
            return true;
        }
    }
    false
}

/// Phase 343: Emergency override — allows venue staff writes even when cloud-authoritative.
/// Set RC_ALLOW_VENUE_STAFF_WRITE=1 on the venue instance for break-glass scenarios.
pub fn allow_venue_staff_write() -> bool {
    std::env::var("RC_ALLOW_VENUE_STAFF_WRITE").as_deref() == Ok("1")
}

/// Phase 349: Emergency override — allows cloud instance to write venue-authoritative tables.
/// Set RC_ALLOW_CLOUD_VENUE_WRITE=1 on the cloud instance for break-glass scenarios.
pub fn allow_cloud_venue_write() -> bool {
    std::env::var("RC_ALLOW_CLOUD_VENUE_WRITE").as_deref() == Ok("1")
}

// ─── JWT Secret Resolution ──────────────────────────────────────────────────

/// Resolve JWT signing secret: env var > config value > auto-generate.
/// The dangerous default "racingpoint-jwt-change-me-in-production" is treated as unset.
fn resolve_jwt_secret(config_value: &str) -> String {
    // 1. Environment variable takes priority
    if let Ok(key) = std::env::var("RACECONTROL_JWT_SECRET")
        && !key.is_empty() {
            tracing::info!("Using JWT secret from RACECONTROL_JWT_SECRET env var");
            return key;
        }
    // 2. Config file value (if not the dangerous default and not empty)
    if config_value != "racingpoint-jwt-change-me-in-production" && !config_value.is_empty() {
        return config_value.to_string();
    }
    // 3. Generate random 256-bit key
    use rand::Rng;
    let key_bytes: [u8; 32] = rand::thread_rng().r#gen();
    let hex_key: String = key_bytes.iter().map(|b| format!("{:02x}", b)).collect();
    tracing::warn!(
        "No JWT secret configured — generated random key. \
         Tokens will be invalidated on restart. \
         Set RACECONTROL_JWT_SECRET env var for persistence."
    );
    hex_key
}

// ─── Verification chain steps for config TOML load (COV-03) ──────────────────

struct StepConfigFileReadable;
impl VerifyStep for StepConfigFileReadable {
    type Input = String;   // file path
    type Output = (String, String);  // (file content, path)
    fn name(&self) -> &str { "file_readable" }
    fn run(&self, input: String) -> Result<(String, String), VerificationError> {
        std::fs::read_to_string(&input)
            .map(|content| (content, input.clone()))
            .map_err(|e| VerificationError::InputParseError {
                step: self.name().to_string(),
                raw_value: format!("path={} error={}", input, e),
            })
    }
}

struct StepConfigTomlParse;
impl VerifyStep for StepConfigTomlParse {
    type Input = (String, String);  // (content, path)
    type Output = Config;
    fn name(&self) -> &str { "toml_parse" }
    fn run(&self, input: (String, String)) -> Result<Config, VerificationError> {
        let (content, path) = input;
        toml::from_str::<Config>(&content).map_err(|e| {
            // COV-03: Log first 3 lines to help diagnose SSH banner corruption
            let first_3_lines: String = content.lines().take(3).collect::<Vec<_>>().join(" | ");
            VerificationError::InputParseError {
                step: self.name().to_string(),
                raw_value: format!("path={} error={} first_3_lines=[{}]", path, e, first_3_lines),
            }
        })
    }
}

struct StepValidateCriticalFields;
impl VerifyStep for StepValidateCriticalFields {
    type Input = Config;
    type Output = Config;
    fn name(&self) -> &str { "validate_critical_fields" }
    fn run(&self, input: Config) -> Result<Config, VerificationError> {
        // Check that critical fields are not at their default values
        let default = Config::default_config();
        let mut fallbacks = Vec::new();
        if input.database.path == default.database.path {
            fallbacks.push("database.path");
        }
        if !fallbacks.is_empty() {
            // COV-03: Emit TransformError through chain for tracing span capture.
            // Config is still usable — caller catches this as non-fatal warning.
            // Using eprintln as well because tracing may not be initialized during config load.
            eprintln!("[config_validate] fields at default values: {:?}", fallbacks);
            return Err(VerificationError::TransformError {
                step: self.name().to_string(),
                raw_value: format!("fields_at_default={:?}", fallbacks),
            });
        }
        Ok(input)
    }
}

// ─── Config loading ─────────────────────────────────────────────────────────

impl Config {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let mut config: Config = toml::from_str(&content)?;
        config.apply_env_overrides();
        Ok(config)
    }

    pub fn load_or_default() -> Self {
        // Build the search path list. Always try:
        //   1. CWD-relative (for dev / explicit cd-before-launch scenarios)
        //   2. Directory of the running executable (reliable for schtasks / HKLM Run / watchdog restarts
        //      where CWD is not guaranteed to match the install directory)
        //   3. /etc/racecontrol/ (Linux/VPS deployments)
        let mut paths: Vec<String> = vec!["racecontrol.toml".to_string()];
        if let Ok(exe_path) = std::env::current_exe()
            && let Some(exe_dir) = exe_path.parent() {
                let exe_cfg = exe_dir.join("racecontrol.toml");
                let exe_cfg_str = exe_cfg.to_string_lossy().into_owned();
                // Only add if different from CWD-relative (avoid duplicate on happy path)
                if exe_cfg_str != "racecontrol.toml" {
                    paths.push(exe_cfg_str);
                }
            }
        paths.push("/etc/racecontrol/racecontrol.toml".to_string());

        for path in &paths {
            let chain = ColdVerificationChain::new("config_load");
            // Step 1: Check file is readable
            match chain.execute_step(&StepConfigFileReadable, path.clone()) {
                Ok((content, path_display)) => {
                    // Step 2: Parse TOML
                    match chain.execute_step(&StepConfigTomlParse, (content, path_display.clone())) {
                        Ok(mut config) => {
                            config.apply_env_overrides();
                            // Step 3: Validate critical fields (non-fatal — TransformError means config is usable but has defaults)
                            let validated_config = chain.execute_step(&StepValidateCriticalFields, config.clone());
                            match validated_config {
                                Ok(config) => {
                                    eprintln!("[config] Loaded config from {}", path_display);
                                    tracing::info!("Loaded config from {}", path_display);
                                    return config;
                                }
                                Err(e) => {
                                    // COV-03: TransformError flows through chain tracing span for structured logging.
                                    // Config is still usable — proceed with it but warn.
                                    tracing::warn!(target: "state", error = %e, path = %path_display, "config field validation detected default fallbacks — using config anyway");
                                    eprintln!("[config] Loaded config from {} (with field validation warnings)", path_display);
                                    return config;
                                }
                            }
                        }
                        Err(e) => {
                            // GAP-1 FIX (MMA 5/5 consensus): File exists but parse failed → PANIC.
                            // Silent fallback to Config::default() caused server to run with
                            // empty config for ALL settings (v31.0 incident).
                            let msg = format!(
                                "[FATAL] Config file '{}' exists but failed to parse: {}\n\
                                 Fix the config file and restart. The server will NOT run on defaults \
                                 when a config file is present but invalid.",
                                path_display, e
                            );
                            eprintln!("{}", msg);
                            panic!("{}", msg);
                        }
                    }
                }
                Err(_) => {
                    // File not readable — expected for most search paths, skip silently
                }
            }
        }
        // OBS-02: No config file found — emit structured warn (eprintln always, tracing if initialized)
        let msg = "[config_fallback] field=config_file source=racecontrol.toml fallback=Config::default() — config file not found, using defaults".to_string();
        eprintln!("{}", msg);
        tracing::warn!(target: "state", field = "config_file", source = "racecontrol.toml", fallback = "Config::default()", "config file not found, using defaults");
        Self::default_config()
    }

    /// Create a default config suitable for tests.
    pub fn default_test() -> Self {
        Self::default_config()
    }

    pub(crate) fn default_config() -> Self {
        Config {
            venue: VenueConfig {
                name: "RacingPoint".to_string(),
                location: infra::default_location(),
                timezone: infra::default_timezone(),
                venue_gstin: infra::default_venue_gstin(),
                venue_id: infra::default_venue_id(),
            },
            server: ServerConfig {
                host: infra::default_host(),
                port: infra::default_port(),
                tls_port: None,
                cert_path: None,
                key_path: None,
                tls: MtlsConfig::default(),
                mdns_enabled: true,
                config_dir: "./deploy/configs".to_string(),
            },
            database: DatabaseConfig {
                path: infra::default_db_path(),
            },
            cloud: CloudConfig::default(),
            pods: PodsConfig::default(),
            branding: BrandingConfig::default(),
            integrations: IntegrationsConfig::default(),
            ai_debugger: AiDebuggerConfig::default(),
            ac_server: AcServerConfig::default(),
            auth: AuthConfig::default(),
            watchdog: WatchdogConfig::default(),
            bono: BonoConfig::default(),
            gmail: GmailConfig::default(),
            monitoring: MonitoringConfig::default(),
            alerting: AlertingConfig::default(),
            process_guard: ProcessGuardConfig::default(),
            cafe: CafeConfig::default(),
            billing: BillingConfig::default(),
            mma: MmaConfig::default(),
            alert_rules: Vec::new(),
            backup: BackupConfig::default(),
            event_archive: EventArchiveConfig::default(),
            presets: PresetsConfig::default(),
        }
    }

    fn apply_env_overrides(&mut self) {
        if let Ok(url) = std::env::var("OLLAMA_URL") {
            tracing::info!("Overriding ollama_url from OLLAMA_URL env var");
            self.ai_debugger.ollama_url = url;
        }
        if let Ok(model) = std::env::var("OLLAMA_MODEL") {
            tracing::info!("Overriding ollama_model from OLLAMA_MODEL env var");
            self.ai_debugger.ollama_model = model;
        }
        if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
            tracing::info!("Overriding anthropic_api_key from ANTHROPIC_API_KEY env var");
            self.ai_debugger.anthropic_api_key = Some(key);
        }

        // --- Secret env var overrides (AUDIT-03) ---
        // JWT secret is handled specially via resolve_jwt_secret (supports auto-generation)
        self.auth.jwt_secret = resolve_jwt_secret(&self.auth.jwt_secret);

        if let Ok(val) = std::env::var("RACECONTROL_ADMIN_PIN_HASH")
            && !val.is_empty() {
                tracing::info!("Overriding admin_pin_hash from RACECONTROL_ADMIN_PIN_HASH env var");
                self.auth.admin_pin_hash = Some(val);
            }

        if let Ok(val) = std::env::var("RACECONTROL_TERMINAL_SECRET")
            && !val.is_empty() {
                tracing::info!("Overriding terminal_secret from RACECONTROL_TERMINAL_SECRET env var");
                self.cloud.terminal_secret = Some(val);
            }
        if let Ok(val) = std::env::var("RACECONTROL_RELAY_SECRET")
            && !val.is_empty() {
                tracing::info!("Overriding relay_secret from RACECONTROL_RELAY_SECRET env var");
                self.bono.relay_secret = Some(val);
            }
        if let Ok(val) = std::env::var("RACECONTROL_EVOLUTION_API_KEY")
            && !val.is_empty() {
                tracing::info!("Overriding evolution_api_key from RACECONTROL_EVOLUTION_API_KEY env var");
                self.auth.evolution_api_key = Some(val);
            }
        if let Ok(val) = std::env::var("RACECONTROL_GMAIL_CLIENT_SECRET")
            && !val.is_empty() {
                tracing::info!("Overriding gmail.client_secret from RACECONTROL_GMAIL_CLIENT_SECRET env var");
                self.gmail.client_secret = Some(val);
            }
        if let Ok(val) = std::env::var("RACECONTROL_GMAIL_REFRESH_TOKEN")
            && !val.is_empty() {
                tracing::info!("Overriding gmail.refresh_token from RACECONTROL_GMAIL_REFRESH_TOKEN env var");
                self.gmail.refresh_token = Some(val);
            }
        if let Ok(val) = std::env::var("RACECONTROL_SYNC_HMAC_KEY")
            && !val.is_empty() {
                tracing::info!("Overriding sync_hmac_key from RACECONTROL_SYNC_HMAC_KEY env var");
                self.cloud.sync_hmac_key = Some(val);
            }
    }
}

#[cfg(test)]
pub(crate) mod tests;
