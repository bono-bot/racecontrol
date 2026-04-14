//! Cloud sync merge/upsert functions — per-table merge logic for pull operations.
//! Extracted from cloud_sync.rs (Phase 385, v49.0 Architecture Completion).

use std::sync::Arc;
use serde_json::Value;
use crate::state::AppState;

pub async fn upsert_driver(state: &Arc<AppState>, driver: &Value) -> anyhow::Result<()> {
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

    // Clear customer_id from any other driver row to avoid UNIQUE constraint violation.
    // Cloud is authoritative: if it says this driver owns this customer_id, release it elsewhere.
    if let Some(cid) = driver.get("customer_id").and_then(|v| v.as_str()) {
        if !cid.is_empty() {
            sqlx::query("UPDATE drivers SET customer_id = NULL WHERE customer_id = ? AND id != ?")
                .bind(cid)
                .bind(id)
                .execute(&state.db)
                .await?;
        }
    }

    // Upsert — cloud wins for customer-owned fields, preserve local-only fields (otp_code etc.)
    // PII stored in _enc/_hash columns only; plaintext columns set to NULL.
    // venue_id: preserve from cloud payload if present, else use local venue_id
    let driver_venue_id = driver.get("venue_id").and_then(|v| v.as_str())
        .unwrap_or(state.config.venue.venue_id.as_str());
    sqlx::query(
        "INSERT INTO drivers (id, customer_id, name, name_enc, phone_hash, phone_enc, email_enc,
            steam_guid, iracing_id, avatar_url,
            total_laps, total_time_ms, has_used_trial, unlimited_trials, pin_hash, phone_verified,
            dob, waiver_signed, waiver_signed_at, waiver_version,
            guardian_name, guardian_phone_hash, guardian_phone_enc, registration_completed, signature_data,
            created_at, updated_at, venue_id)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28)
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
    .bind(driver_venue_id)                                                      // ?28
    .execute(&state.db)
    .await?;

    let name = driver.get("name").and_then(|v| v.as_str()).unwrap_or("?");
    tracing::debug!("Synced driver: {} ({})", name, id);

    Ok(())
}

pub async fn upsert_wallet(state: &Arc<AppState>, wallet: &Value) -> anyhow::Result<()> {
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
    let cloud_rupee_deposited = wallet
        .get("rupee_deposited_paise")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let cloud_rupee_refunded = wallet
        .get("rupee_refunded_paise")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let cloud_bonus_credited = wallet
        .get("bonus_credited_paise")
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
    let local = sqlx::query_as::<_, (i64, i64, i64, i64, i64, i64)>(
        "SELECT balance_paise, total_credited_paise, total_debited_paise, rupee_deposited_paise, rupee_refunded_paise, bonus_credited_paise FROM wallets WHERE driver_id = ?",
    )
    .bind(&local_driver_id)
    .fetch_optional(&state.db)
    .await?;

    match local {
        Some((_local_bal, _local_credited, _local_debited, _local_rupee_dep, _local_rupee_ref, _local_bonus_cr)) => {
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
                        rupee_deposited_paise = ?,
                        rupee_refunded_paise = ?,
                        bonus_credited_paise = ?,
                        updated_at = ?
                     WHERE driver_id = ?",
                )
                .bind(cloud_balance)
                .bind(cloud_credited)
                .bind(cloud_debited)
                .bind(cloud_rupee_deposited)
                .bind(cloud_rupee_refunded)
                .bind(cloud_bonus_credited)
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
                "INSERT OR IGNORE INTO wallets (driver_id, balance_paise, total_credited_paise, total_debited_paise, rupee_deposited_paise, rupee_refunded_paise, bonus_credited_paise, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&local_driver_id)
            .bind(cloud_balance)
            .bind(cloud_credited)
            .bind(cloud_debited)
            .bind(cloud_rupee_deposited)
            .bind(cloud_rupee_refunded)
            .bind(cloud_bonus_credited)
            .bind(cloud_updated)
            .execute(&state.db)
            .await?;
        }
    }

    Ok(())
}

pub async fn upsert_pricing_tier(state: &Arc<AppState>, tier: &Value) -> anyhow::Result<()> {
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
            updated_at = excluded.updated_at
         WHERE excluded.updated_at > COALESCE(pricing_tiers.updated_at, '1970-01-01')",
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

pub async fn upsert_kiosk_experience(state: &Arc<AppState>, exp: &Value) -> anyhow::Result<()> {
    let id = exp
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Experience missing id"))?;

    // venue_id: preserve from cloud payload if present, else use local venue_id
    let exp_venue_id = exp.get("venue_id").and_then(|v| v.as_str())
        .unwrap_or(state.config.venue.venue_id.as_str());
    sqlx::query(
        "INSERT INTO kiosk_experiences (id, name, game, track, car, car_class, duration_minutes, start_type, ac_preset_id, sort_order, is_active, updated_at, venue_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
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
    .bind(exp_venue_id)
    .execute(&state.db)
    .await?;

    Ok(())
}

pub async fn upsert_pricing_rule(state: &Arc<AppState>, rule: &Value) -> anyhow::Result<()> {
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
pub async fn upsert_auth_token(state: &Arc<AppState>, token: &Value) -> anyhow::Result<()> {
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

