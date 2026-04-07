//! Cognitive Gate Protocol (CGP) Engine for rc-sentry.
//!
//! Adapted from rc-agent/src/cognitive_gate.rs -- operates on TierDiagnosis
//! instead of DiagnosticEvent (avoids FailureMonitorState dependency).
//!
//! Gates:
//!   Phase A (pre-action): G0 Problem Definition, G5 Competing Hypotheses, G7 Tool Verification
//!   Phase D (post-action): G1 Outcome Verification, G4 Confidence Calibration, G8 Dependency Cascade
//!
//! DiagnosisPlanner: static match table -> Vec<PlannedAction> for 5 known failure patterns.
//!
//! Phase 323, MIG-06.

use chrono::Utc;
use rc_common::diagnostic_types::{
    DiagnosticTrigger, DiagnosisPlan, PlannedAction, TierDiagnosis,
    normalize_problem_key,
};
use rc_common::mesh_types::{CgpGateId, CgpGateResult, CgpGateStatus};
use serde_json::json;
use std::sync::{Mutex, OnceLock};

const LOG_TARGET: &str = "cognitive-gate";

// --- Last Plan State ---------------------------------------------------------

static LAST_PLAN: OnceLock<Mutex<Option<DiagnosisPlan>>> = OnceLock::new();

fn last_plan_mutex() -> &'static Mutex<Option<DiagnosisPlan>> {
    LAST_PLAN.get_or_init(|| Mutex::new(None))
}

/// Store the most recently generated DiagnosisPlan (for /gate/last-plan endpoint).
pub fn store_last_plan(plan: DiagnosisPlan) {
    if let Ok(mut guard) = last_plan_mutex().lock() {
        *guard = Some(plan);
    }
}

/// Retrieve the most recently generated DiagnosisPlan (None if none yet).
pub fn get_last_plan() -> Option<DiagnosisPlan> {
    last_plan_mutex()
        .lock()
        .ok()
        .and_then(|guard| guard.clone())
}

// --- CGP Engine --------------------------------------------------------------

pub struct CgpEngine;

impl CgpEngine {
    /// Phase A: Pre-action gates -- run BEFORE any AI model call.
    /// Accepts TierDiagnosis (not DiagnosticEvent).
    /// Returns Err only if G0 (Problem Definition) fails.
    pub fn run_phase_a(diag: &TierDiagnosis) -> Result<Vec<CgpGateResult>, CgpError> {
        let mut gates = Vec::with_capacity(3);

        let g0 = Self::gate_g0_problem_definition(&diag.trigger);
        if g0.status == CgpGateStatus::Failed {
            tracing::error!(target: LOG_TARGET, "G0 Problem Definition FAILED -- aborting");
            return Err(CgpError::CriticalGateFailed(CgpGateId::G0ProblemDefinition));
        }
        gates.push(g0);

        // G5: KB access via mi_knowledge_base
        gates.push(Self::gate_g5_competing_hypotheses(&diag.trigger));
        // G7: tier from TierDiagnosis.tier field
        gates.push(Self::gate_g7_tool_verification(&diag.trigger, diag.tier));

        Ok(gates)
    }

    /// Phase D: Post-action gates -- run AFTER fix plan is generated.
    pub fn run_phase_d(diag: &TierDiagnosis, plan_action_count: usize) -> Vec<CgpGateResult> {
        vec![
            Self::gate_g1_outcome_verification(plan_action_count > 0),
            Self::gate_g4_confidence_calibration(diag),
            Self::gate_g8_dependency_cascade(&diag.trigger),
        ]
    }

    // --- Phase A Gates -------------------------------------------------------

    fn gate_g0_problem_definition(trigger: &DiagnosticTrigger) -> CgpGateResult {
        let start = std::time::Instant::now();
        let problem = trigger_to_problem(trigger);
        let plan = trigger_to_plan(trigger);
        let status = if problem.is_empty() { CgpGateStatus::Failed } else { CgpGateStatus::Passed };
        CgpGateResult {
            gate: CgpGateId::G0ProblemDefinition,
            status,
            evidence: json!({ "problem": problem, "plan": plan }),
            timestamp: Utc::now(),
            duration_ms: start.elapsed().as_millis() as u64,
        }
    }

    fn gate_g5_competing_hypotheses(trigger: &DiagnosticTrigger) -> CgpGateResult {
        let start = std::time::Instant::now();
        let hypotheses = trigger_to_hypotheses(trigger);

        // KB-sourced hypotheses via mi_knowledge_base
        let problem_key = normalize_problem_key(trigger);
        let mut all_hyps: Vec<serde_json::Value> = hypotheses.iter()
            .map(|(h, f)| json!({"source": "heuristic", "hypothesis": h, "falsification": f}))
            .collect();

        if let Ok(kb) = crate::mi_knowledge_base::KnowledgeBase::open(crate::mi_knowledge_base::KB_PATH) {
            if let Ok(solutions) = kb.lookup_all(&problem_key, 3) {
                for sol in &solutions {
                    all_hyps.push(json!({
                        "source": "knowledge_base",
                        "hypothesis": format!("Prior fix: {}", sol.root_cause),
                        "confidence": sol.confidence,
                    }));
                }
            }
        }

        let status = if all_hyps.len() >= 2 { CgpGateStatus::Passed } else { CgpGateStatus::Failed };

        CgpGateResult {
            gate: CgpGateId::G5CompetingHypotheses,
            status,
            evidence: json!({ "hypotheses": all_hyps, "count": all_hyps.len() }),
            timestamp: Utc::now(),
            duration_ms: start.elapsed().as_millis() as u64,
        }
    }

    fn gate_g7_tool_verification(trigger: &DiagnosticTrigger, tier: u8) -> CgpGateResult {
        let start = std::time::Instant::now();
        let (requirement, tool) = match tier {
            3 => (format!("Single-model diagnosis for {:?}", trigger),
                  "qwen/qwen3-235b-a22b-2507 (cheapest)".to_string()),
            4 | 5 => (format!("5-model MMA for {:?}", trigger),
                      "5-model stratified pool (Reasoner+CodeExpert+SRE+Domain+Generalist)".to_string()),
            _ => (format!("Deterministic for {:?}", trigger), "none -- Tier 1/2".to_string()),
        };
        CgpGateResult {
            gate: CgpGateId::G7ToolVerification,
            status: CgpGateStatus::Passed,
            evidence: json!({ "requirement": requirement, "tool": tool, "tier": tier }),
            timestamp: Utc::now(),
            duration_ms: start.elapsed().as_millis() as u64,
        }
    }

    // --- Phase D Gates -------------------------------------------------------

    fn gate_g1_outcome_verification(plan_generated: bool) -> CgpGateResult {
        let start = std::time::Instant::now();
        CgpGateResult {
            gate: CgpGateId::G1OutcomeVerification,
            status: if plan_generated { CgpGateStatus::Passed } else { CgpGateStatus::Failed },
            evidence: json!({
                "behavior_tested": "fix plan generated",
                "method": "DiagnosisPlanner.plan_for_trigger()",
                "plan_generated": plan_generated,
            }),
            timestamp: Utc::now(),
            duration_ms: start.elapsed().as_millis() as u64,
        }
    }

    fn gate_g4_confidence_calibration(diag: &TierDiagnosis) -> CgpGateResult {
        let start = std::time::Instant::now();
        let not_tested = match &diag.trigger {
            DiagnosticTrigger::GameLaunchFail | DiagnosticTrigger::GameLaunchTimeout { .. } => {
                vec!["Game actually launches successfully after fix", "Customer can play a full session"]
            },
            DiagnosticTrigger::WsDisconnect { .. } => {
                vec!["WebSocket stays connected for >5 minutes", "No data loss during reconnection"]
            },
            _ => vec!["Long-term stability (>1 hour)", "Edge cases under load"],
        };
        CgpGateResult {
            gate: CgpGateId::G4ConfidenceCalibration,
            status: CgpGateStatus::Passed,
            evidence: json!({
                "tested": ["DiagnosisPlanner generated fix sequence"],
                "not_tested": not_tested,
                "follow_up": "Monitor for recurrence via periodic scan",
            }),
            timestamp: Utc::now(),
            duration_ms: start.elapsed().as_millis() as u64,
        }
    }

    fn gate_g8_dependency_cascade(trigger: &DiagnosticTrigger) -> CgpGateResult {
        let start = std::time::Instant::now();
        let (downstream, risk) = match trigger {
            DiagnosticTrigger::ProcessCrash { .. } => {
                (vec!["All services depending on the crashed process"], "medium")
            },
            DiagnosticTrigger::WsDisconnect { .. } => {
                (vec!["Billing state sync", "Game launch commands", "Fleet health reporting"], "high")
            },
            DiagnosticTrigger::DisplayMismatch { .. } => {
                (vec!["Customer experience", "Lock screen blanking state"], "high")
            },
            _ => (vec!["No critical downstream dependencies identified"], "low"),
        };
        CgpGateResult {
            gate: CgpGateId::G8DependencyCascade,
            status: CgpGateStatus::Passed,
            evidence: json!({ "downstream": downstream, "risk_level": risk }),
            timestamp: Utc::now(),
            duration_ms: start.elapsed().as_millis() as u64,
        }
    }
}

// --- CGP Error ---------------------------------------------------------------

#[derive(Debug)]
pub enum CgpError {
    CriticalGateFailed(CgpGateId),
}

// --- Diagnosis Planner -------------------------------------------------------

pub struct DiagnosisPlanner;

impl DiagnosisPlanner {
    /// Generate a structured fix plan for the given trigger.
    ///
    /// Covers the 5 required failure patterns from MIG-06 success criteria:
    /// 1. rc-agent crash (ProcessCrash with rc-agent)
    /// 2. game stuck (GameLaunchFail, GameLaunchTimeout)
    /// 3. MAINTENANCE_MODE (SentinelUnexpected with MAINTENANCE)
    /// 4. WS disconnect (WsDisconnect)
    /// 5. blanking failure (DisplayMismatch)
    pub fn plan_for_trigger(trigger: &DiagnosticTrigger, tier: u8) -> DiagnosisPlan {
        let (trigger_name, actions, confidence) = match trigger {

            // -- Pattern 1: rc-agent crash ------------------------------------
            DiagnosticTrigger::ProcessCrash { process_name }
                if process_name.to_lowercase().contains("rc-agent") =>
            {
                ("rc-agent-crash", vec![
                    PlannedAction {
                        step: 1,
                        command: "taskkill /F /IM WerFault.exe 2>nul & taskkill /F /IM WerReport.exe 2>nul".into(),
                        risk_level: "safe".into(),
                        rollback: "No rollback needed -- WerFault dialogs are orphaned".into(),
                        expected_outcome: "WerFault/WerReport dialogs closed; no more crash dialogs blocking process table".into(),
                    },
                    PlannedAction {
                        step: 2,
                        command: "del C:\\RacingPoint\\MAINTENANCE_MODE 2>nul & del C:\\RacingPoint\\rcagent-restart-sentinel.txt 2>nul & del C:\\RacingPoint\\GRACEFUL_RELAUNCH 2>nul".into(),
                        risk_level: "safe".into(),
                        rollback: "none -- sentinels re-created by rc-agent if still crashing".into(),
                        expected_outcome: "Restart-blocking sentinels cleared; RCWatchdog will restart rc-agent".into(),
                    },
                    PlannedAction {
                        step: 3,
                        command: "RCWatchdog auto-restarts via WTSQueryUserToken+CreateProcessAsUser (no exec needed)".into(),
                        risk_level: "safe".into(),
                        // v331: RCWatchdog is sole restart authority
                        rollback: "Kill rc-agent.exe -- RCWatchdog service will auto-restart in Session 1".into(),
                        expected_outcome: "rc-agent running in Session 1 (not Session 0) within 15 seconds".into(),
                    },
                    PlannedAction {
                        step: 4,
                        command: "curl -s http://127.0.0.1:8090/health".into(),
                        risk_level: "safe".into(),
                        rollback: "escalate to WhatsApp alert if still down after 60s".into(),
                        expected_outcome: "HTTP 200 with status=ok and non-empty build_id".into(),
                    },
                    PlannedAction {
                        step: 5,
                        command: "tasklist /V /FO CSV | findstr rc-agent".into(),
                        risk_level: "safe".into(),
                        rollback: "none -- read-only check".into(),
                        expected_outcome: "Session column shows Console (not Services) confirming Session 1".into(),
                    },
                ], 0.85)
            }

            // -- Pattern 2: game stuck ----------------------------------------
            DiagnosticTrigger::GameLaunchFail
            | DiagnosticTrigger::GameLaunchTimeout { .. } =>
            {
                ("game-stuck", vec![
                    PlannedAction {
                        step: 1,
                        command: "taskkill /F /IM acs.exe 2>nul & taskkill /F /IM acs_x86.exe 2>nul & taskkill /F /IM AssettoCorsa.exe 2>nul & taskkill /F /IM Content_Manager_x86.exe 2>nul".into(),
                        risk_level: "caution".into(),
                        rollback: "none -- force-kill of stuck game process; billing session may need manual reconciliation".into(),
                        expected_outcome: "No acs.exe or Content_Manager process remaining in tasklist".into(),
                    },
                    PlannedAction {
                        step: 2,
                        command: "del C:\\RacingPoint\\game.pid 2>nul & del C:\\RacingPoint\\FORCE_START 2>nul".into(),
                        risk_level: "safe".into(),
                        rollback: "none -- rc-agent re-creates game.pid on next launch".into(),
                        expected_outcome: "Stale game PID sentinel cleared; rc-agent can accept new launch command".into(),
                    },
                    PlannedAction {
                        step: 3,
                        command: "curl -s http://127.0.0.1:8090/api/v1/game/state".into(),
                        risk_level: "safe".into(),
                        rollback: "none -- read-only check".into(),
                        expected_outcome: "game_state=Idle (not Launching or Running)".into(),
                    },
                    PlannedAction {
                        step: 4,
                        command: "curl -s http://127.0.0.1:8090/health | grep edge_process_count".into(),
                        risk_level: "safe".into(),
                        rollback: "none -- read-only check".into(),
                        expected_outcome: "edge_process_count > 0 (blanking screen showing correctly)".into(),
                    },
                ], 0.80)
            }

            // -- Pattern 3: MAINTENANCE_MODE ----------------------------------
            DiagnosticTrigger::SentinelUnexpected { file_name }
                if file_name.to_uppercase().contains("MAINTENANCE") =>
            {
                ("maintenance-mode", vec![
                    PlannedAction {
                        step: 1,
                        command: "type C:\\RacingPoint\\MAINTENANCE_MODE".into(),
                        risk_level: "safe".into(),
                        rollback: "none -- read-only".into(),
                        expected_outcome: "Shows lock reason and timestamp (helps determine if intentional)".into(),
                    },
                    PlannedAction {
                        step: 2,
                        command: "del C:\\RacingPoint\\MAINTENANCE_MODE".into(),
                        risk_level: "caution".into(),
                        rollback: "Manually recreate: echo MANUAL_CLEAR > C:\\RacingPoint\\MAINTENANCE_MODE".into(),
                        expected_outcome: "MAINTENANCE_MODE sentinel gone; RCWatchdog can restart rc-agent".into(),
                    },
                    PlannedAction {
                        step: 3,
                        command: "del C:\\RacingPoint\\rcagent-restart-sentinel.txt 2>nul".into(),
                        risk_level: "safe".into(),
                        rollback: "none".into(),
                        expected_outcome: "Restart counter cleared; rc-sentry restart tracker resets".into(),
                    },
                    PlannedAction {
                        step: 4,
                        command: "curl -s http://127.0.0.1:8090/health".into(),
                        risk_level: "safe".into(),
                        rollback: "if still down after 30s, check Event Viewer for ntdll crash loop".into(),
                        expected_outcome: "HTTP 200 within 15s after MAINTENANCE_MODE cleared".into(),
                    },
                ], 0.90)
            }

            // -- Pattern 3 (alternate): HealthCheckFail after restart loop ----
            DiagnosticTrigger::HealthCheckFail => {
                ("maintenance-mode-check", vec![
                    PlannedAction {
                        step: 1,
                        command: "if exist C:\\RacingPoint\\MAINTENANCE_MODE (echo MAINTENANCE_MODE present) else (echo not in maintenance)".into(),
                        risk_level: "safe".into(),
                        rollback: "none -- read-only".into(),
                        expected_outcome: "Identifies if MAINTENANCE_MODE is the cause of HealthCheckFail".into(),
                    },
                    PlannedAction {
                        step: 2,
                        command: "del C:\\RacingPoint\\MAINTENANCE_MODE 2>nul & del C:\\RacingPoint\\rcagent-restart-sentinel.txt 2>nul".into(),
                        risk_level: "caution".into(),
                        rollback: "echo MANUAL_CLEAR > C:\\RacingPoint\\MAINTENANCE_MODE".into(),
                        expected_outcome: "Sentinels cleared; rc-agent can restart if RCWatchdog detects it".into(),
                    },
                    PlannedAction {
                        step: 3,
                        command: "curl -s http://127.0.0.1:8090/health".into(),
                        risk_level: "safe".into(),
                        rollback: "escalate to WhatsApp if still down".into(),
                        expected_outcome: "HTTP 200 within 30s".into(),
                    },
                ], 0.70)
            }

            // -- Pattern 4: WS disconnect -------------------------------------
            DiagnosticTrigger::WsDisconnect { .. }
            | DiagnosticTrigger::WsInstability { .. } =>
            {
                ("ws-disconnect", vec![
                    PlannedAction {
                        step: 1,
                        command: "curl -s http://192.168.31.23:8080/api/v1/health".into(),
                        risk_level: "safe".into(),
                        rollback: "none -- read-only probe".into(),
                        expected_outcome: "Server responds with status=ok (rules out server-down as cause)".into(),
                    },
                    PlannedAction {
                        step: 2,
                        command: "netstat -ano | findstr :8080 | findstr CLOSE_WAIT".into(),
                        risk_level: "safe".into(),
                        rollback: "none -- read-only".into(),
                        expected_outcome: "Low or no CLOSE_WAIT entries -- many CLOSE_WAIT = socket exhaustion".into(),
                    },
                    PlannedAction {
                        step: 3,
                        command: "curl -s http://127.0.0.1:8090/health | grep ws_connected".into(),
                        risk_level: "safe".into(),
                        rollback: "none -- read-only".into(),
                        expected_outcome: "ws_connected=true (reconnect may have self-healed)".into(),
                    },
                    PlannedAction {
                        step: 4,
                        command: "IF ws_connected=false THEN rc-agent reconnects automatically (backoff max 60s) -- no manual action needed unless >120s".into(),
                        risk_level: "safe".into(),
                        rollback: "none -- rc-agent has built-in WS reconnect with exponential backoff".into(),
                        expected_outcome: "WS reconnects within 60s from last disconnect".into(),
                    },
                ], 0.75)
            }

            // -- Pattern 5: blanking failure ----------------------------------
            DiagnosticTrigger::DisplayMismatch { expected_edge_count, actual_edge_count } => {
                ("blanking-failure", vec![
                    PlannedAction {
                        step: 1,
                        command: format!("curl -s http://127.0.0.1:8090/health | grep edge_process_count"),
                        risk_level: "safe".into(),
                        rollback: "none -- read-only".into(),
                        expected_outcome: format!("edge_process_count should be {} (actual: {})", expected_edge_count, actual_edge_count),
                    },
                    PlannedAction {
                        step: 2,
                        command: "taskkill /F /IM msedge.exe 2>nul".into(),
                        risk_level: "caution".into(),
                        rollback: "rc-agent will relaunch Edge automatically on next blanking tick".into(),
                        expected_outcome: "All msedge.exe processes terminated; clean slate for relaunch".into(),
                    },
                    PlannedAction {
                        step: 3,
                        command: "curl -s -X POST http://127.0.0.1:8090/api/v1/screen/blank".into(),
                        risk_level: "caution".into(),
                        rollback: "curl -s -X POST http://127.0.0.1:8090/api/v1/screen/unblank".into(),
                        expected_outcome: "RCAGENT_BLANK_SCREEN sentinel written; Edge relaunches with blank URL".into(),
                    },
                    PlannedAction {
                        step: 4,
                        command: "curl -s http://127.0.0.1:8090/health | grep edge_process_count".into(),
                        risk_level: "safe".into(),
                        rollback: "physical check at venue if edge_process_count still 0".into(),
                        expected_outcome: "edge_process_count > 0 within 12 seconds (standing rule: 12s window)".into(),
                    },
                    PlannedAction {
                        step: 5,
                        command: "tasklist /V /FO CSV | findstr msedge".into(),
                        risk_level: "safe".into(),
                        rollback: "none -- read-only".into(),
                        expected_outcome: "Session column for msedge shows Console (not Services)".into(),
                    },
                ], 0.80)
            }

            // -- Generic fallback ---------------------------------------------
            _ => ("generic-diagnosis", vec![
                PlannedAction {
                    step: 1,
                    command: "curl -s http://127.0.0.1:8090/health".into(),
                    risk_level: "safe".into(),
                    rollback: "none".into(),
                    expected_outcome: "Baseline health check to determine rc-agent status".into(),
                },
                PlannedAction {
                    step: 2,
                    command: "dir C:\\RacingPoint\\*.txt C:\\RacingPoint\\MAINTENANCE_MODE C:\\RacingPoint\\OTA_DEPLOYING 2>nul".into(),
                    risk_level: "safe".into(),
                    rollback: "none -- read-only".into(),
                    expected_outcome: "List of sentinel files to identify blocking conditions".into(),
                },
            ], 0.50),
        };

        DiagnosisPlan {
            trigger_name: trigger_name.to_string(),
            tier,
            actions,
            created_at: Utc::now().to_rfc3339(),
            confidence,
            gate_summary: vec![],  // filled in after run_phase_a/D
        }
    }
}

// --- Full CGP + Plan Evaluation ----------------------------------------------

/// Run the full cognitive gate + diagnosis planner for a TierDiagnosis.
/// Stores the resulting plan in LAST_PLAN for /gate/last-plan.
/// Called from the MMA engine after an MMA run, or from the tier engine for Tier 3.
pub fn evaluate_and_plan(diag: &TierDiagnosis) {
    tracing::info!(
        target: LOG_TARGET,
        trigger = ?diag.trigger,
        tier = diag.tier,
        "CGP evaluation starting"
    );

    // Phase A gates
    let phase_a_gates = match CgpEngine::run_phase_a(diag) {
        Ok(gates) => gates,
        Err(CgpError::CriticalGateFailed(gate)) => {
            tracing::error!(target: LOG_TARGET, gate = ?gate, "Critical gate failed -- no plan generated");
            return;
        }
    };

    // Generate fix plan
    let mut plan = DiagnosisPlanner::plan_for_trigger(&diag.trigger, diag.tier);

    // Phase D gates
    let phase_d_gates = CgpEngine::run_phase_d(diag, plan.actions.len());

    // Summarize gate results
    let gate_summary: Vec<String> = phase_a_gates.iter()
        .chain(phase_d_gates.iter())
        .map(|g| format!("{:?}:{:?}", g.gate, g.status))
        .collect();
    plan.gate_summary = gate_summary;

    tracing::info!(
        target: LOG_TARGET,
        trigger_name = %plan.trigger_name,
        action_count = plan.actions.len(),
        confidence = plan.confidence,
        "CGP plan generated"
    );

    // Store for /gate/last-plan
    store_last_plan(plan);
}

// --- Helper Functions (trigger analysis) -------------------------------------

fn trigger_to_problem(trigger: &DiagnosticTrigger) -> String {
    match trigger {
        DiagnosticTrigger::Periodic => "Periodic health scan detected anomaly".into(),
        DiagnosticTrigger::HealthCheckFail => "rc-agent health endpoint not responding".into(),
        DiagnosticTrigger::ProcessCrash { process_name } =>
            format!("Process crash detected: {}", process_name),
        DiagnosticTrigger::GameLaunchFail => "Game launch timed out (>90s, no game_pid)".into(),
        DiagnosticTrigger::GameLaunchTimeout { elapsed_secs } =>
            format!("Game launch timed out ({elapsed_secs}s -- no playable signal)"),
        DiagnosticTrigger::DisplayMismatch { expected_edge_count, actual_edge_count } =>
            format!("Display mismatch: expected {} Edge processes, found {}", expected_edge_count, actual_edge_count),
        DiagnosticTrigger::ErrorSpike { errors_per_min } =>
            format!("Error spike: {} errors/min", errors_per_min),
        DiagnosticTrigger::WsDisconnect { disconnected_secs } =>
            format!("WebSocket disconnected for {}s", disconnected_secs),
        DiagnosticTrigger::WsInstability { reconnects_5m, .. } =>
            format!("WebSocket instability: {} reconnects in 5 min", reconnects_5m),
        DiagnosticTrigger::SentinelUnexpected { file_name } =>
            format!("Unexpected sentinel: {}", file_name),
        DiagnosticTrigger::ViolationSpike { delta } =>
            format!("Process guard violation spike: {} in 5 min", delta),
        DiagnosticTrigger::PreFlightFailed { check_name, detail } =>
            format!("Pre-flight '{}' failed: {}", check_name, detail),
        DiagnosticTrigger::TaskbarVisible => "Taskbar visible when it should be hidden".into(),
        DiagnosticTrigger::GameMidSessionCrash { exit_code, session_duration_secs } =>
            format!("Game crashed mid-session (exit {:?}, {}s)", exit_code, session_duration_secs),
        _ => format!("{:?}", trigger),
    }
}

fn trigger_to_plan(trigger: &DiagnosticTrigger) -> String {
    match trigger {
        DiagnosticTrigger::ProcessCrash { .. } =>
            "1. Kill WerFault 2. Clear sentinels 3. Verify RCWatchdog restart 4. Confirm Session 1".into(),
        DiagnosticTrigger::GameLaunchFail | DiagnosticTrigger::GameLaunchTimeout { .. } =>
            "1. Kill acs.exe 2. Clear game.pid 3. Verify GameTracker=Idle 4. Confirm blanking".into(),
        DiagnosticTrigger::WsDisconnect { .. } =>
            "1. Probe server health 2. Check CLOSE_WAIT sockets 3. Wait for auto-reconnect".into(),
        DiagnosticTrigger::DisplayMismatch { .. } =>
            "1. Kill msedge 2. POST /screen/blank 3. Verify edge_process_count>0 in 12s".into(),
        DiagnosticTrigger::SentinelUnexpected { .. } =>
            "1. Read sentinel content 2. Del if safe 3. Restart rc-agent 4. Verify health".into(),
        _ => "1. Check health 2. Enumerate sentinels 3. Escalate if no obvious fix".into(),
    }
}

fn trigger_to_hypotheses(trigger: &DiagnosticTrigger) -> Vec<(String, String)> {
    match trigger {
        DiagnosticTrigger::HealthCheckFail => vec![
            ("rc-agent crashed and is not running".into(), "tasklist | findstr rc-agent".into()),
            ("MAINTENANCE_MODE sentinel blocking restart".into(), "dir C:\\RacingPoint\\MAINTENANCE_MODE".into()),
            ("Port 8090 bound by zombie process".into(), "netstat -ano | findstr :8090".into()),
        ],
        DiagnosticTrigger::ProcessCrash { .. } => vec![
            ("WerFault dialog blocking process table entry".into(), "tasklist | findstr WerFault".into()),
            ("Crash loop triggering MAINTENANCE_MODE".into(), "dir C:\\RacingPoint\\MAINTENANCE_MODE".into()),
            ("ntdll.dll access violation from corrupted OS state".into(), "wevtutil qe Application /c:5".into()),
        ],
        DiagnosticTrigger::DisplayMismatch { .. } => vec![
            ("Edge launched in Session 0 (schtask start) -- invisible to user".into(), "tasklist /V | findstr msedge".into()),
            ("Blanking screen URL incorrect or resource missing".into(), "curl http://127.0.0.1:8090/health | grep edge".into()),
            ("NVIDIA Surround configuration dropped to single monitor".into(), "physical check required".into()),
        ],
        DiagnosticTrigger::WsDisconnect { .. } => vec![
            ("Server racecontrol process dead".into(), "curl http://192.168.31.23:8080/health".into()),
            ("Network path between pod and server degraded".into(), "ping 192.168.31.23".into()),
            ("CLOSE_WAIT socket accumulation exhausted port pool".into(), "netstat -ano | findstr CLOSE_WAIT".into()),
        ],
        _ => vec![
            ("rc-agent is in a degraded state".into(), "curl http://127.0.0.1:8090/health".into()),
            ("A blocking sentinel file is present".into(), "dir C:\\RacingPoint\\*.txt".into()),
        ],
    }
}

// --- Tests -------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rc_common::diagnostic_types::DiagnosticTrigger;

    fn rc_agent_crash_trigger() -> DiagnosticTrigger {
        DiagnosticTrigger::ProcessCrash { process_name: "rc-agent.exe".into() }
    }

    #[test]
    fn test_plan_rc_agent_crash() {
        let trigger = rc_agent_crash_trigger();
        let plan = DiagnosisPlanner::plan_for_trigger(&trigger, 4);
        assert_eq!(plan.trigger_name, "rc-agent-crash");
        assert!(plan.actions.len() >= 4, "rc-agent-crash needs >=4 actions, got {}", plan.actions.len());
        assert!(plan.confidence > 0.7, "confidence should be >0.7");
    }

    #[test]
    fn test_plan_game_stuck() {
        let trigger = DiagnosticTrigger::GameLaunchFail;
        let plan = DiagnosisPlanner::plan_for_trigger(&trigger, 4);
        assert_eq!(plan.trigger_name, "game-stuck");
        assert!(plan.actions.len() >= 3, "game-stuck needs >=3 actions");
    }

    #[test]
    fn test_plan_maintenance_mode() {
        let trigger = DiagnosticTrigger::SentinelUnexpected {
            file_name: "MAINTENANCE_MODE".into()
        };
        let plan = DiagnosisPlanner::plan_for_trigger(&trigger, 4);
        assert_eq!(plan.trigger_name, "maintenance-mode");
        assert!(plan.actions.len() >= 3, "maintenance-mode needs >=3 actions");
    }

    #[test]
    fn test_plan_ws_disconnect() {
        let trigger = DiagnosticTrigger::WsDisconnect { disconnected_secs: 120 };
        let plan = DiagnosisPlanner::plan_for_trigger(&trigger, 4);
        assert_eq!(plan.trigger_name, "ws-disconnect");
        assert!(plan.actions.len() >= 3, "ws-disconnect needs >=3 actions");
    }

    #[test]
    fn test_plan_blanking_failure() {
        let trigger = DiagnosticTrigger::DisplayMismatch {
            expected_edge_count: 1,
            actual_edge_count: 0
        };
        let plan = DiagnosisPlanner::plan_for_trigger(&trigger, 4);
        assert_eq!(plan.trigger_name, "blanking-failure");
        assert!(plan.actions.len() >= 4, "blanking-failure needs >=4 actions");
    }

    #[test]
    fn test_plan_coverage_all_5_patterns() {
        let triggers = [
            DiagnosticTrigger::ProcessCrash { process_name: "rc-agent.exe".into() },
            DiagnosticTrigger::GameLaunchFail,
            DiagnosticTrigger::SentinelUnexpected { file_name: "MAINTENANCE_MODE".into() },
            DiagnosticTrigger::WsDisconnect { disconnected_secs: 60 },
            DiagnosticTrigger::DisplayMismatch { expected_edge_count: 1, actual_edge_count: 0 },
        ];
        let expected_names = [
            "rc-agent-crash", "game-stuck", "maintenance-mode", "ws-disconnect", "blanking-failure",
        ];
        for (trigger, expected_name) in triggers.iter().zip(expected_names.iter()) {
            let plan = DiagnosisPlanner::plan_for_trigger(trigger, 4);
            assert_eq!(&plan.trigger_name, expected_name,
                "Pattern {} got name {}", expected_name, plan.trigger_name);
            assert!(!plan.actions.is_empty(), "Pattern {} has no actions", expected_name);
        }
    }

    #[test]
    fn test_last_plan_storage() {
        let trigger = DiagnosticTrigger::GameLaunchFail;
        let plan = DiagnosisPlanner::plan_for_trigger(&trigger, 4);
        store_last_plan(plan.clone());
        let retrieved = get_last_plan().expect("should have a plan");
        assert_eq!(retrieved.trigger_name, "game-stuck");
    }

    #[test]
    fn test_cgp_phase_a_passes_for_valid_trigger() {
        let diag = TierDiagnosis {
            trigger: DiagnosticTrigger::HealthCheckFail,
            tier: 4,
            outcome: "pending".into(),
            action: "".into(),
            root_cause: "".into(),
            fix_type: "".into(),
            confidence: 0.0,
            fix_applied: false,
            problem_hash: "health_check_fail".into(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        };
        let result = CgpEngine::run_phase_a(&diag);
        assert!(result.is_ok(), "Phase A should pass for HealthCheckFail");
        let gates = result.expect("Phase A should return gates");
        // G0 must pass (HealthCheckFail has a non-empty problem)
        assert_eq!(gates[0].status, CgpGateStatus::Passed, "G0 should pass");
    }
}
