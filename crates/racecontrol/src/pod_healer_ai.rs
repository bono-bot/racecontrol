//! Pod healer AI escalation — LLM-based analysis and WARN log surge detection.
//!
//! Handles escalation of complex pod issues to AI (Ollama/Claude) and monitors
//! the server WARN log for surge patterns requiring AI diagnosis.
//!
//! Extracted from pod_healer.rs (Phase 385, v49.0 Architecture Completion).

use std::sync::Arc;

use chrono::Utc;
use serde_json::json;

use crate::activity_log::log_pod_activity;
use crate::pod_healer::HealAction;
use crate::state::AppState;
use rc_common::protocol::DashboardEvent;
use rc_common::types::{AiDebugSuggestion, PodInfo, SimType};

// ─── AI Escalation ───────────────────────────────────────────────────────────

pub(crate) async fn escalate_to_ai(
    state: &Arc<AppState>,
    pod: &PodInfo,
    issues: &[String],
    actions_taken: &[HealAction],
) {
    let actions_desc = if actions_taken.is_empty() {
        "No auto-heal actions taken.".to_string()
    } else {
        actions_taken
            .iter()
            .map(|a| format!("  - {} on {} ({})", a.action, a.target, a.reason))
            .collect::<Vec<_>>()
            .join("\n")
    };

    // MMA-C5: Sanitize issue strings before embedding in AI prompt
    let sanitized_issues: Vec<String> = issues
        .iter()
        .map(|i| crate::ai::sanitize_for_prompt(i))
        .collect();
    let context = format!(
        "POD HEALTH ALERT -- Pod {} (#{}, IP: {})\n\n\
         Issues detected:\n{}\n\n\
         Auto-heal actions taken:\n{}\n\n\
         Pod status: {:?}, Last seen: {:?}, Current game: {:?}",
        pod.id,
        pod.number,
        pod.ip_address,
        sanitized_issues
            .iter()
            .map(|i| format!("  - {}", i))
            .collect::<Vec<_>>()
            .join("\n"),
        actions_desc,
        pod.status,
        pod.last_seen,
        pod.current_game,
    );

    // Search fleet KB for prior solutions matching these issues
    let kb_context = {
        let search_query = sanitized_issues.join(" ");
        match crate::fleet_kb::search_solutions(&state.db, &search_query, 3).await {
            Ok(solutions) if !solutions.is_empty() => {
                let entries: Vec<String> = solutions.iter().map(|s| {
                    format!("- [{}] {}: {} (confidence: {:.0}%, applied {} times)",
                        s.problem_key, s.root_cause,
                        serde_json::to_string(&s.fix_action).unwrap_or_default(),
                        s.confidence * 100.0, s.success_count)
                }).collect();
                format!("\n\nKNOWN SOLUTIONS FROM FLEET KB (apply if matching):\n{}", entries.join("\n"))
            }
            _ => String::new(),
        }
    };

    let messages = vec![
        json!({
            "role": "system",
            "content": "You are an expert Windows systems administrator and sim racing venue technician. \
                        Analyze the pod health issues below. Provide a brief root cause hypothesis \
                        and specific remediation steps. Focus on actionable fixes. Keep under 150 words."
        }),
        json!({
            "role": "user",
            "content": format!("{}{}", context, kb_context)
        }),
    ];

    match crate::ai::query_ai(
        &state.config.ai_debugger,
        &messages,
        Some(&state.db),
        Some("healer"),
    )
    .await
    {
        Ok((suggestion, model)) => {
            tracing::info!(
                "Pod healer AI suggestion for {} (via {}): {}",
                pod.id,
                model,
                suggestion.chars().take(100).collect::<String>()
            );

            let debug_suggestion = AiDebugSuggestion {
                pod_id: pod.id.clone(),
                sim_type: pod.current_game.unwrap_or(SimType::AssettoCorsa),
                error_context: context,
                suggestion,
                model,
                created_at: Utc::now(),
                launch_epoch: 0,
            };

            // Persist to DB
            let id = uuid::Uuid::new_v4().to_string();
            let _ = sqlx::query(
                "INSERT INTO ai_suggestions (id, pod_id, sim_type, error_context, suggestion, model, source) \
                 VALUES (?, ?, ?, ?, ?, ?, 'healer')",
            )
            .bind(&id)
            .bind(&debug_suggestion.pod_id)
            .bind(
                serde_json::to_string(&debug_suggestion.sim_type)
                    .unwrap_or_default()
                    .trim_matches('"'),
            )
            .bind(&debug_suggestion.error_context)
            .bind(&debug_suggestion.suggestion)
            .bind(&debug_suggestion.model)
            .execute(&state.db)
            .await;

            // Broadcast to dashboard
            let _ = state
                .dashboard_tx
                .send(DashboardEvent::AiDebugSuggestion(debug_suggestion.clone()));

            // Phase 140: Parse AI suggestion for whitelisted actions and log audit trail.
            if let Some(action_name) = parse_ai_action_server(&debug_suggestion.suggestion) {
                let detail = format!("AI recommended action model={}", debug_suggestion.model);
                log_pod_activity(
                    state,
                    &debug_suggestion.pod_id,
                    "ai_action",
                    action_name,
                    &detail,
                    "ai_debugger",
                    None,
                );
                tracing::info!(
                    "Pod healer: AI action parsed for {} — {} ({})",
                    debug_suggestion.pod_id,
                    action_name,
                    debug_suggestion.model
                );
            }
        }
        Err(e) => {
            tracing::warn!("Pod healer AI escalation failed for {}: {}", pod.id, e);
        }
    }
}

// ─── Phase 140-02: Server-side AI action parsing ─────────────────────────────

/// Parse a whitelisted AI action from a free-text LLM suggestion.
///
/// Mirrors the rc-agent parse_ai_action() logic but returns &'static str
/// instead of the rc-agent enum type, avoiding a cross-crate dependency.
///
/// Returns None if no parseable JSON block with a whitelisted action is found.
/// No .unwrap() — all parse errors return None.
pub(crate) fn parse_ai_action_server(suggestion: &str) -> Option<&'static str> {
    #[derive(serde::Deserialize)]
    struct ActionBlock {
        action: String,
    }

    let bytes = suggestion.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'{'
            && let Some(end) = suggestion[i..].find('}') {
                let candidate = &suggestion[i..=i + end];
                if let Ok(block) = serde_json::from_str::<ActionBlock>(candidate) {
                    let action = match block.action.as_str() {
                        "kill_edge" => Some("kill_edge"),
                        "relaunch_lock_screen" => Some("relaunch_lock_screen"),
                        "restart_rcagent" => Some("restart_rcagent"),
                        "kill_game" => Some("kill_game"),
                        "clear_temp" => Some("clear_temp"),
                        _ => None,
                    };
                    if action.is_some() {
                        return action;
                    }
                }
            }
        i += 1;
    }
    None
}

// ─── Phase 141: WARN Log Scanner ─────────────────────────────────────────────

/// Constants for WARN log scanning.
const WARN_SCAN_WINDOW_SECS: i64 = 300;   // 5-minute rolling window
const WARN_THRESHOLD: usize = 50;          // trigger AI escalation above this
const WARN_COOLDOWN_SECS: i64 = 600;       // 10-minute cooldown between escalations

/// Scan the current racecontrol JSONL log for WARN entries in the last 5 minutes.
///
/// Returns (warn_count, raw_warn_lines) where raw_warn_lines are the matching log
/// lines (used by plan 02 for deduplication). Returns (0, vec![]) on any I/O error
/// so the healer cycle is never interrupted by log read failures.
///
/// No .unwrap() — all errors return the default empty result.
pub(crate) async fn scan_warn_logs(state: &Arc<AppState>) {
    let now = Utc::now();

    // Build path: logs/racecontrol-YYYY-MM-DD.jsonl (relative to server CWD)
    let date_str = now.format("%Y-%m-%d").to_string();
    let log_path = format!("logs/racecontrol-{}.jsonl", date_str);

    let contents = match tokio::fs::read_to_string(&log_path).await {
        Ok(c) => c,
        Err(e) => {
            tracing::debug!("WARN scanner: could not read log {}: {}", log_path, e);
            return;
        }
    };

    let cutoff = now - chrono::Duration::seconds(WARN_SCAN_WINDOW_SECS);

    // Count WARN lines within the rolling window
    let warn_lines: Vec<String> = contents
        .lines()
        .filter(|line| {
            // Fast pre-filter: must contain "WARN" string
            if !line.contains("\"WARN\"") {
                return false;
            }
            // Parse timestamp to check rolling window
            if let Ok(entry) = serde_json::from_str::<serde_json::Value>(line)
                && let Some(ts_str) = entry.get("timestamp").and_then(|v| v.as_str())
                    && let Ok(ts) = chrono::DateTime::parse_from_rfc3339(ts_str) {
                        return ts.with_timezone(&Utc) >= cutoff;
                    }
            false
        })
        .map(|s| s.to_string())
        .collect();

    let warn_count = warn_lines.len();

    if warn_count == 0 {
        tracing::debug!("WARN scanner: {} WARNs in last 5min (below threshold)", warn_count);
        return;
    }

    tracing::info!("WARN scanner: {} WARNs in last 5min (threshold: {})", warn_count, WARN_THRESHOLD);

    if warn_count <= WARN_THRESHOLD {
        return;
    }

    // Threshold breached — check cooldown before escalating
    {
        let last = state.warn_scanner_last_escalated.read().await;
        if let Some(last_time) = *last {
            let elapsed = (now - last_time).num_seconds();
            if elapsed < WARN_COOLDOWN_SECS {
                tracing::debug!(
                    "WARN scanner: threshold breached ({} WARNs) but cooldown active ({}s remaining)",
                    warn_count,
                    WARN_COOLDOWN_SECS - elapsed
                );
                return;
            }
        }
    }

    // Update cooldown timestamp
    {
        let mut last = state.warn_scanner_last_escalated.write().await;
        *last = Some(now);
    }

    tracing::warn!(
        "WARN scanner: ESCALATING — {} WARNs in 5min exceeds threshold of {}",
        warn_count,
        WARN_THRESHOLD
    );

    escalate_warn_surge(state, warn_count, warn_lines).await;
}

/// Deduplicate warn_lines and escalate to AI with a grouped summary.
///
/// Groups identical message strings, counts occurrences, and builds a compact
/// context prompt. Caps at 20 unique messages to keep the prompt under token limits.
/// Uses the same query_ai() path as escalate_to_ai() so results land in ai_suggestions.
///
/// No .unwrap() — all parse errors skip silently; the message field falls back to the raw line.
async fn escalate_warn_surge(
    state: &Arc<AppState>,
    total_warn_count: usize,
    warn_lines: Vec<String>,
) {
    // Deduplicate: extract fields.message, count occurrences
    let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for line in &warn_lines {
        let message = if let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) {
            entry
                .get("fields")
                .and_then(|f| f.get("message"))
                .and_then(|m| m.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| line.chars().take(120).collect())
        } else {
            line.chars().take(120).collect()
        };
        *counts.entry(message).or_insert(0) += 1;
    }

    // Sort by frequency descending, cap at 20 unique messages
    let mut sorted: Vec<(String, usize)> = counts.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));
    sorted.truncate(20);

    let grouped_text = sorted
        .iter()
        .map(|(msg, count)| {
            if *count > 1 {
                format!("  [x{}] {}", count, msg)
            } else {
                format!("  {}", msg)
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    let context = format!(
        "RACECONTROL SERVER WARN SURGE\n\n\
         Total WARNs in last 5 minutes: {}\n\
         Unique message types: {}\n\n\
         Top WARN messages (grouped by frequency):\n{}\n\n\
         Threshold: {} WARNs/5min",
        total_warn_count,
        sorted.len(),
        grouped_text,
        WARN_THRESHOLD,
    );

    let messages = vec![
        serde_json::json!({
            "role": "system",
            "content": "You are an expert Rust/Axum server diagnostician for a sim racing venue management system. \
                        Analyze the WARN log surge below. Identify the most likely root cause from the message patterns. \
                        Suggest one concrete investigation step. Keep under 120 words."
        }),
        serde_json::json!({
            "role": "user",
            "content": context
        }),
    ];

    match crate::ai::query_ai(
        &state.config.ai_debugger,
        &messages,
        Some(&state.db),
        Some("warn_scanner"),
    )
    .await
    {
        Ok((suggestion, model)) => {
            tracing::info!(
                "WARN scanner AI suggestion (via {}): {}",
                model,
                suggestion.chars().take(150).collect::<String>()
            );
            // Persist to ai_suggestions as a server-level event (no pod_id)
            let id = uuid::Uuid::new_v4().to_string();
            let _ = sqlx::query(
                "INSERT INTO ai_suggestions (id, pod_id, sim_type, error_context, suggestion, model, source) \
                 VALUES (?, ?, ?, ?, ?, ?, 'warn_scanner')",
            )
            .bind(&id)
            .bind("server")
            .bind("server")
            .bind(&context)
            .bind(&suggestion)
            .bind(&model)
            .execute(&state.db)
            .await;
        }
        Err(e) => {
            tracing::warn!("WARN scanner AI escalation failed: {}", e);
        }
    }
}
