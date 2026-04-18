//! Billing session recovery and orphan detection.
//!
//! Extracted from billing.rs (Phase 385, v49.0 Architecture Completion).
//! Contains recover_active_sessions (server startup), detect_orphaned_sessions_on_startup,
//! and detect_orphaned_sessions_background (periodic cleanup of stuck sessions).

use std::collections::HashSet;
use std::sync::Arc;

use chrono::{DateTime, Utc};

use rc_common::protocol::{CoreMessage, CoreToAgentMessage, DashboardEvent};
use rc_common::types::{BillingSessionStatus, DrivingState};

use crate::activity_log::log_pod_activity;
use crate::billing::{BillingTimer, PauseReason};
use crate::state::AppState;
use crate::whatsapp_alerter;

// ─── Session Recovery ───────────────────────────────────────────────────────

/// On server startup, recover any active billing sessions from the database
pub async fn recover_active_sessions(state: &Arc<AppState>) -> anyhow::Result<()> {
    // FSM-09: Use COALESCE(bs.elapsed_seconds, bs.driving_seconds) so that after a restart,
    // the count-up timer resumes from the persisted elapsed_seconds (which may differ from
    // driving_seconds when pauses were involved).
    let rows = sqlx::query_as::<_, (String, String, String, String, String, i64, i64, String, Option<String>, Option<i64>, Option<i64>, Option<i64>)>(
        "SELECT bs.id, bs.driver_id, d.name, bs.pod_id, pt.name, bs.allocated_seconds, bs.driving_seconds, bs.status, bs.started_at, bs.split_count, bs.split_duration_minutes, COALESCE(bs.elapsed_seconds, bs.driving_seconds) as elapsed_seconds
         FROM billing_sessions bs
         JOIN drivers d ON bs.driver_id = d.id
         JOIN pricing_tiers pt ON bs.pricing_tier_id = pt.id
         WHERE bs.status IN ('active', 'paused_manual', 'paused_disconnect', 'paused_crash_recovery')",
    )
    .fetch_all(&state.db)
    .await?;

    if rows.is_empty() {
        return Ok(());
    }

    let mut timers = state.billing.active_timers.write().await;
    for row in &rows {
        let status = match row.7.as_str() {
            "active" => BillingSessionStatus::Active,
            "paused_manual" => BillingSessionStatus::PausedManual,
            "paused_disconnect" => BillingSessionStatus::PausedDisconnect,
            "paused_crash_recovery" => BillingSessionStatus::PausedCrashRecovery,
            _ => continue,
        };

        let started_at = row
            .8
            .as_ref()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc));

        let driving_secs = row.6 as u32;
        let allocated_secs = row.5 as u32;
        // FSM-09: Recover elapsed_seconds from DB (row.11 = COALESCE result).
        // Falls back to driving_seconds if elapsed_seconds column is NULL (old sessions).
        let elapsed_secs = row.11.unwrap_or(row.6) as u32;
        let timer = BillingTimer {
            session_id: row.0.clone(),
            driver_id: row.1.clone(),
            driver_name: row.2.clone(),
            pod_id: row.3.clone(),
            pricing_tier_name: row.4.clone(),
            allocated_seconds: allocated_secs,
            driving_seconds: driving_secs,
            status,
            driving_state: DrivingState::Idle, // Will be updated when agent reconnects
            started_at,
            warning_5min_sent: allocated_secs.saturating_sub(elapsed_secs) <= 300,
            warning_1min_sent: allocated_secs.saturating_sub(elapsed_secs) <= 60,
            offline_since: None,
            split_count: row.9.unwrap_or(1) as u32,
            split_duration_minutes: row.10.map(|m| m as u32),
            current_split_number: 1, // Best guess on recovery — exact value non-critical
            pause_count: 0,
            total_paused_seconds: 0,
            last_paused_at: None,
            max_pause_duration_secs: 600,
            elapsed_seconds: elapsed_secs,
            pause_seconds: 0,
            max_session_seconds: allocated_secs,
            sim_type: None,
            recovery_pause_seconds: 0,
            pause_reason: PauseReason::None,
            nonce: String::new(),
            // Per-minute fields — defaults for recovery
            billing_mode: "per_minute".to_string(),
            rate_paise_per_minute: 2500,
            hold_paise: 10000,
            total_debited_paise: 0,
            seconds_since_last_debit: 0,
            wallet_owner_id: row.1.clone(), // default to driver_id
            low_balance_warning_paise: 5000,
            low_balance_warned: false,
            // GLD-C-02: Coverage histogram is lost on crash/restart — starts empty on recovery.
            // Session that was running before restart will have NULL telemetry_coverage_pct (D-05).
            telemetry_seconds_covered: std::collections::HashSet::new(),
            // GLD-C-04: Grace window fields — left as None here. hydrate_grace_fields_from_db
            // runs AFTER recover and patches these from the DB if a grace window was pending.
            // P0-3 fix: original code cleared these explicitly, which clobbered the hydration.
            lap_reject_grace_until: None,
            pending_end_status: None,
            // Phase 414: Idle counter is in-memory only — reset to 0 on recovery (customer-favourable per D-CLOUD-SYNC).
            between_games_idle_seconds: 0,
            idle_warning_sent: false,
            idle_auto_end_queued: false,
        };

        tracing::info!(
            "Recovered billing session {} for driver {} on pod {} ({}/{}s)",
            timer.session_id,
            timer.driver_name,
            timer.pod_id,
            timer.driving_seconds,
            timer.allocated_seconds
        );

        // Update pod state to reflect the active session
        {
            let mut pods = state.pods.write().await;
            if let Some(pod) = pods.get_mut(&timer.pod_id) {
                pod.billing_session_id = Some(timer.session_id.clone());
                pod.current_driver = Some(timer.driver_name.clone());
                pod.status = rc_common::types::PodStatus::InSession;
            }
        }

        timers.insert(row.3.clone(), timer);
    }

    tracing::info!("Recovered {} active billing sessions", rows.len());
    Ok(())
}

// ─── Orphan Session Detection ────────────────────────────────────────────────

/// On server startup, detect billing sessions that were "active" in DB but have
/// a stale last_timer_sync_at (>5 minutes ago). These are sessions that were
/// running when the server crashed/restarted. Flag them and alert staff.
///
/// Called AFTER recover_active_sessions() — sessions already recovered into memory
/// are NOT orphans (they were properly persisted). This catches sessions where
/// last_timer_sync_at is NULL (never synced — server crashed before first 60s sync)
/// or older than 5 minutes.
///
/// FSM-10: Orphaned session detection on startup.
pub async fn detect_orphaned_sessions_on_startup(state: &Arc<AppState>) {
    let threshold_minutes = 5;

    // Find active sessions with stale or NULL last_timer_sync_at
    let orphans = sqlx::query_as::<_, (String, String, String, Option<String>, i64)>(
        "SELECT id, pod_id, driver_id, last_timer_sync_at, driving_seconds
         FROM billing_sessions
         WHERE status IN ('active', 'paused_manual', 'paused_disconnect')
         AND (last_timer_sync_at IS NULL
              OR last_timer_sync_at < datetime('now', ?))",
    )
    .bind(format!("-{} minutes", threshold_minutes))
    .fetch_all(&state.db)
    .await;

    match orphans {
        Ok(rows) if rows.is_empty() => {
            tracing::info!("Startup orphan check: no orphaned sessions found");
        }
        Ok(rows) => {
            let count = rows.len();
            tracing::error!(
                "STARTUP ORPHAN DETECTION: Found {} billing session(s) with no heartbeat for {}+ minutes",
                count, threshold_minutes
            );

            let mut details = Vec::new();
            for (session_id, pod_id, driver_id, last_sync, driving_secs) in &rows {
                let sync_info = last_sync.as_deref().unwrap_or("NEVER");
                tracing::error!(
                    "  Orphaned session: {} on {} (driver={}, last_sync={}, driving={}s)",
                    session_id, pod_id, driver_id, sync_info, driving_secs
                );
                details.push(format!("{} on {} ({}s)", session_id, pod_id, driving_secs));

                // Mark session with end_reason for audit trail
                let _ = sqlx::query(
                    "UPDATE billing_sessions SET end_reason = 'orphan_flagged_startup' WHERE id = ? AND end_reason IS NULL",
                )
                .bind(session_id)
                .execute(&state.db)
                .await;
            }

            // Send WhatsApp alert to staff
            let alert_msg = format!(
                "ORPHAN ALERT (startup): {} stale billing session(s) detected with no heartbeat for {}+ min. Sessions: {}. Check admin dashboard.",
                count, threshold_minutes, details.join(", ")
            );
            if state.config.alerting.enabled {
                whatsapp_alerter::send_whatsapp(&state.config, &alert_msg).await;
            }

            // Log to activity feed for dashboard visibility
            log_pod_activity(state, "server", "billing", "orphan_detection", &alert_msg, "startup", None);
        }
        Err(e) => {
            tracing::error!("Failed to check for orphaned sessions on startup: {}", e);
        }
    }
}

/// Background task: every 5 minutes, check for active billing sessions whose
/// last_timer_sync_at is older than 5 minutes. This catches sessions that became
/// orphaned while the server is running (e.g., agent disconnected, timer loop crashed).
///
/// RESIL-03: Background orphan detection job.
pub async fn detect_orphaned_sessions_background(state: &Arc<AppState>) {
    let threshold_minutes = 5;

    // Snapshot active session IDs from in-memory timers (sessions with active timer are NOT orphans).
    // Drop the lock before any DB query (standing rule: no lock across .await).
    let active_session_ids: HashSet<String> = {
        let timers = state.billing.active_timers.read().await;
        timers.values().map(|t| t.session_id.clone()).collect()
    };

    let db_active = sqlx::query_as::<_, (String, String, String, Option<String>, i64)>(
        "SELECT id, pod_id, driver_id, last_timer_sync_at, driving_seconds
         FROM billing_sessions
         WHERE status IN ('active', 'paused_manual', 'paused_disconnect')
         AND (last_timer_sync_at IS NULL
              OR last_timer_sync_at < datetime('now', ?))",
    )
    .bind(format!("-{} minutes", threshold_minutes))
    .fetch_all(&state.db)
    .await;

    match db_active {
        Ok(rows) => {
            // Filter to only sessions NOT in active_timers (true orphans)
            let orphans: Vec<_> = rows
                .into_iter()
                .filter(|(id, _, _, _, _)| !active_session_ids.contains(id))
                .collect();

            if orphans.is_empty() {
                tracing::debug!("Background orphan check: no orphaned sessions");
                return;
            }

            let count = orphans.len();
            tracing::error!(
                "BACKGROUND ORPHAN DETECTION: Found {} billing session(s) with stale heartbeat ({}+ min)",
                count, threshold_minutes
            );

            let mut details = Vec::new();
            for (session_id, pod_id, driver_id, last_sync, driving_secs) in &orphans {
                let sync_info = last_sync.as_deref().unwrap_or("NEVER");
                tracing::error!(
                    "  Orphaned session: {} on {} (driver={}, last_sync={}, driving={}s)",
                    session_id, pod_id, driver_id, sync_info, driving_secs
                );
                details.push(format!("{} on {} ({}s)", session_id, pod_id, driving_secs));

                // MMA-ITER1-NEW1: Auto-end zombie sessions (not just flag)
                // CAS guard prevents double-end if another path already finalized
                let cas = sqlx::query(
                    "UPDATE billing_sessions SET status = 'ended_early', end_reason = 'orphan_auto_ended_background', ended_at = datetime('now') WHERE id = ? AND status IN ('active', 'paused_manual', 'paused_game_pause', 'paused_disconnect', 'paused_crash_recovery')",
                )
                .bind(session_id)
                .execute(&state.db)
                .await;
                match cas {
                    Ok(r) if r.rows_affected() > 0 => {
                        tracing::error!("ORPHAN AUTO-END: session {} on {} auto-ended (was zombie for {}+ min)", session_id, pod_id, threshold_minutes);
                        // MMA-ITER2: Idempotent orphan refund — use session_id as idempotency key
                        // to prevent double-refund if two concurrent orphan detectors both trigger
                        let already_refunded = sqlx::query_as::<_, (i64,)>(
                            "SELECT COUNT(*) FROM wallet_transactions WHERE idempotency_key = ?",
                        )
                        .bind(format!("orphan_refund_{}", session_id))
                        .fetch_one(&state.db)
                        .await
                        .map(|r| r.0 > 0)
                        .unwrap_or(false);

                        if already_refunded {
                            tracing::warn!("ORPHAN REFUND SKIPPED: session {} already refunded (idempotency guard)", session_id);
                        } else {
                            let wallet_info = sqlx::query_as::<_, (String, i64, Option<i64>, Option<String>)>(
                                "SELECT driver_id, allocated_seconds, wallet_debit_paise, wallet_owner_id FROM billing_sessions WHERE id = ?",
                            )
                            .bind(session_id)
                            .fetch_optional(&state.db)
                            .await
                            .ok()
                            .flatten();
                            if let Some((driver_id, allocated, Some(debit), wallet_owner)) = wallet_info {
                                let refund_target = wallet_owner.as_deref().unwrap_or(&driver_id);
                                let refund = crate::billing::compute_refund(allocated, *driving_secs, debit);
                                if refund > 0 {
                                    match crate::wallet::credit(state, refund_target, refund, "refund_session", Some(session_id), Some("Orphan auto-end refund"), Some(&format!("orphan_refund_{}", session_id))).await {
                                        Ok(_) => tracing::info!("ORPHAN REFUND: {}p for session {}", refund, session_id),
                                        Err(e) => tracing::error!("ORPHAN REFUND FAILED: session {} ({}p): {}", session_id, refund, e),
                                    }
                                }
                            }
                        }
                        // Remove from in-memory timers if still present
                        let mut timers = state.billing.active_timers.write().await;
                        timers.retain(|_, t| t.session_id != *session_id);
                        drop(timers); // Release lock before async work

                        // Clear pod billing state + notify agent + broadcast dashboard
                        {
                            let mut pods = state.pods.write().await;
                            if let Some(pod) = pods.get_mut(pod_id.as_str()) {
                                pod.billing_session_id = None;
                                pod.current_driver = None;
                                pod.status = rc_common::types::PodStatus::Idle;
                                let _ = state.dashboard_tx.send(DashboardEvent::PodUpdate(pod.clone()));
                            }
                        }
                        // Notify agent: session ended (snapshot sender to avoid lock across .await)
                        let sender_clone = {
                            let agent_senders = state.agent_senders.read().await;
                            agent_senders.get(pod_id.as_str()).cloned()
                        };
                        if let Some(sender) = sender_clone {
                            // Stop the game first (matches normal end path behavior)
                            let _ = sender.send(CoreMessage::wrap(CoreToAgentMessage::StopGame)).await;
                            let _ = sender.send(CoreMessage::wrap(CoreToAgentMessage::SessionEnded {
                                billing_session_id: session_id.clone(),
                                driver_name: String::new(),
                                total_laps: 0,
                                best_lap_ms: None,
                                driving_seconds: *driving_secs as u32,
                            })).await;
                        }
                    }
                    _ => {
                        // Already finalized by another path — just flag
                        let _ = sqlx::query(
                            "UPDATE billing_sessions SET end_reason = 'orphan_flagged_background' WHERE id = ? AND end_reason IS NULL",
                        )
                        .bind(session_id)
                        .execute(&state.db)
                        .await;
                    }
                }
            }

            // Alert staff via WhatsApp
            let alert_msg = format!(
                "ORPHAN ALERT (background): {} stale billing session(s) — no heartbeat for {}+ min. Sessions: {}. Investigate immediately.",
                count, threshold_minutes, details.join(", ")
            );
            if state.config.alerting.enabled {
                whatsapp_alerter::send_whatsapp(&state.config, &alert_msg).await;
            }
            log_pod_activity(state, "server", "billing", "orphan_detection", &alert_msg, "background-job", None);
        }
        Err(e) => {
            tracing::error!("Background orphan detection query failed: {}", e);
        }
    }
}

