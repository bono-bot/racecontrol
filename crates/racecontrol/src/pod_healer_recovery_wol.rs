//! Wake-on-LAN recovery step for offline pods.
//!
//! Extracted from pod_healer_recovery.rs (v49.0 Architecture Completion).
//! Implements Step 3 of the graduated recovery ladder with 3 pre-checks:
//!   - CHECK 1: Skip if rc-sentry restarted with spawn_verified=true within 60s
//!   - CHECK 2 (MAINT-04): Skip if MAINTENANCE_MODE file present
//!   - CHECK 2b (OTA-09): Skip if OTA in progress
//!   - PRE-WoL: If sentry alive, retry rc-agent restart before WoL
//!   - CHECK 3: Write WOL_SENT sentinel via rc-sentry before magic packet

use std::sync::Arc;
use std::time::Duration;

use crate::activity_log::log_pod_activity;
use crate::pod_healer::{PodRecoveryStep, PodRecoveryTracker};
use crate::state::AppState;
use crate::wol;
use rc_common::recovery::{
    RecoveryAction, RecoveryAuthority, RecoveryDecision, RecoveryLogger, RECOVERY_LOG_SERVER,
};
use rc_common::types::PodInfo;

/// Execute the WakeOnLan recovery step (Step 3 of graduated recovery).
///
/// Performs 3 pre-checks before sending a WoL magic packet:
///   - CHECK 1: Skip if rc-sentry restarted with spawn_verified=true within 60s
///   - CHECK 2 (MAINT-04): Skip if MAINTENANCE_MODE file present (prevents infinite loop)
///   - CHECK 2b (OTA-09): Skip if OTA in progress
///   - PRE-WoL: If sentry alive, retry rc-agent restart via watchdog before WoL
///   - CHECK 3: Write WOL_SENT sentinel via rc-sentry /exec before sending packet
pub(crate) async fn run_wol_step(
    state: &Arc<AppState>,
    pod: &PodInfo,
    tracker: &mut PodRecoveryTracker,
) {
    tracing::info!(
        target: "pod_healer",
        "Pod {} -- step WoL: checking recovery events before WoL",
        pod.id
    );

    // CHECK 1: Query recovery events — skip WoL if rc-sentry restarted recently
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

    // CHECK 2 (MAINT-04): Read MAINTENANCE_MODE via rc-sentry /files
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

    // CHECK 2b (OTA-09): Check OTA sentinel
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

            // SF-05: Heal-lease check before WoL
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
