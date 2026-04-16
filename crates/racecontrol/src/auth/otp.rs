use std::sync::Arc;

use chrono::{Duration, Utc};
use rand::Rng;
use uuid::Uuid;

use crate::crypto::redaction::redact_phone;
use crate::state::AppState;

use super::create_jwt;

// ─── OTP Hashing (SEC-08) ─────────────────────────────────────────────

/// Hash a one-time password using Argon2id with a random salt.
/// Returns the PHC-format hash string (starts with "$argon2id$").
/// SEC-08: OTPs must be stored as hashes, never plaintext.
pub(crate) fn hash_otp(otp: &str) -> Result<String, String> {
    use argon2::{
        password_hash::{rand_core::OsRng, SaltString},
        Argon2, PasswordHasher,
    };
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    argon2
        .hash_password(otp.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| format!("Argon2 OTP hash error: {}", e))
}

/// Verify a plaintext OTP against an argon2 hash string.
/// Returns `true` if the OTP matches the hash, `false` otherwise (including on parse errors).
/// SEC-08: Must be called via spawn_blocking — argon2 verify is CPU-intensive.
pub(crate) fn verify_otp_hash(otp: &str, hash_str: &str) -> bool {
    use argon2::{Argon2, PasswordHash, PasswordVerifier};
    let Ok(parsed) = PasswordHash::new(hash_str) else {
        return false;
    };
    Argon2::default()
        .verify_password(otp.as_bytes(), &parsed)
        .is_ok()
}

// ─── OTP Send/Verify ──────────────────────────────────────────────────

/// Result of an OTP send attempt. `driver_id` is always set on success;
/// `delivered` indicates whether the WhatsApp message actually reached the API.
pub struct OtpSendResult {
    pub driver_id: String,
    pub delivered: bool,
}

pub async fn send_otp(state: &Arc<AppState>, phone: &str) -> Result<OtpSendResult, String> {
    // Find or create driver by phone (lookup via HMAC hash)
    let phone_hash = state.field_cipher.hash_phone(phone);
    let driver = sqlx::query_as::<_, (String, String)>(
        "SELECT id, name FROM drivers WHERE phone_hash = ?",
    )
    .bind(&phone_hash)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    let driver_id = match driver {
        Some((id, _)) => id,
        None => {
            // Auto-create driver with phone + generate customer_id
            let id = Uuid::new_v4().to_string();

            // Get next customer_id sequence number (numeric MAX to avoid lexicographic issues)
            let max_num = sqlx::query_as::<_, (Option<i64>,)>(
                "SELECT MAX(CAST(REPLACE(customer_id, 'RP', '') AS INTEGER)) FROM drivers WHERE customer_id IS NOT NULL AND customer_id LIKE 'RP%'",
            )
            .fetch_one(&state.db)
            .await
            .ok()
            .and_then(|r| r.0)
            .unwrap_or(0) as u32;
            let customer_id = format!("RP{:03}", max_num + 1);

            let phone_enc = state.field_cipher.encrypt_field(phone)
                .map_err(|e| format!("Encrypt error: {}", e))?;

            sqlx::query(
                "INSERT INTO drivers (id, name, phone_hash, phone_enc, customer_id, updated_at, venue_id) VALUES (?, ?, ?, ?, ?, datetime('now'), ?)",
            )
            .bind(&id)
            .bind(format!("Customer {}", &phone[phone.len().saturating_sub(4)..]))
            .bind(&phone_hash)
            .bind(&phone_enc)
            .bind(&customer_id)
            .bind(&state.config.venue.venue_id)
            .execute(&state.db)
            .await
            .map_err(|e| format!("DB error creating driver: {}", e))?;
            tracing::info!("New customer {} assigned ID {}", id, customer_id);
            id
        }
    };

    // Generate 6-digit OTP
    let otp: u32 = rand::thread_rng().gen_range(100000..=999999);
    let otp_str = format!("{:06}", otp);
    let expires_at = Utc::now() + Duration::seconds(state.config.auth.otp_expiry_secs as i64);

    // SEC-08: Hash the OTP before storing — plaintext OTPs must not be recoverable from DB
    let otp_hash = hash_otp(&otp_str).map_err(|e| format!("OTP hash error: {}", e))?;

    // Store hashed OTP in driver record
    sqlx::query("UPDATE drivers SET otp_code = ?, otp_expires_at = ? WHERE id = ?")
        .bind(&otp_hash)
        .bind(expires_at.to_rfc3339())
        .bind(&driver_id)
        .execute(&state.db)
        .await
        .map_err(|e| format!("DB error storing OTP: {}", e))?;

    // Send OTP via WhatsApp (Evolution API) — uses plaintext otp_str, not the hash
    let delivered = send_otp_whatsapp(state, phone, &otp_str).await;

    Ok(OtpSendResult { driver_id, delivered })
}

/// Send OTP message via WhatsApp Evolution API.
/// Returns `true` if the API accepted the message, `false` on any failure.
/// Uses the shared HTTP client from AppState with a 5-second timeout.
async fn send_otp_whatsapp(state: &Arc<AppState>, phone: &str, otp_str: &str) -> bool {
    let (evo_url, evo_key, evo_instance) = match (
        &state.config.auth.evolution_url,
        &state.config.auth.evolution_api_key,
        &state.config.auth.evolution_instance,
    ) {
        (Some(u), Some(k), Some(i)) => (u.clone(), k.clone(), i.clone()),
        _ => {
            tracing::info!("OTP generated for {} (Evolution API not configured)", redact_phone(phone));
            return false;
        }
    };

    let wa_phone = if phone.starts_with('+') {
        phone[1..].to_string()
    } else if phone.len() == 10 {
        format!("91{}", phone)
    } else {
        phone.to_string()
    };

    let url = format!("{}/message/sendText/{}", evo_url, evo_instance);
    let body = serde_json::json!({
        "number": wa_phone,
        "text": format!("\u{1f3ce}\u{fe0f} *RacingPoint*\n\nYour login code is: *{}*\n\nValid for {} minutes.", otp_str, state.config.auth.otp_expiry_secs / 60)
    });

    match state.http_client
        .post(&url)
        .header("apikey", &evo_key)
        .timeout(std::time::Duration::from_secs(5))
        .json(&body)
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            tracing::info!("OTP sent via WhatsApp to {}", redact_phone(&wa_phone));
            true
        }
        Ok(resp) => {
            tracing::error!("Evolution API returned {} for OTP to {}", resp.status(), redact_phone(phone));
            false
        }
        Err(e) => {
            tracing::error!("Failed to send OTP via WhatsApp: {}. OTP for {}", e, redact_phone(phone));
            false
        }
    }
}

/// Resend OTP for a phone number. Reuses the existing OTP if still valid,
/// otherwise generates a fresh one. Returns delivery status.
pub async fn resend_otp(state: &Arc<AppState>, phone: &str) -> Result<OtpSendResult, String> {
    let phone_hash = state.field_cipher.hash_phone(phone);
    let driver = sqlx::query_as::<_, (String, Option<String>, Option<String>)>(
        "SELECT id, otp_code, otp_expires_at FROM drivers WHERE phone_hash = ?",
    )
    .bind(&phone_hash)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?
    .ok_or_else(|| "Phone number not found. Please start login again.".to_string())?;

    let (driver_id, existing_otp, expires_at) = driver;

    // Reuse existing OTP if still valid (>30s remaining), otherwise generate new
    let otp_str = match (&existing_otp, &expires_at) {
        (Some(otp), Some(exp)) if !otp.is_empty() => {
            if let Ok(exp_dt) = chrono::DateTime::parse_from_rfc3339(exp) {
                if exp_dt > chrono::Utc::now() + chrono::Duration::seconds(30) {
                    otp.clone()
                } else {
                    // Almost expired — generate new
                    
                    generate_and_store_otp(state, &driver_id).await?
                }
            } else {
                
                generate_and_store_otp(state, &driver_id).await?
            }
        }
        _ => {
            
            generate_and_store_otp(state, &driver_id).await?
        }
    };

    let delivered = send_otp_whatsapp(state, phone, &otp_str).await;
    Ok(OtpSendResult { driver_id, delivered })
}

/// Generate a new 6-digit OTP and store it as an argon2 hash in the driver record.
/// Returns the plaintext OTP (needed for WhatsApp delivery), not the hash.
/// SEC-08: plaintext is only kept in memory for the duration of this function.
async fn generate_and_store_otp(state: &Arc<AppState>, driver_id: &str) -> Result<String, String> {
    let otp: u32 = rand::thread_rng().gen_range(100000..=999999);
    let otp_str = format!("{:06}", otp);
    let expires_at = Utc::now() + Duration::seconds(state.config.auth.otp_expiry_secs as i64);

    // SEC-08: Hash OTP before storing
    let otp_hash = hash_otp(&otp_str).map_err(|e| format!("OTP hash error: {}", e))?;

    sqlx::query("UPDATE drivers SET otp_code = ?, otp_expires_at = ? WHERE id = ?")
        .bind(&otp_hash)
        .bind(expires_at.to_rfc3339())
        .bind(driver_id)
        .execute(&state.db)
        .await
        .map_err(|e| format!("DB error storing OTP: {}", e))?;

    Ok(otp_str)
}

pub async fn verify_otp(state: &Arc<AppState>, phone: &str, otp: &str) -> Result<String, String> {
    let phone_hash = state.field_cipher.hash_phone(phone);
    let driver = sqlx::query_as::<_, (String, Option<String>, Option<String>)>(
        "SELECT id, otp_code, otp_expires_at FROM drivers WHERE phone_hash = ?",
    )
    .bind(&phone_hash)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?
    .ok_or_else(|| "Phone number not found".to_string())?;

    let driver_id = driver.0;
    let stored_otp = driver.1.ok_or_else(|| "No OTP pending".to_string())?;
    let expires_at = driver.2.ok_or_else(|| "No OTP pending".to_string())?;

    // Check expiry
    let expires = chrono::DateTime::parse_from_rfc3339(&expires_at)
        .map_err(|_| "Invalid expiry timestamp".to_string())?;
    if Utc::now() > expires {
        return Err("OTP has expired".to_string());
    }

    // SEC-08: Verify OTP using argon2 (constant-time, cryptographically secure)
    // spawn_blocking because argon2 verify is CPU-intensive and must not block tokio runtime
    let otp_owned = otp.to_string();
    let stored_otp_owned = stored_otp.clone();
    let valid = tokio::task::spawn_blocking(move || verify_otp_hash(&otp_owned, &stored_otp_owned))
        .await
        .unwrap_or(false);
    if !valid {
        return Err("Invalid OTP".to_string());
    }

    // Clear OTP and update login timestamp
    sqlx::query(
        "UPDATE drivers SET otp_code = NULL, otp_expires_at = NULL, phone_verified = 1, last_login_at = datetime('now'), updated_at = datetime('now') WHERE id = ?",
    )
    .bind(&driver_id)
    .execute(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    // Create JWT
    let jwt = create_jwt(&driver_id, &state.config.auth.jwt_secret)?;

    // Record customer session
    let session_id = Uuid::new_v4().to_string();
    let expires_at = Utc::now() + Duration::days(30);

    sqlx::query(
        "INSERT INTO customer_sessions (id, driver_id, token_hash, created_at, expires_at)
         VALUES (?, ?, ?, datetime('now'), ?)",
    )
    .bind(&session_id)
    .bind(&driver_id)
    .bind(&jwt[jwt.len().saturating_sub(32)..]) // store last 32 chars as hash
    .bind(expires_at.to_rfc3339())
    .execute(&state.db)
    .await
    .map_err(|e| format!("DB error creating session: {}", e))?;

    tracing::info!("Customer {} verified OTP and logged in", driver_id);

    Ok(jwt)
}

// ─── Guardian OTP (LEGAL-04/05) ────────────────────────────────────────────

/// Send a 6-digit OTP to a minor's guardian phone for consent verification.
/// Stores an argon2-hashed OTP in drivers.guardian_otp_code (SEC-08 compliant).
/// Returns true if WhatsApp delivery succeeded; false if Evolution API is not configured
/// or the send failed (OTP is still stored — staff can relay it verbally).
pub async fn send_guardian_otp(
    state: &Arc<AppState>,
    driver_id: &str,
    guardian_phone: &str,
) -> Result<OtpSendResult, String> {
    // Verify the driver exists
    let exists: Option<(String,)> = sqlx::query_as("SELECT id FROM drivers WHERE id = ?")
        .bind(driver_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| format!("DB error: {}", e))?;
    if exists.is_none() {
        return Err(format!("Driver '{}' not found", driver_id));
    }

    // Generate 6-digit OTP
    let otp: u32 = rand::thread_rng().gen_range(100000..=999999);
    let otp_str = format!("{:06}", otp);
    let expires_at = Utc::now() + Duration::seconds(state.config.auth.otp_expiry_secs as i64);

    // SEC-08: Hash the OTP before storing — plaintext must not be recoverable from DB
    let otp_hash = hash_otp(&otp_str).map_err(|e| format!("OTP hash error: {}", e))?;

    // Store hashed OTP + reset verified flag (new send invalidates any previous verification)
    sqlx::query(
        "UPDATE drivers SET guardian_otp_code = ?, guardian_otp_expires_at = ?, guardian_otp_verified = 0, guardian_phone = ? WHERE id = ?",
    )
    .bind(&otp_hash)
    .bind(expires_at.to_rfc3339())
    .bind(guardian_phone)
    .bind(driver_id)
    .execute(&state.db)
    .await
    .map_err(|e| format!("DB error storing guardian OTP: {}", e))?;

    // Send via WhatsApp — reuse existing Evolution API send logic
    let delivered = send_otp_whatsapp(state, guardian_phone, &otp_str).await;

    tracing::info!(
        driver_id = %driver_id,
        guardian = %redact_phone(guardian_phone),
        delivered = %delivered,
        "Guardian OTP sent for minor consent (LEGAL-04)"
    );

    Ok(OtpSendResult {
        driver_id: driver_id.to_string(),
        delivered,
    })
}

/// Verify a guardian's OTP for a minor customer.
/// On success: sets guardian_otp_verified=1 and guardian_otp_verified_at on the driver record.
/// Returns Ok(true) on valid OTP, Ok(false) on invalid hash.
/// Returns Err on DB failure, missing OTP, or expired OTP.
pub async fn verify_guardian_otp(
    state: &Arc<AppState>,
    driver_id: &str,
    otp: &str,
) -> Result<bool, String> {
    // Fetch stored guardian OTP hash and expiry
    let row: Option<(Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT guardian_otp_code, guardian_otp_expires_at FROM drivers WHERE id = ?",
    )
    .bind(driver_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    let (stored_otp_hash, expires_at_str) = row
        .ok_or_else(|| format!("Driver '{}' not found", driver_id))?;

    let stored_hash = stored_otp_hash.ok_or_else(|| "No guardian OTP pending".to_string())?;
    let expires_str = expires_at_str.ok_or_else(|| "No guardian OTP pending".to_string())?;

    // Check expiry (same 5-min window as regular OTP)
    let expires = chrono::DateTime::parse_from_rfc3339(&expires_str)
        .map_err(|_| "Invalid OTP expiry timestamp".to_string())?;
    if Utc::now() > expires {
        return Err("Guardian OTP has expired".to_string());
    }

    // SEC-08: Verify via argon2 — spawn_blocking because argon2 verify is CPU-intensive
    let otp_owned = otp.to_string();
    let hash_owned = stored_hash.clone();
    let valid = tokio::task::spawn_blocking(move || verify_otp_hash(&otp_owned, &hash_owned))
        .await
        .unwrap_or(false);

    if !valid {
        return Ok(false);
    }

    // Mark guardian OTP as verified — billing gate will pass on next start_billing call
    sqlx::query(
        "UPDATE drivers SET guardian_otp_verified = 1, guardian_otp_verified_at = datetime('now') WHERE id = ?",
    )
    .bind(driver_id)
    .execute(&state.db)
    .await
    .map_err(|e| format!("DB error marking guardian OTP verified: {}", e))?;

    tracing::info!(driver_id = %driver_id, "Guardian OTP verified — minor billing gate will pass (LEGAL-04)");

    Ok(true)
}
