//! Cloud sync pull logic — fetching data from cloud and applying locally.
//! Extracted from cloud_sync.rs (Phase 385, v49.0 Architecture Completion).

use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;

use crate::cloud_sync::{sign_sync_request, SYNC_TABLES};
use crate::cloud_sync_push::{
    get_last_sync_time, process_debit_intents,
    push_to_cloud, update_sync_state,
};
use crate::cloud_sync_upsert::*;
use crate::state::AppState;

/// Perform a single HTTP sync cycle (bidirectional pull + push).
/// This is the original sync path, used as fallback when relay is unavailable.
pub(crate) async fn sync_once_http(state: &Arc<AppState>, cloud_url: &str) -> anyhow::Result<()> {
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
        .timeout(Duration::from_secs(45));

    // Send terminal secret for authentication
    if let Some(secret) = &state.config.cloud.terminal_secret {
        req = req.header("x-terminal-secret", secret);
    }

    // HMAC-SHA256 signing for GET request (AUTH-07) -- sign query string as body
    if let Some(hmac_key) = &state.config.cloud.sync_hmac_key {
        let query_body = format!("since={}&tables={}", last_synced, SYNC_TABLES);
        let (signature, timestamp, nonce) = sign_sync_request(query_body.as_bytes(), hmac_key.as_bytes());
        req = req
            .header("x-sync-timestamp", timestamp.to_string())
            .header("x-sync-nonce", &nonce)
            .header("x-sync-signature", &signature);
    }

    let resp = req.send().await?;

    if !resp.status().is_success() {
        anyhow::bail!("Cloud returned status {}", resp.status());
    }

    // Handle empty response body gracefully — cloud may return 200 with no body
    // when there are no changes to sync (e.g., stale cloud build, empty result set).
    // This is not an error — treat it as "nothing to sync" and update the timestamp.
    let body_bytes = resp.bytes().await?;
    let body: Value = if body_bytes.is_empty() {
        tracing::debug!("Cloud sync: empty response body (no changes)");
        Value::Object(serde_json::Map::new())
    } else {
        serde_json::from_slice(&body_bytes)?
    };
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
                if let Err(e) = sqlx::query(
                    "INSERT INTO kiosk_settings (key, value) VALUES (?, ?)
                     ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                )
                .bind(key)
                .bind(&val_str)
                .execute(&state.db)
                .await
                {
                    tracing::error!("Cloud sync: failed to upsert kiosk_setting '{}': {}", key, e);
                    continue;
                }
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

    // Upsert auth_tokens (PIN/QR codes created on cloud, consumed at venue kiosk)
    if let Some(tokens) = body.get("auth_tokens").and_then(|v| v.as_array()) {
        for token in tokens {
            if let Err(e) = upsert_auth_token(state, token).await {
                tracing::warn!("Failed to upsert auth_token: {}", e);
            } else {
                total_upserted += 1;
            }
        }
    }

    // Upsert staff_members (cloud admin changes propagate to venue)
    // Same upsert logic as sync_push handler in routes.rs.
    if let Some(staff) = body.get("staff_members").and_then(|v| v.as_array()) {
        for s in staff {
            let id = s.get("id").and_then(|v| v.as_str()).unwrap_or_default();
            if id.is_empty() { continue; }
            let r = sqlx::query(
                "INSERT INTO staff_members (id, name, phone, pin, is_active, role, created_at, updated_at, last_login_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)
                 ON CONFLICT(id) DO UPDATE SET
                    name = excluded.name, phone = excluded.phone, pin = excluded.pin,
                    is_active = excluded.is_active, role = excluded.role,
                    updated_at = excluded.updated_at, last_login_at = excluded.last_login_at",
            )
            .bind(id)
            .bind(s.get("name").and_then(|v| v.as_str()))
            .bind(s.get("phone").and_then(|v| v.as_str()))
            .bind(s.get("pin").and_then(|v| v.as_str()))
            .bind(s.get("is_active").and_then(|v| v.as_i64()).unwrap_or(1))
            .bind(s.get("role").and_then(|v| v.as_str()).unwrap_or("staff"))
            .bind(s.get("created_at").and_then(|v| v.as_str()))
            .bind(s.get("updated_at").and_then(|v| v.as_str()))
            .bind(s.get("last_login_at").and_then(|v| v.as_str()))
            .execute(&state.db)
            .await;
            if r.is_ok() { total_upserted += 1; }
        }
        if !staff.is_empty() {
            tracing::info!("Cloud sync pull: {} staff_members", staff.len());
        }
    }

    // Upsert billing_rates (admin sets rates on either side, both must see them)
    if let Some(rates) = body.get("billing_rates").and_then(|v| v.as_array()) {
        total_upserted += upsert_billing_rates(&state.db, rates).await;
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

    // Process any pending debit intents received from cloud
    if let Err(e) = process_debit_intents(state).await {
        tracing::error!(target: "sync", "Failed to process debit intents: {}", e);
    }

    // Phase 2: Push venue data to cloud
    if let Err(e) = push_to_cloud(state, cloud_url).await {
        tracing::error!("Cloud sync push failed: {}", e);
    }

    Ok(())
}

/// Perform a filtered cloud pull for the specified tables only.
/// Used by `sync_pull_now_handler` and `change_staff_pin_safe` for immediate out-of-band pulls.
/// Does NOT update the `last_synced` timestamp (this is an out-of-band pull, not a regular cycle).
pub(crate) async fn pull_tables_now(state: &Arc<AppState>, tables: &[&str]) -> anyhow::Result<()> {
    let cloud_url = state
        .config
        .cloud
        .api_url
        .clone()
        .ok_or_else(|| anyhow::anyhow!("cloud_url not configured"))?;

    let tables_param = tables.join(",");
    let url = format!("{}/sync/changes", cloud_url);

    tracing::debug!("pull_tables_now: requesting tables={} from {}", tables_param, url);

    // Use since=epoch so we get all current data for the requested tables
    let since = "1970-01-01T00:00:00Z";

    let mut req = state
        .http_client
        .get(&url)
        .query(&[
            ("since", since),
            ("tables", tables_param.as_str()),
        ])
        .timeout(Duration::from_secs(30));

    if let Some(secret) = &state.config.cloud.terminal_secret {
        req = req.header("x-terminal-secret", secret);
    }

    if let Some(hmac_key) = &state.config.cloud.sync_hmac_key {
        let query_body = format!("since={}&tables={}", since, tables_param);
        let (signature, timestamp, nonce) = sign_sync_request(query_body.as_bytes(), hmac_key.as_bytes());
        req = req
            .header("x-sync-timestamp", timestamp.to_string())
            .header("x-sync-nonce", &nonce)
            .header("x-sync-signature", &signature);
    }

    let resp = req.send().await?;

    if !resp.status().is_success() {
        anyhow::bail!("pull_tables_now: cloud returned status {}", resp.status());
    }

    let body_bytes = resp.bytes().await?;
    let body: Value = if body_bytes.is_empty() {
        Value::Object(serde_json::Map::new())
    } else {
        serde_json::from_slice(&body_bytes)?
    };
    let mut total_upserted = 0u64;

    // Apply upserts only for the tables present in the response (same logic as sync_once_http)
    if let Some(drivers) = body.get("drivers").and_then(|v| v.as_array()) {
        for driver in drivers {
            if let Err(e) = upsert_driver(state, driver).await {
                tracing::warn!("pull_tables_now: failed to upsert driver: {}", e);
            } else {
                total_upserted += 1;
            }
        }
    }

    if let Some(wallets) = body.get("wallets").and_then(|v| v.as_array()) {
        for wallet in wallets {
            if let Err(e) = upsert_wallet(state, wallet).await {
                tracing::warn!("pull_tables_now: failed to upsert wallet: {}", e);
            } else {
                total_upserted += 1;
            }
        }
    }

    if let Some(tiers) = body.get("pricing_tiers").and_then(|v| v.as_array()) {
        for tier in tiers {
            if let Err(e) = upsert_pricing_tier(state, tier).await {
                tracing::warn!("pull_tables_now: failed to upsert pricing_tier: {}", e);
            } else {
                total_upserted += 1;
            }
        }
    }

    if let Some(staff) = body.get("staff_members").and_then(|v| v.as_array()) {
        for s in staff {
            let id = s.get("id").and_then(|v| v.as_str()).unwrap_or_default();
            if id.is_empty() { continue; }
            let r = sqlx::query(
                "INSERT INTO staff_members (id, name, phone, pin, is_active, role, created_at, updated_at, last_login_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)
                 ON CONFLICT(id) DO UPDATE SET
                    name = excluded.name, phone = excluded.phone, pin = excluded.pin,
                    is_active = excluded.is_active, role = excluded.role,
                    updated_at = excluded.updated_at, last_login_at = excluded.last_login_at",
            )
            .bind(id)
            .bind(s.get("name").and_then(|v| v.as_str()))
            .bind(s.get("phone").and_then(|v| v.as_str()))
            .bind(s.get("pin").and_then(|v| v.as_str()))
            .bind(s.get("is_active").and_then(|v| v.as_i64()).unwrap_or(1))
            .bind(s.get("role").and_then(|v| v.as_str()).unwrap_or("staff"))
            .bind(s.get("created_at").and_then(|v| v.as_str()))
            .bind(s.get("updated_at").and_then(|v| v.as_str()))
            .bind(s.get("last_login_at").and_then(|v| v.as_str()))
            .execute(&state.db)
            .await;
            if r.is_ok() { total_upserted += 1; }
        }
    }

    // Additional tables that may be requested
    if let Some(rates) = body.get("billing_rates").and_then(|v| v.as_array()) {
        total_upserted += upsert_billing_rates(&state.db, rates).await;
    }

    if total_upserted > 0 {
        tracing::info!("pull_tables_now: upserted {} records for tables [{}]", total_upserted, tables_param);
    } else {
        tracing::debug!("pull_tables_now: no records for tables [{}]", tables_param);
    }

    Ok(())
}

/// Upsert billing_rates from a JSON array. Returns count of successful upserts.
async fn upsert_billing_rates(db: &sqlx::SqlitePool, rates: &[Value]) -> u64 {
    let mut count = 0u64;
    for rate in rates {
        let id = rate.get("id").and_then(|v| v.as_str()).unwrap_or_default();
        if id.is_empty() { continue; }
        let r = sqlx::query(
            "INSERT INTO billing_rates (id, tier_name, tier_order, rate_per_min_paise, threshold_minutes, sim_type, is_active)
             VALUES (?1,?2,?3,?4,?5,?6,?7)
             ON CONFLICT(id) DO UPDATE SET
                tier_name = excluded.tier_name, tier_order = excluded.tier_order,
                rate_per_min_paise = excluded.rate_per_min_paise,
                threshold_minutes = excluded.threshold_minutes,
                sim_type = excluded.sim_type, is_active = excluded.is_active",
        )
        .bind(id)
        .bind(rate.get("tier_name").and_then(|v| v.as_str()))
        .bind(rate.get("tier_order").and_then(|v| v.as_i64()).unwrap_or(0))
        .bind(rate.get("rate_per_min_paise").and_then(|v| v.as_i64()).unwrap_or(0))
        .bind(rate.get("threshold_minutes").and_then(|v| v.as_i64()).unwrap_or(0))
        .bind(rate.get("sim_type").and_then(|v| v.as_str()))
        .bind(rate.get("is_active").and_then(|v| v.as_i64()).unwrap_or(1))
        .execute(db)
        .await;
        if r.is_ok() { count += 1; }
    }
    if !rates.is_empty() {
        tracing::info!("Cloud sync pull: {} billing_rates", rates.len());
    }
    count
}
