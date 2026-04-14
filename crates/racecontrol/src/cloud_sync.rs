//! Cloud sync: bidirectional data sync between cloud and venue racecontrol instances.
//!
//! Supports dual-mode operation:
//! - **Relay mode** (30s interval): Routes sync through localhost comms-link relay for real-time sync.
//! - **HTTP fallback** (30s interval): Direct HTTP to remote cloud URL when relay is unavailable.
//!
//! The relay path only pushes deltas (the other side pushes to us independently via /sync/push).
//! The HTTP fallback path does full bidirectional pull+push in a single cycle.
//!
//! ## Module layout
//! - `cloud_sync` (this file): HMAC auth, constants, spawn loop, relay health check
//! - `cloud_sync_push`: Push payload collection, push via relay/HTTP, debit intents, sync state
//! - `cloud_sync_pull`: Pull from cloud, apply upserts, pull_tables_now
//! - `cloud_sync_upsert`: Per-table merge/upsert functions
//! - `cloud_sync_tests`: Unit tests for push payload generation

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use serde_json::Value;

use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::state::AppState;

// Re-export API from submodules so callers can use cloud_sync::X
pub(crate) use crate::cloud_sync_pull::pull_tables_now;
pub use crate::cloud_sync_upsert::*;

/// Nonce replay protection: tracks seen nonces with their timestamps.
/// Entries older than 5 minutes are purged on each check.
static SEEN_NONCES: std::sync::LazyLock<Mutex<HashMap<String, i64>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

type HmacSha256 = Hmac<Sha256>;

// SYNC-FIX-2: metrics_rollups removed from pull — cloud build 540c22b6 returns empty body
// when this table is in the list (handler crash). Metrics are push-only (venue→cloud).
pub const SYNC_TABLES: &str = "drivers,wallets,pricing_tiers,pricing_rules,billing_rates,kiosk_experiences,kiosk_settings,auth_tokens,reservations,debit_intents,staff_members,driver_ratings,fleet_solutions,model_evaluations,launch_notes";

/// Relay sync interval in seconds (fast — localhost only).
const RELAY_INTERVAL_SECS: u64 = 30;

/// Hysteresis thresholds to prevent relay mode flapping.
/// Require N consecutive failures before declaring relay down,
/// and M consecutive successes before declaring relay up.
const RELAY_DOWN_THRESHOLD: u32 = 3; // 3 failures x 2s = 6s grace
const RELAY_UP_THRESHOLD: u32 = 2;   // 2 successes x 2s = 4s to confirm

// ─── HMAC-SHA256 Sync Payload Signing (AUTH-07) ─────────────────────────────

/// Sign an outbound sync request body with HMAC-SHA256 + timestamp + nonce.
/// Returns (hex_signature, unix_timestamp, nonce_string).
pub(crate) fn sign_sync_request(body: &[u8], key: &[u8]) -> (String, i64, String) {
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

    // Nonce replay protection: reject if this nonce was already seen
    if let Ok(mut seen) = SEEN_NONCES.lock() {
        // Purge expired nonces (older than 5 minutes)
        seen.retain(|_, ts| (now - *ts).abs() <= 300);

        if seen.contains_key(nonce) {
            tracing::warn!(target: "cloud_sync", "HMAC nonce replay detected: {}", nonce);
            return false;
        }

        // Verify signature first, then record nonce only if signature is valid
        let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts any key length");
        mac.update(&timestamp.to_be_bytes());
        mac.update(nonce.as_bytes());
        mac.update(body);
        if mac.verify_slice(&hex::decode(signature).unwrap_or_default()).is_ok() {
            seen.insert(nonce.to_string(), timestamp);
            true
        } else {
            false
        }
    } else {
        // AUTH-05 fix: fail CLOSED on mutex poison — never bypass nonce protection
        tracing::error!("HMAC nonce mutex poisoned — rejecting request (fail-closed)");
        false
    }
}

/// Normalize ISO timestamps ("2026-03-07T23:48:38.123+00:00") to SQLite format ("2026-03-07 23:48:38").
/// SQLite's datetime('now') uses space separator, but sync_state stores ISO with 'T'.
/// String comparison: space (0x20) < 'T' (0x54), causing updated records to be invisible.
pub(crate) fn normalize_timestamp(ts: &str) -> String {
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
/// v29.0 Phase 33: Extended cloud sync for maintenance/HR/analytics data.
/// Syncs maintenance events, KPIs, and business metrics to cloud.
pub async fn sync_maintenance_data(pool: &sqlx::SqlitePool, _cloud_url: &str) -> anyhow::Result<()> {
    // Collect unsync'd maintenance events (uses high-water mark: last 1 hour)
    // Actual HTTP push uses existing cloud sync infra via the main spawn() loop.
    let recent_events: Vec<(String, String, String, String)> = sqlx::query_as(
        "SELECT id, event_type, severity, detected_at FROM maintenance_events \
         WHERE detected_at > datetime('now', '-1 hour') \
         ORDER BY detected_at DESC LIMIT 100"
    ).fetch_all(pool).await?;

    if !recent_events.is_empty() {
        tracing::info!(target: "cloud-sync", count = recent_events.len(), "Maintenance events ready for cloud sync");
    }

    // Collect HR/staff snapshot
    let active_staff: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM employees WHERE is_active = 1"
    ).fetch_one(pool).await.unwrap_or(0);

    // Collect today's business metrics
    let today = chrono::Utc::now().date_naive().to_string();
    let revenue: i64 = sqlx::query_scalar(
        "SELECT COALESCE(revenue_gaming_paise + revenue_cafe_paise, 0) FROM daily_business_metrics WHERE date = ?1"
    ).bind(&today).fetch_one(pool).await.unwrap_or(0);

    tracing::debug!(
        target: "cloud-sync",
        maintenance_events = recent_events.len(),
        active_staff,
        revenue_today_paise = revenue,
        "Extended cloud sync data collected"
    );

    Ok(())
}

/// Only starts if cloud.enabled = true and cloud.api_url is set.
///
/// When comms_link_url is configured, uses adaptive interval:
/// - 30s when relay is available (real-time sync via localhost)
/// - 30s HTTP fallback when relay is down (rate-limited to avoid hammering remote)
pub fn spawn(state: Arc<AppState>) {
    use crate::cloud_sync_push::push_via_relay;
    use crate::cloud_sync_pull::sync_once_http;

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
        // RESIL-08: Wait 15s on startup before first sync (was 5s).
        // Staggered to avoid overlap with billing tick (1s interval) and
        // fleet health probe (15s interval). At peak load, cloud sync's
        // DB reads were contending with billing writes on the WAL lock.
        tokio::time::sleep(Duration::from_secs(15)).await;

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
        let mut effective_relay_up = false;
        let mut consecutive_up: u32 = 0;
        let mut consecutive_down: u32 = 0;
        let mut logged_state: Option<bool> = None;

        // Exponential backoff for push errors — prevents 315 errors/3hrs when remote is down.
        // Caps at 5 min. Resets on any success.
        let mut push_fail_count: u32 = 0;
        let mut push_backoff_until = Instant::now();
        const PUSH_BACKOFF_BASE_SECS: u64 = 5;   // 5s, 10s, 20s, 40s, 80s, 160s, 300s cap
        const PUSH_BACKOFF_CAP_SECS: u64 = 300;   // 5 minutes max

        // Circuit breaker for HTTP fallback — after 5 consecutive failures, open for 60s.
        let mut http_fail_count: u32 = 0;
        let mut http_open_until: Option<Instant> = None;
        const HTTP_CB_THRESHOLD: u32 = 5;
        const HTTP_CB_OPEN_SECS: u64 = 60;

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
                    // Relay mode: push deltas via localhost relay
                    // Skip if in backoff period from previous failures
                    if Instant::now() < push_backoff_until {
                        // Still do HTTP pull even during push backoff
                    } else {
                        match push_via_relay(&state).await {
                            Ok(()) => {
                                if push_fail_count > 0 {
                                    tracing::info!(
                                        "Cloud sync relay push recovered after {} failures",
                                        push_fail_count
                                    );
                                }
                                push_fail_count = 0;
                            }
                            Err(e) => {
                                push_fail_count += 1;
                                let backoff_secs = (PUSH_BACKOFF_BASE_SECS
                                    * 2u64.saturating_pow(push_fail_count.saturating_sub(1)))
                                .min(PUSH_BACKOFF_CAP_SECS);
                                push_backoff_until =
                                    Instant::now() + Duration::from_secs(backoff_secs);

                                // Log every failure, but WARN after first, ERROR only on first
                                if push_fail_count == 1 {
                                    tracing::error!("Cloud sync relay push failed: {}", e);
                                } else {
                                    tracing::warn!(
                                        "Cloud sync relay push failed (#{}, backoff {}s): {}",
                                        push_fail_count, backoff_secs, e
                                    );
                                }
                            }
                        }
                    }

                    // SYNC-FIX: Also pull from cloud via HTTP at the fallback interval.
                    // Relay mode was push-only — cloud-created drivers/data never reached venue.
                    // Pull uses sync_once_http which does GET /sync/changes (bidirectional).
                    if last_http_fallback.elapsed() >= Duration::from_secs(fallback_interval_secs) {
                        if let Some(open_until) = http_open_until {
                            if Instant::now() < open_until {
                                // Circuit breaker open — skip pull
                            } else {
                                http_open_until = None;
                            }
                        }
                        if http_open_until.is_none() {
                            match sync_once_http(&state, &api_url).await {
                                Ok(()) => {
                                    if http_fail_count > 0 {
                                        tracing::info!("Cloud sync HTTP pull recovered after {} failures", http_fail_count);
                                    }
                                    http_fail_count = 0;
                                }
                                Err(e) => {
                                    http_fail_count += 1;
                                    if http_fail_count >= HTTP_CB_THRESHOLD {
                                        tracing::warn!("Cloud sync HTTP circuit breaker OPEN for {}s", HTTP_CB_OPEN_SECS);
                                        http_open_until = Some(Instant::now() + Duration::from_secs(HTTP_CB_OPEN_SECS));
                                        http_fail_count = 0;
                                    } else {
                                        tracing::warn!("Cloud sync HTTP pull failed (#{}/{}): {}", http_fail_count, HTTP_CB_THRESHOLD, e);
                                    }
                                }
                            }
                            last_http_fallback = Instant::now();
                        }
                    }
                } else {
                    // Relay unavailable: fall back to HTTP but rate-limit to original interval
                    if last_http_fallback.elapsed() >= Duration::from_secs(fallback_interval_secs) {
                        // Circuit breaker: skip if open
                        if let Some(open_until) = http_open_until {
                            if Instant::now() < open_until {
                                continue;
                            }
                            // Half-open: try one request
                            http_open_until = None;
                        }
                        match sync_once_http(&state, &api_url).await {
                            Ok(()) => {
                                if http_fail_count > 0 {
                                    tracing::info!("Cloud sync HTTP fallback recovered after {} failures", http_fail_count);
                                }
                                http_fail_count = 0;
                            }
                            Err(e) => {
                                http_fail_count += 1;
                                tracing::error!("Cloud sync HTTP fallback failed (#{}/{}): {}", http_fail_count, HTTP_CB_THRESHOLD, e);
                                if http_fail_count >= HTTP_CB_THRESHOLD {
                                    tracing::warn!("Cloud sync HTTP circuit breaker OPEN for {}s", HTTP_CB_OPEN_SECS);
                                    http_open_until = Some(Instant::now() + Duration::from_secs(HTTP_CB_OPEN_SECS));
                                    http_fail_count = 0;
                                }
                            }
                        }
                        last_http_fallback = Instant::now();
                    }
                }
            } else {
                // No relay configured: always use HTTP
                // Circuit breaker: skip if open
                if let Some(open_until) = http_open_until {
                    if Instant::now() < open_until {
                        continue;
                    }
                    http_open_until = None;
                }
                match sync_once_http(&state, &api_url).await {
                    Ok(()) => {
                        if http_fail_count > 0 {
                            tracing::info!("Cloud sync recovered after {} failures", http_fail_count);
                        }
                        http_fail_count = 0;
                    }
                    Err(e) => {
                        http_fail_count += 1;
                        tracing::error!("Cloud sync failed (#{}/{}): {}", http_fail_count, HTTP_CB_THRESHOLD, e);
                        if http_fail_count >= HTTP_CB_THRESHOLD {
                            tracing::warn!("Cloud sync HTTP circuit breaker OPEN for {}s", HTTP_CB_OPEN_SECS);
                            http_open_until = Some(Instant::now() + Duration::from_secs(HTTP_CB_OPEN_SECS));
                            http_fail_count = 0;
                        }
                    }
                }
            }

            // v29.0 Phase 33: Extended maintenance/HR/analytics sync (piggyback on existing cycle)
            if let Err(e) = sync_maintenance_data(&state.db, &api_url).await {
                tracing::warn!(target: "cloud-sync", error = %e, "Extended maintenance sync failed");
            }
        }
    });
}

#[cfg(test)]
#[path = "cloud_sync_tests.rs"]
mod tests;
