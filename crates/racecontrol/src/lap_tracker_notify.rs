//! Track record notification and helper utilities for lap_tracker.
//!
//! Extracted from `lap_tracker.rs` for module size compliance (<500 lines).
//! Contains: compute_assist_evidence, send_gmail, format_lap_time, get_previous_record_holder.

use crate::config::GmailConfig;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;

/// Compute assist evidence fields from an assist config JSON string.
/// Returns (assist_config_hash, assist_tier).
///
/// assist_tier derivation rules (UX-06):
///   - 'pro'      = traction_control=0, stability_control=0, abs=0, ideal_line=false
///   - 'amateur'  = ideal_line=true (visual assistance — strongest assist)
///   - 'semi-pro' = any other assist on (TC, SC, or ABS), but not ideal_line
///   - 'unknown'  = assist config not available
///
/// The config JSON is sorted-key serialized before hashing for reproducibility.
pub(crate) fn compute_assist_evidence(assist_config_json: Option<&str>) -> (Option<String>, String) {
    let Some(json_str) = assist_config_json else {
        return (None, "unknown".to_string());
    };
    // Parse the JSON as a BTreeMap for stable key ordering
    let config: std::collections::BTreeMap<String, serde_json::Value> =
        match serde_json::from_str(json_str) {
            Ok(m) => m,
            Err(_) => return (None, "unknown".to_string()),
        };

    // Compute SHA-256 of the canonically sorted JSON
    let canonical = serde_json::to_string(&config).unwrap_or_default();
    let hash = format!("{:x}", Sha256::digest(canonical.as_bytes()));

    // Derive tier from assist values
    // Keys expected: traction_control, stability_control, abs, autoclutch, ideal_line
    let get_bool = |key: &str| -> bool {
        config
            .get(key)
            .map(|v| match v {
                serde_json::Value::Bool(b) => *b,
                serde_json::Value::Number(n) => n.as_f64().unwrap_or(0.0) > 0.0,
                _ => false,
            })
            .unwrap_or(false)
    };
    let get_int = |key: &str| -> i64 {
        config
            .get(key)
            .and_then(|v| v.as_i64())
            .unwrap_or(0)
    };

    let ideal_line = get_bool("ideal_line");
    let tc = get_int("traction_control");
    let sc = get_int("stability_control");
    let abs_val = get_int("abs");

    let tier = if ideal_line {
        "amateur"
    } else if tc == 0 && sc == 0 && abs_val == 0 {
        "pro"
    } else {
        "semi-pro"
    };

    (Some(hash), tier.to_string())
}

// ─── Gmail API (native, no external script) ──────────────────────────────────

pub(crate) async fn send_gmail(
    http: &reqwest::Client,
    gmail: &GmailConfig,
    to: &str,
    subject: &str,
    body: &str,
) -> Result<(), String> {
    if !gmail.enabled {
        return Err("Gmail not enabled in config".into());
    }
    let client_id = gmail.client_id.as_deref().ok_or("gmail.client_id missing")?;
    let client_secret = gmail.client_secret.as_deref().ok_or("gmail.client_secret missing")?;
    let refresh_token = gmail.refresh_token.as_deref().ok_or("gmail.refresh_token missing")?;

    // Step 1: Exchange refresh_token for access_token
    let token_resp = http
        .post("https://oauth2.googleapis.com/token")
        .form(&[
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("refresh_token", refresh_token),
            ("grant_type", "refresh_token"),
        ])
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| format!("Token request failed: {}", e))?;

    if !token_resp.status().is_success() {
        let status = token_resp.status();
        let body = token_resp.text().await.unwrap_or_default();
        return Err(format!("Token refresh failed ({}): {}", status, body));
    }

    let token_json: serde_json::Value = token_resp
        .json()
        .await
        .map_err(|e| format!("Token parse failed: {}", e))?;
    let access_token = token_json["access_token"]
        .as_str()
        .ok_or("No access_token in response")?;

    // Step 2: Build RFC 2822 message and base64url encode
    let from = &gmail.from_email;
    let raw_message = format!(
        "From: {}\r\nTo: {}\r\nSubject: {}\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n{}",
        from, to, subject, body
    );
    let encoded = URL_SAFE_NO_PAD.encode(raw_message.as_bytes());

    // Step 3: Send via Gmail API
    let send_resp = http
        .post("https://gmail.googleapis.com/gmail/v1/users/me/messages/send")
        .bearer_auth(access_token)
        .json(&serde_json::json!({ "raw": encoded }))
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| format!("Gmail send failed: {}", e))?;

    if !send_resp.status().is_success() {
        let status = send_resp.status();
        let body = send_resp.text().await.unwrap_or_default();
        return Err(format!("Gmail send failed ({}): {}", status, body));
    }

    Ok(())
}

/// Fetch the current track record holder's best time, name, and email for a given track+car.
///
/// Returns `Some((best_lap_ms, driver_name, Option<email>))` if a record exists,
/// or `None` if no record has been set for this track+car combination.
///
/// This function is called BEFORE the UPSERT in `persist_lap()` so that the
/// previous holder's data is captured before it gets overwritten.
pub async fn get_previous_record_holder(
    db: &SqlitePool,
    track: &str,
    car: &str,
    sim_type: &str,
) -> Option<(i64, String, Option<String>)> {
    sqlx::query_as::<_, (i64, String, Option<String>)>(
        "SELECT tr.best_lap_ms, d.name, d.email
         FROM track_records tr
         JOIN drivers d ON tr.driver_id = d.id
         WHERE tr.track = ? AND tr.car = ? AND tr.sim_type = ?",
    )
    .bind(track)
    .bind(car)
    .bind(sim_type)
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
}

/// Format a lap time in milliseconds as M:SS.mmm (e.g., 90123 -> "1:30.123").
pub(crate) fn format_lap_time(ms: i64) -> String {
    let minutes = ms / 60000;
    let seconds = (ms % 60000) / 1000;
    let millis = ms % 1000;
    format!("{}:{:02}.{:03}", minutes, seconds, millis)
}
