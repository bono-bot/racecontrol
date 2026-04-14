//! Graduated recovery for offline pods.
//!
//! Extracted from pod_healer.rs (Phase 385, v49.0 Architecture Completion).
//! Contains: run_graduated_recovery, PodRecoveryStep, PodRecoveryTracker.
//!
//! Step 1 (Waiting): Record first_detected_at, wait 30s -- no action.
//! Step 2 (TierOneRestart): Attempt rc-agent restart via sentry /exec.
//! Step 3 (WakeOnLan): Context-aware WoL with 3 pre-checks.
//! Step 4 (AiEscalation): Escalate to AI via query_ai().
//! Step 5+ (AlertStaff): Send email alert each cycle until pod recovers.

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;

use crate::activity_log::log_pod_activity;
use crate::pod_healer_diagnostics::has_active_billing;
use crate::state::AppState;
use crate::wol;
use rc_common::recovery::{RecoveryAction, RecoveryAuthority, RecoveryDecision, RecoveryIntent, RecoveryLogger, RECOVERY_LOG_SERVER};
use rc_common::types::PodInfo;

// --- Graduated Recovery Types ------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PodRecoveryStep {
    /// First offline detection -- waiting 30s before acting.
    Waiting,
    /// Second cycle -- attempt Tier 1 rc-agent restart.
    TierOneRestart,
    /// Third cycle -- context-aware WoL after Tier 1 fails.
    WakeOnLan,
    /// Fourth cycle -- escalate to AI.
    AiEscalation,
    /// Fifth+ cycle -- alert staff.
    AlertStaff,
}

/// Per-pod graduated recovery state. Held in a HashMap inside heal_all_pods.
/// Not shared with AppState -- local to the healer loop.
#[derive(Debug)]
pub(crate) struct PodRecoveryTracker {
    pub(crate) step: PodRecoveryStep,
    pub(crate) first_detected_at: Option<std::time::Instant>,
    /// CONN-RESIL: Timestamp of last staff alert sent. Used to throttle re-alerts
    /// to every 15 minutes instead of every 2-minute healer cycle.
    pub(crate) last_staff_alert_at: Option<std::time::Instant>,
}

impl PodRecoveryTracker {
    pub(crate) fn new() -> Self {
        Self {
            step: PodRecoveryStep::Waiting,
            first_detected_at: None,
            last_staff_alert_at: None,
        }
    }

    pub(crate) fn reset(&mut self) {
        self.step = PodRecoveryStep::Waiting;
        self.first_detected_at = None;
        self.last_staff_alert_at = None;
    }
}

impl Default for PodRecoveryTracker {
    fn default() -> Self {
        Self::new()
    }
}

// --- Graduated Recovery Function ---------------------------------------------

/// Graduated recovery for offline pods.
///
/// Gates:
/// - in_maintenance=true  -> log SkipMaintenanceMode, return (no step advance)
/// - billing_active=true  -> log SkipCascadeGuardActive, return (no step advance)
/// - cascade guard paused -> skip silently, return
pub(crate) async fn run_graduated_recovery(
    state: &Arc<AppState>,
    pod: &PodInfo,
    trackers: &mut std::collections::HashMap<String, PodRecoveryTracker>,
) {
    // Cascade guard check
    {
        let guard = state.cascade_guard.lock().unwrap_or_else(|e| e.into_inner());
        if guard.is_paused() {
            tracing::warn!(
                target: "pod_healer",
                "graduated recovery for {} skipped — cascade guard paused",
                pod.id
            );
            return;
        }
    }

    // Maintenance gate (PMON-01): never touch a pod in maintenance
    let in_maintenance = {
        let health = state.pod_fleet_health.read().await;
        health.get(&pod.id).map(|h| h.in_maintenance).unwrap_or(false)
    };
    if in_maintenance {
        let decision = RecoveryDecision::new(
            "server",
            "rc-agent.exe",
            RecoveryAuthority::PodHealer,
            RecoveryAction::SkipMaintenanceMode,
            "pod_in_maintenance",
        );
        let _ = RecoveryLogger::new(RECOVERY_LOG_SERVER).log(&decision);
        tracing::info!(
            target: "pod_healer",
            "Pod {} in maintenance — skipping graduated recovery",
            pod.id
        );
        return;
    }

    // Billing gate: never restart a pod with an active session
    if has_active_billing(state, &pod.id).await {
        let decision = RecoveryDecision::new(
            "server",
            "rc-agent.exe",
            RecoveryAuthority::PodHealer,
            RecoveryAction::SkipCascadeGuardActive,
            "billing_active",
        );
        let _ = RecoveryLogger::new(RECOVERY_LOG_SERVER).log(&decision);
        tracing::info!(
            target: "pod_healer",
            "Pod {} has active billing — skipping graduated recovery",
            pod.id
        );
        return;
    }

    // COORD-02: Check if another authority has an active recovery intent for this pod
    {
        let intents = state.recovery_intents.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(active) = intents.has_active_intent(&pod.id, "rc-agent.exe") {
            let remaining = 120 - (Utc::now() - active.created_at).num_seconds();
            tracing::info!(
                target: "pod_healer",
                "Pod {} has active recovery intent from {:?} ({}), skipping — TTL expires in {}s",
                pod.id, active.authority, active.reason, remaining
            );
            return;
        }
    }

    // COORD-03: Check GRACEFUL_RELAUNCH sentinel via rc-sentry /files endpoint
    // If the sentinel is present, rc-agent is in the middle of a planned self-restart — not a crash.
    let sentry_url = format!(
        "http://{}:8091/files?path=C%3A%5CRacingPoint%5CGRACEFUL_RELAUNCH",
        pod.ip_address
    );
    match state
        .sentry_get(&sentry_url)
        .timeout(Duration::from_secs(3))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            tracing::info!(
                target: "pod_healer",
                "Pod {} GRACEFUL_RELAUNCH sentinel present — intentional restart in progress, skipping recovery",
                pod.id
            );
            let decision = RecoveryDecision::new(
                "server",
                "rc-agent.exe",
                RecoveryAuthority::PodHealer,
                RecoveryAction::SkipCascadeGuardActive,
                "graceful_relaunch_sentinel_present",
            );
            let _ = RecoveryLogger::new(RECOVERY_LOG_SERVER).log(&decision);
            return;
        }
        _ => {} // Sentinel absent or rc-sentry unreachable — proceed with recovery
    }

    // SF-05: Server-side heal-lease coordination via LeaseManager (v31.0 Phase 267).
    if state.lease_manager.get_lease(&pod.id).is_some() {
        tracing::info!(pod_id = %pod.id, "pod_healer: active heal lease — skipping recovery");
        return;
    }
    tracing::debug!(pod_id = %pod.id, "pod_healer: SF-05 coordination check passed, proceeding with recovery");

    let tracker = trackers.entry(pod.id.clone()).or_insert_with(PodRecoveryTracker::new);
    let now_instant = std::time::Instant::now();

    match tracker.step {
        PodRecoveryStep::Waiting => {
            if tracker.first_detected_at.is_none() {
                // First detection: record timestamp, log, wait
                tracker.first_detected_at = Some(now_instant);
                let decision = RecoveryDecision::new(
                    "server",
                    "rc-agent.exe",
                    RecoveryAuthority::PodHealer,
                    RecoveryAction::SkipCascadeGuardActive,
                    "graduated_step1_wait_30s",
                );
                let _ = RecoveryLogger::new(RECOVERY_LOG_SERVER).log(&decision);
                tracing::info!(
                    target: "pod_healer",
                    "Pod {} offline — step 1: waiting 30s before acting",
                    pod.id
                );
            } else if now_instant.duration_since(
                tracker.first_detected_at.unwrap_or(now_instant),
            ) >= std::time::Duration::from_secs(30)
            {
                // 30s elapsed: advance to TierOneRestart (fires on next cycle)
                tracker.step = PodRecoveryStep::TierOneRestart;
                tracing::info!(
                    target: "pod_healer",
                    "Pod {} — 30s elapsed, advancing to Tier 1 restart",
                    pod.id
                );
            }
        }

        // MON-02: Sentry fallback path — when rc-agent :8090 is unreachable but
        // rc-sentry :8091 is reachable, restart rc-agent via sentry's /exec endpoint.
        // Uses sc start RCWatchdog + taskkill (NOT schtasks) per Session 1 standing rule.
        PodRecoveryStep::TierOneRestart => {
            tracing::info!(
                target: "pod_healer",
                "Pod {} — step 2: Tier 1 restart (rc-agent via pod-agent)",
                pod.id
            );

            // COORD-01: ProcessOwnership enforcement
            // rc-agent.exe is registered to RcSentry — PodHealer should not perform
            // a direct process restart on it. Skip Tier 1 and advance to AI escalation.
            {
                let ownership = state.process_ownership.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(owner) = ownership.owner_of("rc-agent.exe") {
                    if owner != RecoveryAuthority::PodHealer {
                        tracing::info!(
                            target: "pod_healer",
                            "Pod {} rc-agent.exe owned by {:?}, not PodHealer — skipping Tier 1 restart, advancing to AI escalation",
                            pod.id, owner
                        );
                        tracker.step = PodRecoveryStep::AiEscalation;
                        return;
                    }
                }
                // If unregistered, PodHealer may proceed (backward compat)
            }

            let decision = RecoveryDecision::new(
                "server",
                "rc-agent.exe",
                RecoveryAuthority::PodHealer,
                RecoveryAction::Restart,
                "graduated_step2_tier1_restart",
            );
            {
                let mut guard = state.cascade_guard.lock().unwrap_or_else(|e| e.into_inner());
                if guard.record(&decision) {
                    tracing::error!(
                        target: "pod_healer",
                        "Cascade guard triggered — aborting graduated recovery for {}",
                        pod.id
                    );
                    return;
                }
            }
            let _ = RecoveryLogger::new(RECOVERY_LOG_SERVER).log(&decision);

            // COORD-02: Register PodHealer's recovery intent before acting.
            // This prevents concurrent recovery by another authority within the 2-min TTL.
            {
                let mut intents = state.recovery_intents.lock().unwrap_or_else(|e| e.into_inner());
                intents.register(RecoveryIntent::new(
                    &pod.id,
                    "rc-agent.exe",
                    RecoveryAuthority::PodHealer,
                    "graduated_tier1_restart",
                ));
            }

            // CONN-RESIL: Network partition detection — check if pod is network-reachable
            // before attempting restart. If we can't reach rc-sentry :8091 either, the
            // problem is likely a network partition (switch failure, cable unplugged),
            // not a crashed rc-agent. WoL won't help either in this case.
            let sentry_url = format!("http://{}:8091/health", pod.ip_address);
            let sentry_reachable = state
                .http_client
                .get(&sentry_url)
                .timeout(std::time::Duration::from_secs(3))
                .send()
                .await
                .is_ok();

            if !sentry_reachable {
                // Neither rc-agent nor rc-sentry is reachable — likely network partition
                tracing::warn!(
                    target: "pod_healer",
                    "Pod {} — network partition detected (rc-sentry :8091 also unreachable). \
                     Skipping Tier 1 restart (network issue, not process crash). Advancing to WoL.",
                    pod.id
                );
                log_pod_activity(
                    state,
                    &pod.id,
                    "race_engineer",
                    "Network Partition Detected",
                    "Both rc-agent and rc-sentry unreachable — likely network issue, not process crash",
                    "race_engineer",
                    None,
                );
                tracker.step = PodRecoveryStep::WakeOnLan;
                return;
            }

            // Restart strategy: ensure RCWatchdog service is running, then kill rc-agent.
            let exec_url = format!("http://{}:8091/exec", pod.ip_address);

            // Step A: Ensure RCWatchdog service is running (it may have stopped)
            let watchdog_result = state
                .sentry_post(&exec_url)
                .json(&serde_json::json!({ "cmd": "sc start RCWatchdog", "timeout": 10 }))
                .timeout(std::time::Duration::from_secs(15))
                .send()
                .await;
            match &watchdog_result {
                Ok(resp) if resp.status().is_success() => {
                    tracing::info!(
                        target: "pod_healer",
                        "Pod {} Tier 1: ensured RCWatchdog service is running",
                        pod.id
                    );
                }
                _ => {
                    tracing::warn!(
                        target: "pod_healer",
                        "Pod {} Tier 1: failed to ensure RCWatchdog (sentry exec failed)",
                        pod.id
                    );
                }
            }

            // Step B: Kill rc-agent (may already be dead, that's fine).
            let kill_result = state
                .sentry_post(&exec_url)
                .json(&serde_json::json!({ "cmd": "taskkill /F /IM rc-agent.exe", "timeout": 10 }))
                .timeout(std::time::Duration::from_secs(15))
                .send()
                .await;
            match kill_result {
                Ok(resp) if resp.status().is_success() => {
                    tracing::info!(
                        target: "pod_healer",
                        "Pod {} Tier 1 restart: killed rc-agent, RCWatchdog will respawn in Session 1",
                        pod.id
                    );
                    log_pod_activity(
                        state,
                        &pod.id,
                        "race_engineer",
                        "Graduated Restart (Tier 1)",
                        "rc-agent killed via sentry, RCWatchdog respawns in Session 1 (graduated step 2)",
                        "race_engineer",
                        None,
                    );
                }
                _ => {
                    tracing::warn!(
                        target: "pod_healer",
                        "Pod {} Tier 1 restart failed (sentry exec failed — auth mismatch?)",
                        pod.id
                    );
                }
            }
            tracker.step = PodRecoveryStep::WakeOnLan;
        }

        PodRecoveryStep::WakeOnLan => {
            tracing::info!(
                target: "pod_healer",
                "Pod {} -- step WoL: checking recovery events before WoL",
                pod.id
            );

            // CHECK 1: Query recovery events -- skip WoL if rc-sentry restarted
            // with spawn_verified=true within 60s (rc-sentry already handled recovery).
            let skip_wol_sentry = {
                let store = state.recovery_events.lock().unwrap_or_else(|e| e.into_inner());
                let recent = store.query(Some(&pod.id), Some(60));
                recent.iter().any(|e| {
                    e.authority == RecoveryAuthority::RcSentry
                        && matches!(e.action, RecoveryAction::Restart)
                        && e.spawn_verified == Some(true)
                })
            };
            if skip_wol_sentry {
                tracing::info!(
                    target: "pod_healer",
                    "Pod {} -- skipping WoL, sentry restarted within grace window (spawn_verified=true within 60s)",
                    pod.id
                );
                let decision = RecoveryDecision::new(
                    "server",
                    "rc-agent.exe",
                    RecoveryAuthority::PodHealer,
                    RecoveryAction::SkipCascadeGuardActive,
                    "sentry_restarted_within_60s_skip_wol",
                );
                let _ = RecoveryLogger::new(RECOVERY_LOG_SERVER).log(&decision);
                tracker.step = PodRecoveryStep::AiEscalation;
                return;
            }

            // CHECK 2 (MAINT-04): Read MAINTENANCE_MODE via rc-sentry /files before WoL.
            let maintenance_url = format!(
                "http://{}:8091/files?path=C%3A%5CRacingPoint%5CMAINTENANCE_MODE",
                pod.ip_address
            );
            let in_maintenance_file = match state
                .sentry_get(&maintenance_url)
                .timeout(Duration::from_secs(3))
                .send()
                .await
            {
                Ok(resp) if resp.status().is_success() => true,
                _ => false,
            };
            if in_maintenance_file {
                tracing::warn!(
                    target: "pod_healer",
                    "Pod {} has MAINTENANCE_MODE file -- skipping WoL to prevent WoL->restart->block infinite loop",
                    pod.id
                );
                let decision = RecoveryDecision::new(
                    "server",
                    "rc-agent.exe",
                    RecoveryAuthority::PodHealer,
                    RecoveryAction::SkipMaintenanceMode,
                    "maintenance_mode_file_present_skip_wol",
                );
                let _ = RecoveryLogger::new(RECOVERY_LOG_SERVER).log(&decision);
                tracker.step = PodRecoveryStep::AlertStaff;
                return;
            }

            // CHECK 2b (OTA-09): Check OTA sentinel — skip WoL if OTA deploy in progress.
            let ota_check_url = format!(
                "http://{}:8091/files?path=C%3A%5CRacingPoint%5Cota-in-progress.flag",
                pod.ip_address
            );
            let ota_in_progress = match state
                .sentry_get(&ota_check_url)
                .timeout(Duration::from_secs(3))
                .send()
                .await
            {
                Ok(resp) if resp.status().is_success() => true,
                _ => false,
            };
            if ota_in_progress {
                tracing::info!(
                    target: "pod_healer",
                    "Pod {} has OTA in progress -- skipping WoL to prevent WoL<>OTA conflict",
                    pod.id
                );
                tracker.step = PodRecoveryStep::AlertStaff;
                return;
            }

            // PRE-WoL: If rc-sentry IS reachable, try rc-agent restart via sentry first.
            let sentry_health = format!("http://{}:8091/health", pod.ip_address);
            let sentry_alive = state
                .sentry_get(&sentry_health)
                .timeout(std::time::Duration::from_secs(3))
                .send()
                .await
                .is_ok();
            if sentry_alive {
                tracing::info!(
                    target: "pod_healer",
                    "Pod {} — rc-sentry alive (machine is ON), retrying rc-agent restart via watchdog before WoL",
                    pod.id
                );
                let restart_url = format!("http://{}:8091/exec", pod.ip_address);
                let _ = state
                    .sentry_post(&restart_url)
                    .json(&serde_json::json!({ "cmd": "sc start RCWatchdog", "timeout": 10 }))
                    .timeout(std::time::Duration::from_secs(15))
                    .send()
                    .await;
                let _ = state
                    .sentry_post(&restart_url)
                    .json(&serde_json::json!({ "cmd": "taskkill /F /IM rc-agent.exe", "timeout": 10 }))
                    .timeout(std::time::Duration::from_secs(15))
                    .send()
                    .await;
                log_pod_activity(
                    state,
                    &pod.id,
                    "race_engineer",
                    "Graduated Recovery (Tier 2: Watchdog Restart)",
                    "rc-agent killed via sentry before WoL, RCWatchdog respawns in Session 1 (machine is on, WoL useless)",
                    "race_engineer",
                    None,
                );
            }

            // CHECK 3: Write WOL_SENT sentinel via rc-sentry /exec BEFORE sending magic packet.
            let sentinel_cmd = r#"echo WOL_SENT > C:\RacingPoint\WOL_SENT"#;
            let exec_url = format!("http://{}:8091/exec", pod.ip_address);
            let sentinel_body = serde_json::json!({ "cmd": sentinel_cmd, "timeout_ms": 5000 });
            match state
                .sentry_post(&exec_url)
                .json(&sentinel_body)
                .timeout(Duration::from_secs(5))
                .send()
                .await
            {
                Ok(resp) if resp.status().is_success() => {
                    tracing::info!(
                        target: "pod_healer",
                        "Pod {} WOL_SENT sentinel written via rc-sentry",
                        pod.id
                    );
                }
                _ => {
                    tracing::warn!(
                        target: "pod_healer",
                        "Pod {} failed to write WOL_SENT sentinel (rc-sentry may be down) -- proceeding with WoL anyway",
                        pod.id
                    );
                }
            }

            // SEND WoL: look up MAC address from pod info
            let mac = {
                let pods = state.pods.read().await;
                pods.get(&pod.id).and_then(|p| p.mac_address.clone())
            };
            match mac {
                Some(ref mac_addr) => {
                    let decision = RecoveryDecision::new(
                        "server",
                        &pod.id,
                        RecoveryAuthority::PodHealer,
                        RecoveryAction::WakeOnLan,
                        "graduated_wol_after_tier1_failed",
                    );
                    {
                        let mut guard = state.cascade_guard.lock().unwrap_or_else(|e| e.into_inner());
                        if guard.record(&decision) {
                            tracing::error!(
                                target: "pod_healer",
                                "Cascade guard triggered on WoL for {} -- aborting",
                                pod.id
                            );
                            return;
                        }
                    }
                    let _ = RecoveryLogger::new(RECOVERY_LOG_SERVER).log(&decision);

                    // SF-05: Heal-lease check before WoL (v31.0 Phase 267).
                    if state.lease_manager.get_lease(&pod.id).is_some() {
                        tracing::info!(pod_id = %pod.id, "pod_healer: active heal lease — skipping WoL");
                        return;
                    }
                    match wol::send_wol(mac_addr).await {
                        Ok(()) => {
                            tracing::info!(
                                target: "pod_healer",
                                "Pod {} WoL magic packet sent (MAC: {})",
                                pod.id,
                                mac_addr
                            );
                            log_pod_activity(
                                state,
                                &pod.id,
                                "race_engineer",
                                "Wake-on-LAN Sent",
                                &format!(
                                    "Graduated recovery WoL (Tier 1 restart failed, MAC: {})",
                                    mac_addr
                                ),
                                "race_engineer",
                                None,
                            );
                        }
                        Err(e) => {
                            tracing::warn!(
                                target: "pod_healer",
                                "Pod {} WoL failed: {}",
                                pod.id,
                                e
                            );
                        }
                    }
                }
                None => {
                    tracing::warn!(
                        target: "pod_healer",
                        "Pod {} has no MAC address -- cannot send WoL",
                        pod.id
                    );
                }
            }

            tracker.step = PodRecoveryStep::AiEscalation;
        }

        PodRecoveryStep::AiEscalation => {
            tracing::info!(
                target: "pod_healer",
                "Pod {} — step 3: AI escalation",
                pod.id
            );
            let decision = RecoveryDecision::new(
                "server",
                "rc-agent.exe",
                RecoveryAuthority::PodHealer,
                RecoveryAction::EscalateToAi,
                "graduated_step3_ai_escalation",
            );
            let _ = RecoveryLogger::new(RECOVERY_LOG_SERVER).log(&decision);

            let context = format!(
                "Pod {} is offline. Tier 1 restart was attempted and pod remains offline. \
                 Last seen: {:?}. Please suggest root cause and next steps.",
                pod.id, pod.last_seen
            );
            let messages = vec![
                serde_json::json!({
                    "role": "system",
                    "content": "You are a sim racing venue technician. A pod has failed to recover \
                                after an automated restart. Provide a brief root cause and specific \
                                manual steps. Keep under 150 words."
                }),
                serde_json::json!({ "role": "user", "content": context.clone() }),
            ];
            match crate::ai::query_ai(
                &state.config.ai_debugger,
                &messages,
                Some(&state.db),
                Some("healer_graduated"),
            )
            .await
            {
                Ok((suggestion, model)) => {
                    tracing::info!(
                        target: "pod_healer",
                        "Pod {} AI suggestion ({}): {}",
                        pod.id,
                        model,
                        suggestion.chars().take(100).collect::<String>()
                    );
                    log_pod_activity(
                        state,
                        &pod.id,
                        "race_engineer",
                        "AI Escalation",
                        &format!(
                            "AI suggestion ({}): {}",
                            model,
                            suggestion.chars().take(200).collect::<String>()
                        ),
                        "race_engineer",
                        None,
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        target: "pod_healer",
                        "Pod {} AI escalation failed: {}",
                        pod.id,
                        e
                    );
                }
            }
            tracker.step = PodRecoveryStep::AlertStaff;
        }

        PodRecoveryStep::AlertStaff => {
            // CONN-RESIL: Re-alert every 15 minutes instead of every 2-minute cycle.
            const RE_ALERT_INTERVAL: std::time::Duration = std::time::Duration::from_secs(15 * 60);
            let should_alert = tracker.last_staff_alert_at
                .map(|t| t.elapsed() >= RE_ALERT_INTERVAL)
                .unwrap_or(true); // First alert always fires

            if !should_alert {
                tracing::info!(
                    target: "pod_healer",
                    "Pod {} — still at AlertStaff, re-alert suppressed (next in {}s)",
                    pod.id,
                    RE_ALERT_INTERVAL.as_secs().saturating_sub(
                        tracker.last_staff_alert_at.map(|t| t.elapsed().as_secs()).unwrap_or(0)
                    )
                );
                return;
            }

            tracing::warn!(
                target: "pod_healer",
                "Pod {} — step 4: alerting staff (re-alert every 15min)",
                pod.id
            );
            let decision = RecoveryDecision::new(
                "server",
                "rc-agent.exe",
                RecoveryAuthority::PodHealer,
                RecoveryAction::AlertStaff,
                "graduated_step4_staff_alert",
            );
            let _ = RecoveryLogger::new(RECOVERY_LOG_SERVER).log(&decision);

            let offline_duration = tracker.first_detected_at
                .map(|t| t.elapsed())
                .unwrap_or_default();
            let body = format!(
                "Pod {} has failed all automated recovery steps.\n\
                 Tier 1 restart attempted. AI escalated. Pod still offline.\n\
                 Offline for: {}min {}s\n\
                 Last seen: {:?}\n\
                 Manual intervention required.\n\
                 (This alert repeats every 15 minutes until resolved.)",
                pod.id,
                offline_duration.as_secs() / 60,
                offline_duration.as_secs() % 60,
                pod.last_seen
            );
            let subject = format!(
                "[RaceControl] Pod {} — Manual Intervention Required ({}min offline)",
                pod.id,
                offline_duration.as_secs() / 60
            );
            state
                .email_alerter
                .write()
                .await
                .send_alert(&pod.id, &subject, &body)
                .await;
            tracker.last_staff_alert_at = Some(std::time::Instant::now());
            log_pod_activity(
                state,
                &pod.id,
                "race_engineer",
                "Staff Alert Sent",
                &format!(
                    "All automated recovery steps exhausted — staff alerted (offline {}min)",
                    offline_duration.as_secs() / 60
                ),
                "race_engineer",
                None,
            );
        }
    }
}
