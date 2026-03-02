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

/// Try Ollama first, fall back to Anthropic. Returns (response, model_used).
pub async fn query_ai(
    config: &AiDebuggerConfig,
    messages: &[Value],
) -> anyhow::Result<(String, String)> {
    // Try Ollama
    match query_ollama(&config.ollama_url, &config.ollama_model, messages).await {
        Ok(reply) => {
            return Ok((reply, format!("ollama/{}", config.ollama_model)));
        }
        Err(e) => {
            tracing::warn!("Ollama failed: {}. Trying Anthropic...", e);
        }
    }

    // Fallback to Anthropic
    if let Some(api_key) = &config.anthropic_api_key {
        let reply = query_anthropic(api_key, &config.anthropic_model, messages).await?;
        Ok((reply, format!("anthropic/{}", config.anthropic_model)))
    } else {
        anyhow::bail!("Ollama unavailable and no Anthropic API key configured")
    }
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
        "You are James, the friendly AI assistant at RacingPoint eSports Cafe. \
        You help customers with their racing stats, venue info, and sim racing tips.\n\n\
        CUSTOMER & VENUE DATA:\n{}\n\n\
        Be friendly and enthusiastic about sim racing. Keep responses concise. \
        If asked about other customers' data, politely decline.",
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
    ctx.push_str("\nGames available: Assetto Corsa, iRacing, Le Mans Ultimate, F1 25, Forza\n");
    ctx.push_str("Location: Bandlaguda, Hyderabad\n");

    ctx
}
