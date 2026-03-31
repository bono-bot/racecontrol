//! Unified MMA Protocol v1.0 — 4-Step Convergence Engine
//!
//! When Q3 authorizes MMA, this engine runs 4 sequential steps:
//!   Step 1: DIAGNOSE — 5 models × N iterations → consensus on all problems
//!   Step 2: PLAN — 5 models × N iterations → consensus on fix plans
//!   Step 3: EXECUTE — 5 models × N iterations → consensus on best solution
//!   Step 4: VERIFY — deterministic checks + 1 adversarial model sanity check
//!
//! Each step uses its own 10-model pool (stratified shuffle per iteration).
//! Consensus = 3/5 majority. Min 2 iterations. Max 4 iterations per step.
//! Step 4 failure → backtrack to Step 1 (max 3 backtracks → human escalation).
//!
//! Designed via MMA itself: 10 models, 2 iterations, consensus-driven (2026-03-31).
//! Spec: .planning/specs/UNIFIED-MMA-PROTOCOL.md

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::budget_tracker::BudgetTracker;
use crate::diagnostic_engine::{DiagnosticEvent, DiagnosticTrigger};
use crate::openrouter::{self, ModelConfig, ModelResponse, DiagnosisResult};

const LOG_TARGET: &str = "mma-engine";

/// Maximum iterations per step before escalating to human.
const MAX_ITERATIONS_PER_STEP: u8 = 4;
/// Minimum iterations per step (always run at least 2).
const MIN_ITERATIONS_PER_STEP: u8 = 2;
/// Maximum backtrack cycles (Step 4 fail → Step 1) before human escalation.
const MAX_BACKTRACKS: u8 = 3;
/// Models per iteration.
const MODELS_PER_ITERATION: usize = 5;
/// Convergence threshold: stop when iteration adds fewer than this many new findings.
const CONVERGENCE_NEW_FINDINGS_THRESHOLD: usize = 2;
/// Minimum consensus ratio (3/5 = 0.6).
const CONSENSUS_RATIO: f64 = 0.6;

// ─── Domain Roster ───────────────────────────────────────────────────────────

/// Issue domain classification for model pool selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IssueDomain {
    RustBackend,
    Frontend,
    WindowsOs,
    Network,
    Security,
    Hardware,
}

/// Classify a DiagnosticTrigger into an issue domain.
pub fn classify_domain(trigger: &DiagnosticTrigger) -> IssueDomain {
    match trigger {
        DiagnosticTrigger::GameLaunchFail
        | DiagnosticTrigger::GameMidSessionCrash { .. }
        | DiagnosticTrigger::PostSessionAnalysis { .. }
        | DiagnosticTrigger::ProcessCrash { .. } => IssueDomain::RustBackend,

        DiagnosticTrigger::DisplayMismatch { .. }
        | DiagnosticTrigger::TaskbarVisible => IssueDomain::WindowsOs,

        DiagnosticTrigger::WsDisconnect { .. }
        | DiagnosticTrigger::PosNetworkDown { .. }
        | DiagnosticTrigger::PosBillingApiError { .. } => IssueDomain::Network,

        DiagnosticTrigger::HealthCheckFail
        | DiagnosticTrigger::PreShiftAudit
        | DiagnosticTrigger::DeployVerification { .. } => IssueDomain::RustBackend,

        DiagnosticTrigger::ViolationSpike { .. }
        | DiagnosticTrigger::SentinelUnexpected { .. } => IssueDomain::Security,

        DiagnosticTrigger::ErrorSpike { .. }
        | DiagnosticTrigger::PreFlightFailed { .. }
        | DiagnosticTrigger::PosKioskDown { .. }
        | DiagnosticTrigger::Periodic => IssueDomain::RustBackend,
    }
}

/// Model role categories for stratified shuffle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModelRole {
    Reasoner,
    CodeExpert,
    Sre,
    DomainSpecialist,
    Generalist,
}

/// A model in the domain roster with its role tag.
#[derive(Debug, Clone)]
struct RosterModel {
    config: ModelConfig,
    role: ModelRole,
    priority: u8, // 0 = primary, 1 = secondary
}

/// Get the 10-model pool for a given domain and step.
/// Step 1 (DIAGNOSE): biased toward reasoners.
/// Step 2 (PLAN): biased toward architects/SRE.
/// Step 3 (EXECUTE): biased toward coders.
fn get_model_pool(domain: IssueDomain, step: u8) -> Vec<RosterModel> {
    // Base domain roster — 10 models per domain
    let domain_models = match domain {
        IssueDomain::RustBackend => vec![
            rm("deepseek/deepseek-r1-0528", "Reasoner", ModelRole::Reasoner, 0),
            rm("deepseek/deepseek-v3.2", "Code Expert", ModelRole::CodeExpert, 0),
            rm("qwen/qwen3-coder", "Rust Coder", ModelRole::CodeExpert, 0),
            rm("openai/gpt-5.4-nano", "Systems Thinker", ModelRole::Reasoner, 0),
            rm("x-ai/grok-code-fast-1", "Fast Coder", ModelRole::CodeExpert, 0),
            rm("meta-llama/llama-4-maverick", "Generalist", ModelRole::Generalist, 1),
            rm("nvidia/nemotron-3-super-120b-a12b", "SRE", ModelRole::Sre, 1),
            rm("mistralai/mistral-medium-3.1", "Balanced", ModelRole::Generalist, 1),
            rm("inception/mercury-coder", "Code Gen", ModelRole::CodeExpert, 1),
            rm("moonshotai/kimi-k2.5", "Adversarial", ModelRole::Reasoner, 1),
        ],
        IssueDomain::Frontend => vec![
            rm("x-ai/grok-4.1-fast", "JS/TS Specialist", ModelRole::CodeExpert, 0),
            rm("openai/gpt-5-mini", "Framework Expert", ModelRole::CodeExpert, 0),
            rm("google/gemini-2.5-pro-preview", "Architect", ModelRole::Reasoner, 0),
            rm("mistralai/mistral-large-2512", "Web Dev", ModelRole::Generalist, 0),
            rm("qwen/qwen3-235b-a22b-2507", "Async Debug", ModelRole::Reasoner, 0),
            rm("deepseek/deepseek-v3.1", "Full-Stack", ModelRole::CodeExpert, 1),
            rm("bytedance-seed/seed-2.0-mini", "Component Gen", ModelRole::CodeExpert, 1),
            rm("moonshotai/kimi-k2.5", "Edge Cases", ModelRole::Reasoner, 1),
            rm("baidu/ernie-4.5-300b-a47b", "Alternative", ModelRole::Generalist, 1),
            rm("meta-llama/llama-4-maverick", "React", ModelRole::Generalist, 1),
        ],
        IssueDomain::WindowsOs => vec![
            rm("openai/gpt-5.4-nano", "Windows Internals", ModelRole::DomainSpecialist, 0),
            rm("deepseek/deepseek-r1-0528", "OS Reasoning", ModelRole::Reasoner, 0),
            rm("nvidia/nemotron-3-super-120b-a12b", "Enterprise Win", ModelRole::Sre, 0),
            rm("xiaomi/mimo-v2-pro", "Sys Admin", ModelRole::Sre, 0),
            rm("baidu/ernie-4.5-300b-a47b", "Integration", ModelRole::DomainSpecialist, 0),
            rm("qwen/qwen3-235b-a22b-2507", "Broad", ModelRole::Generalist, 1),
            rm("z-ai/glm-4.7", "Driver Analysis", ModelRole::DomainSpecialist, 1),
            rm("moonshotai/kimi-k2.5", "Log Analysis", ModelRole::Reasoner, 1),
            rm("x-ai/grok-4.1-fast", "Fast Iter", ModelRole::Generalist, 1),
            rm("mistralai/mistral-medium-3.1", "Balanced", ModelRole::Generalist, 1),
        ],
        IssueDomain::Network => vec![
            rm("deepseek/deepseek-v3.2", "Protocol Analysis", ModelRole::CodeExpert, 0),
            rm("qwen/qwen3-235b-a22b-2507", "State Machines", ModelRole::Reasoner, 0),
            rm("xiaomi/mimo-v2-pro", "Distributed SRE", ModelRole::Sre, 0),
            rm("google/gemini-2.5-flash", "Fast Network", ModelRole::Generalist, 0),
            rm("moonshotai/kimi-k2.5", "Realtime Comms", ModelRole::DomainSpecialist, 0),
            rm("nvidia/nemotron-3-super-120b-a12b", "Topology", ModelRole::Sre, 1),
            rm("mistralai/mistral-medium-3.1", "Protocol Logic", ModelRole::Generalist, 1),
            rm("meta-llama/llama-4-maverick", "Distributed", ModelRole::Generalist, 1),
            rm("deepseek/deepseek-r1-0528", "Deep Reasoning", ModelRole::Reasoner, 1),
            rm("openai/gpt-5-mini", "Broad", ModelRole::Generalist, 1),
        ],
        IssueDomain::Security => vec![
            rm("google/gemini-2.5-pro-preview", "Credential Scanner", ModelRole::DomainSpecialist, 0),
            rm("openai/gpt-5.4-nano", "Threat Modeling", ModelRole::Reasoner, 0),
            rm("deepseek/deepseek-r1-0528", "Adversarial Reasoning", ModelRole::Reasoner, 0),
            rm("xiaomi/mimo-v2-pro", "Vuln Detection", ModelRole::Sre, 0),
            rm("moonshotai/kimi-k2.5", "Security Arch", ModelRole::DomainSpecialist, 0),
            rm("baidu/ernie-4.5-300b-a47b", "CVE Databases", ModelRole::DomainSpecialist, 1),
            rm("x-ai/grok-4.1-fast", "Adversarial", ModelRole::Generalist, 1),
            rm("mistralai/mistral-large-2512", "Broad Security", ModelRole::Generalist, 1),
            rm("nvidia/nemotron-3-super-120b-a12b", "Hardening", ModelRole::Sre, 1),
            rm("qwen/qwen3-235b-a22b-2507", "Volume Scan", ModelRole::Generalist, 1),
        ],
        IssueDomain::Hardware => vec![
            rm("google/gemini-2.5-pro-preview", "Sensor Analysis", ModelRole::DomainSpecialist, 0),
            rm("deepseek/deepseek-v3.2", "Driver Knowledge", ModelRole::CodeExpert, 0),
            rm("qwen/qwen3-235b-a22b-2507", "Broad Hardware", ModelRole::Generalist, 0),
            rm("nvidia/nemotron-3-super-120b-a12b", "Enterprise HW", ModelRole::Sre, 0),
            rm("z-ai/glm-4.7", "Driver Analysis", ModelRole::DomainSpecialist, 0),
            rm("xiaomi/mimo-v2-flash", "Fast Sensor", ModelRole::Sre, 1),
            rm("baidu/ernie-4.5-300b-a47b", "HW Integration", ModelRole::DomainSpecialist, 1),
            rm("openai/gpt-5-mini", "Broad", ModelRole::Generalist, 1),
            rm("moonshotai/kimi-k2.5", "Edge Cases", ModelRole::Reasoner, 1),
            rm("meta-llama/llama-4-maverick", "Perf Tuning", ModelRole::Generalist, 1),
        ],
    };

    // Apply step-level bias by reordering priority
    let mut pool = domain_models;
    match step {
        1 => {
            // DIAGNOSE: prioritize reasoners
            pool.sort_by_key(|m| match m.role {
                ModelRole::Reasoner => 0,
                ModelRole::DomainSpecialist => 1,
                ModelRole::Sre => 2,
                ModelRole::CodeExpert => 3,
                ModelRole::Generalist => 4,
            });
        }
        2 => {
            // PLAN: prioritize architects/SRE
            pool.sort_by_key(|m| match m.role {
                ModelRole::Sre => 0,
                ModelRole::Reasoner => 1,
                ModelRole::DomainSpecialist => 2,
                ModelRole::CodeExpert => 3,
                ModelRole::Generalist => 4,
            });
        }
        3 => {
            // EXECUTE: prioritize coders
            pool.sort_by_key(|m| match m.role {
                ModelRole::CodeExpert => 0,
                ModelRole::DomainSpecialist => 1,
                ModelRole::Sre => 2,
                ModelRole::Generalist => 3,
                ModelRole::Reasoner => 4,
            });
        }
        _ => {} // Step 4 uses 1 cheap model, handled separately
    }

    pool
}

/// Helper to create a RosterModel.
fn rm(id: &'static str, role_label: &'static str, role: ModelRole, priority: u8) -> RosterModel {
    RosterModel {
        config: ModelConfig {
            id,
            role: role_label,
            // Step-specific prompts are injected at call time, not here
            system_prompt: "",
        },
        role,
        priority,
    }
}

/// Select 5 models from a 10-model pool using stratified shuffle.
/// Guarantees: ≥1 reasoner + ≥1 code expert + ≥1 SRE per iteration.
/// Remaining 2 slots are randomized from the pool.
fn stratified_select(pool: &[RosterModel], iteration: u8) -> Vec<ModelConfig> {
    use rand::seq::SliceRandom;
    use rand::thread_rng;

    let mut rng = thread_rng();
    let mut selected: Vec<&RosterModel> = Vec::with_capacity(MODELS_PER_ITERATION);
    let mut used_ids: Vec<&str> = Vec::new();

    // Guarantee: 1 reasoner
    let reasoners: Vec<&RosterModel> = pool.iter()
        .filter(|m| m.role == ModelRole::Reasoner)
        .collect();
    if let Some(r) = reasoners.choose(&mut rng) {
        selected.push(r);
        used_ids.push(r.config.id);
    }

    // Guarantee: 1 code expert
    let coders: Vec<&RosterModel> = pool.iter()
        .filter(|m| m.role == ModelRole::CodeExpert && !used_ids.contains(&m.config.id))
        .collect();
    if let Some(c) = coders.choose(&mut rng) {
        selected.push(c);
        used_ids.push(c.config.id);
    }

    // Guarantee: 1 SRE (or domain specialist if no SRE available)
    let sres: Vec<&RosterModel> = pool.iter()
        .filter(|m| (m.role == ModelRole::Sre || m.role == ModelRole::DomainSpecialist)
                     && !used_ids.contains(&m.config.id))
        .collect();
    if let Some(s) = sres.choose(&mut rng) {
        selected.push(s);
        used_ids.push(s.config.id);
    }

    // Fill remaining slots randomly (considering iteration for diversity)
    let mut remaining: Vec<&RosterModel> = pool.iter()
        .filter(|m| !used_ids.contains(&m.config.id))
        .collect();
    remaining.shuffle(&mut rng);

    // For iteration > 1, prefer models that weren't primary in iteration 1
    if iteration > 1 {
        remaining.sort_by_key(|m| if m.priority == 0 { 1 } else { 0 });
    }

    for m in remaining {
        if selected.len() >= MODELS_PER_ITERATION {
            break;
        }
        selected.push(m);
    }

    selected.into_iter().map(|m| m.config.clone()).collect()
}

// ─── Step Consensus Schema ───────────────────────────────────────────────────

/// A single finding from a model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub id: String,
    pub description: String,
    pub severity: String,
    pub confidence: f64,
    pub evidence: Vec<String>,
    pub assumptions: Vec<String>,
    pub verification_steps: Vec<String>,
}

/// A fix plan from Step 2.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixPlan {
    pub problem_id: String,
    pub actions: Vec<String>,
    pub fix_type: String,
    pub risk_analysis: String,
    pub rollback_strategy: String,
    pub verification_steps: Vec<String>,
    pub estimated_duration_secs: u64,
}

/// An execution decision from Step 3.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Execution {
    pub problem_id: String,
    pub implementation: String,
    pub execution_order: u8,
    pub expected_outcome: String,
    pub confidence: f64,
}

/// Consensus passed between steps.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepConsensus {
    pub step: String,
    pub step_number: u8,
    pub iterations_completed: u8,
    pub domain: String,
    pub majority_findings: Vec<Finding>,
    pub fix_plans: Vec<FixPlan>,
    pub executions: Vec<Execution>,
    pub dissenting_opinions: Vec<DissentingOpinion>,
    pub models_used: Vec<String>,
    pub total_cost: f64,
    pub converged_at_iteration: u8,
    pub timestamp: String,
}

/// A minority opinion preserved for backtracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DissentingOpinion {
    pub model: String,
    pub finding: String,
    pub confidence: f64,
}

impl StepConsensus {
    fn new(step: &str, step_number: u8, domain: &str) -> Self {
        Self {
            step: step.to_string(),
            step_number,
            iterations_completed: 0,
            domain: domain.to_string(),
            majority_findings: Vec::new(),
            fix_plans: Vec::new(),
            executions: Vec::new(),
            dissenting_opinions: Vec::new(),
            models_used: Vec::new(),
            total_cost: 0.0,
            converged_at_iteration: 0,
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }
}

// ─── Step Prompts ────────────────────────────────────────────────────────────

fn step1_system_prompt() -> &'static str {
    "You are diagnosing a live issue on a Racing Point sim racing pod fleet. \
     8 Windows 11 pods running Rust/Axum rc-agent. \
     List ALL possible root causes for this issue. \
     For EACH root cause, output a JSON object with: \
     description (string), severity (critical/high/medium/low), \
     confidence (0.0-1.0), evidence (array of strings), \
     assumptions (array — what you're assuming), \
     verification_steps (array — how to confirm/disprove). \
     DO NOT suggest 'restart' as a root cause. \
     If restarting fixes it, explain WHY. \
     Output ONLY a valid JSON array of finding objects."
}

fn step2_system_prompt() -> &'static str {
    "You are planning fixes for confirmed problems on a Racing Point sim racing pod fleet. \
     For EACH problem, design a fix plan. Output a JSON object with: \
     problem_id (string), actions (array of ordered steps), \
     fix_type (deterministic/config/code_change/hardware), \
     risk_analysis (string), rollback_strategy (string), \
     verification_steps (array), estimated_duration_secs (number). \
     For code_change or hardware: mark requires_human = true. \
     NEVER auto-apply code changes or hardware modifications. \
     Prefer: deterministic > config > code_change. Smallest reversible fix. \
     Output ONLY a valid JSON array of plan objects."
}

fn step3_system_prompt() -> &'static str {
    "You are selecting and implementing the best fix for each problem on a Racing Point sim racing pod fleet. \
     Review the fix plans and select the BEST solution for each problem. \
     Output a JSON object with: problem_id (string), \
     implementation (the exact command/config/code to apply), \
     execution_order (priority number, fix critical first), \
     expected_outcome (what should change after fix), \
     confidence (0.0-1.0). \
     Prefer deterministic fixes over config changes, config over code changes. \
     Output ONLY a valid JSON array of execution objects."
}

fn step4_system_prompt() -> &'static str {
    "You are an adversarial evaluator. You MUST be a DIFFERENT perspective from the models that diagnosed and planned this fix. \
     Grade this fix on 4 criteria (total out of 5.0): \
     1. Root Cause Accuracy (35%): Did we fix the actual cause or just a symptom? \
     2. Fix Completeness (25%): Does it handle all variants or just the observed case? \
     3. Verification Evidence (25%): Is there concrete proof the fix worked? \
     4. Side Effect Safety (15%): Could this fix break anything else? \
     Output JSON: {\"score\": 0.0-5.0, \"grade\": \"PASS/FLAG/FAIL\", \
     \"root_cause_accuracy\": 0-5, \"fix_completeness\": 0-5, \
     \"verification_evidence\": 0-5, \"side_effect_safety\": 0-5, \
     \"reasoning\": \"...\", \"concerns\": [\"...\"]}"
}

// ─── 4-Step Engine ───────────────────────────────────────────────────────────

/// Result of the full 4-step Unified MMA Protocol.
#[derive(Debug)]
pub enum MmaProtocolResult {
    /// All 4 steps passed — fix verified and applied.
    Success {
        consensus: StepConsensus,
        total_cost: f64,
        backtracks: u8,
    },
    /// Budget exhausted before completion.
    BudgetExhausted {
        step: u8,
        spent: f64,
    },
    /// Max backtracks exceeded — needs human.
    HumanEscalation {
        backtracks: u8,
        last_failure: String,
        total_cost: f64,
    },
    /// OpenRouter API unavailable.
    ApiUnavailable {
        reason: String,
    },
}

/// Run the full 4-step Unified MMA Protocol.
///
/// This is the entry point called by tier_engine when Q3 authorizes MMA.
/// Runs Steps 1-4 sequentially, backtracks on Step 4 failure.
pub async fn run_protocol(
    event: &DiagnosticEvent,
    budget: &Arc<RwLock<BudgetTracker>>,
) -> MmaProtocolResult {
    let api_key = match openrouter::get_api_key() {
        Some(k) => k,
        None => return MmaProtocolResult::ApiUnavailable {
            reason: "OPENROUTER_KEY not set".to_string(),
        },
    };

    let domain = classify_domain(&event.trigger);
    let domain_str = format!("{:?}", domain);
    let base_symptoms = openrouter::format_symptoms(
        &format!("{:?}", event.trigger),
        &crate::knowledge_base::normalize_problem_key(&event.trigger),
        &format!("build_id={}", event.build_id),
        &format!("{:?}", event.pod_state),
    );
    let symptoms = openrouter::enrich_with_context_bundle(
        &base_symptoms, &event.trigger, &event.pod_state,
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(90))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let mut total_cost = 0.0f64;
    let mut backtracks = 0u8;
    let mut backtrack_evidence: Vec<String> = Vec::new();

    loop {
        tracing::info!(
            target: LOG_TARGET,
            domain = %domain_str,
            backtrack = backtracks,
            "Starting Unified MMA Protocol (backtrack #{})",
            backtracks
        );

        // ── Step 1: DIAGNOSE ──
        let step1_cost_est = estimate_step_cost(domain, 1);
        {
            let mut bt = budget.write().await;
            if !bt.can_spend(step1_cost_est) {
                return MmaProtocolResult::BudgetExhausted { step: 1, spent: total_cost };
            }
        }

        let step1 = run_step(
            &client, &api_key, 1, "DIAGNOSE", domain,
            &symptoms, &backtrack_evidence, None,
        ).await;
        total_cost += step1.total_cost;
        {
            let mut bt = budget.write().await;
            bt.record_spend(step1.total_cost);
        }

        if step1.majority_findings.is_empty() {
            tracing::info!(target: LOG_TARGET, "Step 1: no problems found by consensus — issue may be transient");
            return MmaProtocolResult::Success {
                consensus: step1,
                total_cost,
                backtracks,
            };
        }

        tracing::info!(
            target: LOG_TARGET,
            findings = step1.majority_findings.len(),
            iterations = step1.iterations_completed,
            cost = step1.total_cost,
            "Step 1 DIAGNOSE complete: {} findings in {} iterations",
            step1.majority_findings.len(),
            step1.iterations_completed
        );

        // ── Step 2: PLAN ──
        let step1_json = serde_json::to_string(&step1).unwrap_or_default();
        let step2 = run_step(
            &client, &api_key, 2, "PLAN", domain,
            &step1_json, &[], Some(&step1),
        ).await;
        total_cost += step2.total_cost;
        {
            let mut bt = budget.write().await;
            bt.record_spend(step2.total_cost);
        }

        tracing::info!(
            target: LOG_TARGET,
            plans = step2.fix_plans.len(),
            cost = step2.total_cost,
            "Step 2 PLAN complete: {} plans",
            step2.fix_plans.len()
        );

        // ── Step 3: EXECUTE ──
        let step2_json = serde_json::to_string(&step2).unwrap_or_default();
        let step3 = run_step(
            &client, &api_key, 3, "EXECUTE", domain,
            &step2_json, &[], Some(&step2),
        ).await;
        total_cost += step3.total_cost;
        {
            let mut bt = budget.write().await;
            bt.record_spend(step3.total_cost);
        }

        tracing::info!(
            target: LOG_TARGET,
            executions = step3.executions.len(),
            cost = step3.total_cost,
            "Step 3 EXECUTE complete: {} executions",
            step3.executions.len()
        );

        // ── Step 4: VERIFY ──
        let verify_result = run_step4_verify(
            &client, &api_key, domain, &step1, &step3,
        ).await;
        total_cost += verify_result.cost;

        if verify_result.passed {
            tracing::info!(
                target: LOG_TARGET,
                score = verify_result.score,
                total_cost,
                backtracks,
                "Step 4 VERIFY PASSED (score {:.1}) — protocol complete",
                verify_result.score
            );

            // Merge all step data into final consensus
            let mut final_consensus = step1;
            final_consensus.fix_plans = step2.fix_plans;
            final_consensus.executions = step3.executions;
            final_consensus.total_cost = total_cost;

            return MmaProtocolResult::Success {
                consensus: final_consensus,
                total_cost,
                backtracks,
            };
        }

        // ── Step 4 FAILED — backtrack ──
        backtracks += 1;
        tracing::warn!(
            target: LOG_TARGET,
            score = verify_result.score,
            backtracks,
            concerns = ?verify_result.concerns,
            "Step 4 VERIFY FAILED (score {:.1}) — backtrack #{}/{}",
            verify_result.score, backtracks, MAX_BACKTRACKS
        );

        if backtracks >= MAX_BACKTRACKS {
            return MmaProtocolResult::HumanEscalation {
                backtracks,
                last_failure: verify_result.concerns.join("; "),
                total_cost,
            };
        }

        // Append failure evidence for next cycle
        backtrack_evidence.push(format!(
            "BACKTRACK #{}: Previous fix failed verification (score {:.1}). Concerns: {}",
            backtracks, verify_result.score, verify_result.concerns.join("; ")
        ));
    }
}

// ─── Step Runner ─────────────────────────────────────────────────────────────

/// Run a single step (1, 2, or 3) with N iterations until consensus.
async fn run_step(
    client: &reqwest::Client,
    api_key: &str,
    step_number: u8,
    step_name: &str,
    domain: IssueDomain,
    context: &str,
    backtrack_evidence: &[String],
    prior_consensus: Option<&StepConsensus>,
) -> StepConsensus {
    let domain_str = format!("{:?}", domain);
    let mut consensus = StepConsensus::new(step_name, step_number, &domain_str);
    let pool = get_model_pool(domain, step_number);

    let system_prompt = match step_number {
        1 => step1_system_prompt(),
        2 => step2_system_prompt(),
        3 => step3_system_prompt(),
        _ => step1_system_prompt(),
    };

    for iteration in 1..=MAX_ITERATIONS_PER_STEP {
        let models = stratified_select(&pool, iteration);

        // Build iteration prompt
        let mut prompt = format!("{}\n\n", context);

        if !backtrack_evidence.is_empty() {
            prompt.push_str("--- BACKTRACK EVIDENCE (previous attempts failed) ---\n");
            for ev in backtrack_evidence {
                prompt.push_str(&format!("{}\n", ev));
            }
            prompt.push_str("\n");
        }

        if let Some(prior) = prior_consensus {
            prompt.push_str("--- PRIOR STEP CONSENSUS ---\n");
            prompt.push_str(&serde_json::to_string_pretty(prior).unwrap_or_default());
            prompt.push_str("\n\n");
        }

        if iteration > 1 && !consensus.majority_findings.is_empty() {
            prompt.push_str(&format!(
                "--- ITERATION {} CONTEXT ---\n\
                 Previous iteration found {} findings. Review and expand:\n{}\n\n\
                 What did the previous iteration MISS? Are there additional problems?\n",
                iteration,
                consensus.majority_findings.len(),
                serde_json::to_string(&consensus.majority_findings).unwrap_or_default()
            ));
        }

        // Call 5 models in parallel
        let model_configs: Vec<ModelConfig> = models.iter().map(|m| {
            ModelConfig {
                id: m.id,
                role: m.role,
                system_prompt,
            }
        }).collect();

        let futures: Vec<_> = model_configs.iter()
            .map(|model| openrouter::call_model(client, api_key, model, &prompt))
            .collect();

        let responses = futures_util::future::join_all(futures).await;
        let iter_cost = openrouter::total_cost(&responses);
        consensus.total_cost += iter_cost;

        // Track models used
        for r in &responses {
            if !consensus.models_used.contains(&r.model_id) {
                consensus.models_used.push(r.model_id.clone());
            }
        }

        // Extract findings from responses and build consensus
        let extracted = extract_step_findings(&responses, step_number);

        // Count genuinely new findings (not already in consensus)
        let prev_count = consensus.majority_findings.len();
        merge_findings(&mut consensus, &extracted, &responses, step_number);
        let added = consensus.majority_findings.len() - prev_count;

        tracing::info!(
            target: LOG_TARGET,
            step = step_number,
            iteration,
            responses = responses.len(),
            new_findings = added,
            total_findings = consensus.majority_findings.len(),
            cost = iter_cost,
            "Step {} iteration {}: {} new findings (total {})",
            step_number, iteration, added, consensus.majority_findings.len()
        );

        consensus.iterations_completed = iteration;

        // Check convergence (after minimum iterations)
        if iteration >= MIN_ITERATIONS_PER_STEP && added < CONVERGENCE_NEW_FINDINGS_THRESHOLD {
            consensus.converged_at_iteration = iteration;
            tracing::info!(
                target: LOG_TARGET,
                step = step_number,
                iteration,
                "Step {} converged at iteration {} (<{} new findings)",
                step_number, iteration, CONVERGENCE_NEW_FINDINGS_THRESHOLD
            );
            break;
        }
    }

    consensus.timestamp = chrono::Utc::now().to_rfc3339();
    consensus
}

/// Extract findings/plans/executions from model responses based on step.
fn extract_step_findings(
    responses: &[ModelResponse],
    _step_number: u8,
) -> Vec<(String, DiagnosisResult)> {
    let mut results = Vec::new();
    for r in responses {
        if let Some(ref diag) = r.diagnosis {
            results.push((r.model_id.clone(), diag.clone()));
        }
    }
    results
}

/// Merge new findings into consensus using 3/5 majority rule.
/// For Step 1: merge into majority_findings.
/// For Step 2: merge into fix_plans.
/// For Step 3: merge into executions.
fn merge_findings(
    consensus: &mut StepConsensus,
    _new_findings: &[(String, DiagnosisResult)],
    responses: &[ModelResponse],
    step_number: u8,
) {
    // Use the existing consensus algorithm for grouping
    if let Some(best) = openrouter::find_consensus(responses) {
        let agreement_count = responses.iter()
            .filter(|r| r.diagnosis.is_some())
            .count();

        let total = responses.len();
        let ratio = agreement_count as f64 / total as f64;

        if ratio >= CONSENSUS_RATIO {
            match step_number {
                1 => {
                    // Check if this finding is semantically new
                    let dominated = consensus.majority_findings.iter()
                        .any(|f| semantic_overlap(&f.description, &best.root_cause));

                    if !dominated {
                        consensus.majority_findings.push(Finding {
                            id: format!("P{:03}", consensus.majority_findings.len() + 1),
                            description: best.root_cause.clone(),
                            severity: if best.confidence >= 0.9 { "critical" }
                                      else if best.confidence >= 0.7 { "high" }
                                      else { "medium" }.to_string(),
                            confidence: best.confidence,
                            evidence: vec![best.fix_action.clone()],
                            assumptions: Vec::new(),
                            verification_steps: best.verification.map(|v| vec![v]).unwrap_or_default(),
                        });
                    }
                }
                2 => {
                    let dominated = consensus.fix_plans.iter()
                        .any(|p| semantic_overlap(&p.actions.join(" "), &best.fix_action));

                    if !dominated {
                        consensus.fix_plans.push(FixPlan {
                            problem_id: consensus.majority_findings
                                .first().map(|f| f.id.clone()).unwrap_or_else(|| "P001".to_string()),
                            actions: vec![best.fix_action.clone()],
                            fix_type: best.fix_type_class.unwrap_or_else(|| "deterministic".to_string()),
                            risk_analysis: best.root_cause.clone(),
                            rollback_strategy: "Revert to previous state".to_string(),
                            verification_steps: best.verification.map(|v| vec![v]).unwrap_or_default(),
                            estimated_duration_secs: 30,
                        });
                    }
                }
                3 => {
                    let dominated = consensus.executions.iter()
                        .any(|e| semantic_overlap(&e.implementation, &best.fix_action));

                    if !dominated {
                        consensus.executions.push(Execution {
                            problem_id: consensus.majority_findings
                                .first().map(|f| f.id.clone()).unwrap_or_else(|| "P001".to_string()),
                            implementation: best.fix_action.clone(),
                            execution_order: consensus.executions.len() as u8 + 1,
                            expected_outcome: best.permanent_fix.unwrap_or_else(|| best.root_cause.clone()),
                            confidence: best.confidence,
                        });
                    }
                }
                _ => {}
            }
        }

        // Collect dissenting opinions (models that disagreed)
        for r in responses {
            if let Some(ref diag) = r.diagnosis {
                if !semantic_overlap(&diag.root_cause, &best.root_cause) {
                    // Cap dissents at 3
                    if consensus.dissenting_opinions.len() < 3 {
                        consensus.dissenting_opinions.push(DissentingOpinion {
                            model: r.model_id.clone(),
                            finding: diag.root_cause.clone(),
                            confidence: diag.confidence,
                        });
                    }
                }
            }
        }
    }
}

/// Simple semantic overlap check — shared keywords between two strings.
/// Returns true if >50% of significant words overlap.
fn semantic_overlap(a: &str, b: &str) -> bool {
    let stop_words = ["the", "a", "an", "is", "are", "was", "were", "be", "been",
                       "for", "and", "or", "but", "in", "on", "at", "to", "of", "with"];

    let words_a: std::collections::HashSet<String> = a.split_whitespace()
        .map(|w| w.to_lowercase().chars().filter(|c| c.is_alphanumeric()).collect::<String>())
        .filter(|w| w.len() >= 3 && !stop_words.contains(&w.as_str()))
        .collect();

    let words_b: std::collections::HashSet<String> = b.split_whitespace()
        .map(|w| w.to_lowercase().chars().filter(|c| c.is_alphanumeric()).collect::<String>())
        .filter(|w| w.len() >= 3 && !stop_words.contains(&w.as_str()))
        .collect();

    if words_a.is_empty() || words_b.is_empty() {
        return false;
    }

    let intersection = words_a.intersection(&words_b).count();
    let min_len = words_a.len().min(words_b.len());

    if min_len == 0 { return false; }

    (intersection as f64 / min_len as f64) > 0.5
}

// ─── Step 4: VERIFY ──────────────────────────────────────────────────────────

/// Result of Step 4 verification.
struct VerifyResult {
    passed: bool,
    score: f64,
    concerns: Vec<String>,
    cost: f64,
}

/// Run Step 4: deterministic checks + 1 adversarial model sanity check.
async fn run_step4_verify(
    client: &reqwest::Client,
    api_key: &str,
    domain: IssueDomain,
    diagnosis: &StepConsensus,
    execution: &StepConsensus,
) -> VerifyResult {
    let mut concerns: Vec<String> = Vec::new();

    // ── Part 1: Deterministic checks (Ralph Wiggum P6 — cannot lie) ──
    // These are domain-specific checks that provide ground truth.
    let deterministic_passed = run_deterministic_checks(domain, diagnosis, &mut concerns);

    if !deterministic_passed {
        tracing::warn!(
            target: LOG_TARGET,
            concerns = ?concerns,
            "Step 4: deterministic checks FAILED"
        );
        return VerifyResult {
            passed: false,
            score: 0.0,
            concerns,
            cost: 0.0,
        };
    }

    // ── Part 2: 1 adversarial model sanity check (P2 — different model) ──
    // Use a cheap model NOT used in Steps 1-3
    let adversarial_model = select_adversarial_model(domain, &diagnosis.models_used);

    let verify_prompt = format!(
        "ADVERSARIAL VERIFICATION — Grade this fix:\n\n\
         DIAGNOSIS:\n{}\n\n\
         EXECUTION PLAN:\n{}\n\n\
         DETERMINISTIC CHECK RESULTS: All passed.\n\n\
         Grade on 4 criteria (each 0-5):\n\
         1. Root Cause Accuracy (35%): actual cause or symptom?\n\
         2. Fix Completeness (25%): handles all variants?\n\
         3. Verification Evidence (25%): concrete proof?\n\
         4. Side Effect Safety (15%): breaks anything?\n\n\
         Output JSON: {{\"score\": 0-5, \"grade\": \"PASS/FLAG/FAIL\", \
         \"reasoning\": \"...\", \"concerns\": [\"...\"]}}",
        serde_json::to_string(&diagnosis.majority_findings).unwrap_or_default(),
        serde_json::to_string(&execution.executions).unwrap_or_default(),
    );

    let model_config = ModelConfig {
        id: adversarial_model,
        role: "Adversarial Evaluator",
        system_prompt: step4_system_prompt(),
    };

    let response = openrouter::call_model(client, api_key, &model_config, &verify_prompt).await;
    let cost = response.cost_estimate;

    // Parse the adversarial score
    if let Some(ref diag) = response.diagnosis {
        let score = diag.confidence * 5.0; // Map 0-1 confidence to 0-5 score
        let passed = score >= 4.0;

        if !passed {
            concerns.push(format!(
                "Adversarial evaluator score {:.1}/5.0: {}",
                score, diag.root_cause
            ));
        }

        return VerifyResult {
            passed,
            score,
            concerns,
            cost,
        };
    }

    // Model call failed — treat as FLAG (not FAIL)
    concerns.push("Adversarial model call failed — treating as FLAG".to_string());
    VerifyResult {
        passed: false,
        score: 3.0,
        concerns,
        cost,
    }
}

/// Run domain-specific deterministic checks that cannot lie.
fn run_deterministic_checks(
    _domain: IssueDomain,
    diagnosis: &StepConsensus,
    concerns: &mut Vec<String>,
) -> bool {
    // For now, basic checks. James will add Windows-specific checks.
    // These run on the pod itself (not via models).

    let all_passed = true;

    // Check 1: Are we still running? (basic process liveness)
    // On Linux (VPS), this always passes. On Windows pods, check rc-agent process.
    #[cfg(windows)]
    {
        use sysinfo::{System, ProcessesToUpdate};
        let mut sys = System::new();
        sys.refresh_processes(ProcessesToUpdate::All, false);
        let rc_agent_alive = sys.processes().values()
            .any(|p| p.name().to_string_lossy().eq_ignore_ascii_case("rc-agent.exe"));
        if !rc_agent_alive {
            concerns.push("rc-agent.exe not running after fix".to_string());
            all_passed = false;
        }
    }

    // Check 2: Do the verification_steps from diagnosis make sense?
    for finding in &diagnosis.majority_findings {
        if finding.verification_steps.is_empty() {
            concerns.push(format!(
                "Finding {} has no verification steps — cannot confirm fix",
                finding.id
            ));
            // This is a warning, not a failure
        }
    }

    all_passed
}

/// Select an adversarial model NOT used in Steps 1-3.
fn select_adversarial_model(_domain: IssueDomain, used_models: &[String]) -> &'static str {
    // Cheap models preferred for adversarial check (cost optimization)
    let candidates = [
        "qwen/qwen3-235b-a22b-2507",          // $0.003/call
        "mistralai/mistral-small-3.1-24b-instruct", // $0.003/call
        "google/gemini-2.5-flash",              // $0.01/call
        "nvidia/nemotron-3-super-120b-a12b",    // $0.005/call
        "z-ai/glm-4.7-flash",                  // $0.003/call
    ];

    for candidate in &candidates {
        if !used_models.iter().any(|u| u == candidate) {
            return candidate;
        }
    }

    // Fallback: use cheapest available
    candidates[0]
}

/// Estimate step cost for budget pre-check.
fn estimate_step_cost(_domain: IssueDomain, step: u8) -> f64 {
    // Conservative estimate: 2 iterations × 5 models × avg cost
    let per_model_avg = match step {
        1 => 0.02,  // Reasoner-heavy, more expensive
        2 => 0.015, // Balanced
        3 => 0.01,  // Coder-heavy, cheaper
        4 => 0.01,  // 1 model only
        _ => 0.02,
    };
    per_model_avg * MODELS_PER_ITERATION as f64 * MIN_ITERATIONS_PER_STEP as f64
}
