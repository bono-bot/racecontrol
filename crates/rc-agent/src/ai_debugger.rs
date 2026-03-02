use chrono::Utc;
use serde::Deserialize;
use tokio::sync::mpsc;

use rc_common::types::{AiDebugSuggestion, SimType};

#[derive(Debug, Clone, Default, Deserialize)]
pub struct AiDebuggerConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_ollama_url")]
    pub ollama_url: String,
    #[serde(default = "default_ollama_model")]
    pub ollama_model: String,
    pub anthropic_api_key: Option<String>,
}

fn default_ollama_url() -> String {
    "http://localhost:11434".to_string()
}
fn default_ollama_model() -> String {
    "llama3.1:8b".to_string()
}

/// Analyze a crash/error and produce a debug suggestion.
/// Runs as a spawned async task — makes HTTP calls to Ollama/Anthropic.
pub async fn analyze_crash(
    config: AiDebuggerConfig,
    pod_id: String,
    sim_type: SimType,
    error_context: String,
    result_tx: mpsc::Sender<AiDebugSuggestion>,
) {
    let prompt = build_prompt(&sim_type, &error_context);

    // Try Ollama first (local, fast, no internet needed)
    match query_ollama(&config.ollama_url, &config.ollama_model, &prompt).await {
        Ok(suggestion) => {
            let _ = result_tx
                .send(AiDebugSuggestion {
                    pod_id,
                    sim_type,
                    error_context,
                    suggestion,
                    model: format!("ollama/{}", config.ollama_model),
                    created_at: Utc::now(),
                })
                .await;
            return;
        }
        Err(e) => {
            tracing::warn!("Ollama query failed: {}. Trying Anthropic fallback...", e);
        }
    }

    // Fallback to Anthropic API
    if let Some(api_key) = &config.anthropic_api_key {
        match query_anthropic(api_key, &prompt).await {
            Ok(suggestion) => {
                let _ = result_tx
                    .send(AiDebugSuggestion {
                        pod_id,
                        sim_type,
                        error_context,
                        suggestion,
                        model: "anthropic/claude-sonnet".to_string(),
                        created_at: Utc::now(),
                    })
                    .await;
            }
            Err(e) => {
                tracing::error!("Anthropic query also failed: {}", e);
            }
        }
    } else {
        tracing::warn!("No Anthropic API key configured and Ollama failed — no AI debug available");
    }
}

fn build_prompt(sim_type: &SimType, error_context: &str) -> String {
    format!(
        "You are an expert sim racing technician debugging issues at a commercial sim racing venue. \
        The game {} has encountered an issue on one of our gaming pods.\n\n\
        Error context: {}\n\n\
        Provide a concise, actionable debugging suggestion. Consider common causes like:\n\
        - GPU driver crashes\n\
        - Out of memory\n\
        - Content/mod conflicts\n\
        - Corrupted game files\n\
        - Network/Steam issues\n\
        - Controller/wheelbase disconnection\n\n\
        Keep the response under 200 words and focus on the most likely cause and fix.",
        sim_type, error_context
    )
}

async fn query_ollama(url: &str, model: &str, prompt: &str) -> anyhow::Result<String> {
    let client = reqwest::Client::new();
    let resp = client
        .post(&format!("{}/api/generate", url))
        .json(&serde_json::json!({
            "model": model,
            "prompt": prompt,
            "stream": false,
        }))
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await?;

    #[derive(Deserialize)]
    struct OllamaResponse {
        response: String,
    }
    let body: OllamaResponse = resp.json().await?;
    Ok(body.response)
}

async fn query_anthropic(api_key: &str, prompt: &str) -> anyhow::Result<String> {
    let client = reqwest::Client::new();
    let resp = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .json(&serde_json::json!({
            "model": "claude-sonnet-4-20250514",
            "max_tokens": 500,
            "messages": [{"role": "user", "content": prompt}]
        }))
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await?;

    #[derive(Deserialize)]
    struct AnthropicContent {
        text: String,
    }
    #[derive(Deserialize)]
    struct AnthropicResponse {
        content: Vec<AnthropicContent>,
    }
    let body: AnthropicResponse = resp.json().await?;
    Ok(body
        .content
        .first()
        .map(|c| c.text.clone())
        .unwrap_or_default())
}
