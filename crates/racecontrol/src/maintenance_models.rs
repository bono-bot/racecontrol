//! Phase 2 (v29.0): Maintenance event data models.
//!
//! Core types for the unified maintenance event system — tracking self-heal
//! attempts, staff interventions, predictive alerts, component RUL, and
//! business metrics.  All monetary values use `_paise: i64`.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum MaintenanceEventType {
    SelfHealAttempted,
    Tier1FixApplied,
    PodHealerIntervention,
    AIDiagnosisCompleted,
    PredictiveAlert,
    StaffMaintenanceScheduled,
    StaffMaintenanceCompleted,
    PartReplaced,
    SoftwareUpdateApplied,
    EmergencyShutdown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, PartialOrd)]
#[serde(rename_all = "PascalCase")]
pub enum Severity {
    Critical,
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ComponentType {
    GPU,
    CPU,
    Memory,
    Storage,
    Network,
    PowerSupply,
    Cooling,
    Peripherals,
    Software,
    OS,
    Game,
    ForceFeedback,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ResolutionMethod {
    AutoHealed(String),
    ManualFix(String),
    PartReplacement(String),
    SoftwareUpdate(String),
    Restart,
    Unresolved,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum TaskStatus {
    Open,
    Assigned,
    InProgress,
    PendingValidation,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum StaffRole {
    Manager,
    Technician,
    FrontDesk,
    GameMaster,
    Cashier,
}

// ---------------------------------------------------------------------------
// Structs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceEvent {
    pub id: Uuid,
    pub pod_id: Option<u8>,
    pub event_type: MaintenanceEventType,
    pub severity: Severity,
    pub component: ComponentType,
    pub description: String,
    pub detected_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub resolution_method: Option<ResolutionMethod>,
    pub source: String,
    pub correlation_id: Option<Uuid>,
    pub revenue_impact_paise: Option<i64>,
    pub customers_affected: Option<u32>,
    pub downtime_minutes: Option<u32>,
    pub cost_estimate_paise: Option<i64>,
    pub assigned_staff_id: Option<String>,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceTask {
    pub id: Uuid,
    pub title: String,
    pub description: String,
    pub pod_id: Option<u8>,
    pub component: ComponentType,
    pub priority: u8,
    pub status: TaskStatus,
    pub created_at: DateTime<Utc>,
    pub due_by: Option<DateTime<Utc>>,
    pub assigned_to: Option<String>,
    pub source_event_id: Option<Uuid>,
    pub before_metrics: Option<serde_json::Value>,
    pub after_metrics: Option<serde_json::Value>,
    pub cost_estimate_paise: Option<i64>,
    pub actual_cost_paise: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceSummary {
    pub total_events: u32,
    pub by_severity: std::collections::HashMap<String, u32>,
    pub by_type: std::collections::HashMap<String, u32>,
    pub mttr_minutes: f64,
    pub self_heal_rate: f64,
    pub open_tasks: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Employee {
    pub id: Uuid,
    pub name: String,
    pub role: StaffRole,
    pub skills: Vec<String>,
    pub hourly_rate_paise: i64,
    pub phone: String,
    pub is_active: bool,
    pub face_enrollment_id: Option<String>,
    pub hired_at: NaiveDate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyBusinessMetrics {
    pub date: NaiveDate,
    pub revenue_gaming_paise: i64,
    pub revenue_cafe_paise: i64,
    pub revenue_other_paise: i64,
    pub expense_rent_paise: i64,
    pub expense_utilities_paise: i64,
    pub expense_salaries_paise: i64,
    pub expense_maintenance_paise: i64,
    pub expense_other_paise: i64,
    pub sessions_count: u32,
    pub occupancy_rate_pct: f32,
    pub peak_occupancy_pct: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentRUL {
    pub pod_id: u8,
    pub component: ComponentType,
    pub component_name: String,
    pub rul_hours: f32,
    pub rul_confidence: f32,
    pub degradation_rate_per_day: f64,
    pub last_updated: DateTime<Utc>,
    pub method: String,
    pub explanation: String,
}

// ---------------------------------------------------------------------------
// Phase 11 (v29.0): Business intelligence
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EbitdaSummary {
    pub total_revenue_paise: i64,
    pub total_expenses_paise: i64,
    pub ebitda_paise: i64,
    pub days: u32,
    pub avg_daily_ebitda_paise: i64,
    pub best_day: Option<String>,
    pub worst_day: Option<String>,
}

// ---------------------------------------------------------------------------
// Phase 14 (v29.0): Attendance tracking
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttendanceRecord {
    pub id: Uuid,
    pub employee_id: Uuid,
    pub date: NaiveDate,
    pub clock_in: Option<String>,
    pub clock_out: Option<String>,
    pub source: String,
    pub hours_worked: f64,
}

// ---------------------------------------------------------------------------
// Phase 17 (v29.0): Payroll & labor cost
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayrollSummary {
    pub year: i32,
    pub month: u32,
    pub total_hours: f64,
    pub total_paise: i64,
    pub by_employee: Vec<EmployeePayroll>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmployeePayroll {
    pub employee_id: String,
    pub name: String,
    pub hours_worked: f64,
    pub rate_paise: i64,
    pub total_paise: i64,
}
