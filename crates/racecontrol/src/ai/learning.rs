//! AI learning system: training pairs, keyword extraction, few-shot enhancement,
//! domain context, prompt sanitization, and message formatting.

use std::collections::HashSet;

use serde_json::{json, Value};
use sqlx::SqlitePool;

// ─── Learning System ────────────────────────────────────────────────────────

pub(crate) struct TrainingPair {
    pub id: String,
    pub query_text: String,
    pub response_text: String,
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
pub(crate) fn extract_user_query(messages: &[Value]) -> String {
    messages
        .iter()
        .filter(|m| m.get("role").and_then(|r| r.as_str()) == Some("user"))
        .filter_map(|m| m.get("content").and_then(|c| c.as_str()))
        .collect::<Vec<&str>>()
        .join("\n")
}

/// Find similar past training pairs by keyword overlap.
pub(crate) async fn find_similar_pairs(
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
pub(crate) async fn log_training_pair(
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
pub(crate) fn build_enhanced_messages(messages: &[Value], few_shot: &[TrainingPair]) -> Vec<Value> {
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
    for pattern in &injection_patterns {
        // Case-insensitive scan: rebuild sanitized by searching lowercase copy
        loop {
            let lower = sanitized.to_lowercase();
            if let Some(pos) = lower.find(pattern) {
                tracing::warn!(target: "ai", "Prompt injection pattern detected and stripped: {}", pattern);
                let end = pos + pattern.len();
                sanitized = format!("{}[FILTERED]{}", &sanitized[..pos], &sanitized[end..]);
            } else {
                break;
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
pub(crate) fn messages_to_prompt(messages: &[Value]) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
