//! Cloud sync push logic — pushing venue data to cloud and managing sync state.
//! Extracted from cloud_sync.rs (Phase 385, v49.0 Architecture Completion).
//!
//! ## Module layout
//! - `cloud_sync_push` (this file): Push via relay/HTTP, sync state management
//! - `cloud_sync_payload`: Push payload collection (all table queries)
//! - `cloud_sync_debit`: Debit intent processing for remote bookings

use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;

use crate::cloud_sync::{sign_sync_request, SYNC_TABLES};
use crate::state::AppState;

#[path = "cloud_sync_payload.rs"]
mod cloud_sync_payload;

#[path = "cloud_sync_debit.rs"]
mod cloud_sync_debit;

// Re-export submodule functions so callers can use cloud_sync_push::X
pub(crate) use cloud_sync_payload::collect_push_payload;
pub(crate) use cloud_sync_debit::process_debit_intents;

/// Schema version bumped when tables/columns change.
/// Cloud side can reject pushes if it hasn't migrated yet.
pub(crate) const SCHEMA_VERSION: u32 = 4;

/// Push sync deltas via the comms-link relay (localhost HTTP).
/// In relay mode, only pushes are needed — the other side pushes to us independently
/// via the /sync/push endpoint (called by comms-link when it receives WS sync_push).
///
/// ## Anti-loop protection
///
/// Sync loops are prevented by the `_push` timestamp in `sync_state`:
/// 1. After a successful push (relay or HTTP), `update_push_state()` records the current time.
/// 2. The next `collect_push_payload()` call queries `WHERE created_at > last_push` (or `updated_at >`).
/// 3. When the OTHER side pushes data to us via `/sync/push` (routes.rs), that handler does NOT
///    call `update_push_state()` — it only upserts received data into the DB.
/// 4. The received data has timestamps older than "now", and since our `_push` was updated after
///    our last outbound push, the received data's timestamps fall before `_push` and won't be
///    re-collected in our next push cycle.
///
/// This means: Cloud pushes to Venue -> Venue receives via /sync/push -> Venue's own push cycle
/// won't re-push that data because its timestamps are older than Venue's `_push` marker.
/// The same logic works in reverse (Venue -> Cloud).
pub(crate) async fn push_via_relay(state: &Arc<AppState>) -> anyhow::Result<()> {
    let relay_url = state
        .config
        .cloud
        .comms_link_url
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("comms_link_url not configured"))?;

    let (payload, has_data) = collect_push_payload(state).await?;
    if !has_data {
        tracing::debug!("Cloud sync relay: nothing to push");
        return Ok(());
    }

    let url = format!("{}/relay/sync", relay_url);
    let resp = state
        .http_client
        .post(&url)
        .json(&payload)
        .timeout(Duration::from_secs(2))
        .send()
        .await?;

    if !resp.status().is_success() {
        anyhow::bail!("Relay sync returned status {}", resp.status());
    }

    // Update push timestamp on success
    update_push_state(state).await;

    tracing::debug!("Cloud sync relay: push successful");
    Ok(())
}

/// Push venue-generated data (laps, billing, pods, leaderboard) to cloud via direct HTTP.
pub(crate) async fn push_to_cloud(state: &Arc<AppState>, cloud_url: &str) -> anyhow::Result<()> {
    let (payload, has_data) = collect_push_payload(state).await?;

    if !has_data {
        tracing::debug!("Cloud sync push: nothing to push");
        return Ok(());
    }

    // POST to cloud
    let push_url = format!("{}/sync/push", cloud_url);
    let body_bytes = serde_json::to_vec(&payload)?;
    let mut req = state.http_client
        .post(&push_url)
        .header("content-type", "application/json")
        .body(body_bytes.clone())
        .timeout(std::time::Duration::from_secs(30));

    if let Some(secret) = &state.config.cloud.terminal_secret {
        req = req.header("x-terminal-secret", secret);
    }

    // HMAC-SHA256 signing (AUTH-07)
    if let Some(hmac_key) = &state.config.cloud.sync_hmac_key {
        let (signature, timestamp, nonce) = sign_sync_request(&body_bytes, hmac_key.as_bytes());
        req = req
            .header("x-sync-timestamp", timestamp.to_string())
            .header("x-sync-nonce", &nonce)
            .header("x-sync-signature", &signature);
    }

    let resp = match req.send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("Cloud push failed (network): {e} — will retry next cycle");
            return Ok(());
        }
    };

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        tracing::warn!("Cloud push rejected (HTTP {status}): {body} — skipping until next cycle");
        return Ok(());
    }

    let body_bytes = resp.bytes().await?;
    let result: serde_json::Value = if body_bytes.is_empty() {
        Value::Object(serde_json::Map::new())
    } else {
        serde_json::from_slice(&body_bytes)?
    };
    let upserted = result.get("upserted").and_then(|v| v.as_u64()).unwrap_or(0);

    if upserted > 0 {
        tracing::info!("Cloud sync push: cloud accepted {} records", upserted);
    }

    // Update push timestamp
    update_push_state(state).await;

    Ok(())
}

pub(crate) async fn get_last_push_time(state: &Arc<AppState>) -> String {
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

pub(crate) async fn update_push_state(state: &Arc<AppState>) {
    let now = chrono::Utc::now().to_rfc3339();
    if let Err(e) = sqlx::query(
        "INSERT INTO sync_state (table_name, last_synced_at, last_sync_count, updated_at)
         VALUES ('_push', ?, 0, datetime('now'))
         ON CONFLICT(table_name) DO UPDATE SET
            last_synced_at = excluded.last_synced_at,
            updated_at = datetime('now')",
    )
    .bind(&now)
    .execute(&state.db)
    .await
    {
        tracing::error!("Cloud sync: failed to update push state: {}", e);
    }
}

pub(crate) async fn get_last_sync_time(state: &Arc<AppState>) -> String {
    // AUTH-03 fix: exclude _push sentinel row from MIN() — only track pull windows
    let row = match sqlx::query_as::<_, (String,)>(
        "SELECT MIN(last_synced_at) FROM sync_state WHERE table_name != '_push'",
    )
    .fetch_optional(&state.db)
    .await
    {
        Ok(row) => row,
        Err(e) => {
            tracing::error!("Cloud sync: failed to read last sync time: {}", e);
            return "1970-01-01T00:00:00Z".to_string();
        }
    };

    row.map(|r| r.0)
        .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string())
}

pub(crate) async fn update_sync_state(state: &Arc<AppState>, synced_at: &str, count: u64) {
    for table in SYNC_TABLES.split(',') {
        if let Err(e) = sqlx::query(
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
        .await
        {
            tracing::error!("Cloud sync: failed to update sync state for '{}': {}", table, e);
        }
    }
}
