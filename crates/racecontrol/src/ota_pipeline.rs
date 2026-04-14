//! OTA Pipeline — state-machine-driven fleet deployment with canary-first waves,
//! SHA256 binary verification, session gating, and auto-rollback.
//!
//! v22.0 Phase 179: Deploys rc-agent and rc-sentry to 8 pods via a gated pipeline:
//! Wave 1 (canary Pod 8) → Wave 2 (Pods 1-4) → Wave 3 (Pods 5-7).
//! Each wave verifies health gates (WS connected, HTTP reachable, SHA256 match)
//! before advancing. Pods with active billing sessions are deferred, not failed.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

// ── Submodules ─────────────────────────────────────────────────────────────
#[path = "ota_pipeline_ops.rs"]
pub mod ops;

pub use ops::{
    clear_ota_sentinel, has_active_billing_session, rollback_wave, set_kill_switch,
    set_ota_sentinel,
};

#[cfg(test)]
#[path = "ota_pipeline_tests.rs"]
mod tests;

// ── Wave constants (OTA-02, OTA-06) ────────────────────────────────────────
/// Canary wave: Pod 8 always goes first.
pub const WAVE_1: &[u32] = &[8];
/// Second wave: Pods 1-4.
pub const WAVE_2: &[u32] = &[1, 2, 3, 4];
/// Third wave: remaining Pods 5-7.
pub const WAVE_3: &[u32] = &[5, 6, 7];

/// All waves in deployment order.
pub const ALL_WAVES: &[&[u32]] = &[WAVE_1, WAVE_2, WAVE_3];

// ── State file path ────────────────────────────────────────────────────────
/// Location of the pipeline state file on the server.
const DEPLOY_STATE_FILE: &str = r"C:\RacingPoint\deploy-state.json";

// ── ReleaseManifest (OTA-01, OTA-10, SYNC-05) ─────────────────────────────

/// A release manifest locks binary SHA256, config schema version, frontend
/// build_id, git commit, and timestamp as one immutable bundle.
/// No manifest = no deploy starts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReleaseManifest {
    pub release: ReleaseInfo,
    pub binaries: BinaryHashes,
    pub compatibility: CompatibilityMatrix,
    pub deploy: DeployConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReleaseInfo {
    pub version: String,
    /// ISO-8601 timestamp in IST (e.g. "2026-03-24T15:00:00+05:30")
    pub timestamp: String,
    pub git_commit: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BinaryHashes {
    pub rc_agent_sha256: String,
    pub rc_sentry_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CompatibilityMatrix {
    pub racecontrol_min_version: String,
    pub config_schema_version: u32,
    pub kiosk_build_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeployConfig {
    pub binary_url_base: String,
}

/// Parse a TOML string into a ReleaseManifest.
pub fn parse_manifest(toml_str: &str) -> Result<ReleaseManifest, String> {
    toml::from_str(toml_str).map_err(|e| format!("manifest parse error: {e}"))
}

// ── PipelineState (OTA-08) ─────────────────────────────────────────────────

/// State machine for the OTA deploy pipeline.
/// Persisted to deploy-state.json so the pipeline survives server restarts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PipelineState {
    Idle,
    Building,
    Staging,
    Canary,
    StagedRollout,
    HealthChecking,
    Completed,
    RollingBack,
    Paused, // HUMAN-CONFIRM gate pending operator confirmation
}

impl PipelineState {
    /// Returns true if this state represents a terminal (non-active) condition.
    pub fn is_terminal(&self) -> bool {
        matches!(self, PipelineState::Idle | PipelineState::Completed)
    }
}

// ── DeployRecord (OTA-08) ──────────────────────────────────────────────────

/// Persisted pipeline state — written atomically to deploy-state.json.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployRecord {
    pub state: PipelineState,
    pub manifest_version: String,
    pub started_at: String,
    pub updated_at: String,
    pub waves_completed: u8,
    pub failed_pods: Vec<String>,
    pub rollback_reason: Option<String>,
}

impl DeployRecord {
    /// Create a new deploy record for a fresh pipeline run.
    pub fn new(manifest_version: &str) -> Self {
        let now = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%:z").to_string();
        Self {
            state: PipelineState::Idle,
            manifest_version: manifest_version.to_string(),
            started_at: now.clone(),
            updated_at: now,
            waves_completed: 0,
            failed_pods: Vec::new(),
            rollback_reason: None,
        }
    }

    /// Update the record timestamp and state.
    pub fn transition(&mut self, new_state: PipelineState) {
        self.state = new_state;
        self.updated_at = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%:z").to_string();
    }
}

// ── SHA256 utilities (OTA-10) ──────────────────────────────────────────────

/// Compute SHA256 hex digest of a byte slice.
pub fn compute_sha256(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

/// Compute SHA256 hex digest of a file using streaming reads (8KB chunks).
/// Does NOT load the entire file into memory.
pub fn compute_sha256_file(path: &std::path::Path) -> Result<String, String> {
    use std::io::Read;
    let mut file = std::fs::File::open(path)
        .map_err(|e| format!("open failed: {e}"))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let n = file.read(&mut buffer).map_err(|e| format!("read failed: {e}"))?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

// ── deploy-state.json persistence ──────────────────────────────────────────

/// Atomically persist pipeline state to deploy-state.json (tmp file + rename).
/// If the process dies mid-write, the old file remains intact.
pub fn persist_pipeline_state(record: &DeployRecord) -> Result<(), String> {
    let json = serde_json::to_string_pretty(record)
        .map_err(|e| format!("serialize failed: {e}"))?;
    let tmp_path = format!("{DEPLOY_STATE_FILE}.tmp");
    std::fs::write(&tmp_path, &json)
        .map_err(|e| format!("write tmp failed: {e}"))?;
    std::fs::rename(&tmp_path, DEPLOY_STATE_FILE)
        .map_err(|e| format!("rename failed: {e}"))?;
    Ok(())
}

/// Load pipeline state from deploy-state.json.
/// Returns None if file does not exist or contains invalid JSON.
pub fn load_pipeline_state() -> Option<DeployRecord> {
    let data = std::fs::read_to_string(DEPLOY_STATE_FILE).ok()?;
    serde_json::from_str(&data).ok()
}

/// Check if a previous pipeline was interrupted (non-terminal state on startup).
/// If found, logs a warning and marks it as interrupted.
pub fn check_interrupted_pipeline() {
    if let Some(mut record) = load_pipeline_state() {
        if !record.state.is_terminal() {
            tracing::warn!(
                state = ?record.state,
                version = %record.manifest_version,
                waves_completed = record.waves_completed,
                "Interrupted OTA pipeline detected on startup — marking as interrupted"
            );
            record.rollback_reason = Some("server_restart_interrupted".to_string());
            record.transition(PipelineState::RollingBack);
            if let Err(e) = persist_pipeline_state(&record) {
                tracing::error!("Failed to persist interrupted pipeline state: {e}");
            }
        }
    }
}

// ── Standing Rules Gate (SR-04) ─────────────────────────────────────────────

/// Exit codes from gate-check.sh
#[derive(Debug, PartialEq, Eq)]
pub enum GateResult {
    Pass,           // exit 0 -- all checks passed
    Fail(String),   // exit 1 -- gate failure, must rollback
    HumanConfirm,   // exit 2 -- HUMAN-CONFIRM items pending, must pause
}

/// Run gate-check.sh with the specified mode.
/// Returns GateResult based on exit code.
pub fn run_gate_check(mode: &str) -> GateResult {
    let repo_root = std::env::current_dir().unwrap_or_default();
    let script = repo_root.join("test").join("gate-check.sh");

    if !script.exists() {
        return GateResult::Fail(format!("gate-check.sh not found at {}", script.display()));
    }

    let output = std::process::Command::new("bash")
        .arg(&script)
        .arg(mode)
        .current_dir(&repo_root)
        .output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            match out.status.code() {
                Some(0) => GateResult::Pass,
                Some(2) => GateResult::HumanConfirm,
                Some(code) => GateResult::Fail(format!(
                    "gate-check.sh exited with code {}\nstdout: {}\nstderr: {}",
                    code,
                    stdout.chars().take(500).collect::<String>(),
                    stderr.chars().take(500).collect::<String>()
                )),
                None => GateResult::Fail("gate-check.sh killed by signal".to_string()),
            }
        }
        Err(e) => GateResult::Fail(format!("Failed to run gate-check.sh: {}", e)),
    }
}

/// Run pre-deploy gate. Returns the state the pipeline should transition to.
/// On Pass: proceed to Canary
/// On Fail: RollingBack
/// On HumanConfirm: Paused
pub fn run_pre_deploy_gate() -> PipelineState {
    match run_gate_check("--pre-deploy") {
        GateResult::Pass => PipelineState::Canary,
        GateResult::Fail(reason) => {
            tracing::error!("Pre-deploy gate FAILED: {}", reason);
            PipelineState::RollingBack
        }
        GateResult::HumanConfirm => {
            tracing::warn!("Pre-deploy gate requires HUMAN-CONFIRM -- pipeline paused");
            PipelineState::Paused
        }
    }
}

/// Run post-wave gate. Returns the state the pipeline should transition to.
pub fn run_post_wave_gate(wave: u32) -> PipelineState {
    match run_gate_check(&format!("--post-wave {}", wave)) {
        GateResult::Pass => PipelineState::StagedRollout,
        GateResult::Fail(reason) => {
            tracing::error!("Post-wave {} gate FAILED: {}", wave, reason);
            PipelineState::RollingBack
        }
        GateResult::HumanConfirm => {
            tracing::warn!("Post-wave {} gate requires HUMAN-CONFIRM -- pipeline paused", wave);
            PipelineState::Paused
        }
    }
}

/// Resume pipeline from Paused state after operator confirmation.
/// Only valid when current state is Paused.
pub fn resume_from_pause(record: &mut DeployRecord) -> Result<(), String> {
    if record.state != PipelineState::Paused {
        return Err(format!(
            "Cannot resume: pipeline is in {:?}, not Paused",
            record.state
        ));
    }
    // Re-run the gate check to confirm operator has resolved all items
    match run_gate_check("--pre-deploy") {
        GateResult::Pass => {
            record.transition(PipelineState::Canary);
            Ok(())
        }
        GateResult::Fail(reason) => {
            record.transition(PipelineState::RollingBack);
            Err(format!("Gate still failing after resume: {}", reason))
        }
        GateResult::HumanConfirm => {
            Err("HUMAN-CONFIRM items still pending".to_string())
        }
    }
}

// ── Health Gate (OTA-02, OTA-10) ───────────────────────────────────────────

/// Error spike threshold — pods with more than this many violations fail the health gate.
const ERROR_SPIKE_THRESHOLD: u32 = 100;

/// Pipeline errors with structured context for logging and rollback decisions.
#[derive(Debug)]
pub enum PipelineError {
    ManifestInvalid(String),
    HealthGateFailed { wave: u8, failures: Vec<HealthFailure> },
    SessionTimeout { pod_id: String },
    RollbackTriggered { wave: u8, reason: String },
    PodNotFound(String),
    PersistFailed(String),
}

impl std::fmt::Display for PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PipelineError::ManifestInvalid(e) => write!(f, "manifest invalid: {e}"),
            PipelineError::HealthGateFailed { wave, failures } => {
                write!(f, "health gate failed on wave {wave}: ")?;
                for (i, fail) in failures.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}={}", fail.pod_id, fail.reason)?;
                }
                Ok(())
            }
            PipelineError::SessionTimeout { pod_id } => write!(f, "session timeout on {pod_id}"),
            PipelineError::RollbackTriggered { wave, reason } => write!(f, "rollback on wave {wave}: {reason}"),
            PipelineError::PodNotFound(id) => write!(f, "pod not found: {id}"),
            PipelineError::PersistFailed(e) => write!(f, "persist failed: {e}"),
        }
    }
}

/// A single health check failure for a specific pod.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthFailure {
    pub pod_id: String,
    pub reason: String,
}

/// Verify a pod's health after deploy. Uses binary SHA256 (OTA-10) for identity,
/// not git commit hash. Pure function — testable without AppState.
///
/// - `binary_sha256`: from pod's /health endpoint (computed once at startup)
/// - `expected_sha256`: from manifest.binaries.rc_agent_sha256
pub fn health_check_pod(
    _pod_id: &str,
    ws_connected: bool,
    http_reachable: bool,
    binary_sha256: Option<&str>,
    expected_sha256: &str,
    violation_count_24h: u32,
    scan_failure_count: u32,
) -> Result<(), String> {
    if !ws_connected {
        return Err("ws_disconnected".to_string());
    }
    if !http_reachable {
        return Err("http_unreachable".to_string());
    }
    match binary_sha256 {
        Some(sha) if sha == expected_sha256 => {}
        Some(_sha) => return Err("sha256_mismatch".to_string()),
        None => return Err("sha256_missing".to_string()),
    }
    if violation_count_24h > ERROR_SPIKE_THRESHOLD {
        return Err(format!(
            "error_spike: {} violations (threshold {})",
            violation_count_24h, ERROR_SPIKE_THRESHOLD
        ));
    }
    // MMA-P1: Fail-closed on scan failures — a broken scanner must NOT be
    // treated as "0 violations". Block OTA if any scan failures occurred.
    if scan_failure_count > 0 {
        return Err(format!(
            "scan_failed: {} scan failures — process guard scanner is broken, cannot verify pod safety",
            scan_failure_count
        ));
    }
    Ok(())
}
