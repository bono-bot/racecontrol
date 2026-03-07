//! Cloud sync: pull customer data from cloud rc-core to local.
//!
//! Runs as a background task on the local instance.
//! Calls GET /api/v1/sync/changes?since=<last_sync> on the cloud.
//! Upserts received records into local SQLite.

use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;

use crate::state::AppState;

const SYNC_TABLES: &str = "drivers,wallets,pricing_tiers,pricing_rules,kiosk_experiences,kiosk_settings";

/// Spawn the cloud sync background task.
/// Only starts if cloud.enabled = true and cloud.api_url is set.
pub fn spawn(state: Arc<AppState>) {
    let cloud = &state.config.cloud;
    if !cloud.enabled {
        tracing::info!("Cloud sync disabled");
        return;
    }

    let api_url = match &cloud.api_url {
        Some(url) => url.clone(),
        None => {
            tracing::warn!("Cloud sync enabled but no api_url configured");
            return;
        }
    };

    let interval_secs = cloud.sync_interval_secs;
    tracing::info!(
        "Cloud sync enabled: {} (every {}s)",
        api_url,
        interval_secs
    );

    tokio::spawn(async move {
        // Wait 5s on startup before first sync
        tokio::time::sleep(Duration::from_secs(5)).await;

        let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
        loop {
            interval.tick().await;
            if let Err(e) = sync_once(&state, &api_url).await {
                tracing::error!("Cloud sync failed: {}", e);
            }
        }
    });
}

/// Perform a single sync cycle.
async fn sync_once(state: &Arc<AppState>, cloud_url: &str) -> anyhow::Result<()> {
    let last_synced = get_last_sync_time(state).await;

    let url = format!("{}/sync/changes", cloud_url);

    tracing::debug!("Cloud sync: fetching since {}", last_synced);

    let mut req = state
        .http_client
        .get(&url)
        .query(&[
            ("since", last_synced.as_str()),
            ("tables", SYNC_TABLES),
        ])
        .timeout(Duration::from_secs(15));

    // Send terminal secret for authentication
    if let Some(secret) = &state.config.cloud.terminal_secret {
        req = req.header("x-terminal-secret", secret);
    }

    let resp = req.send().await?;

    if !resp.status().is_success() {
        anyhow::bail!("Cloud returned status {}", resp.status());
    }

    let body: Value = resp.json().await?;
    let mut total_upserted = 0u64;

    // Upsert drivers
    if let Some(drivers) = body.get("drivers").and_then(|v| v.as_array()) {
        for driver in drivers {
            if let Err(e) = upsert_driver(state, driver).await {
                tracing::warn!("Failed to upsert driver: {}", e);
            } else {
                total_upserted += 1;
            }
        }
    }

    // Upsert wallets
    if let Some(wallets) = body.get("wallets").and_then(|v| v.as_array()) {
        for wallet in wallets {
            if let Err(e) = upsert_wallet(state, wallet).await {
                tracing::warn!("Failed to upsert wallet: {}", e);
            } else {
                total_upserted += 1;
            }
        }
    }

    // Upsert pricing_tiers
    if let Some(tiers) = body.get("pricing_tiers").and_then(|v| v.as_array()) {
        for tier in tiers {
            if let Err(e) = upsert_pricing_tier(state, tier).await {
                tracing::warn!("Failed to upsert pricing tier: {}", e);
            } else {
                total_upserted += 1;
            }
        }
    }

    // Upsert pricing_rules (dynamic pricing multipliers)
    if let Some(rules) = body.get("pricing_rules").and_then(|v| v.as_array()) {
        for rule in rules {
            if let Err(e) = upsert_pricing_rule(state, rule).await {
                tracing::warn!("Failed to upsert pricing rule: {}", e);
            } else {
                total_upserted += 1;
            }
        }
    }

    // Upsert kiosk_experiences
    if let Some(experiences) = body.get("kiosk_experiences").and_then(|v| v.as_array()) {
        for exp in experiences {
            if let Err(e) = upsert_kiosk_experience(state, exp).await {
                tracing::warn!("Failed to upsert kiosk experience: {}", e);
            } else {
                total_upserted += 1;
            }
        }
    }

    // Upsert kiosk_settings and broadcast to agents if changed
    if let Some(settings) = body.get("kiosk_settings").and_then(|v| v.as_object()) {
        let mut changed = false;
        for (key, value) in settings {
            let val_str = value.as_str().unwrap_or(&value.to_string()).to_string();
            let local = sqlx::query_as::<_, (String,)>(
                "SELECT value FROM kiosk_settings WHERE key = ?",
            )
            .bind(key)
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten();

            let needs_update = match &local {
                Some((v,)) => v != &val_str,
                None => true,
            };

            if needs_update {
                let _ = sqlx::query(
                    "INSERT INTO kiosk_settings (key, value) VALUES (?, ?)
                     ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                )
                .bind(key)
                .bind(&val_str)
                .execute(&state.db)
                .await;
                changed = true;
                total_upserted += 1;
            }
        }

        // Broadcast to connected agents so pods react immediately
        if changed {
            let settings_map: std::collections::HashMap<String, String> = settings
                .iter()
                .map(|(k, v)| (k.clone(), v.as_str().unwrap_or(&v.to_string()).to_string()))
                .collect();
            state.broadcast_settings(&settings_map).await;
            let agent_count = state.agent_senders.read().await.len();
            tracing::info!("Cloud sync: kiosk settings updated and broadcast to {} agents", agent_count);
        }
    }

    // Update sync timestamp
    let fallback_ts = chrono::Utc::now().to_rfc3339();
    let synced_at = body
        .get("synced_at")
        .and_then(|v| v.as_str())
        .unwrap_or(&fallback_ts);

    update_sync_state(state, synced_at, total_upserted).await;

    if total_upserted > 0 {
        tracing::info!("Cloud sync pull: upserted {} records", total_upserted);
    } else {
        tracing::debug!("Cloud sync pull: no new changes");
    }

    // Phase 2: Push venue data to cloud
    if let Err(e) = push_to_cloud(state, cloud_url).await {
        tracing::error!("Cloud sync push failed: {}", e);
    }

    Ok(())
}

/// Push venue-generated data (laps, billing, pods, leaderboard) to cloud.
async fn push_to_cloud(state: &Arc<AppState>, cloud_url: &str) -> anyhow::Result<()> {
    let last_push = get_last_push_time(state).await;
    let mut payload = serde_json::json!({});
    let mut has_data = false;

    // Collect laps since last push
    let laps = sqlx::query_as::<_, (String,)>(
        "SELECT json_object(
            'id', id, 'session_id', session_id, 'driver_id', driver_id,
            'pod_id', pod_id, 'sim_type', sim_type, 'track', track, 'car', car,
            'lap_number', lap_number, 'lap_time_ms', lap_time_ms,
            'sector1_ms', sector1_ms, 'sector2_ms', sector2_ms, 'sector3_ms', sector3_ms,
            'valid', valid, 'created_at', created_at
        ) FROM laps WHERE created_at > ? ORDER BY created_at ASC LIMIT 500",
    )
    .bind(&last_push)
    .fetch_all(&state.db)
    .await?;

    if !laps.is_empty() {
        let items: Vec<serde_json::Value> = laps.iter()
            .filter_map(|r| serde_json::from_str(&r.0).ok())
            .collect();
        tracing::info!("Cloud sync push: {} laps", items.len());
        payload["laps"] = serde_json::json!(items);
        has_data = true;
    }

    // Collect track records (always push all — small table)
    let records = sqlx::query_as::<_, (String,)>(
        "SELECT json_object(
            'track', track, 'car', car, 'driver_id', driver_id,
            'best_lap_ms', best_lap_ms, 'lap_id', lap_id, 'achieved_at', achieved_at
        ) FROM track_records",
    )
    .fetch_all(&state.db)
    .await?;

    if !records.is_empty() {
        let items: Vec<serde_json::Value> = records.iter()
            .filter_map(|r| serde_json::from_str(&r.0).ok())
            .collect();
        payload["track_records"] = serde_json::json!(items);
        has_data = true;
    }

    // Collect personal bests (always push all — small table)
    let pbs = sqlx::query_as::<_, (String,)>(
        "SELECT json_object(
            'driver_id', driver_id, 'track', track, 'car', car,
            'best_lap_ms', best_lap_ms, 'lap_id', lap_id, 'achieved_at', achieved_at
        ) FROM personal_bests",
    )
    .fetch_all(&state.db)
    .await?;

    if !pbs.is_empty() {
        let items: Vec<serde_json::Value> = pbs.iter()
            .filter_map(|r| serde_json::from_str(&r.0).ok())
            .collect();
        payload["personal_bests"] = serde_json::json!(items);
        has_data = true;
    }

    // Collect billing sessions since last push
    let sessions = sqlx::query_as::<_, (String,)>(
        "SELECT json_object(
            'id', id, 'driver_id', driver_id, 'pod_id', pod_id,
            'pricing_tier_id', pricing_tier_id, 'allocated_seconds', allocated_seconds,
            'driving_seconds', driving_seconds, 'status', status,
            'custom_price_paise', custom_price_paise, 'notes', notes,
            'started_at', started_at, 'ended_at', ended_at, 'created_at', created_at,
            'experience_id', experience_id, 'car', car, 'track', track, 'sim_type', sim_type
        ) FROM billing_sessions WHERE created_at > ? OR ended_at > ?
        ORDER BY created_at ASC LIMIT 500",
    )
    .bind(&last_push)
    .bind(&last_push)
    .fetch_all(&state.db)
    .await?;

    if !sessions.is_empty() {
        let items: Vec<serde_json::Value> = sessions.iter()
            .filter_map(|r| serde_json::from_str(&r.0).ok())
            .collect();
        tracing::info!("Cloud sync push: {} billing sessions", items.len());
        payload["billing_sessions"] = serde_json::json!(items);
        has_data = true;
    }

    // Push driver changes (has_used_trial, total_laps, total_time_ms, registration)
    let drivers = sqlx::query_as::<_, (String,)>(
        "SELECT json_object(
            'id', id, 'has_used_trial', COALESCE(has_used_trial, 0),
            'total_laps', COALESCE(total_laps, 0),
            'total_time_ms', COALESCE(total_time_ms, 0),
            'registration_completed', COALESCE(registration_completed, 0),
            'waiver_signed', COALESCE(waiver_signed, 0),
            'waiver_signed_at', waiver_signed_at,
            'waiver_version', waiver_version,
            'updated_at', updated_at
        ) FROM drivers WHERE updated_at > ?
        ORDER BY updated_at ASC LIMIT 500",
    )
    .bind(&last_push)
    .fetch_all(&state.db)
    .await?;

    if !drivers.is_empty() {
        let items: Vec<serde_json::Value> = drivers.iter()
            .filter_map(|r| serde_json::from_str(&r.0).ok())
            .collect();
        tracing::info!("Cloud sync push: {} driver updates", items.len());
        payload["drivers"] = serde_json::json!(items);
        has_data = true;
    }

    // Push live pod status from in-memory state
    let pods = state.pods.read().await;
    if !pods.is_empty() {
        let pod_list: Vec<serde_json::Value> = pods.values().map(|p| {
            serde_json::json!({
                "id": p.id,
                "number": p.number,
                "name": p.name,
                "ip_address": p.ip_address,
                "mac_address": p.mac_address,
                "sim_type": p.sim_type,
                "status": p.status,
                "game_state": p.game_state,
                "current_game": p.current_game,
                "current_driver": p.current_driver,
                "current_session_id": p.current_session_id,
                "billing_session_id": p.billing_session_id,
            })
        }).collect();
        payload["pods"] = serde_json::json!(pod_list);
        has_data = true;
    }
    drop(pods);

    // Push wallet balances (venue is authoritative for debits)
    // Include driver phone/email so cloud can match by identity when IDs differ
    let wallets = sqlx::query_as::<_, (String,)>(
        "SELECT json_object(
            'driver_id', w.driver_id, 'balance_paise', w.balance_paise,
            'total_credited_paise', w.total_credited_paise,
            'total_debited_paise', w.total_debited_paise,
            'updated_at', w.updated_at,
            'phone', d.phone, 'email', d.email
        ) FROM wallets w JOIN drivers d ON d.id = w.driver_id
        WHERE w.updated_at > ?",
    )
    .bind(&last_push)
    .fetch_all(&state.db)
    .await?;

    if !wallets.is_empty() {
        let items: Vec<serde_json::Value> = wallets.iter()
            .filter_map(|r| serde_json::from_str(&r.0).ok())
            .collect();
        tracing::info!("Cloud sync push: {} wallets", items.len());
        payload["wallets"] = serde_json::json!(items);
        has_data = true;
    }

    if !has_data {
        tracing::debug!("Cloud sync push: nothing to push");
        return Ok(());
    }

    // POST to cloud
    let push_url = format!("{}/sync/push", cloud_url);
    let mut req = state.http_client
        .post(&push_url)
        .json(&payload)
        .timeout(std::time::Duration::from_secs(30));

    if let Some(secret) = &state.config.cloud.terminal_secret {
        req = req.header("x-terminal-secret", secret);
    }

    let resp = req.send().await?;
    if !resp.status().is_success() {
        anyhow::bail!("Cloud push returned status {}", resp.status());
    }

    let result: serde_json::Value = resp.json().await?;
    let upserted = result.get("upserted").and_then(|v| v.as_u64()).unwrap_or(0);

    if upserted > 0 {
        tracing::info!("Cloud sync push: cloud accepted {} records", upserted);
    }

    // Update push timestamp
    update_push_state(state).await;

    Ok(())
}

async fn get_last_push_time(state: &Arc<AppState>) -> String {
    let row = sqlx::query_as::<_, (String,)>(
        "SELECT last_synced_at FROM sync_state WHERE table_name = '_push'",
    )
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    row.map(|r| r.0)
        .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string())
}

async fn update_push_state(state: &Arc<AppState>) {
    let now = chrono::Utc::now().to_rfc3339();
    let _ = sqlx::query(
        "INSERT INTO sync_state (table_name, last_synced_at, last_sync_count, updated_at)
         VALUES ('_push', ?, 0, datetime('now'))
         ON CONFLICT(table_name) DO UPDATE SET
            last_synced_at = excluded.last_synced_at,
            updated_at = datetime('now')",
    )
    .bind(&now)
    .execute(&state.db)
    .await;
}

async fn get_last_sync_time(state: &Arc<AppState>) -> String {
    let row = sqlx::query_as::<_, (String,)>(
        "SELECT MIN(last_synced_at) FROM sync_state",
    )
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    row.map(|r| r.0)
        .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string())
}

async fn update_sync_state(state: &Arc<AppState>, synced_at: &str, count: u64) {
    for table in SYNC_TABLES.split(',') {
        let _ = sqlx::query(
            "INSERT INTO sync_state (table_name, last_synced_at, last_sync_count, updated_at)
             VALUES (?, ?, ?, datetime('now'))
             ON CONFLICT(table_name) DO UPDATE SET
                last_synced_at = excluded.last_synced_at,
                last_sync_count = excluded.last_sync_count,
                updated_at = datetime('now')",
        )
        .bind(table)
        .bind(synced_at)
        .bind(count as i64)
        .execute(&state.db)
        .await;
    }
}

async fn upsert_driver(state: &Arc<AppState>, driver: &Value) -> anyhow::Result<()> {
    let id = driver
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Driver missing id"))?;

    // Check if local row exists and compare updated_at
    let local_updated = sqlx::query_as::<_, (Option<String>,)>(
        "SELECT updated_at FROM drivers WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?;

    let cloud_updated = driver
        .get("updated_at")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // Skip if local is newer or equal
    if let Some((Some(ref local_ts),)) = local_updated {
        if local_ts.as_str() >= cloud_updated {
            return Ok(());
        }
    }

    // Upsert — cloud wins for customer-owned fields, preserve local-only fields (otp_code etc.)
    sqlx::query(
        "INSERT INTO drivers (id, customer_id, name, email, phone, steam_guid, iracing_id, avatar_url,
            total_laps, total_time_ms, has_used_trial, pin_hash, phone_verified,
            dob, waiver_signed, waiver_signed_at, waiver_version,
            guardian_name, guardian_phone, registration_completed, signature_data,
            created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23)
        ON CONFLICT(id) DO UPDATE SET
            customer_id = COALESCE(excluded.customer_id, drivers.customer_id),
            name = excluded.name,
            email = excluded.email,
            phone = excluded.phone,
            steam_guid = COALESCE(excluded.steam_guid, drivers.steam_guid),
            iracing_id = COALESCE(excluded.iracing_id, drivers.iracing_id),
            avatar_url = COALESCE(excluded.avatar_url, drivers.avatar_url),
            has_used_trial = excluded.has_used_trial,
            pin_hash = COALESCE(excluded.pin_hash, drivers.pin_hash),
            phone_verified = excluded.phone_verified,
            dob = excluded.dob,
            waiver_signed = excluded.waiver_signed,
            waiver_signed_at = excluded.waiver_signed_at,
            waiver_version = excluded.waiver_version,
            guardian_name = excluded.guardian_name,
            guardian_phone = excluded.guardian_phone,
            registration_completed = excluded.registration_completed,
            signature_data = COALESCE(excluded.signature_data, drivers.signature_data),
            updated_at = excluded.updated_at",
    )
    .bind(id)
    .bind(driver.get("customer_id").and_then(|v| v.as_str()))
    .bind(driver.get("name").and_then(|v| v.as_str()).unwrap_or("Unknown"))
    .bind(driver.get("email").and_then(|v| v.as_str()))
    .bind(driver.get("phone").and_then(|v| v.as_str()))
    .bind(driver.get("steam_guid").and_then(|v| v.as_str()))
    .bind(driver.get("iracing_id").and_then(|v| v.as_str()))
    .bind(driver.get("avatar_url").and_then(|v| v.as_str()))
    .bind(driver.get("total_laps").and_then(|v| v.as_i64()).unwrap_or(0))
    .bind(driver.get("total_time_ms").and_then(|v| v.as_i64()).unwrap_or(0))
    .bind(driver.get("has_used_trial").and_then(|v| v.as_i64()).unwrap_or(0))
    .bind(driver.get("pin_hash").and_then(|v| v.as_str()))
    .bind(driver.get("phone_verified").and_then(|v| v.as_i64()).unwrap_or(0))
    .bind(driver.get("dob").and_then(|v| v.as_str()))
    .bind(driver.get("waiver_signed").and_then(|v| v.as_i64()).unwrap_or(0))
    .bind(driver.get("waiver_signed_at").and_then(|v| v.as_str()))
    .bind(driver.get("waiver_version").and_then(|v| v.as_str()))
    .bind(driver.get("guardian_name").and_then(|v| v.as_str()))
    .bind(driver.get("guardian_phone").and_then(|v| v.as_str()))
    .bind(driver.get("registration_completed").and_then(|v| v.as_i64()).unwrap_or(0))
    .bind(driver.get("signature_data").and_then(|v| v.as_str()))
    .bind(driver.get("created_at").and_then(|v| v.as_str()))
    .bind(cloud_updated)
    .execute(&state.db)
    .await?;

    let name = driver.get("name").and_then(|v| v.as_str()).unwrap_or("?");
    tracing::debug!("Synced driver: {} ({})", name, id);

    Ok(())
}

async fn upsert_wallet(state: &Arc<AppState>, wallet: &Value) -> anyhow::Result<()> {
    let cloud_driver_id = wallet
        .get("driver_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Wallet missing driver_id"))?;

    let cloud_credited = wallet
        .get("total_credited_paise")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let cloud_balance = wallet
        .get("balance_paise")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let cloud_debited = wallet
        .get("total_debited_paise")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let cloud_updated = wallet
        .get("updated_at")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // Resolve the local driver_id — cloud and local may have different UUIDs
    // for the same person. Try direct match first, then phone, then email.
    let local_driver_id = {
        // Direct match: does this driver_id exist locally?
        let exists = sqlx::query_as::<_, (String,)>(
            "SELECT id FROM drivers WHERE id = ?",
        )
        .bind(cloud_driver_id)
        .fetch_optional(&state.db)
        .await?;

        if let Some((id,)) = exists {
            id
        } else {
            // ID mismatch — resolve by phone or email
            let phone = wallet.get("phone").and_then(|v| v.as_str()).unwrap_or("");
            let email = wallet.get("email").and_then(|v| v.as_str()).unwrap_or("");

            let resolved = if !phone.is_empty() {
                sqlx::query_as::<_, (String,)>(
                    "SELECT id FROM drivers WHERE phone = ?",
                )
                .bind(phone)
                .fetch_optional(&state.db)
                .await?
            } else {
                None
            };

            let resolved = if resolved.is_none() && !email.is_empty() {
                sqlx::query_as::<_, (String,)>(
                    "SELECT id FROM drivers WHERE email = ?",
                )
                .bind(email)
                .fetch_optional(&state.db)
                .await?
            } else {
                resolved
            };

            match resolved {
                Some((local_id,)) => {
                    tracing::info!(
                        "Wallet sync: resolved cloud driver {} → local {} via phone/email",
                        cloud_driver_id, local_id
                    );
                    local_id
                }
                None => {
                    tracing::debug!(
                        "Wallet sync: no local driver for cloud {} (phone={}, email={}), skipping",
                        cloud_driver_id,
                        wallet.get("phone").and_then(|v| v.as_str()).unwrap_or(""),
                        wallet.get("email").and_then(|v| v.as_str()).unwrap_or(""),
                    );
                    return Ok(());
                }
            }
        }
    };

    // Check if wallet exists locally for the resolved driver
    let local = sqlx::query_as::<_, (i64, i64, i64)>(
        "SELECT balance_paise, total_credited_paise, total_debited_paise FROM wallets WHERE driver_id = ?",
    )
    .bind(&local_driver_id)
    .fetch_optional(&state.db)
    .await?;

    match local {
        Some((_local_bal, _local_credited, _local_debited)) => {
            // Cloud is authoritative for wallet balance.
            // Topups happen on cloud/dashboard, debits happen at venue but are
            // pushed back to cloud via push_to_cloud(). Cloud balance already
            // reflects all transactions from both sides.
            sqlx::query(
                "UPDATE wallets SET
                    balance_paise = ?,
                    total_credited_paise = ?,
                    total_debited_paise = ?,
                    updated_at = ?
                 WHERE driver_id = ?",
            )
            .bind(cloud_balance)
            .bind(cloud_credited)
            .bind(cloud_debited)
            .bind(cloud_updated)
            .bind(&local_driver_id)
            .execute(&state.db)
            .await?;
        }
        None => {
            // Wallet doesn't exist locally — create it for the resolved driver
            sqlx::query(
                "INSERT OR IGNORE INTO wallets (driver_id, balance_paise, total_credited_paise, total_debited_paise, updated_at)
                 VALUES (?, ?, ?, ?, ?)",
            )
            .bind(&local_driver_id)
            .bind(cloud_balance)
            .bind(cloud_credited)
            .bind(cloud_debited)
            .bind(cloud_updated)
            .execute(&state.db)
            .await?;
        }
    }

    Ok(())
}

async fn upsert_pricing_tier(state: &Arc<AppState>, tier: &Value) -> anyhow::Result<()> {
    let id = tier
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Tier missing id"))?;

    sqlx::query(
        "INSERT INTO pricing_tiers (id, name, duration_minutes, price_paise, is_trial, is_active, sort_order, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(id) DO UPDATE SET
            name = excluded.name,
            duration_minutes = excluded.duration_minutes,
            price_paise = excluded.price_paise,
            is_trial = excluded.is_trial,
            is_active = excluded.is_active,
            sort_order = excluded.sort_order,
            updated_at = excluded.updated_at",
    )
    .bind(id)
    .bind(tier.get("name").and_then(|v| v.as_str()).unwrap_or("Unknown"))
    .bind(tier.get("duration_minutes").and_then(|v| v.as_i64()).unwrap_or(30))
    .bind(tier.get("price_paise").and_then(|v| v.as_i64()).unwrap_or(0))
    .bind(tier.get("is_trial").and_then(|v| v.as_i64()).unwrap_or(0))
    .bind(tier.get("is_active").and_then(|v| v.as_i64()).unwrap_or(1))
    .bind(tier.get("sort_order").and_then(|v| v.as_i64()).unwrap_or(0))
    .bind(tier.get("updated_at").and_then(|v| v.as_str()))
    .execute(&state.db)
    .await?;

    Ok(())
}

async fn upsert_kiosk_experience(state: &Arc<AppState>, exp: &Value) -> anyhow::Result<()> {
    let id = exp
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Experience missing id"))?;

    sqlx::query(
        "INSERT INTO kiosk_experiences (id, name, game, track, car, car_class, duration_minutes, start_type, ac_preset_id, sort_order, is_active, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
         ON CONFLICT(id) DO UPDATE SET
            name = excluded.name,
            game = excluded.game,
            track = excluded.track,
            car = excluded.car,
            car_class = excluded.car_class,
            duration_minutes = excluded.duration_minutes,
            start_type = excluded.start_type,
            ac_preset_id = excluded.ac_preset_id,
            sort_order = excluded.sort_order,
            is_active = excluded.is_active,
            updated_at = excluded.updated_at",
    )
    .bind(id)
    .bind(exp.get("name").and_then(|v| v.as_str()).unwrap_or("Unknown"))
    .bind(exp.get("game").and_then(|v| v.as_str()).unwrap_or("assetto_corsa"))
    .bind(exp.get("track").and_then(|v| v.as_str()).unwrap_or("spa"))
    .bind(exp.get("car").and_then(|v| v.as_str()).unwrap_or("ferrari_sf15t"))
    .bind(exp.get("car_class").and_then(|v| v.as_str()))
    .bind(exp.get("duration_minutes").and_then(|v| v.as_i64()).unwrap_or(30))
    .bind(exp.get("start_type").and_then(|v| v.as_str()).unwrap_or("pitlane"))
    .bind(exp.get("ac_preset_id").and_then(|v| v.as_str()))
    .bind(exp.get("sort_order").and_then(|v| v.as_i64()).unwrap_or(0))
    .bind(exp.get("is_active").and_then(|v| v.as_i64()).unwrap_or(1))
    .bind(exp.get("updated_at").and_then(|v| v.as_str()))
    .execute(&state.db)
    .await?;

    Ok(())
}

async fn upsert_pricing_rule(state: &Arc<AppState>, rule: &Value) -> anyhow::Result<()> {
    let id = rule
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Pricing rule missing id"))?;

    sqlx::query(
        "INSERT INTO pricing_rules (id, rule_name, rule_type, day_of_week, hour_start, hour_end, multiplier, flat_adjustment_paise, is_active)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT(id) DO UPDATE SET
            rule_name = excluded.rule_name,
            rule_type = excluded.rule_type,
            day_of_week = excluded.day_of_week,
            hour_start = excluded.hour_start,
            hour_end = excluded.hour_end,
            multiplier = excluded.multiplier,
            flat_adjustment_paise = excluded.flat_adjustment_paise,
            is_active = excluded.is_active",
    )
    .bind(id)
    .bind(rule.get("rule_name").and_then(|v| v.as_str()).unwrap_or("Unknown"))
    .bind(rule.get("rule_type").and_then(|v| v.as_str()).unwrap_or("custom"))
    .bind(rule.get("day_of_week").and_then(|v| v.as_str()))
    .bind(rule.get("hour_start").and_then(|v| v.as_i64()))
    .bind(rule.get("hour_end").and_then(|v| v.as_i64()))
    .bind(rule.get("multiplier").and_then(|v| v.as_f64()).unwrap_or(1.0))
    .bind(rule.get("flat_adjustment_paise").and_then(|v| v.as_i64()).unwrap_or(0))
    .bind(rule.get("is_active").and_then(|v| v.as_i64()).unwrap_or(1))
    .execute(&state.db)
    .await?;

    Ok(())
}
