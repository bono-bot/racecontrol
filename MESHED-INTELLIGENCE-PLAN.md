# Meshed Intelligence v29.0 — Preventive Maintenance & Operational Intelligence

**14-Model MMA Consensus Plan** | 2026-03-30 | James Vowles
**Models:** Claude Opus 4, Claude Sonnet 4, GPT-5.4, GPT-4.1, Gemini 2.5 Pro, Gemini 2.5 Flash, DeepSeek R1, Qwen3-235B, QwQ-32B, Llama 4 Maverick, Nemotron Ultra 253B, Mistral Large, Phi-4, Hermes 4 405B

---

## Guiding Principles (14/14 consensus)

1. **Extend, don't replace** — Reuse existing modules (telemetry_store, pod_healer, failure_monitor, self_heal, activity_log, error_aggregator, experience_score, scheduler, whatsapp_alerter, cloud_sync, psychology). No new crates.
2. **Rule-based first, ML later** — Static thresholds → statistical baselines → local Ollama → cloud OpenRouter → full ML models (only after 3+ months of labeled data).
3. **Local AI is default** — Ollama qwen2.5:3b for routine, llama3.1:8b for complex. Cloud OpenRouter only for high-value analysis ($5/day/pod budget).
4. **Integer paise for all money** — `_paise: i64` suffix, never `f64` for currency.
5. **Typed Rust enums** — All event types, severities, statuses as enums with `#[derive(Serialize, Deserialize)]`.
6. **Business-impact scoring** — Every maintenance decision weighted by revenue impact, not just technical severity.
7. **XAI on all AI decisions** — Every automated recommendation includes a human-readable explanation.

---

## Architecture: Module Placement

```
racecontrol (server :8080)        rc-agent (pod :8090)           rc-sentry-ai (James)
├── maintenance_models.rs (NEW)   ├── predictive_maintenance.rs  ├── attendance/ (EXISTS)
├── maintenance_store.rs  (NEW)   │   (EXTEND: add probes)       ├── alerts/ (EXISTS)
├── maintenance_engine.rs (NEW)   ├── failure_monitor.rs          └── ai_diagnosis.rs (NEW)
├── maintenance_scheduler.rs(NEW) │   (EXTEND: pattern emit)
├── maintenance_kpi.rs    (NEW)   ├── experience_score.rs
├── maintenance_xai.rs    (NEW)   │   (EXTEND: feedback loop)
├── business_analytics.rs (NEW)   └── maintenance_probe.rs (NEW)
├── business_forecast.rs  (NEW)
├── hr_models.rs          (NEW)   rc-sentry (pod :8091)
├── hr_store.rs           (NEW)   ├── watchdog.rs (EXISTS)
├── payroll.rs            (NEW)   ├── tier1_fixes.rs (EXISTS)
├── dynamic_pricing.rs    (NEW)   └── maintenance_gate.rs (NEW)
├── occupancy_analytics.rs(NEW)
├── feedback_loop.rs      (NEW)   Next.js Admin (:3201)
├── snapshot_manager.rs   (NEW)   ├── /maintenance (NEW)
├── telemetry_store.rs  (EXTEND)  ├── /maintenance/[podId] (NEW)
├── pod_healer.rs       (EXTEND)  ├── /analytics/business (NEW)
├── scheduler.rs        (EXTEND)  ├── /analytics/ebitda (NEW)
├── error_aggregator.rs (EXTEND)  ├── /hr/employees (NEW)
├── activity_log.rs     (EXTEND)  ├── /hr/shifts (NEW)
└── whatsapp_alerter.rs (EXTEND)  └── /hr/payroll (NEW)
```

---

## Data Flow

```
Pods (rc-agent)          James (rc-sentry-ai)        Server (racecontrol)
┌─────────────┐          ┌──────────────┐            ┌──────────────────────┐
│ Telemetry   │──WS───→  │              │            │ telemetry_store      │
│ GPU/CPU/FPS │          │ Ollama AI    │←──HTTP──── │ maintenance_engine   │
│ SMART/Events│          │ qwen2.5:3b   │───resp───→ │   ↓                  │
│ Game crashes│          │ llama3.1:8b  │            │ maintenance_scheduler│
└──────┬──────┘          └──────────────┘            │   ↓                  │
       │                                              │ maintenance_kpi     │
       └──────────WS/HTTP─────────────────────────→  │   ↓                  │
                                                      │ business_analytics  │
     Admin Dashboard (:3201) ←──────── API ──────── │   ↓                  │
     WhatsApp ←──────────── whatsapp_alerter ────── │ whatsapp_alerter     │
     PWA (:3200) ←──────────── WebSocket ────────── │ feedback_loop        │
                                                      └──────────────────────┘
```

---

## Phase Plan: 25 Phases, 5 Tiers, ~8 Weeks

### Tier 1: Foundation (Phases 1-5) — Week 1-2

---

### Phase 1: Extended Telemetry Collection
**Goal:** Capture hardware health metrics beyond current CPU/GPU/FPS
**Duration:** 2 days | **Consensus:** 13/14

**Modify:** `rc-agent/src/predictive_maintenance.rs`
```rust
/// Extended hardware metrics for preventive maintenance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtendedTelemetry {
    pub pod_id: u8,
    pub timestamp: DateTime<Utc>,

    // Existing (already collected)
    pub cpu_usage_pct: f32,
    pub gpu_usage_pct: f32,
    pub memory_usage_pct: f32,
    pub disk_usage_pct: f32,
    pub fps: u16,
    pub frame_loss_pct: f32,
    pub network_latency_ms: u32,

    // NEW: Hardware health
    pub gpu_temp_celsius: Option<f32>,         // nvidia-smi or nvml
    pub cpu_temp_celsius: Option<f32>,         // WMI MSAcpi_ThermalZoneTemperature
    pub gpu_power_watts: Option<f32>,          // nvidia-smi
    pub fan_speed_rpm: Option<Vec<u32>>,       // WMI Win32_Fan
    pub disk_smart_health_pct: Option<u8>,     // smartctl or WMI
    pub disk_power_on_hours: Option<u32>,      // SMART attribute
    pub disk_reallocated_sectors: Option<u32>, // SMART attribute
    pub vram_usage_mb: Option<u32>,            // nvidia-smi

    // NEW: System health
    pub game_crashes_last_hour: Option<u8>,    // from game_doctor.rs
    pub windows_critical_errors: Option<Vec<String>>, // Get-WinEvent last 5min
    pub process_handle_count: Option<u32>,     // GetProcessHandleCount
    pub system_uptime_secs: Option<u64>,       // GetTickCount64
    pub usb_device_count: Option<u8>,          // WMI Win32_USBControllerDevice
}
```

**API:** Extend existing WebSocket telemetry message. No new endpoint.
**Integration:** `mesh_gossip.rs` carries extended telemetry → `telemetry_store.rs` persists.
**Windows:** nvidia-smi via `Command::new("nvidia-smi")`, WMI via PowerShell `Get-WmiObject`, SMART via `wmic diskdrive get status`.
**Risk:** Performance overhead. **Mitigation:** Cache SMART/static metrics for 1hr, batch PowerShell calls, <1% CPU target. Graceful fallback if any collector fails.

---

### Phase 2: Maintenance Event Logging
**Goal:** Unified event system for all maintenance actions (auto-heal, manual fixes, part replacements)
**Duration:** 1 day | **Consensus:** 11/14

**Create:** `racecontrol/src/maintenance_models.rs`
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MaintenanceEventType {
    SelfHealAttempted,        // Tier 2 rc-agent
    Tier1FixApplied,          // rc-sentry
    PodHealerIntervention,    // Server Tier 3
    AIDiagnosisCompleted,     // Tier 4/5 Ollama/OpenRouter
    PredictiveAlert,          // Failure predicted before occurrence
    StaffMaintenanceScheduled,
    StaffMaintenanceCompleted,
    PartReplaced,
    SoftwareUpdateApplied,
    EmergencyShutdown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Severity { Critical, High, Medium, Low }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComponentType {
    GPU, CPU, Memory, Storage, Network, PowerSupply, Cooling,
    Peripherals, Software, OS, Game, ForceFeeback,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResolutionMethod {
    AutoHealed(String),      // tier name
    ManualFix(String),       // staff + description
    PartReplacement(String), // part name
    SoftwareUpdate(String),  // version
    Restart,
    Unresolved,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceEvent {
    pub id: Uuid,
    pub pod_id: Option<u8>,              // None for server-wide
    pub event_type: MaintenanceEventType,
    pub severity: Severity,
    pub component: ComponentType,
    pub description: String,
    pub detected_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub resolution_method: Option<ResolutionMethod>,
    pub source: String,                  // "rc-agent", "pod_healer", "admin"
    pub correlation_id: Option<Uuid>,    // links related events
    pub revenue_impact_paise: Option<i64>,
    pub customers_affected: Option<u32>,
    pub downtime_minutes: Option<u32>,
    pub cost_estimate_paise: Option<i64>,
    pub assigned_staff_id: Option<String>,
    pub metadata: serde_json::Value,     // flexible event-specific data
}
```

**Create:** `racecontrol/src/maintenance_store.rs` — CRUD for MaintenanceEvent
**API:**
- `POST /api/v1/maintenance/events` — Log event (used by all components)
- `GET /api/v1/maintenance/events?pod_id=1&severity=Critical&hours=24` — Query with filters
- `GET /api/v1/maintenance/summary?hours=24` — Dashboard summary (MTTR, self-heal rate, by-type counts)

**Integration:**
- `rc-agent/self_heal.rs` → log every heal attempt
- `rc-sentry/tier1_fixes.rs` → log every fix
- `racecontrol/pod_healer.rs` → log interventions
- `racecontrol/whatsapp_alerter.rs` → critical events trigger alerts

---

### Phase 3: Historical Data Warehouse + Retention
**Goal:** Time-series storage for 90-day trend analysis
**Duration:** 2 days | **Consensus:** 12/14

**Extend:** `racecontrol/src/telemetry_store.rs`
```rust
/// Aggregated telemetry for trend analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryAggregate {
    pub pod_id: u8,
    pub metric_name: String,      // "gpu_temp", "fps", "disk_health"
    pub period_start: DateTime<Utc>,
    pub period_hours: u8,         // 1 = hourly, 24 = daily
    pub min_value: f64,
    pub max_value: f64,
    pub avg_value: f64,
    pub median_value: f64,
    pub std_dev: f64,
    pub sample_count: u32,
    // Business context at time of measurement
    pub had_active_session: bool,
    pub was_peak_hours: bool,
}
```

**DB Schema:**
```sql
CREATE TABLE telemetry_raw (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    pod_id INTEGER NOT NULL,
    timestamp TEXT NOT NULL,       -- ISO 8601
    metric_name TEXT NOT NULL,
    metric_value REAL NOT NULL,
    has_active_session INTEGER DEFAULT 0,
    is_peak_hours INTEGER DEFAULT 0
);
CREATE INDEX idx_raw_pod_time ON telemetry_raw(pod_id, timestamp);
CREATE INDEX idx_raw_metric ON telemetry_raw(metric_name, timestamp);

CREATE TABLE telemetry_aggregates (
    pod_id INTEGER NOT NULL,
    metric_name TEXT NOT NULL,
    period_start TEXT NOT NULL,
    period_hours INTEGER NOT NULL,
    min_val REAL, max_val REAL, avg_val REAL, median_val REAL, std_dev REAL,
    sample_count INTEGER,
    PRIMARY KEY (pod_id, metric_name, period_start, period_hours)
);
```

**Retention policy (10/14 consensus):**
- Raw data: 7 days at 60s intervals
- Hourly aggregates: 30 days
- Daily aggregates: 90 days
- Archive: compressed JSON on Bono VPS via cloud_sync
- Cleanup: tokio cron job at 03:00 IST daily

**API:**
- `GET /api/v1/analytics/telemetry?pod_id=1&metric=gpu_temp&start=...&end=...&agg=hourly`
- `GET /api/v1/analytics/trends?pod_id=1&metric=disk_smart_health&window_days=30`
  - Response: `{ current_value, trend: "declining", rate_per_day: -0.5, predicted_failure_date, confidence }`

---

### Phase 4: Maintenance Dashboard Foundation
**Goal:** Admin Dashboard UI for maintenance events, telemetry trends, and component health
**Duration:** 2 days | **Consensus:** 12/14

**Create (Next.js Admin :3201):**
- `/maintenance` — Fleet overview: maintenance events timeline, active alerts, self-heal rate
- `/maintenance/[podId]` — Pod detail: component health gauges, telemetry sparklines, event history
- `/maintenance/tasks` — Open tasks/tickets, assignment, status tracking

**Components:**
- `MaintenanceTimeline.tsx` — chronological event feed with severity colors
- `ComponentHealthGauge.tsx` — RUL gauge per component (GPU, Storage, Network)
- `TelemetrySparkline.tsx` — 24h mini-chart using existing recharts
- `MaintenanceKPI.tsx` — MTTR, self-heal rate, events/day, downtime minutes

**Integration:** Fetches from Phase 2-3 APIs. WebSocket for real-time event updates.

---

### Phase 5: Rule-Based Anomaly Detection Engine
**Goal:** Detect anomalies using static thresholds + statistical baselines
**Duration:** 2 days | **Consensus:** 13/14

**Create:** `racecontrol/src/maintenance_engine.rs`
```rust
#[derive(Debug, Clone)]
pub struct AnomalyRule {
    pub name: String,
    pub component: ComponentType,
    pub severity: Severity,
    pub condition: AnomalyCondition,
    pub cooldown_minutes: u32,    // prevent alert fatigue
    pub min_sustained_minutes: u32, // debounce: must persist before alerting
}

pub enum AnomalyCondition {
    /// Value exceeds absolute threshold for N minutes
    AbsoluteThreshold { metric: String, threshold: f64, above: bool },
    /// Value deviates >N std_devs from 7-day rolling baseline
    StatisticalDeviation { metric: String, std_dev_threshold: f64 },
    /// Rate of change exceeds threshold (e.g., temp rising >2C/min)
    RateOfChange { metric: String, rate_per_minute: f64 },
    /// Value hasn't changed for N minutes (stuck sensor)
    Stale { metric: String, max_age_minutes: u32 },
}

/// Initial rule set (extend via admin dashboard later)
pub fn default_rules() -> Vec<AnomalyRule> {
    vec![
        // GPU overheating: >85C sustained for 5 min
        AnomalyRule {
            name: "GPU Overheat".into(),
            component: ComponentType::GPU,
            severity: Severity::High,
            condition: AnomalyCondition::AbsoluteThreshold {
                metric: "gpu_temp_celsius".into(), threshold: 85.0, above: true,
            },
            cooldown_minutes: 30,
            min_sustained_minutes: 5,
        },
        // Disk health declining: <80% SMART
        AnomalyRule {
            name: "Disk Health Warning".into(),
            component: ComponentType::Storage,
            severity: Severity::Medium,
            condition: AnomalyCondition::AbsoluteThreshold {
                metric: "disk_smart_health_pct".into(), threshold: 80.0, above: false,
            },
            cooldown_minutes: 1440, // once per day
            min_sustained_minutes: 60,
        },
        // FPS drop: >2 std devs below 7-day baseline
        AnomalyRule {
            name: "FPS Anomaly".into(),
            component: ComponentType::GPU,
            severity: Severity::Medium,
            condition: AnomalyCondition::StatisticalDeviation {
                metric: "fps".into(), std_dev_threshold: 2.0,
            },
            cooldown_minutes: 15,
            min_sustained_minutes: 3,
        },
        // Network latency spike
        AnomalyRule {
            name: "Network Latency Spike".into(),
            component: ComponentType::Network,
            severity: Severity::Medium,
            condition: AnomalyCondition::AbsoluteThreshold {
                metric: "network_latency_ms".into(), threshold: 100.0, above: true,
            },
            cooldown_minutes: 10,
            min_sustained_minutes: 2,
        },
        // Handle leak: >10000 handles
        AnomalyRule {
            name: "Handle Leak".into(),
            component: ComponentType::Software,
            severity: Severity::High,
            condition: AnomalyCondition::AbsoluteThreshold {
                metric: "process_handle_count".into(), threshold: 10000.0, above: true,
            },
            cooldown_minutes: 60,
            min_sustained_minutes: 10,
        },
    ]
}
```

**Integration:** Runs on `tokio::time::interval(Duration::from_secs(60))`, queries last N minutes of telemetry, fires matched rules → creates `MaintenanceEvent` (Phase 2) + WhatsApp alert if Critical.

**Risk:** False positives. **Mitigation:** `min_sustained_minutes` debounce + `cooldown_minutes` throttle. Adjustable via admin API.

---

### Tier 2: Core Predictive (Phases 6-10) — Week 2-3

---

### Phase 6: Failure Pattern Correlation
**Goal:** Detect multi-metric failure signatures (e.g., GPU temp + fan speed + FPS = imminent crash)
**Duration:** 2 days | **Consensus:** 11/14

**Create:** `racecontrol/src/maintenance_engine.rs` (extend)
```rust
/// Multi-metric pattern that predicts failure
pub struct FailurePattern {
    pub name: String,
    pub component: ComponentType,
    pub conditions: Vec<(String, f64, Ordering)>, // (metric, threshold, above/below)
    pub min_matching: usize,                       // how many conditions must match
    pub lookback_minutes: u32,
    pub confidence: f32,                           // 0.0-1.0
}

/// Historical correlation analysis
pub struct CorrelationResult {
    pub metric_a: String,
    pub metric_b: String,
    pub correlation: f64,        // Pearson r
    pub time_lag_minutes: i32,   // A leads B by N minutes
    pub sample_count: u32,
}
```

**Logic:** Hourly batch job computes Pearson correlation between all metric pairs per pod. Patterns with r > 0.7 (Hermes consensus) are surfaced. Admin validates and promotes to active patterns.

---

### Phase 7: RUL (Remaining Useful Life) Estimation
**Goal:** Per-component remaining life prediction
**Duration:** 2 days | **Consensus:** 12/14

**Create:** `racecontrol/src/maintenance_engine.rs` (extend)
```rust
pub struct ComponentRUL {
    pub pod_id: u8,
    pub component: ComponentType,
    pub component_name: String,          // "NVIDIA RTX 4070", "WD SN770 1TB"
    pub rul_hours: f32,                  // estimated remaining hours
    pub rul_confidence: f32,             // 0.0-1.0
    pub degradation_rate_per_day: f64,   // from Phase 3 trend API
    pub last_updated: DateTime<Utc>,
    pub method: RULMethod,
    pub explanation: String,             // XAI: why this estimate
}

pub enum RULMethod {
    LinearTrend,          // Simple linear extrapolation
    ExponentialDecay,     // For temperature-accelerated wear
    StatisticalBaseline,  // Based on fleet-wide component lifespans
    AIEstimate,           // Ollama/OpenRouter generated
}
```

**AI approach:** Start with `LinearTrend` (rule-based). After 30 days of data, enable `StatisticalBaseline`. After 90 days, enable Ollama-assisted `AIEstimate` (14/14 consensus: rule-based first).

**API:** `GET /api/v1/maintenance/rul?pod_id=1` → `Vec<ComponentRUL>`

---

### Phase 8: Ollama AI Diagnosis Integration
**Goal:** Enhance Tier 4 with structured maintenance-specific prompts
**Duration:** 2 days | **Consensus:** 11/14

**Extend:** `rc-sentry-ai/src/ai_diagnosis.rs` (NEW)
- Receives anomaly context from server
- Formats structured prompt with telemetry history, recent events, component RUL
- Calls Ollama qwen2.5:3b (or llama3.1:8b for complex cases)
- Parses response into structured `DiagnosisResult`

```rust
pub struct DiagnosisResult {
    pub root_cause: String,
    pub recommended_action: MaintenanceAction,
    pub urgency: Severity,
    pub confidence: f32,
    pub explanation: String,  // Human-readable XAI
    pub similar_past_events: Vec<Uuid>,
}

pub enum MaintenanceAction {
    AutoHeal(String),           // specific tier1/2 action
    ScheduleMaintenance(String), // task description
    OrderPart(String),          // part to order
    NotifyStaff(String),        // escalation message
    MonitorOnly,                // watch but no action
}
```

**Guard function (Claude Sonnet unique insight):**
```rust
fn should_use_ai(anomaly: &AnomalyResult) -> bool {
    // Only invoke expensive AI when rules are ambiguous
    anomaly.confidence < 0.6 || anomaly.severity >= Severity::High
}
```

---

### Phase 9: Maintenance Task/Ticket System
**Goal:** Structured work orders with assignment and tracking
**Duration:** 2 days | **Consensus:** 11/14

**Create:** `racecontrol/src/maintenance_scheduler.rs`
```rust
pub struct MaintenanceTask {
    pub id: Uuid,
    pub title: String,
    pub description: String,
    pub pod_id: Option<u8>,
    pub component: ComponentType,
    pub priority: u8,                     // 1-100, business-impact weighted
    pub status: TaskStatus,
    pub created_at: DateTime<Utc>,
    pub due_by: Option<DateTime<Utc>>,
    pub assigned_to: Option<String>,      // staff_id
    pub source_event_id: Option<Uuid>,    // link to MaintenanceEvent
    pub pre_check_passed: Option<bool>,   // Phase 16
    pub before_metrics: Option<serde_json::Value>, // GPT-5.4 unique insight
    pub after_metrics: Option<serde_json::Value>,  // validate fix worked
    pub cost_estimate_paise: Option<i64>,
    pub actual_cost_paise: Option<i64>,
}

pub enum TaskStatus {
    Open,
    Assigned,
    InProgress,
    PendingValidation,
    Completed,
    Failed,
    Cancelled,
}
```

**API:**
- `POST /api/v1/maintenance/tasks` — Create task
- `GET /api/v1/maintenance/tasks?status=Open&pod_id=1` — Query tasks
- `PATCH /api/v1/maintenance/tasks/{id}` — Update status/assignment
- `POST /api/v1/maintenance/tasks/{id}/validate` — Close with before/after comparison

---

### Phase 10: Business-Aware Priority Scoring
**Goal:** Weight maintenance priorities by revenue impact (GPT-4.1 death spiral warning: use EXPECTED revenue, not historical)
**Duration:** 1 day | **Consensus:** 10/14

**Create:** `racecontrol/src/maintenance_engine.rs` (extend)
```rust
pub fn calculate_priority(event: &MaintenanceEvent, pod_id: u8) -> u8 {
    let base_severity = match event.severity {
        Severity::Critical => 80,
        Severity::High => 60,
        Severity::Medium => 40,
        Severity::Low => 20,
    };

    // Revenue multiplier: EXPECTED per-hour revenue, NOT historical
    // (avoids death spiral where broken pods get deprioritized)
    let hourly_revenue_paise = get_expected_hourly_revenue(pod_id);
    let revenue_factor = if hourly_revenue_paise > 100_000 { 1.3 } // >1000 INR/hr
                         else if hourly_revenue_paise > 50_000 { 1.1 }
                         else { 1.0 };

    // Peak hours multiplier
    let peak_factor = if is_peak_hours() { 1.5 } else { 1.0 };

    // Active session multiplier
    let session_factor = if has_active_session(pod_id) { 1.4 } else { 1.0 };

    let score = (base_severity as f64 * revenue_factor * peak_factor * session_factor).min(100.0);
    score as u8
}
```

---

### Tier 3: Business Intelligence (Phases 11-15) — Week 3-4

---

### Phase 11: Expense Tracking & Revenue Aggregation
**Goal:** Capture all venue costs + aggregate existing billing revenue
**Duration:** 2 days | **Consensus:** 11/14

**Create:** `racecontrol/src/business_analytics.rs`
```rust
pub struct DailyBusinessMetrics {
    pub date: NaiveDate,
    pub revenue_gaming_paise: i64,    // from billing_fsm
    pub revenue_cafe_paise: i64,      // from cafe.rs
    pub revenue_other_paise: i64,     // events, merch
    pub expense_rent_paise: i64,      // manual entry
    pub expense_utilities_paise: i64, // manual entry
    pub expense_salaries_paise: i64,  // from payroll (Phase 17)
    pub expense_maintenance_paise: i64, // from tasks (Phase 9)
    pub expense_other_paise: i64,
    pub sessions_count: u32,
    pub occupancy_rate_pct: f32,      // active_pods/total_pods per hour avg
    pub peak_occupancy_pct: f32,
}
```

**API:**
- `GET /api/v1/analytics/business?start=2026-03-01&end=2026-03-30` → `Vec<DailyBusinessMetrics>`
- `POST /api/v1/analytics/expenses` — Manual expense entry (admin only)

### Phase 12: EBITDA Calculator & Financial Dashboard
**Goal:** Real-time profitability view
**Duration:** 1 day | **Consensus:** 11/14

**Dashboard page:** `/analytics/ebitda` showing daily/weekly/monthly EBITDA, trend line, break-even indicator.

### Phase 13: HR Employee Database
**Goal:** Staff records linked to maintenance assignment and camera attendance
**Duration:** 2 days | **Consensus:** 11/14

**Create:** `racecontrol/src/hr_models.rs`
```rust
pub struct Employee {
    pub id: Uuid,
    pub name: String,
    pub role: StaffRole,
    pub skills: Vec<String>,             // ["hardware", "network", "game_setup"]
    pub hourly_rate_paise: i64,
    pub phone: String,                   // for WhatsApp alerts
    pub is_active: bool,
    pub face_enrollment_id: Option<String>, // links to rc-sentry-ai attendance
    pub hired_at: NaiveDate,
}

pub enum StaffRole {
    Manager,
    Technician,
    FrontDesk,
    GameMaster,
    Cashier,
}
```

**API:** CRUD on `/api/v1/hr/employees`

### Phase 14: Shift & Attendance Tracking
**Goal:** Camera AI attendance linked to HR records
**Duration:** 2 days | **Consensus:** 11/14

**Integration:** `rc-sentry-ai/attendance/` already detects faces. Link detection events to employee face_enrollment_id → compute actual hours worked.

### Phase 15: Tiered Escalation Workflow
**Goal:** 3-tier alert routing (11/14 consensus)
**Duration:** 2 days | **Consensus:** 11/14

```
Tier 1: Auto-fix applied, logged only (rc-sentry tier1, rc-agent self_heal)
Tier 2: Repeated anomaly → notify technician on duty (WhatsApp + dashboard)
Tier 3: Failed fix OR Critical severity → escalate to Uday (WhatsApp + call)
```

**Logic in** `racecontrol/src/maintenance_engine.rs`:
```rust
pub fn escalate(event: &MaintenanceEvent, attempt_count: u32) -> EscalationTier {
    if attempt_count == 0 && event.severity <= Severity::Medium {
        EscalationTier::Auto  // Tier 1: try auto-fix
    } else if attempt_count <= 2 || event.severity == Severity::High {
        EscalationTier::Technician  // Tier 2: notify staff
    } else {
        EscalationTier::Manager  // Tier 3: escalate to Uday
    }
}
```

---

### Tier 4: Advanced Features (Phases 16-21) — Week 5-6

### Phase 16: Pre-Maintenance Automated Checks
Validate system state before starting maintenance (backup exists, no active session, pod idle).

### Phase 17: Payroll & Labor Cost Integration
Link HR hours + rate to business analytics for per-session labor cost.

### Phase 18: Demand Forecasting Engine
Time-series occupancy prediction using 30+ days of historical data. EMA + day-of-week + seasonal factors.

### Phase 19: Dynamic Pricing Foundation
Extend v14.0 psychology pricing with demand-driven adjustments. Admin approval required.

### Phase 20: XAI Explainability Layer
Every AI recommendation includes human-readable explanation. Store in `decision_explanations` table. Show in dashboard.

### Phase 21: Maintenance KPIs
MTTR, MTBF, self-heal rate, prediction accuracy, false positive rate, downtime minutes/week.

---

### Tier 5: Polish & Future (Phases 22-25) — Week 7-8

### Phase 22: User Feedback Loop
Post-maintenance quality survey via rc-agent overlay (Session 1 only — GPT-4.1 insight). Feeds into feedback_loop.rs.

### Phase 23: Predictive Inventory / Spare Parts
RUL data drives part ordering recommendations. Cost lookup table (Hermes insight): GPU fan: 1200, SSD: 5500, RAM stick: 4800.

### Phase 24: Player Downtime Notifications + Staff Gamification
PWA notification: "Pod 5 under optimization — available at 8:30 PM". Staff leaderboard for maintenance tasks completed.

### Phase 25: System Snapshots & Multi-Venue Prep
Config snapshots before patches (extend OTA pipeline). Abstract venue-specific config for future multi-site.

---

## Risk Registry (14-model consensus)

| Risk | Consensus | Mitigation |
|------|-----------|------------|
| False positives / alert fatigue | 12/14 | Debounce (min_sustained_minutes), cooldown, adjustable thresholds |
| AI hallucination | 11/14 | Dual validation (rule + AI), strict JSON parsing, confidence gates |
| Cloud AI budget overrun | 10/14 | Hard daily cap, local Ollama fallback, $5/day/pod limit |
| Database growth | 10/14 | 7d/30d/90d tiered retention, daily cleanup at 03:00 IST |
| Pod performance overhead | 9/14 | Cache static metrics 1hr, batch PS calls, <1% CPU, graceful fallback |
| Revenue death spiral (GPT-4.1) | 1/14 | Use EXPECTED revenue for priority, not historical |
| Schema drift (QwQ-32B) | 1/14 | Weekly golden-dump comparison |

---

## Dependency Graph

```
Phase 1 (Telemetry) ──→ Phase 3 (Warehouse) ──→ Phase 5 (Rules) ──→ Phase 6 (Patterns)
                    ↘                                              ↘
Phase 2 (Events) ──→ Phase 4 (Dashboard) ──→ Phase 9 (Tasks) ──→ Phase 15 (Escalation)
                                                ↗
Phase 7 (RUL) ──→ Phase 8 (Ollama) ──→ Phase 10 (Priority)
                                           ↓
Phase 11 (Expenses) ──→ Phase 12 (EBITDA) ──→ Phase 18 (Forecast) ──→ Phase 19 (Pricing)
                    ↘
Phase 13 (HR) ──→ Phase 14 (Attendance) ──→ Phase 17 (Payroll)
                                              ↓
Phase 16 (Pre-checks) ──→ Phase 20 (XAI) ──→ Phase 21 (KPIs)
                                              ↓
Phase 22 (Feedback) ──→ Phase 23 (Inventory) ──→ Phase 24 (Notifications) ──→ Phase 25 (Snapshots)
```

---

## MMA Model Credits

| Round | Model | Response Size | Key Contribution |
|-------|-------|--------------|-----------------|
| R1 | Claude Opus 4 | 54K | Comprehensive architecture mapping |
| R1 | GPT-5.4 | 56K | Best SQL schema, before/after metrics, feature windows |
| R1 | Gemini 2.5 Pro | 24K | Clean phased architecture, event store design |
| R1 | DeepSeek R1 | 32K | Most detailed phases 1-6, ML algorithm choices |
| R1 | Qwen3-235B | 9K | Dependency graph, TelemetryType taxonomy |
| R2 | Claude Sonnet 4 | 58K | Typed enums, should_use_ml() guard, Arrhenius model |
| R2 | Llama 4 Maverick | 7K | Concise foundation, clean data models |
| R2 | Nemotron Ultra | 6K | RUL scoring, snapshot design |
| R2 | Mistral Large | 13K | Compliance perspective, PII handling |
| R2 | GPT-4.1 | 21K | Revenue death spiral warning, Session 0 feedback constraint |
| R3 | Gemini 2.5 Flash | 59K | Most detailed Windows implementation, sub-phases |
| R3 | QwQ-32B | 9K | Schema audit, drift prevention |
| R3 | Phi-4 | 8K | Compact validation |
| R3 | Hermes 4 405B | 19K | Cost lookup tables, correlation targets |
