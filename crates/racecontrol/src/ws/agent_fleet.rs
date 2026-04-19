use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;
use rc_common::protocol::{CoreMessage, CoreToAgentMessage, DashboardEvent, EscalationPayload};
use rc_common::types::{ProcessViolation, BillingSessionStatus, GameState};
use crate::{activity_log::log_pod_activity, event_archive, state::AppState, billing};

// Phase 206 (OBS-04): Rate-limit sentinel WhatsApp alerts per pod per 5 min.
//
// SCALE-03 (2026-04-20): tokio::sync::Mutex instead of std::sync::Mutex.
// std::sync::Mutex.lock() parks the Tokio worker thread on contention;
// tokio::sync::Mutex yields the task, preserving parallelism under
// simultaneous sentinel events on 8 pods (e.g., deploy wave creating
// OTA_DEPLOYING sentinels fleet-wide).
static SENTINEL_ALERT_COOLDOWN: std::sync::LazyLock<Mutex<HashMap<String, Instant>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

/// Check and update sentinel alert cooldown. Returns true if the alert should fire.
async fn check_sentinel_cooldown(key: &str) -> bool {
    const COOLDOWN_SECS: u64 = 300;
    let mut map = SENTINEL_ALERT_COOLDOWN.lock().await;
    let now = Instant::now();
    if let Some(last) = map.get(key)
        && now.duration_since(*last).as_secs() < COOLDOWN_SECS {
            return false;
        }
    map.insert(key.to_string(), now);
    true
}

/// Handle AgentMessage::StartupReport.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn handle_startup_report(
    state: &Arc<AppState>,
    pod_id: &str,
    version: &str,
    uptime_secs: u64,
    config_hash: &str,
    crash_recovery: bool,
    repairs: &[String],
    lock_screen_port_bound: bool,
    remote_ops_port_bound: bool,
    hid_detected: bool,
    udp_ports_bound: &[u16],
    windows_session_id: Option<u32>,
) {
    tracing::info!(
        "Pod {} startup report: version={}, uptime={}s, config_hash={}, crash_recovery={}, repairs={:?}, \
         lock_screen={}, remote_ops={}, hid={}, udp_ports={:?}",
        pod_id, version, uptime_secs, config_hash, crash_recovery, repairs,
        lock_screen_port_bound, remote_ops_port_bound, hid_detected, udp_ports_bound
    );
    if crash_recovery {
        tracing::warn!("Pod {} recovered from a crash!", pod_id);
    }
    if !repairs.is_empty() {
        tracing::warn!("Pod {} self-healed: {:?}", pod_id, repairs);
    }
    if !lock_screen_port_bound {
        tracing::warn!("Pod {} BOOT WARNING: lock screen port 18923 NOT bound!", pod_id);
    }
    if !remote_ops_port_bound {
        tracing::warn!("Pod {} BOOT WARNING: remote ops port 8090 NOT bound!", pod_id);
    }
    // MI hardening: Session 0 detection
    if let Some(session) = windows_session_id
        && session == 0 {
            tracing::error!(
                target: "fleet-anomaly",
                "SESSION_0: Pod {} rc-agent running in Session 0 (Services) — ALL GUI features broken (Edge, overlay, game launch, blanking)",
                pod_id
            );
            let msg = format!(
                "\u{1f6a8} SESSION 0: Pod {} rc-agent running in Session 0!\nGUI features (blanking, game launch, overlay) WILL NOT WORK.\nFix: kill rc-agent \u{2192} RCWatchdog restarts in Session 1. Never use schtasks directly.",
                pod_id
            );
            let config = state.config.clone();
            tokio::spawn(async move {
                crate::whatsapp_alerter::send_whatsapp(&config, &msg).await;
            });
        }
    log_pod_activity(
        state, pod_id, "system", "Startup Report",
        &format!("v{} uptime={}s hash={} crash_recovery={} repairs={:?}",
            version, uptime_secs, config_hash, crash_recovery, repairs),
        "agent", None,
    );
    // Store version + uptime + boot verification for fleet health dashboard.
    let crash_loop_just_detected = {
        let mut fleet = state.pod_fleet_health.write().await;
        let store = fleet.entry(pod_id.to_string()).or_default();
        let was_looping = store.crash_loop;
        crate::fleet_health::store_startup_report(
            store, version, uptime_secs, crash_recovery,
            lock_screen_port_bound, remote_ops_port_bound,
            hid_detected, udp_ports_bound,
            windows_session_id,
        );
        !was_looping && store.crash_loop
    };
    if crash_loop_just_detected {
        let restart_count = {
            let fleet = state.pod_fleet_health.read().await;
            fleet.get(pod_id)
                .map(|s| s.startup_timestamps.len())
                .unwrap_or(0)
        };
        let summary = format!(
            "CRASH LOOP: Pod {} is restarting every ~{}s ({} restarts in 5 min). Likely OS/hardware issue. Reboot required.",
            pod_id, uptime_secs, restart_count
        );
        tracing::error!(
            target: "fleet-health",
            pod_id = %pod_id,
            restart_count = restart_count,
            uptime_secs = uptime_secs,
            "{}",
            summary
        );
        let escalation = state.whatsapp_escalation.clone();
        let pod_id_owned = pod_id.to_string();
        tokio::spawn(async move {
            escalation.handle_escalation(EscalationPayload {
                pod_id: pod_id_owned.clone(),
                incident_id: format!("crash_loop_{}", pod_id_owned),
                severity: "critical".to_string(),
                trigger: "CrashLoop".to_string(),
                summary,
                actions_tried: vec!["monitoring_only".to_string()],
                impact: format!("Pod {} unavailable — all customer sessions on this pod are broken", pod_id_owned),
                dashboard_url: "http://192.168.31.23:8080/api/v1/fleet/health".to_string(),
                timestamp: crate::whatsapp_alerter::ist_now_string(),
            }).await;
        });
    }
}

/// Handle AgentMessage::HardwareDisconnect.
pub(crate) async fn handle_hardware_disconnect(
    state: &Arc<AppState>,
    pod_id: &str,
    device: &str,
    timestamp: &str,
) {
    tracing::error!(
        "RESIL-04: Hardware disconnect on pod {}: {} at {}",
        pod_id, device, timestamp
    );
    log_pod_activity(state, pod_id, "hardware", "USB Disconnect",
        &format!("{} disconnected", device), "agent", None);

    let has_active_billing = {
        let timers = state.billing.active_timers.read().await;
        timers.get(pod_id).is_some_and(|t| {
            matches!(t.status, BillingSessionStatus::Active)
        })
    };
    if has_active_billing {
        let mut timers = state.billing.active_timers.write().await;
        if let Some(timer) = timers.get_mut(pod_id)
            && timer.status == BillingSessionStatus::Active {
                timer.status = BillingSessionStatus::PausedGamePause;
                timer.pause_count += 1;
                tracing::info!(
                    "RESIL-04: Billing auto-paused for pod {} — {} disconnected",
                    pod_id, device
                );
            }
        drop(timers);
    }

    let alert_msg = format!(
        "[HW ALERT] {} disconnected on Pod {}. Billing paused. {}",
        device, pod_id,
        crate::whatsapp_alerter::ist_now_string()
    );
    crate::whatsapp_alerter::send_whatsapp(&state.config, &alert_msg).await;
}

/// Handle AgentMessage::Disconnect (graceful).
pub(crate) async fn handle_disconnect(
    state: &Arc<AppState>,
    pod_id: &str,
) {
    tracing::info!("Pod {} disconnected", pod_id);
    log_pod_activity(state, pod_id, "system", "Pod Offline", "Agent sent disconnect", "agent", None);
    event_archive::append_event(&state.db, "pod.offline", "ws", Some(pod_id), serde_json::json!({ "reason": "agent_disconnect" }), &state.config.venue.venue_id);
    let has_active_billing = state
        .billing
        .active_timers
        .read()
        .await
        .contains_key(pod_id);

    if let Some(pod) = state.pods.write().await.get_mut(pod_id) {
        if pod.status == rc_common::types::PodStatus::Disabled {
            return; // Don't overwrite Disabled — caller should break
        }
        pod.status = rc_common::types::PodStatus::Offline;
        pod.driving_state = Some(rc_common::types::DrivingState::NoDevice);
        if !has_active_billing {
            pod.game_state = Some(GameState::Idle);
            pod.current_game = None;
        }
        let _ = state
            .dashboard_tx
            .send(DashboardEvent::PodUpdate(pod.clone()));
    }
    billing::update_driving_state(
        state,
        pod_id,
        rc_common::types::DrivingState::NoDevice,
    )
    .await;
    if let Err(e) = sqlx::query(
        "UPDATE pods SET status = 'offline', last_seen = datetime('now')
         WHERE id = ? AND status NOT IN ('disabled', 'maintenance')"
    )
    .bind(pod_id)
    .execute(&state.db)
    .await {
        tracing::warn!("Failed to sync pod {} graceful disconnect to DB: {}", pod_id, e);
    }
    {
        let mut fleet = state.pod_fleet_health.write().await;
        if let Some(store) = fleet.get_mut(pod_id) {
            crate::fleet_health::clear_on_disconnect(store);
        }
    }
}

/// Handle AgentMessage::ProcessViolation.
pub(crate) async fn handle_process_violation(
    state: &Arc<AppState>,
    violation: &ProcessViolation,
    registered_pod_id: &Option<String>,
) {
    let machine_id = violation.machine_id.clone();
    let name = violation.name.clone();
    let action = violation.action_taken.clone();
    let ts = violation.timestamp.clone();
    let now = chrono::Utc::now();

    if action == "reported" || action == "report_only" {
        tracing::debug!(
            "[guard] Violation on {}: {} action={} ts={}",
            machine_id, name, action, ts
        );
    } else {
        tracing::warn!(
            "[guard] Violation on {}: {} action={} ts={}",
            machine_id, name, action, ts
        );
    }

    let pod_key = if let Some(pod_id) = registered_pod_id {
        pod_id.clone()
    } else {
        machine_id.replace('-', "_")
    };

    let should_escalate = {
        let mut vmap = state.pod_violations.write().await;
        let store = vmap.entry(pod_key.clone()).or_default();
        let escalate = store.repeat_offender_check(violation, now);
        store.push(violation.clone());
        escalate
    };

    if should_escalate {
        let subject = format!(
            "GUARD ALERT: Repeat offender on {} — {}",
            machine_id, name
        );
        let body = format!(
            "Process '{}' has been killed 3+ times in the last 5 minutes on machine '{}'.\n\
             Last action: {}\nTimestamp: {}\n\n\
             Check C:\\RacingPoint\\process-guard.log on the affected machine.",
            name, machine_id, action, ts
        );
        let mut alerter = state.email_alerter.write().await;
        alerter.send_alert(&pod_key, &subject, &body).await;
    }
}

/// Handle AgentMessage::SentinelChange.
pub(crate) async fn handle_sentinel_change(
    state: &Arc<AppState>,
    pod_id: &str,
    file: &str,
    action: &str,
    timestamp: &str,
) {
    tracing::info!(
        target: "fleet",
        pod = %pod_id,
        sentinel = %file,
        action = %action,
        timestamp = %timestamp,
        "sentinel file change received"
    );

    let pod_number: u32 = pod_id.strip_prefix("pod_")
        .and_then(|n| n.parse().ok())
        .unwrap_or(0);

    let updated_sentinels = {
        let mut fleet = state.pod_fleet_health.write().await;
        let store = fleet.entry(pod_id.to_string()).or_default();
        crate::fleet_health::update_sentinel(store, file, action);
        store.active_sentinels.clone()
    };

    let _ = state.dashboard_tx.send(
        DashboardEvent::SentinelChanged {
            pod_id: pod_id.to_string(),
            pod_number,
            file: file.to_string(),
            action: action.to_string(),
            timestamp: timestamp.to_string(),
            active_sentinels: updated_sentinels,
        }
    );

    if file == "MAINTENANCE_MODE" && action == "created" {
        let cooldown_key = format!("sentinel_{}_{}", file, pod_number);
        if check_sentinel_cooldown(&cooldown_key).await {
            let alert_msg = format!(
                "[ALERT] Pod {} entered MAINTENANCE_MODE at {} (IST). \
                Sentinel file created — all restarts are now blocked. \
                Check fleet health or clear sentinel to recover.",
                pod_number, timestamp
            );
            crate::whatsapp_alerter::send_whatsapp(&state.config, &alert_msg).await;
            tracing::warn!(
                target: "state",
                pod = pod_number,
                "MAINTENANCE_MODE WhatsApp alert sent to Uday"
            );
        } else {
            tracing::debug!(
                target: "state",
                pod = pod_number,
                "MAINTENANCE_MODE WhatsApp alert suppressed (5-min cooldown active)"
            );
        }
    }
}

/// Handle AgentMessage::DiagnosticResult.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn handle_diagnostic_result(
    state: &Arc<AppState>,
    correlation_id: &str,
    tier: u8,
    outcome: &str,
    root_cause: &str,
    fix_action: &str,
    fix_type: &str,
    confidence: f64,
    fix_applied: bool,
    problem_hash: &str,
    summary: &str,
    registered_pod_id: &Option<String>,
) {
    tracing::info!(
        target: "debug-bridge",
        correlation_id = %correlation_id,
        tier = tier,
        outcome = %outcome,
        fix_applied = fix_applied,
        "DiagnosticResult received from pod"
    );

    if outcome == "fixed" || !root_cause.is_empty() {
        let res_id = uuid::Uuid::new_v4().to_string();
        let resolution_text = if !summary.is_empty() {
            format!("[Tier {}] {}", tier, summary)
        } else {
            format!("[Tier {}] {}: {}", tier, outcome, fix_action)
        };
        let effectiveness = if outcome == "fixed" { 4 } else { 2 };

        let category: String = sqlx::query_scalar(
            "SELECT category FROM debug_incidents WHERE id = ?"
        )
        .bind(correlation_id)
        .fetch_optional(&state.db)
        .await
        .unwrap_or(None)
        .unwrap_or_else(|| "unknown".to_string());

        let _ = sqlx::query(
            "INSERT INTO debug_resolutions (id, incident_id, category, resolution_text, effectiveness) \
             VALUES (?, ?, ?, ?, ?)"
        )
        .bind(&res_id)
        .bind(correlation_id)
        .bind(&category)
        .bind(&resolution_text)
        .bind(effectiveness)
        .execute(&state.db)
        .await;

        let log_detail = format!(
            "Tier {} diagnosis: {} (confidence: {:.0}%, applied: {})",
            tier, summary, confidence * 100.0, fix_applied
        );
        let log_pod = registered_pod_id.as_deref().unwrap_or("unknown");
        crate::activity_log::log_pod_activity(
            state, log_pod, "race_engineer", "Pod Diagnosis Complete",
            &log_detail, "race_engineer", None,
        );
    }

    // Fleet Distribution (v27.0): Only broadcast Tier 2+ high-confidence fixes
    let is_fleet_worthy = tier >= 2 && confidence >= 0.8;
    if outcome == "fixed" && is_fleet_worthy && !problem_hash.is_empty() {
        tracing::info!(
            target: "debug-bridge",
            problem_hash = %problem_hash,
            confidence = confidence,
            "Broadcasting solution to fleet via MeshSolutionBroadcast"
        );

        let broadcast_msg = CoreToAgentMessage::MeshSolutionBroadcast {
            problem_hash: problem_hash.to_string(),
            problem_key: format!("staff_{}", correlation_id),
            root_cause: root_cause.to_string(),
            fix_action: fix_action.to_string(),
            fix_type: fix_type.to_string(),
            confidence,
            source_node: registered_pod_id.clone().unwrap_or_else(|| "unknown".to_string()),
            promotion_status: "fleet_verified".to_string(),
        };

        let origin_pod = match registered_pod_id {
            Some(id) => id.clone(),
            None => {
                tracing::warn!(target: "debug-bridge", "Fleet broadcast skipped — origin pod not registered");
                String::new()
            }
        };
        let sender_snapshot: Vec<_> = if origin_pod.is_empty() {
            vec![]
        } else {
            let senders = state.agent_senders.read().await;
            senders.iter()
                .filter(|(id, _)| **id != origin_pod)
                .map(|(id, s)| (id.clone(), s.clone()))
                .collect()
        };
        for (target_pod_id, sender) in &sender_snapshot {
            if let Err(e) = sender.send(CoreMessage::wrap(broadcast_msg.clone())).await {
                tracing::debug!(
                    target: "debug-bridge",
                    pod = %target_pod_id,
                    "Failed to broadcast solution: {}",
                    e
                );
            }
        }
    }

    if outcome == "fixed" && fix_applied {
        match sqlx::query(
            "UPDATE debug_incidents SET status = 'resolved', resolved_at = datetime('now') \
             WHERE id = ? AND status = 'open'"
        )
        .bind(correlation_id)
        .execute(&state.db)
        .await {
            Ok(result) if result.rows_affected() == 0 => {
                tracing::debug!(
                    target: "debug-bridge",
                    correlation_id = %correlation_id,
                    "Incident already closed or not found — skipping resolution"
                );
            }
            Ok(_) => {
                tracing::info!(
                    target: "debug-bridge",
                    correlation_id = %correlation_id,
                    "Incident auto-resolved by tier engine diagnosis"
                );
            }
            Err(e) => {
                tracing::warn!(
                    target: "debug-bridge",
                    correlation_id = %correlation_id,
                    "Failed to resolve incident: {}",
                    e
                );
            }
        }
    }
}

#[path = "agent_fleet_lockdown.rs"]
mod lockdown;
pub(crate) use lockdown::handle_kiosk_lockdown;
