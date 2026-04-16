//! AI provider implementations: Claude CLI, Ollama, OpenRouter, Anthropic API.

use std::time::Duration;

use serde::Deserialize;
use serde_json::{json, Value};

// ─── Claude CLI Call ─────────────────────────────────────────────────────────

/// Query Claude CLI in non-interactive print mode. Prompt is piped via stdin.
pub async fn query_claude_cli(prompt: &str, timeout_secs: u32) -> anyhow::Result<String> {
    use tokio::io::AsyncWriteExt;

    let mut child = tokio::process::Command::new("claude")
        .args(["-p", "--output-format", "text"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| anyhow::anyhow!("Claude CLI not found or failed to spawn: {}", e))?;

    // Write prompt to stdin
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(prompt.as_bytes()).await?;
        stdin.shutdown().await?;
    }

    // Wait with timeout
    let output = tokio::time::timeout(
        Duration::from_secs(timeout_secs as u64),
        child.wait_with_output(),
    )
    .await
    .map_err(|_| anyhow::anyhow!("Claude CLI timed out after {}s", timeout_secs))??;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Claude CLI exited with {}: {}", output.status, stderr.trim());
    }

    let result = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if result.is_empty() {
        anyhow::bail!("Claude CLI returned empty response");
    }
    Ok(result)
}

// ─── Ollama + Anthropic Calls ────────────────────────────────────────────────

/// Query Ollama's /api/chat endpoint with a message array.
pub async fn query_ollama(
    url: &str,
    model: &str,
    messages: &[Value],
) -> anyhow::Result<String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(90))
        .build()
        .unwrap_or_default();
    let resp = client
        .post(format!("{}/api/chat", url))
        .json(&json!({
            "model": model,
            "messages": messages,
            "stream": false,
            "options": {
                "temperature": 0.7,
                "num_predict": 1024,
            }
        }))
        .timeout(Duration::from_secs(60))
        .send()
        .await?;

    if !resp.status().is_success() {
        anyhow::bail!("Ollama returned status {}", resp.status());
    }

    #[derive(Deserialize)]
    struct OllamaMessage {
        content: String,
    }
    #[derive(Deserialize)]
    struct OllamaResponse {
        message: OllamaMessage,
    }
    let body: OllamaResponse = resp.json().await?;
    Ok(body.message.content)
}

// ─── OpenRouter Call ────────────────────────────────────────────────────────

const OPENROUTER_API_URL: &str = "https://openrouter.ai/api/v1/chat/completions";

/// Read OpenRouter API key: data file first (fresh), then env var (may be stale from session).
pub(crate) fn read_openrouter_key() -> Option<String> {
    // File first — always fresh, not subject to Windows session env caching issues.
    // On Windows, setx /M writes to registry but running processes keep the old value
    // until a full logoff/reboot. The file is always current.
    let path = std::path::Path::new("data/openrouter-mma-key.txt");
    if let Ok(contents) = std::fs::read_to_string(path) {
        let k = contents.trim().to_string();
        if !k.is_empty() {
            return Some(k);
        }
    }
    // Fallback: env var
    if let Ok(k) = std::env::var("OPENROUTER_KEY")
        && !k.is_empty() {
            return Some(k);
        }
    None
}

/// Query OpenRouter API (MMA-class models for higher quality diagnostics).
pub async fn query_openrouter(
    model: &str,
    messages: &[Value],
) -> anyhow::Result<String> {
    let api_key = read_openrouter_key()
        .ok_or_else(|| anyhow::anyhow!("No OpenRouter key (OPENROUTER_KEY env or data/openrouter-mma-key.txt)"))?;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(90))
        .build()
        .unwrap_or_default();

    let resp = client
        .post(OPENROUTER_API_URL)
        .bearer_auth(&api_key)
        .header("HTTP-Referer", "https://racingpoint.in")
        .header("X-Title", "RacingPoint Debug Diagnostics")
        .json(&json!({
            "model": model,
            "messages": messages,
            "max_tokens": 2048,
            "temperature": 0.3,
        }))
        .timeout(Duration::from_secs(60))
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("OpenRouter returned {} — {}", status, text);
    }

    #[derive(Deserialize)]
    struct Choice {
        message: ChoiceMessage,
    }
    #[derive(Deserialize)]
    struct ChoiceMessage {
        content: String,
    }
    #[derive(Deserialize)]
    struct OpenRouterResponse {
        choices: Vec<Choice>,
    }

    let body: OpenRouterResponse = resp.json().await?;
    body.choices
        .first()
        .map(|c| c.message.content.clone())
        .ok_or_else(|| anyhow::anyhow!("OpenRouter returned empty choices"))
}

/// Query Anthropic Messages API.
pub async fn query_anthropic(
    api_key: &str,
    model: &str,
    messages: &[Value],
) -> anyhow::Result<String> {
    // Anthropic expects system message separate from messages array
    let system = messages
        .iter()
        .find(|m| m.get("role").and_then(|r| r.as_str()) == Some("system"))
        .and_then(|m| m.get("content").and_then(|c| c.as_str()))
        .unwrap_or("");

    let user_messages: Vec<&Value> = messages
        .iter()
        .filter(|m| m.get("role").and_then(|r| r.as_str()) != Some("system"))
        .collect();

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .unwrap_or_default();
    let resp = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .json(&json!({
            "model": model,
            "max_tokens": 1024,
            "system": system,
            "messages": user_messages,
        }))
        .timeout(Duration::from_secs(30))
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("Anthropic returned {} — {}", status, text);
    }

    #[derive(Deserialize)]
    struct Content {
        text: String,
    }
    #[derive(Deserialize)]
    struct AnthropicResponse {
        content: Vec<Content>,
    }
    let body: AnthropicResponse = resp.json().await?;
    Ok(body
        .content
        .first()
        .map(|c| c.text.clone())
        .unwrap_or_default())
}
