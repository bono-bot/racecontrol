//! Act 3: Customer visit lifecycle management.
//!
//! A "visit" groups all sessions + cafe orders for a customer during one venue visit.
//! - Opens automatically when a billing session starts (if no open visit exists)
//! - Closes via: staff kiosk "End Visit", customer WhatsApp link, or 1-hour auto-close
//! - Receipt sent on close (WhatsApp + PWA + print on request)

use std::sync::Arc;
use crate::state::AppState;

/// Open or retrieve an existing open visit for a driver.
/// Called at billing session start — creates a visit if none is open.
/// Returns the visit_id.
pub async fn open_or_get_visit(
    state: &Arc<AppState>,
    driver_id: &str,
) -> Result<String, String> {
    // Check for existing open visit
    let existing = sqlx::query_as::<_, (String,)>(
        "SELECT id FROM visits WHERE driver_id = ? AND status = 'open' ORDER BY created_at DESC LIMIT 1",
    )
    .bind(driver_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    if let Some((visit_id,)) = existing {
        return Ok(visit_id);
    }

    // Create new visit
    let visit_id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO visits (id, driver_id, status, venue_id) VALUES (?, ?, 'open', ?)",
    )
    .bind(&visit_id)
    .bind(driver_id)
    .bind(&state.config.venue.venue_id)
    .execute(&state.db)
    .await
    .map_err(|e| format!("DB error creating visit: {}", e))?;

    tracing::info!("Visit opened: {} for driver {}", visit_id, driver_id);
    Ok(visit_id)
}

/// End a visit — closes the visit, calculates totals, triggers receipt.
/// Validates that all linked racers have no active sessions before closing.
/// `end_method`: "staff" (kiosk button), "customer" (WhatsApp link), "auto" (1-hour timeout)
pub async fn end_visit(
    state: &Arc<AppState>,
    visit_id: &str,
    end_method: &str,
) -> Result<VisitSummary, String> {
    // Verify visit exists and is open
    let visit = sqlx::query_as::<_, (String, String)>(
        "SELECT id, driver_id FROM visits WHERE id = ? AND status = 'open'",
    )
    .bind(visit_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?
    .ok_or_else(|| format!("Visit '{}' not found or already closed", visit_id))?;

    let (_vid, driver_id) = visit;

    // Check for active sessions on this driver OR any linked racers
    let active_sessions = sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM billing_sessions bs \
         JOIN drivers d ON bs.driver_id = d.id \
         WHERE (d.id = ? OR d.linked_to = ?) \
         AND bs.status IN ('active', 'paused_manual', 'paused_game_pause', 'paused_disconnect', 'paused_crash_recovery', 'waiting_for_game')",
    )
    .bind(&driver_id)
    .bind(&driver_id)
    .fetch_one(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    if active_sessions.0 > 0 {
        return Err(format!(
            "Cannot end visit — {} active session(s) for this customer or linked racers",
            active_sessions.0
        ));
    }

    // Calculate visit totals
    let totals = sqlx::query_as::<_, (i64, i64)>(
        "SELECT COUNT(*), COALESCE(SUM(COALESCE(wallet_debit_paise, 0)), 0) \
         FROM billing_sessions WHERE visit_id = ?",
    )
    .bind(visit_id)
    .fetch_one(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    let (total_sessions, total_spent) = totals;

    // Close the visit
    sqlx::query(
        "UPDATE visits SET status = 'closed', ended_at = datetime('now'), end_method = ?, \
         total_sessions = ?, total_spent_paise = ?, updated_at = datetime('now') \
         WHERE id = ?",
    )
    .bind(end_method)
    .bind(total_sessions)
    .bind(total_spent)
    .bind(visit_id)
    .execute(&state.db)
    .await
    .map_err(|e| format!("DB error closing visit: {}", e))?;

    tracing::info!(
        "Visit closed: {} (method={}, sessions={}, spent={}p)",
        visit_id, end_method, total_sessions, total_spent
    );

    // Get wallet balance for receipt
    let wallet_balance = sqlx::query_as::<_, (i64,)>(
        "SELECT COALESCE(balance_paise, 0) FROM wallets WHERE driver_id = ?",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .map(|(b,)| b)
    .unwrap_or(0);

    Ok(VisitSummary {
        visit_id: visit_id.to_string(),
        driver_id,
        total_sessions: total_sessions as u32,
        total_spent_paise: total_spent,
        wallet_balance_paise: wallet_balance,
        end_method: end_method.to_string(),
    })
}

/// Background task: auto-close visits with no activity for 1 hour.
/// Runs every 5 minutes, checks for open visits where the last session ended > 1 hour ago.
pub async fn auto_close_stale_visits(state: &Arc<AppState>) {
    let stale_visits: Vec<(String,)> = sqlx::query_as(
        "SELECT v.id FROM visits v \
         WHERE v.status = 'open' \
         AND NOT EXISTS ( \
             SELECT 1 FROM billing_sessions bs \
             WHERE bs.visit_id = v.id \
             AND bs.status IN ('active', 'paused_manual', 'paused_game_pause', 'paused_disconnect', 'paused_crash_recovery', 'waiting_for_game') \
         ) \
         AND ( \
             SELECT MAX(bs2.ended_at) FROM billing_sessions bs2 WHERE bs2.visit_id = v.id \
         ) < datetime('now', '-1 hour') \
         AND v.started_at < datetime('now', '-1 hour')",
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    for (visit_id,) in stale_visits {
        match end_visit(state, &visit_id, "auto").await {
            Ok(summary) => {
                tracing::info!(
                    "Auto-closed stale visit {} ({}p spent, {} sessions)",
                    summary.visit_id, summary.total_spent_paise, summary.total_sessions
                );
                // TODO: Send WhatsApp receipt on auto-close
            }
            Err(e) => {
                tracing::warn!("Failed to auto-close visit {}: {}", visit_id, e);
            }
        }
    }
}

/// Summary returned when a visit is closed.
#[derive(Debug, Clone)]
pub struct VisitSummary {
    pub visit_id: String,
    pub driver_id: String,
    pub total_sessions: u32,
    pub total_spent_paise: i64,
    pub wallet_balance_paise: i64,
    pub end_method: String,
}
