use std::sync::Arc;
use tokio::sync::mpsc;

use rc_common::protocol::{
    AgentMessage, CoreMessage, CoreToAgentMessage, DashboardEvent,
    EvalRecordPayload, ReputationPayload,
};
use rc_common::types::{
    BillingSessionStatus, ContentManifest, ConfigAckPayload,
    FlagCacheSyncPayload, GameInventory, ComboValidationResult,
    LaunchTimeline,
};

use crate::activity_log::log_pod_activity;
use crate::state::AppState;

/// Handle AgentMessage::ContentManifest.
pub(crate) async fn handle_content_manifest(
    state: &Arc<AppState>,
    manifest: &ContentManifest,
    registered_pod_id: &Option<String>,
) {
    if let Some(pod_id) = registered_pod_id {
        let car_count = manifest.cars.len();
        let track_count = manifest.tracks.len();
        tracing::info!("Pod {} content manifest: {} cars, {} tracks", pod_id, car_count, track_count);
        log_pod_activity(state, pod_id, "content", "Content Scanned",
            &format!("{} cars, {} tracks", car_count, track_count), "agent", None);
        state.pod_manifests.write().await.insert(pod_id.clone(), manifest.clone());
    }
}

/// Handle AgentMessage::ExecResult.
pub(crate) async fn handle_exec_result(
    state: &Arc<AppState>,
    request_id: &str,
    success: bool,
    exit_code: Option<i32>,
    stdout: &str,
    stderr: &str,
) {
    tracing::info!("WS command result {}: success={}", request_id, success);
    let mut pending = state.pending_ws_execs.write().await;
    if let Some(sender) = pending.remove(request_id) {
        let _ = sender.send(crate::state::WsExecResult {
            success,
            exit_code,
            stdout: stdout.to_string(),
            stderr: stderr.to_string(),
        });
    } else {
        tracing::warn!("No pending request for request_id={}", request_id);
    }
}

/// Handle AgentMessage::CommandAck (Phase 312 WSCMD-01/02).
pub(crate) async fn handle_command_ack(
    state: &Arc<AppState>,
    command_id: &str,
    success: bool,
    error: &Option<String>,
    registered_pod_id: &Option<String>,
) {
    tracing::info!("CommandAck from pod {:?}: cmd={} success={} err={:?}",
        registered_pod_id, command_id, success, error);
    let mut pending = state.pending_command_acks.write().await;
    if let Some(sender) = pending.remove(command_id) {
        let _ = sender.send(crate::state::CommandAckResult {
            success,
            error: error.clone(),
        });
    } else {
        tracing::debug!("CommandAck for unknown command_id {} (already timed out?)", command_id);
    }
}

/// Handle AgentMessage::FlagCacheSync.
pub(crate) async fn handle_flag_cache_sync(
    state: &Arc<AppState>,
    payload: &FlagCacheSyncPayload,
    cmd_tx: &mpsc::Sender<CoreMessage>,
) {
    tracing::info!(
        "Pod {} requests flag sync (cached_version={})",
        payload.pod_id, payload.cached_version
    );
    let max_version = {
        let flags = state.feature_flags.read().await;
        flags.values().map(|f| f.version).max().unwrap_or(0)
    };
    if payload.cached_version < max_version as u64 {
        let flags = state.feature_flags.read().await;
        let pod_id = &payload.pod_id;
        let flag_map: std::collections::HashMap<String, bool> = flags
            .iter()
            .map(|(name, row)| {
                let effective = serde_json::from_str::<std::collections::HashMap<String, bool>>(&row.overrides)
                    .ok()
                    .and_then(|ovr| ovr.get(pod_id).copied())
                    .unwrap_or(row.enabled);
                (name.clone(), effective)
            })
            .collect();
        let sync_payload = rc_common::types::FlagSyncPayload {
            flags: flag_map,
            version: max_version as u64,
        };
        let _ = cmd_tx
            .send(CoreMessage::wrap(CoreToAgentMessage::FlagSync(sync_payload)))
            .await;
    }
    // Replay pending config pushes for this pod.
    crate::config_push::replay_pending_config_pushes(
        state,
        &payload.pod_id,
        cmd_tx,
    )
    .await;
}

/// Handle AgentMessage::ConfigAck.
pub(crate) async fn handle_config_ack(
    state: &Arc<AppState>,
    payload: &ConfigAckPayload,
) {
    tracing::info!(
        "Pod {} acked config push seq={} accepted={}",
        payload.pod_id, payload.sequence, payload.accepted
    );
    let ack_result = sqlx::query(
        "UPDATE config_push_queue SET status = 'acked', acked_at = datetime('now') \
         WHERE pod_id = ? AND seq_num = ?",
    )
    .bind(&payload.pod_id)
    .bind(payload.sequence as i64)
    .execute(&state.db)
    .await;
    if let Err(e) = ack_result {
        tracing::warn!(
            "Failed to update config_push_queue ack for pod {} seq {}: {}",
            payload.pod_id, payload.sequence, e
        );
    }
    let audit_lookup = sqlx::query_scalar::<_, i64>(
        "SELECT id FROM config_audit_log WHERE entity_type = 'config' AND seq_num = ?",
    )
    .bind(payload.sequence as i64)
    .fetch_optional(&state.db)
    .await;
    if let Ok(Some(audit_id)) = audit_lookup {
        let current_acked: String = sqlx::query_scalar(
            "SELECT pods_acked FROM config_audit_log WHERE id = ?",
        )
        .bind(audit_id)
        .fetch_one(&state.db)
        .await
        .unwrap_or_else(|_| "[]".to_string());
        let mut acked: Vec<String> =
            serde_json::from_str(&current_acked).unwrap_or_default();
        if !acked.contains(&payload.pod_id) {
            acked.push(payload.pod_id.clone());
        }
        let _ = sqlx::query(
            "UPDATE config_audit_log SET pods_acked = ? WHERE id = ?",
        )
        .bind(
            serde_json::to_string(&acked)
                .unwrap_or_else(|_| "[]".to_string()),
        )
        .bind(audit_id)
        .execute(&state.db)
        .await;
    } else {
        tracing::warn!(
            "No audit log entry found for config push seq={}",
            payload.sequence
        );
    }
}

/// Handle AgentMessage::SessionAutoEnded.
pub(crate) fn handle_session_auto_ended(
    state: &Arc<AppState>,
    pod_id: &str,
    billing_session_id: &str,
    reason: &str,
) {
    tracing::warn!(
        "[session-auto-end] Pod {} session {} auto-ended by agent: {}",
        pod_id, billing_session_id, reason
    );
    log_pod_activity(
        state, pod_id, "billing", "Session Auto-Ended",
        &format!("session={} reason={}", billing_session_id, reason),
        "rc-agent", None,
    );
}

/// Handle AgentMessage::BillingPaused (crash recovery).
pub(crate) async fn handle_billing_paused(
    state: &Arc<AppState>,
    pod_id: &str,
    billing_session_id: &str,
) {
    tracing::info!(
        "[billing] Pod {} session {} billing paused (crash recovery)",
        pod_id, billing_session_id
    );
    let timers = &state.billing.active_timers;
    let mut guard = timers.write().await;
    if let Some(timer) = guard.values_mut().find(|t| t.session_id == *billing_session_id) {
        if let Err(e) = crate::billing_fsm::validate_transition(
            timer.status,
            crate::billing_fsm::BillingEvent::CrashPause,
        ) {
            tracing::warn!("[billing] FSM rejected CrashPause for session {}: {}", billing_session_id, e);
        } else {
            timer.status = rc_common::types::BillingSessionStatus::PausedGamePause;
            timer.pause_reason = crate::billing::PauseReason::CrashRecovery;
            tracing::info!("[billing] Timer paused for session {} (FSM-04 crash recovery)", billing_session_id);
        }
    }
    drop(guard);
}

/// Handle AgentMessage::BillingResumed.
pub(crate) fn handle_billing_resumed(pod_id: &str, billing_session_id: &str) {
    tracing::info!(
        "[billing] Pod {} session {} billing resumed",
        pod_id, billing_session_id
    );
}

/// Handle AgentMessage::PreFlightPassed.
pub(crate) async fn handle_preflight_passed(
    state: &Arc<AppState>,
    pod_id: &str,
) {
    tracing::info!("Pod {} pre-flight checks passed", pod_id);
    log_pod_activity(state, pod_id, "system", "Pre-flight Passed", "All checks passed before session start", "agent", None);
    {
        let mut fleet = state.pod_fleet_health.write().await;
        let store = fleet.entry(pod_id.to_string()).or_default();
        store.in_maintenance = false;
        store.maintenance_failures.clear();
    }
}

/// Handle AgentMessage::PreFlightFailed.
pub(crate) async fn handle_preflight_failed(
    state: &Arc<AppState>,
    pod_id: &str,
    failures: &[String],
) {
    tracing::warn!("Pod {} pre-flight checks failed: {:?}", pod_id, failures);
    log_pod_activity(state, pod_id, "system", "Pre-flight Failed", &format!("Failures: {:?}", failures), "agent", None);
    {
        let mut fleet = state.pod_fleet_health.write().await;
        let store = fleet.entry(pod_id.to_string()).or_default();
        store.in_maintenance = true;
        store.maintenance_failures = failures.to_vec();
    }
}

/// Handle AgentMessage::SelfTestResult.
pub(crate) async fn handle_self_test_result(
    state: &Arc<AppState>,
    pod_id: &str,
    request_id: &str,
    report: &serde_json::Value,
) {
    tracing::info!(
        "[self-test] Pod {} returned self-test results for request_id={}",
        pod_id, request_id
    );
    let mut pending = state.pending_self_tests.write().await;
    if let Some((_pod_id, tx)) = pending.remove(request_id) {
        let _ = tx.send(report.clone());
    } else {
        tracing::warn!("[self-test] Received SelfTestResult for unknown request_id: {}", request_id);
    }
}

/// Handle AgentMessage::ProcessGuardStatus.
pub(crate) fn handle_process_guard_status(
    pod_id: &str,
    scan_count: u64,
    violation_count_total: u64,
    violation_count_last_scan: u32,
    last_scan_at: &str,
    guard_active: bool,
) {
    tracing::info!(
        "[guard] Status from {}: scans={}, violations_total={}, violations_last_scan={}, last_scan={}, active={}",
        pod_id, scan_count, violation_count_total, violation_count_last_scan, last_scan_at, guard_active
    );
}

/// Handle AgentMessage::IdleHealthFailed.
pub(crate) async fn handle_idle_health_failed(
    state: &Arc<AppState>,
    pod_id: &str,
    failures: &[String],
    consecutive_count: u32,
    timestamp: &str,
) {
    tracing::warn!(
        "Pod {} idle health failed: {:?} ({} consecutive, at {})",
        pod_id, failures, consecutive_count, timestamp
    );
    log_pod_activity(
        state, pod_id, "system", "Idle Health Failed",
        &format!("Failures: {:?}, consecutive: {}", failures, consecutive_count),
        "agent", None,
    );
    {
        let mut fleet = state.pod_fleet_health.write().await;
        let store = fleet.entry(pod_id.to_string()).or_default();
        store.idle_health_fail_count = consecutive_count;
        store.idle_health_failures = failures.to_vec();
    }
}

/// Handle AgentMessage::InactivityAlert (BILL-01).
pub(crate) async fn handle_inactivity_alert(
    state: &Arc<AppState>,
    pod_id: &str,
    idle_seconds: u64,
) {
    let (driver_name, session_id) = {
        let timers = state.billing.active_timers.read().await;
        if let Some(timer) = timers.get(pod_id) {
            (timer.driver_name.clone(), timer.session_id.clone())
        } else {
            (String::from("unknown"), String::from("unknown"))
        }
    };
    tracing::warn!(
        "BILL-01: Pod {} idle for {}s — staff alerted (driver={}, session={})",
        pod_id, idle_seconds, driver_name, session_id
    );
    log_pod_activity(
        state, pod_id, "billing", "Customer Idle",
        &format!("No input for {}s — staff alert sent", idle_seconds),
        "agent", None,
    );
    let _ = state.dashboard_tx.send(DashboardEvent::InactivityAlert {
        pod_id: pod_id.to_string(),
        idle_seconds,
        driver_name,
        session_id,
    });
}

/// Handle AgentMessage::ExtendedTelemetry (v29.0).
pub(crate) fn handle_extended_telemetry(
    state: &Arc<AppState>,
    pod_id: &str,
    agent_msg: &AgentMessage,
) {
    tracing::debug!(
        pod = %pod_id,
        "v29.0: Extended telemetry received"
    );
    crate::telemetry_store::store_extended_telemetry(state, pod_id, agent_msg);
}

/// Handle AgentMessage::ModelEvalSync (EVAL-03 / Phase 290).
pub(crate) async fn handle_model_eval_sync(
    state: &Arc<AppState>,
    pod_id: &str,
    records: &[EvalRecordPayload],
) {
    tracing::debug!(
        target: "racecontrol::ws",
        pod_id = %pod_id,
        record_count = records.len(),
        "EVAL-03: received model eval sync from pod"
    );
    for rec in records.iter() {
        if let Err(e) = crate::fleet_kb::insert_eval_record(&state.db, rec).await {
            tracing::warn!(
                target: "racecontrol::ws",
                error = %e,
                model_id = %rec.model_id,
                "EVAL-03: failed to insert eval record"
            );
        }
    }
}

/// Handle AgentMessage::ModelReputationSync (MREP-04 / Phase 292).
pub(crate) async fn handle_model_reputation_sync(
    state: &Arc<AppState>,
    pod_id: &str,
    rows: &[ReputationPayload],
) {
    tracing::debug!(
        target: "racecontrol::ws",
        pod_id = %pod_id,
        model_count = rows.len(),
        "MREP-04: received model reputation sync from pod"
    );
    for row in rows.iter() {
        if let Err(e) = crate::fleet_kb::upsert_reputation(&state.db, row, pod_id).await {
            tracing::warn!(
                target: "racecontrol::ws",
                error = %e,
                model_id = %row.model_id,
                "MREP-04: failed to upsert reputation row"
            );
        }
    }
}

/// Handle AgentMessage::EscalationRequest (Tier 5 WhatsApp escalation).
pub(crate) fn handle_escalation_request(
    state: &Arc<AppState>,
    payload: &rc_common::protocol::EscalationPayload,
) {
    tracing::warn!(
        target: "racecontrol::ws",
        pod_id = %payload.pod_id,
        incident_id = %payload.incident_id,
        severity = %payload.severity,
        "Received Tier 5 escalation from pod"
    );
    let escalation = state.whatsapp_escalation.clone();
    let payload_owned = payload.clone();
    tokio::spawn(async move {
        escalation.handle_escalation(payload_owned).await;
    });
}

/// Handle AgentMessage::ExperienceScoreReport (CX-06).
pub(crate) async fn handle_experience_score_report(
    state: &Arc<AppState>,
    pod_id: &str,
    total_score: f64,
    status: &str,
) {
    tracing::debug!(
        target: "racecontrol::ws",
        pod_id = %pod_id,
        score = total_score,
        status = %status,
        "Received experience score report from pod"
    );
    let mut fleet = state.pod_fleet_health.write().await;
    let store = fleet.entry(pod_id.to_string()).or_default();
    store.experience_score = Some(total_score);
    store.experience_status = Some(status.to_string());
}

/// Handle AgentMessage::GameInventoryUpdate (Phase 317 INV-02).
pub(crate) fn handle_game_inventory_update(
    state: &Arc<AppState>,
    inventory: &GameInventory,
) {
    tracing::info!(
        target: "fleet-inventory",
        "GameInventoryUpdate from pod {}: {} games",
        inventory.pod_id, inventory.games.len()
    );
    let state_clone = state.clone();
    let inv = inventory.clone();
    tokio::spawn(async move {
        crate::game_inventory::handle_game_inventory_update(&state_clone, inv).await;
    });
}

/// Handle AgentMessage::ComboValidationReport (Phase 317 COMBO-03/04).
pub(crate) fn handle_combo_validation_report(
    state: &Arc<AppState>,
    pod_id: &str,
    results: &[ComboValidationResult],
) {
    tracing::info!(
        target: "fleet-inventory",
        "ComboValidationReport from pod {}: {} results",
        pod_id, results.len()
    );
    let state_clone = state.clone();
    let pid = pod_id.to_string();
    let res = results.to_vec();
    tokio::spawn(async move {
        crate::game_inventory::handle_combo_validation_report(&state_clone, pid, res).await;
    });
}

/// Handle AgentMessage::LaunchTimelineReport (Phase 318 LAUNCH-05).
pub(crate) fn handle_launch_timeline_report(
    state: &Arc<AppState>,
    timeline: &LaunchTimeline,
) {
    let db = state.db.clone();
    let events_json = serde_json::to_string(&timeline.events)
        .unwrap_or_else(|_| "[]".to_string());
    let created_at = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();
    let tl = timeline.clone();
    tokio::spawn(async move {
        let result = sqlx::query(
            "INSERT OR REPLACE INTO launch_timeline_spans
             (launch_id, pod_id, sim_type, preset_id, billing_session_id,
              outcome, total_duration_ms, started_at, events_json, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&tl.launch_id)
        .bind(&tl.pod_id)
        .bind(tl.sim_type.to_string())
        .bind(&tl.preset_id)
        .bind(&tl.billing_session_id)
        .bind(&tl.outcome)
        .bind(tl.total_duration_ms as i64)
        .bind(&tl.started_at)
        .bind(&events_json)
        .bind(&created_at)
        .execute(&db)
        .await;
        if let Err(e) = result {
            tracing::error!(
                "LAUNCH-05: Failed to insert launch_timeline_spans for {}: {}",
                tl.launch_id, e
            );
        } else {
            tracing::info!(
                "LAUNCH-05: Persisted launch timeline for {} (outcome={})",
                tl.launch_id, tl.outcome
            );
        }
    });
}

/// Handle AgentMessage::ConfigMismatchDetected.
pub(crate) async fn handle_config_mismatch_detected(
    state: &Arc<AppState>,
    pod_id: &str,
    sim_type: &rc_common::types::SimType,
    mismatches: &[(String, String, String)],
    timestamp: &str,
) {
    let mismatch_details: Vec<String> = mismatches.iter()
        .map(|(field, expected, actual)| {
            format!("{}: expected '{}', got '{}'", field, expected, actual)
        })
        .collect();
    let detail_str = mismatch_details.join("; ");
    tracing::warn!(
        "CONFIG MISMATCH on Pod {} ({:?}): {} [{}]",
        pod_id, sim_type, detail_str, timestamp
    );

    crate::event_archive::append_event(
        &state.db,
        "game.config_mismatch",
        "agent",
        Some(pod_id),
        serde_json::json!({
            "sim_type": format!("{:?}", sim_type),
            "mismatches": mismatches,
            "timestamp": timestamp,
        }),
        &state.config.venue.venue_id,
    );

    let alert_msg = format!(
        "\u{26a0}\u{fe0f} CONFIG MISMATCH \u{2014} Pod {}\nSim: {:?}\n{}\nTimestamp: {}\n\nCustomer may have wrong game settings. Check kiosk wizard \u{2192} race.ini pipeline.",
        pod_id, sim_type, detail_str, timestamp
    );
    crate::whatsapp_alerter::send_whatsapp(&state.config, &alert_msg).await;

    let _ = state.dashboard_tx.send(DashboardEvent::ConfigMismatch {
        pod_id: pod_id.to_string(),
        sim_type: format!("{:?}", sim_type),
        details: mismatch_details,
        timestamp: timestamp.to_string(),
    });
}
