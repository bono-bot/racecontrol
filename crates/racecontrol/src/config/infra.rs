//! Infrastructure configuration types: venue, server, database, cloud, pods, branding, MMA.

use serde::Deserialize;

use super::default_true;

// ─── Metric Alert Types (ALRT-01, ALRT-02) ────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertCondition {
    Gt,
    Lt,
    Eq,
}

fn default_alert_severity() -> String {
    "warn".to_string()
}

#[derive(Debug, Clone, Deserialize)]
pub struct MetricAlertRule {
    pub name: String,
    pub metric: String,
    pub condition: AlertCondition,
    pub threshold: f64,
    #[serde(default = "default_alert_severity")]
    pub severity: String,
    pub message_template: String,
}

/// Unified MMA Protocol v3.0 config (v31.0+) — 30-day AI training period settings
#[derive(Clone, Debug, Default, Deserialize)]
pub struct MmaConfig {
    #[serde(default)]
    pub training_mode: bool,
    pub training_start: Option<String>,
    pub training_end: Option<String>,
    #[serde(default = "default_daily_budget_pod")]
    pub daily_budget_pod: f64,
    #[serde(default = "default_daily_budget_server")]
    pub daily_budget_server: f64,
    #[serde(default = "default_daily_budget_pos")]
    pub daily_budget_pos: f64,
}

fn default_daily_budget_pod() -> f64 { 15.0 }
fn default_daily_budget_server() -> f64 { 25.0 }
fn default_daily_budget_pos() -> f64 { 8.0 }

/// Gmail API config for sending notification emails (track record beaten, etc.)
/// Uses OAuth2 refresh_token flow — no external script needed.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct GmailConfig {
    #[serde(default)]
    pub enabled: bool,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub refresh_token: Option<String>,
    #[serde(default = "default_gmail_from")]
    pub from_email: String,
}

fn default_gmail_from() -> String { "james@racingpoint.in".to_string() }

#[derive(Debug, Clone, Deserialize)]
pub struct VenueConfig {
    pub name: String,
    #[serde(default = "default_location")]
    pub location: String,
    #[serde(default = "default_timezone")]
    pub timezone: String,
    /// GST Identification Number (15-char alphanumeric). Used for invoice generation.
    #[serde(default = "default_venue_gstin")]
    pub venue_gstin: String,
    /// Venue identifier for multi-venue schema (Phase 303, VENUE-01).
    /// Default 'racingpoint-hyd-001' — backward compatible, no TOML change required.
    #[serde(default = "default_venue_id")]
    pub venue_id: String,
}

pub(crate) fn default_venue_gstin() -> String {
    "36PLACEHOLDER0Z0".to_string()
}

pub(crate) fn default_venue_id() -> String {
    "racingpoint-hyd-001".to_string()
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    /// HTTPS port. When set, enables TLS listener alongside HTTP.
    #[serde(default)]
    pub tls_port: Option<u16>,
    /// Path to TLS certificate PEM file. Auto-generated if missing.
    #[serde(default)]
    pub cert_path: Option<String>,
    /// Path to TLS private key PEM file. Auto-generated if missing.
    #[serde(default)]
    pub key_path: Option<String>,
    /// v38.0 mTLS configuration (venue CA + mutual TLS).
    /// `[server.tls]` in racecontrol.toml. Defaults to disabled (plain HTTP).
    #[serde(default)]
    pub tls: MtlsConfig,
    /// mDNS auto-discovery: advertise `_racecontrol._tcp.local.` on the LAN.
    /// Pods with `core.mdns_enabled = true` will find this server without hardcoded IPs.
    #[serde(default = "default_true")]
    pub mdns_enabled: bool,
    /// Directory containing per-pod TOML files (`rc-agent-pod{N}.toml`) that the
    /// `/api/v1/pods/{id}/inventory` endpoint reads to expose pod content
    /// inventory to kiosk/admin clients. On production server .23 this is
    /// `C:\RacingPoint\deploy\configs`; on dev machines it defaults to
    /// `./deploy/configs`. Phase 361-01.
    #[serde(default = "default_config_dir")]
    pub config_dir: String,
}

fn default_config_dir() -> String {
    "./deploy/configs".to_string()
}

impl ServerConfig {
    /// Expand `config_dir` into a filesystem `PathBuf`. Separate helper so
    /// handlers do not need to clone the underlying String.
    pub fn config_dir_path(&self) -> std::path::PathBuf {
        std::path::PathBuf::from(&self.config_dir)
    }
}

/// Mutual TLS configuration for the racecontrol server (v38.0 Phase 305).
///
/// When `enabled = false` (default), the server uses plain HTTP -- no change
/// from pre-305 behaviour. Set `enabled = true` after running
/// `bash scripts/generate-venue-ca.sh` to generate venue certs.
#[derive(Debug, Clone, Deserialize)]
pub struct MtlsConfig {
    /// Enable venue CA TLS on the main :8080 listener. Default: false (plain HTTP).
    #[serde(default)]
    pub enabled: bool,
    /// Path to the venue CA certificate PEM.
    #[serde(default = "default_ca_cert_path")]
    pub ca_cert_path: String,
    /// Path to the server certificate PEM (signed by venue CA).
    #[serde(default = "default_server_cert_path")]
    pub server_cert_path: String,
    /// Path to the server private key PEM.
    #[serde(default = "default_server_key_path")]
    pub server_key_path: String,
    /// Require clients to present a valid venue CA-signed certificate (full mTLS).
    /// Default: false (TLS only -- safe phase-in mode).
    #[serde(default)]
    pub require_client_cert: bool,
    /// Allow Tailscale connections (100.64.0.0/10) to bypass client cert requirement.
    /// Default: true -- Tailscale already provides end-to-end encryption.
    #[serde(default = "default_true")]
    pub tailscale_bypass: bool,
}

impl Default for MtlsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            ca_cert_path: default_ca_cert_path(),
            server_cert_path: default_server_cert_path(),
            server_key_path: default_server_key_path(),
            require_client_cert: false,
            tailscale_bypass: true,
        }
    }
}

fn default_ca_cert_path() -> String {
    if cfg!(windows) {
        "C:\\RacingPoint\\tls\\ca.pem".to_string()
    } else {
        "/etc/racingpoint/tls/ca.pem".to_string()
    }
}

fn default_server_cert_path() -> String {
    if cfg!(windows) {
        "C:\\RacingPoint\\tls\\server.pem".to_string()
    } else {
        "/etc/racingpoint/tls/server.pem".to_string()
    }
}

fn default_server_key_path() -> String {
    if cfg!(windows) {
        "C:\\RacingPoint\\tls\\server-key.pem".to_string()
    } else {
        "/etc/racingpoint/tls/server-key.pem".to_string()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    #[serde(default = "default_db_path")]
    pub path: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct CloudConfig {
    #[serde(default)]
    pub enabled: bool,
    pub turso_url: Option<String>,
    /// Base URL for the cloud racecontrol API (e.g., "https://app.racingpoint.cloud/api/v1")
    pub api_url: Option<String>,
    #[serde(default = "default_sync_interval")]
    pub sync_interval_secs: u64,
    /// How often to poll for cloud actions (default: 3 seconds)
    #[serde(default = "default_action_poll_interval")]
    pub action_poll_interval_secs: u64,
    /// Shared secret for terminal command access
    pub terminal_secret: Option<String>,
    /// PIN for terminal web UI authentication (only Uday knows this)
    pub terminal_pin: Option<String>,
    /// Localhost URL for the comms-link relay (e.g., "http://localhost:8765" on cloud,
    /// "http://localhost:8766" on venue). When set, sync routes through the relay for
    /// real-time 2s sync instead of 30s HTTP polling.
    #[serde(default)]
    pub comms_link_url: Option<String>,
    /// HMAC-SHA256 key for signing cloud sync payloads (AUTH-07).
    /// When set, outbound sync requests include x-sync-signature, x-sync-timestamp,
    /// x-sync-nonce headers. Inbound requests are verified (permissive mode initially).
    #[serde(default)]
    pub sync_hmac_key: Option<String>,
    /// Identity of this racecontrol instance for sync origin tagging.
    /// Set to "local" on venue server, "cloud" on VPS. Prevents sync loops.
    #[serde(default = "default_origin_local")]
    pub origin_id: String,
    /// Phase 343: Tables that are cloud-authoritative. Venue mutations to these
    /// tables return 409 Conflict, directing the caller to the cloud endpoint.
    #[serde(default = "default_authoritative_tables")]
    pub authoritative_tables: Vec<String>,
}

fn default_authoritative_tables() -> Vec<String> {
    vec!["staff_members".to_string()]
}

impl CloudConfig {
    /// Phase 343: Check if a table is cloud-authoritative on this instance.
    /// Returns true only when cloud sync is enabled AND the table is in the list.
    pub fn is_cloud_authoritative_for(&self, table: &str) -> bool {
        self.enabled && self.authoritative_tables.iter().any(|t| t == table)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct PodsConfig {
    #[serde(default = "default_pod_count")]
    pub count: u32,
    #[serde(default = "default_true")]
    pub discovery: bool,
    #[serde(default, rename = "static")]
    pub static_pods: Vec<StaticPodConfig>,
    #[serde(default = "default_true")]
    pub healer_enabled: bool,
    #[serde(default = "default_healer_interval")]
    pub healer_interval_secs: u32,
    /// Service key for authenticating with rc-sentry :8091/exec on pods.
    /// Must match the RCSENTRY_SERVICE_KEY env var set on each pod.
    #[serde(default)]
    pub sentry_service_key: Option<String>,
}

impl Default for PodsConfig {
    fn default() -> Self {
        Self {
            count: 16,
            discovery: true,
            static_pods: Vec::new(),
            healer_enabled: true,
            healer_interval_secs: default_healer_interval(),
            sentry_service_key: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct StaticPodConfig {
    pub number: u32,
    pub name: String,
    pub ip: String,
    pub sim: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct BrandingConfig {
    pub logo: Option<String>,
    #[serde(default = "default_color")]
    pub primary_color: String,
    #[serde(default = "default_theme")]
    pub theme: String,
}

// ─── Default functions ───────────────────────────────────────────────────────

pub(crate) fn default_host() -> String { "0.0.0.0".to_string() }
pub(crate) fn default_port() -> u16 { 8080 }
pub(crate) fn default_db_path() -> String { "./data/racecontrol.db".to_string() }
pub(crate) fn default_location() -> String { "Bandlaguda, Hyderabad".to_string() }
pub(crate) fn default_timezone() -> String { "Asia/Kolkata".to_string() }
fn default_sync_interval() -> u64 { 30 }
fn default_action_poll_interval() -> u64 { 3 }
fn default_origin_local() -> String { "local".to_string() }
fn default_pod_count() -> u32 { 16 }
fn default_color() -> String { "#E10600".to_string() }
fn default_theme() -> String { "dark".to_string() }
fn default_healer_interval() -> u32 { 120 }
