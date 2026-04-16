//! Pod healer graduated recovery — offline pod recovery with escalating steps.
//!
//! Implements a 5-step recovery ladder for offline pods:
//!   Step 1 (Waiting): 30s grace period before action
//!   Step 2 (TierOneRestart): rc-agent restart via sentry + RCWatchdog
//!   Step 3 (WakeOnLan): Context-aware WoL with 3 pre-checks
//!   Step 4 (AiEscalation): Escalate to AI for root cause analysis
//!   Step 5+ (AlertStaff): Repeated staff alerts every 15 minutes
//!
//! Extracted from pod_healer.rs (Phase 385, v49.0 Architecture Completion).
//!
//! ## Module structure
//!
//! - `pod_healer_recovery_wol` — WakeOnLan step with 3 pre-checks
//! - `pod_healer_recovery_escalation` — AI escalation + staff alerting steps

#[path = "pod_healer_recovery_wol.rs"]
mod wol_step;

#[path = "pod_healer_recovery_escalation.rs"]
mod escalation;

pub(crate) use escalation::{run_ai_escalation_step, run_alert_staff_step};
pub(crate) use wol_step::run_wol_step;

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;

use crate::activity_log::log_pod_activity;
use crate::pod_healer::{has_active_billing, PodRecoveryStep, PodRecoveryTracker};
use crate::state::AppState;
use rc_common::recovery::{
    RecoveryAction, RecoveryAuthority, RecoveryDecision, RecoveryIntent, RecoveryLogger,
    RECOVERY_LOG_SERVER,
};
use rc_common::types::PodInfo;

// ─── Graduated Recovery ──────────────────────────────────────────────────────

/// Graduated recovery for offline pods.
///
/// Step 1 (Waiting): Record first_detected_at, wait 30s — no action.
/// Step 2 (TierOneRestart): Attempt rc-agent restart via pod-agent /exec.
/// Step 3 (WakeOnLan): Context-aware WoL — 3 pre-checks before magic packet:
///   - CHECK 1: Skip if rc-sentry restarted with spawn_verified=true within 60s
///   - CHECK 2 (MAINT-04): Skip if MAINTENANCE_MODE file present (prevents infinite loop)
///   - CHECK 3: Write WOL_SENT sentinel via rc-sentry /exec before sending packet
/// Step 4 (AiEscalation): Escalate to AI via query_ai().
/// Step 5+ (AlertStaff): Send email alert and log AlertStaff each cycle until pod recovers.
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

    let tracker = trackers.entry(pod.id.clone()).or_default();
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
        PodRecoveryStep::TierOneRestart => {
            tracing::info!(
                target: "pod_healer",
                "Pod {} — step 2: Tier 1 restart (rc-agent via pod-agent)",
                pod.id
            );

            // COORD-01: ProcessOwnership enforcement
            {
                let ownership = state.process_ownership.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(owner) = ownership.owner_of("rc-agent.exe")
                    && owner != RecoveryAuthority::PodHealer {
                        tracing::info!(
                            target: "pod_healer",
                            "Pod {} rc-agent.exe owned by {:?}, not PodHealer — skipping Tier 1 restart, advancing to AI escalation",
                            pod.id, owner
                        );
                        tracker.step = PodRecoveryStep::AiEscalation;
                        return;
                    }
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
            {
                let mut intents = state.recovery_intents.lock().unwrap_or_else(|e| e.into_inner());
                intents.register(RecoveryIntent::new(
                    &pod.id,
                    "rc-agent.exe",
                    RecoveryAuthority::PodHealer,
                    "graduated_tier1_restart",
                ));
            }

            // CONN-RESIL: Network partition detection
            let sentry_url = format!("http://{}:8091/health", pod.ip_address);
            let sentry_reachable = state
                .http_client
                .get(&sentry_url)
                .timeout(std::time::Duration::from_secs(3))
                .send()
                .await
                .is_ok();

            if !sentry_reachable {
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

            // Step A: Ensure RCWatchdog service is running
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
            run_wol_step(state, pod, tracker).await;
        }

        PodRecoveryStep::AiEscalation => {
            run_ai_escalation_step(state, pod, tracker).await;
        }

        PodRecoveryStep::AlertStaff => {
            run_alert_staff_step(state, pod, tracker).await;
        }
    }
}
