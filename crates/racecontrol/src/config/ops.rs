//! Operational configuration types: backup, event archive, presets.

use serde::Deserialize;

use super::default_true;

// ─── Backup Config (BACKUP-01, BACKUP-02) ────────────────────────────────────

/// Configuration for the SQLite backup pipeline.
/// All fields have serde defaults — no [backup] section needed in racecontrol.toml.
#[derive(Debug, Clone, Deserialize)]
pub struct BackupConfig {
    /// Enable the backup pipeline (default: true)
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Directory to store backup files (default: "./data/backups")
    #[serde(default = "default_backup_dir")]
    pub backup_dir: String,
    /// How often to run backups in seconds (default: 3600 = 1 hour)
    #[serde(default = "default_backup_interval_secs")]
    pub interval_secs: u64,
    /// Number of daily backup files to retain per database (default: 30, per OPS-09)
    #[serde(default = "default_daily_retain")]
    pub daily_retain: usize,
    /// Number of weekly backup files to retain per database (default: 4)
    #[serde(default = "default_weekly_retain")]
    pub weekly_retain: usize,
    /// Number of monthly backup files to retain per database (default: 12, per OPS-10)
    #[serde(default = "default_monthly_retain")]
    pub monthly_retain: usize,
    /// Path to admin.db for VACUUM INTO backup (default: empty = skip admin backup).
    /// Venue: "C:/RacingPoint/admin/data/admin.db" (per D-02, CONTEXT.md).
    #[serde(default)]
    pub admin_db_path: String,
    /// Use rsync instead of SCP for remote transfer (default: true, per OPS-11).
    /// Set to false if rsync.exe is unavailable on the host (SCP fallback is used automatically).
    #[serde(default = "default_true")]
    pub use_rsync: bool,
    /// Enable remote backup transfer via rsync/scp (default: true)
    #[serde(default = "default_true")]
    pub remote_enabled: bool,
    /// Remote host for backup transfers (default: Bono VPS)
    #[serde(default = "default_remote_host")]
    pub remote_host: String,
    /// Remote path for backup storage (default: /root/racecontrol-backups)
    #[serde(default = "default_remote_path")]
    pub remote_path: String,
    /// Hours without a successful backup before firing a WhatsApp alert (default: 2)
    #[serde(default = "default_staleness_alert_hours")]
    pub staleness_alert_hours: u64,
}

fn default_backup_dir() -> String { "./data/backups".to_string() }
fn default_backup_interval_secs() -> u64 { 3600 }
fn default_daily_retain() -> usize { 30 }
fn default_weekly_retain() -> usize { 4 }
fn default_monthly_retain() -> usize { 12 }
pub(crate) fn default_remote_host() -> String { "root@100.70.177.44".to_string() }
fn default_remote_path() -> String { "/root/racecontrol-backups".to_string() }
fn default_staleness_alert_hours() -> u64 { 2 }

impl Default for BackupConfig {
    fn default() -> Self {
        Self {
            enabled: default_true(),
            backup_dir: default_backup_dir(),
            interval_secs: default_backup_interval_secs(),
            daily_retain: default_daily_retain(),
            weekly_retain: default_weekly_retain(),
            monthly_retain: default_monthly_retain(),
            admin_db_path: String::new(),
            use_rsync: default_true(),
            remote_enabled: default_true(),
            remote_host: default_remote_host(),
            remote_path: default_remote_path(),
            staleness_alert_hours: default_staleness_alert_hours(),
        }
    }
}

// ─── Event Archive Config (EVENT-01 to EVENT-04, Phase 302) ──────────────────

/// Configuration for the structured event archive pipeline.
/// Stores system-wide events in SQLite, exports daily JSONL, purges after 90 days,
/// and SCPs JSONL files to Bono VPS nightly.
/// All fields have serde defaults — no [event_archive] section needed in racecontrol.toml.
#[derive(Debug, Clone, Deserialize)]
pub struct EventArchiveConfig {
    /// Enable the event archive pipeline (default: true)
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Directory to store JSONL export files (default: "./data/event-archive")
    #[serde(default = "default_event_archive_dir")]
    pub archive_dir: String,
    /// Enable remote JSONL transfer to Bono VPS (default: true)
    #[serde(default = "default_true")]
    pub remote_enabled: bool,
    /// Remote host for JSONL transfers (default: Bono VPS)
    #[serde(default = "default_remote_host")]
    pub remote_host: String,
    /// Remote path for JSONL storage (default: /root/racecontrol-event-archive)
    #[serde(default = "default_event_remote_path")]
    pub remote_path: String,
    /// Days to retain events in SQLite before purge (default: 90)
    #[serde(default = "default_retention_days")]
    pub retention_days: u32,
}

fn default_event_archive_dir() -> String { "./data/event-archive".to_string() }
fn default_event_remote_path() -> String { "/root/racecontrol-event-archive".to_string() }
fn default_retention_days() -> u32 { 90 }

impl Default for EventArchiveConfig {
    fn default() -> Self {
        Self {
            enabled: default_true(),
            archive_dir: default_event_archive_dir(),
            remote_enabled: default_true(),
            remote_host: default_remote_host(),
            remote_path: default_event_remote_path(),
            retention_days: default_retention_days(),
        }
    }
}

/// Phase 298 PRESET-04: Config for preset reliability scoring.
/// Presets below `unreliable_threshold` (and with >= 5 launches) are flagged as unreliable.
#[derive(Clone, Debug, Deserialize)]
pub struct PresetsConfig {
    /// Success rate below which a preset is flagged unreliable. Default 0.6 (60%).
    #[serde(default = "default_unreliable_threshold")]
    pub unreliable_threshold: f64,
}

fn default_unreliable_threshold() -> f64 { 0.6 }

impl Default for PresetsConfig {
    fn default() -> Self {
        Self { unreliable_threshold: default_unreliable_threshold() }
    }
}
