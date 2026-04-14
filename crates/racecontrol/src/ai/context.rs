//! Business context gathering and prompt building for AI chat endpoints.

use sqlx::SqlitePool;
use tokio::sync::RwLock;

use crate::billing::BillingManager;
use crate::game_launcher::GameManager;
use rc_common::types::PodInfo;

// ─── Business Context ────────────────────────────────────────────────────────

/// Gather live venue state from database + in-memory state.
pub async fn gather_business_context(
    db: &SqlitePool,
    pods: &RwLock<std::collections::HashMap<String, PodInfo>>,
    billing: &BillingManager,
    game_launcher: &GameManager,
) -> String {
    let mut ctx = String::new();

    // Today's sessions
    let today_sessions: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM billing_sessions WHERE date(started_at) = date('now')",
    )
    .fetch_one(db)
    .await
    .unwrap_or(0);

    // Today's revenue
    let today_revenue: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(p.price_paise), 0) FROM billing_sessions bs \
         JOIN pricing_tiers p ON bs.pricing_tier_id = p.id \
         WHERE date(bs.started_at) = date('now') AND bs.status IN ('completed', 'active', 'ended_early')",
    )
    .fetch_one(db)
    .await
    .unwrap_or(0);

    // This week revenue
    let week_revenue: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(p.price_paise), 0) FROM billing_sessions bs \
         JOIN pricing_tiers p ON bs.pricing_tier_id = p.id \
         WHERE bs.started_at >= datetime('now', '-7 days')",
    )
    .fetch_one(db)
    .await
    .unwrap_or(0);

    // Total drivers
    let total_drivers: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM drivers")
            .fetch_one(db)
            .await
            .unwrap_or(0);

    // MMA-#3: Redact exact revenue for external AI providers (Anthropic).
    // Ollama is venue-local, so full context is safe there.
    // Use ranges instead of exact figures to preserve diagnostic value.
    let revenue_today_range = match today_revenue / 100 {
        0 => "0".to_string(),
        1..=1000 => "low (<1K)".to_string(),
        1001..=5000 => "moderate (1K-5K)".to_string(),
        _ => "high (>5K)".to_string(),
    };
    let revenue_week_range = match week_revenue / 100 {
        0 => "0".to_string(),
        1..=10000 => "low (<10K)".to_string(),
        10001..=50000 => "moderate (10K-50K)".to_string(),
        _ => "high (>50K)".to_string(),
    };
    ctx.push_str(&format!(
        "Today's sessions: {}\nToday's revenue: {} INR\nThis week's revenue: {} INR\nTotal registered drivers: {}\n\n",
        today_sessions,
        revenue_today_range,
        revenue_week_range,
        total_drivers,
    ));

    // Active billing sessions
    let timers = billing.active_timers.read().await;
    if timers.is_empty() {
        ctx.push_str("Active billing sessions: none\n");
    } else {
        ctx.push_str("Active billing sessions:\n");
        for (_, timer) in timers.iter() {
            ctx.push_str(&format!(
                "  - Pod {}: {} ({}, {}s remaining)\n",
                timer.pod_id, timer.driver_name, timer.pricing_tier_name, timer.remaining_seconds()
            ));
        }
    }
    drop(timers);
    ctx.push('\n');

    // Connected pods
    let pods_map = pods.read().await;
    if pods_map.is_empty() {
        ctx.push_str("Connected pods: none\n");
    } else {
        ctx.push_str(&format!("Connected pods: {}\n", pods_map.len()));
        for (_id, pod) in pods_map.iter() {
            ctx.push_str(&format!(
                "  - {} (Pod #{}): {:?}, game: {:?}\n",
                pod.name, pod.number, pod.status, pod.current_game
            ));
        }
    }
    ctx.push('\n');

    // Active games
    let games = game_launcher.active_games.read().await;
    if !games.is_empty() {
        ctx.push_str("Active games:\n");
        for (pod_id, tracker) in games.iter() {
            ctx.push_str(&format!(
                "  - Pod {}: {:?} ({:?})\n",
                pod_id,
                tracker.to_info().sim_type,
                tracker.to_info().game_state
            ));
        }
        ctx.push('\n');
    }

    // Recent crashes (last 24h)
    let crashes = sqlx::query_as::<_, (String, String, Option<String>, String)>(
        "SELECT pod_id, sim_type, error_message, created_at FROM game_launch_events \
         WHERE event_type = 'crash' AND created_at > datetime('now', '-24 hours') \
         ORDER BY created_at DESC LIMIT 5",
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();

    if !crashes.is_empty() {
        ctx.push_str("Recent crashes (last 24h):\n");
        for (pod_id, sim, err, time) in &crashes {
            ctx.push_str(&format!(
                "  - {} on pod {} at {} ({})\n",
                sim,
                pod_id,
                time,
                err.as_deref().unwrap_or("no details")
            ));
        }
        ctx.push('\n');
    }

    // Pricing tiers
    let tiers = sqlx::query_as::<_, (String, i64, i64)>(
        "SELECT name, duration_minutes, price_paise FROM pricing_tiers WHERE is_active = 1 ORDER BY sort_order",
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();

    if !tiers.is_empty() {
        ctx.push_str("Pricing tiers:\n");
        for (name, mins, price) in &tiers {
            ctx.push_str(&format!("  - {}: {} min, {} INR\n", name, mins, price / 100));
        }
    }

    ctx
}

/// Build system prompt for staff/admin AI chat.
pub fn build_staff_prompt(context: &str) -> String {
    format!(
        "You are James, the AI operations assistant for RacingPoint eSports and Cafe \
        in Bandlaguda, Hyderabad. You help staff and admins with venue operations, \
        billing, pod management, and troubleshooting.\n\n\
        CURRENT VENUE STATE (live data):\n{}\n\n\
        Answer concisely and accurately based on the data above. If you don't have \
        enough data to answer, say so. Prices are in INR. Keep responses under 200 words \
        unless asked for detail.",
        context
    )
}

/// Build system prompt for customer AI chat.
pub fn build_customer_prompt(context: &str) -> String {
    format!(
        "You are Bono, the friendly AI assistant at RacingPoint eSports Cafe \
        in Bandlaguda, Hyderabad. You help customers with their racing stats, \
        venue info, pricing, and sim racing tips.\n\n\
        CUSTOMER & VENUE DATA:\n{}\n\n\
        Be friendly, enthusiastic, and knowledgeable about sim racing. Keep responses concise. \
        When mentioning lap times, use a format like \"1:23.456\". \
        Proactively share interesting facts like the fastest lap of the day when relevant. \
        If asked about other customers' private data, politely decline. \
        You may share public leaderboard data like track records and fastest laps.",
        context
    )
}

/// Gather customer-scoped context (only their own data).
pub async fn gather_customer_context(db: &SqlitePool, driver_id: &str) -> String {
    let mut ctx = String::new();

    // Driver info
    let driver = sqlx::query_as::<_, (String, i64, i64)>(
        "SELECT name, total_laps, total_time_ms FROM drivers WHERE id = ?",
    )
    .bind(driver_id)
    .fetch_optional(db)
    .await
    .ok()
    .flatten();

    if let Some((name, laps, time_ms)) = driver {
        ctx.push_str(&format!(
            "Customer: {}\nTotal laps: {}\nTotal drive time: {} minutes\n\n",
            name,
            laps,
            time_ms / 60000
        ));
    }

    // Personal bests
    let bests = sqlx::query_as::<_, (String, String, i64)>(
        "SELECT track, car, best_lap_ms FROM personal_bests WHERE driver_id = ? ORDER BY best_lap_ms ASC LIMIT 10",
    )
    .bind(driver_id)
    .fetch_all(db)
    .await
    .unwrap_or_default();

    if !bests.is_empty() {
        ctx.push_str("Personal bests:\n");
        for (track, car, lap_ms) in &bests {
            let secs = *lap_ms as f64 / 1000.0;
            ctx.push_str(&format!("  - {} ({}): {:.3}s\n", track, car, secs));
        }
        ctx.push('\n');
    }

    // Recent sessions
    let sessions = sqlx::query_as::<_, (String, String, i64, String)>(
        "SELECT pt.name, bs.pod_id, bs.driving_seconds, bs.started_at \
         FROM billing_sessions bs JOIN pricing_tiers pt ON bs.pricing_tier_id = pt.id \
         WHERE bs.driver_id = ? ORDER BY bs.started_at DESC LIMIT 5",
    )
    .bind(driver_id)
    .fetch_all(db)
    .await
    .unwrap_or_default();

    if !sessions.is_empty() {
        ctx.push_str("Recent sessions:\n");
        for (tier, pod, secs, started) in &sessions {
            ctx.push_str(&format!(
                "  - {} on pod {} ({} min driven, {})\n",
                tier, pod, secs / 60, started
            ));
        }
        ctx.push('\n');
    }

    // Venue pricing
    let tiers = sqlx::query_as::<_, (String, i64, i64)>(
        "SELECT name, duration_minutes, price_paise FROM pricing_tiers WHERE is_active = 1 ORDER BY sort_order",
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();

    ctx.push_str("Available pricing:\n");
    for (name, mins, price) in &tiers {
        ctx.push_str(&format!("  - {}: {} min, {} INR\n", name, mins, price / 100));
    }
    // Fastest lap of the day
    let fastest_today = sqlx::query_as::<_, (String, String, String, i64)>(
        "SELECT d.name, l.track, l.car, l.lap_time_ms \
         FROM laps l JOIN drivers d ON l.driver_id = d.id \
         WHERE date(l.created_at) = date('now') AND l.valid = 1 \
         ORDER BY l.lap_time_ms ASC LIMIT 1",
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten();

    if let Some((name, track, car, lap_ms)) = fastest_today {
        let mins = lap_ms / 60000;
        let secs = (lap_ms % 60000) as f64 / 1000.0;
        if mins > 0 {
            ctx.push_str(&format!(
                "\nFastest lap of the day: set by {} on {} with {} — {}:{:06.3}\n",
                name, track, car, mins, secs
            ));
        } else {
            ctx.push_str(&format!(
                "\nFastest lap of the day: set by {} on {} with {} — {:.3}s\n",
                name, track, car, secs
            ));
        }
    }

    ctx.push_str("\nGames available: Assetto Corsa, iRacing, Le Mans Ultimate, F1 25, Forza\n");
    ctx.push_str("Location: Bandlaguda, Hyderabad\n");

    ctx
}
