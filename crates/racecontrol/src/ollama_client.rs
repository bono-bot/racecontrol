//! v29.0 Phase 34: Ollama HTTP client for AI diagnosis.
//! Calls local Ollama on James machine (192.168.31.27:11434).

use serde::{Serialize, Deserialize};

const LOG_TARGET: &str = "ollama";
const OLLAMA_URL: &str = "http://192.168.31.27:11434/api/generate";
const DEFAULT_MODEL: &str = "qwen2.5:3b";
const FALLBACK_MODEL: &str = "llama3.1:8b";
const TIMEOUT_SECS: u64 = 30;

#[derive(Serialize)]
struct OllamaRequest {
    model: String,
    prompt: String,
    stream: bool,
}

#[derive(Deserialize)]
struct OllamaResponse {
    response: String,
    #[serde(default)]
    #[allow(dead_code)]
    done: bool,
}

/// Send a diagnosis prompt to Ollama and get structured response.
pub async fn diagnose(prompt: &str) -> anyhow::Result<String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(TIMEOUT_SECS))
        .build()?;

    // Try primary model first
    match call_ollama(&client, DEFAULT_MODEL, prompt).await {
        Ok(response) => {
            tracing::info!(target: LOG_TARGET, model = DEFAULT_MODEL, len = response.len(), "Ollama diagnosis complete");
            Ok(response)
        }
        Err(e) => {
            tracing::warn!(target: LOG_TARGET, model = DEFAULT_MODEL, error = %e, "Primary model failed, trying fallback");
            // Fallback to larger model
            match call_ollama(&client, FALLBACK_MODEL, prompt).await {
                Ok(response) => {
                    tracing::info!(target: LOG_TARGET, model = FALLBACK_MODEL, len = response.len(), "Fallback diagnosis complete");
                    Ok(response)
                }
                Err(e2) => {
                    // MMA-v29: Log full errors server-side but return generic message to callers
                    // (prevents leaking internal model names, IPs, and reqwest error details)
                    tracing::error!(target: LOG_TARGET, primary_error = %e, fallback_error = %e2, "Both Ollama models failed");
                    Err(anyhow::anyhow!("AI diagnosis service unavailable"))
                }
            }
        }
    }
}

async fn call_ollama(client: &reqwest::Client, model: &str, prompt: &str) -> anyhow::Result<String> {
    let req = OllamaRequest {
        model: model.to_string(),
        prompt: prompt.to_string(),
        stream: false,
    };

    let resp = client.post(OLLAMA_URL)
        .json(&req)
        .send()
        .await?;

    if !resp.status().is_success() {
        // MMA-v29: Log status server-side, return generic error
        tracing::warn!(target: LOG_TARGET, status = %resp.status(), model, "Ollama HTTP error");
        return Err(anyhow::anyhow!("AI diagnosis service error"));
    }

    let body: OllamaResponse = resp.json().await?;
    Ok(body.response)
}

/// Check if Ollama is reachable.
pub async fn health_check() -> bool {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build();

    match client {
        Ok(c) => c.get("http://192.168.31.27:11434/api/tags")
            .send().await
            .map(|r| r.status().is_success())
            .unwrap_or(false),
        Err(_) => false,
    }
}
