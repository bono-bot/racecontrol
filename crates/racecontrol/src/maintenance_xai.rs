//! v29.0 Phase 8+20: AI diagnosis integration and XAI explainability.
//!
//! Prepares structured prompts for Ollama AI on James's machine.
//! Guard function ensures AI is only invoked when rule-based diagnosis
//! is ambiguous (Claude Sonnet insight: save AI budget for complex cases).

use serde::Serialize;

/// Structured diagnosis request — collects all context for AI analysis.
#[derive(Debug, Clone, Serialize)]
pub struct DiagnosisRequest {
    pub pod_id: String,
    pub anomalies: Vec<String>,
    pub recent_events: Vec<String>,
    pub component_rul: Vec<String>,
    pub telemetry_summary: String,
}

/// AI diagnosis result — structured response from Ollama.
#[derive(Debug, Clone, Serialize)]
pub struct DiagnosisResult {
    pub root_cause: String,
    pub recommended_action: String,
    pub urgency: String,
    pub confidence: f32,
    pub explanation: String,
}

/// Sanitize user-controlled strings before embedding in AI prompts.
/// P2-3: Strip control characters, limit length to prevent prompt injection.
fn sanitize_for_prompt(input: &str, max_len: usize) -> String {
    input
        .chars()
        .filter(|c| !c.is_control() || *c == '\n')
        .take(max_len)
        .collect()
}

/// Build a structured prompt for Ollama diagnosis.
///
/// The prompt is formatted to elicit a JSON response with specific fields
/// for downstream parsing by the maintenance engine.
/// P2-3: All user-controlled strings are sanitized (control chars stripped, length capped).
pub fn build_diagnosis_prompt(req: &DiagnosisRequest) -> String {
    let pod_id = sanitize_for_prompt(&req.pod_id, 32);
    let anomalies = req.anomalies.iter()
        .map(|a| sanitize_for_prompt(a, 200))
        .collect::<Vec<_>>()
        .join(", ");
    let events = req.recent_events.iter()
        .map(|e| sanitize_for_prompt(e, 200))
        .collect::<Vec<_>>()
        .join(", ");
    let rul = req.component_rul.iter()
        .map(|r| sanitize_for_prompt(r, 200))
        .collect::<Vec<_>>()
        .join(", ");
    let telemetry = sanitize_for_prompt(&req.telemetry_summary, 1000);

    format!(
        "You are an AI maintenance technician for a racing simulator venue.\n\
         Pod: {}\n\
         Active anomalies: {}\n\
         Recent events: {}\n\
         Component health: {}\n\
         Telemetry: {}\n\n\
         Diagnose the root cause, recommend an action, and rate urgency (Critical/High/Medium/Low).\n\
         Respond in JSON: {{\"root_cause\": \"...\", \"recommended_action\": \"...\", \
         \"urgency\": \"...\", \"confidence\": 0.0-1.0, \"explanation\": \"...\"}}",
        pod_id,
        anomalies,
        events,
        rul,
        telemetry,
    )
}

// ─── Phase 20: XAI Explainability Layer ─────────────────────────────────────

/// Decision explanation record — stored for every AI/automated decision
#[derive(Debug, Clone, Serialize)]
pub struct DecisionExplanation {
    pub id: String,
    pub decision_type: String,    // "anomaly_alert", "escalation", "pricing", "rul_estimate"
    pub timestamp: String,
    pub pod_id: Option<u8>,
    pub input_summary: String,    // what data was used
    pub decision: String,         // what was decided
    pub reasoning: String,        // why (human-readable)
    pub confidence: f32,
    pub alternative_considered: Option<String>,
}

/// Build XAI explanation for an anomaly alert
pub fn explain_anomaly(
    rule_name: &str,
    pod_id: &str,
    metric_value: f64,
    threshold: f64,
    sustained_minutes: u32,
) -> DecisionExplanation {
    DecisionExplanation {
        id: uuid::Uuid::new_v4().to_string(),
        decision_type: "anomaly_alert".into(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        pod_id: pod_id.strip_prefix("pod_").and_then(|s| s.parse().ok()),
        input_summary: format!(
            "{} = {:.1} (threshold: {:.1}, sustained: {}min)",
            rule_name, metric_value, threshold, sustained_minutes
        ),
        decision: format!("Alert triggered: {}", rule_name),
        reasoning: format!(
            "The metric has been {} the threshold of {:.1} for {} minutes continuously. \
             This exceeds the minimum sustained period required to trigger this alert, \
             reducing the chance of a false positive from transient spikes.",
            if metric_value > threshold {
                "above"
            } else {
                "below"
            },
            threshold,
            sustained_minutes
        ),
        confidence: 0.85,
        alternative_considered: Some(
            "Transient spike (dismissed: sustained period exceeded)".into(),
        ),
    }
}

/// Build XAI explanation for an escalation decision
pub fn explain_escalation(
    severity: &str,
    attempts: u32,
    tier: &str,
) -> DecisionExplanation {
    DecisionExplanation {
        id: uuid::Uuid::new_v4().to_string(),
        decision_type: "escalation".into(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        pod_id: None,
        input_summary: format!("severity={}, auto_fix_attempts={}", severity, attempts),
        decision: format!("Escalated to {}", tier),
        reasoning: format!(
            "After {} auto-fix attempt(s) with {} severity, the issue requires {} intervention. \
             Auto-fix is only used for first-time Medium/Low issues that aren't recurring.",
            attempts, severity, tier
        ),
        confidence: 0.95,
        alternative_considered: None,
    }
}

// ─── Original Phase 8 content ───────────────────────────────────────────────

/// Guard function: only invoke AI when rules are ambiguous.
///
/// AI diagnosis is expensive (Ollama inference). Only trigger when:
/// - More than 2 anomalies are active (complex multi-fault scenario)
/// - Max severity is Critical or High (warrants deeper analysis)
///
/// Simple cases (single anomaly, low severity) are handled by deterministic rules.
pub fn should_use_ai(anomaly_count: usize, max_severity: &str) -> bool {
    anomaly_count > 2 || max_severity == "Critical" || max_severity == "High"
}
