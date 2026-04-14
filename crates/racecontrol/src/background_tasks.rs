//! Background task spawning for the RaceControl server.
//!
//! Each function spawns one or more `tokio::spawn` tasks that run for the
//! lifetime of the server. Grouped here to keep main.rs under 500 lines.
//!
//! Extracted from main.rs to keep the binary entrypoint under 500 lines.

use std::sync::Arc;
use std::time::Duration;

use racecontrol_crate::state::AppState;
use racecontrol_crate::{
    ac_camera, ac_server, action_queue, app_health_monitor, auth,
    backup_pipeline, billing, bono_relay, cloud_sync, deploy_awareness,
    error_aggregator, event_archive, fleet_health, game_launcher, pod_healer,
    pod_monitor, pod_reservation, process_guard, psychology, remote_terminal,
    scheduler, server_ops, udp_heartbeat,
};

/// Spawns all background tasks that run for the server's lifetime.
///
/// Call this after AppState is fully initialized (pods seeded, billing recovered, etc.).
pub fn spawn_all(
    state: Arc<AppState>,
    email_alert_rx: tokio::sync::broadcast::Receiver<()>,
    wa_alert_rx: tokio::sync::broadcast::Receiver<()>,
    error_rate_email_enabled: bool,
    email_script_for_alerter: String,
) {
    // v34.0 Phase 285: Metrics TSDB -- async ingestion pipeline + rollup/purge
    let metrics_tx = racecontrol_crate::metrics_tsdb::spawn_metrics_ingestion(state.db.clone());
    racecontrol_crate::metrics_tsdb::spawn_rollup_and_purge(state.db.clone());
    tracing::info!("Metrics TSDB ingestion + rollup/purge tasks spawned");
    racecontrol_crate::metrics_producers::spawn_metric_producers(state.clone(), metrics_tx);
    tracing::info!("Metrics producers spawned (ws_connections, game_sessions, pod_health, billing_revenue)");

    // Spawn error rate alerter task — sends to both James and Uday on error spikes
    if error_rate_email_enabled {
        let email_script = email_script_for_alerter;
        let recipients = vec![
            "james@racingpoint.in".to_string(),
            "usingh@racingpoint.in".to_string(),
        ];
        tokio::spawn(racecontrol_crate::error_rate::error_rate_alerter_task(email_alert_rx, email_script, recipients));
    }

    // Spawn WhatsApp P0 alerter task
    if state.config.alerting.enabled {
        let wa_state = state.clone();
        tokio::spawn(racecontrol_crate::whatsapp_alerter::whatsapp_alerter_task(
            wa_state,
            wa_alert_rx,
        ));
    }

    // Spawn metric alert evaluation task
    if !state.config.alert_rules.is_empty() {
        let alert_state = state.clone();
        tokio::spawn(racecontrol_crate::metric_alerts::metric_alert_task(alert_state));
        tracing::info!(target: "startup", "metric alert task spawned ({} rules)", state.config.alert_rules.len());
    }

    // Spawn policy engine evaluation task (Phase 299 — re-loads rules each cycle from DB)
    let policy_state = state.clone();
    tokio::spawn(racecontrol_crate::policy_engine::policy_engine_task(policy_state));

    // Spawn notification outbox worker (UX-01: durable retry with exponential backoff)
    {
        let notif_state = state.clone();
        tokio::spawn(racecontrol_crate::notification_outbox::notification_worker_task(notif_state));
    }

    // UX-08: Spawn virtual queue expire task (expires 'called' entries after 10 minutes, runs every 5 min)
    {
        let queue_db = state.db.clone();
        tokio::spawn(racecontrol_crate::api::routes::queue_expire_task(queue_db));
    }

    // Act 3: Auto-close stale visits (runs every 5 minutes, closes visits idle > 1 hour)
    {
        let visit_state = state.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(300));
            loop {
                interval.tick().await;
                racecontrol_crate::visits::auto_close_stale_visits(&visit_state).await;
            }
        });
    }

    // Phase 368 F-01: LaunchStateMachine prune task (every 60s).
    // Without this, prune() is never called and stale launch cards accumulate
    // until the 100-card LRU cap forces eviction; the STALE_AFTER_SECS window
    // becomes dead code. See crates/racecontrol/src/launch_state.rs:171.
    {
        let prune_state = state.clone();
        tokio::spawn(async move {
            tracing::info!("launch-state-prune task started (60s interval)");
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            loop {
                interval.tick().await;
                prune_state.launch_state_machine.prune().await;
            }
        });
    }

    // Spawn data retention background task (LEGAL-08: daily, 1-hour initial delay)
    // Anonymizes drivers inactive for > pii_inactive_months (default 24 months).
    // Financial records are never touched (Income Tax Act: 8-year retention).
    {
        let retention_state = state.clone();
        tokio::spawn(async move {
            racecontrol_crate::api::routes::spawn_data_retention_job(retention_state).await;
        });
    }

    // Phase 365: AI behavior MMA batch (weekly)
    {
        let ai_batch_state = state.clone();
        tokio::spawn(async move {
            racecontrol_crate::ai_behavior_batch::spawn_ai_behavior_batch(ai_batch_state).await;
        });
    }

    // Spawn daily staff PIN rotation (10:00 AM IST every day)
    spawn_staff_pin_rotation(state.clone());

    // Spawn billing background tasks
    spawn_billing_tasks(state.clone());

    // Spawn game health check loop (5 second interval)
    let game_state = state.clone();
    tokio::spawn(async move {
        tracing::info!("game-health-check task started (5s interval)");
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        loop {
            interval.tick().await;
            game_launcher::check_game_health(&game_state).await;
        }
    });

    // Spawn AC server health check loop (5 second interval)
    let ac_state = state.clone();
    tokio::spawn(async move {
        tracing::info!("ac-server-health task started (5s interval)");
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        loop {
            interval.tick().await;
            ac_server::check_ac_server_health(&ac_state).await;
        }
    });

    // Spawn auth token expiry loop (30 second interval)
    let auth_state = state.clone();
    tokio::spawn(async move {
        tracing::info!("auth-token-expiry task started (30s interval)");
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            auth::expire_stale_tokens(&auth_state).await;
        }
    });

    // Spawn pod reservation expiry loop (30 second interval)
    let res_state = state.clone();
    tokio::spawn(async move {
        tracing::info!("pod-reservation-expiry task started (30s interval)");
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            pod_reservation::expire_idle_reservations(&res_state).await;
        }
    });

    // RESIL-09: Spawn HashMap eviction task (60 second interval)
    spawn_hashmap_eviction(state.clone());

    // Spawn camera control tick loop (2 second interval)
    let cam_state = state.clone();
    tokio::spawn(async move {
        tracing::info!("camera-control-tick task started (2s interval)");
        let mut interval = tokio::time::interval(Duration::from_secs(2));
        loop {
            interval.tick().await;
            ac_camera::tick(&cam_state).await;
        }
    });

    // Spawn proactive error pattern detection
    error_aggregator::spawn(state.clone());

    // Spawn cloud sync (pulls customer data from cloud racecontrol)
    cloud_sync::spawn(state.clone());

    // v29.0 Phase 35: Spawn data collector + RUL threshold checks (15-min interval)
    if let Some(telem_pool) = state.telemetry_db.clone() {
        racecontrol_crate::data_collector::spawn_data_collector(state.db.clone(), telem_pool);
    } else {
        tracing::warn!("Data collector skipped — telemetry DB not initialized");
    }

    // Spawn Bono relay (pushes events to Bono's VPS over Tailscale mesh)
    bono_relay::spawn(state.clone());

    // Spawn remote terminal (polls cloud for commands to execute locally)
    remote_terminal::spawn(state.clone());

    // Spawn action queue (polls cloud for pending actions — bookings, wallet, QR, etc.)
    action_queue::spawn(state.clone());

    // Spawn pod monitor (Tier 2: detect stale pods, auto-restart via pod-agent)
    pod_monitor::spawn(state.clone());

    // Spawn pod healer (Tier 3: deep diagnostics, auto-fix zombies, AI escalation)
    pod_healer::spawn(state.clone());

    // Spawn smart scheduler (auto-wake/shutdown pods, peak hour tracking)
    scheduler::spawn(state.clone());

    // Spawn SQLite backup pipeline (hourly VACUUM INTO, rotation, staleness alert)
    backup_pipeline::spawn(state.clone());
    tracing::info!(target: "startup", "backup pipeline spawned");

    // Spawn event archive pipeline (hourly tick: JSONL export, 90-day purge, nightly SCP)
    event_archive::spawn(state.clone());
    tracing::info!(target: "startup", "event_archive pipeline spawned");

    // Spawn psychology notification dispatcher (drains nudge_queue, routes to channels)
    psychology::spawn_dispatcher(state.clone());

    // Spawn UDP heartbeat listener (fast liveness detection alongside WebSocket)
    udp_heartbeat::spawn(state.clone());

    // Spawn fleet health probe loop (15s interval, HTTP :8090/health on each registered pod)
    fleet_health::start_probe_loop(state.clone());

    // Phase 366 GLD-F-03: Content drift detector (60-min interval, compares TOML vs live disk)
    racecontrol_crate::content_drift::spawn_content_drift_task(state.clone());

    // Spawn deployment awareness (60s interval, fleet version consistency + crash detection)
    deploy_awareness::spawn(state.clone());

    // Spawn venue state monitor (60s interval, ping-based venue open detection)
    racecontrol_crate::venue_state::spawn();

    // Spawn app health monitor (30s interval, probes admin/kiosk/web health endpoints)
    app_health_monitor::spawn(state.clone());

    // Spawn subsystem health probes (10s interval, per-subsystem ground truth for /api/v1/health)
    racecontrol_crate::subsystem_health::spawn(state.clone());

    // Spawn log sync task (daily rsync of JSONL logs to Bono VPS, OPS-07)
    racecontrol_crate::log_sync::spawn(state.clone());

    // Spawn synthetic transaction monitor (5min interval, golden-path API validation)
    racecontrol_crate::synthetic_monitor::spawn(state.clone());

    // Spawn Meshed Intelligence promotion pipeline (60s: promote candidates, detect patterns, expire stale)
    racecontrol_crate::promotion::spawn(state.clone());

    // Spawn server self-diagnostics (MMA consensus: WS drift, split-brain, DB health — 60s interval)
    racecontrol_crate::server_diagnostics::spawn(state.clone());

    // Spawn server-side process guard (monitors server .23 for unauthorized processes)
    process_guard::spawn_server_guard(state.clone());

    // Start server_ops HTTP endpoint on :8090 (remote command execution, file ops)
    server_ops::start();
}

/// Spawns all billing-related background tasks.
fn spawn_billing_tasks(state: Arc<AppState>) {
    // Spawn billing tick loop (1 second interval, refresh rates every 60s)
    // MMA-Iter3: Wrap in restart loop so panics don't silently kill billing
    let tick_state = state.clone();
    tokio::spawn(async move {
        tracing::info!("billing-tick task started (1s interval)");
        loop {
            let state = tick_state.clone();
            let handle = tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(1));
                let mut refresh_counter: u32 = 0;
                loop {
                    interval.tick().await;
                    billing::tick_all_timers(&state).await;
                    refresh_counter += 1;
                    if refresh_counter >= 60 {
                        refresh_counter = 0;
                        billing::refresh_rate_tiers(&state).await;
                    }
                }
            });
            if let Err(e) = handle.await {
                tracing::error!("CRITICAL: billing-tick task panicked: {:?} — restarting in 1s", e);
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    });

    // Spawn billing DB sync loop (5 second interval)
    let sync_state = state.clone();
    tokio::spawn(async move {
        tracing::info!("billing-db-sync task started (5s interval)");
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        loop {
            interval.tick().await;
            billing::sync_timers_to_db(&sync_state).await;
        }
    });

    // Spawn staggered timer persistence loop (RESIL-02 + FSM-09)
    // Each pod persists elapsed_seconds at a different second offset within the minute:
    // Pod 1 at :07, Pod 2 at :14, Pod 3 at :21, Pod 4 at :28,
    // Pod 5 at :35, Pod 6 at :42, Pod 7 at :49, Pod 8 at :56
    // Formula: Pod N writes at second (N * 7) % 60
    let persist_state = state.clone();
    tokio::spawn(async move {
        tracing::info!("timer-persist task started (60s staggered by pod index)");
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        let mut second_counter: u64 = 0;
        loop {
            interval.tick().await;
            second_counter += 1;
            let second_in_minute = second_counter % 60;

            // Check if any pod should write this second
            for pod_num in 1u32..=8 {
                if (pod_num as u64 * 7) % 60 == second_in_minute {
                    billing::persist_timer_state(&persist_state, Some(pod_num)).await;
                }
            }
        }
    });

    // Spawn orphan detection background task (RESIL-03: every 5 minutes)
    let orphan_state = state.clone();
    tokio::spawn(async move {
        tracing::info!("orphan-detector task started (300s interval)");
        // Initial delay: wait 5 minutes before first background check
        // (startup check already ran — avoid duplicate alerts for same orphans)
        tokio::time::sleep(Duration::from_secs(300)).await;
        let mut interval = tokio::time::interval(Duration::from_secs(300));
        loop {
            interval.tick().await;
            billing::detect_orphaned_sessions_background(&orphan_state).await;
        }
    });

    // Spawn wallet reconciliation background task (FATM-12: every 30 minutes)
    billing::spawn_reconciliation_job(state.clone());

    // BILL-03: Spawn PWA game request TTL cleanup task (every 60 seconds)
    billing::spawn_cleanup_expired_game_requests(state.clone());

    // FATM-08: Spawn coupon TTL expiry task (every 60s, 120s initial delay)
    billing::spawn_coupon_ttl_expiry_job(state.clone());
}

/// Spawns the daily staff PIN rotation task (10:00 AM IST every day).
fn spawn_staff_pin_rotation(state: Arc<AppState>) {
    use rand::Rng;

    let pin_state = state;
    tokio::spawn(async move {
        loop {
            // Compute delay until next 10:00 AM IST (UTC+5:30 = 04:30 UTC)
            let now = chrono::Utc::now();
            let today_target = now.date_naive().and_hms_opt(4, 30, 0).unwrap();
            let target = if now.naive_utc() < today_target {
                today_target
            } else {
                today_target + chrono::Duration::days(1)
            };
            let delay = (target - now.naive_utc()).to_std().unwrap_or(std::time::Duration::from_secs(3600));
            tracing::info!("staff-pin-rotation: next run in {}s", delay.as_secs());
            tokio::time::sleep(delay).await;

            // Generate new 4-digit PINs for all active staff
            let staff = sqlx::query_as::<_, (String, String, String)>(
                "SELECT id, name, phone FROM staff_members WHERE is_active = 1",
            )
            .fetch_all(&pin_state.db)
            .await
            .unwrap_or_default();

            for (id, name, phone) in &staff {
                let new_pin = format!("{:04}", rand::thread_rng().gen_range(1000u32..=9999));
                if let Err(e) = sqlx::query("UPDATE staff_members SET pin = ?, updated_at = datetime('now') WHERE id = ?")
                    .bind(&new_pin)
                    .bind(id)
                    .execute(&pin_state.db)
                    .await
                {
                    tracing::error!("staff-pin-rotation: failed to update PIN for {}: {}", name, e);
                    continue;
                }

                let msg = format!(
                    "Racing Point - Your new staff PIN for today is: {}\nValid until tomorrow 10 AM.",
                    new_pin
                );
                racecontrol_crate::whatsapp_alerter::send_whatsapp_to(&pin_state.config, phone, &msg).await;
                tracing::info!("staff-pin-rotation: rotated PIN for {} ({})", name, id);
            }

            if !staff.is_empty() {
                tracing::info!("staff-pin-rotation: rotated PINs for {} staff members", staff.len());
            }
        }
    });
}

/// RESIL-09: Spawn HashMap eviction task (60 second interval).
/// Evicts stale entries from pending_ws_execs, pending_command_acks, and
/// chain_failure_tracker that accumulate from crashed pods or timed-out operations.
fn spawn_hashmap_eviction(state: Arc<AppState>) {
    let evict_state = state;
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            // Evict closed oneshot senders from pending_ws_execs
            {
                let mut execs = evict_state.pending_ws_execs.write().await;
                let before = execs.len();
                execs.retain(|_, sender| !sender.is_closed());
                let evicted = before - execs.len();
                if evicted > 0 {
                    tracing::debug!("RESIL-09: evicted {} stale pending_ws_execs entries", evicted);
                }
            }
            // Evict closed oneshot senders from pending_command_acks
            {
                let mut acks = evict_state.pending_command_acks.write().await;
                let before = acks.len();
                acks.retain(|_, sender| !sender.is_closed());
                let evicted = before - acks.len();
                if evicted > 0 {
                    tracing::debug!("RESIL-09: evicted {} stale pending_command_acks entries", evicted);
                }
            }
        }
    });
}
