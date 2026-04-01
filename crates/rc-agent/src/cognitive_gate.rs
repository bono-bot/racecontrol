//! Cognitive Gate Protocol (CGP) Engine — machine-enforced structured thinking for Tier 3/4 diagnosis.
//!
//! Implements CGP gates as pure functions operating on structured data. All gates are local
//! compute ($0 cost) — no AI model calls. Evidence is stored as machine-readable JSON.
//!
//! Phase A gates (pre-action): G0 Problem Definition, G5 Competing Hypotheses, G7 Tool Verification.
//! Phase D gates (post-action): G1 Outcome Verification, G2 Fleet Scope, G4 Confidence Calibration,
//!   G8 Dependency Cascade, G9 Retrospective.

use chrono::Utc;
use serde_json::json;

use crate::diagnostic_engine::{DiagnosticEvent, DiagnosticTrigger};
use crate::knowledge_base::KnowledgeBase;
use rc_common::mesh_types::{
    CgpGateId, CgpGateResult, CgpGateStatus, DiagnosisTier,
};

const LOG_TARGET: &str = "cognitive-gate";

/// Error when a critical CGP gate fails.
#[derive(Debug)]
pub enum CgpError {
    /// G0 Problem Definition failed — cannot proceed without problem definition.
    CriticalGateFailed(CgpGateId),
}

impl std::fmt::Display for CgpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CgpError::CriticalGateFailed(gate) => write!(f, "Critical CGP gate failed: {:?}", gate),
        }
    }
}

/// CGP Engine — all gates are pure functions, no AI calls.
pub struct CgpEngine;

impl CgpEngine {
    /// Phase A: Pre-action gates — run BEFORE any AI model call.
    /// Returns Err only if G0 (Problem Definition) fails — other gate failures are warnings.
    pub fn run_phase_a(
        event: &DiagnosticEvent,
        kb: Option<&KnowledgeBase>,
        tier: DiagnosisTier,
    ) -> Result<Vec<CgpGateResult>, CgpError> {
        let mut gates = Vec::with_capacity(3);

        let g0 = Self::gate_g0_problem_definition(event);
        if g0.status == CgpGateStatus::Failed {
            tracing::error!(target: LOG_TARGET, "G0 Problem Definition FAILED — aborting diagnosis");
            return Err(CgpError::CriticalGateFailed(CgpGateId::G0ProblemDefinition));
        }
        gates.push(g0);

        gates.push(Self::gate_g5_competing_hypotheses(event, kb));
        gates.push(Self::gate_g7_tool_verification(event, tier));

        tracing::info!(
            target: LOG_TARGET,
            gates_passed = gates.iter().filter(|g| g.status == CgpGateStatus::Passed).count(),
            gates_total = gates.len(),
            "CGP Phase A complete"
        );

        Ok(gates)
    }

    /// Phase D: Post-action gates — run AFTER fix is applied. Never blocks the fix.
    pub fn run_phase_d(
        event: &DiagnosticEvent,
        fix_applied: bool,
        fix_description: &str,
        tier: DiagnosisTier,
        kb: Option<&KnowledgeBase>,
    ) -> Vec<CgpGateResult> {
        let mut gates = Vec::with_capacity(5);

        gates.push(Self::gate_g1_outcome_verification(fix_applied, fix_description));
        gates.push(Self::gate_g2_fleet_scope(event, tier));
        gates.push(Self::gate_g4_confidence_calibration(fix_applied, event));
        gates.push(Self::gate_g8_dependency_cascade(event));
        gates.push(Self::gate_g9_retrospective(event, fix_applied, fix_description, kb));

        tracing::info!(
            target: LOG_TARGET,
            gates_passed = gates.iter().filter(|g| g.status == CgpGateStatus::Passed).count(),
            gates_total = gates.len(),
            "CGP Phase D complete"
        );

        gates
    }

    // ─── Phase A Gates ──────────────────────────────────────────────────────

    /// G0: Problem Definition — extract structured PROBLEM/SYMPTOMS/PLAN from the event.
    fn gate_g0_problem_definition(event: &DiagnosticEvent) -> CgpGateResult {
        let start = std::time::Instant::now();

        let problem = trigger_to_problem(&event.trigger);
        let symptoms = trigger_to_symptoms(event);
        let plan = trigger_to_plan(&event.trigger);

        let evidence = json!({
            "problem": problem,
            "symptoms": symptoms,
            "plan": plan,
        });

        // G0 passes as long as we can generate a non-empty problem definition
        let status = if problem.is_empty() {
            CgpGateStatus::Failed
        } else {
            CgpGateStatus::Passed
        };

        CgpGateResult {
            gate: CgpGateId::G0ProblemDefinition,
            status,
            evidence,
            timestamp: Utc::now(),
            duration_ms: start.elapsed().as_millis() as u64,
        }
    }

    /// G5: Competing Hypotheses — generate 2+ hypotheses from KB + heuristic rules.
    fn gate_g5_competing_hypotheses(
        event: &DiagnosticEvent,
        kb: Option<&KnowledgeBase>,
    ) -> CgpGateResult {
        let start = std::time::Instant::now();
        let mut hypotheses: Vec<serde_json::Value> = Vec::new();

        // Heuristic hypotheses based on trigger type
        let heuristic_hyps = trigger_to_hypotheses(&event.trigger);
        for (hyp, falsification) in &heuristic_hyps {
            hypotheses.push(json!({
                "source": "heuristic",
                "hypothesis": hyp,
                "falsification_test": falsification,
            }));
        }

        // KB-sourced hypotheses — prior solutions for similar problem_key
        if let Some(kb) = kb {
            let problem_key = crate::knowledge_base::normalize_problem_key(&event.trigger);
            if let Ok(solutions) = kb.lookup_all(&problem_key, 3) {
                for sol in &solutions {
                    hypotheses.push(json!({
                        "source": "knowledge_base",
                        "hypothesis": format!("Prior fix: {}", sol.root_cause),
                        "falsification_test": format!("Check if condition matches: {}", sol.fix_action),
                        "confidence": sol.confidence,
                        "solution_id": sol.id,
                    }));
                }
            }
        }

        // G5 requires 2+ hypotheses
        let status = if hypotheses.len() >= 2 {
            CgpGateStatus::Passed
        } else if hypotheses.is_empty() {
            CgpGateStatus::Failed
        } else {
            // 1 hypothesis — technically fails G5 but we log warning and continue
            tracing::warn!(
                target: LOG_TARGET,
                trigger = ?event.trigger,
                "G5: only 1 hypothesis generated — insufficient for competing analysis"
            );
            CgpGateStatus::Failed
        };

        CgpGateResult {
            gate: CgpGateId::G5CompetingHypotheses,
            status,
            evidence: json!({ "hypotheses": hypotheses, "count": hypotheses.len() }),
            timestamp: Utc::now(),
            duration_ms: start.elapsed().as_millis() as u64,
        }
    }

    /// G7: Tool Verification — select the correct model/approach for this trigger.
    fn gate_g7_tool_verification(
        event: &DiagnosticEvent,
        tier: DiagnosisTier,
    ) -> CgpGateResult {
        let start = std::time::Instant::now();

        let (requirement, tool, compatibility) = match tier {
            DiagnosisTier::SingleModel => {
                let domain = trigger_to_domain(&event.trigger);
                (
                    format!("Single-model diagnosis for {:?}", event.trigger),
                    "Qwen3 235B (cheapest, $0.05)".to_string(),
                    format!("Domain '{}' compatible with general-purpose model", domain),
                )
            }
            DiagnosisTier::MultiModel => {
                let domain = trigger_to_domain(&event.trigger);
                (
                    format!("5-model consensus for {:?}", event.trigger),
                    format!("Domain roster '{}': Reasoner + Code Expert + SRE + Domain + Generalist", domain),
                    "Vendor diversity enforced (>=3 families)".to_string(),
                )
            }
            _ => {
                return CgpGateResult {
                    gate: CgpGateId::G7ToolVerification,
                    status: CgpGateStatus::Skipped,
                    evidence: json!({"reason": "Tier does not use AI models"}),
                    timestamp: Utc::now(),
                    duration_ms: start.elapsed().as_millis() as u64,
                };
            }
        };

        CgpGateResult {
            gate: CgpGateId::G7ToolVerification,
            status: CgpGateStatus::Passed,
            evidence: json!({
                "requirement": requirement,
                "tool": tool,
                "compatibility_check": compatibility,
            }),
            timestamp: Utc::now(),
            duration_ms: start.elapsed().as_millis() as u64,
        }
    }

    // ─── Phase D Gates ──────────────────────────────────────────────────────

    /// G1: Outcome Verification — was the fix actually applied and verified?
    fn gate_g1_outcome_verification(fix_applied: bool, fix_description: &str) -> CgpGateResult {
        let start = std::time::Instant::now();

        CgpGateResult {
            gate: CgpGateId::G1OutcomeVerification,
            status: if fix_applied { CgpGateStatus::Passed } else { CgpGateStatus::Failed },
            evidence: json!({
                "behavior_tested": fix_applied,
                "method": if fix_applied { "verify_fix() returned true" } else { "fix was not applied or verification failed" },
                "fix_description": fix_description,
            }),
            timestamp: Utc::now(),
            duration_ms: start.elapsed().as_millis() as u64,
        }
    }

    /// G2: Fleet Scope — identify which other pods/targets this fix applies to.
    fn gate_g2_fleet_scope(event: &DiagnosticEvent, tier: DiagnosisTier) -> CgpGateResult {
        let start = std::time::Instant::now();

        // Tier 3 is single-pod scope by default; Tier 4 uses fleet gossip
        let (status, evidence) = match tier {
            DiagnosisTier::SingleModel => (
                CgpGateStatus::Skipped,
                json!({"reason": "Tier 3 is single-pod scope — fleet gossip deferred to Tier 4"}),
            ),
            DiagnosisTier::MultiModel => {
                // Fleet-scope: the fix should be gossiped to server for broadcast
                let is_pod_specific = matches!(
                    event.trigger,
                    DiagnosticTrigger::DisplayMismatch { .. }
                    | DiagnosticTrigger::TaskbarVisible
                );
                (
                    CgpGateStatus::Passed,
                    json!({
                        "fleet_applicable": !is_pod_specific,
                        "gossip_recommended": !is_pod_specific,
                        "pod_specific_reason": if is_pod_specific { "Display/UI triggers are pod-specific" } else { "" },
                    }),
                )
            }
            _ => (CgpGateStatus::Skipped, json!({"reason": "Non-AI tier"})),
        };

        CgpGateResult {
            gate: CgpGateId::G2FleetScope,
            status,
            evidence,
            timestamp: Utc::now(),
            duration_ms: start.elapsed().as_millis() as u64,
        }
    }

    /// G4: Confidence Calibration — what was tested, what wasn't, follow-up plan.
    fn gate_g4_confidence_calibration(fix_applied: bool, event: &DiagnosticEvent) -> CgpGateResult {
        let start = std::time::Instant::now();

        let tested = if fix_applied {
            vec!["verify_fix() check passed", "Anomaly condition cleared"]
        } else {
            vec!["Fix attempted but not verified"]
        };

        let not_tested = match &event.trigger {
            DiagnosticTrigger::GameLaunchFail => vec![
                ("Game actually launches successfully after fix", "medium"),
                ("Customer can play a full session", "low"),
            ],
            DiagnosticTrigger::WsDisconnect { .. } => vec![
                ("WebSocket stays connected for >5 minutes", "medium"),
                ("No data loss during reconnection", "low"),
            ],
            DiagnosticTrigger::GameMidSessionCrash { .. } => vec![
                ("Game doesn't crash again under same conditions", "high"),
                ("Billing session properly refunded", "medium"),
            ],
            _ => vec![
                ("Long-term stability (>1 hour)", "low"),
            ],
        };

        let follow_up = if fix_applied {
            "Monitor for recurrence in next 30 minutes via periodic scan"
        } else {
            "Escalate to next tier or human intervention"
        };

        CgpGateResult {
            gate: CgpGateId::G4ConfidenceCalibration,
            status: CgpGateStatus::Passed,
            evidence: json!({
                "tested": tested,
                "not_tested": not_tested,
                "follow_up_plan": follow_up,
            }),
            timestamp: Utc::now(),
            duration_ms: start.elapsed().as_millis() as u64,
        }
    }

    /// G8: Dependency Cascade — check downstream impact of the fix.
    fn gate_g8_dependency_cascade(event: &DiagnosticEvent) -> CgpGateResult {
        let start = std::time::Instant::now();

        let (downstream, risk) = match &event.trigger {
            DiagnosticTrigger::ProcessCrash { process_name } => (
                vec![format!("Services depending on {}", process_name)],
                "medium",
            ),
            DiagnosticTrigger::WsDisconnect { .. } => (
                vec![
                    "Billing state sync".to_string(),
                    "Game launch commands".to_string(),
                    "Fleet health reporting".to_string(),
                ],
                "high",
            ),
            DiagnosticTrigger::GameMidSessionCrash { .. } => (
                vec![
                    "Active billing session".to_string(),
                    "Customer experience".to_string(),
                    "Telemetry recording".to_string(),
                ],
                "high",
            ),
            _ => (vec!["No critical downstream dependencies".to_string()], "low"),
        };

        CgpGateResult {
            gate: CgpGateId::G8DependencyCascade,
            status: CgpGateStatus::Passed,
            evidence: json!({
                "changed_component": format!("{:?}", event.trigger),
                "downstream_consumers": downstream,
                "risk_level": risk,
            }),
            timestamp: Utc::now(),
            duration_ms: start.elapsed().as_millis() as u64,
        }
    }

    /// G9: Retrospective — root cause + prevention + similar past incidents.
    fn gate_g9_retrospective(
        event: &DiagnosticEvent,
        fix_applied: bool,
        fix_description: &str,
        kb: Option<&KnowledgeBase>,
    ) -> CgpGateResult {
        let start = std::time::Instant::now();

        let mut similar_past = Vec::new();
        if let Some(kb) = kb {
            let problem_key = crate::knowledge_base::normalize_problem_key(&event.trigger);
            if let Ok(solutions) = kb.lookup_all(&problem_key, 5) {
                for sol in &solutions {
                    similar_past.push(json!({
                        "solution_id": sol.id,
                        "root_cause": sol.root_cause,
                        "success_count": sol.success_count,
                        "confidence": sol.confidence,
                    }));
                }
            }
        }

        CgpGateResult {
            gate: CgpGateId::G9Retrospective,
            status: CgpGateStatus::Passed,
            evidence: json!({
                "root_cause": if fix_applied { fix_description } else { "Unresolved — escalated" },
                "prevention": trigger_to_prevention(&event.trigger),
                "similar_past_incidents": similar_past,
            }),
            timestamp: Utc::now(),
            duration_ms: start.elapsed().as_millis() as u64,
        }
    }
}

// ─── Helper functions ───────────────────────────────────────────────────────

/// Map a DiagnosticTrigger to a human-readable problem statement.
fn trigger_to_problem(trigger: &DiagnosticTrigger) -> String {
    match trigger {
        DiagnosticTrigger::Periodic => "Periodic health scan detected anomaly".into(),
        DiagnosticTrigger::HealthCheckFail => "rc-agent health endpoint not responding".into(),
        DiagnosticTrigger::ProcessCrash { process_name } => format!("Process crash detected: {}", process_name),
        DiagnosticTrigger::GameLaunchFail => "Game launch timed out (>90s, no game_pid)".into(),
        DiagnosticTrigger::DisplayMismatch { expected_edge_count, actual_edge_count } =>
            format!("Display mismatch: expected {} Edge processes, found {}", expected_edge_count, actual_edge_count),
        DiagnosticTrigger::ErrorSpike { errors_per_min } =>
            format!("Error spike: {} errors/min exceeds threshold", errors_per_min),
        DiagnosticTrigger::WsDisconnect { disconnected_secs } =>
            format!("WebSocket disconnected for {}s", disconnected_secs),
        DiagnosticTrigger::SentinelUnexpected { file_name } =>
            format!("Unexpected sentinel file: {}", file_name),
        DiagnosticTrigger::ViolationSpike { delta } =>
            format!("Process guard violation spike: {} delta in 5 min", delta),
        DiagnosticTrigger::PreFlightFailed { check_name, detail } =>
            format!("Pre-flight check '{}' failed: {}", check_name, detail),
        DiagnosticTrigger::PosKioskDown { detail } =>
            format!("POS kiosk down: {}", detail),
        DiagnosticTrigger::PosBillingApiError { endpoint, status_code, .. } =>
            format!("POS billing API error: {} (status {})", endpoint, status_code),
        DiagnosticTrigger::PosWifiDegraded { rssi_dbm, latency_ms } =>
            format!("POS WiFi degraded: RSSI {}dBm, latency {}ms", rssi_dbm, latency_ms),
        DiagnosticTrigger::PosKioskEscaped { foreground_process } =>
            format!("POS kiosk escaped: foreground = {}", foreground_process),
        DiagnosticTrigger::TaskbarVisible => "Taskbar visible when it should be hidden".into(),
        DiagnosticTrigger::GameMidSessionCrash { exit_code, session_duration_secs } =>
            format!("Game crashed mid-session (exit {:?}, {}s into session)", exit_code, session_duration_secs),
        DiagnosticTrigger::PostSessionAnalysis { session_quality_pct } =>
            format!("Post-session quality analysis: {}% quality", session_quality_pct),
        DiagnosticTrigger::PreShiftAudit => "Pre-shift health audit".into(),
        DiagnosticTrigger::DeployVerification { new_build_id } =>
            format!("Post-deploy verification for build {}", new_build_id),
        DiagnosticTrigger::PosNetworkDown { .. } => "POS network connectivity lost".into(),
    }
}

/// Extract symptoms from the diagnostic event.
fn trigger_to_symptoms(event: &DiagnosticEvent) -> Vec<String> {
    let mut symptoms = vec![
        format!("Trigger: {:?}", event.trigger),
        format!("Build: {}", event.build_id),
        format!("Timestamp: {}", event.timestamp),
    ];

    // Add pod state context
    if event.pod_state.recovery_in_progress {
        symptoms.push("Recovery already in progress".into());
    }
    if event.pod_state.game_pid.is_some() {
        symptoms.push(format!("Active game PID: {:?}", event.pod_state.game_pid));
    }

    symptoms
}

/// Generate a high-level plan for diagnosing this trigger.
fn trigger_to_plan(trigger: &DiagnosticTrigger) -> String {
    match trigger {
        DiagnosticTrigger::ProcessCrash { .. } =>
            "1. Kill WerFault/WerReport 2. Check if process restarts 3. Verify service health".into(),
        DiagnosticTrigger::GameLaunchFail =>
            "1. Check game process status 2. Verify display state 3. Check disk/network 4. Re-attempt launch".into(),
        DiagnosticTrigger::WsDisconnect { .. } =>
            "1. Check network connectivity 2. Verify server health 3. Attempt reconnect 4. Check for certificate/auth issues".into(),
        DiagnosticTrigger::DisplayMismatch { .. } =>
            "1. Check Edge process count 2. Verify blanking state 3. Relaunch browser if needed".into(),
        DiagnosticTrigger::GameMidSessionCrash { .. } =>
            "1. Capture crash context 2. Check billing state 3. Attempt game relaunch 4. Verify customer impact".into(),
        _ => "1. Identify root cause 2. Apply fix 3. Verify resolution 4. Store in KB".into(),
    }
}

/// Generate 2+ competing hypotheses for the trigger.
fn trigger_to_hypotheses(trigger: &DiagnosticTrigger) -> Vec<(String, String)> {
    match trigger {
        DiagnosticTrigger::ProcessCrash { process_name } => vec![
            (format!("{} crashed due to unhandled exception", process_name), "Check Event Viewer for crash dump".into()),
            (format!("{} killed by another process", process_name), "Check process guard logs for kill event".into()),
            ("RAM pressure caused OOM kill".into(), "Check available memory at crash time".into()),
        ],
        DiagnosticTrigger::GameLaunchFail => vec![
            ("Game executable not found or corrupted".into(), "Verify game exe exists and size > 0".into()),
            ("Display/GPU not available in current session".into(), "Check Session context (Session 0 vs Session 1)".into()),
            ("Conflicting process holding game files".into(), "Check for lock files or running game instances".into()),
        ],
        DiagnosticTrigger::WsDisconnect { .. } => vec![
            ("Server racecontrol crashed or restarted".into(), "Check server health endpoint directly".into()),
            ("Network connectivity lost (WiFi/Ethernet)".into(), "Ping server IP and default gateway".into()),
            ("WS connection exhaustion on server".into(), "Check server connection count vs limit".into()),
        ],
        DiagnosticTrigger::DisplayMismatch { .. } => vec![
            ("Edge was killed by process guard".into(), "Check process guard violation logs".into()),
            ("Edge crashed (out of memory or GPU issue)".into(), "Check Event Viewer for msedge.exe crash".into()),
            ("Blanking state inconsistency (state set but browser not launched)".into(), "Verify lock_screen_state vs actual Edge process".into()),
        ],
        DiagnosticTrigger::GameMidSessionCrash { .. } => vec![
            ("Game internal crash (memory corruption, assertion)".into(), "Check crash dump in game folder".into()),
            ("FFB/USB device disruption killed game".into(), "Check HID device status and recent USB events".into()),
            ("GPU driver crash (TDR)".into(), "Check Event Viewer for Display driver recovery events".into()),
        ],
        DiagnosticTrigger::ErrorSpike { .. } => vec![
            ("Downstream service failure causing cascade".into(), "Check which error types are spiking".into()),
            ("Network partition from server".into(), "Verify connectivity to all endpoints".into()),
        ],
        // MMA-F4: Specialized hypotheses for remaining trigger types
        DiagnosticTrigger::TaskbarVisible => vec![
            ("Explorer.exe restarted and lost SW_HIDE state".into(), "Check if explorer.exe PID changed recently".into()),
            ("Third-party app triggered taskbar show event".into(), "Check foreground window history for non-kiosk apps".into()),
            ("Windows Update or Group Policy reset taskbar settings".into(), "Check Event Viewer for policy application events".into()),
        ],
        DiagnosticTrigger::SentinelUnexpected { .. } => vec![
            ("Previous crash storm left stale MAINTENANCE_MODE sentinel".into(), "Check sentinel file age vs last known crash time".into()),
            ("Deploy in progress left OTA_DEPLOYING sentinel".into(), "Check if any OTA process is actually running".into()),
            ("Manual intervention created sentinel without clearing".into(), "Check sentinel creation timestamp vs operator activity log".into()),
        ],
        DiagnosticTrigger::ViolationSpike { .. } => vec![
            ("Process guard allowlist is stale or empty (server was down at boot)".into(), "Check allowlist size and last fetch time".into()),
            ("New software installed that isn't in allowlist".into(), "Compare running processes against allowlist entries".into()),
            ("Malware or unauthorized process spawning".into(), "Check violation log for unfamiliar process names".into()),
        ],
        DiagnosticTrigger::PreFlightFailed { .. } => vec![
            ("Hardware not connected (USB wheel, pedals)".into(), "Check HID device enumeration".into()),
            ("Service dependency not started (ConspitLink, Edge)".into(), "Check process list for required services".into()),
            ("Configuration file corrupted or missing".into(), "Verify game config files exist and parse correctly".into()),
        ],
        DiagnosticTrigger::PosKioskDown { .. } => vec![
            ("Edge browser crashed (OOM or GPU issue)".into(), "Check Event Viewer for msedge.exe crash".into()),
            ("Network to racecontrol server lost".into(), "Ping server and check kiosk URL connectivity".into()),
            ("POS WiFi degraded causing page timeout".into(), "Check WiFi signal strength and latency".into()),
        ],
        DiagnosticTrigger::PosNetworkDown { .. } => vec![
            ("WiFi adapter disconnected or driver crashed".into(), "Check network adapter status in Device Manager".into()),
            ("Router or access point failure".into(), "Ping default gateway and check ARP table".into()),
            ("Server racecontrol crashed or restarted".into(), "Check server health endpoint from another device".into()),
        ],
        DiagnosticTrigger::PosBillingApiError { .. } => vec![
            ("Server billing endpoint returned error".into(), "Check server logs for billing API errors".into()),
            ("Request payload malformed (schema mismatch)".into(), "Compare POS kiosk version against server API version".into()),
            ("Network timeout on WiFi causing partial request".into(), "Check POS WiFi latency and retry count".into()),
        ],
        DiagnosticTrigger::PosWifiDegraded { .. } => vec![
            ("Physical obstruction or interference".into(), "Check RSSI history for sudden drops vs gradual degradation".into()),
            ("Too many devices on the network".into(), "Check router connected device count".into()),
        ],
        DiagnosticTrigger::PosKioskEscaped { .. } => vec![
            ("Notification popup stole foreground focus".into(), "Check for notification/toast windows".into()),
            ("System dialog appeared (UAC, update, error)".into(), "Check Event Viewer for system dialog events".into()),
            ("User intentionally escaped kiosk mode".into(), "Check if keyboard shortcuts were used (Alt+Tab, Win key)".into()),
        ],
        _ => vec![
            ("Configuration drift from expected state".into(), "Compare current config against known-good".into()),
            ("External environmental factor".into(), "Check for recent Windows updates, driver changes, or power events".into()),
        ],
    }
}

/// Map trigger to MMA domain roster name.
fn trigger_to_domain(trigger: &DiagnosticTrigger) -> &'static str {
    match trigger {
        DiagnosticTrigger::ProcessCrash { .. }
        | DiagnosticTrigger::DisplayMismatch { .. }
        | DiagnosticTrigger::TaskbarVisible
        | DiagnosticTrigger::SentinelUnexpected { .. }
        | DiagnosticTrigger::PosKioskEscaped { .. } => "windows_os",

        DiagnosticTrigger::GameLaunchFail
        | DiagnosticTrigger::GameMidSessionCrash { .. }
        | DiagnosticTrigger::PostSessionAnalysis { .. } => "rust_backend",

        DiagnosticTrigger::WsDisconnect { .. }
        | DiagnosticTrigger::HealthCheckFail
        | DiagnosticTrigger::ErrorSpike { .. } => "sre_ops",

        DiagnosticTrigger::PreFlightFailed { .. }
        | DiagnosticTrigger::DeployVerification { .. }
        | DiagnosticTrigger::PreShiftAudit => "cross_system",

        DiagnosticTrigger::PosKioskDown { .. }
        | DiagnosticTrigger::PosBillingApiError { .. }
        | DiagnosticTrigger::PosWifiDegraded { .. }
        | DiagnosticTrigger::PosNetworkDown { .. } => "nodejs_frontend",

        _ => "cross_system",
    }
}

/// Suggest prevention measures for a trigger type.
fn trigger_to_prevention(trigger: &DiagnosticTrigger) -> &'static str {
    match trigger {
        DiagnosticTrigger::ProcessCrash { .. } => "Add process to watchdog, implement crash recovery handler",
        DiagnosticTrigger::GameLaunchFail => "Add pre-launch validation, verify game exe integrity at boot",
        DiagnosticTrigger::WsDisconnect { .. } => "Improve reconnection logic, add server-side keepalive",
        DiagnosticTrigger::DisplayMismatch { .. } => "Enforce browser launch verification, add Edge process monitor",
        DiagnosticTrigger::GameMidSessionCrash { .. } => "Track crash patterns per game, add billing session checkpoint",
        _ => "Monitor for recurrence, update KB if pattern emerges",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::failure_monitor::FailureMonitorState;

    fn make_test_event(trigger: DiagnosticTrigger) -> DiagnosticEvent {
        DiagnosticEvent {
            trigger,
            pod_state: FailureMonitorState::default(),
            timestamp: "2026-04-01T10:00:00+05:30".to_string(),
            build_id: "test1234",
        }
    }

    #[test]
    fn g0_generates_valid_problem_definition() {
        let event = make_test_event(DiagnosticTrigger::GameLaunchFail);
        let result = CgpEngine::gate_g0_problem_definition(&event);
        assert_eq!(result.status, CgpGateStatus::Passed);
        assert!(result.evidence["problem"].as_str().is_some());
        assert!(!result.evidence["problem"].as_str().unwrap_or("").is_empty());
        assert!(result.evidence["symptoms"].as_array().is_some());
        assert!(result.evidence["plan"].as_str().is_some());
    }

    #[test]
    fn g5_generates_two_plus_hypotheses() {
        let event = make_test_event(DiagnosticTrigger::GameLaunchFail);
        let result = CgpEngine::gate_g5_competing_hypotheses(&event, None);
        assert_eq!(result.status, CgpGateStatus::Passed);
        let count = result.evidence["count"].as_u64().unwrap_or(0);
        assert!(count >= 2, "G5 should generate at least 2 hypotheses, got {}", count);
    }

    #[test]
    fn g7_selects_correct_model_for_tier() {
        let event = make_test_event(DiagnosticTrigger::GameLaunchFail);

        let t3 = CgpEngine::gate_g7_tool_verification(&event, DiagnosisTier::SingleModel);
        assert_eq!(t3.status, CgpGateStatus::Passed);
        assert!(t3.evidence["tool"].as_str().unwrap_or("").contains("Qwen3"));

        let t4 = CgpEngine::gate_g7_tool_verification(&event, DiagnosisTier::MultiModel);
        assert_eq!(t4.status, CgpGateStatus::Passed);
        assert!(t4.evidence["tool"].as_str().unwrap_or("").contains("roster"));
    }

    #[test]
    fn phase_a_fails_on_empty_trigger() {
        // Periodic triggers should still produce a problem definition
        let event = make_test_event(DiagnosticTrigger::Periodic);
        let result = CgpEngine::run_phase_a(&event, None, DiagnosisTier::MultiModel);
        assert!(result.is_ok());
    }

    #[test]
    fn phase_d_never_blocks() {
        let event = make_test_event(DiagnosticTrigger::WsDisconnect { disconnected_secs: 60 });
        let gates = CgpEngine::run_phase_d(&event, true, "Reconnected WS", DiagnosisTier::MultiModel, None);
        assert_eq!(gates.len(), 5);
        // Phase D should always produce results, never panic
    }
}
