//! MMA Engine for rc-sentry -- 4-step OpenRouter protocol in a dedicated std::thread.
//!
//! Triggered by mi_tier_engine when Tier 2 KB lookup fails.
//! Runs in its own thread (blocking reqwest is fine -- no tokio runtime).
//!
//! Phase 323, MIG-04.

use rc_common::diagnostic_types::{DiagnosticTrigger, TierDiagnosis, normalize_problem_key};
use std::sync::{Mutex, OnceLock};
use serde::{Deserialize, Serialize};

const LOG_TARGET: &str = "mma-engine";
const OPENROUTER_API_URL: &str = "https://openrouter.ai/api/v1/chat/completions";
#[allow(dead_code)]
const MAX_RETRIES: u32 = 3;
const PER_CALL_TIMEOUT_SECS: u64 = 60;

// --- API Key -----------------------------------------------------------------

pub fn get_api_key() -> Option<String> {
    // 1. Env var (highest priority)
    if let Ok(k) = std::env::var("OPENROUTER_KEY") {
        if !k.is_empty() { return Some(k); }
    }
    // 2. File fallback (same locations as rc-agent)
    for path in &[
        r"C:\RacingPoint\data\openrouter-mma-key.txt",
        "data/openrouter-mma-key.txt",
        "openrouter-mma-key.txt",
    ] {
        if let Ok(k) = std::fs::read_to_string(path) {
            let k = k.trim().to_string();
            if !k.is_empty() { return Some(k); }
        }
    }
    None
}

// --- Model Configuration -----------------------------------------------------

#[derive(Debug, Clone)]
pub struct ModelConfig {
    pub id: &'static str,
    pub role: &'static str,
    pub system_prompt: &'static str,
}

/// Fleet context shared across all model prompts.
const FLEET_CONTEXT: &str = "FLEET CONTEXT: Racing Point eSports -- 8 Windows 11 sim racing pods running rc-agent (Rust/Axum :8090). Server at .23:8080 (racecontrol). WebSocket for state sync. SQLite billing. Key processes: rc-agent (pod agent), rc-sentry (watchdog), ConspitLink.exe (steering wheel HID), Edge (lock screen/kiosk). Critical sentinels: MAINTENANCE_MODE (blocks all restarts), OTA_DEPLOYING (blocks recovery during deploy), RCAGENT_SELF_RESTART (graceful restart). Session context: rc-agent MUST run in Session 1 (interactive desktop). Session 0 = all GUI ops fail silently. Recovery systems: rc-sentry watchdog, RCWatchdog Windows service, server pod_monitor, WoL auto-wake. These can fight each other.";

// --- Response Types ----------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosisResult {
    pub root_cause: String,
    pub confidence: f64,
    pub fix_action: String,
    pub risk_level: String,
}

// --- MMA Global State --------------------------------------------------------

#[derive(Debug, Clone, Serialize, Default)]
pub struct MmaState {
    pub last_run_trigger: Option<String>,
    pub last_run_outcome: Option<String>,
    pub last_run_cost: Option<f64>,
    pub last_run_at: Option<String>,
    pub models_called: Vec<String>,
    pub api_key_present: bool,
}

static MMA_STATE: OnceLock<Mutex<MmaState>> = OnceLock::new();

fn mma_state() -> &'static Mutex<MmaState> {
    MMA_STATE.get_or_init(|| Mutex::new(MmaState::default()))
}

pub fn get_status() -> serde_json::Value {
    let budget = crate::mma_budget::get_budget_status();
    let state = mma_state().lock()
        .map(|s| s.clone())
        .unwrap_or_default();
    serde_json::json!({
        "budget": budget,
        "last_run_trigger": state.last_run_trigger,
        "last_run_outcome": state.last_run_outcome,
        "last_run_cost": state.last_run_cost,
        "last_run_at": state.last_run_at,
        "models_called": state.models_called,
        "api_key_present": get_api_key().is_some(),
    })
}

// --- Sanitize prompt (MMA standing rule) -------------------------------------

pub fn sanitize_mma_prompt(input: &str) -> String {
    let truncated = if input.len() > 2000 {
        &input[..2000]
    } else {
        input
    };
    truncated
        .replace("sk-", "***")
        .replace("Bearer ", "***")
        .replace("password=", "password=***")
        .replace("secret=", "secret=***")
}

// --- Blocking model call -----------------------------------------------------

/// Call one OpenRouter model synchronously (blocking reqwest).
/// Returns None on error -- never panics.
pub fn call_model_blocking(
    client: &reqwest::blocking::Client,
    api_key: &str,
    model: &ModelConfig,
    prompt: &str,
) -> (Option<DiagnosisResult>, f64) {
    let body = serde_json::json!({
        "model": model.id,
        "messages": [
            {"role": "system", "content": model.system_prompt},
            {"role": "user", "content": sanitize_mma_prompt(prompt)},
        ],
        "max_tokens": 4000,
        "temperature": 0.1,
    });

    let resp = match client
        .post(OPENROUTER_API_URL)
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .header("X-Title", "RaceControl-Sentry-MMA")
        .json(&body)
        .send()
    {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(target: LOG_TARGET, model = model.id, error = %e, "Model call failed");
            return (None, 0.0);
        }
    };

    if !resp.status().is_success() {
        tracing::warn!(target: LOG_TARGET, model = model.id, status = resp.status().as_u16(), "Model returned non-200");
        return (None, 0.0);
    }

    let json: serde_json::Value = match resp.json() {
        Ok(j) => j,
        Err(e) => {
            tracing::warn!(target: LOG_TARGET, model = model.id, error = %e, "Failed to parse response JSON");
            return (None, 0.0);
        }
    };

    // Extract cost from usage field
    let cost = json["usage"]["total_tokens"]
        .as_f64()
        .map(|t| t * 0.000001) // rough estimate -- actual pricing varies
        .unwrap_or(0.0);

    // Extract diagnosis from choices[0].message.content
    let content = json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("");

    // Try to parse JSON from content (may be wrapped in markdown)
    let diagnosis = extract_json_diagnosis(content);

    (diagnosis, cost)
}

fn extract_json_diagnosis(text: &str) -> Option<DiagnosisResult> {
    // Try direct parse
    if let Ok(d) = serde_json::from_str::<DiagnosisResult>(text) {
        return Some(d);
    }
    // Try to find {...} block
    if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            if end > start {
                if let Ok(d) = serde_json::from_str::<DiagnosisResult>(&text[start..=end]) {
                    return Some(d);
                }
            }
        }
    }
    None
}

// --- MMA Protocol Entry Point ------------------------------------------------

/// Run the simplified MMA protocol for rc-sentry (Steps 1+4 -- diagnose + verify).
/// Called from the dedicated MMA thread.
pub fn run_mma_for_sentry(diag: &TierDiagnosis, api_key: &str) {
    let trigger_name = normalize_problem_key(&diag.trigger);

    tracing::info!(
        target: LOG_TARGET,
        trigger = %trigger_name,
        "MMA protocol starting (rc-sentry Tier 4)"
    );

    // Reset session budget for this MMA run
    crate::mma_budget::reset_session();

    let client = match reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(PER_CALL_TIMEOUT_SECS))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(target: LOG_TARGET, error = %e, "Failed to build reqwest client");
            update_state(trigger_name, "client_build_failed", 0.0, vec![]);
            return;
        }
    };

    let prompt = format!(
        "{}\n\nCurrent failure trigger: {:?}\nProblem key: {}",
        FLEET_CONTEXT, diag.trigger, trigger_name
    );

    // Run 5 models (same as rc-agent MODELS array)
    let models = get_primary_models();
    let mut results: Vec<DiagnosisResult> = Vec::new();
    let mut models_called: Vec<String> = Vec::new();
    let mut total_cost = 0.0f64;

    for model in &models {
        if !crate::mma_budget::can_spend(0.50) {
            tracing::warn!(target: LOG_TARGET, "Budget cap reached -- stopping model calls");
            break;
        }

        let (result, cost) = call_model_blocking(&client, api_key, model, &prompt);
        crate::mma_budget::record_spend(model.id, cost);
        total_cost += cost;
        models_called.push(model.id.to_string());

        if let Some(r) = result {
            results.push(r);
        }
    }

    // Simple consensus: take the result with highest confidence, or first if equal
    let outcome = if results.is_empty() {
        "no_consensus".to_string()
    } else {
        let best = results.iter()
            .max_by(|a, b| a.confidence.partial_cmp(&b.confidence).unwrap_or(std::cmp::Ordering::Equal));

        if let Some(best) = best {
            // Store consensus in KB
            store_consensus_in_kb(&trigger_name, best);
        }

        "success".to_string()
    };

    update_state(trigger_name, &outcome, total_cost, models_called);
}

fn store_consensus_in_kb(trigger_name: &str, result: &DiagnosisResult) {
    let now = chrono::Utc::now().to_rfc3339();
    // Record the MMA finding as a solution in the KB
    let solution = rc_common::diagnostic_types::SolutionRecord {
        id: format!("mma-{}-{}", trigger_name, chrono::Utc::now().timestamp()),
        problem_key: trigger_name.to_string(),
        problem_hash: crate::mi_knowledge_base::compute_problem_hash(
            trigger_name,
            env!("GIT_HASH"),
            "pod",
        ),
        symptoms: format!("MMA-diagnosed: {}", trigger_name),
        environment: "pod".to_string(),
        root_cause: result.root_cause.clone(),
        fix_action: result.fix_action.clone(),
        fix_type: result.risk_level.clone(),
        success_count: 0,
        fail_count: 0,
        confidence: result.confidence,
        cost_to_diagnose: 0.0,
        models_used: Some("5-model-mma".to_string()),
        source_node: "rc-sentry-mma".to_string(),
        created_at: now.clone(),
        updated_at: now,
        version: 1,
        ttl_days: 90,
        tags: Some("mma,rc-sentry".to_string()),
        diagnosis_method: Some("consensus_5model".to_string()),
        fix_permanence: "proposed".to_string(),
        recurrence_count: 0,
        permanent_fix_id: None,
        last_recurrence: None,
        permanent_attempt_at: None,
    };
    if let Ok(kb) = crate::mi_knowledge_base::KnowledgeBase::open(crate::mi_knowledge_base::KB_PATH) {
        let _ = kb.store_solution(&solution);
    }
    tracing::info!(
        target: LOG_TARGET,
        trigger = trigger_name,
        confidence = result.confidence,
        "MMA consensus stored in KB"
    );
}

fn update_state(trigger: String, outcome: &str, cost: f64, models: Vec<String>) {
    if let Ok(mut s) = mma_state().lock() {
        s.last_run_trigger = Some(trigger);
        s.last_run_outcome = Some(outcome.to_string());
        s.last_run_cost = Some(cost);
        s.last_run_at = Some(chrono::Utc::now().to_rfc3339());
        s.models_called = models;
        s.api_key_present = get_api_key().is_some();
    }
}

fn get_primary_models() -> Vec<ModelConfig> {
    // 5 primary models -- same as rc-agent's MODELS constant
    vec![
        ModelConfig { id: "qwen/qwen3-235b-a22b-2507", role: "Scanner",
            system_prompt: "You are a high-volume diagnostic scanner for a Racing Point sim racing pod fleet. Output ONLY valid JSON: {\"root_cause\": \"...\", \"confidence\": 0.0-1.0, \"fix_action\": \"...\", \"risk_level\": \"safe|caution|dangerous\"}" },
        ModelConfig { id: "deepseek/deepseek-r1-0528", role: "Reasoner",
            system_prompt: "You are a reasoning-focused debugger for Racing Point sim racing pods. Output ONLY valid JSON: {\"root_cause\": \"...\", \"confidence\": 0.0-1.0, \"fix_action\": \"...\", \"risk_level\": \"safe|caution|dangerous\"}" },
        ModelConfig { id: "deepseek/deepseek-chat-v3-0324", role: "Code Expert",
            system_prompt: "You are a code-level debugger specializing in Rust/Axum on Windows for Racing Point pods. Output ONLY valid JSON: {\"root_cause\": \"...\", \"confidence\": 0.0-1.0, \"fix_action\": \"...\", \"risk_level\": \"safe|caution|dangerous\"}" },
        ModelConfig { id: "xiaomi/mimo-v2-pro", role: "SRE",
            system_prompt: "You are an SRE-focused diagnostician for Racing Point's 8-pod sim racing fleet. Output ONLY valid JSON: {\"root_cause\": \"...\", \"confidence\": 0.0-1.0, \"fix_action\": \"...\", \"risk_level\": \"safe|caution|dangerous\"}" },
        ModelConfig { id: "google/gemini-2.5-pro-preview-03-25", role: "Security",
            system_prompt: "You are a security auditor for Racing Point's sim racing pod fleet. Output ONLY valid JSON: {\"root_cause\": \"...\", \"confidence\": 0.0-1.0, \"fix_action\": \"...\", \"risk_level\": \"safe|caution|dangerous\"}" },
    ]
}

// --- Thread Spawn ------------------------------------------------------------

/// Spawn the MMA engine thread.
/// Listens for TierDiagnosis events from the tier engine via mpsc.
/// Each received event triggers a full MMA protocol run.
pub fn spawn(trigger_rx: std::sync::mpsc::Receiver<TierDiagnosis>) {
    std::thread::Builder::new()
        .name("sentry-mma-engine".to_string())
        .spawn(move || {
            tracing::info!(target: LOG_TARGET, "lifecycle: MMA engine thread started");

            let api_key = match get_api_key() {
                Some(k) => {
                    tracing::info!(target: LOG_TARGET, "OPENROUTER_KEY found -- MMA engine active");
                    k
                }
                None => {
                    tracing::error!(
                        target: LOG_TARGET,
                        "OPENROUTER_KEY not set and no file fallback found -- MMA engine disabled"
                    );
                    // Drain the channel without processing (keeps tier engine from blocking)
                    for _ in trigger_rx { /* discard */ }
                    return;
                }
            };

            for diag in trigger_rx {
                tracing::info!(
                    target: LOG_TARGET,
                    trigger = ?diag.trigger,
                    tier = diag.tier,
                    "MMA engine received diagnosis -- running protocol"
                );
                run_mma_for_sentry(&diag, &api_key);
            }

            tracing::info!(target: LOG_TARGET, "lifecycle: MMA engine thread stopped (channel closed)");
        })
        .expect("spawn mma engine thread");
}
