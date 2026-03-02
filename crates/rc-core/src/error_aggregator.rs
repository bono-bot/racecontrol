//! Proactive error pattern detection.
//!
//! Runs on a background interval, detects crash patterns, and feeds them
//! to the AI for analysis. Broadcasts results as DashboardEvent::AiDebugSuggestion.

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use serde_json::json;

use rc_common::protocol::DashboardEvent;
use rc_common::types::{AiDebugSuggestion, SimType};

use crate::state::AppState;

/// Spawn the error aggregator loop. Runs every 5 minutes.
pub fn spawn(state: Arc<AppState>) {
    if !state.config.ai_debugger.enabled || !state.config.ai_debugger.proactive_analysis {
        tracing::info!("Proactive AI analysis disabled, skipping error aggregator");
        return;
    }

    tokio::spawn(async move {
        // Wait 60s before first run to let the system stabilize
        tokio::time::sleep(Duration::from_secs(60)).await;

        let mut interval = tokio::time::interval(Duration::from_secs(300));
        loop {
            interval.tick().await;
            if let Err(e) = detect_patterns(&state).await {
                tracing::warn!("Error aggregator failed: {}", e);
            }
        }
    });
}

/// Detect crash patterns in the last hour and ask AI for analysis.
async fn detect_patterns(state: &Arc<AppState>) -> anyhow::Result<()> {
    let db = &state.db;

    // Pattern 1: Same pod crashing 3+ times in the last hour
    let pod_crashes = sqlx::query_as::<_, (String, i64)>(
        "SELECT pod_id, COUNT(*) as cnt FROM game_launch_events \
         WHERE event_type = 'crash' AND created_at > datetime('now', '-1 hour') \
         GROUP BY pod_id HAVING cnt >= 3",
    )
    .fetch_all(db)
    .await?;

    // Pattern 2: Same sim crashing across multiple pods in the last hour
    let sim_crashes = sqlx::query_as::<_, (String, i64)>(
        "SELECT sim_type, COUNT(DISTINCT pod_id) as pod_count FROM game_launch_events \
         WHERE event_type = 'crash' AND created_at > datetime('now', '-1 hour') \
         GROUP BY sim_type HAVING pod_count >= 2",
    )
    .fetch_all(db)
    .await?;

    // Pattern 3: Overall crash rate spike (more than 5 crashes in last hour)
    let total_crashes: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM game_launch_events \
         WHERE event_type = 'crash' AND created_at > datetime('now', '-1 hour')",
    )
    .fetch_one(db)
    .await
    .unwrap_or(0);

    // If no patterns detected, skip AI call
    if pod_crashes.is_empty() && sim_crashes.is_empty() && total_crashes < 5 {
        return Ok(());
    }

    // Build a context string describing the patterns
    let mut pattern_desc = String::from("CRASH PATTERN ALERT — detected in the last hour:\n");

    for (pod_id, count) in &pod_crashes {
        // Get recent error messages for this pod
        let errors = sqlx::query_as::<_, (String, Option<String>)>(
            "SELECT sim_type, error_message FROM game_launch_events \
             WHERE event_type = 'crash' AND pod_id = ? AND created_at > datetime('now', '-1 hour') \
             ORDER BY created_at DESC LIMIT 3",
        )
        .bind(pod_id)
        .fetch_all(db)
        .await
        .unwrap_or_default();

        pattern_desc.push_str(&format!("Pod {} crashed {} times:\n", pod_id, count));
        for (sim, err) in &errors {
            pattern_desc.push_str(&format!(
                "  - {} ({})\n",
                sim,
                err.as_deref().unwrap_or("no details")
            ));
        }
    }

    for (sim, pod_count) in &sim_crashes {
        pattern_desc.push_str(&format!(
            "{} crashing across {} different pods\n",
            sim, pod_count
        ));
    }

    if total_crashes >= 5 {
        pattern_desc.push_str(&format!("Total crash count: {} (elevated)\n", total_crashes));
    }

    tracing::info!("Error aggregator detected patterns:\n{}", pattern_desc);

    // Ask AI for analysis
    let messages = vec![
        json!({
            "role": "system",
            "content": "You are an expert sim racing venue technician. Analyze the crash patterns below \
                        and provide a brief root cause analysis with actionable recommendations. \
                        Keep response under 150 words."
        }),
        json!({
            "role": "user",
            "content": pattern_desc.clone()
        }),
    ];

    // Determine the primary sim type from crashes (deserialize from DB string)
    let primary_sim: SimType = sim_crashes
        .first()
        .and_then(|(s, _)| serde_json::from_value(json!(s)).ok())
        .unwrap_or(SimType::AssettocCorsa);

    match crate::ai::query_ai(&state.config.ai_debugger, &messages).await {
        Ok((suggestion, model)) => {
            let debug_suggestion = AiDebugSuggestion {
                pod_id: pod_crashes
                    .first()
                    .map(|(id, _)| id.clone())
                    .unwrap_or_else(|| "venue".to_string()),
                sim_type: primary_sim,
                error_context: pattern_desc,
                suggestion,
                model,
                created_at: Utc::now(),
            };

            // Broadcast to dashboard
            let _ = state
                .dashboard_tx
                .send(DashboardEvent::AiDebugSuggestion(debug_suggestion));
        }
        Err(e) => {
            tracing::warn!("Error aggregator AI query failed: {}", e);
        }
    }

    Ok(())
}
