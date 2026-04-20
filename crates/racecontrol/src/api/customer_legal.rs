#![allow(unused_imports)]
use super::customer_auth::extract_driver_id;
use super::routes::anonymize_driver_pii;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde_json::{Value, json};
use sqlx::{Sqlite, Transaction};
use std::sync::Arc;

use crate::state::AppState;

// ─── DPDP §12 right-of-erasure target tables ────────────────────────────────
//
// Each entry: (table_name, &[fk_columns]). Multi-column entries generate
// `WHERE col1 = ? OR col2 = ? ...` with the driver_id bound once per column.
//
// Children-first is implicit: all entries FK back to drivers(id), not each other,
// so the order here only matters for the final driver row deletion (handled after
// this list). If a future entry introduces an inter-table FK (billing_sessions →
// laps for example), reorder accordingly.
//
// NOTE on financial tables: this endpoint (customer_data_delete) hard-deletes
// financial rows to match the "right-of-erasure full purge" product intent.
// revoke_consent_handler instead calls anonymize_driver_pii() which preserves
// financial rows for the Income Tax Act 8-year retention. Two paths, two
// policies. Any change to the financial-delete policy must be coordinated with
// the venue's accountant and updated in both docstrings + CLAUDE.md.
pub(crate) const ERASE_TABLES: &[(&str, &[&str])] = &[
    // ── Ordering-critical (FK to billing_sessions, must precede it) ─────────
    // session_feedback.billing_session_id REFERENCES billing_sessions(id) with
    // NO ACTION. If session_feedback rows survive the billing_sessions DELETE,
    // SQLite rejects the parent delete with SQLITE_CONSTRAINT.
    ("session_feedback", &["driver_id"]),
    // Financial (hard-delete per current product intent)
    ("wallet_transactions", &["driver_id"]),
    ("wallets", &["driver_id"]),
    ("refunds", &["driver_id"]),
    ("invoices", &["driver_id"]),
    ("debit_intents", &["driver_id"]),
    ("dispute_requests", &["driver_id"]),
    ("billing_sessions", &["driver_id"]),
    // ── Ordering-critical (FK to laps, must precede it) ─────────────────────
    // personal_bests.lap_id, track_records.lap_id, hotlap_event_entries.lap_id
    // all REFERENCE laps(id). personal_bests_v2 / track_records_v2 are the
    // Phase 88 rebuilt schema — on post-rename DBs these tables no longer
    // exist and the DELETE is a skippable no-op via is_missing_table_error.
    // All lap-child rows MUST delete before the laps parent.
    ("personal_bests", &["driver_id"]),
    ("personal_bests_v2", &["driver_id"]),
    ("track_records", &["driver_id"]),
    ("track_records_v2", &["driver_id"]),
    ("hotlap_event_entries", &["driver_id"]),
    ("laps", &["driver_id"]),
    // Gamification
    ("driver_ratings", &["driver_id"]),
    ("driver_achievements", &["driver_id"]),
    ("streaks", &["driver_id"]),
    ("driving_passport", &["driver_id"]),
    ("variable_reward_log", &["driver_id"]),
    ("championship_standings", &["driver_id"]),
    // Social
    ("friend_requests", &["sender_id", "receiver_id"]),
    ("friendships", &["driver_a_id", "driver_b_id"]),
    ("referrals", &["referrer_id", "referee_id"]),
    ("group_session_members", &["driver_id"]),
    // Competition
    ("tournament_matches", &["driver_a", "driver_b", "winner_id"]),
    ("tournament_registrations", &["driver_id"]),
    ("multiplayer_results", &["driver_id"]),
    ("event_entries", &["driver_id"]),
    // Kiosk / sessions / bookings
    ("customer_sessions", &["driver_id"]),
    ("pod_reservations", &["driver_id"]),
    ("auth_tokens", &["driver_id"]),
    ("cafe_orders", &["driver_id"]),
    ("game_launch_requests", &["driver_id"]),
    ("bookings", &["driver_id"]),
    ("reservations", &["driver_id"]),
    // Feedback / marketing
    ("coupon_redemptions", &["driver_id"]),
    ("memberships", &["driver_id"]),
    ("session_highlights", &["driver_id"]),
    ("review_nudges", &["driver_id"]),
    ("customer_preferences", &["driver_id"]),
    ("nudge_queue", &["driver_id"]),
    ("promo_delivery_log", &["driver_id"]),
];

// Pointer columns on non-PII tables — nulled out (UPDATE SET NULL) instead of
// DELETE. Used for:
//   - foreign keys on retention tables (visits) where the row survives but the
//     link to the erased driver must be severed;
//   - operational pointers (pods.current_driver_id) that dangle after delete;
//   - self-referential parent/child links (drivers.linked_to) — if left populated,
//     the final drivers DELETE would fail with SQLITE_CONSTRAINT on the FK
//     from any sub-driver row back to this driver.
//
// NOTE: every column listed here must be nullable. visits.driver_id is made
// nullable by the table-rebuild migration in migrate_cross_domain.rs.
pub(crate) const POINTER_TABLES: &[(&str, &str)] = &[
    ("pods", "current_driver_id"),
    ("drivers", "linked_to"),
    ("visits", "driver_id"),
];

// Tables whose rows reference a driver TRANSITIVELY — no direct driver_id
// column, only an FK to another table that has one. Each entry is a DELETE
// statement with exactly one `?` placeholder that receives driver_id.
//
// These execute AFTER POINTER_TABLES and BEFORE ERASE_TABLES so the parent
// rows (billing_sessions etc.) can be deleted without FK violations.
pub(crate) const TRANSITIVE_ERASE_SQL: &[(&str, &str)] = &[
    // split_sessions.parent_session_id REFERENCES billing_sessions(id).
    // Must be cleared before billing_sessions is deleted in ERASE_TABLES.
    (
        "split_sessions",
        "DELETE FROM split_sessions WHERE parent_session_id IN \
         (SELECT id FROM billing_sessions WHERE driver_id = ?)",
    ),
    // billing_events.billing_session_id REFERENCES billing_sessions(id).
    // Customer's per-session event stream (launch, timeout, refund-trigger,
    // tier-change). No direct driver_id — must clear transitively before
    // billing_sessions is deleted.
    (
        "billing_events",
        "DELETE FROM billing_events WHERE billing_session_id IN \
         (SELECT id FROM billing_sessions WHERE driver_id = ?)",
    ),
    // telemetry_samples.lap_id REFERENCES laps(id).
    // Customer telemetry recorded per-lap. Without this, samples outlive the
    // lap row and remain attributable to the erased driver via indirect join.
    // Must clear transitively before laps is deleted.
    (
        "telemetry_samples",
        "DELETE FROM telemetry_samples WHERE lap_id IN \
         (SELECT id FROM laps WHERE driver_id = ?)",
    ),
];

async fn erase_table_rows(
    tx: &mut Transaction<'_, Sqlite>,
    table: &str,
    columns: &[&str],
    driver_id: &str,
) -> Result<u64, sqlx::Error> {
    let where_clause = columns
        .iter()
        .map(|c| format!("{} = ?", c))
        .collect::<Vec<_>>()
        .join(" OR ");
    let sql = format!("DELETE FROM {} WHERE {}", table, where_clause);
    let mut q = sqlx::query(&sql);
    for _ in columns {
        q = q.bind(driver_id);
    }
    let r = q.execute(&mut **tx).await?;
    Ok(r.rows_affected())
}

async fn null_pointer_column(
    tx: &mut Transaction<'_, Sqlite>,
    table: &str,
    column: &str,
    driver_id: &str,
) -> Result<u64, sqlx::Error> {
    let sql = format!("UPDATE {} SET {} = NULL WHERE {} = ?", table, column, column);
    let r = sqlx::query(&sql).bind(driver_id).execute(&mut **tx).await?;
    Ok(r.rows_affected())
}

/// Returns true if the error is SQLite reporting that an expected table or
/// column does not exist — these are treated as a skippable no-op (migration
/// may not have run this shape yet on older DBs). Any other error class is a
/// genuine failure that must roll back.
///
/// Covers both:
///   - "no such table: foo"      (table absent entirely)
///   - "no such column: foo.bar" (table present, column added later)
///
/// This matters for `POINTER_TABLES` where the column itself is the thing
/// being nulled — a missing column is the direct equivalent of a missing
/// table, not a bug in the erase logic.
fn is_missing_table_error(e: &sqlx::Error) -> bool {
    let msg = format!("{}", e);
    msg.contains("no such table") || msg.contains("no such column")
}

// Re-export dispute handlers (BILL-08) — extracted to customer_disputes.rs
pub(crate) use super::customer_disputes::{
    create_dispute_handler,
    list_disputes_handler,
    dispute_details_handler,
    resolve_dispute_handler,
};

// Re-export data retention job (LEGAL-08) — extracted to customer_data_retention.rs
pub use super::customer_data_retention::spawn_data_retention_job;
pub(crate) use super::customer_data_retention::run_pii_anonymization_cycle;

/// GET /api/v1/customer/data-export
/// Returns a JSON dump of all customer data with decrypted PII fields.
/// Requires valid customer JWT.
pub(crate) async fn customer_data_export(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Result<(axum::http::StatusCode, Json<Value>), (axum::http::StatusCode, Json<Value>)> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => {
            return Err((
                axum::http::StatusCode::UNAUTHORIZED,
                Json(json!({ "error": e })),
            ))
        }
    };

    let driver = sqlx::query_as::<_, (String, String, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, i64, i64)>(
        "SELECT id, name, email, phone, name_enc, email_enc, phone_enc, total_laps, total_time_ms FROM drivers WHERE id = ?",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await;

    let d = match driver {
        Ok(Some(row)) => row,
        Ok(None) => {
            return Err((
                axum::http::StatusCode::NOT_FOUND,
                Json(json!({ "error": "Driver not found" })),
            ))
        }
        Err(e) => {
            tracing::error!("data_export DB error for driver {}: {}", driver_id, e);
            return Err((
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "Database error" })),
            ));
        }
    };

    // Decrypt PII fields; fallback to plaintext columns if decryption fails or enc is NULL
    let name = d.4.as_deref()
        .and_then(|enc| state.field_cipher.decrypt_field(enc).ok())
        .or_else(|| Some(d.1.clone()));
    let email = d.5.as_deref()
        .and_then(|enc| state.field_cipher.decrypt_field(enc).ok())
        .or(d.2.clone());
    let phone = d.6.as_deref()
        .and_then(|enc| state.field_cipher.decrypt_field(enc).ok())
        .or(d.3.clone());

    // Fetch nickname
    let nickname: Option<String> = sqlx::query_as::<_, (Option<String>,)>(
        "SELECT nickname FROM drivers WHERE id = ?",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .and_then(|r| r.0);

    // Fetch wallet balance
    let wallet_balance: i64 = sqlx::query_as::<_, (i64,)>(
        "SELECT COALESCE(balance, 0) FROM wallets WHERE driver_id = ?",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .map(|r| r.0)
    .unwrap_or(0);

    let exported_at = chrono::Utc::now().to_rfc3339();
    tracing::info!("Data export requested by driver {}", driver_id);

    Ok((
        axum::http::StatusCode::OK,
        Json(json!({
            "driver_id": d.0,
            "name": name,
            "email": email,
            "phone": phone,
            "nickname": nickname,
            "total_laps": d.7,
            "total_time_ms": d.8,
            "wallet_balance": wallet_balance,
            "exported_at": exported_at,
        })),
    ))
}

/// DELETE /api/v1/customer/data-delete
/// Cascades deletion to all child tables and the driver record in a single transaction.
/// Requires valid customer JWT.
pub(crate) async fn customer_data_delete(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Result<axum::http::StatusCode, (axum::http::StatusCode, Json<Value>)> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => {
            return Err((
                axum::http::StatusCode::UNAUTHORIZED,
                Json(json!({ "error": e })),
            ))
        }
    };

    // Verify driver exists
    let exists = sqlx::query_as::<_, (String,)>(
        "SELECT id FROM drivers WHERE id = ?",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await;

    match exists {
        Ok(None) => {
            return Err((
                axum::http::StatusCode::NOT_FOUND,
                Json(json!({ "error": "Driver not found" })),
            ))
        }
        Err(e) => {
            tracing::error!("data_delete lookup error for driver {}: {}", driver_id, e);
            return Err((
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "Database error" })),
            ));
        }
        Ok(Some(_)) => {}
    }

    // Begin transaction — all deletes must atomically commit or roll back together.
    let mut tx = match state.db.begin().await {
        Ok(tx) => tx,
        Err(e) => {
            tracing::error!("data_delete transaction start error for driver {}: {}", driver_id, e);
            return Err((
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "Database error" })),
            ));
        }
    };

    let mut total_rows_erased: u64 = 0;
    let mut total_rows_nulled: u64 = 0;
    let mut total_rows_transitive: u64 = 0;
    let mut tables_skipped: Vec<&str> = Vec::new();

    // 1. NULL pointer columns (pods.current_driver_id etc.) BEFORE deleting the
    //    driver row, otherwise an FK constraint could block the drivers DELETE.
    for (table, column) in POINTER_TABLES {
        match null_pointer_column(&mut tx, table, column, &driver_id).await {
            Ok(n) => {
                total_rows_nulled = total_rows_nulled.saturating_add(n);
                if n > 0 {
                    tracing::info!(
                        "DPDP erase: nulled {}.{} for driver {} ({} rows)",
                        table, column, driver_id, n
                    );
                }
            }
            Err(ref e) if is_missing_table_error(e) => {
                tracing::warn!(
                    "DPDP erase: pointer table {} does not exist — skipping",
                    table
                );
                tables_skipped.push(table);
            }
            Err(e) => {
                tracing::error!(
                    "DPDP erase FAILED nulling {}.{} for driver {}: {} — rolling back",
                    table, column, driver_id, e
                );
                let _ = tx.rollback().await;
                return Err((
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "error": "Erase failed — transaction rolled back",
                        "table": table,
                        "phase": "null_pointer"
                    })),
                ));
            }
        }
    }

    // 2. TRANSITIVE erase — tables that FK to billing_sessions etc. with no
    //    direct driver_id column. Must run before the matching parent row is
    //    deleted in ERASE_TABLES.
    for (table, sql) in TRANSITIVE_ERASE_SQL {
        match sqlx::query(sql).bind(&driver_id).execute(&mut *tx).await {
            Ok(r) => {
                let n = r.rows_affected();
                total_rows_transitive = total_rows_transitive.saturating_add(n);
                if n > 0 {
                    tracing::info!(
                        "DPDP erase: deleted {} transitive rows from {} for driver {}",
                        n, table, driver_id
                    );
                }
            }
            Err(ref e) if is_missing_table_error(e) => {
                tracing::warn!(
                    "DPDP erase: transitive table {} does not exist — skipping",
                    table
                );
                tables_skipped.push(table);
            }
            Err(e) => {
                tracing::error!(
                    "DPDP erase FAILED at transitive table {} for driver {}: {} — rolling back",
                    table, driver_id, e
                );
                let _ = tx.rollback().await;
                return Err((
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "error": "Erase failed — transaction rolled back",
                        "table": table,
                        "phase": "transitive_erase"
                    })),
                ));
            }
        }
    }

    // 3. DELETE child rows across every declared FK table.
    for (table, columns) in ERASE_TABLES {
        match erase_table_rows(&mut tx, table, columns, &driver_id).await {
            Ok(n) => {
                total_rows_erased = total_rows_erased.saturating_add(n);
                if n > 0 {
                    tracing::info!(
                        "DPDP erase: deleted {} rows from {} for driver {}",
                        n, table, driver_id
                    );
                }
            }
            Err(ref e) if is_missing_table_error(e) => {
                tracing::warn!(
                    "DPDP erase: table {} does not exist on this DB — skipping",
                    table
                );
                tables_skipped.push(table);
            }
            Err(e) => {
                tracing::error!(
                    "DPDP erase FAILED at table {} for driver {}: {} — rolling back",
                    table, driver_id, e
                );
                let _ = tx.rollback().await;
                return Err((
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "error": "Erase failed — transaction rolled back",
                        "table": table,
                        "phase": "delete_children"
                    })),
                ));
            }
        }
    }

    // 4. Delete the driver record itself.
    match sqlx::query("DELETE FROM drivers WHERE id = ?")
        .bind(&driver_id)
        .execute(&mut *tx)
        .await
    {
        Ok(r) => {
            total_rows_erased = total_rows_erased.saturating_add(r.rows_affected());
        }
        Err(e) => {
            tracing::error!(
                "DPDP erase FAILED at drivers row for driver {}: {} — rolling back",
                driver_id, e
            );
            let _ = tx.rollback().await;
            return Err((
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "Erase failed — transaction rolled back",
                    "table": "drivers",
                    "phase": "delete_parent"
                })),
            ));
        }
    }

    // 5. Commit.
    if let Err(e) = tx.commit().await {
        tracing::error!("DPDP erase commit error for driver {}: {}", driver_id, e);
        return Err((
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": "Commit failed",
                "phase": "commit"
            })),
        ));
    }

    tracing::info!(
        "DPDP erase COMPLETE for driver {}: {} rows deleted across {} tables \
         (+1 drivers row), {} transitive rows deleted across {} tables, \
         {} pointer columns nulled across {} tables, {} tables skipped as absent",
        driver_id,
        total_rows_erased,
        ERASE_TABLES.len(),
        total_rows_transitive,
        TRANSITIVE_ERASE_SQL.len(),
        total_rows_nulled,
        POINTER_TABLES.len(),
        tables_skipped.len()
    );
    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ─── LEGAL-09: Consent revocation (customer-initiated via PWA) ───────────────
/// POST /api/v1/customer/revoke-consent
///
/// Allows a driver (or guardian acting on behalf of a minor) to invoke the DPDP Act
/// right of erasure. Anonymizes PII immediately. Financial records (journal entries,
/// invoices, billing_sessions) are NOT deleted — they must be retained for 8 years
/// per the Income Tax Act.
///
/// Body: `{ "reason": "optional reason string" }`
pub(crate) async fn revoke_consent_handler(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<Value>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };
    let reason = body
        .get("reason")
        .and_then(|v| v.as_str())
        .unwrap_or("customer_request");
    anonymize_driver_pii(&state, &driver_id, reason, None).await
}

// ─── LEGAL-09: Consent revocation (staff-initiated for guardian requests) ────
/// POST /api/v1/drivers/{id}/revoke-consent
///
/// Staff endpoint for guardian-initiated revocation — guardian calls the venue,
/// staff (cashier+) processes the data deletion request.
///
/// Body: `{ "reason": "optional reason string" }`
pub(crate) async fn staff_revoke_consent_handler(
    State(state): State<Arc<AppState>>,
    Path(driver_id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let reason = body
        .get("reason")
        .and_then(|v| v.as_str())
        .unwrap_or("guardian_request");
    anonymize_driver_pii(&state, &driver_id, reason, Some("staff")).await
}
