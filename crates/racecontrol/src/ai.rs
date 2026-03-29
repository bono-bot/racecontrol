//! Central AI service for RaceControl.
//!
//! All AI calls (chat, crash analysis, pattern detection) route through this module.
//! Priority: Claude CLI → Ollama (venue, with learned context) → Anthropic API.
//! Automatically logs Claude CLI responses as training pairs for Ollama to learn from.

use std::collections::HashSet;
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
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(90))
        .build()
        .unwrap_or_default();
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

/// Try Claude CLI → Ollama (venue, with learned context) → Anthropic API. Returns (response, model_used).
/// When `db` is provided, automatically logs responses as training pairs for future learning.
pub async fn query_ai(
    config: &AiDebuggerConfig,
    messages: &[Value],
    db: Option<&SqlitePool>,
    source: Option<&str>,
) -> anyhow::Result<(String, String)> {
    let user_query = extract_user_query(messages);

    // 1. Primary: Claude CLI
    if config.claude_cli_enabled {
        let prompt = messages_to_prompt(messages);
        match query_claude_cli(&prompt, config.claude_cli_timeout_secs).await {
            Ok(reply) => {
                // Log this Q&A pair for future learning
                if let Some(db) = db {
                    log_training_pair(
                        db,
                        &user_query,
                        &reply,
                        source.unwrap_or("unknown"),
                        "claude-cli",
                    )
                    .await;
                }
                return Ok((reply, "claude-cli".to_string()));
            }
            Err(e) => {
                tracing::warn!("Claude CLI failed: {}. Trying Ollama...", e);
            }
        }
    }

    // 2. Fallback: Ollama (venue-local, with learned context from training pairs)
    {
        let few_shot = if let Some(db) = db {
            find_similar_pairs(db, &user_query, 3).await
        } else {
            vec![]
        };
        let enhanced = build_enhanced_messages(messages, &few_shot);

        match query_ollama(&config.ollama_url, &config.ollama_model, &enhanced).await {
            Ok(reply) => {
                // Increment use_count on training pairs we used
                if let Some(db) = db {
                    for pair in &few_shot {
                        let _ = sqlx::query(
                            "UPDATE ai_training_pairs SET use_count = use_count + 1 WHERE id = ?",
                        )
                        .bind(&pair.id)
                        .execute(db)
                        .await;
                    }
                }

                tracing::info!(
                    "AI query answered by Ollama (with {} examples)",
                    few_shot.len()
                );
                return Ok((reply, format!("ollama/{}", config.ollama_model)));
            }
            Err(e) => {
                tracing::warn!("Ollama failed: {}. Trying Anthropic API...", e);
            }
        }
    }

    // 3. Final fallback: Anthropic API
    if let Some(api_key) = &config.anthropic_api_key {
        let reply = query_anthropic(api_key, &config.anthropic_model, messages).await?;
        if let Some(db) = db {
            log_training_pair(
                db,
                &user_query,
                &reply,
                source.unwrap_or("unknown"),
                &format!("anthropic/{}", config.anthropic_model),
            )
            .await;
        }
        Ok((reply, format!("anthropic/{}", config.anthropic_model)))
    } else {
        anyhow::bail!("All AI providers failed (Claude CLI, Ollama, Anthropic API)")
    }
}

// ─── Learning System ────────────────────────────────────────────────────────

struct TrainingPair {
    id: String,
    query_text: String,
    response_text: String,
}

/// Stop words to strip when extracting keywords.
const STOP_WORDS: &[&str] = &[
    "the", "is", "a", "an", "and", "or", "but", "in", "on", "at", "to", "for",
    "of", "with", "by", "from", "as", "it", "this", "that", "are", "was", "be",
    "has", "had", "not", "no", "do", "does", "did", "will", "would", "could",
    "should", "may", "can", "been", "being", "have", "were", "they", "them",
    "their", "its", "you", "your", "we", "our", "i", "my", "me", "he", "she",
    "his", "her", "what", "which", "who", "when", "where", "how", "all", "each",
    "every", "both", "few", "more", "most", "other", "some", "such", "than",
    "too", "very", "just", "about", "above", "after", "again", "also", "any",
    "because", "before", "between", "here", "there", "into", "only", "over",
    "same", "so", "then", "these", "those", "through", "under", "up", "out",
];

/// Extract significant keywords from text for similarity matching.
/// (Private version used internally)
fn extract_keywords(text: &str) -> String {
    let stop: HashSet<&str> = STOP_WORDS.iter().copied().collect();
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '_' && c != '.')
        .filter(|w| w.len() >= 2 && !stop.contains(w))
        .collect::<Vec<&str>>()
        .join(" ")
}

/// Public wrapper for extract_keywords — used by training import endpoint.
pub fn extract_keywords_pub(text: &str) -> String {
    extract_keywords(text)
}

/// Extract the user's query text from a messages array.
fn extract_user_query(messages: &[Value]) -> String {
    messages
        .iter()
        .filter(|m| m.get("role").and_then(|r| r.as_str()) == Some("user"))
        .filter_map(|m| m.get("content").and_then(|c| c.as_str()))
        .collect::<Vec<&str>>()
        .join("\n")
}

/// Find similar past training pairs by keyword overlap.
async fn find_similar_pairs(
    db: &SqlitePool,
    query: &str,
    limit: usize,
) -> Vec<TrainingPair> {
    let keywords = extract_keywords(query);
    let words: Vec<&str> = keywords.split_whitespace().collect();

    if words.is_empty() {
        return vec![];
    }

    // Take top 8 most significant keywords (longer words first, more likely to be specific)
    let mut sorted_words: Vec<&str> = words.clone();
    sorted_words.sort_by(|a, b| b.len().cmp(&a.len()));
    sorted_words.truncate(8);

    // Build SQL with LIKE conditions using parameterized bindings
    let like_clauses: Vec<String> = sorted_words
        .iter()
        .enumerate()
        .map(|(_, _)| "(CASE WHEN query_keywords LIKE ? THEN 1 ELSE 0 END)".to_string())
        .collect();

    let score_expr = like_clauses.join(" + ");
    let sql = format!(
        "SELECT id, query_text, response_text, ({}) as score \
         FROM ai_training_pairs \
         WHERE quality_score > 0 AND score >= 2 \
         ORDER BY score DESC, use_count DESC \
         LIMIT ?",
        score_expr
    );

    let mut query = sqlx::query_as::<_, (String, String, String, i32)>(&sql);
    for w in &sorted_words {
        // MMA-C20: Escape SQL LIKE wildcards to prevent wildcard injection
        let escaped = w.replace('%', "\\%").replace('_', "\\_");
        query = query.bind(format!("%{}%", escaped));
    }
    query = query.bind(limit as i32);

    let rows = query
        .fetch_all(db)
        .await
        .unwrap_or_default();

    rows.into_iter()
        .map(|(id, query_text, response_text, _score)| TrainingPair {
            id,
            query_text,
            response_text,
        })
        .collect()
}

/// Log a query-response pair for future Ollama learning.
/// MMA-C9: Validates source and content before storing to prevent training poisoning.
async fn log_training_pair(
    db: &SqlitePool,
    query: &str,
    response: &str,
    source: &str,
    model: &str,
) {
    // MMA-C9: Only accept training pairs from trusted sources
    const TRUSTED_SOURCES: &[&str] = &["healer", "healer_graduated", "chatbot", "diagnostic", "unknown"];
    if !TRUSTED_SOURCES.iter().any(|s| source.starts_with(s)) {
        tracing::warn!(target: "ai", "Training pair rejected: untrusted source '{}'", source);
        return;
    }
    // MMA-C9: Reject oversized pairs that could be context stuffing
    if query.len() > 4096 || response.len() > 8192 {
        tracing::warn!(target: "ai", "Training pair rejected: oversized (query={}B, response={}B)", query.len(), response.len());
        return;
    }
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    // Simple hash for dedup
    let mut hasher = DefaultHasher::new();
    query.hash(&mut hasher);
    let query_hash = format!("{:x}", hasher.finish());

    let keywords = extract_keywords(query);
    let id = uuid::Uuid::new_v4().to_string();

    let result = sqlx::query(
        "INSERT INTO ai_training_pairs \
         (id, query_hash, query_text, query_keywords, response_text, source, model) \
         SELECT ?, ?, ?, ?, ?, ?, ? \
         WHERE NOT EXISTS (SELECT 1 FROM ai_training_pairs WHERE query_hash = ?)",
    )
    .bind(&id)
    .bind(&query_hash)
    .bind(query)
    .bind(&keywords)
    .bind(response)
    .bind(source)
    .bind(model)
    .bind(&query_hash)
    .execute(db)
    .await;

    match result {
        Ok(r) if r.rows_affected() > 0 => {
            tracing::debug!("AI training: logged new pair (source: {}, model: {})", source, model);
        }
        Ok(_) => {
            tracing::debug!("AI training: duplicate query skipped");
        }
        Err(e) => {
            tracing::warn!("AI training: failed to log pair: {}", e);
        }
    }
}

/// Build enhanced messages with domain context and few-shot examples for Ollama.
fn build_enhanced_messages(messages: &[Value], few_shot: &[TrainingPair]) -> Vec<Value> {
    let mut enhanced = Vec::new();

    // Inject domain context into the system message
    let domain_ctx = build_domain_context();

    // Find existing system message or create one
    let existing_system = messages
        .iter()
        .find(|m| m.get("role").and_then(|r| r.as_str()) == Some("system"))
        .and_then(|m| m.get("content").and_then(|c| c.as_str()))
        .unwrap_or("");

    let system_content = if existing_system.is_empty() {
        domain_ctx
    } else {
        format!("{}\n\n{}", existing_system, domain_ctx)
    };

    enhanced.push(json!({"role": "system", "content": system_content}));

    // Add few-shot examples as conversation history
    for pair in few_shot {
        let q = if pair.query_text.len() > 500 {
            format!("{}...", &pair.query_text[..500])
        } else {
            pair.query_text.clone()
        };
        let a = if pair.response_text.len() > 800 {
            format!("{}...", &pair.response_text[..800])
        } else {
            pair.response_text.clone()
        };
        enhanced.push(json!({"role": "user", "content": q}));
        enhanced.push(json!({"role": "assistant", "content": a}));
    }

    // Add original non-system messages
    for msg in messages {
        if msg.get("role").and_then(|r| r.as_str()) != Some("system") {
            enhanced.push(msg.clone());
        }
    }

    enhanced
}

/// Build static domain context about RaceControl for Ollama.
fn build_domain_context() -> String {
    "\
You are James, the AI operations assistant for RacingPoint eSports and Cafe (Bandlaguda, Hyderabad).

SYSTEM KNOWLEDGE:
- 8 sim racing pods on subnet 192.168.31.x (Pod 1-8), each running Windows 11 with rc-agent
- Wheelbases: Conspit Ares 8Nm (OpenFFBoard USB VID:0x1209 PID:0xFFB0)
- racecontrol server runs on 192.168.31.23:8080 (Rust/Axum), manages billing, pods, games
- pod-agent runs on port 8090 on each pod for remote management
- rc-agent lock screen on port 18923, debug on 18924
- Games: Assetto Corsa (UDP 9996), F1 (20777), Forza (5300), iRacing (6789), LMU (5555)
- Billing tiers: 5min free trial, 30min/₹700, 60min/₹900, 10s idle threshold
- Common issues: CLOSE_WAIT zombie sockets (fix: kill stale TCP), USB wheelbase disconnect, \
  pod-agent freeze after long uptime, Content Manager vs acs.exe launch conflicts
- Protected processes (never kill): rc-agent, pod-agent, ConspitLink2.0, explorer, dwm, csrss
- AC launch: acs.exe directly (not Steam), AUTOSPAWN=1 in race.ini, CSP gui.ini FORCE_START=1
- ConspitLink2.0 is managed by rc-agent's 10s watchdog (auto-restarts if crashed, do NOT suggest restarting it)

When diagnosing issues, consider: network (DHCP drift, firewall), USB (wheelbase disconnect), \
process zombies (CLOSE_WAIT), disk space, and Windows updates blocking."
        .to_string()
}

/// MMA-C5: Sanitize text that will be embedded in AI prompts.
/// Strips common prompt injection patterns (role overrides, system instructions).
pub fn sanitize_for_prompt(text: &str) -> String {
    let mut sanitized = text.to_string();
    // Strip role override attempts
    let injection_patterns = [
        "ignore previous instructions",
        "ignore all instructions",
        "you are now",
        "new instructions:",
        "system:",
        "[system]",
        "<|system|>",
        "ASSISTANT:",
        "Human:",
        "```system",
    ];
    let lower = sanitized.to_lowercase();
    for pattern in &injection_patterns {
        if lower.contains(pattern) {
            tracing::warn!(target: "ai", "Prompt injection pattern detected and stripped: {}", pattern);
            sanitized = sanitized.replace(pattern, "[FILTERED]");
            // Case-insensitive replacement
            let lower_sanitized = sanitized.to_lowercase();
            if lower_sanitized.contains(pattern) {
                sanitized = sanitized.to_string();
            }
        }
    }
    // Truncate to prevent context stuffing
    if sanitized.len() > 8192 {
        sanitized.truncate(8192);
        sanitized.push_str("... [TRUNCATED]");
    }
    sanitized
}

/// Flatten messages into a single prompt string for Claude CLI.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_keywords_basic() {
        let kw = extract_keywords("How does billing work in RaceControl?");
        assert!(kw.contains("billing"));
        assert!(kw.contains("racecontrol"));
        assert!(kw.contains("work"));
        // Stop words should be removed
        assert!(!kw.contains(" how "));
        assert!(!kw.contains(" does "));
        assert!(!kw.contains(" in "));
    }

    #[test]
    fn test_extract_keywords_preserves_underscores_dots() {
        let kw = extract_keywords("check pod_agent on 192.168.31.91");
        assert!(kw.contains("pod_agent"));
        assert!(kw.contains("192.168.31.91"));
    }

    #[test]
    fn test_extract_keywords_filters_short_words() {
        let kw = extract_keywords("a b c de xyz billing");
        // 'a', 'b', 'c' are <2 chars, filtered out. 'de' is 2 chars, kept.
        assert!(!kw.split_whitespace().any(|w| w.len() < 2));
        assert!(kw.contains("billing"));
    }

    #[test]
    fn test_extract_keywords_empty() {
        let kw = extract_keywords("");
        assert!(kw.is_empty());
    }

    #[test]
    fn test_build_domain_context_not_empty() {
        let ctx = build_domain_context();
        assert!(!ctx.is_empty());
        assert!(ctx.contains("RacingPoint"));
        assert!(ctx.contains("192.168.31"));
        assert!(ctx.contains("Ollama") || ctx.contains("pod") || ctx.contains("billing"));
    }

    #[test]
    fn test_extract_user_query() {
        let messages = vec![
            json!({"role": "system", "content": "You are James"}),
            json!({"role": "user", "content": "Hello there"}),
            json!({"role": "assistant", "content": "Hi!"}),
            json!({"role": "user", "content": "How are you?"}),
        ];
        let query = extract_user_query(&messages);
        assert!(query.contains("Hello there"));
        assert!(query.contains("How are you?"));
        assert!(!query.contains("You are James"));
        assert!(!query.contains("Hi!"));
    }

    #[test]
    fn test_messages_to_prompt() {
        let messages = vec![
            json!({"role": "system", "content": "System msg"}),
            json!({"role": "user", "content": "User msg"}),
        ];
        let prompt = messages_to_prompt(&messages);
        assert!(prompt.contains("[System]"));
        assert!(prompt.contains("System msg"));
        assert!(prompt.contains("User msg"));
    }

    #[test]
    fn test_build_enhanced_messages_with_few_shot() {
        let messages = vec![
            json!({"role": "system", "content": "Base system"}),
            json!({"role": "user", "content": "What is billing?"}),
        ];
        let pairs = vec![
            TrainingPair {
                id: "1".to_string(),
                query_text: "How does billing work?".to_string(),
                response_text: "Billing starts when...".to_string(),
            },
        ];
        let enhanced = build_enhanced_messages(&messages, &pairs);
        // Should have: system, few-shot user, few-shot assistant, original user
        assert!(enhanced.len() >= 4);
        // System should contain domain context
        let sys = enhanced[0]["content"].as_str().unwrap();
        assert!(sys.contains("RacingPoint"));
    }
}
