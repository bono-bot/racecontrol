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
    pub staff_id: Uuid,
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
