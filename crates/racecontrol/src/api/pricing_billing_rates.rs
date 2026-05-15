//! Billing rate tier CRUD handlers — extracted from pricing_routes.rs.

use super::auth_staff::venue_authority_guard;
use axum::{
    Json,
    extract::{Path, State},
    response::IntoResponse,
};
use serde_json::{Value, json};
use std::sync::Arc;

use crate::accounting;
use crate::state::AppState;

// ─── Pricing ceiling read-out (V2 row 7.3 Phase 1 surface) ──────────────────
//
// V2-PROGRESS-MAP §7 row 7.3 — MAX_DISCOUNT_PCT ceiling primitive.
// Substrate landed via §S-253 cascade (`crates/racecontrol/src/pricing/
// discount_ceiling.rs` + call-site wires at `api/billing_discount.rs:147` +
// `api/billing_start.rs:202` + §S-260 wallet_transactions audit columns +
// §S-272 Phase 2 observability daily clamp count). This endpoint exposes the
// effective cap to staff dashboards + the
// `tests/contract/pricing-discount-ceiling.spec.ts` discovery probe so the
// `CEILING_SURFACE_MISSING` SKIP gate can flip to runtime-check.
//
// **Read-only.** No writes; the cap is a doctrine-encoded constant pending a
// future config-override field on `AppState`. Captain Q-2-1 ratify anchor:
// comms-link §S-252 commit `c203135d` ("max_discount_pct = 0.50").
// RCA: `.planning/audits/RCA-2026-05-13-row-7.3-max-discount-pct-ceiling.md`.

#[cfg_attr(feature = "gen-types", utoipa::path(
    get,
    path = "/api/v1/pricing/ceiling",
    tag = "pricing",
    responses(
        (status = 200, description = "Effective MAX_DISCOUNT_PCT ceiling + cap_source provenance", body = serde_json::Value),
        (status = 401, description = "Staff JWT required"),
    ),
    security(("staffJWT" = []))
))]
pub(crate) async fn get_pricing_ceiling(State(state): State<Arc<AppState>>) -> Json<Value> {
    let cap = crate::pricing::discount_ceiling::max_discount_pct(&state);
    let is_default = (cap - crate::pricing::discount_ceiling::MAX_DISCOUNT_PCT_DEFAULT).abs() < f64::EPSILON;
    let cap_source = if is_default {
        "MAX_DISCOUNT_PCT_DEFAULT"
    } else {
        "AppState.config.discount_ceiling_pct"
    };
    // Unit clarity: the Rust constant `MAX_DISCOUNT_PCT_DEFAULT = 0.50` is
    // fraction-form (0.0..=1.0); applied-discount math in
    // `pricing/discount_ceiling::clamp_discount_pct` uses the same fraction
    // domain. Some downstream consumers (kiosk staff dashboard + the contract
    // test `tests/contract/pricing-discount-ceiling.spec.ts` 0-100 assertion)
    // expect percentage-form. Emitting BOTH avoids the unit-confusion class:
    //   * `max_discount_pct` — fraction (canonical; matches Rust const)
    //   * `max_discount_pct_percent` — percentage (UI/test-friendly)
    //   * `unit` — explicit declaration of which field carries which form
    Json(json!({
        "max_discount_pct": cap,
        "max_discount_pct_percent": cap * 100.0,
        "unit": "fraction",
        "cap_source": cap_source,
        "captain_ratify_anchor": "comms-link §S-252 commit c203135d",
        "rca_anchor": ".planning/audits/RCA-2026-05-13-row-7.3-max-discount-pct-ceiling.md",
        "doctrine_phase": "Post-V2.0-Pricing-Calibration",
        "schema_version": 1,
    }))
}

// ─── Billing Rate Tiers (per-minute rates) ──────────────────────────────────

#[cfg_attr(feature = "gen-types", utoipa::path(
    get,
    path = "/api/v1/billing/rates",
    tag = "billing",
    responses(
        (status = 200, description = "List of per-minute billing rate tiers", body = serde_json::Value),
        (status = 401, description = "Staff JWT required"),
    ),
    security(("staffJWT" = []))
))]
pub(crate) async fn list_billing_rates(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, i64, String, i64, i64, bool, Option<String>)>(
        "SELECT id, tier_order, tier_name, threshold_minutes, rate_per_min_paise, is_active, sim_type
         FROM billing_rates ORDER BY tier_order ASC",
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(rates) => {
            let list: Vec<Value> = rates
                .iter()
                .map(|r| {
                    json!({
                        "id": r.0, "tier_order": r.1, "tier_name": r.2,
                        "threshold_minutes": r.3, "rate_per_min_paise": r.4,
                        "is_active": r.5, "sim_type": r.6,
                    })
                })
                .collect();
            Json(json!({ "rates": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

pub(crate) async fn create_billing_rate(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    // Phase 349: Guard — cloud instance rejects writes to venue-authoritative tables
    if let Some(rejection) = venue_authority_guard(&state, "billing_rates") {
        return rejection.into_response();
    }
    let id = uuid::Uuid::new_v4().to_string();
    let tier_order = body.get("tier_order").and_then(|v| v.as_i64()).unwrap_or(1);
    let tier_name = body.get("tier_name").and_then(|v| v.as_str()).unwrap_or("Custom");
    let threshold_minutes = body.get("threshold_minutes").and_then(|v| v.as_i64()).unwrap_or(0);
    let rate_per_min_paise = body.get("rate_per_min_paise").and_then(|v| v.as_i64()).unwrap_or(2500);

    let sim_type = body.get("sim_type").and_then(|v| v.as_str()).map(|s| s.to_string());

    let result = sqlx::query(
        "INSERT INTO billing_rates (id, tier_order, tier_name, threshold_minutes, rate_per_min_paise, sim_type)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(tier_order)
    .bind(tier_name)
    .bind(threshold_minutes)
    .bind(rate_per_min_paise)
    .bind(&sim_type)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => {
            crate::billing::refresh_rate_tiers(&state).await;
            (axum::http::StatusCode::CREATED, Json(json!({ "id": id, "tier_name": tier_name }))).into_response()
        }
        Err(e) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

pub(crate) async fn update_billing_rate(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    // Phase 349: Guard — cloud instance rejects writes to venue-authoritative tables
    if let Some(rejection) = venue_authority_guard(&state, "billing_rates") {
        return rejection.into_response();
    }
    let old_snapshot = accounting::snapshot_row(&state, "billing_rates", &id).await;

    let tier_name = body.get("tier_name").and_then(|v| v.as_str());
    let tier_order = body.get("tier_order").and_then(|v| v.as_i64());
    let threshold_minutes = body.get("threshold_minutes").and_then(|v| v.as_i64());
    let rate_per_min_paise = body.get("rate_per_min_paise").and_then(|v| v.as_i64());
    let is_active = body.get("is_active").and_then(|v| v.as_bool());
    // sim_type: present in body = update (even if null to clear); absent = don't touch
    let sim_type_in_body = body.get("sim_type").map(|v| v.as_str().map(|s| s.to_string()));

    // SAFETY: Column names are hardcoded string literals below — not from user input.
    // All values use bind parameters (?). No SQL injection risk.
    let mut updates = Vec::new();
    let mut binds: Vec<String> = Vec::new();

    if let Some(n) = tier_name {
        updates.push("tier_name = ?");
        binds.push(n.to_string());
    }
    if let Some(o) = tier_order {
        updates.push("tier_order = ?");
        binds.push(o.to_string());
    }
    if let Some(t) = threshold_minutes {
        updates.push("threshold_minutes = ?");
        binds.push(t.to_string());
    }
    if let Some(r) = rate_per_min_paise {
        updates.push("rate_per_min_paise = ?");
        binds.push(r.to_string());
    }
    if let Some(a) = is_active {
        updates.push("is_active = ?");
        binds.push(if a { "1".to_string() } else { "0".to_string() });
    }
    let sim_type_val: Option<String> = if let Some(opt_s) = sim_type_in_body {
        updates.push("sim_type = ?");
        binds.push(opt_s.clone().unwrap_or_default());
        opt_s
    } else {
        None
    };
    let _ = sim_type_val; // used via binds above

    if updates.is_empty() {
        return Json(json!({ "error": "No fields to update" })).into_response();
    }

    updates.push("updated_at = datetime('now')");
    let query = format!("UPDATE billing_rates SET {} WHERE id = ?", updates.join(", "));

    let mut q = sqlx::query(&query);
    for b in &binds {
        q = q.bind(b);
    }
    q = q.bind(&id);

    match q.execute(&state.db).await {
        Ok(_) => {
            crate::billing::refresh_rate_tiers(&state).await;
            let new_values = serde_json::to_string(&body).ok();
            accounting::log_audit(
                &state, "billing_rates", &id, "update",
                old_snapshot.as_deref(), new_values.as_deref(), None,
            ).await;
            Json(json!({ "ok": true })).into_response()
        }
        Err(e) => Json(json!({ "error": e.to_string() })).into_response(),
    }
}

pub(crate) async fn delete_billing_rate(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    // Phase 349: Guard — cloud instance rejects writes to venue-authoritative tables
    if let Some(rejection) = venue_authority_guard(&state, "billing_rates") {
        return rejection.into_response();
    }
    let old_snapshot = accounting::snapshot_row(&state, "billing_rates", &id).await;

    match sqlx::query(
        "UPDATE billing_rates SET is_active = 0, updated_at = datetime('now') WHERE id = ?",
    )
    .bind(&id)
    .execute(&state.db)
    .await
    {
        Ok(_) => {
            crate::billing::refresh_rate_tiers(&state).await;
            accounting::log_audit(
                &state,
                "billing_rates",
                &id,
                "delete",
                old_snapshot.as_deref(),
                Some("{\"is_active\":false}"),
                None,
            )
            .await;
            axum::http::StatusCode::NO_CONTENT.into_response()
        }
        Err(e) => {
            tracing::error!("delete_billing_rate DB error for {}: {}", id, e);
            axum::http::StatusCode::NO_CONTENT.into_response()
        }
    }
}

// ─── V2 row 7.3 Phase 1 tests ────────────────────────────────────────────────

#[cfg(test)]
mod row_7_3_tests {
    use super::*;
    use crate::pricing::discount_ceiling::MAX_DISCOUNT_PCT_DEFAULT;

    /// `get_pricing_ceiling` emits both fraction + percentage forms with
    /// matching values and the `unit` declaration. Closes the unit-confusion
    /// class surfaced during PR authoring (Rust const = fraction 0.50;
    /// contract test assertion expected percentage 0..100).
    #[tokio::test]
    async fn get_pricing_ceiling_emits_both_unit_forms() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite pool");
        let mut config = crate::config::Config::default_test();
        config.auth.jwt_secret = "test".to_string();
        let field_cipher = crate::crypto::encryption::test_field_cipher();
        let state = Arc::new(AppState::new_with_test_v2db(config, pool, field_cipher));

        let Json(body) = get_pricing_ceiling(State(state)).await;

        // Fraction-form: must equal the Rust doctrine const (Captain Q-2-1
        // ratify §S-252 c203135d). f64 equality is safe here because both
        // sides flow from the same const.
        let fraction = body["max_discount_pct"].as_f64().expect("max_discount_pct present");
        assert!(
            (fraction - MAX_DISCOUNT_PCT_DEFAULT).abs() < f64::EPSILON,
            "max_discount_pct fraction-form drifted from MAX_DISCOUNT_PCT_DEFAULT"
        );

        // Percentage-form: fraction × 100.
        let percent = body["max_discount_pct_percent"].as_f64().expect("max_discount_pct_percent present");
        assert!(
            (percent - fraction * 100.0).abs() < f64::EPSILON,
            "max_discount_pct_percent must equal fraction × 100"
        );

        // Unit declaration is explicit.
        assert_eq!(body["unit"].as_str(), Some("fraction"));

        // Captain ratify anchor + RCA anchor are present (closes provenance audit).
        assert_eq!(
            body["captain_ratify_anchor"].as_str(),
            Some("comms-link §S-252 commit c203135d")
        );
        assert!(body["rca_anchor"].as_str().is_some());

        // Cap source declares whether the value came from default-const or
        // a future config override. Test-state has no override → default.
        assert_eq!(body["cap_source"].as_str(), Some("MAX_DISCOUNT_PCT_DEFAULT"));
    }

    /// Contract test invariant: percentage-form falls within (0, 100] per
    /// the assertions in `tests/contract/pricing-discount-ceiling.spec.ts`
    /// Test 1. This regression pins the doctrine ratify (50%) within the
    /// safe band so a future hot-config change to 0.0 or 1.5 is caught.
    #[tokio::test]
    async fn get_pricing_ceiling_percent_in_safe_band() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite pool");
        let mut config = crate::config::Config::default_test();
        config.auth.jwt_secret = "test".to_string();
        let field_cipher = crate::crypto::encryption::test_field_cipher();
        let state = Arc::new(AppState::new_with_test_v2db(config, pool, field_cipher));

        let Json(body) = get_pricing_ceiling(State(state)).await;
        let percent = body["max_discount_pct_percent"].as_f64().expect("present");

        assert!(percent > 0.0, "ceiling must be strictly positive (>0 invariant)");
        assert!(percent <= 100.0, "ceiling must not exceed 100% (refund-class boundary)");
    }
}
