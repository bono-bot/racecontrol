//! Cafe marketing broadcast handler.
//!
//! POST /api/v1/cafe/marketing/broadcast — sends a WhatsApp message to all
//! drivers with a phone number via the Evolution API, with a 24h per-driver
//! in-memory cooldown (matching the security alert debounce pattern in
//! whatsapp_alerter.rs).

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use axum::{
    Json,
    extract::State,
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::state::AppState;

// ─── Rate-limit map ───────────────────────────────────────────────────────────

/// Keyed by driver_id → last broadcast Instant. Guards against hammering a
/// single driver's phone more than once every 24 hours.
static BROADCAST_COOLDOWN: LazyLock<Mutex<HashMap<String, Instant>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

const BROADCAST_COOLDOWN_SECS: u64 = 86_400; // 24 hours

// ─── Request / Response types ─────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct BroadcastRequest {
    pub message: String,
    pub promo_name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct BroadcastResponse {
    pub attempted: usize,
    pub sent: usize,
    pub skipped_cooldown: usize,
    pub skipped_no_phone: usize,
}

// ─── Driver row ───────────────────────────────────────────────────────────────

#[derive(Debug, sqlx::FromRow)]
struct DriverPhone {
    id: String,
    name: String,
    phone: Option<String>,
}

// ─── Handler ──────────────────────────────────────────────────────────────────

/// POST /api/v1/cafe/marketing/broadcast
///
/// Sends a WhatsApp message to all drivers with a phone number.
/// Applies a 24h per-driver cooldown so the same driver is never
/// messaged more than once per day. Returns a JSON summary of counts.
pub async fn broadcast_promo(
    State(state): State<Arc<AppState>>,
    Json(req): Json<BroadcastRequest>,
) -> Result<Json<BroadcastResponse>, (StatusCode, Json<serde_json::Value>)> {
    // Guard: Evolution API must be configured (marketing route preferred).
    let creds = match state.config.evolution_for(crate::config::WhatsAppCategory::Marketing) {
        Some(c) => c,
        None => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({ "error": "WhatsApp not configured" })),
            ));
        }
    };
    let (evo_url, evo_key, evo_instance) = (creds.url, creds.api_key, creds.instance);

    // Fetch all drivers that have a phone number stored.
    let rows: Vec<DriverPhone> = sqlx::query_as(
        "SELECT id, name, phone FROM drivers WHERE phone IS NOT NULL AND phone != ''",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| {
        tracing::error!(target: "cafe_marketing", "DB error fetching drivers: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "Database error" })),
        )
    })?;

    let mut attempted: usize = 0;
    let mut sent: usize = 0;
    let mut skipped_cooldown: usize = 0;
    let mut skipped_no_phone: usize = 0;

    // Build reqwest client once (5s timeout, shared across all sends).
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(target: "cafe_marketing", "Failed to build HTTP client: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "HTTP client build failed" })),
            ));
        }
    };

    for row in &rows {
        let phone = match &row.phone {
            Some(p) if !p.is_empty() => p.clone(),
            _ => {
                skipped_no_phone += 1;
                continue;
            }
        };

        attempted += 1;

        // Cooldown check — locked briefly, then released before async send.
        {
            let mut map = BROADCAST_COOLDOWN
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if let Some(last) = map.get(&row.id) {
                if last.elapsed().as_secs() < BROADCAST_COOLDOWN_SECS {
                    skipped_cooldown += 1;
                    continue;
                }
            }
            // Record this attempt before sending so concurrent requests don't
            // duplicate sends even if the Evolution call is slow.
            map.insert(row.id.clone(), Instant::now());
        }

        // Build the message text — optionally prefix with promo name.
        let text = if let Some(ref pname) = req.promo_name {
            format!("[{}] {}", pname, req.message)
        } else {
            req.message.clone()
        };

        let send_url = format!("{}/message/sendText/{}", evo_url, evo_instance);
        let body = serde_json::json!({
            "number": phone,
            "text": text,
        });

        match client
            .post(&send_url)
            .header("apikey", &evo_key)
            .json(&body)
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                sent += 1;
                tracing::info!(
                    target: "cafe_marketing",
                    "Broadcast sent to driver {} ({})",
                    row.name,
                    row.id
                );
            }
            Ok(resp) => {
                // Non-2xx from Evolution API — warn and continue, don't abort broadcast.
                tracing::warn!(
                    target: "cafe_marketing",
                    "Evolution API returned {} for driver {} ({})",
                    resp.status(),
                    row.name,
                    row.id
                );
                // Remove cooldown entry so a retry can re-attempt this driver.
                let mut map = BROADCAST_COOLDOWN
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                map.remove(&row.id);
            }
            Err(e) => {
                tracing::warn!(
                    target: "cafe_marketing",
                    "Send failed for driver {} ({}): {}",
                    row.name,
                    row.id,
                    e
                );
                // Remove cooldown entry so a retry can re-attempt this driver.
                let mut map = BROADCAST_COOLDOWN
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                map.remove(&row.id);
            }
        }
    }

    tracing::info!(
        target: "cafe_marketing",
        "Broadcast complete: attempted={} sent={} skipped_cooldown={} skipped_no_phone={}",
        attempted,
        sent,
        skipped_cooldown,
        skipped_no_phone,
    );

    Ok(Json(BroadcastResponse {
        attempted,
        sent,
        skipped_cooldown,
        skipped_no_phone,
    }))
}
