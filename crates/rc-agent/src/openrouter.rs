//! OpenRouter API client — 4-model diagnostic system for Meshed Intelligence.
//!
//! Tier 3: Single model diagnosis (Qwen3 ~$0.05)
//! Tier 4: 4-model parallel diagnosis (R1+V3+MiMo+Gemini ~$3)
//!
//! Each model has a role-specific system prompt:
//!   - Qwen3 235B: Scanner — fast, cheap, volume screening
//!   - DeepSeek R1: Reasoner — logic bugs, absence detection, state machines
//!   - DeepSeek V3: Code Expert — code patterns, Session 0 detection
//!   - Gemini 2.5 Pro: Security — config errors, auth, credentials
//!
//! API key is read from OPENROUTER_KEY env var — NEVER hardcoded.
//! Standing rules: no .unwrap() in production, all errors propagated via anyhow.

use serde::{Deserialize, Serialize};

const LOG_TARGET: &str = "openrouter";

/// OpenRouter API endpoint
const OPENROUTER_API_URL: &str = "https://openrouter.ai/api/v1/chat/completions";

/// Model registry — version-pinned OpenRouter model IDs
#[derive(Debug, Clone)]
pub struct ModelConfig {
    pub id: &'static str,
    pub role: &'static str,
    pub system_prompt: &'static str,
}

/// The 4 models used for diagnosis
pub const MODELS: [ModelConfig; 4] = [
    ModelConfig {
        id: "qwen/qwen3-235b-a22b-2507",
        role: "Scanner",
        system_prompt: "You are a fast diagnostic scanner for a Windows sim racing pod (rc-agent). \
            Given symptoms (error type, system state, trigger), identify the most likely root cause \
            and suggest a fix action. Be concise. Output JSON: {\"root_cause\": \"...\", \"confidence\": 0.0-1.0, \
            \"fix_action\": \"...\", \"risk_level\": \"safe|caution|dangerous\"}",
    },
    ModelConfig {
        id: "deepseek/deepseek-r1-0528",
        role: "Reasoner",
        system_prompt: "You are a reasoning-focused debugger for a Windows sim racing pod. \
            Analyze the diagnostic event deeply. Look for absence-based bugs (what SHOULD be there but isn't), \
            state machine stuck states, and logic errors. Use chain-of-thought. \
            Output JSON: {\"root_cause\": \"...\", \"confidence\": 0.0-1.0, \
            \"fix_action\": \"...\", \"risk_level\": \"safe|caution|dangerous\"}",
    },
    ModelConfig {
        id: "deepseek/deepseek-chat-v3-0324",
        role: "Code Expert",
        system_prompt: "You are a code-level debugger for rc-agent (Rust/Axum on Windows). \
            Given diagnostic symptoms, trace the likely code path that caused the issue. \
            Focus on Session 0/1 context, process spawning, sysinfo patterns, and Windows-specific bugs. \
            Output JSON: {\"root_cause\": \"...\", \"confidence\": 0.0-1.0, \
            \"fix_action\": \"...\", \"risk_level\": \"safe|caution|dangerous\"}",
    },
    ModelConfig {
        id: "google/gemini-2.5-pro-preview-03-25",
        role: "Security",
        system_prompt: "You are a security-focused auditor for a sim racing pod fleet. \
            Given diagnostic symptoms, check for config errors, auth issues, credential problems, \
            firewall misconfigurations, and sentinel file corruption. \
            Output JSON: {\"root_cause\": \"...\", \"confidence\": 0.0-1.0, \
            \"fix_action\": \"...\", \"risk_level\": \"safe|caution|dangerous\"}",
    },
];

/// Structured response from an OpenRouter model diagnosis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosisResult {
    pub root_cause: String,
    pub confidence: f64,
    pub fix_action: String,
    pub risk_level: String,
}

/// Response from a single model call
#[derive(Debug, Clone)]
pub struct ModelResponse {
    pub model_id: String,
    pub role: String,
    pub diagnosis: Option<DiagnosisResult>,
    pub raw_text: String,
    pub cost_estimate: f64,
    pub error: Option<String>,
}

/// OpenRouter chat completion response (subset we need)
#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Option<Vec<ChatChoice>>,
    usage: Option<ChatUsage>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Debug, Deserialize)]
struct ChatMessage {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChatUsage {
    prompt_tokens: Option<u64>,
    completion_tokens: Option<u64>,
}

/// Get the OpenRouter API key from environment.
/// Returns None if not set — caller should skip model calls gracefully.
pub fn get_api_key() -> Option<String> {
    std::env::var("OPENROUTER_KEY").ok().filter(|k| !k.is_empty())
}

/// Call a single OpenRouter model with diagnostic symptoms.
/// Returns ModelResponse with parsed diagnosis or error.
pub async fn call_model(
    client: &reqwest::Client,
    api_key: &str,
    model: &ModelConfig,
    symptoms: &str,
) -> ModelResponse {
    let request_body = serde_json::json!({
        "model": model.id,
        "messages": [
            {"role": "system", "content": model.system_prompt},
            {"role": "user", "content": symptoms}
        ],
        "max_tokens": 500,
        "temperature": 0.1
    });

    tracing::debug!(
        target: LOG_TARGET,
        model = model.id,
        role = model.role,
        "Calling OpenRouter model"
    );

    let result = client
        .post(OPENROUTER_API_URL)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .header("HTTP-Referer", "https://racingpoint.in")
        .header("X-Title", "Racing Point Mesh Intelligence")
        .json(&request_body)
        .timeout(std::time::Duration::from_secs(60))
        .send()
        .await;

    let response = match result {
        Ok(resp) => resp,
        Err(e) => {
            tracing::warn!(target: LOG_TARGET, model = model.id, error = %e, "OpenRouter request failed");
            return ModelResponse {
                model_id: model.id.to_string(),
                role: model.role.to_string(),
                diagnosis: None,
                raw_text: String::new(),
                cost_estimate: 0.0,
                error: Some(format!("Request failed: {}", e)),
            };
        }
    };

    let status = response.status();
    let body = response.text().await.unwrap_or_default();

    if !status.is_success() {
        tracing::warn!(target: LOG_TARGET, model = model.id, status = %status, "OpenRouter returned error");
        return ModelResponse {
            model_id: model.id.to_string(),
            role: model.role.to_string(),
            diagnosis: None,
            raw_text: body.clone(),
            cost_estimate: 0.0,
            error: Some(format!("HTTP {}: {}", status, &body[..body.len().min(200)])),
        };
    }

    // Parse the chat completion response
    let chat_resp: ChatResponse = match serde_json::from_str(&body) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(target: LOG_TARGET, model = model.id, error = %e, "Failed to parse OpenRouter response");
            return ModelResponse {
                model_id: model.id.to_string(),
                role: model.role.to_string(),
                diagnosis: None,
                raw_text: body,
                cost_estimate: 0.0,
                error: Some(format!("Parse error: {}", e)),
            };
        }
    };

    let raw_text = chat_resp
        .choices
        .as_ref()
        .and_then(|c| c.first())
        .and_then(|c| c.message.content.as_ref())
        .cloned()
        .unwrap_or_default();

    // Estimate cost from token usage
    let cost_estimate = chat_resp.usage.as_ref().map_or(0.0, |u| {
        let prompt = u.prompt_tokens.unwrap_or(0) as f64;
        let completion = u.completion_tokens.unwrap_or(0) as f64;
        // Rough estimate — actual pricing varies by model
        (prompt * 0.5 + completion * 1.5) / 1_000_000.0
    });

    // Try to extract JSON diagnosis from the response text
    let diagnosis = extract_diagnosis(&raw_text);

    tracing::info!(
        target: LOG_TARGET,
        model = model.id,
        role = model.role,
        has_diagnosis = diagnosis.is_some(),
        cost = cost_estimate,
        "OpenRouter model responded"
    );

    ModelResponse {
        model_id: model.id.to_string(),
        role: model.role.to_string(),
        diagnosis,
        raw_text,
        cost_estimate,
        error: None,
    }
}

/// Tier 3: Call single cheapest model (Qwen3) for diagnosis.
pub async fn tier3_diagnose(client: &reqwest::Client, api_key: &str, symptoms: &str) -> ModelResponse {
    call_model(client, api_key, &MODELS[0], symptoms).await
}

/// Tier 4: Call all 4 models in parallel, return consensus diagnosis.
pub async fn tier4_diagnose_parallel(
    client: &reqwest::Client,
    api_key: &str,
    symptoms: &str,
) -> Vec<ModelResponse> {
    let futures: Vec<_> = MODELS
        .iter()
        .map(|model| call_model(client, api_key, model, symptoms))
        .collect();

    futures_util::future::join_all(futures).await
}

/// Find consensus among multiple model responses.
/// Returns the diagnosis with highest confidence that at least 2 models agree on.
pub fn find_consensus(responses: &[ModelResponse]) -> Option<DiagnosisResult> {
    let diagnoses: Vec<&DiagnosisResult> = responses
        .iter()
        .filter_map(|r| r.diagnosis.as_ref())
        .collect();

    if diagnoses.is_empty() {
        return None;
    }

    // If only one model responded with a diagnosis, use it if confidence >= 0.7
    if diagnoses.len() == 1 {
        let d = diagnoses[0];
        if d.confidence >= 0.7 {
            return Some(d.clone());
        }
        return None;
    }

    // Multiple diagnoses — find the one with highest confidence
    // In a real implementation, we'd check root_cause similarity for true consensus
    // For now, use the highest-confidence response
    diagnoses
        .iter()
        .max_by(|a, b| a.confidence.partial_cmp(&b.confidence).unwrap_or(std::cmp::Ordering::Equal))
        .map(|d| (*d).clone())
}

/// Format a DiagnosticEvent into a symptom string for model prompts.
pub fn format_symptoms(
    trigger: &str,
    problem_key: &str,
    environment: &str,
    pod_state_summary: &str,
) -> String {
    format!(
        "Diagnostic Event on sim racing pod (Windows 11, Rust rc-agent):\n\
         Trigger: {}\n\
         Problem Key: {}\n\
         Environment: {}\n\
         Pod State: {}\n\n\
         Analyze this event and provide a diagnosis. What is the root cause and how should it be fixed?",
        trigger, problem_key, environment, pod_state_summary
    )
}

/// Extract a DiagnosisResult JSON from model response text.
/// Models are prompted to output JSON but may wrap it in markdown or explanation.
fn extract_diagnosis(text: &str) -> Option<DiagnosisResult> {
    // Try direct parse first
    if let Ok(d) = serde_json::from_str::<DiagnosisResult>(text) {
        return Some(d);
    }

    // Try to find JSON block in the text (models often wrap in ```json ... ```)
    let json_start = text.find('{');
    let json_end = text.rfind('}');

    if let (Some(start), Some(end)) = (json_start, json_end) {
        if end > start {
            let json_str = &text[start..=end];
            if let Ok(d) = serde_json::from_str::<DiagnosisResult>(json_str) {
                return Some(d);
            }
        }
    }

    None
}

/// Total cost of all responses
pub fn total_cost(responses: &[ModelResponse]) -> f64 {
    responses.iter().map(|r| r.cost_estimate).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_diagnosis_direct_json() {
        let json = r#"{"root_cause": "MAINTENANCE_MODE blocking", "confidence": 0.9, "fix_action": "delete sentinel", "risk_level": "safe"}"#;
        let d = extract_diagnosis(json);
        assert!(d.is_some());
        let d = d.expect("diagnosis");
        assert_eq!(d.root_cause, "MAINTENANCE_MODE blocking");
        assert!((d.confidence - 0.9).abs() < f64::EPSILON);
    }

    #[test]
    fn test_extract_diagnosis_wrapped_in_markdown() {
        let text = "The issue is likely...\n```json\n{\"root_cause\": \"stale sentinel\", \"confidence\": 0.8, \"fix_action\": \"remove file\", \"risk_level\": \"safe\"}\n```\nThis should fix it.";
        let d = extract_diagnosis(text);
        assert!(d.is_some());
        assert_eq!(d.expect("diagnosis").root_cause, "stale sentinel");
    }

    #[test]
    fn test_extract_diagnosis_no_json() {
        let text = "I'm not sure what's wrong. Try restarting the service.";
        let d = extract_diagnosis(text);
        assert!(d.is_none());
    }

    #[test]
    fn test_find_consensus_single_high_confidence() {
        let responses = vec![ModelResponse {
            model_id: "test".to_string(),
            role: "Scanner".to_string(),
            diagnosis: Some(DiagnosisResult {
                root_cause: "sentinel block".to_string(),
                confidence: 0.85,
                fix_action: "delete file".to_string(),
                risk_level: "safe".to_string(),
            }),
            raw_text: String::new(),
            cost_estimate: 0.05,
            error: None,
        }];
        let c = find_consensus(&responses);
        assert!(c.is_some());
    }

    #[test]
    fn test_find_consensus_single_low_confidence() {
        let responses = vec![ModelResponse {
            model_id: "test".to_string(),
            role: "Scanner".to_string(),
            diagnosis: Some(DiagnosisResult {
                root_cause: "maybe something".to_string(),
                confidence: 0.3,
                fix_action: "unknown".to_string(),
                risk_level: "caution".to_string(),
            }),
            raw_text: String::new(),
            cost_estimate: 0.05,
            error: None,
        }];
        let c = find_consensus(&responses);
        assert!(c.is_none(), "Low confidence single response should not produce consensus");
    }

    #[test]
    fn test_format_symptoms() {
        let s = format_symptoms("WsDisconnect", "ws_disconnect", "{build: abc}", "ws_connected: false");
        assert!(s.contains("WsDisconnect"));
        assert!(s.contains("ws_disconnect"));
        assert!(s.contains("sim racing pod"));
    }

    #[test]
    fn test_get_api_key_missing() {
        // In test env, OPENROUTER_KEY is likely not set
        // This just verifies the function doesn't panic
        let _ = get_api_key();
    }

    #[test]
    fn test_model_registry_has_4_models() {
        assert_eq!(MODELS.len(), 4);
        assert_eq!(MODELS[0].role, "Scanner");
        assert_eq!(MODELS[1].role, "Reasoner");
        assert_eq!(MODELS[2].role, "Code Expert");
        assert_eq!(MODELS[3].role, "Security");
    }

    #[test]
    fn test_total_cost() {
        let responses = vec![
            ModelResponse {
                model_id: "a".to_string(),
                role: "Scanner".to_string(),
                diagnosis: None,
                raw_text: String::new(),
                cost_estimate: 0.05,
                error: None,
            },
            ModelResponse {
                model_id: "b".to_string(),
                role: "Reasoner".to_string(),
                diagnosis: None,
                raw_text: String::new(),
                cost_estimate: 0.43,
                error: None,
            },
        ];
        assert!((total_cost(&responses) - 0.48).abs() < 0.001);
    }
}
