//! MMA Triage Router — Step 2 of the Mesh Audit Pipeline (MAP).
//!
//! Routes diagnostic queries through the Tier-0 oracle (`audit_known_issues`)
//! BEFORE falling through to MMA. Addresses FM-4 from the MMA comprehension
//! post-mortem: prior to this, every "why does X fail?" query went straight
//! to `multi-model-audit.js` with a full-directory bundle, which produced
//! hallucinations because models retrieved by keyword similarity instead of
//! structural proximity.
//!
//! Contract: see `.planning/specs/MMA-TRIAGE-ROUTER.md`.
//!
//! Endpoint: `POST /api/v1/diagnose/triage` (staff-JWT gated).
//!
//! Returns one of four verdicts:
//! - `TRIAGE_FAST` — Tier-0 oracle hit. Router recommends NOT running MMA.
//! - `AUDIT_DEEP` — No oracle hit but a symptom class matched. Fall through to MMA.
//! - `RESEARCH_NEW` — No symptom class recognized. Escalate (human or targeted research).
//! - `REJECT_AMBIGUOUS` — Query unparseable. Ask clarifying questions.

use axum::{extract::State, response::Json};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::sync::Arc;

use crate::state::AppState;
use crate::fleet_kb::{match_audit_known_issue_structured, AuditKnownIssueMatch};

// ─── Request / Response ────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct TriageRequest {
    pub query: String,
    #[serde(default)]
    pub caller_context: Option<String>, // "ops-triage" | "gsd-debug" | "mma-audit" | "unknown"
    #[serde(default)]
    pub targets_hint: Option<Vec<String>>,
    #[serde(default = "default_max_category_hits")]
    #[allow(dead_code)] // reserved for future multi-hit return; single-hit impl ignores it
    pub max_category_hits: usize,
}

fn default_max_category_hits() -> usize {
    5
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "verdict")]
pub enum TriageVerdict {
    #[serde(rename = "TRIAGE_FAST")]
    TriageFast {
        bug_id: String,
        fix_status: String,
        fixed_in_build: Option<String>,
        affects_targets: Vec<String>,
        escalation_message: String,
        confidence: f32,
        hit_type: String, // "EXACT" | "CATEGORY"
        matched_pattern: String,
        should_run_mma: bool,
    },
    #[serde(rename = "AUDIT_DEEP")]
    AuditDeep {
        relevant_symptom_classes: Vec<String>,
        rationale: String,
        should_run_mma: bool,
    },
    #[serde(rename = "RESEARCH_NEW")]
    ResearchNew {
        rationale: String,
        should_run_mma: bool,
    },
    #[serde(rename = "REJECT_AMBIGUOUS")]
    RejectAmbiguous {
        rationale: String,
        clarifying_questions: Vec<String>,
        should_run_mma: bool,
    },
}

// ─── Query parse + symptom-class expansion ─────────────────────────────────

/// Extracted features used for oracle lookup + classification.
#[derive(Debug, PartialEq)]
pub struct ParsedQuery {
    pub pod_ids: Vec<String>,
    pub symptom_classes: Vec<String>,
    pub expanded_terms: Vec<String>,
}

/// Static synonym map. Keyed by symptom class, values are the phrases whose
/// presence in a query indicate that class. Kept deterministic — LLM expansion
/// can be added later if recall is insufficient.
fn symptom_class_map() -> &'static [(&'static str, &'static [&'static str])] {
    &[
        ("exit-code-1", &["exit 1", "exit code 1", "exited unexpectedly", "crash on start"]),
        (
            "orphan-process",
            &["orphan", "f1_25.exe running", "won't kill", "resists taskkill", "ea anti-cheat"],
        ),
        (
            "steam-dialog",
            &["steam dialog", "vguipopupwindow", "dialog visible", "60s timeout"],
        ),
        ("zero-laps", &["no laps", "laps not recorded", "0 laps", "empty lap table"]),
        (
            "rally-3min",
            &["ac rally", "assetto corsa rally", "3 minute", "3-min", "180s"],
        ),
        (
            "launch-timeout",
            &["launch timed out", "stuck launching", "stopping state", "30s timeout"],
        ),
        (
            "python-ini",
            &["python.ini", "racecontrol section", "plugin not loaded", "python plugin"],
        ),
    ]
}

pub fn parse_query(query: &str, targets_hint: Option<&[String]>) -> ParsedQuery {
    let lower = query.to_lowercase();
    let mut pod_ids = BTreeSet::new();

    // pod_N or "pod N" or "Pod N" extraction
    for n in 1..=8 {
        let variants = [format!("pod_{}", n), format!("pod {}", n), format!("pod{}", n)];
        if variants.iter().any(|v| lower.contains(v)) {
            pod_ids.insert(format!("pod_{}", n));
        }
    }

    if let Some(hint) = targets_hint {
        for t in hint {
            pod_ids.insert(t.clone());
        }
    }

    let mut symptom_classes = Vec::new();
    let mut expanded_terms = Vec::new();

    for (class, phrases) in symptom_class_map() {
        for phrase in *phrases {
            if lower.contains(phrase) {
                if !symptom_classes.iter().any(|c: &String| c == *class) {
                    symptom_classes.push((*class).to_string());
                }
                expanded_terms.push((*phrase).to_string());
            }
        }
    }

    ParsedQuery {
        pod_ids: pod_ids.into_iter().collect(),
        symptom_classes,
        expanded_terms,
    }
}

// ─── Hit classification ────────────────────────────────────────────────────

pub fn classify_hit(match_: &AuditKnownIssueMatch, requested_pods: &[String]) -> (String, f32) {
    if requested_pods.is_empty() {
        // Target-agnostic query — treat any hit as EXACT with moderate confidence.
        return ("EXACT".into(), 0.85);
    }
    let affected: BTreeSet<&String> = match_.affects_targets.iter().collect();
    let overlap = requested_pods.iter().any(|p| affected.contains(p));
    if overlap {
        ("EXACT".into(), 0.95)
    } else {
        // Mechanism may apply, targets differ — PARTIAL_MATCH at Stage 3.
        ("CATEGORY".into(), 0.55)
    }
}

pub fn is_ambiguous(parsed: &ParsedQuery, raw_query: &str) -> bool {
    // Ambiguous = no pod AND no symptom class AND query is short/generic.
    // Long queries with detail but no keyword matches are RESEARCH_NEW, not ambiguous.
    parsed.pod_ids.is_empty()
        && parsed.symptom_classes.is_empty()
        && raw_query.trim().split_whitespace().count() < 5
}

// ─── Verdict builder ───────────────────────────────────────────────────────

pub fn build_verdict(
    parsed: &ParsedQuery,
    raw_query: &str,
    caller_context: Option<&str>,
    oracle_hit: Option<AuditKnownIssueMatch>,
) -> TriageVerdict {
    // Ambiguous check first — short query with no extractable content.
    if is_ambiguous(parsed, raw_query) {
        return TriageVerdict::RejectAmbiguous {
            rationale: "Query has no pod, sim, or recognized symptom class — too vague to triage".into(),
            clarifying_questions: vec![
                "Which pod is affected?".into(),
                "What symptom? (crash, hang, wrong value, etc.)".into(),
                "Which game or subsystem?".into(),
            ],
            should_run_mma: false,
        };
    }

    if let Some(hit) = oracle_hit {
        let (hit_type, base_confidence) = classify_hit(&hit, &parsed.pod_ids);
        // Caller-intent gate: gsd-debug always wants MMA diversity regardless of hit.
        let should_run_mma = caller_context == Some("gsd-debug");
        return TriageVerdict::TriageFast {
            bug_id: hit.problem_key,
            fix_status: hit.fix_status,
            fixed_in_build: hit.fixed_in_build,
            affects_targets: hit.affects_targets,
            escalation_message: hit.escalation_message,
            confidence: base_confidence,
            hit_type,
            matched_pattern: hit.matched_pattern,
            should_run_mma,
        };
    }

    // Oracle miss. If at least one symptom class matched → AUDIT_DEEP (MMA with
    // focused bundle). If no class matched → RESEARCH_NEW.
    if !parsed.symptom_classes.is_empty() {
        TriageVerdict::AuditDeep {
            relevant_symptom_classes: parsed.symptom_classes.clone(),
            rationale: "Symptom class recognized but no known audit issue matched — \
                route to MMA with query-relevant bundle (Step 3 helper)"
                .into(),
            should_run_mma: true,
        }
    } else {
        TriageVerdict::ResearchNew {
            rationale: "No symptom class recognized and no audit hit — likely novel. \
                Escalate to human or targeted research before burning MMA budget."
                .into(),
            should_run_mma: true,
        }
    }
}

// ─── HTTP handler ──────────────────────────────────────────────────────────

pub async fn diagnose_triage(
    State(state): State<Arc<AppState>>,
    Json(req): Json<TriageRequest>,
) -> Json<TriageVerdict> {
    let parsed = parse_query(&req.query, req.targets_hint.as_deref());

    // Build symptom phrase for oracle lookup. Prefer extracted synonyms (which
    // are stored as patterns in the KB), else the raw query.
    let oracle_input = if parsed.expanded_terms.is_empty() {
        req.query.clone()
    } else {
        parsed.expanded_terms.join(" ")
    };

    let oracle_hit = match_audit_known_issue_structured(&state.db, &oracle_input).await;
    let verdict = build_verdict(&parsed, &req.query, req.caller_context.as_deref(), oracle_hit);
    Json(verdict)
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_pod_id_variants() {
        let p = parse_query("why does pod_3 iRacing fail?", None);
        assert_eq!(p.pod_ids, vec!["pod_3".to_string()]);
        let p = parse_query("Pod 4 F1 25 crashing", None);
        assert_eq!(p.pod_ids, vec!["pod_4".to_string()]);
    }

    #[test]
    fn extracts_multiple_pods() {
        let p = parse_query("pod_1 and pod_6 both crashed", None);
        assert_eq!(p.pod_ids, vec!["pod_1".to_string(), "pod_6".to_string()]);
    }

    #[test]
    fn targets_hint_augments_pods() {
        let hint = vec!["pod_8".to_string()];
        let p = parse_query("random query", Some(&hint));
        assert_eq!(p.pod_ids, vec!["pod_8".to_string()]);
    }

    #[test]
    fn recognizes_exit_code_1_class() {
        let p = parse_query("game exited unexpectedly on pod_4", None);
        assert!(p.symptom_classes.contains(&"exit-code-1".to_string()));
    }

    #[test]
    fn recognizes_steam_dialog_class() {
        let p = parse_query("steam dialog visible after 60s timeout", None);
        assert!(p.symptom_classes.contains(&"steam-dialog".to_string()));
    }

    #[test]
    fn multi_class_detection() {
        let p = parse_query("launch timed out and no laps recorded", None);
        assert!(p.symptom_classes.contains(&"launch-timeout".to_string()));
        assert!(p.symptom_classes.contains(&"zero-laps".to_string()));
    }

    fn stub_match(affects: Vec<&str>) -> AuditKnownIssueMatch {
        AuditKnownIssueMatch {
            problem_key: "INV-9".into(),
            severity: "P2".into(),
            fix_status: "code_fixed".into(),
            fixed_in_build: Some("68f4d61e".into()),
            affects_targets: affects.into_iter().map(String::from).collect(),
            escalation_message: "Steam dialog retry fix shipped in 68f4d61e".into(),
            matched_pattern: "steam dialog".into(),
        }
    }

    #[test]
    fn exact_match_when_target_overlaps() {
        let m = stub_match(vec!["pod_1", "pod_3", "pod_7"]);
        let (hit_type, conf) = classify_hit(&m, &["pod_3".to_string()]);
        assert_eq!(hit_type, "EXACT");
        assert!(conf >= 0.9);
    }

    #[test]
    fn category_match_when_target_differs() {
        let m = stub_match(vec!["pod_3", "pod_4"]);
        let (hit_type, conf) = classify_hit(&m, &["pod_8".to_string()]);
        assert_eq!(hit_type, "CATEGORY");
        assert!(conf < 0.8);
    }

    #[test]
    fn exact_when_no_target_requested() {
        let m = stub_match(vec!["pod_3"]);
        let (hit_type, _) = classify_hit(&m, &[]);
        assert_eq!(hit_type, "EXACT");
    }

    #[test]
    fn ambiguous_when_empty() {
        let p = parse_query("help", None);
        assert!(is_ambiguous(&p, "help"));
    }

    #[test]
    fn not_ambiguous_when_pod_known() {
        let p = parse_query("pod_3 problem", None);
        assert!(!is_ambiguous(&p, "pod_3 problem"));
    }

    #[test]
    fn verdict_triage_fast_on_exact_hit() {
        let p = parse_query("pod_3 steam dialog", None);
        let hit = Some(stub_match(vec!["pod_3"]));
        let v = build_verdict(&p, "pod_3 steam dialog", Some("ops-triage"), hit);
        match v {
            TriageVerdict::TriageFast {
                bug_id,
                hit_type,
                should_run_mma,
                ..
            } => {
                assert_eq!(bug_id, "INV-9");
                assert_eq!(hit_type, "EXACT");
                assert!(!should_run_mma);
            }
            _ => panic!("expected TRIAGE_FAST"),
        }
    }

    #[test]
    fn gsd_debug_forces_mma_even_on_hit() {
        let p = parse_query("pod_3 steam dialog", None);
        let hit = Some(stub_match(vec!["pod_3"]));
        let v = build_verdict(&p, "pod_3 steam dialog", Some("gsd-debug"), hit);
        match v {
            TriageVerdict::TriageFast { should_run_mma, .. } => assert!(should_run_mma),
            _ => panic!("expected TRIAGE_FAST"),
        }
    }

    #[test]
    fn verdict_audit_deep_when_class_matches_but_no_oracle_hit() {
        let p = parse_query("pod_2 game exited unexpectedly", None);
        let v = build_verdict(&p, "pod_2 game exited unexpectedly", None, None);
        match v {
            TriageVerdict::AuditDeep {
                relevant_symptom_classes,
                should_run_mma,
                ..
            } => {
                assert!(relevant_symptom_classes.contains(&"exit-code-1".to_string()));
                assert!(should_run_mma);
            }
            _ => panic!("expected AUDIT_DEEP"),
        }
    }

    #[test]
    fn verdict_research_new_when_nothing_matches() {
        let p = parse_query("pod_3 has weird behavior nobody understands yet", None);
        let v = build_verdict(&p, "pod_3 has weird behavior nobody understands yet", None, None);
        match v {
            TriageVerdict::ResearchNew { should_run_mma, .. } => assert!(should_run_mma),
            _ => panic!("expected RESEARCH_NEW"),
        }
    }

    #[test]
    fn verdict_reject_ambiguous_when_nothing_extractable() {
        let p = parse_query("broken", None);
        let v = build_verdict(&p, "broken", None, None);
        match v {
            TriageVerdict::RejectAmbiguous {
                clarifying_questions,
                should_run_mma,
                ..
            } => {
                assert!(!clarifying_questions.is_empty());
                assert!(!should_run_mma);
            }
            _ => panic!("expected REJECT_AMBIGUOUS"),
        }
    }
}
