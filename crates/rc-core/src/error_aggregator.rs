//! Proactive error pattern detection.
//!
//! Runs on a background interval, detects crash patterns, billing anomalies,
//! pod health issues, and API error spikes. Feeds them to the AI for analysis.
//! Broadcasts results as DashboardEvent::AiDebugSuggestion.

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
    let mut all_issues: Vec<String> = Vec::new();

    // ─── Pattern 1: Game crash patterns ──────────────────────────────────────
    if let Some(crash_desc) = check_game_crashes(state).await? {
        all_issues.push(crash_desc);
    }

    // ─── Pattern 2: Billing anomalies ────────────────────────────────────────
    if let Some(billing_desc) = check_billing_anomalies(state).await? {
        all_issues.push(billing_desc);
    }

    // ─── Pattern 3: Pod health ───────────────────────────────────────────────
    if let Some(pod_desc) = check_pod_health(state).await? {
        all_issues.push(pod_desc);
    }

    // ─── Pattern 4: API error spikes ─────────────────────────────────────────
    if let Some(api_desc) = check_api_errors(state) {
        all_issues.push(api_desc);
    }

    // If no patterns detected, skip AI call
    if all_issues.is_empty() {
        return Ok(());
    }

    let pattern_desc = format!(
        "OPERATIONS ALERT — detected in the last check cycle:\n\n{}",
        all_issues.join("\n\n")
    );

    tracing::info!("Error aggregator detected patterns:\n{}", pattern_desc);

    // Ask AI for analysis
    let messages = vec![
        json!({
            "role": "system",
            "content": "You are an expert sim racing venue technician and operations analyst. \
                        Analyze the operational issues below and provide a brief root cause analysis \
                        with actionable recommendations. Prioritize by severity. \
                        Keep response under 200 words."
        }),
        json!({
            "role": "user",
            "content": pattern_desc.clone()
        }),
    ];

    match crate::ai::query_ai(&state.config.ai_debugger, &messages).await {
        Ok((suggestion, model)) => {
            let debug_suggestion = AiDebugSuggestion {
                pod_id: "venue".to_string(),
                sim_type: SimType::AssettoCorsa,
                error_context: pattern_desc,
                suggestion,
                model,
                created_at: Utc::now(),
            };

            // Persist to DB
            let id = uuid::Uuid::new_v4().to_string();
            let _ = sqlx::query(
                "INSERT INTO ai_suggestions (id, pod_id, sim_type, error_context, suggestion, model, source) \
                 VALUES (?, ?, ?, ?, ?, ?, 'aggregator')"
            )
            .bind(&id)
            .bind(&debug_suggestion.pod_id)
            .bind(serde_json::to_string(&debug_suggestion.sim_type).unwrap_or_default().trim_matches('"'))
            .bind(&debug_suggestion.error_context)
            .bind(&debug_suggestion.suggestion)
            .bind(&debug_suggestion.model)
            .execute(&state.db)
            .await;

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

/// Check for game crash patterns (original logic).
async fn check_game_crashes(state: &Arc<AppState>) -> anyhow::Result<Option<String>> {
    let db = &state.db;

    // Same pod crashing 3+ times in the last hour
    let pod_crashes = sqlx::query_as::<_, (String, i64)>(
        "SELECT pod_id, COUNT(*) as cnt FROM game_launch_events \
         WHERE event_type = 'crash' AND created_at > datetime('now', '-1 hour') \
         GROUP BY pod_id HAVING cnt >= 3",
    )
    .fetch_all(db)
    .await?;

    // Same sim crashing across multiple pods
    let sim_crashes = sqlx::query_as::<_, (String, i64)>(
        "SELECT sim_type, COUNT(DISTINCT pod_id) as pod_count FROM game_launch_events \
         WHERE event_type = 'crash' AND created_at > datetime('now', '-1 hour') \
         GROUP BY sim_type HAVING pod_count >= 2",
    )
    .fetch_all(db)
    .await?;

    // Overall crash rate spike
    let total_crashes: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM game_launch_events \
         WHERE event_type = 'crash' AND created_at > datetime('now', '-1 hour')",
    )
    .fetch_one(db)
    .await
    .unwrap_or(0);

    if pod_crashes.is_empty() && sim_crashes.is_empty() && total_crashes < 5 {
        return Ok(None);
    }

    let mut desc = String::from("GAME CRASHES:\n");

    for (pod_id, count) in &pod_crashes {
        let errors = sqlx::query_as::<_, (String, Option<String>)>(
            "SELECT sim_type, error_message FROM game_launch_events \
             WHERE event_type = 'crash' AND pod_id = ? AND created_at > datetime('now', '-1 hour') \
             ORDER BY created_at DESC LIMIT 3",
        )
        .bind(pod_id)
        .fetch_all(db)
        .await
        .unwrap_or_default();

        desc.push_str(&format!("  Pod {} crashed {} times:\n", pod_id, count));
        for (sim, err) in &errors {
            desc.push_str(&format!(
                "    - {} ({})\n",
                sim,
                err.as_deref().unwrap_or("no details")
            ));
        }
    }

    for (sim, pod_count) in &sim_crashes {
        desc.push_str(&format!(
            "  {} crashing across {} different pods\n",
            sim, pod_count
        ));
    }

    if total_crashes >= 5 {
        desc.push_str(&format!("  Total crash count: {} (elevated)\n", total_crashes));
    }

    Ok(Some(desc))
}

/// Check for billing anomalies: stuck sessions, failed starts.
async fn check_billing_anomalies(state: &Arc<AppState>) -> anyhow::Result<Option<String>> {
    let db = &state.db;
    let mut issues = Vec::new();

    // Sessions stuck in 'pending' for more than 60 seconds (should have started)
    let stuck_pending: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT id, pod_id, created_at FROM billing_sessions \
         WHERE status = 'pending' AND created_at < datetime('now', '-60 seconds') \
         AND created_at > datetime('now', '-1 hour')",
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();

    if !stuck_pending.is_empty() {
        issues.push(format!(
            "  {} billing session(s) stuck in 'pending' (never started):",
            stuck_pending.len()
        ));
        for (id, pod_id, created) in &stuck_pending {
            issues.push(format!("    - Session {} on pod {} (created {})", id, pod_id, created));
        }
    }

    // Sessions where allocated time expired but status is still 'active' (stale billing)
    let stale_active: Vec<(String, String, i64, String)> = sqlx::query_as(
        "SELECT id, pod_id, allocated_seconds, started_at FROM billing_sessions \
         WHERE status = 'active' \
         AND datetime(started_at, '+' || allocated_seconds || ' seconds') < datetime('now', '-30 seconds')",
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();

    if !stale_active.is_empty() {
        issues.push(format!(
            "  {} billing session(s) still 'active' past their allocated time (timer may have failed):",
            stale_active.len()
        ));
        for (id, pod_id, secs, started) in &stale_active {
            issues.push(format!(
                "    - Session {} on pod {} ({}s allocated, started {})",
                id, pod_id, secs, started
            ));
        }
    }

    if issues.is_empty() {
        Ok(None)
    } else {
        Ok(Some(format!("BILLING ANOMALIES:\n{}", issues.join("\n"))))
    }
}

/// Check pod connectivity health.
async fn check_pod_health(state: &Arc<AppState>) -> anyhow::Result<Option<String>> {
    let pods = state.pods.read().await;
    let expected_pods = state.config.pods.count;
    let connected = pods.len() as u32;

    if connected >= expected_pods || expected_pods == 0 {
        return Ok(None);
    }

    let offline_count = expected_pods.saturating_sub(connected);
    // Only alert if more than half the pods are offline
    if offline_count < expected_pods / 2 && offline_count < 3 {
        return Ok(None);
    }

    let connected_names: Vec<String> = pods.values().map(|p| format!("Pod #{}", p.number)).collect();

    Ok(Some(format!(
        "POD CONNECTIVITY:\n  {}/{} pods offline. Connected: {}",
        offline_count,
        expected_pods,
        if connected_names.is_empty() {
            "none".to_string()
        } else {
            connected_names.join(", ")
        }
    )))
}

/// Check API error counters for spikes (>5 errors on any endpoint in the last cycle).
fn check_api_errors(state: &Arc<AppState>) -> Option<String> {
    let counts = state.drain_api_error_counts();
    let high_error_endpoints: Vec<(&String, &u32)> =
        counts.iter().filter(|(_, v)| **v >= 5).collect();

    if high_error_endpoints.is_empty() {
        return None;
    }

    let mut desc = String::from("API ERROR SPIKES (last 5 minutes):\n");
    for (endpoint, count) in &high_error_endpoints {
        desc.push_str(&format!("  {} — {} errors\n", endpoint, count));
    }
    Some(desc)
}
