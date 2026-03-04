use serde::Deserialize;

#[derive(Debug, Deserialize)]
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
}

#[derive(Debug, Deserialize)]
pub struct VenueConfig {
    pub name: String,
    #[serde(default = "default_location")]
    pub location: String,
    #[serde(default = "default_timezone")]
    pub timezone: String,
}

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
}

#[derive(Debug, Deserialize)]
pub struct DatabaseConfig {
    #[serde(default = "default_db_path")]
    pub path: String,
}

#[derive(Debug, Default, Deserialize)]
pub struct CloudConfig {
    #[serde(default)]
    pub enabled: bool,
    pub turso_url: Option<String>,
    /// Base URL for the cloud rc-core API (e.g., "https://app.racingpoint.cloud/api/v1")
    pub api_url: Option<String>,
    #[serde(default = "default_sync_interval")]
    pub sync_interval_secs: u64,
    /// Shared secret for terminal command access
    pub terminal_secret: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PodsConfig {
    #[serde(default = "default_pod_count")]
    pub count: u32,
    #[serde(default = "default_true")]
    pub discovery: bool,
    #[serde(default, rename = "static")]
    pub static_pods: Vec<StaticPodConfig>,
}

impl Default for PodsConfig {
    fn default() -> Self {
        Self {
            count: 16,
            discovery: true,
            static_pods: Vec::new(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct StaticPodConfig {
    pub number: u32,
    pub name: String,
    pub ip: String,
    pub sim: String,
}

#[derive(Debug, Default, Deserialize)]
pub struct BrandingConfig {
    pub logo: Option<String>,
    #[serde(default = "default_color")]
    pub primary_color: String,
    #[serde(default = "default_theme")]
    pub theme: String,
}

#[derive(Debug, Default, Deserialize)]
pub struct IntegrationsConfig {
    #[serde(default)]
    pub discord: DiscordConfig,
    #[serde(default)]
    pub whatsapp: WhatsAppConfig,
}

#[derive(Debug, Default, Deserialize)]
pub struct DiscordConfig {
    pub webhook_url: Option<String>,
    pub results_channel: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct WhatsAppConfig {
    #[serde(default)]
    pub enabled: bool,
    pub contact: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AiDebuggerConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_ollama_url")]
    pub ollama_url: String,
    #[serde(default = "default_ollama_model")]
    pub ollama_model: String,
    pub anthropic_api_key: Option<String>,
    #[serde(default = "default_anthropic_model")]
    pub anthropic_model: String,
    #[serde(default = "default_true")]
    pub chat_enabled: bool,
    #[serde(default = "default_true")]
    pub proactive_analysis: bool,
}

impl Default for AiDebuggerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            ollama_url: default_ollama_url(),
            ollama_model: default_ollama_model(),
            anthropic_api_key: None,
            anthropic_model: default_anthropic_model(),
            chat_enabled: true,
            proactive_analysis: true,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct AcServerConfig {
    #[serde(default = "default_acserver_path")]
    pub acserver_path: String,
    #[serde(default = "default_ac_data_dir")]
    pub data_dir: String,
    pub lan_ip: Option<String>,
}

impl Default for AcServerConfig {
    fn default() -> Self {
        Self {
            acserver_path: default_acserver_path(),
            data_dir: default_ac_data_dir(),
            lan_ip: None,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct AuthConfig {
    #[serde(default = "default_jwt_secret")]
    pub jwt_secret: String,
    #[serde(default = "default_pin_expiry")]
    pub pin_expiry_secs: u64,
    #[serde(default = "default_otp_expiry")]
    pub otp_expiry_secs: u64,
    pub evolution_url: Option<String>,
    pub evolution_api_key: Option<String>,
    pub evolution_instance: Option<String>,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            jwt_secret: default_jwt_secret(),
            pin_expiry_secs: default_pin_expiry(),
            otp_expiry_secs: default_otp_expiry(),
            evolution_url: None,
            evolution_api_key: None,
            evolution_instance: None,
        }
    }
}

impl Config {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let mut config: Config = toml::from_str(&content)?;
        config.apply_env_overrides();
        Ok(config)
    }

    pub fn load_or_default() -> Self {
        let paths = ["racecontrol.toml", "/etc/racecontrol/racecontrol.toml"];
        for path in paths {
            if let Ok(config) = Self::load(path) {
                tracing::info!("Loaded config from {}", path);
                return config;
            }
        }
        tracing::warn!("No config file found, using defaults");
        Self::default_config()
    }

    fn default_config() -> Self {
        Config {
            venue: VenueConfig {
                name: "RacingPoint".to_string(),
                location: default_location(),
                timezone: default_timezone(),
            },
            server: ServerConfig {
                host: default_host(),
                port: default_port(),
            },
            database: DatabaseConfig {
                path: default_db_path(),
            },
            cloud: CloudConfig::default(),
            pods: PodsConfig::default(),
            branding: BrandingConfig::default(),
            integrations: IntegrationsConfig::default(),
            ai_debugger: AiDebuggerConfig::default(),
            ac_server: AcServerConfig::default(),
            auth: AuthConfig::default(),
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
    }
}

fn default_host() -> String { "0.0.0.0".to_string() }
fn default_port() -> u16 { 8080 }
fn default_db_path() -> String { "./data/racecontrol.db".to_string() }
fn default_location() -> String { "Bandlaguda, Hyderabad".to_string() }
fn default_timezone() -> String { "Asia/Kolkata".to_string() }
fn default_sync_interval() -> u64 { 30 }
fn default_pod_count() -> u32 { 16 }
fn default_true() -> bool { true }
fn default_color() -> String { "#FF4400".to_string() }
fn default_theme() -> String { "dark".to_string() }
fn default_acserver_path() -> String { "/opt/ac-server/acServer".to_string() }
fn default_ac_data_dir() -> String { "./data/ac_servers".to_string() }
fn default_jwt_secret() -> String { "racingpoint-jwt-change-me-in-production".to_string() }
fn default_pin_expiry() -> u64 { 600 }
fn default_otp_expiry() -> u64 { 300 }
fn default_ollama_url() -> String { "http://localhost:11434".to_string() }
fn default_ollama_model() -> String { "llama3.1:8b".to_string() }
fn default_anthropic_model() -> String { "claude-sonnet-4-20250514".to_string() }
