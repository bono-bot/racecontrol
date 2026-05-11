// V2 customer entity. Schema in migrations/20260503000001_initial_schema.sql.
//
// V2.0 identifier per Captain C5 = staff-typed phone. Driver classes per C3 =
// Rookie / Apex / Podium / Champion (overrides V1 Silver/Gold/Bronze).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Customer {
    pub id: Uuid,
    pub phone: String,
    pub display_name: Option<String>,
    pub email: Option<String>,
    pub driver_class: DriverClass,
    pub registered_at_pwa: bool,
    pub tnc_accepted_at: Option<DateTime<Utc>>,
    pub dpdp_consent_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
pub enum DriverClass {
    Rookie,
    Apex,
    Podium,
    Champion,
}
