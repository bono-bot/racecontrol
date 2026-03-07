//! Central AI service for RaceControl.
//!
//! All AI calls (chat, crash analysis, pattern detection) route through this module.
//! Uses Ollama (local) as primary, Anthropic API as fallback.

use std::time::Duration;

use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::SqlitePool;
use tokio::sync::RwLock;

use crate::billing::BillingManager;
use crate::config::AiDebuggerConfig;
use crate::game_launcher::GameManager;
use rc_common::types::PodInfo;

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
    let client = reqwest::Client::new();
    let resp = client
        .post(&format!("{}/api/chat", url))
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

    let client = reqwest::Client::new();
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

/// Try Claude CLI → Ollama → Anthropic API. Returns (response, model_used).
pub async fn query_ai(
    config: &AiDebuggerConfig,
    messages: &[Value],
) -> anyhow::Result<(String, String)> {
    // 1. Try Claude CLI (best quality when online)
    if config.claude_cli_enabled {
        // Flatten messages into a single prompt for Claude CLI
        let prompt = messages_to_prompt(messages);
        match query_claude_cli(&prompt, config.claude_cli_timeout_secs).await {
            Ok(reply) => {
                return Ok((reply, "claude-cli".to_string()));
            }
            Err(e) => {
                tracing::warn!("Claude CLI failed: {}. Trying Ollama...", e);
            }
        }
    }

    // 2. Try Ollama (always available locally)
    match query_ollama(&config.ollama_url, &config.ollama_model, messages).await {
        Ok(reply) => {
            return Ok((reply, format!("ollama/{}", config.ollama_model)));
        }
        Err(e) => {
            tracing::warn!("Ollama failed: {}. Trying Anthropic API...", e);
        }
    }

    // 3. Fallback to Anthropic API
    if let Some(api_key) = &config.anthropic_api_key {
        let reply = query_anthropic(api_key, &config.anthropic_model, messages).await?;
        Ok((reply, format!("anthropic/{}", config.anthropic_model)))
    } else {
        anyhow::bail!("All AI providers failed (Claude CLI, Ollama, Anthropic API)")
    }
}

/// Flatten a messages array into a single prompt string for Claude CLI.
fn messages_to_prompt(messages: &[Value]) -> String {
    let mut prompt = String::new();
    for msg in messages {
        let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("user");
        let content = msg.get("content").and_then(|c| c.as_str()).unwrap_or("");
        match role {
            "system" => {
                prompt.push_str("[System]\n");
                prompt.push_str(content);
                prompt.push_str("\n\n");
            }
            "user" => {
                prompt.push_str(content);
                prompt.push('\n');
            }
            "assistant" => {
                prompt.push_str("[Previous response]\n");
                prompt.push_str(content);
                prompt.push_str("\n\n");
            }
            _ => {
                prompt.push_str(content);
                prompt.push('\n');
            }
        }
    }
    prompt
}

// ─── Business Context ────────────────────────────────────────────────────────

/// Gather live venue state from database + in-memory state.
pub async fn gather_business_context(
    db: &SqlitePool,
    pods: &RwLock<std::collections::HashMap<String, PodInfo>>,
    billing: &BillingManager,
    game_launcher: &GameManager,
) -> String {
    let mut ctx = String::new();

    // Today's sessions
    let today_sessions: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM billing_sessions WHERE date(started_at) = date('now')",
    )
    .fetch_one(db)
    .await
    .unwrap_or(0);

    // Today's revenue
    let today_revenue: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(p.price_paise), 0) FROM billing_sessions bs \
         JOIN pricing_tiers p ON bs.pricing_tier_id = p.id \
         WHERE date(bs.started_at) = date('now') AND bs.status IN ('completed', 'active', 'ended_early')",
    )
    .fetch_one(db)
    .await
    .unwrap_or(0);

    // This week revenue
    let week_revenue: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(p.price_paise), 0) FROM billing_sessions bs \
         JOIN pricing_tiers p ON bs.pricing_tier_id = p.id \
         WHERE bs.started_at >= datetime('now', '-7 days')",
    )
    .fetch_one(db)
    .await
    .unwrap_or(0);

    // Total drivers
    let total_drivers: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM drivers")
            .fetch_one(db)
            .await
            .unwrap_or(0);

    ctx.push_str(&format!(
        "Today's sessions: {}\nToday's revenue: {} INR\nThis week's revenue: {} INR\nTotal registered drivers: {}\n\n",
        today_sessions,
        today_revenue / 100,
        week_revenue / 100,
        total_drivers,
    ));

    // Active billing sessions
    let timers = billing.active_timers.read().await;
    if timers.is_empty() {
        ctx.push_str("Active billing sessions: none\n");
    } else {
        ctx.push_str("Active billing sessions:\n");
        for (_, timer) in timers.iter() {
            ctx.push_str(&format!(
                "  - Pod {}: {} ({}, {}s remaining)\n",
                timer.pod_id, timer.driver_name, timer.pricing_tier_name, timer.remaining_seconds()
            ));
        }
    }
    drop(timers);
    ctx.push('\n');

    // Connected pods
    let pods_map = pods.read().await;
    if pods_map.is_empty() {
        ctx.push_str("Connected pods: none\n");
    } else {
        ctx.push_str(&format!("Connected pods: {}\n", pods_map.len()));
        for (_id, pod) in pods_map.iter() {
            ctx.push_str(&format!(
                "  - {} (Pod #{}): {:?}, game: {:?}\n",
                pod.name, pod.number, pod.status, pod.current_game
            ));
        }
    }
    ctx.push('\n');

    // Active games
    let games = game_launcher.active_games.read().await;
    if !games.is_empty() {
        ctx.push_str("Active games:\n");
        for (pod_id, tracker) in games.iter() {
            ctx.push_str(&format!(
                "  - Pod {}: {:?} ({:?})\n",
                pod_id,
                tracker.to_info().sim_type,
                tracker.to_info().game_state
            ));
        }
        ctx.push('\n');
    }

    // Recent crashes (last 24h)
    let crashes = sqlx::query_as::<_, (String, String, Option<String>, String)>(
        "SELECT pod_id, sim_type, error_message, created_at FROM game_launch_events \
         WHERE event_type = 'crash' AND created_at > datetime('now', '-24 hours') \
         ORDER BY created_at DESC LIMIT 5",
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();

    if !crashes.is_empty() {
        ctx.push_str("Recent crashes (last 24h):\n");
        for (pod_id, sim, err, time) in &crashes {
            ctx.push_str(&format!(
                "  - {} on pod {} at {} ({})\n",
                sim,
                pod_id,
                time,
                err.as_deref().unwrap_or("no details")
            ));
        }
        ctx.push('\n');
    }

    // Pricing tiers
    let tiers = sqlx::query_as::<_, (String, i64, i64)>(
        "SELECT name, duration_minutes, price_paise FROM pricing_tiers WHERE is_active = 1 ORDER BY sort_order",
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();

    if !tiers.is_empty() {
        ctx.push_str("Pricing tiers:\n");
        for (name, mins, price) in &tiers {
            ctx.push_str(&format!("  - {}: {} min, {} INR\n", name, mins, price / 100));
        }
    }

    ctx
}

/// Build system prompt for staff/admin AI chat.
pub fn build_staff_prompt(context: &str) -> String {
    format!(
        "You are James, the AI operations assistant for RacingPoint eSports and Cafe \
        in Bandlaguda, Hyderabad. You help staff and admins with venue operations, \
        billing, pod management, and troubleshooting.\n\n\
        CURRENT VENUE STATE (live data):\n{}\n\n\
        Answer concisely and accurately based on the data above. If you don't have \
        enough data to answer, say so. Prices are in INR. Keep responses under 200 words \
        unless asked for detail.",
        context
    )
}

/// Build system prompt for customer AI chat.
pub fn build_customer_prompt(context: &str) -> String {
    format!(
        "You are Bono, the friendly AI assistant at RacingPoint eSports Cafe \
        in Bandlaguda, Hyderabad. You help customers with their racing stats, \
        venue info, pricing, and sim racing tips.\n\n\
        CUSTOMER & VENUE DATA:\n{}\n\n\
        Be friendly, enthusiastic, and knowledgeable about sim racing. Keep responses concise. \
        When mentioning lap times, use a format like \"1:23.456\". \
        Proactively share interesting facts like the fastest lap of the day when relevant. \
        If asked about other customers' private data, politely decline. \
        You may share public leaderboard data like track records and fastest laps.",
        context
    )
}

/// Gather customer-scoped context (only their own data).
pub async fn gather_customer_context(db: &SqlitePool, driver_id: &str) -> String {
    let mut ctx = String::new();

    // Driver info
    let driver = sqlx::query_as::<_, (String, i64, i64)>(
        "SELECT name, total_laps, total_time_ms FROM drivers WHERE id = ?",
    )
    .bind(driver_id)
    .fetch_optional(db)
    .await
    .ok()
    .flatten();

    if let Some((name, laps, time_ms)) = driver {
        ctx.push_str(&format!(
            "Customer: {}\nTotal laps: {}\nTotal drive time: {} minutes\n\n",
            name,
            laps,
            time_ms / 60000
        ));
    }

    // Personal bests
    let bests = sqlx::query_as::<_, (String, String, i64)>(
        "SELECT track, car, best_lap_ms FROM personal_bests WHERE driver_id = ? ORDER BY best_lap_ms ASC LIMIT 10",
    )
    .bind(driver_id)
    .fetch_all(db)
    .await
    .unwrap_or_default();

    if !bests.is_empty() {
        ctx.push_str("Personal bests:\n");
        for (track, car, lap_ms) in &bests {
            let secs = *lap_ms as f64 / 1000.0;
            ctx.push_str(&format!("  - {} ({}): {:.3}s\n", track, car, secs));
        }
        ctx.push('\n');
    }

    // Recent sessions
    let sessions = sqlx::query_as::<_, (String, String, i64, String)>(
        "SELECT pt.name, bs.pod_id, bs.driving_seconds, bs.started_at \
         FROM billing_sessions bs JOIN pricing_tiers pt ON bs.pricing_tier_id = pt.id \
         WHERE bs.driver_id = ? ORDER BY bs.started_at DESC LIMIT 5",
    )
    .bind(driver_id)
    .fetch_all(db)
    .await
    .unwrap_or_default();

    if !sessions.is_empty() {
        ctx.push_str("Recent sessions:\n");
        for (tier, pod, secs, started) in &sessions {
            ctx.push_str(&format!(
                "  - {} on pod {} ({} min driven, {})\n",
                tier, pod, secs / 60, started
            ));
        }
        ctx.push('\n');
    }

    // Venue pricing
    let tiers = sqlx::query_as::<_, (String, i64, i64)>(
        "SELECT name, duration_minutes, price_paise FROM pricing_tiers WHERE is_active = 1 ORDER BY sort_order",
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();

    ctx.push_str("Available pricing:\n");
    for (name, mins, price) in &tiers {
        ctx.push_str(&format!("  - {}: {} min, {} INR\n", name, mins, price / 100));
    }
    // Fastest lap of the day
    let fastest_today = sqlx::query_as::<_, (String, String, String, i64)>(
        "SELECT d.name, l.track, l.car, l.lap_time_ms \
         FROM laps l JOIN drivers d ON l.driver_id = d.id \
         WHERE date(l.created_at) = date('now') AND l.valid = 1 \
         ORDER BY l.lap_time_ms ASC LIMIT 1",
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten();

    if let Some((name, track, car, lap_ms)) = fastest_today {
        let mins = lap_ms / 60000;
        let secs = (lap_ms % 60000) as f64 / 1000.0;
        if mins > 0 {
            ctx.push_str(&format!(
                "\nFastest lap of the day: set by {} on {} with {} — {}:{:06.3}\n",
                name, track, car, mins, secs
            ));
        } else {
            ctx.push_str(&format!(
                "\nFastest lap of the day: set by {} on {} with {} — {:.3}s\n",
                name, track, car, secs
            ));
        }
    }

    ctx.push_str("\nGames available: Assetto Corsa, iRacing, Le Mans Ultimate, F1 25, Forza\n");
    ctx.push_str("Location: Bandlaguda, Hyderabad\n");

    ctx
}
