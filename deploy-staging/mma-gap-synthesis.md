# MMA Gap Synthesis — Racing Point eSports v29.0 Meshed Intelligence
**Date:** 2026-03-30 IST
**Models:** GPT-5.4 (42K), Gemini Flash (23K), DeepSeek V3 (16K), MiMo Pro (12K), Claude Sonnet (8K)
**Method:** 3+ model consensus required for inclusion; single-model findings included only where clearly critical.

---

## CRITICAL — Blocks Core Functionality

### C1. Feedback Loop Engine: Stubbed / Non-Functional
**Models confirming: 5/5** (GPT-5.4, Gemini, DeepSeek, MiMo, Claude)

`feedback_loop.rs` is mentioned in architecture diagrams and created as a file in Phase 22, but has **zero internal logic**. No mechanism exists to feed maintenance outcomes back into predictive models, track prediction accuracy, or trigger retraining.

**Broken connections:**
- `maintenance_engine.rs` → `feedback_loop.rs` (task outcomes never flow back)
- `hr_kpi` → `feedback_loop.rs` (staff efficiency never measured)
- `feedback_loop.rs` → `predictive_ai.rs` (no retraining pipeline)
- `PRICING_AI` → `feedback_loop.rs` (pricing effectiveness never audited)

**Fix steps:**
1. Define `FeedbackEvent` struct: outcome type, prediction_id, actual result, accuracy delta
2. Wire task completion in `maintenance_scheduler.rs` to emit `FeedbackEvent`
3. Add nightly retraining trigger: if `feedback_data.count() > 100`, retrain Ollama models + update rule thresholds
4. Track `prediction_precision`, `prediction_recall`, `mean_lead_time_hours` in `maintenance_kpi.rs`
5. Wire pricing experiments: baseline demand → applied price → conversion delta → feed to `business_forecast.rs`

---

### C2. Optimization Engine: Missing Entirely
**Models confirming: 5/5** (GPT-5.4, Gemini, DeepSeek, MiMo, Claude)

The vision's `OPTIMIZATION_AI` (staffing · pricing · resource load balance) does not exist as a unified component. `dynamic_pricing.rs` (Phase 19) and priority scoring (Phase 10) are isolated; no engine balances staffing costs, pod utilization, maintenance schedules, and pricing together.

**Missing modules:** `optimization_engine.rs` — not in any phase plan.

**Fix steps:**
1. Create `racecontrol/src/optimization_engine.rs`
2. Implement objectives: `MaximizeRevenue`, `MinimizeDowntime`, `BalanceStaffWorkload`, `ExtendEquipmentLife`
3. Functions: `recommend_staffing_levels(date_range)`, `recommend_task_assignment(task_id)`, `recommend_maintenance_window(pod_id)`, `recommend_pod_rotation(active_sessions)`
4. Inputs: occupancy forecast, employee availability/skills/overtime cost, expected revenue by hour, maintenance backlog
5. Output recommendations to dashboard with XAI explanation text
6. Wire into `feedback_loop.rs` for outcome tracking

---

### C3. Self-Healing Engine: Detection Exists, Remediation Does Not
**Models confirming: 5/5** (GPT-5.4, Gemini, DeepSeek, MiMo, Claude)

Anomaly detection (Phase 5) and `pod_healer.rs` exist, but **no orchestration layer** connects predictive risk → recovery action → validation → outcome logging. The system can detect but cannot heal autonomously.

**Missing:**
- Decision engine mapping anomaly types to recovery actions
- Recovery validation (compare before/after FPS, crash count, temps)
- Escalation if validation fails
- "Reroute" logic: mark degraded pod unavailable, notify kiosk/PWA/POS not to book it

**Fix steps:**
1. Create `racecontrol/src/self_healing_engine.rs` with `HealingAction` enum: `PodRestart(u8)`, `GameProcessRestart(u8, String)`, `NetworkReroute(u8)`, `LoadRedistribute(Vec<u8>)`, `ServiceReset(u8, String)`
2. Add `execute_recovery_action()` in `maintenance_engine.rs` that dispatches to agent self-heal / sentry tier1 / pod_healer
3. After action, schedule validation: compare telemetry before/after, persist `RecoveryValidationResult`
4. If validation fails, auto-escalate via `whatsapp_alerter.rs`
5. Add pod state transitions: `Available → Degraded → MaintenanceHold → Unavailable`; expose as `GET /api/v1/pods/{id}/sellability` consumed by kiosk, PWA, and POS

---

### C4. Dynamic Pricing Not Connected to Live Selling Channels
**Models confirming: 4/5** (GPT-5.4, Gemini, DeepSeek, Claude — MiMo flagged HIGH)

`dynamic_pricing.rs` computes prices but there is **no write-through path** into the POS, kiosk, PWA, or cloud sync. Price changes computed by the AI are never applied to the channels where customers actually pay.

**Fix steps:**
1. Add canonical pricing table with versioning + effective windows
2. Dashboard approval workflow: `POST /api/v1/pricing/proposals/{id}/approve`
3. On approval, push to: server local billing, POS agent, kiosk app, cloud PWA via `cloud_sync.rs`
4. Add rollback capability (revert to previous version)
5. Track each pricing experiment: baseline demand → applied price → conversion delta → revenue delta → feed into `business_forecast.rs`

---

### C5. Core Telemetry Pipeline Broken: Pods Collect, Server Never Receives
**Models confirming: 3/5** (Claude, GPT-5.4, DeepSeek — unique emphasis by Claude Sonnet)

`ExtendedTelemetry` struct exists in `rc-agent/src/predictive_maintenance.rs` but **no WebSocket emission to server**. The server has no endpoint to receive the new metrics. `telemetry_store.rs` extension is planned but not implemented. The entire AI system has no data to work with.

**Fix steps:**
1. Implement `ExtendedTelemetry::emit_to_server()` via existing WS connection in rc-agent
2. Add server endpoint: `POST /api/v1/telemetry/extended`
3. Implement `telemetry_store::store_extended_telemetry()` with SQLite persistence
4. Wire stored telemetry as input to `maintenance_engine.rs` anomaly rules

---

### C6. Business Analytics Data Sources Not Connected to Real Revenue
**Models confirming: 4/5** (Claude, GPT-5.4, Gemini, MiMo)

`DailyBusinessMetrics` and the EBITDA calculator exist as shells. No integration code connects them to `billing_fsm.rs` (gaming revenue), `cafe.rs` (cafe revenue), or `wallet.rs` (top-up revenue). EBITDA calculations run on empty data.

**Fix steps:**
1. Implement `aggregate_daily_revenue(date)`: query `billing_fsm`, `cafe`, `wallet` tables
2. Add recurring expense templates for fixed costs (rent, utilities, subscriptions)
3. Auto-post maintenance actual cost from completed tasks to `business_analytics.rs`
4. Auto-post payroll period totals when payroll is calculated
5. Add utility anomaly thresholds + ingestion path

---

## HIGH — Degrades Value Significantly

### H1. HR System is an Island: Not Wired to Maintenance Scheduler
**Models confirming: 5/5** (GPT-5.4, Gemini, DeepSeek, MiMo, Claude)

`hr_models.rs` (Phase 13) and `maintenance_scheduler.rs` (Phase 9) exist but are completely disconnected. Maintenance tasks have an `assigned_to` field with no assignment logic. The scheduler has no concept of staff skills, shift availability, overtime limits, or labor cost.

**Fix steps:**
1. Add `hr_store::find_available_staff(component, priority, due_by)` — checks skills, current shift, existing task load
2. Implement `maintenance_scheduler::assign_task_automatically(task)` — queries HR, assigns, sends WhatsApp notification to technician
3. If no available technician, auto-escalate to manager via `whatsapp_alerter.rs`
4. Add supervisor approval endpoint: `POST /api/v1/hr/approvals/maintenance-task`
5. Wire approved tasks back into staff schedules (write back)

---

### H2. Alert Engine Missing: Business Deviations Never Trigger Alerts
**Models confirming: 5/5** (GPT-5.4, Gemini, DeepSeek, MiMo, Claude)

`whatsapp_alerter.rs` exists for critical hardware failures, but **no `alert_engine.rs`** exists to monitor business KPI deviations. Payroll creep, occupancy drops, and utility spikes go unnoticed until manual review.

**Fix steps:**
1. Create `racecontrol/src/alert_engine.rs` with `AlertChannel` enum: `WhatsApp`, `Dashboard`, `SMS`
2. Define deviation rules: payroll > 35% revenue, occupancy < 40%, utility cost spike, occupancy above safe staffing threshold
3. Connect `business_analytics.rs` → `alert_engine.rs` → `whatsapp_alerter.rs`
4. Add tiered escalation matrix: Auto (log only) → Technician notify → Manager escalate → Critical (full escalation)
5. Add alert configuration page to admin dashboard

---

### H3. No Unified Data Collector / Aggregation Layer
**Models confirming: 4/5** (GPT-5.4, Gemini, MiMo, DeepSeek)

The vision's `DATA_COLLECTOR` aggregates game telemetry, HR labor cost, and business cost/revenue into one normalized stream for AI consumption. In v29.0, each module is a silo. AI models cannot correlate staff performance with pod failures, or revenue with maintenance costs.

**Fix steps:**
1. Create `racecontrol/src/data_collector.rs` with: `collect_telemetry_snapshot()`, `collect_business_snapshot()`, `collect_hr_snapshot()`, `collect_operational_snapshot()`
2. Define `VenueSnapshot` and `PodSnapshot` structs as the canonical cross-domain representation
3. Feed snapshots into `maintenance_engine.rs`, `business_forecast.rs`, `optimization_engine.rs`, `feedback_loop.rs`

---

### H4. Cloud Sync Not Extended for Maintenance/HR/Analytics Data
**Models confirming: 4/5** (GPT-5.4, Gemini, DeepSeek, MiMo)

`cloud_sync.rs` only syncs billing/drivers/pricing. Maintenance events, KPIs, HR data, and recovery outcomes are never synced to the cloud. The `SELF_HEALING → CLOUD_SYNC → HISTORY_LOG` vision path is entirely broken.

**Fix steps:**
1. Add sync payload types: `MaintenanceEventSync`, `RecoveryOutcomeSync`, `DowntimeIncidentSync`
2. Trigger cloud sync on: self-heal complete, task completion, emergency shutdown
3. Extend with maintenance ROI aggregation and downtime trend data
4. Build cloud dashboard report endpoints: ROI by maintenance type, downtime trend by pod/component

---

### H5. AI Diagnosis (Ollama Integration) Not Implemented
**Models confirming: 3/5** (Claude, GPT-5.4, DeepSeek)

`ai_diagnosis.rs` is mentioned in Phase 8 as Ollama integration but has no implementation: no structured prompts, no Ollama HTTP client, no fallback chain (`qwen2.5:3b → llama3.1:8b → OpenRouter`), no confidence threshold guard.

**Fix steps:**
1. Implement `rc-sentry-ai/src/ai_diagnosis.rs` with Ollama HTTP client (`POST http://192.168.31.27:11434/api/generate`)
2. Define structured prompt templates per anomaly category
3. Implement fallback chain with confidence threshold (`< 0.6` = escalate to next tier)
4. Parse structured JSON response into `DiagnosisResult` with `recommended_action` + `confidence_score`

---

### H6. Anomaly Detection Rules Defined but Never Executed
**Models confirming: 3/5** (Claude, GPT-5.4, DeepSeek)

`AnomalyRule` evaluation logic exists but **no tokio scheduler runs it**. No cron job, no cooldown state management, no statistical baseline calculation. Rules are defined in code but never evaluated.

**Fix steps:**
1. Add `tokio::spawn` periodic task in `maintenance_engine.rs` to evaluate rules every N seconds
2. Implement cooldown state (HashMap<rule_id, last_fired>) to prevent alert spam
3. Implement statistical baseline calculation using rolling window of last 7 days
4. Wire anomaly firing → `self_healing_engine.rs` for immediate risk; → `maintenance_scheduler.rs` for scheduled risk

---

### H7. Dashboard: Components Exist, Data Bindings Don't
**Models confirming: 4/5** (Claude, Gemini, MiMo, GPT-5.4)

`MaintenanceTimeline.tsx` and `ComponentHealthGauge.tsx` exist but have no real-time data sources. Business analytics dashboard has no data endpoints wired. No WebSocket connection for live maintenance event updates.

**Fix steps:**
1. Add WebSocket channel for maintenance events pushed from `maintenance_engine.rs`
2. Wire `ComponentHealthGauge.tsx` to RUL data from `maintenance_engine.rs`
3. Connect business analytics dashboard components to real aggregated data from `business_analytics.rs`
4. Add `/admin/overview` unified view: active incidents, occupancy, forecast, staffing, EBITDA, pending approvals

---

### H8. Maintenance Scheduler Not Business-Aware (No Revenue-Impact Weighting)
**Models confirming: 4/5** (GPT-5.4, Gemini, DeepSeek, MiMo)

Maintenance tasks are prioritized with static severity scores, not real business impact. A task on a pod during a peak booking window is treated the same as one during off-hours. `business_analytics.rs` data never reaches `maintenance_scheduler.rs`.

**Fix steps:**
1. Add `calculate_business_impact(pod_id, downtime_minutes, time_of_day)` in `maintenance_engine.rs`: queries expected hourly revenue, is_peak_hours, open task count
2. Include `revenue_loss_paise` estimate and `escalation_urgency` multiplier in priority score
3. Add scheduling optimizer: choose time windows minimizing revenue loss (integrate `occupancy_analytics.rs` forecasts)

---

### H9. Admin Override Has No Feedback Path to Models
**Models confirming: 3/5** (GPT-5.4, Gemini, DeepSeek)

Vision explicitly requires dashboard overrides to flow back into MESHSYS and affect future recommendations. No `AdminOverride` model, no API, no feedback path exists.

**Fix steps:**
1. Add `AdminOverride` table: `recommendation_id`, `overridden_by`, `original_action`, `final_action`, `reason`
2. `POST /api/v1/overrides` endpoint
3. In `feedback_loop.rs`: weight override data into threshold tuning, recommendation confidence penalties, false-positive tagging

---

## MEDIUM — Important Features Missing

### M1. XAI Layer: Code Stub, Nothing Displayed to Operators
**Models confirming: 4/5** (GPT-5.4, Gemini, DeepSeek, MiMo)

Phase 20 creates `maintenance_xai.rs` but defines no explanation templates, no structured schema, no confidence scoring methodology, and the dashboard never surfaces explanations. Operators cannot understand or trust AI decisions.

**Fix steps:**
1. Define explanation templates per decision type (anomaly detected, task assigned, price changed)
2. Store human-readable explanation with every `MaintenanceEvent`
3. Surface in dashboard as tooltip/detail panel with confidence score and similar past incidents

---

### M2. Occupancy Analytics Not Connected to Maintenance or Pricing
**Models confirming: 4/5** (GPT-5.4, Gemini, MiMo, Claude)

`occupancy_analytics.rs` (Phase 11) exists but feeds nowhere. Occupancy data is not driving maintenance window selection, pricing adjustments, or staffing recommendations.

**Fix steps:**
1. Emit occupancy events from `billing_fsm.rs` session lifecycle: session_started, paused, ended, transferred
2. Aggregate by pod/hour/day in `occupancy_analytics.rs`
3. Expose `GET /api/v1/occupancy/forecast` consumed by `maintenance_scheduler.rs` and `dynamic_pricing.rs`
4. Use as source of truth for demand forecasting

---

### M3. Maintenance Hold Not Reflected in Customer-Facing Channels
**Models confirming: 3/5** (GPT-5.4, Gemini, DeepSeek)

When a pod is under maintenance or predicted to fail, kiosk/PWA/POS still offer it for booking. Customers can book a pod that is about to go offline.

**Fix steps:**
1. Add pod sellability API: `GET /api/v1/pods/{id}/availability` returning `{state: Available|Degraded|Hold, hold_until, reason}`
2. Kiosk, PWA, and POS consume this before rendering booking options
3. `maintenance_scheduler.rs` writes hold windows when tasks are scheduled

---

### M4. Forecast Engine Not Fed by Real Booking/Session Data
**Models confirming: 3/5** (GPT-5.4, Gemini, DeepSeek)

`business_forecast.rs` (Phase 18) performs time-series forecasting but has no integration with PWA bookings, POS walk-ins, event slots, pod availability windows, or pricing history. Forecasts are based on no real inputs.

**Fix steps:**
1. Define forecast input dataset: booked slots, walk-in conversions, no-shows, pod outage windows, promotions, pricing history
2. Add data connectors from `billing_fsm.rs` session events and PWA booking backend
3. Include maintenance hold windows in demand capacity (available pods × occupancy rate)

---

### M5. Payroll Not Connected to Business Analytics
**Models confirming: 3/5** (GPT-5.4, Gemini, Claude)

`payroll.rs` (Phase 17) calculates labor costs but never feeds them to `business_analytics.rs`. Labor is the largest operating expense and is absent from EBITDA calculations.

**Fix steps:**
1. Auto-post payroll period totals to `DailyBusinessMetrics.expense_salaries_paise`
2. Include maintenance labor cost (hours × rate from HR) in `expense_maintenance_paise`
3. Create `hr_kpi.rs`: cost/session, task completion SLA, technician utilization, overtime ratio

---

### M6. RUL Estimates Not Triggering Scheduling or Inventory Actions
**Models confirming: 3/5** (GPT-5.4, Gemini, MiMo)

Remaining Useful Life calculations exist (Phase 7) but are not wired to any downstream action. No maintenance task is auto-created when RUL falls below threshold. No inventory/parts lookup exists. Dashboard surface "replace before Friday peak" style advisories are absent.

**Fix steps:**
1. If `rul_hours < threshold`, auto-create `MaintenanceTask` in `maintenance_scheduler.rs`
2. Feed expected downtime/cost into `business_analytics.rs` as forward-looking expense
3. Add component model/serial number tracking for spare parts lookup
4. Surface RUL in dashboard with business-aware advisory ("replace this week to avoid weekend peak impact")

---

### M7. Dynamic Pricing Not Integrated with Psychology Module
**Models confirming: 2/5** (Claude, GPT-5.4 — noted as important single-model finding)

`dynamic_pricing.rs` (Phase 19) ignores the existing `psychology.rs` module from v14.0. No admin approval workflow for price changes. Demand forecasting has no connection to actual session booking data.

**Fix steps:**
1. Import `psychology.rs` pricing multipliers into `dynamic_pricing.rs` demand-driven adjustments
2. Add admin approval flow before any price change takes effect

---

### M8. Multi-Venue Abstraction Absent Throughout Codebase
**Models confirming: 3/5** (GPT-5.4, Gemini, Claude)

All code hardcodes single-venue assumptions. No `venue_id` concept in DB schema. Cloud sync assumes single venue. Blocks any future expansion.

**Fix steps:**
1. Add `venue_id` to core DB tables (maintenance_events, business_metrics, hr_records)
2. Abstract venue-specific config (pod IPs, camera config, pricing) behind a venue config layer
3. Cloud sync protocol should carry `venue_id` in all payloads

---

## Summary: Data Flow Breaks (Most Critical Paths)

| Broken Flow | Root Cause | Priority Fix |
|-------------|-----------|-------------|
| Telemetry → AI → Action | No WS emission from rc-agent, no server endpoint | C5 above |
| Business data → EBITDA | `billing_fsm`/`cafe`/`wallet` not queried | C6 above |
| Prediction → Self-Heal | No orchestration layer, no recovery dispatch | C3 above |
| Maintenance outcome → Learning | `feedback_loop.rs` is empty | C1 above |
| HR availability → Task assignment | `maintenance_scheduler` never calls `hr_store` | H1 above |
| Occupancy → Maintenance window | Occupancy feeds nowhere | M2 above |
| RUL → Scheduled task | No threshold trigger implemented | M6 above |
| Pricing decision → Live POS/kiosk | No write-through to selling channels | C4 above |

---

## Implementation Priority Order (Consensus Recommendation)

1. **Telemetry pipeline** (C5) — zero AI intelligence without data
2. **Business analytics data sources** (C6) — EBITDA is meaningless without real numbers
3. **Self-healing orchestration** (C3) — prevents alert fatigue with no resolution
4. **Alert engine** (H2) — financial anomalies invisible without it
5. **HR → Maintenance integration** (H1) — tasks cannot be assigned
6. **Feedback loop** (C1) — system cannot improve without it
7. **Optimization engine** (C2) — delivers the "meshed intelligence" value
8. **Dynamic pricing write-through** (C4) — AI pricing is decorative without it

The foundation (modules, DB schema, rule definitions) is ~60% complete. The system is ~25% functional because the connective tissue — data flows, orchestration, write-back paths — is almost entirely missing. Estimated 10 additional integration-focused phases needed to achieve the v29.0 vision.
