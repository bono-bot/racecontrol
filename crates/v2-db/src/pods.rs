// V2 pod registry. Per C8 hardware: 1 POS PC + 1 cafe tablet + 1 Kiosk on
// .23 + 8 sim pods + N PS5 stations. Pod IDs are TEXT (not Uuid) so the
// staff-facing identifier matches what's painted on the pod door.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Pod {
    pub id: String,
    pub display_name: String,
    pub hardware_class: HardwareClass,
    pub ip_address: Option<String>,
    pub status: PodStatus,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
pub enum HardwareClass {
    SimPod,
    Ps5Station,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
pub enum PodStatus {
    Active,
    Maintenance,
    Offline,
}
