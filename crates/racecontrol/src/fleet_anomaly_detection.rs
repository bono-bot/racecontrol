//! Fleet anomaly detection — extracted from fleet_health.rs (Phase 385 ARCH-03).
//!
//! Checks 9 anomaly dimensions after each probe cycle:
//! 1. Build version skew (majority build_id vs outliers)
//! 2. Clock drift >10s
//! 3. Guard violation spike (fleet-wide)
//! 4. Ready delay >60s
//! 5. App health degradation
//! 6. Dashboard orphan detection
//! 7. Mass pod offline
//! 8. PIN validation failure spike
//! 9. Bat file hash drift
//!
//! Uses static cooldowns (15 min per class) to prevent alert fatigue.

use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;

use crate::fleet_health::FleetHealthStore;
use crate::state::AppState;

/// Called from `start_probe_loop` after each 15s probe cycle.
/// Snapshot fleet state, drop write lock, then pass here.
pub(crate) async fn detect_fleet_anomalies(
    state: &Arc<AppState>,
    fleet: &HashMap<String, FleetHealthStore>,
) {
    use std::sync::atomic::{AtomicI64, Ordering};

    // ── 0. Venue close transition → bulk-resolve stale incidents ───────────
    // Track venue state transitions. When venue closes (all pods off, normal shutdown),
    // auto-resolve all open pod_offline and dashboard_orphan incidents.
    {
        use std::sync::atomic::AtomicBool;
        static WAS_VENUE_OPEN: AtomicBool = AtomicBool::new(true);

        let is_open = crate::venue_state::venue_is_open();
        let was_open = WAS_VENUE_OPEN.swap(is_open, std::sync::atomic::Ordering::Relaxed);

        if was_open && !is_open {
            // Transition: open → closed. Bulk-resolve stale incidents.
            tracing::info!(
                target: "fleet-anomaly",
                "VENUE_CLOSED: Bulk-resolving open pod_offline and dashboard_orphan incidents"
            );
            let db = state.db.clone();
            tokio::spawn(async move {
                let now = chrono::Utc::now().to_rfc3339();
                let _ = sqlx::query(
                    "UPDATE fleet_incidents SET resolved_at = ?1, resolution = 'venue_closed'
                     WHERE resolved_at IS NULL AND (problem_key LIKE 'pod_offline:%' OR problem_key = 'dashboard_orphan:kiosk')"
                )
                .bind(&now)
                .execute(&db)
                .await;
            });
        }
    }

    // Cooldown: one alert per class per 15 minutes
    static LAST_BUILD_ALERT: AtomicI64 = AtomicI64::new(0);
    static LAST_DRIFT_ALERT: AtomicI64 = AtomicI64::new(0);
    static LAST_DELAY_ALERT: AtomicI64 = AtomicI64::new(0);
    static LAST_VIOLATION_ALERT: AtomicI64 = AtomicI64::new(0);
    const COOLDOWN_SECS: i64 = 900; // 15 minutes

    let now_ts = Utc::now().timestamp();

    // ── 0. Empty fleet guard (closes incident #5: pods DB desync) ──────────
    // If fleet map is empty but agents are connected, something is wrong.
    {
        static LAST_EMPTY_ALERT: AtomicI64 = AtomicI64::new(0);
        let agents_connected = {
            let senders = state.agent_senders.read().await;
            senders.values().filter(|s| !s.is_closed()).count()
        };
        if fleet.is_empty() && agents_connected > 0 {
            tracing::error!(
                target: "fleet-anomaly",
                "PODS_EMPTY: fleet health store is empty but {} agents connected — kiosk may show 'Waiting for pods'",
                agents_connected
            );
            if now_ts - LAST_EMPTY_ALERT.load(Ordering::Relaxed) > COOLDOWN_SECS {
                LAST_EMPTY_ALERT.store(now_ts, Ordering::Relaxed);
                let msg = format!(
                    "⚠ PODS EMPTY: Fleet health store empty but {} WS agents connected.\nKiosk may show 'Waiting for pods'. Server may need restart or pods need to reconnect.",
                    agents_connected
                );
                crate::whatsapp_alerter::send_whatsapp(&state.config, &msg).await;
            }
        }
    }

    // ── 1. Build version skew ──────────────────────────────────────────────
    // Find the majority build_id among reachable pods, flag outliers.
    {
        let mut build_counts: HashMap<&str, Vec<&str>> = HashMap::new();
        for (pod_id, store) in fleet.iter() {
            if store.http_reachable {
                if let Some(ref bid) = store.build_id {
                    build_counts.entry(bid.as_str()).or_default().push(pod_id.as_str());
                }
            }
        }

        if build_counts.len() > 1 {
            // Find majority (safe: only reached when build_counts.len() > 1)
            if let Some((majority_build, _majority_pods)) = build_counts
                .iter()
                .max_by_key(|(_, pods)| pods.len())
            {

            let outliers: Vec<(&str, &str)> = build_counts
                .iter()
                .filter(|(bid, _)| *bid != majority_build)
                .flat_map(|(bid, pods)| pods.iter().map(move |p| (*p, *bid)))
                .collect();

            if !outliers.is_empty() {
                for (pod_id, bid) in &outliers {
                    tracing::warn!(
                        target: "fleet-anomaly",
                        "BUILD_SKEW: {} running {} (fleet majority: {})",
                        pod_id, bid, majority_build
                    );
                }

                if now_ts - LAST_BUILD_ALERT.load(Ordering::Relaxed) > COOLDOWN_SECS {
                    LAST_BUILD_ALERT.store(now_ts, Ordering::Relaxed);
                    let outlier_list: Vec<String> = outliers
                        .iter()
                        .map(|(p, b)| format!("{} ({})", p, b))
                        .collect();
                    let msg = format!(
                        "⚠ BUILD SKEW: {} pod(s) running different build than fleet majority ({}):\n{}",
                        outliers.len(), majority_build,
                        outlier_list.join(", ")
                    );
                    crate::whatsapp_alerter::send_whatsapp(&state.config, &msg).await;
                }
            }
            } // end if let Some(majority_build)
        }
    }

    // ── 2. Clock drift >30s ────────────────────────────────────────────────
    {
        // v3.6: Lowered from 30s to 10s — 13s drift across fleet was undetected at old threshold.
        // 10s catches drift before it impacts log correlation, JWT validation, and cloud sync.
        const CLOCK_DRIFT_THRESHOLD_SECS: i64 = 10;
        let mut drifted: Vec<(&str, i64)> = Vec::new();

        for (pod_id, store) in fleet.iter() {
            if let Some(drift) = store.clock_drift_secs {
                if drift.abs() > CLOCK_DRIFT_THRESHOLD_SECS {
                    drifted.push((pod_id.as_str(), drift));
                    tracing::warn!(
                        target: "fleet-anomaly",
                        "CLOCK_DRIFT: {} drifted {}s (threshold: {}s)",
                        pod_id, drift, CLOCK_DRIFT_THRESHOLD_SECS
                    );
                }
            }
        }

        if !drifted.is_empty() && now_ts - LAST_DRIFT_ALERT.load(Ordering::Relaxed) > COOLDOWN_SECS {
            LAST_DRIFT_ALERT.store(now_ts, Ordering::Relaxed);
            let list: Vec<String> = drifted.iter().map(|(p, d)| format!("{}: {}s", p, d)).collect();
            let msg = format!(
                "⚠ CLOCK DRIFT: {} pod(s) drifted >{}s from server:\n{}",
                drifted.len(), CLOCK_DRIFT_THRESHOLD_SECS, list.join(", ")
            );
            crate::whatsapp_alerter::send_whatsapp(&state.config, &msg).await;
        }
    }

    // ── 3. Guard violation spike (fleet-wide) ──────────────────────────────
    // Check violation stores for recent surge (>10 in last hour across fleet).
    {
        let violations = state.pod_violations.read().await;
        let now = Utc::now();
        let mut total_violations_1h: u32 = 0;
        let mut pods_with_violations: Vec<(&str, u32)> = Vec::new();

        for (pod_id, vstore) in violations.iter() {
            let count_24h = vstore.violation_count_24h(now);
            if count_24h > 0 {
                pods_with_violations.push((pod_id.as_str(), count_24h));
                total_violations_1h += count_24h;
            }
        }

        // Fleet-wide pattern: if ALL pods have violations, it's likely a config issue
        let pod_count = fleet.values().filter(|s| s.http_reachable).count();
        let pods_affected = pods_with_violations.len();

        if pods_affected > 0 && (total_violations_1h > 20 || pods_affected >= pod_count.max(1)) {
            tracing::warn!(
                target: "fleet-anomaly",
                "VIOLATION_SPIKE: {} violations across {}/{} pods in 24h — possible config issue",
                total_violations_1h, pods_affected, pod_count
            );

            if now_ts - LAST_VIOLATION_ALERT.load(Ordering::Relaxed) > COOLDOWN_SECS {
                LAST_VIOLATION_ALERT.store(now_ts, Ordering::Relaxed);
                let list: Vec<String> = pods_with_violations
                    .iter()
                    .map(|(p, c)| format!("{}: {}", p, c))
                    .collect();
                let msg = format!(
                    "⚠ VIOLATION SPIKE: {} total violations across {}/{} pods (24h):\n{}\nIf ALL pods affected, check allowlist config.",
                    total_violations_1h, pods_affected, pod_count, list.join(", ")
                );
                crate::whatsapp_alerter::send_whatsapp(&state.config, &msg).await;
            }
        }
    }

    // ── 4. Ready delay >60s (from DB, checked less frequently) ─────────────
    // This runs every probe cycle (15s) but the DB query is lightweight.
    // Only alert if a pod consistently has >60s avg ready delay over 7 days.
    {
        const READY_DELAY_THRESHOLD_MS: f64 = 60_000.0;

        let rows: Vec<(String, f64)> = match sqlx::query_as(
            "SELECT pod_id, AVG(CAST(duration_to_playable_ms AS REAL))
             FROM launch_events
             WHERE duration_to_playable_ms IS NOT NULL
               AND created_at >= datetime('now', '-7 days')
             GROUP BY pod_id
             HAVING AVG(CAST(duration_to_playable_ms AS REAL)) > ?",
        )
        .bind(READY_DELAY_THRESHOLD_MS)
        .fetch_all(&state.db)
        .await
        {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(
                    target: "fleet-anomaly",
                    "SLOW_READY DB query failed: {} — skipping check this cycle",
                    e
                );
                vec![]
            }
        };

        if !rows.is_empty() {
            for (pod_id, avg_ms) in &rows {
                tracing::warn!(
                    target: "fleet-anomaly",
                    "SLOW_READY: {} avg ready delay {:.0}ms (threshold: {:.0}ms)",
                    pod_id, avg_ms, READY_DELAY_THRESHOLD_MS
                );
            }

            if now_ts - LAST_DELAY_ALERT.load(Ordering::Relaxed) > COOLDOWN_SECS {
                LAST_DELAY_ALERT.store(now_ts, Ordering::Relaxed);
                let list: Vec<String> = rows.iter().map(|(p, ms)| format!("{}: {:.0}ms", p, ms)).collect();
                let msg = format!(
                    "⚠ SLOW READY: {} pod(s) with avg game launch delay >{}s:\n{}",
                    rows.len(), READY_DELAY_THRESHOLD_MS / 1000.0, list.join(", ")
                );
                crate::whatsapp_alerter::send_whatsapp(&state.config, &msg).await;
            }
        }
    }

    // ── 5. App health degradation (MI bridge — server-side blind spot) ─────
    // Check app_health_monitor consecutive failures. If any app has been
    // degraded for >=5 consecutive probes (~2.5 min), alert at fleet level.
    // MMA P2: Per-app cooldown so kiosk failure doesn't mask admin failure.
    // MMA P1: WhatsApp error handling + cooldown updated post-send only.
    {
        use std::sync::LazyLock;
        static APP_ALERT_COOLDOWNS: LazyLock<std::sync::Mutex<HashMap<String, i64>>> =
            LazyLock::new(|| std::sync::Mutex::new(HashMap::new()));

        let apps = ["admin", "kiosk", "web"];
        for app in &apps {
            let count = crate::app_health_monitor::get_consecutive_failures(app);
            if count >= 5 {
                tracing::warn!(
                    target: "fleet-anomaly",
                    "APP_DEGRADED: {} has {} consecutive health failures",
                    app, count
                );

                // Per-app cooldown check
                let last_alert = APP_ALERT_COOLDOWNS.lock().ok()
                    .and_then(|m| m.get(*app).copied())
                    .unwrap_or(0);

                if now_ts - last_alert > COOLDOWN_SECS {
                    let msg = format!(
                        "⚠ APP DEGRADED: {} failing health checks ({} consecutive, ≥2.5 min)\nCheck app_health_log for details. May need pm2 restart.",
                        app, count
                    );
                    // Send alert and update per-app cooldown
                    crate::whatsapp_alerter::send_whatsapp(&state.config, &msg).await;
                    if let Ok(mut m) = APP_ALERT_COOLDOWNS.lock() {
                        m.insert(app.to_string(), now_ts);
                    }
                }
            }
        }
    }

    // ── 6. Dashboard orphan detection (MI Phase 1 — client-side blind spot) ─
    // If kiosk app health is "ok" but zero dashboard WS clients connected for
    // 10+ consecutive probe cycles (2.5 min at 15s interval), alert.
    // This catches: WS token missing, CSP blocking JS, 401 on WS upgrade,
    // kiosk JS crash — all invisible to HTTP health probes.
    {
        static DASHBOARD_ORPHAN_COUNT: AtomicI64 = AtomicI64::new(0);
        static LAST_ORPHAN_ALERT: AtomicI64 = AtomicI64::new(0);

        let dashboard_clients = crate::ws::dashboard_client_count();
        let kiosk_healthy = crate::app_health_monitor::get_consecutive_failures("kiosk") == 0;

        if kiosk_healthy && dashboard_clients == 0 && crate::venue_state::venue_is_open() {
            let count = DASHBOARD_ORPHAN_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
            if count >= 10 {
                tracing::warn!(
                    target: "fleet-anomaly",
                    "DASHBOARD_ORPHAN: kiosk app healthy but 0 dashboard WS clients for {}+ probes — possible JS/auth/token issue",
                    count
                );
                if now_ts - LAST_ORPHAN_ALERT.load(Ordering::Relaxed) > COOLDOWN_SECS {
                    LAST_ORPHAN_ALERT.store(now_ts, Ordering::Relaxed);
                    let msg = "⚠ DASHBOARD ORPHAN: Kiosk app reports healthy but NO dashboard WebSocket clients connected (>2.5 min).\n\
                        Possible causes: WS token missing/wrong, CSP blocking JS, 401 on WS upgrade, kiosk JS crash.\n\
                        Check: browser console, .env.production.local, racecontrol.toml terminal_secret.";
                    crate::whatsapp_alerter::send_whatsapp(&state.config, msg).await;

                    // Log fleet incident via MI bridge
                    let incident = rc_common::mesh_types::MeshIncident {
                        id: format!("inc_dashboard_orphan_{}", chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)),
                        node: "server".to_string(),
                        problem_key: "dashboard_orphan:kiosk".to_string(),
                        severity: rc_common::mesh_types::IncidentSeverity::High,
                        cost: 0.0,
                        resolution: None,
                        time_to_resolve_secs: None,
                        resolved_by_tier: None,
                        detected_at: chrono::Utc::now(),
                        resolved_at: None,
                    };
                    let db = state.db.clone();
                    tokio::spawn(async move {
                        let _ = crate::fleet_kb::insert_incident(&db, &incident).await;
                    });
                }
            }
        } else {
            // Reset counter when clients are connected or kiosk is down
            let prev = DASHBOARD_ORPHAN_COUNT.swap(0, Ordering::Relaxed);
            if prev >= 10 {
                tracing::info!(
                    target: "fleet-anomaly",
                    "DASHBOARD_ORPHAN: Resolved — {} dashboard client(s) connected",
                    dashboard_clients
                );
                // Auto-resolve open orphan incidents
                let db = state.db.clone();
                tokio::spawn(async move {
                    let now = chrono::Utc::now().to_rfc3339();
                    let _ = sqlx::query(
                        "UPDATE fleet_incidents SET resolved_at = ?1, resolution = 'dashboard_reconnected'
                         WHERE problem_key = 'dashboard_orphan:kiosk' AND resolved_at IS NULL"
                    )
                    .bind(&now)
                    .execute(&db)
                    .await;
                });
            }
        }
    }

    // ── 7. Mass pod offline (MI bridge — network/power issue) ───────────────
    // If >=3 pods are offline simultaneously while venue is open, this is
    // likely a network/power issue, not individual pod crashes.
    {
        static LAST_MASS_OFFLINE_ALERT: AtomicI64 = AtomicI64::new(0);

        if crate::venue_state::venue_is_open() {
            let offline_pods: Vec<&str> = fleet.iter()
                .filter(|(_, s)| !s.http_reachable)
                .map(|(id, _)| id.as_str())
                .collect();

            if offline_pods.len() >= 3 {
                tracing::warn!(
                    target: "fleet-anomaly",
                    "MASS_OFFLINE: {} pods unreachable while venue is open — possible network/power issue: {:?}",
                    offline_pods.len(), offline_pods
                );

                if now_ts - LAST_MASS_OFFLINE_ALERT.load(Ordering::Relaxed) > COOLDOWN_SECS {
                    LAST_MASS_OFFLINE_ALERT.store(now_ts, Ordering::Relaxed);
                    let msg = format!(
                        "⚠ MASS OFFLINE: {} pods unreachable while venue is open:\n{}\nPossible network outage or power issue. Check router/switch/UPS.",
                        offline_pods.len(),
                        offline_pods.join(", ")
                    );
                    crate::whatsapp_alerter::send_whatsapp(&state.config, &msg).await;
                }
            }
        }
    }

    // ── 8. PIN validation failure spike ────────────────────────────────────
    // Drain API error counts and alert if PIN failures are elevated.
    // This detects: wrong PIN entered repeatedly, kiosk → server connectivity issues,
    // or broken PIN validation logic.
    {
        static LAST_PIN_ALERT: AtomicI64 = AtomicI64::new(0);
        let error_counts = state.drain_api_error_counts();
        let pin_failures: u32 = error_counts.iter()
            .filter(|(k, _)| k.contains("pin") || k.contains("validate"))
            .map(|(_, v)| *v)
            .sum();

        if pin_failures >= 5 {
            tracing::warn!(
                target: "fleet-anomaly",
                "PIN_FAILURE_SPIKE: {} PIN validation failures since last check",
                pin_failures
            );

            if now_ts - LAST_PIN_ALERT.load(Ordering::Relaxed) > COOLDOWN_SECS {
                LAST_PIN_ALERT.store(now_ts, Ordering::Relaxed);
                let details: Vec<String> = error_counts.iter()
                    .filter(|(k, _)| k.contains("pin") || k.contains("validate"))
                    .map(|(k, v)| format!("{}: {}", k, v))
                    .collect();
                let msg = format!(
                    "⚠ PIN FAILURE SPIKE: {} failed PIN attempts in last 15s:\n{}\nCheck kiosk connectivity + staff PIN in DB.",
                    pin_failures, details.join(", ")
                );
                crate::whatsapp_alerter::send_whatsapp(&state.config, &msg).await;

                // MI Bridge: Log fleet incident for PIN failures
                let incident = rc_common::mesh_types::MeshIncident {
                    id: format!("inc_pin_spike_{}", chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)),
                    node: "server".to_string(),
                    problem_key: "pin_failure_spike".to_string(),
                    severity: rc_common::mesh_types::IncidentSeverity::Medium,
                    cost: 0.0,
                    resolution: None,
                    time_to_resolve_secs: None,
                    resolved_by_tier: None,
                    detected_at: chrono::Utc::now(),
                    resolved_at: None,
                };
                let db = state.db.clone();
                tokio::spawn(async move {
                    let _ = crate::fleet_kb::insert_incident(&db, &incident).await;
                });
            }
        }
    }

    // ── 9. Bat file hash drift (closes incident #16, #22) ─────────────────
    // If bat_sha256 differs across the fleet, startup scripts are out of sync.
    // Stale bat files miss process kills, power settings, ConspitLink guards.
    // Data already collected in fleet health store from agent /health probe.
    {
        static LAST_BAT_ALERT: AtomicI64 = AtomicI64::new(0);
        let mut bat_hashes: HashMap<&str, Vec<&str>> = HashMap::new();
        for (pod_id, store) in fleet.iter() {
            if store.http_reachable {
                if let Some(ref hash) = store.bat_sha256 {
                    bat_hashes.entry(hash.as_str()).or_default().push(pod_id.as_str());
                }
            }
        }

        if bat_hashes.len() > 1 {
            let (majority_hash, _) = bat_hashes.iter().max_by_key(|(_, pods)| pods.len()).unwrap();
            let drifted: Vec<(&str, &str)> = bat_hashes.iter()
                .filter(|(h, _)| *h != majority_hash)
                .flat_map(|(h, pods)| pods.iter().map(move |p| (*p, *h)))
                .collect();

            if !drifted.is_empty() {
                for (pod_id, hash) in &drifted {
                    tracing::warn!(
                        target: "fleet-anomaly",
                        "BAT_DRIFT: {} has bat_sha256={} (fleet majority: {})",
                        pod_id, &hash[..16.min(hash.len())], &majority_hash[..16.min(majority_hash.len())]
                    );
                }

                if now_ts - LAST_BAT_ALERT.load(Ordering::Relaxed) > COOLDOWN_SECS {
                    LAST_BAT_ALERT.store(now_ts, Ordering::Relaxed);
                    let list: Vec<String> = drifted.iter().map(|(p, _)| p.to_string()).collect();
                    let msg = format!(
                        "⚠ BAT DRIFT: {} pod(s) have different start-rcagent.bat than fleet majority:\n{}\nDeploy bat sync needed — stale bat causes settings regression.",
                        drifted.len(), list.join(", ")
                    );
                    crate::whatsapp_alerter::send_whatsapp(&state.config, &msg).await;
                }
            }
        }
    }
}
