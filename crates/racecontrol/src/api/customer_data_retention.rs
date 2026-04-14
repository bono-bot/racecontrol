#![allow(unused_imports)]
use std::sync::Arc;

use crate::state::AppState;

// ─── LEGAL-08: Data retention background job ─────────────────────────────────
/// Spawned at server startup (in main.rs). Runs daily with a 1-hour initial delay
/// to avoid congestion at boot. Reads pii_inactive_months from data_retention_config
/// and anonymizes drivers who have been inactive beyond that threshold.
///
/// Financial records (journal_entries, invoices, billing_sessions, wallet_transactions)
/// are never touched — retained for 8 years per Income Tax Act.
pub async fn spawn_data_retention_job(state: Arc<AppState>) {
    tracing::info!(
        target: "data_retention",
        "data-retention task started (86400s interval, 3600s initial delay)"
    );
    // Initial delay: 1 hour — avoid boot congestion alongside orphan detector, reconciler, etc.
    tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;

    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(86400));
    loop {
        interval.tick().await;
        run_pii_anonymization_cycle(state.clone()).await;
    }
}

/// Single anonymization cycle — called daily by spawn_data_retention_job.
pub(crate) async fn run_pii_anonymization_cycle(state: Arc<AppState>) {
    // Read retention policy from config table
    let policy: Option<(i64,)> = sqlx::query_as(
        "SELECT pii_inactive_months FROM data_retention_config WHERE id = 'default'",
    )
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    let pii_inactive_months = policy.map(|r| r.0).unwrap_or(24);

    // Find drivers inactive beyond the threshold who have not yet been anonymized
    // and have not already revoked consent (those are handled immediately on revocation).
    // BUG FIX (2026-04-06): last_activity_at IS NULL matched newly registered drivers
    // who hadn't raced yet. Added created_at check to exclude drivers created within
    // the retention window — a driver registered 17 minutes ago is NOT "inactive for 24 months."
    let inactive_drivers: Vec<(String,)> = sqlx::query_as(
        "SELECT id FROM drivers
         WHERE (last_activity_at IS NULL
                OR last_activity_at < datetime('now', '-' || ? || ' months'))
           AND created_at < datetime('now', '-' || ? || ' months')
           AND COALESCE(pii_anonymized, 0) = 0
           AND COALESCE(consent_revoked, 0) = 0
         LIMIT 500",
    )
    .bind(pii_inactive_months)
    .bind(pii_inactive_months)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let mut anonymized_count: u32 = 0;
    for (driver_id,) in inactive_drivers {
        let result = sqlx::query(
            "UPDATE drivers SET
                name = 'ANONYMIZED-' || substr(id, 1, 8),
                email = NULL,
                phone = NULL,
                phone_hash = NULL,
                guardian_name = NULL,
                guardian_phone = NULL,
                guardian_phone_hash = NULL,
                dob = NULL,
                pii_anonymized = 1,
                pii_anonymized_at = datetime('now')
            WHERE id = ? AND COALESCE(pii_anonymized, 0) = 0",
        )
        .bind(&driver_id)
        .execute(&state.db)
        .await;

        match result {
            Ok(r) if r.rows_affected() > 0 => anonymized_count += 1,
            Ok(_) => {} // already anonymized between SELECT and UPDATE — idempotent
            Err(e) => tracing::warn!(
                target: "data_retention",
                driver_id = %driver_id,
                "Failed to anonymize inactive driver: {}",
                e
            ),
        }
    }

    tracing::info!(
        target: "data_retention",
        count = anonymized_count,
        threshold_months = pii_inactive_months,
        "PII anonymization cycle complete"
    );
}
