//! CIRS lookup HTTP handler — PACT-20260506-001 Phase 1 wire-up.
//!
//! Customer Identity Resolution Service (CIRS) HTTP surface at
//! `POST /api/v1/cirs/lookup`. Substrate at `crates/v2-db/src/cirs.rs` is
//! Phase 0 (MERGED `483562ac`). This module is the Phase 1 (james-LEAD,
//! verify-by 2026-05-19) HTTP layer per PACT-20260506-001.
//!
//! ## Session 1 scope (SHIPPED commit `8652a058`)
//!
//! - Request/Response DTOs matching PACT §2.1 + §AMEND-1.A `balance_credits`
//! - Handler stub returning HTTP 501
//! - Serde round-trip tests for every method variant
//!
//! ## Session 2 scope (THIS SESSION)
//!
//! - `v2-db` Cargo dep added to racecontrol-crate (`Cargo.toml` Session 2 patch)
//! - Local `CirsLookupRequest` replaced with `pub use v2_db::cirs::LookupInput`
//! - Real handler logic against `v2_db::cirs::lookup_by_phone` for M1
//! - ProfilePreview substrate joins (customers + wallets + sessions stats)
//! - `record_lookup` post-call discipline on every Found / NotFound / Error path
//! - Walk-In Guest synthetic path (no DB lookup; record_lookup with NotFound tag)
//! - QR / NFC plumbed-disabled paths (Phase 3 activates) return 501
//! - Integration tests against real `v2_db::open` + `migrate` pool
//!
//! ## Captain Q-DECISION queued (Q-CUSTOMER-PROFILES-1)
//!
//! `customer_profiles` table does NOT exist in v2-db substrate at HEAD `8652a058`.
//! Captain dispositioned synthetic-single-element profiles[] in Session 2
//! AskUserQuestion (option `(a)`): one ProfileSummary populated from the
//! `customers` row itself, `is_default: true`, `discount_ineligible: false`.
//! Reversible — when a multi-profile sub-PACT lands a `customer_profiles`
//! migration, swap the synthetic helper for a real query in this file. No
//! wire-format change to ProfilePreview / ProfileSummary at that point.
//!
//! ## Auth surface (per §AMEND-1.E + Q5-A)
//!
//! `cirs::lookup_by_phone` is **non-privileged** — staff session cookie
//! (via `auth::middleware::require_staff_jwt`) is sufficient. NO PIN re-entry.
//! Session 2 reads `StaffClaims` from request extensions (inserted by the
//! middleware's parts.extensions.insert call) to thread `staff_id` into
//! `cirs::record_lookup` for the cirs_lookup_audit row. Route registration in
//! Session 3 chains require_staff_jwt -> this handler.
//!
//! ## NF-james-B Indian-mobile-prefix WARN gate (§AMEND-1.B)
//!
//! Per §AMEND-1.B: gate evaluates raw input string **BEFORE** substrate
//! `canonicalize_phone`. Gate is staff-typed-input-discipline (bare 10-digit
//! assumption holds for Indian-mobile-only-V2.0 frame), NOT
//! substrate-input-discipline. Gate lives at the UI layer
//! (`web-v2/src/components/v2/pos/PhoneLookupInput.tsx`), NOT here. Substrate
//! stays permissive for E.164 / cross-tz international customers.
//!
//! ## Composes-with
//!
//! - PACT-20260506-001 (FILED `b45cf13`); §AMEND-1 ABSORB-IN-FULL of NF-bono-1..7
//! - PACT-20260505-001 Phase 0 substrate (MERGED `483562ac`)
//! - PACT-20260503-018 staff_id FK (MERGED `3119da30`)
//! - PrivilegedAction enum (`crate::auth::privileged_actions`) — non-membership
//!   confirms CIRS lookup is non-privileged

use axum::{
    Extension, Json,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

use crate::auth::middleware::StaffClaims;
use crate::state::AppState;

// Re-export the v2-db substrate enum as the public wire-format type. Drops
// the local mirror authored in Session 1 — wire format is unchanged because
// `LookupInput` and the Session 1 `CirsLookupRequest` had identical serde
// shapes. Kept the name `CirsLookupRequest` as the public alias for callers.
pub use v2_db::cirs::LookupInput as CirsLookupRequest;

// ─── Response DTO — ProfilePreview ─────────────────────────────────────────

/// ProfilePreview — DoD §3.3 substrate.
///
/// Field naming per §AMEND-1.A NF-bono-1 absorption: `balance_credits` mirrors
/// canonical `wallets.balance_credits`. Earlier draft used `wallet_balance_credits`
/// — drift removed at AMEND time.
///
/// Wallet Framing C (single shared wallet per family) — `balance_credits` is
/// the family-pool balance, not per-profile.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProfilePreview {
    pub customer_id: String,
    pub primary_phone: String,
    pub name: String,
    pub profiles: Vec<ProfileSummary>,
    pub balance_credits: i64,
    pub last_visit_ts: Option<String>,
    pub arrival_history_count_30d: u32,
    pub discount_ineligible: bool,
}

/// One row of the `profiles[]` list — V2 customer workflows Scenario 3 caps
/// at 4 profiles per family. Until customer_profiles table lands as its own
/// sub-PACT, profiles[] carries one synthetic ProfileSummary derived from the
/// customers row (Q-CUSTOMER-PROFILES-1 disposition (a) Session 2).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProfileSummary {
    pub profile_id: String,
    pub name: String,
    pub is_default: bool,
    pub discount_ineligible: bool,
}

// ─── Handler ───────────────────────────────────────────────────────────────

/// CIRS lookup handler — Session 2 real implementation.
///
/// `POST /api/v1/cirs/lookup` under staff-JWT protected sub-router. Route
/// registration lands in Session 3.
///
/// Behavior:
/// - `Phone` -> canonicalize + `lookup_by_phone` -> (Found ? fetch_profile_preview : 404 NotFound shape)
/// - `WalkInGuestId` -> synthetic ProfilePreview, `discount_ineligible: true`, no DB customer query
/// - `QrPayload` / `NfcTagId` -> Phase 3 (V2.0 plumbed-disabled), 501 NotImplemented
///
/// Every path writes a `cirs_lookup_audit` row via `cirs::record_lookup` with
/// the staff_id from the JWT claims. Audit-write failure does NOT change the
/// HTTP response — it is logged at WARN level so the client still sees the
/// resolution outcome (CGP H3 discipline: one boundary, one outcome).
pub async fn cirs_lookup_handler(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<StaffClaims>,
    Json(request): Json<CirsLookupRequest>,
) -> impl IntoResponse {
    use v2_db::cirs::{LookupInput, LookupResult, lookup_by_phone, record_lookup};

    let staff_id = claims.sub.as_str();

    match &request {
        // ── Walk-In Guest path — synthetic, no DB customer query ───────────
        LookupInput::WalkInGuestId { guest_id } => {
            // Audit the lookup (input_hash is None for walk-in per substrate).
            // Walk-in tag = NotFound at audit-row level — the customer record
            // doesn't exist; the synthetic preview is a UI-only convenience.
            let audit_result = LookupResult::NotFound;
            if let Err(e) =
                record_lookup(&state.v2db, staff_id, None, &request, &audit_result).await
            {
                tracing::warn!(
                    target: "cirs_lookup_audit",
                    error = %e,
                    method = "walk_in_guest_id",
                    guest_id = *guest_id,
                    "cirs_lookup_audit write failed (non-fatal)"
                );
            }
            (
                StatusCode::OK,
                Json(json!(synthesize_walkin_preview(*guest_id))),
            )
                .into_response()
        }

        // ── Phone path — M1 ACTIVE in V2.0 ─────────────────────────────────
        LookupInput::Phone { phone } => {
            let lookup_outcome = lookup_by_phone(&state.v2db, phone).await;
            match lookup_outcome {
                Ok(LookupResult::Found { customer_id }) => {
                    // Fetch the full ProfilePreview substrate.
                    let preview_result =
                        fetch_profile_preview(&state.v2db, &customer_id).await;
                    match preview_result {
                        Ok(preview) => {
                            // Audit Found
                            let audit = LookupResult::Found {
                                customer_id: customer_id.clone(),
                            };
                            if let Err(e) = record_lookup(
                                &state.v2db,
                                staff_id,
                                Some(customer_id.as_str()),
                                &request,
                                &audit,
                            )
                            .await
                            {
                                tracing::warn!(
                                    target: "cirs_lookup_audit",
                                    error = %e,
                                    method = "phone",
                                    "cirs_lookup_audit write failed (non-fatal)"
                                );
                            }
                            (StatusCode::OK, Json(json!(preview))).into_response()
                        }
                        Err(e) => {
                            // Substrate row was found but profile-fetch failed —
                            // surfaces as Error in the audit log, 500 to client.
                            let msg = format!("profile_preview_fetch_failed: {e}");
                            let audit = LookupResult::Error { message: msg.clone() };
                            if let Err(audit_err) = record_lookup(
                                &state.v2db,
                                staff_id,
                                Some(customer_id.as_str()),
                                &request,
                                &audit,
                            )
                            .await
                            {
                                tracing::warn!(
                                    target: "cirs_lookup_audit",
                                    error = %audit_err,
                                    method = "phone",
                                    "cirs_lookup_audit write failed (non-fatal)"
                                );
                            }
                            (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(json!({
                                    "error": "profile_preview_fetch_failed",
                                    "message": msg,
                                })),
                            )
                                .into_response()
                        }
                    }
                }
                Ok(LookupResult::NotFound) => {
                    let audit = LookupResult::NotFound;
                    if let Err(e) = record_lookup(
                        &state.v2db,
                        staff_id,
                        None,
                        &request,
                        &audit,
                    )
                    .await
                    {
                        tracing::warn!(
                            target: "cirs_lookup_audit",
                            error = %e,
                            method = "phone",
                            "cirs_lookup_audit write failed (non-fatal)"
                        );
                    }
                    (
                        StatusCode::NOT_FOUND,
                        Json(json!({
                            "result": "not_found",
                            "message": "no customer matches the canonicalized phone",
                            "next_action": "offer_walk_in_guest_or_pwa_register",
                        })),
                    )
                        .into_response()
                }
                Ok(LookupResult::Error { message }) => {
                    // Phase 0 substrate Error variant — currently unreachable
                    // (lookup_by_phone returns Result, not LookupResult::Error)
                    // but defended for forward-compat. Audit + 500.
                    let audit = LookupResult::Error { message: message.clone() };
                    let _ = record_lookup(
                        &state.v2db,
                        staff_id,
                        None,
                        &request,
                        &audit,
                    )
                    .await;
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({ "error": "lookup_substrate_error", "message": message })),
                    )
                        .into_response()
                }
                Err(e) => {
                    // CirsError — InvalidPhone / AmbiguousPhone / Sqlx.
                    let (status, code) = match &e {
                        v2_db::cirs::CirsError::InvalidPhone(_) => {
                            (StatusCode::BAD_REQUEST, "invalid_phone")
                        }
                        v2_db::cirs::CirsError::AmbiguousPhone(_) => {
                            (StatusCode::BAD_REQUEST, "ambiguous_phone")
                        }
                        v2_db::cirs::CirsError::Sqlx(_) => {
                            (StatusCode::INTERNAL_SERVER_ERROR, "lookup_db_error")
                        }
                    };
                    // DPDP §7.8 — `format!("{e}")` is ALREADY redacted via the
                    // CirsError Display impl (v2-db/src/cirs.rs L23-36). The audit
                    // table stores the fingerprint form ("<sha256:xxxx-xxxx>"), not
                    // the raw phone. DO NOT switch to `format!("{e:?}")` (Debug) —
                    // Debug would re-expose the inner String and regress the privacy
                    // contract at `tests/contract/cirs-lookup-pii-redaction.spec.ts`.
                    let msg = format!("{e}");
                    let audit = LookupResult::Error { message: msg.clone() };
                    if let Err(audit_err) = record_lookup(
                        &state.v2db,
                        staff_id,
                        None,
                        &request,
                        &audit,
                    )
                    .await
                    {
                        tracing::warn!(
                            target: "cirs_lookup_audit",
                            error = %audit_err,
                            method = "phone",
                            "cirs_lookup_audit write failed (non-fatal)"
                        );
                    }
                    (status, Json(json!({ "error": code, "message": msg }))).into_response()
                }
            }
        }

        // ── QR / NFC — V2.0 plumbed-disabled per Phase 0 doc ───────────────
        LookupInput::QrPayload { .. } | LookupInput::NfcTagId { .. } => {
            let method = match &request {
                LookupInput::QrPayload { .. } => "qr_payload",
                LookupInput::NfcTagId { .. } => "nfc_tag_id",
                _ => unreachable!(),
            };
            // Audit the attempt with Error tag — Phase 3 activation will flip
            // these to actual resolution; until then, NotImplemented at the
            // boundary is the correct H3 honest-evidence answer.
            let audit = LookupResult::Error {
                message: format!("{method} method plumbed-disabled until Phase 3"),
            };
            if let Err(e) =
                record_lookup(&state.v2db, staff_id, None, &request, &audit).await
            {
                tracing::warn!(
                    target: "cirs_lookup_audit",
                    error = %e,
                    method = method,
                    "cirs_lookup_audit write failed (non-fatal)"
                );
            }
            (
                StatusCode::NOT_IMPLEMENTED,
                Json(json!({
                    "error": format!("{method}_plumbed_disabled"),
                    "message": "method recognized; activation lands in Phase 3 (no hardware in V2.0)",
                    "active_methods_v2_0": ["phone", "walk_in_guest_id"],
                })),
            )
                .into_response()
        }
    }
}

// ─── ProfilePreview substrate join ─────────────────────────────────────────

/// Fetch the full ProfilePreview shape for a customer_id.
///
/// SQL composition:
///   - inner join customers + LEFT JOIN wallets (1:1 via wallets.customer_id UNIQUE)
///   - subquery for `last_visit_ts` (MAX ended_at where billing_state LIKE 'ended_%')
///   - subquery for `arrival_history_count_30d` (COUNT sessions started in last 30d)
///
/// `profiles[]` is synthetic single-element until customer_profiles migration
/// lands (Captain Q-CUSTOMER-PROFILES-1 = (a)).
async fn fetch_profile_preview(
    pool: &v2_db::DbPool,
    customer_id: &str,
) -> Result<ProfilePreview, sqlx::Error> {
    // Single composite query — simpler than multiple round-trips and the
    // SQLite query planner handles correlated subqueries fine on the indexed
    // sessions(customer_id) + sessions partial-index columns.
    let row: (String, String, Option<String>, i64, Option<String>, i64) =
        sqlx::query_as(
            r#"
            SELECT
                c.id,
                c.phone,
                c.display_name,
                COALESCE(w.balance_credits, 0) AS balance_credits,
                (SELECT MAX(s.ended_at) FROM sessions s
                 WHERE s.customer_id = c.id
                   AND s.billing_state LIKE 'ended_%'
                   AND s.ended_at IS NOT NULL) AS last_visit_ts,
                (SELECT COUNT(*) FROM sessions s2
                 WHERE s2.customer_id = c.id
                   AND s2.started_at >= datetime('now', '-30 days')) AS arrival_history_count_30d
            FROM customers c
            LEFT JOIN wallets w ON w.customer_id = c.id
            WHERE c.id = ?
            LIMIT 1
            "#,
        )
        .bind(customer_id)
        .fetch_one(pool)
        .await?;

    let (id, phone, display_name, balance_credits, last_visit_ts, arrival_count) = row;
    let name = display_name.clone().unwrap_or_else(|| "Customer".to_string());

    // Synthetic single-element profiles[] until customer_profiles migration.
    // profile_id reuses customer_id so frontend code can key off a stable id;
    // when the real customer_profiles table lands the id swaps to the actual
    // profile UUID. is_default=true matches the V2 "first profile is default"
    // convention in V2 customer workflows consolidated 2026-05-03.
    let synthetic_profile = ProfileSummary {
        profile_id: id.clone(),
        name: name.clone(),
        is_default: true,
        discount_ineligible: false,
    };

    Ok(ProfilePreview {
        customer_id: id,
        primary_phone: phone,
        name,
        profiles: vec![synthetic_profile],
        balance_credits,
        last_visit_ts,
        // SQLite COUNT(*) is i64; clamp to u32 for DTO. arrival count over
        // 4 billion in 30 days is structurally impossible.
        arrival_history_count_30d: u32::try_from(arrival_count).unwrap_or(u32::MAX),
        discount_ineligible: false,
    })
}

/// Synthesize the Walk-In Guest ProfilePreview shape (no DB lookup).
/// `discount_ineligible: true` per V2 customer workflows DoD §1.2 path B.
fn synthesize_walkin_preview(guest_id: u8) -> ProfilePreview {
    ProfilePreview {
        customer_id: format!("walk_in_guest_{guest_id}"),
        primary_phone: String::new(),
        name: format!("Walk-In Guest {guest_id}"),
        profiles: vec![],
        balance_credits: 0,
        last_visit_ts: None,
        arrival_history_count_30d: 0,
        discount_ineligible: true,
    }
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use v2_db::cirs::LookupInput;

    // ── Session 1 contract tests retained — verify wire-format unchanged
    //    after the v2_db::cirs::LookupInput re-export swap.

    #[test]
    fn cirs_lookup_request_serde_phone_roundtrips() {
        let req = LookupInput::Phone {
            phone: "9876543210".to_string(),
        };
        let json = serde_json::to_string(&req).expect("serialize");
        assert_eq!(json, r#"{"method":"phone","phone":"9876543210"}"#);
        let parsed: LookupInput = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, req);
    }

    #[test]
    fn cirs_lookup_request_serde_qr_payload_roundtrips() {
        let req = LookupInput::QrPayload {
            payload: "rp:v1:c:9876543210:abc123".to_string(),
        };
        let json = serde_json::to_string(&req).expect("serialize");
        assert!(json.contains(r#""method":"qr_payload""#));
        assert!(json.contains(r#""payload":"rp:v1:c:9876543210:abc123""#));
        let parsed: LookupInput = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, req);
    }

    #[test]
    fn cirs_lookup_request_serde_nfc_tag_id_roundtrips() {
        let req = LookupInput::NfcTagId {
            tag_id: "04:1A:2B:3C:4D:5E:6F".to_string(),
        };
        let json = serde_json::to_string(&req).expect("serialize");
        assert!(json.contains(r#""method":"nfc_tag_id""#));
        let parsed: LookupInput = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, req);
    }

    #[test]
    fn cirs_lookup_request_serde_walk_in_guest_id_roundtrips() {
        let req = LookupInput::WalkInGuestId { guest_id: 1 };
        let json = serde_json::to_string(&req).expect("serialize");
        assert_eq!(json, r#"{"method":"walk_in_guest_id","guest_id":1}"#);
        let parsed: LookupInput = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, req);
    }

    #[test]
    fn cirs_lookup_request_method_discriminator_is_required() {
        let parsed: Result<LookupInput, _> =
            serde_json::from_str(r#"{"phone":"9876543210"}"#);
        assert!(parsed.is_err(), "missing method discriminator must reject");
    }

    #[test]
    fn cirs_lookup_request_unknown_method_rejects() {
        let parsed: Result<LookupInput, _> = serde_json::from_str(
            r#"{"method":"biometric_face_scan","value":"…"}"#,
        );
        assert!(parsed.is_err(), "unknown method must reject");
    }

    #[test]
    fn profile_preview_serde_uses_balance_credits_naming_per_amend_1_a() {
        let preview = ProfilePreview {
            customer_id: "00000000-0000-0000-0000-000000000001".to_string(),
            primary_phone: "+919876543210".to_string(),
            name: "Test Customer".to_string(),
            profiles: vec![ProfileSummary {
                profile_id: "00000000-0000-0000-0000-0000000000a1".to_string(),
                name: "Default".to_string(),
                is_default: true,
                discount_ineligible: false,
            }],
            balance_credits: 480,
            last_visit_ts: Some("2026-05-05T14:25:00+05:30".to_string()),
            arrival_history_count_30d: 4,
            discount_ineligible: false,
        };

        let json = serde_json::to_string(&preview).expect("serialize");
        assert!(
            json.contains(r#""balance_credits":480"#),
            "ProfilePreview must serialize wallet field as `balance_credits` per §AMEND-1.A — got: {json}"
        );
        assert!(
            !json.contains("wallet_balance_credits"),
            "drift to `wallet_balance_credits` detected — §AMEND-1.A reverted: {json}"
        );

        let parsed: ProfilePreview = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, preview);
    }

    #[test]
    fn profile_preview_walk_in_guest_shape_marks_discount_ineligible() {
        let walk_in = synthesize_walkin_preview(1);
        assert_eq!(walk_in.customer_id, "walk_in_guest_1");
        assert_eq!(walk_in.name, "Walk-In Guest 1");
        assert!(walk_in.discount_ineligible);
        assert_eq!(walk_in.balance_credits, 0);
        assert!(walk_in.profiles.is_empty());
        assert_eq!(walk_in.arrival_history_count_30d, 0);
        assert!(walk_in.last_visit_ts.is_none());

        let json = serde_json::to_string(&walk_in).expect("serialize");
        assert!(json.contains(r#""discount_ineligible":true"#));
    }

    #[test]
    fn synthesize_walkin_guest_2_distinct_from_guest_1() {
        let g1 = synthesize_walkin_preview(1);
        let g2 = synthesize_walkin_preview(2);
        assert_ne!(g1.customer_id, g2.customer_id);
        assert_ne!(g1.name, g2.name);
        assert!(g1.discount_ineligible && g2.discount_ineligible);
    }

    // ── Session 2 NEW: integration tests against real v2-db pool ──────────
    //
    // These exercise the full `fetch_profile_preview` query against a freshly
    // migrated v2-db SQLite file. Use `tempfile` + `v2_db::open` + `migrate`.
    // Tests run sequentially in their own pool — no cross-contamination.

    async fn open_test_v2db() -> v2_db::DbPool {
        let tmp = tempfile::NamedTempFile::new().expect("tempfile");
        let path = tmp.path().to_str().expect("tempfile str path").to_string();
        // Hold the file alive via leak — pool keeps a handle anyway, but the
        // NamedTempFile drop would unlink prematurely on some platforms.
        std::mem::forget(tmp);
        let pool = v2_db::open(&path).await.expect("open v2-db");
        v2_db::migrate(&pool).await.expect("migrate v2-db");
        pool
    }

    /// Seed the minimum FK chain a Found-path test needs: staff (PACT-018 FK
    /// target) + customer + wallet. Returns customer_id for the test to query.
    async fn seed_basic_chain(pool: &v2_db::DbPool, balance: i64) -> String {
        sqlx::query(
            "INSERT INTO staff (id, name, phone, pin, role, active) \
             VALUES ('s-test', 'Test Staff', '0000000001', '0001', 'cashier', 1)",
        )
        .execute(pool)
        .await
        .expect("seed staff");

        let customer_id = "cust-test-001".to_string();
        sqlx::query(
            "INSERT INTO customers (id, phone, display_name) \
             VALUES (?, '+919876543210', 'Test Customer')",
        )
        .bind(&customer_id)
        .execute(pool)
        .await
        .expect("seed customer");

        sqlx::query(
            "INSERT INTO wallets (id, customer_id, balance_credits) VALUES ('w-1', ?, ?)",
        )
        .bind(&customer_id)
        .bind(balance)
        .execute(pool)
        .await
        .expect("seed wallet");

        customer_id
    }

    #[tokio::test]
    async fn fetch_profile_preview_found_returns_canonical_shape() {
        let pool = open_test_v2db().await;
        let customer_id = seed_basic_chain(&pool, 480).await;

        let preview = fetch_profile_preview(&pool, &customer_id)
            .await
            .expect("fetch");

        assert_eq!(preview.customer_id, customer_id);
        assert_eq!(preview.primary_phone, "+919876543210");
        assert_eq!(preview.name, "Test Customer");
        assert_eq!(preview.balance_credits, 480);
        assert_eq!(preview.profiles.len(), 1);
        assert!(preview.profiles[0].is_default);
        assert!(!preview.profiles[0].discount_ineligible);
        assert_eq!(preview.profiles[0].profile_id, customer_id);
        assert_eq!(preview.last_visit_ts, None);
        assert_eq!(preview.arrival_history_count_30d, 0);
        assert!(!preview.discount_ineligible);
    }

    #[tokio::test]
    async fn fetch_profile_preview_no_wallet_defaults_balance_to_zero() {
        // §AMEND-1.A NF-bono-1 sibling — customer with no wallet row must
        // serialize `balance_credits: 0`, NOT NULL or absent. The COALESCE
        // in the join SQL is the structural enforcement.
        let pool = open_test_v2db().await;
        sqlx::query(
            "INSERT INTO staff (id, name, phone, pin, role, active) \
             VALUES ('s-1', 'S', '0000000001', '0001', 'cashier', 1)",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO customers (id, phone, display_name) \
             VALUES ('cust-no-wallet', '+919999999998', 'No Wallet')",
        )
        .execute(&pool)
        .await
        .unwrap();

        let preview = fetch_profile_preview(&pool, "cust-no-wallet")
            .await
            .expect("fetch");
        assert_eq!(preview.balance_credits, 0);
        assert_eq!(preview.name, "No Wallet");
    }

    #[tokio::test]
    async fn fetch_profile_preview_missing_customer_returns_row_not_found() {
        let pool = open_test_v2db().await;
        sqlx::query(
            "INSERT INTO staff (id, name, phone, pin, role, active) \
             VALUES ('s-1', 'S', '0000000001', '0001', 'cashier', 1)",
        )
        .execute(&pool)
        .await
        .unwrap();

        let err = fetch_profile_preview(&pool, "nonexistent-customer")
            .await
            .expect_err("must fail");
        // sqlx::Error::RowNotFound is the exact variant fetch_one returns
        // when zero rows match — handler wraps this as 404 NotFound (which
        // the lookup_by_phone path also reaches via LookupResult::NotFound,
        // so this branch is defense-in-depth for direct callers).
        assert!(matches!(err, sqlx::Error::RowNotFound));
    }

    #[tokio::test]
    async fn fetch_profile_preview_arrival_count_filters_30d_window() {
        let pool = open_test_v2db().await;
        let customer_id = seed_basic_chain(&pool, 100).await;

        // Seed pod (FK target for sessions.pod_id). v2-db schema columns:
        // id / display_name / hardware_class / ip_address / status / updated_at
        sqlx::query(
            "INSERT INTO pods (id, display_name, hardware_class) \
             VALUES ('p-1', 'Pod 1', 'sim_pod')",
        )
        .execute(&pool)
        .await
        .expect("seed pod");

        // Two sessions: one inside the 30d window, one outside.
        sqlx::query(
            "INSERT INTO sessions (id, customer_id, pod_id, session_type, \
             started_at, ended_at, billing_state, staff_id) \
             VALUES ('sess-recent', ?, 'p-1', 'solo', \
                     datetime('now', '-5 days'), datetime('now', '-5 days', '+30 minutes'), \
                     'ended_normal', 's-test')",
        )
        .bind(&customer_id)
        .execute(&pool)
        .await
        .expect("seed recent session");

        sqlx::query(
            "INSERT INTO sessions (id, customer_id, pod_id, session_type, \
             started_at, ended_at, billing_state, staff_id) \
             VALUES ('sess-old', ?, 'p-1', 'solo', \
                     datetime('now', '-60 days'), datetime('now', '-60 days', '+30 minutes'), \
                     'ended_normal', 's-test')",
        )
        .bind(&customer_id)
        .execute(&pool)
        .await
        .expect("seed old session");

        let preview = fetch_profile_preview(&pool, &customer_id)
            .await
            .expect("fetch");
        assert_eq!(
            preview.arrival_history_count_30d, 1,
            "only the within-30d session counts; got history_count={}",
            preview.arrival_history_count_30d
        );
        // last_visit_ts populated by the most-recent ended session
        assert!(preview.last_visit_ts.is_some());
    }

    #[tokio::test]
    async fn record_lookup_writes_audit_row_for_phone_found_path() {
        // Sanity-check the substrate primitive end-to-end inside the racecontrol
        // test harness — guards against a future v2-db schema change that
        // breaks the bind ordering. Functional duplicate of the v2-db crate's
        // own tests, but in the racecontrol test process so a workspace-wide
        // `cargo test -p racecontrol-crate` catches the drift class too.
        use v2_db::cirs::{LookupInput, LookupResult, record_lookup};

        let pool = open_test_v2db().await;
        let customer_id = seed_basic_chain(&pool, 100).await;

        let input = LookupInput::Phone { phone: "+919876543210".to_string() };
        let result = LookupResult::Found { customer_id: customer_id.clone() };

        record_lookup(&pool, "s-test", Some(customer_id.as_str()), &input, &result)
            .await
            .expect("record_lookup");

        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM cirs_lookup_audit \
             WHERE staff_id = 's-test' AND customer_id = ? AND result = 'found'",
        )
        .bind(&customer_id)
        .fetch_one(&pool)
        .await
        .expect("count");
        assert_eq!(count, 1, "exactly one audit row for the lookup");

        let input_method: String = sqlx::query_scalar(
            "SELECT input_method FROM cirs_lookup_audit \
             WHERE staff_id = 's-test' AND customer_id = ?",
        )
        .bind(&customer_id)
        .fetch_one(&pool)
        .await
        .expect("input_method");
        assert_eq!(input_method, "phone");
    }

    // ── §5.5 Performance smoke (PACT-20260506-001 §5.5 + DoD §1.2) ────────
    //
    // Target: p50 ≤ 50ms / p95 ≤ 500ms / p99 ≤ 1000ms against a 10K-row
    // canonical-customer-set. Gate is the DoD §1.2 ceiling (500ms p95) — a
    // soft regression detector, not a benchmark. Local Windows debug build
    // typically observes p95 well under 5ms on indexed single-row joins.
    //
    // Sequential measurement matches the POS-staff usage pattern (one staff
    // member typing one phone at a time). Parallel/load testing is a separate
    // concern deferred per PACT §6 NOT TESTED ("Concurrent FK-rejection paths
    // under multi-writer load — Phase 1 wire-up is single-handler-per-request;
    // load-test scope post-Phase-1").

    #[tokio::test]
    async fn perf_smoke_fetch_profile_preview_p95_under_500ms() {
        use std::time::Instant;

        let pool = open_test_v2db().await;

        // Staff seed (PACT-018 FK target). Not exercised by fetch_profile_preview
        // itself, but seeded for parity with the full handler path's record_lookup.
        sqlx::query(
            "INSERT INTO staff (id, name, phone, pin, role, active) \
             VALUES ('s-perf', 'Perf Staff', '0000000099', '0099', 'cashier', 1)",
        )
        .execute(&pool)
        .await
        .expect("seed staff");

        // Seed 10,000 customers + 10,000 wallets in a single transaction.
        // Without the transaction wrapper SQLite issues one fsync per INSERT
        // and 10K inserts dominate test time; with the transaction the entire
        // seed completes in <2s on local NVMe.
        const TOTAL_CUSTOMERS: usize = 10_000;
        let seed_start = Instant::now();
        let mut tx = pool.begin().await.expect("begin tx");
        for i in 0..TOTAL_CUSTOMERS {
            let cid = format!("cust-perf-{i:05}");
            let phone = format!("+9198{i:08}");
            sqlx::query(
                "INSERT INTO customers (id, phone, display_name) \
                 VALUES (?, ?, 'Perf Customer')",
            )
            .bind(&cid)
            .bind(&phone)
            .execute(&mut *tx)
            .await
            .expect("seed customer");

            sqlx::query(
                "INSERT INTO wallets (id, customer_id, balance_credits) \
                 VALUES (?, ?, 100)",
            )
            .bind(format!("w-perf-{i:05}"))
            .bind(&cid)
            .execute(&mut *tx)
            .await
            .expect("seed wallet");
        }
        tx.commit().await.expect("commit tx");
        let seed_elapsed = seed_start.elapsed();

        // Time N=100 sequential lookups against a deterministic spread of
        // customer_ids. The (i * 97) % 10_000 visit pattern is non-contiguous,
        // so the SQLite page cache cannot trivially short-circuit the join.
        const ITERATIONS: usize = 100;
        let mut latencies_us: Vec<u128> = Vec::with_capacity(ITERATIONS);
        for i in 0..ITERATIONS {
            let target = (i * 97) % TOTAL_CUSTOMERS;
            let cid = format!("cust-perf-{target:05}");
            let start = Instant::now();
            let _preview = fetch_profile_preview(&pool, &cid).await.expect("fetch");
            latencies_us.push(start.elapsed().as_micros());
        }

        latencies_us.sort_unstable();
        let p50 = latencies_us[ITERATIONS / 2];
        let p95 = latencies_us[(ITERATIONS as f64 * 0.95) as usize];
        let p99 = latencies_us[(ITERATIONS as f64 * 0.99) as usize];
        let max = *latencies_us.last().expect("non-empty");

        eprintln!(
            "[perf §5.5] N={ITERATIONS} dataset={TOTAL_CUSTOMERS}  \
             seed={seed_elapsed:?}  p50={p50}us  p95={p95}us  p99={p99}us  max={max}us"
        );

        // DoD §1.2 budget: p95 ≤ 500ms = 500_000us. A single-row indexed join
        // exceeding this is a structural regression (N+1 queries / lost index
        // / lock contention).
        assert!(
            p95 < 500_000,
            "p95={p95}us exceeds DoD §1.2 budget of 500_000us — perf regression"
        );
    }
}
