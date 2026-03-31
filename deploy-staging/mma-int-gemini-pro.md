Excellent. This is the integration audit for v29.0 Meshed Intelligence. Based on the provided code, I have identified several integration findings across the new phases.

Here are my findings:

---
### Finding 1: Critical State Not Initialized for Self-Healing

-   **Severity**: P1
-   **File**: `main.rs` (conceptual omission, fix location would be `state.rs` and `main.rs`)
-   **What's wrong**: The new Self-Healing module (`self_healing.rs`, Phase 27) introduces a `PodAvailabilityMap` to track the real-time operational status of each pod (Available, Degraded, Unavailable). This state is essential for the new public API endpoint `/api/v1/pods/{id}/availability`, which allows the kiosk and other frontends to prevent users from selecting a faulty pod. However, this critical state map is **never initialized or stored in `AppState`**. The `self_healing::new_availability_map()` function is never called, and the resulting map is not added to the shared application state. This renders the entire pod availability feature and its dependent API endpoint non-functional.
-   **Concrete fix**:
    1.  Add the `PodAvailabilityMap` to the `AppState` struct (in `state.rs`).
        ```rust
        // in racecontrol_crate/src/state.rs
        pub struct AppState {
            // ... existing fields
            pub pod_availability: crate::self_healing::PodAvailabilityMap,
        }
        ```
    2.  Initialize the map in `AppState::new` and store it.
        ```rust
        // in racecontrol_crate/src/state.rs
        impl AppState {
            pub fn new(config: Config, db: Pool<Sqlite>, field_cipher: FieldCipher) -> Self {
                // ...
                let pod_availability = crate::self_healing::new_availability_map();
                Self {
                    // ...
                    pod_availability,
                    // ...
                }
            }
        }
        ```
    3.  Ensure the API handler for `/pods/{id}/availability` reads from `state.pod_availability`.

---
### Finding 2: AI Data Snapshot Collected but Immediately Discarded

-   **Severity**: P2
-   **File**: `data_collector.rs:141`
-   **What's wrong**: The Unified Data Collector (Phase 35) is designed to create a `VenueSnapshot` for "AI consumption". The `spawn_data_collector` task correctly calls `collect_venue_snapshot` every 15 minutes. However, the returned `snapshot` object, containing valuable aggregated data, is only used in a debug log statement and then immediately dropped. The primary data product of this module is never stored, broadcast, or sent to an AI service, breaking the entire data flow for this feature.
-   **Concrete fix**: The collected `snapshot` must be consumed. This could involve storing it in a new database table for historical analysis, sending it to an AI/ML endpoint, or broadcasting it over the `/ws/ai` WebSocket.
    Example fix (sending over WebSocket):
    ```rust
    // in racecontrol_crate/src/data_collector.rs:141
            interval.tick().await;
            let snapshot = collect_venue_snapshot(&pool).await;
            tracing::debug!(target: LOG_TARGET,
                revenue = snapshot.revenue_today_paise,
                tasks = snapshot.open_maintenance_tasks,
                alerts = snapshot.critical_alerts_active,
                "Venue snapshot collected"
            );

            // FIX: Broadcast the snapshot for AI client consumption.
            // This requires adding the ai_tx broadcast sender to AppState
            // and passing it into the spawner.
            // if let Some(ai_tx) = &state.ai_tx {
            //     let _ = ai_tx.send(snapshot);
            // }

            if let Err(e) = check_rul_thresholds(&pool, &telem_pool).await {
                // ...
    ```

---
### Finding 3: AI Data Snapshot Populated with Hardcoded Placeholder Data

-   **Severity**: P2
    **File**: `data_collector.rs:72-78`
-   **What's wrong**: The `collect_venue_snapshot` function, which is supposed to provide a real-time overview of the venue, is populated with hardcoded, incorrect data. Fields like `pod_count_online`, `active_sessions`, `occupancy_pct`, `avg_gpu_temp`, and `avg_network_latency` are all set to static default values (e.g., `8`, `0`, `0.0`, `None`) instead of being queried from their true sources (fleet health state, billing FSM, telemetry DB). `// TODO` comments in the code confirm this is incomplete. This makes the snapshot data misleading and useless for any diagnostic or AI purpose.
-   **Concrete fix**: Replace hardcoded values with actual queries to the relevant data sources, such as the billing FSM for active sessions and the telemetry database for average GPU temperature.
    ```rust
    // in racecontrol_crate/src/data_collector.rs
    pub async fn collect_venue_snapshot(state: &Arc<AppState>) -> VenueSnapshot {
        // ... (existing queries for revenue, tasks etc.)

        // FIX: Query real data sources instead of using hardcoded values.
        let live_pods = state.get_live_pod_counts().await; // Assumes state method exists
        let active_sessions_count = state.get_active_session_count().await; // Assumes state method exists
        let telemetry_summary = crate::telemetry_store::get_telemetry_summary(&state.telemetry_db).await;

        VenueSnapshot {
            timestamp: Utc::now(),
            pod_count_online: live_pods.online,
            pod_count_degraded: live_pods.degraded,
            pod_count_unavailable: live_pods.unavailable,
            active_sessions: active_sessions_count,
            occupancy_pct: (active_sessions_count as f32 / live_pods.online as f32) * 100.0,
            revenue_today_paise: revenue,
            open_maintenance_tasks: open_tasks as u32,
            critical_alerts_active: critical_alerts as u32,
            staff_on_duty: staff as u32,
            avg_gpu_temp: telemetry_summary.avg_gpu_temp,
            avg_network_latency: telemetry_summary.avg_latency,
        }
    }
    ```

---
### Finding 4: Critical Business Alerts Are Generated and Ignored

-   **Severity**: P2
-   **File**: `alert_engine.rs:125`
-   **What's wrong**: The Business Alert Engine (Phase 30) is spawned to monitor financial KPIs. The `check_business_alerts` function correctly queries metrics and creates a vector of `BusinessAlert` structs for issues like revenue drops. However, in the `spawn_alert_checker` loop, the return value from this function is explicitly ignored with `let _alerts = ...`. A `// TODO` comment further confirms that the alerts are never passed to the WhatsApp alerter or any other notification system. Consequently, critical business alerts are silently dropped.
-   **Concrete fix**: Wire the generated alerts to the appropriate notification channel(s), such as the existing `whatsapp_alerter`. This requires passing the application state (or a channel sender) into the spawner.
    ```rust
    // in racecontrol_crate/src/alert_engine.rs
    // Modify `spawn_alert_checker` to accept AppState
    pub fn spawn_alert_checker(state: Arc<AppState>) {
        tokio::spawn(async move {
            // ...
            loop {
                interval.tick().await;
                // FIX: Use the alerts instead of ignoring them.
                let alerts = check_business_alerts(&state.db).await;
                for alert in alerts {
                    if matches!(alert.channel, AlertChannel::WhatsApp | AlertChannel::Both) {
                         let _ = crate::whatsapp_alerter::send_wa_alert(
                            &state,
                            &alert.alert_type,
                            &alert.message,
                        ).await;
                    }
                    // Also dispatch to dashboard if needed via state.dashboard_tx
                }
            }
        });
    }
    ```
    And update the spawner call in `main.rs:608` to pass the state.

---
### Finding 5: Non-Atomic Read-Modify-Write in Business Aggregator

-   **Severity**: P3
-   **File**: `business_aggregator.rs:77-101`
-   **What's wrong**: The `aggregate_daily_revenue` function, which runs hourly, calculates and saves daily business metrics. It uses a non-atomic `read-modify-write` pattern: it first reads the existing metrics for the day from the DB, modifies them in Rust code to preserve expense data, and then writes the entire modified record back using `upsert_daily_metrics`. If this task were ever to run concurrently (e.g., manual trigger during an auto-run), a race condition could occur where one update overwrites another, leading to incorrect financial metrics.
-   **Concrete fix**: The update logic should be performed atomically in a single SQL statement using `INSERT ... ON CONFLICT ... DO UPDATE`, which is the standard and safe way to handle "upserts". This avoids the race condition entirely.
    ```rust
    // in racecontrol_crate/maintenance_store.rs (in `upsert_daily_metrics`)
    pub async fn upsert_daily_metrics(pool: &SqlitePool, date: &str, metrics: &DailyBusinessMetrics) -> anyhow::Result<()> {
        // FIX: Use an atomic ON CONFLICT statement.
        sqlx::query(
            "INSERT INTO daily_business_metrics (date, revenue_gaming_paise, revenue_cafe_paise, sessions_count, occupancy_rate_pct, peak_occupancy_pct)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(date) DO UPDATE SET
                revenue_gaming_paise = excluded.revenue_gaming_paise,
                revenue_cafe_paise = excluded.revenue_cafe_paise,
                sessions_count = excluded.sessions_count,
                occupancy_rate_pct = excluded.occupancy_rate_pct,
                peak_occupancy_pct = MAX(peak_occupancy_pct, excluded.peak_occupancy_pct)"
        )
        .bind(date)
        .bind(metrics.revenue_gaming_paise)
        .bind(metrics.revenue_cafe_paise)
        .bind(metrics.sessions_count)
        .bind(metrics.occupancy_rate_pct)
        .bind(metrics.peak_occupancy_pct)
        .execute(pool)
        .await?;
        Ok(())
    }
    ```

---
### Finding 6: Extended Cloud Sync Logic is Never Called
- **Severity**: P3
- **File**: `cloud_sync.rs` (conceptual omission)
- **What's wrong**: A new function `sync_maintenance_data` was added (Phase 33) to collect and prepare maintenance, HR, and business data for synchronization to the cloud. However, the main `cloud_sync::spawn` function, which is called in `main.rs` to start the background sync task, has not been updated to call this new function. As a result, the new data collection logic runs, but the data is never integrated into the actual sync process and will not be uploaded.
- **Concrete Fix**: Modify the main loop inside `cloud_sync::spawn` to call `sync_maintenance_data` and incorporate its results into the payload that is sent to the cloud.
    ```rust
    // in racecontrol_crate/src/cloud_sync.rs inside spawn()
    let sync_state = state.clone();
    tokio::spawn(async move {
        // ... (existing spawn logic with interval)
        loop {
            interval.tick().await;
            // ... (existing sync logic)

            // FIX: Call the new sync function within the loop.
            if let Err(e) = sync_maintenance_data(&sync_state.db, &cloud_url).await {
                tracing::warn!(target: "cloud-sync", "Failed to collect extended maintenance data: {e}");
            }
            // Add the collected data to the payload before sending.
        }
    });

    ```