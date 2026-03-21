//! Cloud sync: bidirectional data sync between cloud and venue racecontrol instances.
//!
//! Supports dual-mode operation:
//! - **Relay mode** (2s interval): Routes sync through localhost comms-link relay for real-time sync.
//! - **HTTP fallback** (30s interval): Direct HTTP to remote cloud URL when relay is unavailable.
//!
//! The relay path only pushes deltas (the other side pushes to us independently via /sync/push).
//! The HTTP fallback path does full bidirectional pull+push in a single cycle.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use serde_json::Value;

use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::state::AppState;

type HmacSha256 = Hmac<Sha256>;

const SYNC_TABLES: &str = "drivers,wallets,pricing_tiers,pricing_rules,billing_rates,kiosk_experiences,kiosk_settings,auth_tokens,reservations,debit_intents";

/// Relay sync interval in seconds (fast — localhost only).
const RELAY_INTERVAL_SECS: u64 = 2;

/// Hysteresis thresholds to prevent relay mode flapping.
/// Require N consecutive failures before declaring relay down,
/// and M consecutive successes before declaring relay up.
const RELAY_DOWN_THRESHOLD: u32 = 3; // 3 failures × 2s = 6s grace
const RELAY_UP_THRESHOLD: u32 = 2;   // 2 successes × 2s = 4s to confirm

// ─── HMAC-SHA256 Sync Payload Signing (AUTH-07) ─────────────────────────────

/// Sign an outbound sync request body with HMAC-SHA256 + timestamp + nonce.
/// Returns (hex_signature, unix_timestamp, nonce_string).
fn sign_sync_request(body: &[u8], key: &[u8]) -> (String, i64, String) {
    let timestamp = chrono::Utc::now().timestamp();
    let nonce = uuid::Uuid::new_v4().to_string();
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(&timestamp.to_be_bytes());
    mac.update(nonce.as_bytes());
    mac.update(body);
    let signature = hex::encode(mac.finalize().into_bytes());
    (signature, timestamp, nonce)
}

/// Verify an inbound sync request's HMAC-SHA256 signature.
/// Rejects if timestamp is more than 5 minutes from current time (replay prevention).
pub(crate) fn verify_sync_signature(
    body: &[u8],
    key: &[u8],
    timestamp: i64,
    nonce: &str,
    signature: &str,
) -> bool {
    let now = chrono::Utc::now().timestamp();
    if (now - timestamp).abs() > 300 {
        tracing::warn!(target: "cloud_sync", "HMAC timestamp expired: {}s difference", (now - timestamp).abs());
        return false;
    }
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(&timestamp.to_be_bytes());
    mac.update(nonce.as_bytes());
    mac.update(body);
    mac.verify_slice(&hex::decode(signature).unwrap_or_default()).is_ok()
}

/// Normalize ISO timestamps ("2026-03-07T23:48:38.123+00:00") to SQLite format ("2026-03-07 23:48:38").
/// SQLite's datetime('now') uses space separator, but sync_state stores ISO with 'T'.
/// String comparison: space (0x20) < 'T' (0x54), causing updated records to be invisible.
fn normalize_timestamp(ts: &str) -> String {
    ts.replace('T', " ")
        .split('+')
        .next()
        .unwrap_or("1970-01-01 00:00:00")
        .trim_end_matches('Z')
        .to_string()
}

/// Check if the comms-link relay is available and connected to the remote peer.
/// Returns false if comms_link_url is not configured, relay is unreachable, or peer is disconnected.
pub async fn is_relay_available(state: &Arc<AppState>) -> bool {
    let relay_url = match &state.config.cloud.comms_link_url {
        Some(url) => url.clone(),
        None => return false,
    };

    let health_url = format!("{}/relay/health", relay_url);
    let result = state
        .http_client
        .get(&health_url)
        .timeout(Duration::from_millis(500))
        .send()
        .await;

    match result {
        Ok(resp) if resp.status().is_success() => {
            match resp.json::<Value>().await {
                Ok(body) => body.get("connected").and_then(|v| v.as_bool()).unwrap_or(false),
                Err(_) => false,
            }
        }
        _ => false,
    }
}

/// Spawn the cloud sync background task.
/// Only starts if cloud.enabled = true and cloud.api_url is set.
///
/// When comms_link_url is configured, uses adaptive interval:
/// - 2s when relay is available (real-time sync via localhost)
/// - 30s HTTP fallback when relay is down (rate-limited to avoid hammering remote)
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

    let has_relay = cloud.comms_link_url.is_some();
    let fallback_interval_secs = cloud.sync_interval_secs;

    // Log HMAC signing status (AUTH-07)
    if cloud.sync_hmac_key.is_some() {
        tracing::info!("Cloud sync HMAC signing enabled");
    } else {
        tracing::warn!("Cloud sync HMAC signing NOT configured -- using x-terminal-secret only");
    }

    if has_relay {
        tracing::info!(
            "Cloud sync enabled: {} (relay: {}s, fallback: {}s)",
            api_url,
            RELAY_INTERVAL_SECS,
            fallback_interval_secs
        );
    } else {
        tracing::info!(
            "Cloud sync enabled: {} (every {}s, no relay configured)",
            api_url,
            fallback_interval_secs
        );
    }

    tokio::spawn(async move {
        // Wait 5s on startup before first sync
        tokio::time::sleep(Duration::from_secs(5)).await;

        // Use 2s tick when relay is configured, otherwise use the fallback interval.
        // When relay is unavailable, we rate-limit HTTP fallback to run only every
        // fallback_interval_secs by tracking last_http_fallback.
        let tick_interval = if has_relay {
            RELAY_INTERVAL_SECS
        } else {
            fallback_interval_secs
        };
        let mut interval = tokio::time::interval(Duration::from_secs(tick_interval));
        let mut last_http_fallback = Instant::now() - Duration::from_secs(fallback_interval_secs + 1);

        // Hysteresis state: track consecutive successes/failures to prevent flapping.
        // The effective relay state only transitions after sustained signal.
        let mut effective_relay_up = false;
        let mut consecutive_up: u32 = 0;
        let mut consecutive_down: u32 = 0;
        let mut logged_state: Option<bool> = None;

        loop {
            interval.tick().await;

            if has_relay {
                let raw_up = is_relay_available(&state).await;

                // Update hysteresis counters
                if raw_up {
                    consecutive_up += 1;
                    consecutive_down = 0;
                } else {
                    consecutive_down += 1;
                    consecutive_up = 0;
                }

                // Apply hysteresis: only transition after sustained signal
                if effective_relay_up && consecutive_down >= RELAY_DOWN_THRESHOLD {
                    effective_relay_up = false;
                } else if !effective_relay_up && consecutive_up >= RELAY_UP_THRESHOLD {
                    effective_relay_up = true;
                }

                // Update shared AtomicBool for action_queue to read
                state.relay_available.store(effective_relay_up, Ordering::Relaxed);

                // Log mode transitions (only on change, not every cycle)
                if logged_state != Some(effective_relay_up) {
                    if effective_relay_up {
                        tracing::info!("Sync mode: relay (comms-link connected)");
                    } else {
                        tracing::info!("Sync mode: HTTP fallback (comms-link unavailable)");
                    }
                    logged_state = Some(effective_relay_up);
                }

                if effective_relay_up {
                    // Relay mode: push deltas via localhost relay (2s cycle)
                    if let Err(e) = push_via_relay(&state).await {
                        tracing::error!("Cloud sync relay push failed: {}", e);
                    }
                } else {
                    // Relay unavailable: fall back to HTTP but rate-limit to original interval
                    if last_http_fallback.elapsed() >= Duration::from_secs(fallback_interval_secs) {
                        if let Err(e) = sync_once_http(&state, &api_url).await {
                            tracing::error!("Cloud sync HTTP fallback failed: {}", e);
                        }
                        last_http_fallback = Instant::now();
                    }
                }
            } else {
                // No relay configured: always use HTTP
                if let Err(e) = sync_once_http(&state, &api_url).await {
                    tracing::error!("Cloud sync failed: {}", e);
                }
            }
        }
    });
}

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
async fn push_via_relay(state: &Arc<AppState>) -> anyhow::Result<()> {
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

/// Process pending debit intents received from cloud.
/// Called after sync pull to process wallet debits on the local server.
/// Local is the single writer for wallet debits -- cloud NEVER directly modifies wallet.
pub(crate) async fn process_debit_intents(state: &Arc<AppState>) -> anyhow::Result<u64> {
    let pending = sqlx::query_as::<_, (String, String, i64, String)>(
        "SELECT id, driver_id, amount_paise, reservation_id
         FROM debit_intents WHERE status = 'pending' ORDER BY created_at ASC",
    )
    .fetch_all(&state.db)
    .await?;

    if pending.is_empty() {
        return Ok(0);
    }

    let mut processed = 0u64;
    for (intent_id, driver_id, amount, reservation_id) in &pending {
        let balance = sqlx::query_as::<_, (i64,)>(
            "SELECT balance_paise FROM wallets WHERE driver_id = ?",
        )
        .bind(driver_id)
        .fetch_optional(&state.db)
        .await?;

        match balance {
            Some((bal,)) if bal >= *amount => {
                let new_balance = bal - amount;
                let txn_id = uuid::Uuid::new_v4().to_string();

                // Debit wallet
                sqlx::query(
                    "UPDATE wallets SET balance_paise = ?, total_debited_paise = total_debited_paise + ?,
                     updated_at = datetime('now') WHERE driver_id = ?",
                )
                .bind(new_balance).bind(amount).bind(driver_id)
                .execute(&state.db).await?;

                // Record wallet transaction
                sqlx::query(
                    "INSERT INTO wallet_transactions (id, driver_id, amount_paise, balance_after_paise,
                     txn_type, reference_id, notes, created_at)
                     VALUES (?, ?, ?, ?, 'debit_session', ?, 'Remote booking debit', datetime('now'))",
                )
                .bind(&txn_id).bind(driver_id).bind(-amount).bind(new_balance).bind(reservation_id)
                .execute(&state.db).await?;

                // Mark intent completed
                sqlx::query(
                    "UPDATE debit_intents SET status = 'completed', wallet_txn_id = ?,
                     processed_at = datetime('now'), updated_at = datetime('now') WHERE id = ?",
                )
                .bind(&txn_id).bind(intent_id)
                .execute(&state.db).await?;

                // Update reservation to confirmed
                sqlx::query(
                    "UPDATE reservations SET status = 'confirmed', updated_at = datetime('now')
                     WHERE id = ?",
                )
                .bind(reservation_id)
                .execute(&state.db).await?;

                tracing::info!(target: "sync", "Debit intent {} completed: {} paise from driver {}",
                    intent_id, amount, driver_id);
                processed += 1;
            }
            _ => {
                // Insufficient balance or no wallet
                let reason = if balance.is_none() { "no_wallet" } else { "insufficient_balance" };
                sqlx::query(
                    "UPDATE debit_intents SET status = 'failed', failure_reason = ?,
                     processed_at = datetime('now'), updated_at = datetime('now') WHERE id = ?",
                )
                .bind(reason).bind(intent_id)
                .execute(&state.db).await?;

                sqlx::query(
                    "UPDATE reservations SET status = 'failed', updated_at = datetime('now')
                     WHERE id = ?",
                )
                .bind(reservation_id)
                .execute(&state.db).await?;

                tracing::warn!(target: "sync", "Debit intent {} failed ({}): {} paise from driver {}",
                    intent_id, reason, amount, driver_id);
                processed += 1;
            }
        }
    }

    if processed > 0 {
        tracing::info!(target: "sync", "Processed {} debit intents", processed);
    }
    Ok(processed)
}

/// Collect the push payload (shared between relay and HTTP push paths).
/// Returns (payload, has_data).
/// Schema version bumped when tables/columns change.
/// Cloud side can reject pushes if it hasn't migrated yet.
const SCHEMA_VERSION: u32 = 3;

async fn collect_push_payload(state: &Arc<AppState>) -> anyhow::Result<(Value, bool)> {
    let last_push = normalize_timestamp(&get_last_push_time(state).await);
    let origin = &state.config.cloud.origin_id;
    let mut payload = serde_json::json!({
        "schema_version": SCHEMA_VERSION,
        "origin": origin,
    });
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
            'experience_id', experience_id, 'car', car, 'track', track, 'sim_type', sim_type,
            'split_count', split_count, 'split_duration_minutes', split_duration_minutes,
            'wallet_debit_paise', wallet_debit_paise,
            'discount_paise', discount_paise, 'coupon_id', coupon_id,
            'original_price_paise', original_price_paise, 'discount_reason', discount_reason,
            'pause_count', pause_count, 'total_paused_seconds', total_paused_seconds, 'refund_paise', refund_paise,
            'end_reason', end_reason
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
            'unlimited_trials', COALESCE(unlimited_trials, 0),
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

    // Push wallet transactions (immutable, use >= to avoid missing same-timestamp rows)
    let wallet_txns = sqlx::query_as::<_, (String,)>(
        "SELECT json_object(
            'id', id, 'driver_id', driver_id, 'amount_paise', amount_paise,
            'balance_after_paise', balance_after_paise, 'txn_type', txn_type,
            'reference_id', reference_id, 'notes', notes, 'staff_id', staff_id,
            'created_at', created_at
        ) FROM wallet_transactions WHERE created_at >= ? ORDER BY created_at ASC LIMIT 500",
    )
    .bind(&last_push)
    .fetch_all(&state.db)
    .await?;

    if !wallet_txns.is_empty() {
        let items: Vec<serde_json::Value> = wallet_txns.iter()
            .filter_map(|r| serde_json::from_str(&r.0).ok())
            .collect();
        tracing::info!("Cloud sync push: {} wallet transactions", items.len());
        payload["wallet_transactions"] = serde_json::json!(items);
        has_data = true;
    }

    // Collect billing events since last push
    let billing_events = sqlx::query_as::<_, (String,)>(
        "SELECT json_object(
            'id', id, 'billing_session_id', billing_session_id,
            'event_type', event_type, 'driving_seconds_at_event', driving_seconds_at_event,
            'metadata', metadata, 'created_at', created_at
        ) FROM billing_events WHERE created_at >= ? ORDER BY created_at ASC LIMIT 500",
    )
    .bind(&last_push)
    .fetch_all(&state.db)
    .await?;

    if !billing_events.is_empty() {
        let items: Vec<serde_json::Value> = billing_events.iter()
            .filter_map(|r| serde_json::from_str(&r.0).ok())
            .collect();
        tracing::info!("Cloud sync push: {} billing events", items.len());
        payload["billing_events"] = serde_json::json!(items);
        has_data = true;
    }

    // Collect reservation status updates (local updates: redeemed, expired status changes)
    let reservations = sqlx::query_as::<_, (String,)>(
        "SELECT json_object(
            'id', id, 'driver_id', driver_id, 'experience_id', experience_id,
            'pin', pin, 'status', status, 'pod_number', pod_number,
            'debit_intent_id', debit_intent_id,
            'created_at', created_at, 'expires_at', expires_at,
            'redeemed_at', redeemed_at, 'cancelled_at', cancelled_at,
            'updated_at', updated_at
        ) FROM reservations WHERE updated_at > ? ORDER BY updated_at ASC LIMIT 500",
    )
    .bind(&last_push)
    .fetch_all(&state.db)
    .await?;

    if !reservations.is_empty() {
        let items: Vec<serde_json::Value> = reservations.iter()
            .filter_map(|r| serde_json::from_str(&r.0).ok())
            .collect();
        tracing::info!("Cloud sync push: {} reservations", items.len());
        payload["reservations"] = serde_json::json!(items);
        has_data = true;
    }

    // Collect debit intent status updates (local processes: completed/failed results)
    let intents = sqlx::query_as::<_, (String,)>(
        "SELECT json_object(
            'id', id, 'driver_id', driver_id, 'amount_paise', amount_paise,
            'reservation_id', reservation_id, 'status', status,
            'failure_reason', failure_reason, 'wallet_txn_id', wallet_txn_id,
            'origin', origin,
            'created_at', created_at, 'processed_at', processed_at,
            'updated_at', updated_at
        ) FROM debit_intents WHERE updated_at > ? ORDER BY updated_at ASC LIMIT 500",
    )
    .bind(&last_push)
    .fetch_all(&state.db)
    .await?;

    if !intents.is_empty() {
        let items: Vec<serde_json::Value> = intents.iter()
            .filter_map(|r| serde_json::from_str(&r.0).ok())
            .collect();
        tracing::info!("Cloud sync push: {} debit_intents", items.len());
        payload["debit_intents"] = serde_json::json!(items);
        has_data = true;
    }

    Ok((payload, has_data))
}

/// Perform a single HTTP sync cycle (bidirectional pull + push).
/// This is the original sync path, used as fallback when relay is unavailable.
async fn sync_once_http(state: &Arc<AppState>, cloud_url: &str) -> anyhow::Result<()> {
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

/// Push venue-generated data (laps, billing, pods, leaderboard) to cloud via direct HTTP.
async fn push_to_cloud(state: &Arc<AppState>, cloud_url: &str) -> anyhow::Result<()> {
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

async fn get_last_sync_time(state: &Arc<AppState>) -> String {
    let row = match sqlx::query_as::<_, (String,)>(
        "SELECT MIN(last_synced_at) FROM sync_state",
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

async fn update_sync_state(state: &Arc<AppState>, synced_at: &str, count: u64) {
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

    // Encrypt incoming PII before storing
    let incoming_name = driver.get("name").and_then(|v| v.as_str()).unwrap_or("Unknown");
    let incoming_phone = driver.get("phone").and_then(|v| v.as_str());
    let incoming_email = driver.get("email").and_then(|v| v.as_str());
    let incoming_guardian_phone = driver.get("guardian_phone").and_then(|v| v.as_str());

    let phone_hash: Option<String> = incoming_phone.filter(|p| !p.is_empty())
        .map(|p| state.field_cipher.hash_phone(p));
    let phone_enc: Option<String> = incoming_phone.filter(|p| !p.is_empty())
        .map(|p| state.field_cipher.encrypt_field(p))
        .transpose().map_err(|e| anyhow::anyhow!("encrypt phone: {}", e))?;
    let email_enc: Option<String> = incoming_email.filter(|e| !e.is_empty())
        .map(|e| state.field_cipher.encrypt_field(e))
        .transpose().map_err(|e| anyhow::anyhow!("encrypt email: {}", e))?;
    let name_enc: Option<String> = if !incoming_name.is_empty() {
        Some(state.field_cipher.encrypt_field(incoming_name)
            .map_err(|e| anyhow::anyhow!("encrypt name: {}", e))?)
    } else { None };
    let guardian_phone_hash: Option<String> = incoming_guardian_phone.filter(|p| !p.is_empty())
        .map(|p| state.field_cipher.hash_phone(p));
    let guardian_phone_enc: Option<String> = incoming_guardian_phone.filter(|p| !p.is_empty())
        .map(|p| state.field_cipher.encrypt_field(p))
        .transpose().map_err(|e| anyhow::anyhow!("encrypt guardian_phone: {}", e))?;

    // Upsert — cloud wins for customer-owned fields, preserve local-only fields (otp_code etc.)
    // PII stored in _enc/_hash columns only; plaintext columns set to NULL.
    sqlx::query(
        "INSERT INTO drivers (id, customer_id, name, name_enc, phone_hash, phone_enc, email_enc,
            steam_guid, iracing_id, avatar_url,
            total_laps, total_time_ms, has_used_trial, unlimited_trials, pin_hash, phone_verified,
            dob, waiver_signed, waiver_signed_at, waiver_version,
            guardian_name, guardian_phone_hash, guardian_phone_enc, registration_completed, signature_data,
            created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27)
        ON CONFLICT(id) DO UPDATE SET
            customer_id = COALESCE(excluded.customer_id, drivers.customer_id),
            name = excluded.name,
            name_enc = excluded.name_enc,
            phone_hash = excluded.phone_hash,
            phone_enc = excluded.phone_enc,
            email_enc = excluded.email_enc,
            phone = NULL,
            email = NULL,
            guardian_phone = NULL,
            steam_guid = COALESCE(excluded.steam_guid, drivers.steam_guid),
            iracing_id = COALESCE(excluded.iracing_id, drivers.iracing_id),
            avatar_url = COALESCE(excluded.avatar_url, drivers.avatar_url),
            has_used_trial = excluded.has_used_trial,
            unlimited_trials = excluded.unlimited_trials,
            pin_hash = COALESCE(excluded.pin_hash, drivers.pin_hash),
            phone_verified = excluded.phone_verified,
            dob = excluded.dob,
            waiver_signed = excluded.waiver_signed,
            waiver_signed_at = excluded.waiver_signed_at,
            waiver_version = excluded.waiver_version,
            guardian_name = excluded.guardian_name,
            guardian_phone_hash = excluded.guardian_phone_hash,
            guardian_phone_enc = excluded.guardian_phone_enc,
            registration_completed = excluded.registration_completed,
            signature_data = COALESCE(excluded.signature_data, drivers.signature_data),
            updated_at = excluded.updated_at",
    )
    .bind(id)                                                                   // ?1
    .bind(driver.get("customer_id").and_then(|v| v.as_str()))                   // ?2
    .bind(incoming_name)                                                        // ?3 name (keep for leaderboard)
    .bind(&name_enc)                                                            // ?4 name_enc
    .bind(&phone_hash)                                                          // ?5 phone_hash
    .bind(&phone_enc)                                                           // ?6 phone_enc
    .bind(&email_enc)                                                           // ?7 email_enc
    .bind(driver.get("steam_guid").and_then(|v| v.as_str()))                    // ?8
    .bind(driver.get("iracing_id").and_then(|v| v.as_str()))                    // ?9
    .bind(driver.get("avatar_url").and_then(|v| v.as_str()))                    // ?10
    .bind(driver.get("total_laps").and_then(|v| v.as_i64()).unwrap_or(0))       // ?11
    .bind(driver.get("total_time_ms").and_then(|v| v.as_i64()).unwrap_or(0))    // ?12
    .bind(driver.get("has_used_trial").and_then(|v| v.as_i64()).unwrap_or(0))   // ?13
    .bind(driver.get("unlimited_trials").and_then(|v| v.as_i64()).unwrap_or(0)) // ?14
    .bind(driver.get("pin_hash").and_then(|v| v.as_str()))                      // ?15
    .bind(driver.get("phone_verified").and_then(|v| v.as_i64()).unwrap_or(0))   // ?16
    .bind(driver.get("dob").and_then(|v| v.as_str()))                           // ?17
    .bind(driver.get("waiver_signed").and_then(|v| v.as_i64()).unwrap_or(0))    // ?18
    .bind(driver.get("waiver_signed_at").and_then(|v| v.as_str()))              // ?19
    .bind(driver.get("waiver_version").and_then(|v| v.as_str()))                // ?20
    .bind(driver.get("guardian_name").and_then(|v| v.as_str()))                 // ?21
    .bind(&guardian_phone_hash)                                                 // ?22
    .bind(&guardian_phone_enc)                                                  // ?23
    .bind(driver.get("registration_completed").and_then(|v| v.as_i64()).unwrap_or(0)) // ?24
    .bind(driver.get("signature_data").and_then(|v| v.as_str()))                // ?25
    .bind(driver.get("created_at").and_then(|v| v.as_str()))                    // ?26
    .bind(cloud_updated)                                                        // ?27
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
                let ph = state.field_cipher.hash_phone(phone);
                sqlx::query_as::<_, (String,)>(
                    "SELECT id FROM drivers WHERE phone_hash = ?",
                )
                .bind(&ph)
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
            // Only overwrite if cloud's updated_at is newer than local.
            // This prevents stale cloud data from overwriting venue debits
            // that haven't been pushed yet.
            let local_ts: Option<(Option<String>,)> = sqlx::query_as(
                "SELECT updated_at FROM wallets WHERE driver_id = ?",
            )
            .bind(&local_driver_id)
            .fetch_optional(&state.db)
            .await?;

            let should_update = match &local_ts {
                Some((Some(ts),)) => cloud_updated > ts.as_str(),
                _ => true,
            };

            if should_update {
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
            } else {
                tracing::debug!(
                    "Wallet sync: skipping update for driver {} — local is newer",
                    local_driver_id
                );
            }
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

/// Upsert a single auth_token from cloud → venue.
/// Only inserts pending tokens; skips if token already exists locally (prevents overwriting consumed state).
async fn upsert_auth_token(state: &Arc<AppState>, token: &Value) -> anyhow::Result<()> {
    let id = token
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Auth token missing id"))?;

    // Only insert if not already present — never overwrite local status
    // (venue may have already consumed/expired the token)
    sqlx::query(
        "INSERT OR IGNORE INTO auth_tokens (id, pod_id, driver_id, pricing_tier_id, auth_type, token, status, custom_price_paise, custom_duration_minutes, experience_id, custom_launch_args, created_at, expires_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
    )
    .bind(id)
    .bind(token.get("pod_id").and_then(|v| v.as_str()).unwrap_or(""))
    .bind(token.get("driver_id").and_then(|v| v.as_str()).unwrap_or(""))
    .bind(token.get("pricing_tier_id").and_then(|v| v.as_str()).unwrap_or(""))
    .bind(token.get("auth_type").and_then(|v| v.as_str()).unwrap_or("pin"))
    .bind(token.get("token").and_then(|v| v.as_str()).unwrap_or(""))
    .bind(token.get("status").and_then(|v| v.as_str()).unwrap_or("pending"))
    .bind(token.get("custom_price_paise").and_then(|v| v.as_i64()))
    .bind(token.get("custom_duration_minutes").and_then(|v| v.as_i64()))
    .bind(token.get("experience_id").and_then(|v| v.as_str()))
    .bind(token.get("custom_launch_args").and_then(|v| v.as_str()))
    .bind(token.get("created_at").and_then(|v| v.as_str()))
    .bind(token.get("expires_at").and_then(|v| v.as_str()).unwrap_or(""))
    .execute(&state.db)
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    /// Verify billing_sessions push query includes pause_count, total_paused_seconds, refund_paise
    #[tokio::test]
    async fn push_payload_includes_billing_session_extra_columns() {
        let pool = sqlx::SqlitePool::connect(":memory:").await.unwrap();

        sqlx::query(
            "CREATE TABLE billing_sessions (
                id TEXT PRIMARY KEY, driver_id TEXT, pod_id TEXT, pricing_tier_id TEXT, end_reason TEXT,
                allocated_seconds INTEGER, driving_seconds INTEGER DEFAULT 0,
                status TEXT DEFAULT 'pending', custom_price_paise INTEGER, notes TEXT,
                started_at TEXT, ended_at TEXT, created_at TEXT,
                experience_id TEXT, car TEXT, track TEXT, sim_type TEXT,
                split_count INTEGER, split_duration_minutes INTEGER,
                wallet_debit_paise INTEGER, discount_paise INTEGER, coupon_id TEXT,
                original_price_paise INTEGER, discount_reason TEXT,
                pause_count INTEGER DEFAULT 0, total_paused_seconds INTEGER DEFAULT 0,
                refund_paise INTEGER DEFAULT 0
            )"
        ).execute(&pool).await.unwrap();

        sqlx::query(
            "INSERT INTO billing_sessions (id, driver_id, pod_id, pricing_tier_id, allocated_seconds, status, created_at, pause_count, total_paused_seconds, refund_paise, end_reason)
             VALUES ('s1', 'd1', 'p1', 'tier1', 1800, 'Completed', '2026-01-01T00:00:00', 3, 120, 5000, 'orphan_timeout')"
        ).execute(&pool).await.unwrap();

        let rows = sqlx::query_as::<_, (String,)>(
            "SELECT json_object(
                'id', id, 'driver_id', driver_id, 'pod_id', pod_id,
                'pricing_tier_id', pricing_tier_id, 'allocated_seconds', allocated_seconds,
                'driving_seconds', driving_seconds, 'status', status,
                'custom_price_paise', custom_price_paise, 'notes', notes,
                'started_at', started_at, 'ended_at', ended_at, 'created_at', created_at,
                'experience_id', experience_id, 'car', car, 'track', track, 'sim_type', sim_type,
                'split_count', split_count, 'split_duration_minutes', split_duration_minutes,
                'wallet_debit_paise', wallet_debit_paise,
                'discount_paise', discount_paise, 'coupon_id', coupon_id,
                'original_price_paise', original_price_paise, 'discount_reason', discount_reason,
                'pause_count', pause_count, 'total_paused_seconds', total_paused_seconds, 'refund_paise', refund_paise,
                'end_reason', end_reason
            ) FROM billing_sessions WHERE created_at > '2025-01-01' ORDER BY created_at ASC LIMIT 500"
        ).fetch_all(&pool).await.unwrap();

        assert_eq!(rows.len(), 1);
        let val: serde_json::Value = serde_json::from_str(&rows[0].0).unwrap();
        assert_eq!(val["pause_count"], 3);
        assert_eq!(val["total_paused_seconds"], 120);
        assert_eq!(val["refund_paise"], 5000);
        assert_eq!(val["id"], "s1");
        assert_eq!(val["status"], "Completed");
        assert_eq!(val["end_reason"], "orphan_timeout", "end_reason must be included in billing_sessions push payload");
    }

    /// Verify billing_events push query produces correct JSON
    #[tokio::test]
    async fn push_payload_includes_billing_events() {
        let pool = sqlx::SqlitePool::connect(":memory:").await.unwrap();

        sqlx::query(
            "CREATE TABLE billing_events (
                id TEXT PRIMARY KEY, billing_session_id TEXT NOT NULL,
                event_type TEXT NOT NULL, driving_seconds_at_event INTEGER NOT NULL DEFAULT 0,
                metadata TEXT, created_at TEXT DEFAULT (datetime('now'))
            )"
        ).execute(&pool).await.unwrap();

        sqlx::query(
            "INSERT INTO billing_events (id, billing_session_id, event_type, driving_seconds_at_event, metadata, created_at)
             VALUES ('e1', 's1', 'started', 0, NULL, '2026-01-01T00:00:00'),
                    ('e2', 's1', 'paused', 300, '{\"reason\":\"customer_request\"}', '2026-01-01T00:05:00')"
        ).execute(&pool).await.unwrap();

        let rows = sqlx::query_as::<_, (String,)>(
            "SELECT json_object(
                'id', id, 'billing_session_id', billing_session_id,
                'event_type', event_type, 'driving_seconds_at_event', driving_seconds_at_event,
                'metadata', metadata, 'created_at', created_at
            ) FROM billing_events WHERE created_at >= ? ORDER BY created_at ASC LIMIT 500"
        ).bind("2025-01-01").fetch_all(&pool).await.unwrap();

        assert_eq!(rows.len(), 2);

        let ev1: serde_json::Value = serde_json::from_str(&rows[0].0).unwrap();
        assert_eq!(ev1["id"], "e1");
        assert_eq!(ev1["event_type"], "started");
        assert_eq!(ev1["driving_seconds_at_event"], 0);
        assert_eq!(ev1["billing_session_id"], "s1");

        let ev2: serde_json::Value = serde_json::from_str(&rows[1].0).unwrap();
        assert_eq!(ev2["id"], "e2");
        assert_eq!(ev2["event_type"], "paused");
        assert_eq!(ev2["driving_seconds_at_event"], 300);
    }
}
