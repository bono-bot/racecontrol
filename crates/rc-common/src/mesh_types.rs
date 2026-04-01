//! Shared types for v26.0 Meshed Intelligence — Self-Healing AI Fleet.
//!
//! Used by both racecontrol (server coordinator) and rc-agent (pod diagnostic engine).
//! All types derive Serialize/Deserialize for WS gossip + SQLite storage.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ─── Solution Status Lifecycle ──────────────────────────────────────────────

/// Lifecycle status of a mesh solution.
///
/// ```text
/// candidate → fleet_verified → hardened
///     ↓            ↓
///   demoted      retired
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SolutionStatus {
    /// Newly discovered by a single node, not yet verified across fleet.
    Candidate,
    /// Verified on 3+ successes across 2+ unique pods.
    FleetVerified,
    /// 10+ successes, zero failures — battle-tested.
    Hardened,
    /// Confidence dropped below threshold, demoted back for re-evaluation.
    Demoted,
    /// Manually or automatically retired (superseded, no longer applicable).
    Retired,
}

// ─── Fix Type ───────────────────────────────────────────────────────────────

/// Classification of what kind of fix a solution applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FixType {
    /// Local deterministic fix (kill process, clear sentinel, restart).
    Deterministic,
    /// Configuration change (TOML, registry, env var).
    Config,
    /// Service restart (rc-agent, racecontrol, ConspitLink).
    Restart,
    /// Requires a code change and redeploy.
    CodeChange,
    /// Requires human intervention.
    Manual,
    /// Physical hardware intervention needed (replace/clean/reconfigure).
    Hardware,
}

// ─── Diagnosis Tier ─────────────────────────────────────────────────────────

/// Which tier of the diagnostic engine found or applied this solution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosisTier {
    /// Tier 1: Deterministic checks (local, $0).
    Deterministic,
    /// Tier 2: Knowledge base lookup (local, $0).
    KnowledgeBase,
    /// Tier 3: Single model diagnosis ($0.05–$0.43).
    SingleModel,
    /// Tier 4: Full 4-model parallel diagnosis (~$3.01).
    MultiModel,
    /// Tier 5: Escalated to human.
    Human,
}

// ─── Mesh Solution ──────────────────────────────────────────────────────────

/// A diagnosed and verified solution to a specific problem.
/// Stored in local KB (per node) and fleet KB (server).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MeshSolution {
    /// Unique ID — hash of problem_key.
    pub id: String,
    /// Normalized problem signature (e.g. "rc-agent_crash_0xC0000005").
    pub problem_key: String,
    /// Hash of error + environment fingerprint for dedup.
    pub problem_hash: String,
    /// Symptoms: error message, stack trace, system state.
    pub symptoms: serde_json::Value,
    /// Environment: OS version, driver version, build_id, hardware.
    pub environment: serde_json::Value,
    /// Confirmed root cause description.
    pub root_cause: String,
    /// Steps to apply the fix (executable actions).
    pub fix_action: serde_json::Value,
    /// Classification of fix type.
    pub fix_type: FixType,
    /// Lifecycle status.
    pub status: SolutionStatus,
    /// Number of times this fix succeeded.
    pub success_count: u32,
    /// Number of times this fix failed.
    pub fail_count: u32,
    /// Confidence score: success_count / (success_count + fail_count).
    pub confidence: f64,
    /// Dollar cost spent diagnosing this problem.
    pub cost_to_diagnose: f64,
    /// Which AI models helped find this solution.
    pub models_used: Option<Vec<String>>,
    /// Which tier found the solution.
    pub diagnosis_tier: DiagnosisTier,
    /// Which node (pod number or "server") first solved this.
    pub source_node: String,
    /// Venue ID for multi-venue KB (Phase 227).
    pub venue_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// Increments on every update for conflict resolution.
    pub version: u32,
    /// Auto-expire after N days without use.
    pub ttl_days: u32,
    /// Categorization tags (e.g. ["game_launch", "display", "billing"]).
    pub tags: Option<Vec<String>>,
}

// ─── Mesh Experiment ────────────────────────────────────────────────────────

/// A hypothesis being tested as part of Cause Elimination (Phase D).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MeshExperiment {
    pub id: String,
    /// Links to the problem being investigated.
    pub problem_key: String,
    /// The hypothesis being tested.
    pub hypothesis: String,
    /// How to test this hypothesis.
    pub test_plan: String,
    /// Result of the experiment.
    pub result: Option<ExperimentResult>,
    /// Dollar cost of running this experiment.
    pub cost: f64,
    /// Which node ran this experiment.
    pub node: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExperimentResult {
    Confirmed,
    Eliminated,
    Inconclusive,
}

// ─── Mesh Heartbeat ─────────────────────────────────────────────────────────

/// Periodic heartbeat from each node to the server coordinator.
/// Used for fleet health + KB drift detection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshHeartbeat {
    /// Node identifier (e.g. "pod-3", "server").
    pub node_id: String,
    /// Number of solutions in local KB.
    pub kb_size: u32,
    /// Hash of all solution IDs — detects drift from fleet KB.
    pub kb_hash: String,
    /// Current daily budget remaining (dollars).
    pub budget_remaining: f64,
    /// Number of active diagnoses in progress.
    pub active_diagnoses: u32,
    /// Last diagnosis timestamp (if any).
    pub last_diagnosis: Option<DateTime<Utc>>,
    /// Node build_id for version tracking.
    pub build_id: String,
    /// Node uptime in seconds.
    pub uptime_secs: u64,
    pub timestamp: DateTime<Utc>,
}

// ─── Gossip Messages (WS) ──────────────────────────────────────────────────

/// All mesh gossip message types sent over the existing WS connection.
/// Prefixed with "mesh:" in the WS message type field.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MeshMessage {
    /// Node found a new solution → broadcast to server for fleet KB.
    #[serde(rename = "mesh:solution")]
    Solution(MeshSolutionAnnouncement),

    /// Node requests full solution details from server KB.
    #[serde(rename = "mesh:request_solution")]
    RequestSolution(MeshSolutionRequest),

    /// Server responds with full solution details.
    #[serde(rename = "mesh:solution_response")]
    SolutionResponse(Box<MeshSolution>),

    /// Node reports an experiment (for fleet-wide tracking).
    #[serde(rename = "mesh:experiment")]
    Experiment(MeshExperiment),

    /// Periodic heartbeat from node → server.
    #[serde(rename = "mesh:heartbeat")]
    Heartbeat(MeshHeartbeat),

    /// Server broadcasts a promoted solution to all nodes.
    #[serde(rename = "mesh:fleet_update")]
    FleetUpdate(MeshFleetUpdate),

    /// Server detects a systemic pattern → alert all nodes.
    #[serde(rename = "mesh:systemic_alert")]
    SystemicAlert(MeshSystemicAlert),

    /// Server sends KB delta to a node with drift.
    #[serde(rename = "mesh:kb_sync")]
    KbSync(MeshKbSync),
}

/// Compact announcement when a node discovers a new solution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshSolutionAnnouncement {
    pub solution_id: String,
    pub problem_key: String,
    pub problem_hash: String,
    pub fix_type: FixType,
    pub diagnosis_tier: DiagnosisTier,
    pub confidence: f64,
    pub source_node: String,
    pub cost: f64,
    pub timestamp: DateTime<Utc>,
}

/// Request for full solution details by ID or problem_key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshSolutionRequest {
    pub requesting_node: String,
    /// Look up by solution_id OR problem_key.
    pub solution_id: Option<String>,
    pub problem_key: Option<String>,
}

/// Server broadcasts a promoted/hardened solution to the fleet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshFleetUpdate {
    pub solution: Box<MeshSolution>,
    /// Why this update was sent.
    pub reason: FleetUpdateReason,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FleetUpdateReason {
    Promoted,
    Hardened,
    Retired,
    Updated,
}

/// Server detects a systemic pattern (3+ pods, same problem, within 5 min).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshSystemicAlert {
    pub pattern_id: String,
    pub problem_key: String,
    /// Which nodes reported this problem.
    pub affected_nodes: Vec<String>,
    /// Severity based on node count and customer impact.
    pub severity: SystemicSeverity,
    /// Recommended action from fleet KB (if available).
    pub recommended_solution: Option<String>,
    pub detected_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SystemicSeverity {
    /// 3+ pods affected, no customer impact confirmed.
    Warning,
    /// 3+ pods affected, customer impact likely.
    Critical,
    /// 5+ pods affected OR server affected.
    Emergency,
}

/// Server sends KB delta to a node whose kb_hash doesn't match fleet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshKbSync {
    pub target_node: String,
    /// Solutions to add/update on the target node.
    pub solutions: Vec<MeshSolution>,
    /// Solution IDs to remove (retired/demoted).
    pub remove_ids: Vec<String>,
    pub fleet_kb_hash: String,
    pub timestamp: DateTime<Utc>,
}

// ─── Budget Tracking ────────────────────────────────────────────────────────

/// Per-node budget status for the dashboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshBudgetStatus {
    pub node_id: String,
    pub daily_limit: f64,
    pub spent_today: f64,
    pub remaining: f64,
    /// Per-model breakdown of spend.
    pub model_spend: Vec<ModelSpend>,
    pub reset_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelSpend {
    pub model_id: String,
    pub calls: u32,
    pub total_cost: f64,
    pub findings: u32,
    pub false_positives: u32,
}

// ─── Incident Log ───────────────────────────────────────────────────────────

/// An incident recorded in the fleet incident log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshIncident {
    pub id: String,
    pub node: String,
    pub problem_key: String,
    pub severity: IncidentSeverity,
    /// Dollar cost of diagnosis.
    pub cost: f64,
    /// How it was resolved.
    pub resolution: Option<String>,
    /// Time from detection to resolution (seconds).
    pub time_to_resolve_secs: Option<u64>,
    /// Which tier resolved it.
    pub resolved_by_tier: Option<DiagnosisTier>,
    pub detected_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IncidentSeverity {
    Low,
    Medium,
    High,
    Critical,
}

// ─── v26.1 Agent Harness Types ──────────────────────────────────────────────

/// Graded evaluation of a solution by an adversarial evaluator (Principle 3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolutionGrade {
    /// Root cause accuracy (1-5): did we find the actual cause?
    pub root_cause_accuracy: u8,
    /// Fix completeness (1-5): handles all variants?
    pub fix_completeness: u8,
    /// Verification evidence (1-5): concrete proof it works?
    pub verification_evidence: u8,
    /// Side effect safety (1-5): breaks anything else?
    pub side_effect_safety: u8,
    /// Weighted total (0.0-5.0): 35% + 25% + 25% + 15%
    pub weighted_total: f64,
    /// Which model evaluated (MUST differ from diagnostician).
    pub evaluator_model: String,
    /// Evaluator's notes and reasoning.
    pub evaluator_notes: String,
    pub graded_at: DateTime<Utc>,
}

impl SolutionGrade {
    pub fn compute_weighted_total(rca: u8, fc: u8, ve: u8, ses: u8) -> f64 {
        (rca as f64 * 0.35) + (fc as f64 * 0.25) + (ve as f64 * 0.25) + (ses as f64 * 0.15)
    }
}

/// State of a diagnostic session running inside the harness (Principle 1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticSession {
    pub session_id: String,
    pub node: String,
    pub phase: DiagnosticPhase,
    pub anomaly_summary: String,
    pub hypotheses: Vec<Hypothesis>,
    /// Ralph Wiggum loop iteration count (Principle 6).
    pub ralph_wiggum_loops: u32,
    pub max_loops: u32,
    pub tokens_used: u64,
    pub budget_spent: f64,
    pub grade: Option<SolutionGrade>,
    pub started_at: DateTime<Utc>,
}

/// Diagnostic harness phases — must progress sequentially.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticPhase {
    Detect,
    Hypothesize,
    Test,
    Evaluate,
    Validate,
    Promote,
    Escalated,
}

/// A hypothesis being tracked through the Cause Elimination process.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hypothesis {
    pub id: String,
    pub description: String,
    pub status: HypothesisStatus,
    pub evidence: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HypothesisStatus {
    Untested,
    Testing,
    Confirmed,
    Eliminated,
}

/// A deterministic validation check in the Ralph Wiggum loop (Principle 6).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationCheck {
    pub check_type: String,
    pub target: String,
    pub expected: String,
    pub actual: Option<String>,
    pub passed: bool,
}

// ─── Cognitive Gate Protocol (CGP) Types ────────────────────────────────────

/// CGP gate identifiers — subset of the 10-gate protocol relevant to machine diagnosis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CgpGateId {
    /// G0: Problem Definition (PROBLEM/SYMPTOMS/PLAN)
    G0ProblemDefinition,
    /// G1: Outcome Verification (behavior tested + evidence)
    G1OutcomeVerification,
    /// G2: Fleet Scope (per-target applicability table)
    G2FleetScope,
    /// G4: Confidence Calibration (tested/not-tested/follow-up)
    G4ConfidenceCalibration,
    /// G5: Competing Hypotheses (2+ with falsification tests)
    G5CompetingHypotheses,
    /// G7: Tool Verification (model/approach selection)
    G7ToolVerification,
    /// G8: Dependency Cascade (downstream impact check)
    G8DependencyCascade,
    /// G9: Retrospective (root cause + prevention)
    G9Retrospective,
}

/// Outcome of evaluating a single CGP gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CgpGateStatus {
    Passed,
    Failed,
    /// Gate not applicable for this tier (e.g. G2 skipped for single-pod Tier 3).
    Skipped,
    /// Live incident — gate deferred per emergency bypass rules.
    EmergencyBypass,
}

/// Result of evaluating one CGP gate. Evidence is machine-readable JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CgpGateResult {
    pub gate: CgpGateId,
    pub status: CgpGateStatus,
    /// Structured proof artifact (e.g. problem/symptoms/plan for G0, hypotheses for G5).
    pub evidence: serde_json::Value,
    pub timestamp: DateTime<Utc>,
    pub duration_ms: u64,
}

// ─── Diagnosis Plan Manager Types ───────────────────────────────────────────

/// Status of a single step in a diagnosis plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanStepStatus {
    Todo,
    InProgress,
    Done,
    Blocked,
    Skipped,
}

/// One atomic step in a diagnosis plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosisPlanStep {
    pub step_id: u8,
    pub description: String,
    pub status: PlanStepStatus,
    /// Step IDs that must be Done before this step can start.
    pub depends_on: Vec<u8>,
    /// Output/result of this step (populated when Done).
    pub output: Option<serde_json::Value>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

/// A full diagnosis plan — atomic steps with dependency tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosisPlan {
    pub plan_id: String,
    /// Links to the DiagnosticEvent that spawned this plan.
    pub incident_id: String,
    pub problem_key: String,
    pub steps: Vec<DiagnosisPlanStep>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub tier: DiagnosisTier,
}

// ─── CGP + Plan + MMA Structured Audit Trail ────────────────────────────────

/// Complete audit trail for a Tier 3/4 diagnosis: CGP gates + plan steps + MMA result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredDiagnosisAudit {
    pub incident_id: String,
    pub problem_key: String,
    pub tier: DiagnosisTier,
    /// All CGP gates evaluated (Phase A + Phase D).
    pub cgp_gates: Vec<CgpGateResult>,
    /// Diagnosis plan with step-by-step progress.
    pub plan: Option<DiagnosisPlan>,
    /// MMA protocol result summary (serialized from MmaProtocolResult).
    pub mma_summary: Option<serde_json::Value>,
    /// Total dollar cost of this diagnosis.
    pub total_cost: f64,
    /// Total wall-clock duration in milliseconds.
    pub total_duration_ms: u64,
    pub timestamp: DateTime<Utc>,
}
