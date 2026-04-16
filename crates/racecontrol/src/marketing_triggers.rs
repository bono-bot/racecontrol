//! Autonomous Marketing Triggers (Phase 388).
//!
//! Detects empty pods during typically busy hours and sends targeted offers
//! to opted-in customers. Uses Phase 387 can_send_promo() for anti-spam enforcement.
//!
//! Design constraints (Uday's hard rule — see feedback_marketing_opt_out.md):
//! - NEVER flood customers. Opt-in required, easy opt-out, frequency cap, engagement throttle.
//! - Only send during venue open hours (10:00-23:00 IST).
//! - Max 1 marketing sweep per hour.

use std::sync::Arc;
use chrono::{Utc, Timelike, Datelike, Weekday, Duration as ChronoDuration};
use crate::api::customer_preferences;
use crate::config::Config;
use crate::state::AppState;
use crate::whatsapp_alerter::send_whatsapp_to;

// ─── Constants ──────────────────────────────────────────────────────────────

/// Minimum empty pods to trigger marketing (don't market when venue is nearly full)
const MIN_EMPTY_PODS_TO_TRIGGER: usize = 4;

/// Venue open hours in IST (UTC+5:30)
const VENUE_OPEN_HOUR_IST: u32 = 10;
const VENUE_CLOSE_HOUR_IST: u32 = 23;

/// Max customers to contact per sweep
const MAX_CONTACTS_PER_SWEEP: usize = 5;

// ─── Offer types ────────────────────────────────────────────────────────────

#[derive(Debug)]
struct MarketingOffer {
    driver_id: String,
    phone: String,
    name: String,
    message: String,
    campaign_id: String,
}

// ─── Pod utilization check ──────────────────────────────────────────────────

/// Count how many pods have active billing sessions right now.
async fn count_active_pods(state: &Arc<AppState>) -> usize {
    let count: Option<(i32,)> = sqlx::query_as(
        "SELECT COUNT(DISTINCT pod_id) FROM billing_sessions WHERE status = 'active'"
    )
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    count.map(|(c,)| c as usize).unwrap_or(0)
}

/// Check if current time is within venue operating hours (IST).
fn is_venue_hours() -> bool {
    let now_utc = Utc::now();
    // IST = UTC + 5:30
    let ist = now_utc + ChronoDuration::hours(5) + ChronoDuration::minutes(30);
    let hour = ist.hour();
    hour >= VENUE_OPEN_HOUR_IST && hour < VENUE_CLOSE_HOUR_IST
}

/// Check if it's a typically busy period (weekday evenings or weekends).
fn is_typically_busy() -> bool {
    let now_utc = Utc::now();
    let ist = now_utc + ChronoDuration::hours(5) + ChronoDuration::minutes(30);
    let hour = ist.hour();
    let weekday = ist.weekday();

    match weekday {
        Weekday::Sat | Weekday::Sun => hour >= 11 && hour < 22,
        _ => hour >= 16 && hour < 22, // weekday evenings
    }
}

// ─── Customer targeting ─────────────────────────────────────────────────────

/// Find customers who are eligible for a promotional offer.
/// Criteria: opted in, not auto-paused, has phone, visited in last 90 days,
/// not currently at venue (no active session).
async fn find_eligible_customers(state: &Arc<AppState>, limit: usize) -> Vec<(String, String, String)> {
    // Returns (driver_id, phone, name) for eligible customers
    let rows: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT d.id, d.phone, d.name FROM drivers d
         LEFT JOIN customer_preferences cp ON cp.driver_id = d.id
         WHERE d.phone IS NOT NULL
           AND d.phone != ''
           AND d.registration_completed = 1
           AND (cp.opt_in_promotions IS NULL OR cp.opt_in_promotions = 1)
           AND (cp.auto_paused IS NULL OR cp.auto_paused = 0)
           AND d.last_activity_at > datetime('now', '-90 days')
           AND d.id NOT IN (SELECT driver_id FROM billing_sessions WHERE status = 'active')
         ORDER BY d.last_activity_at DESC
         LIMIT ?"
    )
    .bind(limit as i32)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    rows
}

// ─── Offer generation ───────────────────────────────────────────────────────

/// Generate a personalized offer message for a customer.
async fn generate_offer(state: &Arc<AppState>, driver_id: &str, name: &str, empty_pods: usize) -> Option<MarketingOffer> {
    // Check if we can send (frequency cap, opt-in, etc.)
    if let Err(reason) = customer_preferences::can_send_promo(state, driver_id).await {
        tracing::debug!(target: "marketing", "Skipping {}: {}", driver_id, reason);
        return None;
    }

    // Get customer's session count for personalization
    let session_count: Option<(i32,)> = sqlx::query_as(
        "SELECT COUNT(*) FROM billing_sessions WHERE driver_id = ?"
    )
    .bind(driver_id)
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    let sessions = session_count.map(|(c,)| c).unwrap_or(0);

    // Pick offer type based on customer history
    let (message, campaign_id) = if sessions == 0 {
        // Never visited — first-timer offer
        (
            format!(
                "Hey {}! RacingPoint has {} open rigs right now. \
                 First-timers get a FREE 5-min trial session — \
                 come experience sim racing today! Walk-ins welcome.",
                name, empty_pods
            ),
            "auto_first_timer".to_string(),
        )
    } else if sessions <= 3 {
        // Early returner — value offer
        (
            format!(
                "Hi {}! {} pods available right now at RacingPoint. \
                 Book a 60-min session for just Rs.900 — \
                 that is 25% cheaper per minute than 30-min! See you soon.",
                name, empty_pods
            ),
            "auto_value_offer".to_string(),
        )
    } else {
        // Regular — combo deal
        (
            format!(
                "Hey {}! Quiet hour at RacingPoint — {} pods free. \
                 Book 1 hour and grab a free coffee on us. \
                 Walk in or message to reserve. See you!",
                name, empty_pods
            ),
            "auto_combo_deal".to_string(),
        )
    };

    // Get phone from the query that found this customer
    let phone: Option<(String,)> = sqlx::query_as(
        "SELECT phone FROM drivers WHERE id = ? AND phone IS NOT NULL"
    )
    .bind(driver_id)
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    let phone = phone?.0;

    Some(MarketingOffer {
        driver_id: driver_id.to_string(),
        phone,
        name: name.to_string(),
        message,
        campaign_id,
    })
}

// ─── Main sweep ─────────────────────────────────────────────────────────────

/// Run one marketing sweep: check utilization, find targets, send offers.
/// Called periodically from background_tasks (every 30 min during venue hours).
pub async fn run_marketing_sweep(state: &Arc<AppState>, config: &Config) {
    // Guard: only during venue hours
    if !is_venue_hours() {
        tracing::debug!(target: "marketing", "Outside venue hours, skipping sweep");
        return;
    }

    // Guard: only when pods are significantly empty
    let total_pods: usize = 8;
    let active = count_active_pods(state).await;
    let empty = total_pods.saturating_sub(active);

    if empty < MIN_EMPTY_PODS_TO_TRIGGER {
        tracing::debug!(target: "marketing", "Only {} empty pods (threshold: {}), skipping", empty, MIN_EMPTY_PODS_TO_TRIGGER);
        return;
    }

    // Guard: only during typically busy periods (don't market during expected quiet times)
    if !is_typically_busy() {
        tracing::debug!(target: "marketing", "Not a typically busy period, skipping");
        return;
    }

    tracing::info!(target: "marketing", "Marketing sweep: {}/{} pods empty during busy hours", empty, total_pods);

    // Find eligible customers
    let candidates = find_eligible_customers(state, MAX_CONTACTS_PER_SWEEP * 2).await;
    if candidates.is_empty() {
        tracing::info!(target: "marketing", "No eligible customers found");
        return;
    }

    let mut sent = 0;
    for (driver_id, _phone, name) in &candidates {
        if sent >= MAX_CONTACTS_PER_SWEEP {
            break;
        }

        let offer = match generate_offer(state, driver_id, name, empty).await {
            Some(o) => o,
            None => continue,
        };

        // Send via WhatsApp
        send_whatsapp_to(config, &offer.phone, &offer.message).await;

        // Record in promo_delivery_log and update weekly counter
        if let Err(e) = customer_preferences::record_promo_sent(
            state,
            &offer.driver_id,
            Some(&offer.campaign_id),
            "whatsapp",
            &offer.message[..offer.message.len().min(100)],
        ).await {
            tracing::warn!(target: "marketing", "Failed to record promo send: {}", e);
        }

        sent += 1;
        tracing::info!(target: "marketing", "Sent {} offer to {} ({})", offer.campaign_id, offer.name, offer.driver_id);
    }

    // Log low_utilization event
    if sent > 0 {
        let _ = sqlx::query(
            "INSERT INTO activity_log (event_type, details, created_at)
             VALUES ('low_utilization_marketing', ?, datetime('now'))"
        )
        .bind(format!("Sent {} offers. {}/{} pods empty during busy hours.", sent, empty, total_pods))
        .execute(&state.db)
        .await;
    }

    tracing::info!(target: "marketing", "Sweep complete: {} offers sent to {} candidates", sent, candidates.len());
}
