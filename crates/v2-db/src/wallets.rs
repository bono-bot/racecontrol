// V2 wallet — Single-Purpose Voucher under Wallet Framing C
// (Captain decision 2026-05-03).
//
// Legal basis: CGST Sec 13(4)(a) + CBIC Circular 106/25/2019 para 4(a).
// 18% GST collected at top-up (NOT redemption). Credits redeem ONLY for
// sim racing + PS5. Cafe transactions live in `cafe_transactions` (sibling
// scope, deferred — see PACT-20260503-003 Q-ASK-Q2). Credits never expire
// to the customer; internal breakage at 24-36 months per Ind AS 115.30.
//
// Money is stored in PAISE (i64) end-to-end. Credits are an i64 count where
// 1 credit = ₹1 face value. No floats anywhere — audit + reconciliation
// requires exact equality.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Wallet {
    pub id: Uuid,
    pub customer_id: Uuid,
    pub balance_credits: i64,
    pub last_activity_at: DateTime<Utc>,
    pub breakage_recognized_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct WalletTopup {
    pub id: Uuid,
    pub wallet_id: Uuid,
    pub credits_purchased: i64,
    pub gst_collected_paise: i64,
    pub amount_paid_paise: i64,
    pub gst_rate_bps: i32, // basis points; 1800 = 18%
    pub payment_method: PaymentMethod,
    pub payment_ref: Option<String>,
    // PACT-018 AMEND-1 NEW-FINDING-1 absorbed: staff_id is String not Uuid.
    // staff(id) is TEXT PRIMARY KEY (not UUID) per PACT-018 §2.2.
    pub staff_id: String,
    pub pos_id: String,
    pub tax_invoice_no: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct WalletRedemption {
    pub id: Uuid,
    pub wallet_id: Uuid,
    pub session_id: Uuid,
    pub credits_redeemed: i64,
    pub redeemed_for: RedemptionKind,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
pub enum PaymentMethod {
    Cash,
    Upi,
    Card,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
pub enum RedemptionKind {
    Sim,
    Ps5,
}

// ─── W1-S9 GST helpers (§S-101 cross-cutting GST-INCLUSIVE doctrine) ────────
//
// V2 Wave 1 W1-S9 — Captain disposition 2026-05-08 ~06:41 IST verbatim
// "Remember all prices are inclusive of GST" — RATIFIED past silent-expire.
//
// All customer-facing prices V2.0 are GST-inclusive (tax embedded). Operator
// reporting / Wave 4 MI viability dashboard / break-even calculations need the
// net (post-GST) amount. These pure helpers extract net + GST from an inclusive
// figure without coupling to AppState or DB.

/// Standard Indian GST rate at V2.0 launch: 18% (basis points = 1800).
/// `WalletTopup.gst_rate_bps` carries the actual rate per top-up for audit
/// (CHECK constraint on schema locks 1800 per Wallet Framing C migration 003).
pub const RP_GST_RATE_BPS_18: i32 = 1800;

/// Extract operator-net paise from a GST-inclusive paise figure.
///
/// Math: `net = inclusive × 10000 / (10000 + rate_bps)` (integer floor).
///
/// Examples (18% / rate_bps=1800):
/// - ₹118 (11800p) → ₹100 (10000p) net
/// - ₹15400 (1540000p) → ₹13050.84 → 1305084p net (floor)
///
/// Returns the input unchanged if `gst_rate_bps <= 0` (degenerate / disabled).
pub fn operator_net_paise(inclusive_paise: i64, gst_rate_bps: i32) -> i64 {
    if gst_rate_bps <= 0 {
        return inclusive_paise;
    }
    let denom: i64 = 10_000 + gst_rate_bps as i64;
    inclusive_paise.saturating_mul(10_000) / denom
}

/// Extract the GST portion of paise from a GST-inclusive paise figure.
///
/// Math: `gst = inclusive × rate_bps / (10000 + rate_bps)` (integer floor).
///
/// Examples (18% / rate_bps=1800):
/// - ₹118 (11800p) → ₹18 (1800p) GST
/// - ₹15400 (1540000p) → ₹2349.15 → 234915p GST (floor)
///
/// Returns 0 if `gst_rate_bps <= 0`. The sum
/// `operator_net_paise() + gst_amount_paise()` may differ from `inclusive_paise`
/// by ±1 paise due to integer floor on both helpers — acceptable for operator
/// reporting (auditors handle paise-level rounding).
pub fn gst_amount_paise(inclusive_paise: i64, gst_rate_bps: i32) -> i64 {
    if gst_rate_bps <= 0 {
        return 0;
    }
    let denom: i64 = 10_000 + gst_rate_bps as i64;
    inclusive_paise.saturating_mul(gst_rate_bps as i64) / denom
}

// ─── W1-S2 WalletService (Wave 1 reserve+reconcile, kaizen-min) ─────────────
//
// V2 Wave 1 W1-S2 — Captain dispositions §S-82 + §S-83 + Wallet Framing C +
// PACT-013 RATIFIED. Surface scope:
// - reserve_credits — optimistic atomic debit on session start
// - reconcile_redemption — settle reserved-vs-actual at session end + insert
//   `wallet_redemptions` audit row
//
// HOLD-RELEASE-CAPTURE state machine + idempotency cascade DEFERRED to Wave 3
// (per PHASE-1-WAVE-1-PLAN.md §1.2 OOS / PACT-024 wallet concurrency parent).
// Wave 1 surface is naive optimistic UPDATE; race-safety relies on SQLite
// single-writer + WHERE-guarded predicate + CHECK(balance_credits >= 0).

use crate::{DbPool, Error};

/// Wave 1 W1-S2 wallet service. Per Wallet Framing C: 18% GST collected at
/// TOP-UP (not redemption); credits redeem ONLY for sim/PS5 (cafe is sibling
/// scope per PACT-20260503-003 Q-ASK-Q2).
///
/// Stateless — methods take `&DbPool` directly. No per-customer locks; SQLite
/// single-writer is the concurrency floor.
#[derive(Debug, Default)]
pub struct WalletService;

impl WalletService {
    /// Atomically debit `credits` from the customer's wallet. Fails with
    /// `Error::InsufficientFunds` if `balance_credits < credits` and
    /// `Error::WalletNotFound` if no wallet row exists for the customer.
    ///
    /// Returns the new balance after debit. Updates `last_activity_at` and
    /// `updated_at` to current UTC in the same UPDATE.
    ///
    /// Wave 1 kaizen-min: NO idempotency key (deferred to Wave 3 PACT-024).
    /// Concurrent reserve calls for the same customer rely on SQLite
    /// single-writer + WHERE-guarded predicate; the second UPDATE will return
    /// 0 rows (and trip InsufficientFunds) if the first consumed the balance.
    ///
    /// Edge cases:
    /// - `credits == 0` → no-op; returns current balance (or WalletNotFound)
    /// - `credits < 0` → returns InsufficientFunds (the caller is asking for a
    ///   negative debit; they should call `reconcile_redemption` for refunds)
    pub async fn reserve_credits(
        pool: &DbPool,
        customer_id: uuid::Uuid,
        credits: i64,
    ) -> Result<i64, Error> {
        if credits < 0 {
            // Negative debit not a valid reserve; surface as InsufficientFunds
            // so callers see the structural mismatch.
            let probe: Option<(i64,)> =
                sqlx::query_as("SELECT balance_credits FROM wallets WHERE customer_id = ?")
                    .bind(customer_id)
                    .fetch_optional(pool)
                    .await?;
            return match probe {
                Some((b,)) => Err(Error::InsufficientFunds {
                    customer_id,
                    balance_credits: b,
                    required_credits: credits,
                }),
                None => Err(Error::WalletNotFound(customer_id)),
            };
        }

        if credits == 0 {
            // No-op: query current balance for return-value parity.
            let row: Option<(i64,)> =
                sqlx::query_as("SELECT balance_credits FROM wallets WHERE customer_id = ?")
                    .bind(customer_id)
                    .fetch_optional(pool)
                    .await?;
            return row
                .map(|(b,)| b)
                .ok_or(Error::WalletNotFound(customer_id));
        }

        let now = Utc::now();
        let row: Option<(i64,)> = sqlx::query_as(
            "UPDATE wallets
             SET balance_credits = balance_credits - ?,
                 last_activity_at = ?,
                 updated_at = ?
             WHERE customer_id = ? AND balance_credits >= ?
             RETURNING balance_credits",
        )
        .bind(credits)
        .bind(now)
        .bind(now)
        .bind(customer_id)
        .bind(credits)
        .fetch_optional(pool)
        .await?;

        match row {
            Some((new_balance,)) => Ok(new_balance),
            None => {
                // Either wallet doesn't exist or insufficient balance — disambiguate.
                let probe: Option<(i64,)> = sqlx::query_as(
                    "SELECT balance_credits FROM wallets WHERE customer_id = ?",
                )
                .bind(customer_id)
                .fetch_optional(pool)
                .await?;
                match probe {
                    None => Err(Error::WalletNotFound(customer_id)),
                    Some((current,)) => Err(Error::InsufficientFunds {
                        customer_id,
                        balance_credits: current,
                        required_credits: credits,
                    }),
                }
            }
        }
    }

    /// Settle the difference between reserved and actual credits at session
    /// end. Inserts a `wallet_redemptions` row recording `actual_credits` when
    /// `actual_credits > 0` (schema CHECK constraint).
    ///
    /// `delta = reserved - actual`:
    /// - `delta > 0`: overage refunded → wallet balance += delta
    /// - `delta < 0`: underage debited → wallet balance -= |delta| (may fail
    ///   InsufficientFunds if customer wallet now empty due to interim spend)
    /// - `delta == 0`: no balance change
    ///
    /// Edge case: `actual_credits == 0` (e.g. session canceled before any
    /// consumption) → no `wallet_redemptions` row inserted (schema requires
    /// `credits_redeemed > 0`); reserved credits returned to wallet in full.
    ///
    /// Returns the new wallet balance after reconciliation.
    pub async fn reconcile_redemption(
        pool: &DbPool,
        customer_id: uuid::Uuid,
        session_id: uuid::Uuid,
        reserved_credits: i64,
        actual_credits: i64,
        redeemed_for: RedemptionKind,
    ) -> Result<i64, Error> {
        // Look up wallet_id (FK target for wallet_redemptions row).
        let wallet_row: Option<(uuid::Uuid,)> =
            sqlx::query_as("SELECT id FROM wallets WHERE customer_id = ?")
                .bind(customer_id)
                .fetch_optional(pool)
                .await?;
        let wallet_id = wallet_row
            .map(|(id,)| id)
            .ok_or(Error::WalletNotFound(customer_id))?;

        // Insert audit row only when actual_credits > 0 (schema CHECK).
        if actual_credits > 0 {
            let redemption_id = uuid::Uuid::new_v4();
            let now = Utc::now();
            sqlx::query(
                "INSERT INTO wallet_redemptions
                 (id, wallet_id, session_id, credits_redeemed, redeemed_for, created_at)
                 VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(redemption_id)
            .bind(wallet_id)
            .bind(session_id)
            .bind(actual_credits)
            .bind(redeemed_for)
            .bind(now)
            .execute(pool)
            .await?;
        }

        let delta = reserved_credits - actual_credits;
        let now = Utc::now();

        if delta == 0 {
            // No balance change — refresh activity timestamps + return current.
            let row: Option<(i64,)> = sqlx::query_as(
                "UPDATE wallets
                 SET last_activity_at = ?,
                     updated_at = ?
                 WHERE customer_id = ?
                 RETURNING balance_credits",
            )
            .bind(now)
            .bind(now)
            .bind(customer_id)
            .fetch_optional(pool)
            .await?;
            return row
                .map(|(b,)| b)
                .ok_or(Error::WalletNotFound(customer_id));
        }

        if delta > 0 {
            // Refund overage to wallet.
            let row: Option<(i64,)> = sqlx::query_as(
                "UPDATE wallets
                 SET balance_credits = balance_credits + ?,
                     last_activity_at = ?,
                     updated_at = ?
                 WHERE customer_id = ?
                 RETURNING balance_credits",
            )
            .bind(delta)
            .bind(now)
            .bind(now)
            .bind(customer_id)
            .fetch_optional(pool)
            .await?;
            return row
                .map(|(b,)| b)
                .ok_or(Error::WalletNotFound(customer_id));
        }

        // delta < 0 — additional debit needed for underage. Reuse reserve_credits
        // for atomic guard + InsufficientFunds disambiguation.
        Self::reserve_credits(pool, customer_id, -delta).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{migrate, open};

    /// Spin up a fresh in-memory-equivalent SQLite pool with all migrations
    /// applied. Each test gets its own pool — full isolation.
    async fn test_pool() -> (DbPool, tempfile::NamedTempFile) {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_str().unwrap();
        let pool = open(path).await.expect("open pool");
        migrate(&pool).await.expect("apply migrations");
        (pool, tmp)
    }

    /// Seed the FK chain: customer → wallet (with `balance_credits`),
    /// pod (sim_pod), staff (active cashier), session (solo).
    /// Returns (customer_id, session_id) for use in reconcile tests.
    async fn seed_fk_chain(pool: &DbPool, balance_credits: i64) -> (uuid::Uuid, uuid::Uuid) {
        let customer_id = uuid::Uuid::new_v4();
        let wallet_id = uuid::Uuid::new_v4();
        let pod_id = format!("test-pod-{}", uuid::Uuid::new_v4().simple());
        let staff_id = format!("test-staff-{}", uuid::Uuid::new_v4().simple());
        let session_id = uuid::Uuid::new_v4();

        sqlx::query("INSERT INTO customers (id, phone) VALUES (?, ?)")
            .bind(customer_id)
            .bind(format!("+91{}", customer_id.simple()))
            .execute(pool)
            .await
            .unwrap();

        sqlx::query(
            "INSERT INTO wallets (id, customer_id, balance_credits) VALUES (?, ?, ?)",
        )
        .bind(wallet_id)
        .bind(customer_id)
        .bind(balance_credits)
        .execute(pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO pods (id, display_name, hardware_class) VALUES (?, ?, 'sim_pod')",
        )
        .bind(&pod_id)
        .bind("Test Pod")
        .execute(pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO staff (id, name, phone, pin, role, active)
             VALUES (?, ?, ?, ?, 'cashier', 1)",
        )
        .bind(&staff_id)
        .bind("Test Staff")
        .bind(format!("999{}", &uuid::Uuid::new_v4().simple().to_string()[..7]))
        .bind(format!("p{}", &uuid::Uuid::new_v4().simple().to_string()[..3]))
        .execute(pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO sessions (id, customer_id, pod_id, session_type, started_at, staff_id)
             VALUES (?, ?, ?, 'solo', ?, ?)",
        )
        .bind(session_id)
        .bind(customer_id)
        .bind(&pod_id)
        .bind(Utc::now())
        .bind(&staff_id)
        .execute(pool)
        .await
        .unwrap();

        (customer_id, session_id)
    }

    // ─── W1-S9 GST helper tests ─────────────────────────────────────────────

    /// §S-101 example A: ₹118 inclusive → ₹100 net at 18%.
    #[test]
    fn operator_net_paise_18pct_canonical() {
        // ₹118 (11800 paise) inclusive → ₹100 (10000 paise) net
        assert_eq!(operator_net_paise(11_800, RP_GST_RATE_BPS_18), 10_000);
    }

    /// §S-101 example B: ₹15,400/day inclusive break-even → ~₹13,050 net.
    #[test]
    fn operator_net_paise_18pct_breakeven_anchor() {
        // ₹15,400 (1,540,000 paise) inclusive → 1,305,084 paise net (floor)
        // §S-101 quotes ~₹13,051 (round); floor math yields 1,305,084
        assert_eq!(operator_net_paise(1_540_000, RP_GST_RATE_BPS_18), 1_305_084);
    }

    /// GST extraction at 18%: ₹118 inclusive → ₹18 GST.
    #[test]
    fn gst_amount_paise_18pct_canonical() {
        assert_eq!(gst_amount_paise(11_800, RP_GST_RATE_BPS_18), 1_800);
    }

    /// Sum invariant: net + gst ≈ inclusive (within ±1 paise floor rounding).
    #[test]
    fn gst_inclusive_split_sum_invariant() {
        let inclusive = 11_800_i64; // ₹118
        let net = operator_net_paise(inclusive, RP_GST_RATE_BPS_18);
        let gst = gst_amount_paise(inclusive, RP_GST_RATE_BPS_18);
        assert!((net + gst - inclusive).abs() <= 1);
        // Larger anchor: ₹15,400
        let inclusive2 = 1_540_000_i64;
        let net2 = operator_net_paise(inclusive2, RP_GST_RATE_BPS_18);
        let gst2 = gst_amount_paise(inclusive2, RP_GST_RATE_BPS_18);
        assert!((net2 + gst2 - inclusive2).abs() <= 1);
    }

    /// Disabled-rate edge case (gst_rate_bps == 0): pass-through.
    #[test]
    fn operator_net_paise_zero_rate_passthrough() {
        assert_eq!(operator_net_paise(11_800, 0), 11_800);
        assert_eq!(gst_amount_paise(11_800, 0), 0);
    }

    // ─── W1-S2 WalletService tests ──────────────────────────────────────────

    #[tokio::test]
    async fn reserve_credits_success_returns_new_balance() {
        let (pool, _tmp) = test_pool().await;
        let (customer_id, _session_id) = seed_fk_chain(&pool, 1_000).await;
        let new_balance = WalletService::reserve_credits(&pool, customer_id, 250)
            .await
            .expect("reserve should succeed");
        assert_eq!(new_balance, 750);
    }

    #[tokio::test]
    async fn reserve_credits_insufficient_funds_returns_error_with_actual_balance() {
        let (pool, _tmp) = test_pool().await;
        let (customer_id, _session_id) = seed_fk_chain(&pool, 100).await;
        let err = WalletService::reserve_credits(&pool, customer_id, 250)
            .await
            .expect_err("reserve should fail");
        match err {
            Error::InsufficientFunds {
                balance_credits,
                required_credits,
                ..
            } => {
                assert_eq!(balance_credits, 100);
                assert_eq!(required_credits, 250);
            }
            other => panic!("expected InsufficientFunds, got {:?}", other),
        }
        // Confirm balance untouched
        let row: (i64,) =
            sqlx::query_as("SELECT balance_credits FROM wallets WHERE customer_id = ?")
                .bind(customer_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(row.0, 100);
    }

    #[tokio::test]
    async fn reserve_credits_wallet_not_found_returns_error() {
        let (pool, _tmp) = test_pool().await;
        let phantom_customer = uuid::Uuid::new_v4();
        let err = WalletService::reserve_credits(&pool, phantom_customer, 100)
            .await
            .expect_err("reserve should fail");
        assert!(matches!(err, Error::WalletNotFound(_)));
    }

    #[tokio::test]
    async fn reserve_credits_zero_is_noop_returns_current_balance() {
        let (pool, _tmp) = test_pool().await;
        let (customer_id, _session_id) = seed_fk_chain(&pool, 500).await;
        let balance = WalletService::reserve_credits(&pool, customer_id, 0)
            .await
            .expect("zero-reserve should succeed as noop");
        assert_eq!(balance, 500);
    }

    #[tokio::test]
    async fn reconcile_redemption_refund_overage() {
        let (pool, _tmp) = test_pool().await;
        let (customer_id, session_id) = seed_fk_chain(&pool, 1_000).await;
        // Pre-debit 750 (reserve)
        WalletService::reserve_credits(&pool, customer_id, 750)
            .await
            .unwrap();
        // Actual was 600 (overage 150 to refund)
        let new_balance = WalletService::reconcile_redemption(
            &pool,
            customer_id,
            session_id,
            750,
            600,
            RedemptionKind::Sim,
        )
        .await
        .expect("reconcile should succeed");
        // 1000 - 750 + 150 = 400
        assert_eq!(new_balance, 400);
        // Verify wallet_redemptions row created with actual=600
        let credits: (i64,) = sqlx::query_as(
            "SELECT credits_redeemed FROM wallet_redemptions WHERE session_id = ?",
        )
        .bind(session_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(credits.0, 600);
    }

    #[tokio::test]
    async fn reconcile_redemption_charge_underage_succeeds_when_balance_sufficient() {
        let (pool, _tmp) = test_pool().await;
        let (customer_id, session_id) = seed_fk_chain(&pool, 1_000).await;
        // Reserve 200; actual 350 (underage 150 needs additional debit)
        WalletService::reserve_credits(&pool, customer_id, 200)
            .await
            .unwrap();
        let new_balance = WalletService::reconcile_redemption(
            &pool,
            customer_id,
            session_id,
            200,
            350,
            RedemptionKind::Sim,
        )
        .await
        .expect("reconcile should succeed (balance 800 >= underage 150)");
        // 1000 - 200 - 150 = 650
        assert_eq!(new_balance, 650);
    }

    #[tokio::test]
    async fn reconcile_redemption_zero_actual_full_refund_no_redemption_row() {
        let (pool, _tmp) = test_pool().await;
        let (customer_id, session_id) = seed_fk_chain(&pool, 1_000).await;
        WalletService::reserve_credits(&pool, customer_id, 500)
            .await
            .unwrap();
        // Session canceled before consumption: actual=0
        let new_balance = WalletService::reconcile_redemption(
            &pool,
            customer_id,
            session_id,
            500,
            0,
            RedemptionKind::Sim,
        )
        .await
        .expect("reconcile zero-actual should succeed");
        // Full refund: 1000 - 500 + 500 = 1000
        assert_eq!(new_balance, 1_000);
        // Verify NO wallet_redemptions row (schema CHECK credits_redeemed > 0)
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM wallet_redemptions WHERE session_id = ?",
        )
        .bind(session_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn reconcile_redemption_zero_delta_inserts_row_no_balance_change() {
        let (pool, _tmp) = test_pool().await;
        let (customer_id, session_id) = seed_fk_chain(&pool, 1_000).await;
        WalletService::reserve_credits(&pool, customer_id, 750)
            .await
            .unwrap();
        let new_balance = WalletService::reconcile_redemption(
            &pool,
            customer_id,
            session_id,
            750,
            750,
            RedemptionKind::Sim,
        )
        .await
        .expect("reconcile zero-delta should succeed");
        assert_eq!(new_balance, 250); // 1000 - 750 = 250
        let credits: (i64,) = sqlx::query_as(
            "SELECT credits_redeemed FROM wallet_redemptions WHERE session_id = ?",
        )
        .bind(session_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(credits.0, 750);
    }
}
