// V2 billing/timing session. Per CR-6 a session is bound to a pod, not just
// a customer — pod-is-unit-of-session. Multiplayer sessions reference a
// `lobby_id`; solo sessions leave it NULL.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Session {
    pub id: Uuid,
    pub customer_id: Uuid,
    pub pod_id: String,
    pub lobby_id: Option<Uuid>,
    pub session_type: SessionType,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub billing_state: BillingState,
    pub credits_consumed: i64,
    // PACT-018 AMEND-1 NEW-FINDING-1 absorbed: staff_id is String not Uuid.
    // staff(id) is TEXT PRIMARY KEY (not UUID) per PACT-018 §2.2 — admin UI
    // assigns whatever string shape it currently uses. Cross-stack JSON-string
    // consistency; FK target alignment with TEXT staff(id).
    pub staff_id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
pub enum SessionType {
    Solo,
    MpLobbyMember,
    Ps5,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
pub enum BillingState {
    Active,
    BlankingGrace, // C7 wallet floor: 2-min grace + auto-blank
    EndedNormal,
    EndedLowBalance,
    EndedHardwareFail,
    EndedGameCrash,
    EndedStaffCancelled,
}
