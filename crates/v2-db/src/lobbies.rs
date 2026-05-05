// V2 multiplayer lobby — host customer + N pods. Per Scenario 4 friends-MP
// and CR-6 pod-is-unit-of-session, the lobby is a session container, not a
// session itself. Each pod in the lobby has its own `sessions` row referencing
// `lobby_id`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Lobby {
    pub id: Uuid,
    pub host_customer_id: Uuid,
    pub pod_count: i32,
    pub billing_mode: BillingMode,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
pub enum BillingMode {
    SplitEqual,
    HostPays,
    PerSession,
}
