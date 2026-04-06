//! Shared diagnostic types used across rc-agent and racecontrol.
//!
//! These types are the common language for the Meshed Intelligence diagnostic pipeline:
//! - `DiagnosticTrigger`: Why a diagnostic cycle was initiated
//! - `TierDiagnosis`: Result of a single tier's diagnosis attempt
//! - `SolutionRecord`: A knowledge base solution entry (KB lookup result)
//!
//! Phase 322, Wave 0: Extract shared types from rc-agent into rc-common
//! so racecontrol server can participate in fleet-wide diagnostic coordination.

use serde::{Deserialize, Serialize};

/// All possible reasons a diagnostic cycle can be triggered.
/// Sent as part of DiagnosticEvent to the tier engine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagnosticTrigger {
    /// Scheduled 5-minute periodic scan (DIAG-07)
    Periodic,
    /// rc-agent health endpoint not responding
    HealthCheckFail,
    /// WerFault or abnormal process exit detected
    ProcessCrash { process_name: String },
    /// Game launch timed out (>90s, no game_pid)
    GameLaunchFail,
    /// Edge process count is 0 when blanking screen should be active
    DisplayMismatch { expected_edge_count: u32, actual_edge_count: u32 },
    /// Error log rate exceeded threshold (>5 errors/min)
    ErrorSpike { errors_per_min: u64 },
    /// WebSocket disconnected for more than 30s
    WsDisconnect { disconnected_secs: u64 },
    /// WebSocket instability -- high reconnect frequency (>=3 reconnects in 5 min).
    WsInstability { reconnects_5m: u64, reconnects_lifetime: u64 },
    /// Unexpected sentinel file found in C:\RacingPoint\
    SentinelUnexpected { file_name: String },
    /// Process guard violation count spiked (delta >50 in 5 min)
    ViolationSpike { delta: u64 },
    /// Pre-flight check failed -- emitted by ws_handler on BillingStarted failure.
    /// The check_name identifies which of the 11 checks failed (e.g. "hid", "conspit_link").
    /// Multiple PreFlightFailed events may be emitted per pre-flight run (one per failed check).
    PreFlightFailed {
        check_name: String,
        detail: String,
    },

    // --- POS-Specific Triggers (v26.0 Meshed Intelligence -- POS node) ---
    /// POS: Kiosk Edge browser not running or unresponsive
    PosKioskDown { detail: String },
    /// POS: Network connectivity to racecontrol server lost (HTTP probe failed)
    PosNetworkDown { server_ip: String, detail: String },
    /// POS: Billing API unresponsive or returning errors
    PosBillingApiError { endpoint: String, status_code: u16 },
    /// POS: WiFi signal degraded -- high latency or weak signal during active operations
    /// MMA consensus (4/4): RSSI < -70dBm or latency > 500ms = transaction risk
    PosWifiDegraded { rssi_dbm: i32, latency_ms: u64 },
    /// POS: Edge kiosk escaped -- non-kiosk window in foreground (security + ops risk)
    /// MMA consensus (4/4): foreground != msedge.exe for > 10s
    PosKioskEscaped { foreground_process: String },

    // --- UI State Triggers (DIAG-01n: taskbar enforcement) ---
    /// Taskbar was found visible when it should be hidden (kiosk mode active).
    /// This indicates explorer.exe restarted and ShowWindow(SW_HIDE) was lost.
    TaskbarVisible,

    // --- MMA-First Protocol Triggers (v31.0) ---
    /// Game crashed mid-session (process exited while billing was active).
    /// Rich context: exit code, session duration, game/track/car at time of crash.
    GameMidSessionCrash { exit_code: Option<i32>, session_duration_secs: u64 },
    /// Post-session quality analysis -- lightweight MMA call after session ends.
    /// Evaluates session quality (micro-stutters, telemetry gaps, FPS drops).
    /// Quality score as integer percentage (0-100) to maintain Eq derive.
    PostSessionAnalysis { session_quality_pct: u8 },
    /// Pre-shift health audit -- comprehensive check before venue opens.
    /// Runs full MMA diagnosis on each pod's overnight health state.
    PreShiftAudit,
    /// Post-deploy verification -- validates new binary after OTA deploy.
    DeployVerification { new_build_id: String },

    /// Phase 318 (LAUNCH-01): Server detected game launch timeout -- no playable signal
    /// within the configured window. Agent feeds this into tier engine for recovery.
    /// elapsed_secs: total seconds from launch command to timeout.
    GameLaunchTimeout { elapsed_secs: u64 },
}

/// Result of a single tier's diagnosis attempt.
/// Used for fleet-wide diagnostic coordination and reporting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierDiagnosis {
    pub trigger: DiagnosticTrigger,
    pub tier: u8,
    /// "fixed", "failed_to_fix", "not_applicable", "stub"
    pub outcome: String,
    pub action: String,
    pub root_cause: String,
    pub fix_type: String,
    pub confidence: f64,
    pub fix_applied: bool,
    pub problem_hash: String,
    /// IST ISO-8601 timestamp
    pub timestamp: String,
}

/// A knowledge base solution record -- shared representation of a Solution row.
///
/// Renamed from `Solution` (rc-agent internal) to `SolutionRecord` to avoid
/// name collisions when used across crates. rc-agent re-exports as `Solution`
/// via type alias for backward compatibility.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolutionRecord {
    pub id: String,
    pub problem_key: String,
    pub problem_hash: String,
    pub symptoms: String,
    pub environment: String,
    pub root_cause: String,
    pub fix_action: String,
    pub fix_type: String,
    pub success_count: i64,
    pub fail_count: i64,
    pub confidence: f64,
    pub cost_to_diagnose: f64,
    pub models_used: Option<String>,
    pub source_node: String,
    pub created_at: String,
    pub updated_at: String,
    pub version: i64,
    pub ttl_days: i64,
    pub tags: Option<String>,
    /// MMA diagnostic methodology that found this solution.
    /// Values: "scanner_enumeration", "reasoner_absence", "sre_stuck_state",
    /// "code_expert_session0", "security_checklist", "consensus_5model",
    /// "deterministic", "fleet_gossip", or model-specific role names.
    /// Enables the fleet to learn not just WHAT to fix but HOW to diagnose.
    pub diagnosis_method: Option<String>,
    /// MMA-First Protocol: whether this is a workaround or permanent fix.
    /// Values: "workaround", "permanent", "pending_permanent", "fallback"
    #[serde(default = "default_fix_permanence")]
    pub fix_permanence: String,
    /// How many times Q1 has applied this solution (issue recurrence count).
    #[serde(default)]
    pub recurrence_count: i64,
    /// Links a workaround to its permanent replacement solution ID.
    #[serde(default)]
    pub permanent_fix_id: Option<String>,
    /// ISO 8601 timestamp of last Q1 application.
    #[serde(default)]
    pub last_recurrence: Option<String>,
    /// ISO 8601 timestamp of last Q4 permanent fix attempt.
    #[serde(default)]
    pub permanent_attempt_at: Option<String>,
}

fn default_fix_permanence() -> String {
    "workaround".to_string()
}

/// A single step in a structured fix plan produced by rc-sentry's DiagnosisPlanner.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedAction {
    pub step: u8,
    /// Shell command or descriptive instruction (not always executable directly).
    pub command: String,
    /// "safe" | "caution" | "dangerous"
    pub risk_level: String,
    /// How to undo this step if it makes things worse.
    pub rollback: String,
    /// What should be observable after this step succeeds.
    pub expected_outcome: String,
}

/// A structured fix plan produced by rc-sentry's cognitive gate + diagnosis planner.
/// Exposed via :8091/gate/last-plan endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosisPlan {
    /// Canonical name of the failure pattern (e.g. "rc-agent-crash", "game-stuck").
    pub trigger_name: String,
    /// Which MI tier produced this plan.
    pub tier: u8,
    /// Ordered list of actions to resolve the failure.
    pub actions: Vec<PlannedAction>,
    /// ISO-8601 UTC timestamp when plan was generated.
    pub created_at: String,
    /// Confidence that these actions will resolve the issue (0.0-1.0).
    pub confidence: f64,
    /// Summary of cognitive gate results (G0/G5/G7 pass/fail/skipped).
    pub gate_summary: Vec<String>,
}

/// Map a DiagnosticTrigger to a stable problem key for KB lookup.
/// Matches the logic in rc-agent/src/knowledge_base.rs -- must stay in sync.
pub fn normalize_problem_key(trigger: &DiagnosticTrigger) -> String {
    match trigger {
        DiagnosticTrigger::HealthCheckFail => "health_check_fail".into(),
        DiagnosticTrigger::ProcessCrash { process_name } =>
            format!("process_crash:{}", process_name.to_lowercase()),
        DiagnosticTrigger::GameLaunchFail => "game_launch_fail".into(),
        DiagnosticTrigger::GameLaunchTimeout { .. } => "game_launch_timeout".into(),
        DiagnosticTrigger::DisplayMismatch { .. } => "display_mismatch".into(),
        DiagnosticTrigger::ErrorSpike { .. } => "error_spike".into(),
        DiagnosticTrigger::WsDisconnect { .. } => "ws_disconnect".into(),
        DiagnosticTrigger::WsInstability { .. } => "ws_instability".into(),
        DiagnosticTrigger::SentinelUnexpected { file_name } =>
            format!("sentinel_unexpected:{}", file_name.to_lowercase()),
        DiagnosticTrigger::ViolationSpike { .. } => "violation_spike".into(),
        DiagnosticTrigger::PreFlightFailed { check_name, .. } =>
            format!("preflight_failed:{}", check_name.to_lowercase()),
        DiagnosticTrigger::TaskbarVisible => "taskbar_visible".into(),
        DiagnosticTrigger::GameMidSessionCrash { .. } => "game_mid_session_crash".into(),
        DiagnosticTrigger::PostSessionAnalysis { .. } => "post_session_analysis".into(),
        DiagnosticTrigger::PreShiftAudit => "pre_shift_audit".into(),
        DiagnosticTrigger::DeployVerification { .. } => "deploy_verification".into(),
        DiagnosticTrigger::Periodic => "periodic".into(),
        _ => format!("unknown:{:?}", trigger).to_lowercase(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_trigger_roundtrip_json() {
        let triggers = vec![
            DiagnosticTrigger::Periodic,
            DiagnosticTrigger::HealthCheckFail,
            DiagnosticTrigger::ProcessCrash { process_name: "acs.exe".to_string() },
            DiagnosticTrigger::GameLaunchFail,
            DiagnosticTrigger::DisplayMismatch { expected_edge_count: 3, actual_edge_count: 0 },
            DiagnosticTrigger::ErrorSpike { errors_per_min: 10 },
            DiagnosticTrigger::WsDisconnect { disconnected_secs: 45 },
            DiagnosticTrigger::PreFlightFailed { check_name: "hid".to_string(), detail: "no device".to_string() },
            DiagnosticTrigger::GameLaunchTimeout { elapsed_secs: 120 },
        ];

        for trigger in &triggers {
            let json = serde_json::to_string(trigger).expect("serialize trigger");
            let back: DiagnosticTrigger = serde_json::from_str(&json).expect("deserialize trigger");
            assert_eq!(*trigger, back);
        }
    }

    #[test]
    fn tier_diagnosis_serialize() {
        let diag = TierDiagnosis {
            trigger: DiagnosticTrigger::HealthCheckFail,
            tier: 1,
            outcome: "fixed".to_string(),
            action: "restarted health endpoint".to_string(),
            root_cause: "port conflict".to_string(),
            fix_type: "deterministic".to_string(),
            confidence: 0.95,
            fix_applied: true,
            problem_hash: "abc123".to_string(),
            timestamp: "2026-04-06T14:30:00+05:30".to_string(),
        };

        let json = serde_json::to_string(&diag).expect("serialize TierDiagnosis");
        let back: TierDiagnosis = serde_json::from_str(&json).expect("deserialize TierDiagnosis");
        assert_eq!(back.tier, 1);
        assert_eq!(back.outcome, "fixed");
        assert!(back.fix_applied);
    }

    #[test]
    fn solution_record_default_fix_permanence() {
        // Verify serde default works for fix_permanence
        let json = r#"{
            "id": "sol-1",
            "problem_key": "test",
            "problem_hash": "hash",
            "symptoms": "s",
            "environment": "e",
            "root_cause": "r",
            "fix_action": "a",
            "fix_type": "deterministic",
            "success_count": 5,
            "fail_count": 0,
            "confidence": 0.9,
            "cost_to_diagnose": 0.0,
            "models_used": null,
            "source_node": "pod-1",
            "created_at": "2026-04-06",
            "updated_at": "2026-04-06",
            "version": 1,
            "ttl_days": 30,
            "tags": null,
            "diagnosis_method": null
        }"#;

        let record: SolutionRecord = serde_json::from_str(json).expect("deserialize SolutionRecord");
        assert_eq!(record.fix_permanence, "workaround");
        assert_eq!(record.recurrence_count, 0);
        assert!(record.permanent_fix_id.is_none());
    }
}
