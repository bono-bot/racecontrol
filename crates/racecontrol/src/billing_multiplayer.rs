//! Billing multiplayer — synchronized pause/resume/stop + split session lifecycle.
//!
//! Extracted from billing.rs (Phase 385, v49.0 Architecture Completion).
//! BILL-07 (multiplayer pause/resume), MULTI-02 (group server stop),
//! FSM-07 (split session create/transition/cancel).

use std::sync::Arc;

use rc_common::pod_id::normalize_pod_id;
use rc_common::protocol::DashboardEvent;
use rc_common::types::BillingSessionStatus;

use crate::billing::PauseReason;
use crate::state::AppState;

// ─── BILL-07: Multiplayer Synchronized Billing Pause/Resume ─────────────────

/// BILL-07: Pause billing for ALL pods in a multiplayer group when one pod crashes.
///
/// Queries all group members from group_session_members, then sets each active
/// billing timer to PausedGamePause with CrashRecovery reason. Logs a
/// `multiplayer_group_paused` billing_event on each affected session for audit trail.
///
/// Broadcasts `MultiplayerGroupPaused` dashboard event so staff see the group pause.
pub async fn pause_multiplayer_group(
    state: &Arc<AppState>,
    group_session_id: &str,
    reason: &str,
) {
    // Query all pod_ids in this group
    let member_pods: Vec<(String,)> = match sqlx::query_as(
        "SELECT pod_id FROM group_session_members WHERE group_session_id = ? AND pod_id IS NOT NULL",
    )
    .bind(group_session_id)
    .fetch_all(&state.db)
    .await
    {
        Ok(rows) => rows,
        Err(e) => {
            tracing::error!("BILL-07: Failed to query group_session_members for group {}: {}", group_session_id, e);
            return;
        }
    };

    if member_pods.is_empty() {
        tracing::warn!("BILL-07: pause_multiplayer_group called for group {} but no members found", group_session_id);
        return;
    }

    let pod_ids: Vec<String> = member_pods.iter().map(|(p,)| p.clone()).collect();

    // Snapshot the timer map — do NOT hold lock across async DB calls (standing rule)
    let sessions_to_pause: Vec<(String, String)> = {
        let timers = state.billing.active_timers.read().await;
        pod_ids
            .iter()
            .filter_map(|pod_id| {
                timers.get(pod_id).map(|t| (pod_id.clone(), t.session_id.clone()))
            })
            .collect()
    }; // lock dropped here

    tracing::info!(
        "BILL-07: Pausing all {} pods in multiplayer group {} — reason: {}",
        sessions_to_pause.len(),
        group_session_id,
        reason
    );

    // Apply CrashRecovery pause to each pod's timer
    {
        let mut timers = state.billing.active_timers.write().await;
        for (pod_id, _) in &sessions_to_pause {
            if let Some(timer) = timers.get_mut(pod_id) {
                match crate::billing_fsm::validate_transition(timer.status, crate::billing_fsm::BillingEvent::CrashPause) {
                    Ok(new_status) => {
                        timer.status = new_status;
                        timer.pause_seconds = 0;
                        timer.pause_count += 1;
                        // BILL-07: CrashRecovery reason — pause time excluded from billable seconds
                        timer.pause_reason = PauseReason::CrashRecovery;
                        tracing::info!("BILL-07: Paused billing for pod {} in multiplayer group {}", pod_id, group_session_id);
                    }
                    Err(e) => {
                        tracing::warn!("BILL-07: Could not pause pod {} in group {}: {}", pod_id, group_session_id, e);
                    }
                }
            }
        }
    } // timers lock dropped

    // Log billing_events for each paused session (audit trail)
    for (pod_id, session_id) in &sessions_to_pause {
        let _ = sqlx::query(
            "INSERT INTO billing_events (id, billing_session_id, event_type, driving_seconds_at_event, metadata, venue_id)
             VALUES (?, ?, 'multiplayer_group_paused', 0, ?, ?)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(session_id)
        .bind(format!(
            "{{\"group_session_id\":\"{}\",\"reason\":\"{}\",\"pod_id\":\"{}\"}}",
            group_session_id, reason, pod_id
        ))
        .bind(&state.config.venue.venue_id)
        .execute(&state.db)
        .await
        .map_err(|e| tracing::warn!("BILL-07: Failed to log multiplayer_group_paused event for session {}: {}", session_id, e));
    }

    // Broadcast MultiplayerGroupPaused to dashboards
    let _ = state.dashboard_tx.send(DashboardEvent::MultiplayerGroupPaused {
        group_session_id: group_session_id.to_string(),
        pod_ids: pod_ids.clone(),
        reason: reason.to_string(),
    });
}

/// BILL-07: Resume billing for ALL pods in a multiplayer group after crash recovery.
///
/// Queries all group members, then resumes each timer that is in
/// PausedGamePause+CrashRecovery state. Logs `multiplayer_group_resumed`
/// billing_event on each resumed session for audit trail.
pub async fn resume_multiplayer_group(state: &Arc<AppState>, group_session_id: &str) {
    // Query all pod_ids in this group
    let member_pods: Vec<(String,)> = match sqlx::query_as(
        "SELECT pod_id FROM group_session_members WHERE group_session_id = ? AND pod_id IS NOT NULL",
    )
    .bind(group_session_id)
    .fetch_all(&state.db)
    .await
    {
        Ok(rows) => rows,
        Err(e) => {
            tracing::error!("BILL-07: Failed to query group_session_members for group {}: {}", group_session_id, e);
            return;
        }
    };

    if member_pods.is_empty() {
        tracing::warn!("BILL-07: resume_multiplayer_group called for group {} but no members found", group_session_id);
        return;
    }

    let pod_ids: Vec<String> = member_pods.iter().map(|(p,)| p.clone()).collect();

    // Snapshot timers eligible for resume — do NOT hold lock across async calls
    let sessions_to_resume: Vec<(String, String)> = {
        let timers = state.billing.active_timers.read().await;
        pod_ids
            .iter()
            .filter_map(|pod_id| {
                timers.get(pod_id).and_then(|t| {
                    // BILL-07: Only resume timers paused for CrashRecovery (not manual ESC pauses)
                    if t.status == BillingSessionStatus::PausedGamePause
                        && t.pause_reason == PauseReason::CrashRecovery
                    {
                        Some((pod_id.clone(), t.session_id.clone()))
                    } else {
                        None
                    }
                })
            })
            .collect()
    }; // lock dropped here

    tracing::info!(
        "BILL-07: Resuming all pods in multiplayer group {} ({} eligible)",
        group_session_id,
        sessions_to_resume.len()
    );

    // Apply resume to each eligible timer
    {
        let mut timers = state.billing.active_timers.write().await;
        for (pod_id, _) in &sessions_to_resume {
            if let Some(timer) = timers.get_mut(pod_id) {
                match crate::billing_fsm::validate_transition(timer.status, crate::billing_fsm::BillingEvent::Resume) {
                    Ok(new_status) => {
                        timer.status = new_status;
                        timer.pause_seconds = 0;
                        // BILL-07: Clear pause reason on resume
                        timer.pause_reason = PauseReason::None;
                        tracing::info!("BILL-07: Resumed billing for pod {} in multiplayer group {}", pod_id, group_session_id);
                    }
                    Err(e) => {
                        tracing::warn!("BILL-07: Could not resume pod {} in group {}: {}", pod_id, group_session_id, e);
                    }
                }
            }
        }
    } // timers lock dropped

    // Log billing_events for each resumed session (audit trail)
    for (pod_id, session_id) in &sessions_to_resume {
        let _ = sqlx::query(
            "INSERT INTO billing_events (id, billing_session_id, event_type, driving_seconds_at_event, metadata, venue_id)
             VALUES (?, ?, 'multiplayer_group_resumed', 0, ?, ?)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(session_id)
        .bind(format!(
            "{{\"group_session_id\":\"{}\",\"pod_id\":\"{}\"}}",
            group_session_id, pod_id
        ))
        .bind(&state.config.venue.venue_id)
        .execute(&state.db)
        .await
        .map_err(|e| tracing::warn!("BILL-07: Failed to log multiplayer_group_resumed event for session {}: {}", session_id, e));
    }
}

/// MULTI-02: Check if all pods in a multiplayer group session have ended billing.
/// If so, stop the AC server associated with the group.
/// Called after each billing end (both tick-expired and manual stop).
pub async fn check_and_stop_multiplayer_server(state: &Arc<AppState>, pod_id: &str) {
    // Normalize pod_id to canonical form (pod_N) at entry
    let pod_id_normalized = normalize_pod_id(pod_id).unwrap_or_else(|_| pod_id.to_string());
    let pod_id = pod_id_normalized.as_str();
    // Look up the group_session_id for this pod's billing session
    let group_info = sqlx::query_as::<_, (String, Option<String>)>(
        "SELECT gs.id, gs.ac_session_id
         FROM group_session_members gsm
         JOIN group_sessions gs ON gs.id = gsm.group_session_id
         WHERE gsm.pod_id = ? AND gs.status IN ('active', 'forming')
         ORDER BY gs.created_at DESC LIMIT 1",
    )
    .bind(pod_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let (group_session_id, ac_session_id) = match group_info {
        Some(info) => info,
        None => return, // Not a multiplayer pod
    };

    let ac_session_id = match ac_session_id {
        Some(id) => id,
        None => return, // No AC server for this group
    };

    // Get all pod_ids in this group session
    let member_pods: Vec<(String,)> = sqlx::query_as(
        "SELECT pod_id FROM group_session_members WHERE group_session_id = ? AND pod_id IS NOT NULL",
    )
    .bind(&group_session_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    // Check if any pod still has active billing
    let timers = state.billing.active_timers.read().await;
    let any_still_billing = member_pods.iter().any(|(mpod,)| timers.contains_key(mpod));
    drop(timers);

    if any_still_billing {
        tracing::debug!(
            "Multiplayer group {} still has active billing — AC server {} stays running",
            group_session_id, ac_session_id
        );
        return;
    }

    // GROUP-02: If continuous mode is enabled, defer stop to monitor_continuous_session loop.
    {
        let instances = state.ac_server.instances.read().await;
        if let Some(inst) = instances.get(&ac_session_id)
            && inst.continuous_mode {
                tracing::info!(
                    "Continuous mode active for group {} — deferring stop to monitor loop",
                    group_session_id
                );
                return;
            }
    }

    // All pods done — stop the AC server
    tracing::info!(
        "MULTI-02: All billing ended for multiplayer group {} — stopping AC server {}",
        group_session_id, ac_session_id
    );

    if let Err(e) = crate::ac_server::stop_ac_server(state, &ac_session_id).await {
        tracing::error!(
            "Failed to stop AC server {} for group {}: {}",
            ac_session_id, group_session_id, e
        );
    }

    // Update group session status to completed
    let _ = sqlx::query("UPDATE group_sessions SET status = 'completed' WHERE id = ?")
        .bind(&group_session_id)
        .execute(&state.db)
        .await;
}

// ─── FSM-07: Split Session Lifecycle ─────────────────────────────────────────

/// FSM-07: Create child split entitlements for a parent session.
///
/// Called when a session starts with split_count > 1.
/// Each split gets an equal share of total allocated_seconds; the last split
/// gets any remainder seconds to ensure the sum equals total_allocated_seconds.
///
/// Split 1 is immediately activated. Splits 2..N remain Pending.
pub async fn create_split_records(
    db: &sqlx::SqlitePool,
    parent_session_id: &str,
    split_count: u32,
    total_allocated_seconds: u32,
    venue_id: &str,
) -> Result<(), String> {
    if split_count == 0 {
        return Err("split_count must be >= 1".to_string());
    }
    let per_split = total_allocated_seconds / split_count;
    let remainder = total_allocated_seconds % split_count;

    for i in 1..=split_count {
        // Last split gets remainder seconds (ensures total adds up correctly)
        let alloc = if i == split_count { per_split + remainder } else { per_split };
        sqlx::query(
            "INSERT INTO split_sessions (parent_session_id, split_number, allocated_seconds, status, venue_id) \
             VALUES (?, ?, ?, 'pending', ?)",
        )
        .bind(parent_session_id)
        .bind(i as i64)
        .bind(alloc as i64)
        .bind(venue_id)
        .execute(db)
        .await
        .map_err(|e| format!("FSM-07: Failed to create split record {}: {}", i, e))?;
    }

    // Activate split 1 immediately (first split starts when session starts)
    sqlx::query(
        "UPDATE split_sessions SET status = 'active', started_at = datetime('now') \
         WHERE parent_session_id = ? AND split_number = 1 AND status = 'pending'",
    )
    .bind(parent_session_id)
    .execute(db)
    .await
    .map_err(|e| format!("FSM-07: Failed to activate split 1: {}", e))?;

    tracing::info!(
        "FSM-07: Created {} split records for session {} ({}s total, {}s per split)",
        split_count, parent_session_id, total_allocated_seconds, per_split
    );
    Ok(())
}

/// FSM-07: Get the next pending split for a session (for split transitions).
///
/// Returns (split_number, allocated_seconds) for the lowest-numbered pending split,
/// or None if all splits have been activated/completed.
pub async fn get_next_pending_split(
    db: &sqlx::SqlitePool,
    parent_session_id: &str,
) -> Result<Option<(i64, i64)>, String> {
    sqlx::query_as::<_, (i64, i64)>(
        "SELECT split_number, allocated_seconds FROM split_sessions \
         WHERE parent_session_id = ? AND status = 'pending' \
         ORDER BY split_number ASC LIMIT 1",
    )
    .bind(parent_session_id)
    .fetch_optional(db)
    .await
    .map_err(|e| format!("FSM-07: Failed to query next pending split: {}", e))
}

/// FSM-07: Complete the current active split and activate the next pending split.
///
/// Uses CAS (Compare-And-Swap): only completes a split if it is currently Active.
/// Returns the next split_number if one was activated, or None if all splits are done.
///
/// Returns Err if the CAS fails (split is not in Active state — concurrent transition guard).
pub async fn transition_split(
    db: &sqlx::SqlitePool,
    parent_session_id: &str,
    current_split_number: i64,
) -> Result<Option<i64>, String> {
    // CAS: only complete if the split is currently active
    let completed = sqlx::query(
        "UPDATE split_sessions \
         SET status = 'completed', ended_at = datetime('now') \
         WHERE parent_session_id = ? AND split_number = ? AND status = 'active'",
    )
    .bind(parent_session_id)
    .bind(current_split_number)
    .execute(db)
    .await
    .map_err(|e| format!("FSM-07: Failed to complete split {}: {}", current_split_number, e))?;

    if completed.rows_affected() == 0 {
        return Err(format!(
            "FSM-07: CAS failed — split {} for session {} is not in active state (concurrent transition guard)",
            current_split_number, parent_session_id
        ));
    }

    // Activate the next pending split
    let next = get_next_pending_split(db, parent_session_id).await?;
    if let Some((next_number, _)) = next {
        sqlx::query(
            "UPDATE split_sessions \
             SET status = 'active', started_at = datetime('now') \
             WHERE parent_session_id = ? AND split_number = ? AND status = 'pending'",
        )
        .bind(parent_session_id)
        .bind(next_number)
        .execute(db)
        .await
        .map_err(|e| format!("FSM-07: Failed to activate split {}: {}", next_number, e))?;

        tracing::info!(
            "FSM-07: Split {} completed, activated split {} for session {}",
            current_split_number, next_number, parent_session_id
        );
        Ok(Some(next_number))
    } else {
        tracing::info!(
            "FSM-07: Split {} completed — no more pending splits for session {}",
            current_split_number, parent_session_id
        );
        Ok(None) // No more splits — session is ready to end
    }
}

/// FSM-07: Cancel all pending splits for a session (called when parent session is cancelled).
///
/// Leaves Active and Completed splits unchanged — only Pending splits are cancelled.
pub async fn cancel_pending_splits(
    db: &sqlx::SqlitePool,
    parent_session_id: &str,
) -> Result<(), String> {
    sqlx::query(
        "UPDATE split_sessions SET status = 'cancelled' \
         WHERE parent_session_id = ? AND status = 'pending'",
    )
    .bind(parent_session_id)
    .execute(db)
    .await
    .map_err(|e| format!("FSM-07: Failed to cancel pending splits: {}", e))?;
    Ok(())
}
