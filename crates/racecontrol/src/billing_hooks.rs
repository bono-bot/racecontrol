//! Billing post-session hooks — referral rewards, review nudges, membership hours,
//! WhatsApp receipts, commitment ladder, PDF receipt generation.
//!
//! Extracted from billing.rs (Phase 385, v49.0 Architecture Completion).
//! Called after a billing session ends. All hooks are best-effort (fire-and-forget).

use std::sync::Arc;

use crate::crypto::redaction::redact_phone;
use crate::state::AppState;

// ─── WhatsApp Phone Formatting ────────────────────────────────────────────────

/// Format a phone number for WhatsApp (Evolution API format).
/// Strips leading '+', prepends '91' for 10-digit Indian numbers.
pub fn format_wa_phone(phone: &str) -> String {
    if phone.starts_with('+') {
        phone[1..].to_string()
    } else if phone.len() == 10 {
        format!("91{}", phone)
    } else {
        phone.to_string()
    }
}

// ─── Receipt Formatting & PDF ─────────────────────────────────────────────────

/// Format a WhatsApp receipt message for a completed session.
pub fn format_receipt_message(
    first_name: &str,
    driving_secs: i64,
    cost_paise: i64,
    best_lap_ms: Option<i64>,
    balance_paise: i64,
) -> String {
    let duration_min = driving_secs / 60;
    let duration_sec = driving_secs % 60;
    let cost_credits = cost_paise / 100;
    let balance_credits = balance_paise / 100;

    let best_lap_text = match best_lap_ms.filter(|&ms| ms > 0) {
        Some(ms) => {
            let mins = ms / 60000;
            let secs = (ms % 60000) / 1000;
            let millis = ms % 1000;
            format!("{}:{:02}.{:03}", mins, secs, millis)
        }
        None => "No valid laps".to_string(),
    };

    format!(
        "\u{1f3c1} *RacingPoint \u{2014} Session Complete*\n\nHey {}!\n\n\u{23f1} Duration: {}m {}s\n\u{1f4b0} Cost: {} credits\n\u{1f3ce} Best Lap: {}\n\u{1f4b3} Wallet Balance: {} credits\n\nSee you on track! \u{1f3c1}",
        first_name, duration_min, duration_sec, cost_credits, best_lap_text, balance_credits
    )
}

/// Generate a minimal PDF receipt (80mm thermal style) using raw PDF commands.
/// No external crate needed — Courier + Courier-Bold are built-in PDF fonts.
fn generate_receipt_pdf(
    first_name: &str, driving_secs: i64, cost_paise: i64,
    best_lap_ms: Option<i64>, balance_paise: i64, session_id: &str,
) -> Vec<u8> {
    let (pw, ph) = (227.0_f64, 397.0_f64); // 80mm x 140mm in points
    let mins = driving_secs / 60;
    let secs = driving_secs % 60;
    let credits = cost_paise / 100;
    let bal = balance_paise / 100;
    let lap = match best_lap_ms {
        Some(ms) if ms > 0 => format!("{}:{:02}.{:03}", ms/60000, (ms/1000)%60, ms%1000),
        _ => "No valid laps".to_string(),
    };
    let sid = if session_id.len() >= 8 { &session_id[..8] } else { session_id };
    let sep = "--------------------------------";

    // Build content stream with text positioning
    let mut s = String::from("BT\n");
    let mut y = ph - 30.0;
    let line = |s: &mut String, font: &str, sz: f64, txt: &str, y: &mut f64| {
        let esc = txt.replace('\\', "\\\\").replace('(', "\\(").replace(')', "\\)");
        s.push_str(&format!("{} {} Tf\n12 {} Td\n({}) Tj\n", font, sz, *y, esc));
        *y -= sz + 4.0;
    };
    line(&mut s, "/F2", 14.0, "    RACING POINT", &mut y);
    line(&mut s, "/F1", 10.0, "     eSports & Cafe", &mut y);
    y -= 6.0;
    line(&mut s, "/F1", 9.0, sep, &mut y);
    line(&mut s, "/F1", 9.0, &format!("Session:  {}", sid), &mut y);
    line(&mut s, "/F1", 9.0, &format!("Customer: {}", first_name), &mut y);
    line(&mut s, "/F1", 9.0, &format!("Duration: {}m {}s", mins, secs), &mut y);
    line(&mut s, "/F1", 9.0, &format!("Best Lap: {}", lap), &mut y);
    line(&mut s, "/F1", 9.0, sep, &mut y);
    line(&mut s, "/F2", 11.0, &format!("TOTAL:    {} credits", credits), &mut y);
    line(&mut s, "/F1", 9.0, &format!("Balance:  {} credits", bal), &mut y);
    line(&mut s, "/F1", 9.0, sep, &mut y);
    line(&mut s, "/F1", 9.0, "  Thank you for racing!", &mut y);
    line(&mut s, "/F1", 9.0, "     racingpoint.in", &mut y);
    s.push_str("ET\n");
    let slen = s.len();

    let mut p = String::from("%PDF-1.4\n");
    let o1 = p.len(); p.push_str("1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");
    let o2 = p.len(); p.push_str("2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n");
    let o3 = p.len(); p.push_str(&format!("3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Contents 4 0 R /Resources << /Font << /F1 5 0 R /F2 6 0 R >> >> >>\nendobj\n", pw, ph));
    let o4 = p.len(); p.push_str(&format!("4 0 obj\n<< /Length {} >>\nstream\n", slen));
    p.push_str(&s); p.push_str("endstream\nendobj\n");
    let o5 = p.len(); p.push_str("5 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Courier >>\nendobj\n");
    let o6 = p.len(); p.push_str("6 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Courier-Bold >>\nendobj\n");
    let xr = p.len();
    p.push_str("xref\n0 7\n0000000000 65535 f \n");
    for o in [o1, o2, o3, o4, o5, o6] { p.push_str(&format!("{:010} 00000 n \n", o)); }
    p.push_str(&format!("trailer\n<< /Size 7 /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n", xr));
    p.into_bytes()
}

// ─── WhatsApp Receipt Sending ─────────────────────────────────────────────────

/// Send a WhatsApp receipt for a completed session via Evolution API.
/// Best-effort: never blocks session end, 5-second timeout.
pub async fn send_whatsapp_receipt(state: &Arc<AppState>, session_id: &str, driver_id: &str) {
    // Get driver phone
    let driver: Option<(String, Option<String>)> = sqlx::query_as(
        "SELECT name, phone FROM drivers WHERE id = ?",
    )
    .bind(driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let (driver_name, phone) = match driver {
        Some((name, Some(phone))) if !phone.is_empty() => (name, phone),
        Some((name, _)) => {
            tracing::warn!("No phone for driver {} ({}) -- skipping WhatsApp receipt", driver_id, name);
            return;
        }
        None => return,
    };

    // Get session details
    let session: Option<(i64, i64)> = sqlx::query_as(
        "SELECT driving_seconds, COALESCE(wallet_debit_paise, COALESCE(custom_price_paise, (SELECT price_paise FROM pricing_tiers WHERE id = billing_sessions.pricing_tier_id)), 0)
         FROM billing_sessions WHERE id = ?",
    )
    .bind(session_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let (driving_secs, cost_paise) = match session {
        Some(s) => s,
        None => return,
    };

    // Best lap
    let best_lap: Option<(i64,)> = sqlx::query_as(
        "SELECT MIN(lap_time_ms) FROM laps WHERE session_id = ? AND valid = 1",
    )
    .bind(session_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    // Wallet balance
    let balance: Option<(i64,)> = sqlx::query_as(
        "SELECT COALESCE(SUM(CASE WHEN txn_type LIKE 'credit%' OR txn_type LIKE 'refund%' THEN amount_paise ELSE -amount_paise END), 0) FROM wallet_transactions WHERE driver_id = ?",
    )
    .bind(driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let first_name = driver_name.split_whitespace().next().unwrap_or("Racer");
    let balance_paise = balance.map(|b| b.0).unwrap_or(0);
    let best_lap_ms = best_lap.map(|b| b.0);

    // Send via Evolution API
    if let (Some(evo_url), Some(evo_key), Some(evo_instance)) = (
        &state.config.auth.evolution_url,
        &state.config.auth.evolution_api_key,
        &state.config.auth.evolution_instance,
    ) {
        let wa_phone = format_wa_phone(&phone);

        // 5-second timeout -- receipt is best-effort, never block session end
        let client = match reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("Failed to build HTTP client for receipt: {}", e);
                return;
            }
        };

        // Phase 4.1: Try PDF receipt first via sendMedia
        let pdf_bytes = generate_receipt_pdf(
            first_name, driving_secs, cost_paise, best_lap_ms, balance_paise, session_id,
        );
        let pdf_b64 = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD, &pdf_bytes,
        );
        let media_url = format!("{}/message/sendMedia/{}", evo_url, evo_instance);
        let media_body = serde_json::json!({
            "number": wa_phone,
            "mediatype": "document",
            "mimetype": "application/pdf",
            "caption": format!("Racing Point - Session Receipt ({}m {}s)", driving_secs / 60, driving_secs % 60),
            "media": format!("data:application/pdf;base64,{}", pdf_b64),
            "fileName": format!("RacingPoint_Receipt_{}.pdf", &session_id[..std::cmp::min(8, session_id.len())]),
        });

        let sent_pdf = match client.post(&media_url).header("apikey", evo_key).json(&media_body).send().await {
            Ok(resp) if resp.status().is_success() => {
                tracing::info!("WhatsApp PDF receipt sent to {} for session {}", redact_phone(&wa_phone), session_id);
                true
            }
            Ok(resp) => {
                tracing::warn!("sendMedia returned {} for {} -- falling back to text", resp.status(), redact_phone(&wa_phone));
                false
            }
            Err(e) => {
                tracing::warn!("sendMedia failed for {}: {} -- falling back to text", redact_phone(&wa_phone), e);
                false
            }
        };

        // Fallback: plain text message
        if !sent_pdf {
            let message = format_receipt_message(first_name, driving_secs, cost_paise, best_lap_ms, balance_paise);
            let text_url = format!("{}/message/sendText/{}", evo_url, evo_instance);
            let text_body = serde_json::json!({ "number": wa_phone, "text": message });
            match client.post(&text_url).header("apikey", evo_key).json(&text_body).send().await {
                Ok(resp) if resp.status().is_success() => {
                    tracing::info!("WhatsApp text receipt sent to {} for session {}", redact_phone(&wa_phone), session_id);
                }
                Ok(resp) => {
                    tracing::warn!("sendText returned {} for receipt to {}", resp.status(), redact_phone(&wa_phone));
                }
                Err(e) => {
                    tracing::warn!("Failed to send text receipt to {}: {}", redact_phone(&wa_phone), e);
                }
            }
        }
    } else {
        tracing::debug!("Evolution API not configured -- skipping WhatsApp receipt for session {}", session_id);
    }
}

// ─── Driver Phone Helper ──────────────────────────────────────────────────────

// UX-03: Fetch a driver's phone number for notification purposes.
// Returns Err if driver not found, anonymized (phone is NULL), or phone is empty.
async fn get_driver_phone(db: &sqlx::SqlitePool, driver_id: &str) -> anyhow::Result<String> {
    let row: Option<(Option<String>,)> = sqlx::query_as(
        "SELECT phone FROM drivers WHERE id = ?",
    )
    .bind(driver_id)
    .fetch_optional(db)
    .await?;

    match row {
        Some((Some(phone),)) if !phone.is_empty() => Ok(phone),
        Some(_) => anyhow::bail!("Driver {} has no phone number", driver_id),
        None => anyhow::bail!("Driver {} not found", driver_id),
    }
}

// ─── Post-Session Hooks ───────────────────────────────────────────────────────

/// Record a post-session hook failure as a billing_event for operator visibility.
/// Errors here are also logged but never propagate — the session is already ended.
async fn record_hook_failure(
    state: &Arc<AppState>,
    session_id: &str,
    hook_name: &str,
    detail: &serde_json::Value,
) {
    let meta = serde_json::json!({
        "hook": hook_name,
        "detail": detail,
    })
    .to_string();
    if let Err(ie) = sqlx::query(
        "INSERT INTO billing_events (id, billing_session_id, event_type, metadata, venue_id) \
         VALUES (?, ?, 'post_session_hook_failed', ?, ?)",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(session_id)
    .bind(&meta)
    .bind(&state.config.venue.venue_id)
    .execute(&state.db)
    .await
    {
        tracing::error!(
            session_id = %session_id,
            hook = %hook_name,
            "Failed to record post_session_hook_failed billing event: {}",
            ie
        );
    }
}

/// Post-session hooks: credit referral rewards, schedule review nudge.
pub async fn post_session_hooks(
    state: &Arc<AppState>,
    session_id: &str,
    driver_id: &str,
    seconds_covered: u32,
    pod_id: &str,
) {
    tracing::info!(
        session_id = %session_id,
        driver_id = %driver_id,
        pod_id = %pod_id,
        "post_session_hooks: start"
    );

    // Phase 364 CONSIST-01: clear rolling lap history to prevent stale data leaking to next session
    if let Some(pod) = state.pods.write().await.get_mut(pod_id) {
        pod.recent_lap_times.clear();
    } else {
        tracing::warn!(pod_id = %pod_id, "post_session_hooks: pod not in state.pods — skipping lap clear");
    }

    // 1. Credit referral reward if this is the referee's first completed session
    let pending_referral: Option<(String, String)> = match sqlx::query_as(
        "SELECT r.id, r.referrer_id FROM referrals r
         WHERE r.referee_id = ? AND r.reward_credited = 0",
    )
    .bind(driver_id)
    .fetch_optional(&state.db)
    .await
    {
        Ok(row) => row,
        Err(e) => {
            tracing::error!(session_id = %session_id, driver_id = %driver_id, "referral lookup failed: {}", e);
            record_hook_failure(state, session_id, "referral_lookup",
                &serde_json::json!({ "driver_id": driver_id, "error": e.to_string() })).await;
            None
        }
    };

    if let Some((referral_id, referrer_id)) = pending_referral {
        // Credit 100 credits (₹100 = 10000 paise) to referrer
        if let Err(e) = crate::wallet::credit(
            state,
            &referrer_id,
            10000,
            "referral_reward",
            Some(&referral_id),
            Some("Referral reward — friend completed first session"),
            None,
        )
        .await {
            tracing::error!(
                session_id = %session_id, referral_id = %referral_id, referrer_id = %referrer_id,
                "referral_reward credit FAILED: {}", e
            );
            record_hook_failure(state, session_id, "referral_reward_credit", &serde_json::json!({
                "referral_id": referral_id, "referrer_id": referrer_id,
                "amount_paise": 10000, "error": e.to_string(),
            })).await;
        }
        // Credit 50 credits to referee
        if let Err(e) = crate::wallet::credit(
            state,
            driver_id,
            5000,
            "referral_bonus",
            Some(&referral_id),
            Some("Welcome reward — referred by a friend"),
            None,
        )
        .await {
            tracing::error!(
                session_id = %session_id, referral_id = %referral_id, referee_id = %driver_id,
                "referral_bonus credit FAILED: {}", e
            );
            record_hook_failure(state, session_id, "referral_bonus_credit", &serde_json::json!({
                "referral_id": referral_id, "referee_id": driver_id,
                "amount_paise": 5000, "error": e.to_string(),
            })).await;
        }
        // Marking the referral credited is CRITICAL — if this fails but credits succeeded,
        // the next session will re-credit (double-reward risk). Log + record event so operators
        // can manually reconcile.
        if let Err(e) = sqlx::query("UPDATE referrals SET reward_credited = 1 WHERE id = ?")
            .bind(&referral_id)
            .execute(&state.db)
            .await
        {
            tracing::error!(
                session_id = %session_id, referral_id = %referral_id,
                "referral marker UPDATE FAILED — double-credit risk on next session: {}", e
            );
            record_hook_failure(state, session_id, "referral_marker_update", &serde_json::json!({
                "referral_id": referral_id, "severity": "double_credit_risk",
                "error": e.to_string(),
            })).await;
        } else {
            tracing::info!("Referral reward credited: referrer={}, referee={}", referrer_id, driver_id);
        }
    }

    // 2. Schedule review nudge (record for WhatsApp bot to pick up)
    let already_nudged: Option<(i64,)> = match sqlx::query_as(
        "SELECT COUNT(*) FROM review_nudges WHERE driver_id = ?",
    )
    .bind(driver_id)
    .fetch_optional(&state.db)
    .await
    {
        Ok(row) => row,
        Err(e) => {
            tracing::warn!(session_id = %session_id, driver_id = %driver_id,
                "review_nudges lookup failed: {}", e);
            None
        }
    };

    // Only nudge once per driver
    if already_nudged.map(|c| c.0 == 0).unwrap_or(true) {
        if let Err(e) = sqlx::query(
            "INSERT INTO review_nudges (id, driver_id, billing_session_id, sent_at, venue_id) VALUES (?, ?, ?, NULL, ?)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(driver_id)
        .bind(session_id)
        .bind(&state.config.venue.venue_id)
        .execute(&state.db)
        .await
        {
            tracing::warn!(session_id = %session_id, driver_id = %driver_id,
                "review_nudge INSERT failed — driver will not receive review prompt: {}", e);
            record_hook_failure(state, session_id, "review_nudge_insert",
                &serde_json::json!({ "driver_id": driver_id, "error": e.to_string() })).await;
        }
    }

    // 3. Update membership hours if member
    let membership: Option<(String, f64)> = match sqlx::query_as(
        "SELECT m.id, bs.driving_seconds / 3600.0
         FROM memberships m
         JOIN billing_sessions bs ON bs.driver_id = m.driver_id AND bs.id = ?
         WHERE m.driver_id = ? AND m.status = 'active'",
    )
    .bind(session_id)
    .bind(driver_id)
    .fetch_optional(&state.db)
    .await
    {
        Ok(row) => row,
        Err(e) => {
            tracing::error!(session_id = %session_id, driver_id = %driver_id,
                "membership lookup failed — active member hours may not update: {}", e);
            record_hook_failure(state, session_id, "membership_lookup",
                &serde_json::json!({ "driver_id": driver_id, "error": e.to_string() })).await;
            None
        }
    };

    if let Some((membership_id, hours_used)) = membership {
        // Membership hours is financial — customer paid for hours that won't be tracked if UPDATE fails.
        if let Err(e) = sqlx::query(
            "UPDATE memberships SET hours_used = hours_used + ? WHERE id = ?",
        )
        .bind(hours_used)
        .bind(&membership_id)
        .execute(&state.db)
        .await
        {
            tracing::error!(
                session_id = %session_id, membership_id = %membership_id, hours_used = hours_used,
                "membership hours UPDATE FAILED — member's hours under-counted: {}", e
            );
            record_hook_failure(state, session_id, "membership_hours_update", &serde_json::json!({
                "membership_id": membership_id, "driver_id": driver_id,
                "hours_used_unapplied": hours_used, "error": e.to_string(),
            })).await;
        }
    }

    // 4. Send WhatsApp receipt (best-effort, direct Evolution API)
    send_whatsapp_receipt(state, session_id, driver_id).await;

    // UX-03: Also enqueue receipt notification via durable outbox (retry-capable fallback)
    if let Ok(phone) = get_driver_phone(&state.db, driver_id).await {
        // Fetch driving_seconds and cost for the message
        let session_info: Option<(i64, i64)> = sqlx::query_as(
            "SELECT driving_seconds, COALESCE(wallet_debit_paise, 0) FROM billing_sessions WHERE id = ?",
        )
        .bind(session_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();

        if let Some((secs, cost_paise)) = session_info {
            let receipt_url = format!("/customer/sessions/{}/receipt", session_id);
            let msg = format!(
                "Your Racing Point session is complete! Duration: {}min {}s, Charged: Rs.{:.2}. View receipt: {}",
                secs / 60, secs % 60,
                cost_paise as f64 / 100.0,
                receipt_url
            );
            let _ = crate::notification_outbox::enqueue_notification(
                &state.db, &phone, "whatsapp", &msg, Some("receipt"), Some(session_id),
            )
            .await
            .map_err(|e| tracing::warn!("UX-03: Failed to enqueue receipt notification for session {}: {}", session_id, e));
        }
    }

    // 5. Evaluate badges for this driver (fire-and-forget, errors logged internally)
    crate::psychology::evaluate_badges(state, driver_id).await;

    // 6. Update visit streak for this driver
    crate::psychology::update_streak(state, driver_id).await;

    // 7. Maybe grant variable reward for milestone (10% probability, capped at 5% spend)
    crate::psychology::maybe_grant_variable_reward(state, driver_id, "milestone").await;

    // 8. Evaluate commitment ladder and queue escalation nudge (v14.0 Phase 94)
    evaluate_commitment_ladder(state, driver_id).await;

    // 9. Phase 363 GLD-C-01/C-02: Session audit (lap count + telemetry coverage flags)
    // CLAUDE.md: feature_flags is a RwLock — snapshot + drop guard before any .await.
    // The read guard is dropped inside run_session_audit before DB awaits.
    if let Err(e) = crate::session_audit::run_session_audit(
        &state.db,
        &state.feature_flags,
        session_id,
        seconds_covered,
    )
    .await
    {
        tracing::warn!(
            session_id = %session_id,
            error = %e,
            "session_audit failed — audit columns may be NULL for this session"
        );
    }
}

/// Evaluate driver's commitment ladder position based on completed session count.
/// Queue WhatsApp nudge at escalation thresholds (2 sessions → package, 5 → membership).
async fn evaluate_commitment_ladder(state: &Arc<AppState>, driver_id: &str) {
    let session_count: i64 = match sqlx::query_scalar(
        "SELECT COUNT(*) FROM billing_sessions
         WHERE driver_id = ? AND status IN ('completed', 'ended_early')
         AND is_trial = 0"
    )
    .bind(driver_id)
    .fetch_one(&state.db)
    .await
    {
        Ok(n) => n,
        Err(e) => {
            // Without the session count we skip ladder evaluation entirely rather than
            // silently default to 0 (which would wrongly demote to 'trial' and re-fire nudges).
            tracing::warn!(driver_id = %driver_id,
                "commitment_ladder session_count query failed — skipping evaluation: {}", e);
            return;
        }
    };

    let (new_position, should_nudge, nudge_message) = match session_count {
        0     => ("trial",   false, ""),
        1     => ("single",  false, ""),
        2     => ("single",  true,  "You've done 2 sessions at RacingPoint! Save 20% with a 5-pack — ask at the counter."),
        3..=4 => ("package", false, ""),
        5     => ("package", true,  "5 sessions in! Become a RacingPoint member for unlimited sessions and priority booking."),
        _     => ("member",  false, ""),
    };

    // Update ladder position
    if let Err(e) = sqlx::query(
        "UPDATE drivers SET commitment_ladder = ? WHERE id = ?"
    )
    .bind(new_position)
    .bind(driver_id)
    .execute(&state.db)
    .await
    {
        tracing::warn!(driver_id = %driver_id, new_position = %new_position,
            "commitment_ladder UPDATE failed: {}", e);
    }

    // Queue nudge if at escalation point (with 7-day dedup)
    if should_nudge {
        let already_sent: bool = match sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM nudge_queue
             WHERE driver_id = ? AND template = ?
             AND created_at >= datetime('now', '-7 days')"
        )
        .bind(driver_id)
        .bind(nudge_message)
        .fetch_one(&state.db)
        .await
        {
            Ok(v) => v,
            Err(e) => {
                // Fail closed: if we can't check dedup, skip the nudge rather than spam.
                tracing::warn!(driver_id = %driver_id,
                    "nudge dedup query failed — skipping nudge to avoid spam risk: {}", e);
                return;
            }
        };

        if !already_sent {
            crate::psychology::queue_notification(
                state,
                driver_id,
                crate::psychology::NotificationChannel::Whatsapp,
                3, // priority 3 (lower than PB notifications)
                nudge_message,
                "{}",
            )
            .await;
        }
    }
}
