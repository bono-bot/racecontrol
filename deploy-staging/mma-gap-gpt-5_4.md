Below is a ruthless gap analysis of **vision vs. v29.0 plan/built scope** across the full Racing Point eSports ecosystem.

I’m treating **“what was built” as the 25-phase implementation plan and module map**, not assuming all 25 phases are fully completed in code. So gaps include:
- things absent from the v29 plan,
- things mentioned but obviously placeholder/stub-level,
- things planned as modules but not fully wired across the actual ecosystem,
- end-to-end flow breaks versus the Mermaid vision.

---

# Executive Summary

## Biggest gaps

### 1. **No true unified meshed control plane**
**Severity: CRITICAL**  
The vision shows a **single meshed intelligence core** where game telemetry, HR, business, optimization, scheduler, self-healing, feedback, alerts, and admin overrides all loop together.  
v29.0 creates many modules, but it is still mostly a **set of adjacent subsystems**, not a real closed-loop control plane.

Missing:
- shared decision bus / orchestration layer,
- canonical event stream linking telemetry → anomaly → diagnosis → task → approval → action → validation → learning,
- admin override propagation back into models and operating policies.

---

### 2. **Self-healing loop is only partially represented**
**Severity: CRITICAL**  
Vision requires:
- predictive AI detects immediate risk,
- self-healing engine acts,
- game system is restarted/rebalanced/validated,
- post-recovery logs sync to cloud,
- outcomes feed back into learning.

v29 has:
- `self_heal`, `pod_healer`, `maintenance_gate`, `tier1_fixes`, `maintenance_engine`,
but the plan does **not define a full orchestrated remediation pipeline** with validation and outcome scoring end to end.

---

### 3. **HR/payroll/business are not fully connected into maintenance decisions**
**Severity: CRITICAL**  
Vision explicitly requires:
- staff DB + attendance + payroll + HR KPI
- labor cost enters collector
- scheduler auto-assigns tasks into HR
- optimization updates staffing
- payroll/utilities/occupancy deviations trigger alerting.

v29 includes HR modules and payroll phases, but the wiring is incomplete:
- labor cost is not clearly fed into predictive or optimization engines,
- maintenance scheduler is not actually using skills/availability/overtime/cost in assignment logic,
- HR KPI does not materially flow into `feedback_loop.rs`,
- alert engine for payroll/utilities/occupancy deviations is not concretely implemented.

---

### 4. **Business analytics is not deeply integrated with live operations**
**Severity: HIGH**  
Vision expects:
- revenue + expense feed into forecast,
- forecast into pricing AI,
- pricing AI into dashboard for approval,
- dashboard decisions back into business systems,
- occupancy/utilities/payroll anomaly alerts.

v29 covers expenses, EBITDA, forecast, dynamic pricing, but not the **full control loop**:
- no strong link to `billing_fsm`, `wallet`, `cafe`, PWA bookings, kiosk demand, actual pod occupancy by slot,
- no explicit write-back path into live pricing tables used by POS/PWA/cloud sync,
- no closed-loop effectiveness audit beyond a generic feedback mention.

---

### 5. **Cloud/admin/history path is incomplete**
**Severity: HIGH**  
Vision requires cloud sync of:
- logs,
- KPIs,
- schedules,
- feedback,
- historical analytics / maintenance ROI / downtime trends,
- dashboard unified view,
- WhatsApp multi-channel.

v29 mentions `cloud_sync` and history, but missing:
- exact maintenance/HR/business payload sync contracts,
- dashboard-level historical ROI views,
- cloud-side analytics consolidation,
- cross-device/admin override consistency.

---

# Gap Register

---

# A. Meshed Intelligence Core Gaps

## A1. No canonical “Meshed Intelligence” orchestration layer
**Severity: CRITICAL**

### What’s missing
The vision’s `MESHSYS` acts as one coordinated brain:
- collector,
- predictive AI,
- self-healing,
- optimization,
- scheduler,
- feedback loop.

v29 splits these into files:
- `maintenance_engine.rs`
- `maintenance_scheduler.rs`
- `feedback_loop.rs`
- `business_forecast.rs`
- `dynamic_pricing.rs`
- `maintenance_kpi.rs`

But there is no **single orchestrator/state machine** coordinating:
- event ingestion,
- scoring,
- decisioning,
- action dispatch,
- approval,
- verification,
- learning.

### Modules needing connection
- `telemetry_store.rs`
- `maintenance_engine.rs`
- `maintenance_scheduler.rs`
- `feedback_loop.rs`
- `business_forecast.rs`
- `dynamic_pricing.rs`
- `pod_healer.rs`
- `whatsapp_alerter.rs`
- admin dashboard APIs

### Implementation steps
1. Create `racecontrol/src/mesh_orchestrator.rs`
   - `handle_telemetry_tick()`
   - `handle_maintenance_event()`
   - `handle_business_update()`
   - `handle_hr_update()`
   - `apply_admin_override()`
2. Add canonical decision objects:
   - `DecisionContext`
   - `RecommendedAction`
   - `DecisionOutcome`
3. Wire from:
   - telemetry ingestion,
   - billing/session updates,
   - attendance/payroll updates,
   - dashboard actions.
4. Persist orchestration trace:
   - input facts,
   - recommendation,
   - approval,
   - action executed,
   - validation result.

---

## A2. Data collector is not actually unified
**Severity: HIGH**

### What’s missing
Vision has one `DATA_COLLECTOR` aggregating:
- game telemetry,
- logs,
- performance metrics,
- HR labor cost/staffing data,
- business cost/revenue/utility data.

v29 has telemetry and separate business/HR modules, but not a unified normalized collector.

### Modules needing connection
- `telemetry_store.rs`
- `business_analytics.rs`
- `hr_store.rs`
- `maintenance_store.rs`
- `feedback_loop.rs`

### Implementation steps
1. Add normalized ingest layer in `racecontrol/src/data_collector.rs`
   - `collect_telemetry_snapshot()`
   - `collect_business_snapshot()`
   - `collect_hr_snapshot()`
   - `collect_operational_snapshot()`
2. Define common structs:
   - `VenueSnapshot`
   - `PodSnapshot`
3. Feed these into:
   - `maintenance_engine`
   - `business_forecast`
   - `optimization_ai`
   - `feedback_loop`

---

## A3. No optimization engine equivalent to vision’s staffing/pricing/load-balance optimizer
**Severity: HIGH**

### What’s missing
Vision includes `OPTIMIZATION_AI` for:
- staffing,
- pricing,
- resource load balancing.

v29 has:
- `dynamic_pricing.rs`
- some maintenance priority logic,
but no true optimizer for:
- staffing levels,
- task allocation,
- pod balancing,
- scheduling by occupancy and labor cost.

### Modules needing connection
- `dynamic_pricing.rs`
- `maintenance_scheduler.rs`
- `hr_store.rs`
- `occupancy_analytics.rs`
- `business_forecast.rs`

### Implementation steps
1. Create/complete `racecontrol/src/optimization_engine.rs`
2. Functions:
   - `recommend_staffing_levels(date_range)`
   - `recommend_task_assignment(task_id)`
   - `recommend_pod_rotation(active_sessions)`
   - `recommend_maintenance_window(pod_id)`
3. Inputs:
   - occupancy forecast,
   - employee availability/skills/overtime,
   - expected revenue by hour,
   - maintenance backlog.
4. Output recommendations to dashboard with XAI text.

---

## A4. Admin override does not flow back into model policy
**Severity: HIGH**

### What’s missing
Vision explicitly says dashboard can override AI decisions and feed back into MESHSYS.  
v29 mentions XAI and dashboard approval, but no concrete override-learning mechanism.

### Modules needing connection
- dashboard `/maintenance`, `/analytics/business`
- `feedback_loop.rs`
- `maintenance_engine.rs`
- `dynamic_pricing.rs`
- `business_forecast.rs`

### Implementation steps
1. Add `AdminOverride` model/table:
   - recommendation_id,
   - overridden_by,
   - original_action,
   - final_action,
   - reason.
2. APIs:
   - `POST /api/v1/overrides`
3. In `feedback_loop.rs`, weight override data into:
   - threshold tuning,
   - recommendation confidence penalties,
   - false-positive tagging.

---

# B. Self-Healing and Recovery Gaps

## B1. Predictive → self-healing → validation loop incomplete
**Severity: CRITICAL**

### What’s missing
Vision requires:
- immediate failure risk triggers self-healing,
- restart/rebalance/validate back into game system.

v29 has pieces, but no explicit chain:
- predictive anomaly detects immediate risk,
- chooses recovery action,
- invokes pod agent/sentry/server-side healer,
- validates telemetry recovery,
- logs result and feeds back.

### Modules needing connection
- `maintenance_engine.rs`
- `pod_healer.rs`
- `rc-agent/self_heal.rs`
- `rc-sentry/maintenance_gate.rs`
- `maintenance_store.rs`
- `feedback_loop.rs`

### Implementation steps
1. Add `execute_recovery_action()` in `maintenance_engine.rs`
2. Dispatch by action type:
   - agent self-heal,
   - sentry tier1 fix,
   - pod_healer intervention,
   - emergency drain/disable pod.
3. After action, schedule validation:
   - compare before/after FPS, crash count, temp, process handles.
4. Persist `RecoveryValidationResult` in maintenance event/task.
5. If validation fails, escalate automatically.

---

## B2. No resource reroute/rebalance concept
**Severity: HIGH**

### What’s missing
Vision says self-healing can “reroute / rebalance / recovery”.
For an 8-pod venue, this means:
- shifting bookings/sessions to another pod,
- disabling a degraded pod from customer-facing availability,
- adjusting occupancy/load.

v29 does not define live operational rerouting.

### Modules needing connection
- `billing_fsm.rs`
- kiosk availability
- PWA booking availability
- `occupancy_analytics.rs`
- `pod_healer.rs`
- dashboard pod control

### Implementation steps
1. Add pod state transitions:
   - `Available`, `Degraded`, `MaintenanceHold`, `Unavailable`
2. If anomaly/high failure risk:
   - mark pod unavailable for new sessions,
   - notify billing/POS/kiosk/PWA inventory.
3. Add session reassignment flow where possible.
4. Sync state to cloud and kiosk.

---

## B3. No post-recovery analytics pipeline to cloud history
**Severity: HIGH**

### What’s missing
Vision: `SELF_HEALING -> CLOUD_SYNC -> HISTORY_LOG`.  
v29 mentions cloud sync generally, but not explicit post-recovery payloads.

### Modules needing connection
- `pod_healer.rs`
- `maintenance_store.rs`
- `cloud_sync.rs`
- cloud racecontrol
- admin historical views

### Implementation steps
1. Add sync payload types:
   - `MaintenanceEventSync`
   - `RecoveryOutcomeSync`
   - `DowntimeIncidentSync`
2. Trigger cloud sync on:
   - self-heal complete,
   - task completion,
   - emergency shutdown.
3. Build cloud dashboard report endpoints:
   - ROI by maintenance type,
   - downtime trend by pod/component.

---

# C. Maintenance Intelligence Gaps

## C1. No closed-loop failure prediction accuracy measurement
**Severity: HIGH**

### What’s missing
Vision’s feedback loop requires learning from actual outcomes.  
v29 includes KPIs later, but no strict linkage from prediction to actual failure/non-failure.

### Modules needing connection
- `maintenance_engine.rs`
- `maintenance_store.rs`
- `maintenance_kpi.rs`
- `feedback_loop.rs`

### Implementation steps
1. Add `prediction_id` and `predicted_failure_window_hours`
2. On actual event occurrence/non-occurrence, reconcile:
   - true positive,
   - false positive,
   - false negative,
   - lead time.
3. KPI APIs:
   - `prediction_precision`
   - `prediction_recall`
   - `mean_lead_time_hours`

---

## C2. Failure pattern correlation is batch-only and not operationalized
**Severity: HIGH**

### What’s missing
Phase 6 computes correlations, but the vision needs active pattern use in prediction/scheduling.

### Modules needing connection
- `maintenance_engine.rs`
- dashboard pattern approval UI
- `feedback_loop.rs`

### Implementation steps
1. Add active pattern registry table.
2. After admin approves a pattern, make it part of live anomaly scoring.
3. Track outcome quality per pattern and auto-demote weak ones.

---

## C3. RUL estimates not connected to scheduling, inventory, or pricing of downtime
**Severity: HIGH**

### What’s missing
RUL exists conceptually, but vision expects actionable use:
- scheduler,
- feedback,
- history,
- ROI,
- business-aware planning.

### Modules needing connection
- `maintenance_engine.rs`
- `maintenance_scheduler.rs`
- `business_analytics.rs`
- `feedback_loop.rs`
- future inventory

### Implementation steps
1. If `rul_hours < threshold`, auto-create maintenance task.
2. Feed expected downtime/cost into business analytics.
3. Surface in dashboard:
   - “replace before Friday peak”.
4. Include RUL in maintenance prioritization score.

---

## C4. Preventive scheduler not truly intelligent yet
**Severity: HIGH**

### What’s missing
Vision scheduler includes:
- pre-check,
- task assignment,
- priority planning,
- HR approval/availability loop.

v29 scheduler is mostly task CRUD plus some pre-check concepts.

### Modules needing connection
- `maintenance_scheduler.rs`
- `hr_store.rs`
- `attendance`
- `payroll.rs`
- `occupancy_analytics.rs`
- `business_forecast.rs`

### Implementation steps
1. Add assignment algorithm using:
   - skills match,
   - on-shift status,
   - overtime limits,
   - labor cost,
   - expected occupancy impact.
2. Add schedule optimizer:
   - choose time windows minimizing revenue loss.
3. Dashboard approve/adjust workflow.
4. Write approved tasks back into staff schedules.

---

## C5. No maintenance ROI reporting
**Severity: MEDIUM**

### What’s missing
Vision history log specifically includes maintenance ROI and downtime trends.

### Modules needing connection
- `maintenance_store.rs`
- `maintenance_scheduler.rs`
- `business_analytics.rs`
- `cloud_sync.rs`

### Implementation steps
1. Compute per task/event:
   - downtime avoided,
   - cost incurred,
   - revenue preserved.
2. Add `/api/v1/analytics/maintenance-roi`
3. Show by pod, component, month.

---

# D. HR Module Gaps

## D1. HR attendance is not fully merged with maintenance scheduling
**Severity: CRITICAL**

### What’s missing
Vision: scheduler auto-assigns to HR, HR availability/supervisor approval returns to dashboard.  
v29 has employee DB and attendance, but not real assignment integration.

### Modules needing connection
- `maintenance_scheduler.rs`
- `hr_store.rs`
- `rc-sentry-ai/attendance/`
- dashboard HR pages

### Implementation steps
1. Add employee availability query:
   - current shift,
   - break state,
   - overtime cap,
   - skill fit.
2. `maintenance_scheduler.assign_best_staff(task_id)`
3. Supervisor approval endpoint:
   - `POST /api/v1/hr/approvals/maintenance-task`

---

## D2. Payroll exists conceptually but no incentive/bonus engine
**Severity: MEDIUM**

### What’s missing
Vision explicitly includes “Payroll & Incentives”.  
v29 Phase 17 is payroll and labor cost integration only.

### Modules needing connection
- `payroll.rs`
- `maintenance_kpi.rs`
- task completion metrics
- dashboard HR/payroll

### Implementation steps
1. Add incentive models:
   - SLA completion bonus,
   - uptime bonus,
   - on-call bonus.
2. Pull from completed maintenance tasks and attendance quality.
3. Expose in payroll UI.

---

## D3. HR KPI module underdefined and not wired into feedback loop
**Severity: HIGH**

### What’s missing
Vision: HR KPIs → feedback loop.  
v29 has no strong HR KPI engine.

### Modules needing connection
- `hr_store.rs`
- `payroll.rs`
- `maintenance_scheduler.rs`
- `maintenance_kpi.rs`
- `feedback_loop.rs`

### Implementation steps
1. Create `racecontrol/src/hr_kpi.rs`
   - cost/session,
   - task completion SLA,
   - technician utilization,
   - overtime ratio,
   - absenteeism.
2. Feed KPIs into `feedback_loop.rs`
3. Use in staffing optimization.

---

## D4. No labor-cost-aware scheduling
**Severity: HIGH**

### What’s missing
Vision wants optimization across staffing and business.  
v29 links payroll to analytics, but scheduling does not clearly optimize on labor cost.

### Modules needing connection
- `maintenance_scheduler.rs`
- `payroll.rs`
- `business_forecast.rs`
- `occupancy_analytics.rs`

### Implementation steps
1. Add cost-aware score:
   - skill fit,
   - available now,
   - labor cost,
   - overtime risk,
   - revenue risk if delayed.
2. Use score in assignment recommendations.

---

# E. Business Analytics Module Gaps

## E1. Revenue data ingestion is incomplete across all revenue streams
**Severity: CRITICAL**

### What’s missing
Vision says revenue data = game + wallet + food.  
v29 Phase 11 mentions billing FSM and cafe revenue, but **wallet flow is omitted** and likely event/merch too.

### Modules needing connection
- `billing_fsm.rs`
- `wallet.rs`
- `cafe.rs`
- POS flows
- cloud booking/PWA pre-book
- `business_analytics.rs`

### Implementation steps
1. Add daily aggregation from:
   - session billing,
   - wallet top-ups/deductions,
   - cafe sales,
   - online bookings/deposits,
   - event/merch if present.
2. Standardize into `RevenueLineItem`.
3. Ensure reconciliation by day and source.

---

## E2. Expense ingestion is too manual and incomplete
**Severity: HIGH**

### What’s missing
Vision includes rent, utilities, maintenance, salaries.  
v29 mostly uses manual expense entry, with payroll later.

Missing:
- utility anomaly ingestion,
- recurring fixed expenses schedule,
- maintenance actuals auto-post,
- OTA/software/subscription costs.

### Modules needing connection
- `business_analytics.rs`
- `maintenance_scheduler.rs`
- `payroll.rs`
- settings/admin

### Implementation steps
1. Add recurring expense templates.
2. Auto-post maintenance actual cost from completed tasks.
3. Auto-post payroll period totals.
4. Add utility bill entry + anomaly thresholds.

---

## E3. Forecast engine not fully fed by bookings and occupancy sources
**Severity: HIGH**

### What’s missing
Vision forecasting should reflect demand and promotions.  
v29 says time-series occupancy prediction but does not explicitly integrate:
- PWA bookings,
- walk-ins/POS,
- event slots,
- pod availability/maintenance hold,
- pricing changes.

### Modules needing connection
- PWA booking backend
- POS booking/session creation
- `billing_fsm.rs`
- `occupancy_analytics.rs`
- `business_forecast.rs`
- `dynamic_pricing.rs`

### Implementation steps
1. Add forecast input dataset:
   - booked slots,
   - walk-in conversions,
   - no-shows,
   - pod outage windows,
   - promotions,
   - pricing history.
2. Include maintenance hold windows in demand capacity.

---

## E4. Dynamic pricing not connected to live selling channels
**Severity: CRITICAL**

### What’s missing
Vision: pricing AI adjusts settings via dashboard approval.  
v29 mentions dynamic pricing foundation, but not write-through into:
- POS,
- PWA,
- kiosk,
- cloud sync,
- local billing.

### Modules needing connection
- `dynamic_pricing.rs`
- `billing_fsm.rs`
- POS pricing config
- PWA pricing config
- kiosk app
- `cloud_sync.rs`

### Implementation steps
1. Add canonical pricing table with versioning.
2. Dashboard approve proposed price change.
3. Push price config to:
   - server local billing,
   - POS agent,
   - kiosk,
   - cloud PWA.
4. Add rollback capability and effective windows.

---

## E5. Pricing effectiveness audit is underdefined
**Severity: MEDIUM**

### What’s missing
Vision: forecast engine audits pricing effectiveness and feeds feedback loop.  
v29 only mentions it at high level.

### Modules needing connection
- `dynamic_pricing.rs`
- `business_forecast.rs`
- `feedback_loop.rs`
- booking/session revenue sources

### Implementation steps
1. Track each pricing experiment:
   - baseline demand,
   - applied price,
   - conversion delta,
   - revenue delta,
   - occupancy delta.
2. Feed outcome into future recommendations.

---

## E6. Alert engine for payroll/utilities/occupancy deviations not concretely implemented
**Severity: CRITICAL**

### What’s missing
This is in the vision explicitly; in v29 it is only implicit.

### Modules needing connection
- `business_analytics.rs`
- `payroll.rs`
- `occupancy_analytics.rs`
- `whatsapp_alerter.rs`
- dashboard alerts

### Implementation steps
1. Create `racecontrol/src/alert_engine.rs`
2. Rules:
   - payroll > expected by X%,
   - utility cost spike,
   - occupancy below forecast,
   - occupancy above safe staffing threshold.
3. Send to:
   - dashboard,
   - WhatsApp,
   - `feedback_loop`/forecast retraining queue.

---

# F. Occupancy / Session / Customer Flow Gaps

## F1. Occupancy monitor is not clearly based on actual session state
**Severity: HIGH**

### What’s missing
Vision includes occupancy monitor with active sessions and peak hours.  
v29 has occupancy analytics but no explicit hard integration to `billing_fsm.rs`.

### Modules needing connection
- `billing_fsm.rs`
- kiosk launcher
- pod states
- `occupancy_analytics.rs`

### Implementation steps
1. Emit occupancy events from session lifecycle:
   - session started,
   - paused,
   - ended,
   - transferred.
2. Aggregate by pod/hour/day.
3. Make this the source of truth for business forecasting and scheduling.

---

## F2. Maintenance hold not reflected in customer journey channels
**Severity: HIGH**

### What’s missing
If a pod is under maintenance or predicted to fail, the customer-facing channels must stop offering it.

### Modules needing connection
- kiosk app
- PWA booking
- POS
- `pod_healer.rs`
- `maintenance_scheduler.rs`

### Implementation steps
1. Add pod sellability API:
   - available,
   - degraded,
   - hold until,
   - reason.
2. Kiosk/PWA/POS consume it before booking or launch.

---

## F3. Player downtime notifications are delayed to Phase 24 and not part of core recovery
**Severity: MEDIUM**

### What’s missing
Vision implies alert/communication integration as part of operations.  
v29 treats player notifications as future polish.

### Modules needing connection
- `whatsapp_alerter.rs`
- kiosk/PWA
- billing/session state
- maintenance events

### Implementation steps
1. If active session interrupted:
   - notify staff dashboard,
   - optionally notify customer via PWA/WhatsApp if booked remotely,
   - suggest alternate pod/time.

---

# G. Cloud / Dashboard / History Gaps

## G1. Unified admin dashboard is split, not unified
**Severity: HIGH**

### What’s missing
Vision wants one unified dashboard for AI insights, overrides, schedules, business, HR, maintenance.  
v29 adds pages but likely remains page-siloed.

### Modules needing connection
- Next.js Admin pages
- business APIs
- HR APIs
- maintenance APIs
- alert APIs
- override APIs

### Implementation steps
1. Add `/admin/overview` or equivalent:
   - active incidents,
   - occupancy,
   - forecast,
   - staffing,
   - EBITDA,
   - pending approvals.
2. Cross-link recommendation cards with approval actions.

---

## G2. No explicit historical analytics module for maintenance ROI and downtime trends
**Severity: HIGH**

### What’s missing
Vision has `HISTORY_LOG` as a first-class cloud/admin module.  
v29 has storage pieces but not a history product.

### Modules needing connection
- `cloud_sync.rs`
- `maintenance_store.rs`
- `business_analytics.rs`
- `maintenance_kpi.rs`

### Implementation steps
1. Create history endpoints:
   - downtime by pod/component/time,
   - MTTR trend,
   - prevented failure estimates,
   - ROI trend.
2. Add cloud-side retention and reports.

---

## G3. WhatsApp alerting is not truly multi-channel escalation-aware
**Severity: MEDIUM**

### What’s missing
Vision says multi-channel alerts for critical/manager/routine.  
v29 centers on WhatsApp only.

### Modules needing connection
- `whatsapp_alerter.rs`
- dashboard notifications
- maybe email/SMS/call provider

### Implementation steps
1. Add alert routing policy:
   - dashboard only,
   - WhatsApp,
   - voice/call escalation,
   - PWA push.
2. Persist ACK/resolve state.

---

## G4. Dashboard ACK / investigate / resolve actions not clearly persisted into history
**Severity: HIGH**

### What’s missing
Vision explicitly shows dashboard operator action into history log.

### Modules needing connection
- dashboard actions
- `maintenance_store.rs`
- history log table
- `feedback_loop.rs`

### Implementation steps
1. Add incident action table:
   - acked_at,
   - investigated_at,
   - resolved_at,
   - operator notes.
2. Surface in event timeline and KPI metrics.

---

# H. Integration with Existing Ecosystem Gaps

## H1. `wallet.rs` not integrated into business analytics vision
**Severity: HIGH**

### What’s missing
Wallet is a core existing flow but omitted from the plan’s explicit integrations.

### Modules needing connection
- `wallet.rs`
- `business_analytics.rs`
- cloud sync
- PWA balance/revenue reports

### Implementation steps
1. Export wallet top-ups, deductions, liabilities, breakage if applicable.
2. Distinguish cashflow vs recognized gaming revenue.

---

## H2. `cloud_sync.rs` not extended enough for new domains
**Severity: CRITICAL**

### What’s missing
Vision depends heavily on cloud sync for logs, KPIs, schedules, feedback.  
v29 lists cloud_sync as existing but doesn’t specify expanded contracts for:
- maintenance tasks,
- HR records,
- payroll summaries,
- KPI history,
- pricing versions,
- override decisions.

### Modules needing connection
- `cloud_sync.rs`
- all new modules above
- Bono cloud services

### Implementation steps
1. Define sync DTOs for each domain.
2. Add incremental sync cursors per table.
3. Resolve offline conflict policy:
   - venue authoritative for ops,
   - cloud authoritative for shared analytics/config where applicable.

---

## H3. `ota_pipeline.rs` only touched at Phase 25, not connected to maintenance workflows earlier
**Severity: MEDIUM**

### What’s missing
Software update applied is a maintenance event type, but OTA is not integrated into current maintenance decisioning.

### Modules needing connection
- `ota_pipeline.rs`
- `maintenance_store.rs`
- `maintenance_scheduler.rs`
- `snapshot_manager.rs`

### Implementation steps
1. When update planned/applied, log maintenance event.
2. Snapshot config before deployment.
3. Validate post-update metrics.
4. Feed outcome into maintenance history.

---

## H4. Kiosk app is missing from maintenance and pricing integration
**Severity: HIGH**

### What’s missing
The kiosk is customer-facing and controls game launch/booking choice. It must know:
- pod maintenance hold,
- pricing changes,
- ETA for availability.

### Modules needing connection
- kiosk app `.23:3300`
- `dynamic_pricing.rs`
- pod availability APIs
- maintenance scheduler

### Implementation steps
1. Add kiosk APIs:
   - live pod availability,
   - effective price list,
   - maintenance ETA.
2. Refresh kiosk cache on updates.

---

## H5. POS agent not explicitly integrated with dynamic pricing and maintenance constraints
**Severity: HIGH**

### What’s missing
POS books sessions and cafe orders, but v29 doesn’t explicitly wire new pricing and pod status constraints into POS.

### Modules needing connection
- `rc-pos-agent`
- `billing_fsm.rs`
- `dynamic_pricing.rs`
- pod sellability API

### Implementation steps
1. POS must fetch current effective price version.
2. POS booking flow must reject unavailable/maintenance-hold pods.
3. POS should show recommendation for alternate pod/time.

---

# I. Data Model / Schema / Operational Gaps

## I1. Mixed use of `f64` in telemetry aggregates without normalization strategy
**Severity: LOW**

### What’s missing
Not a blocker, but there’s no clear metric catalog or unit normalization. Over time this causes drift.

### Modules needing connection
- `telemetry_store.rs`
- `maintenance_engine.rs`
- dashboard charts

### Implementation steps
1. Add metric registry:
   - name,
   - unit,
   - source,
   - valid range,
   - collection cadence.

---

## I2. No canonical incident model separate from raw maintenance events
**Severity: HIGH**

### What’s missing
Many low-level events can belong to one incident. Vision’s dashboard/history imply incident-level operations.

### Modules needing connection
- `maintenance_store.rs`
- `maintenance_engine.rs`
- dashboard
- alerting

### Implementation steps
1. Add `Incident` table:
   - opened_at,
   - affected_pod,
   - root cause,
   - current status,
   - linked events/tasks.
2. Group correlated events using `correlation_id`.

---

## I3. No robust state machine for task/incidents/escalations
**Severity: MEDIUM**

### What’s missing
Escalation exists as a function, but not as persistent workflow state.

### Modules needing connection
- `maintenance_scheduler.rs`
- `maintenance_store.rs`
- dashboard
- `whatsapp_alerter.rs`

### Implementation steps
1. Persist escalation state:
   - current tier,
   - last notified,
   - acked_by,
   - SLA deadline.
2. Cron/escalation reevaluation.

---

## I4. Utility data source is absent
**Severity: HIGH**

### What’s missing
Vision includes utilities in business analytics and alerting, but no source/system is defined.

### Modules needing connection
- `business_analytics.rs`
- admin expense inputs
- alert engine

### Implementation steps
1. Add utility bill/manual meter entry.
2. Track monthly baseline and alert on deviations.

---

## I5. No PII/data governance details for HR + face attendance + WhatsApp
**Severity: MEDIUM**

### What’s missing
Not explicit in vision, but critical in reality. Especially in India with employee data + facial attendance.

### Modules needing connection
- `hr_store.rs`
- attendance
- cloud sync
- dashboard auth

### Implementation steps
1. Encrypt face enrollment references/phone numbers where feasible.
2. Add role-based access controls for HR pages.
3. Audit log access to employee/payroll records.

---

# J. Feedback Loop Gaps

## J1. `feedback_loop.rs` is underdefined and likely stub-level
**Severity: CRITICAL**

### What’s missing
Vision’s continuous feedback engine is central:
- review,
- learn,
- refine,
- reapply.

v29 names `feedback_loop.rs` but does not define:
- inputs,
- outputs,
- retraining cadence,
- threshold adaptation,
- recommendation quality metrics.

### Modules needing connection
- `maintenance_store.rs`
- `maintenance_kpi.rs`
- `business_forecast.rs`
- `dynamic_pricing.rs`
- `hr_kpi.rs`
- dashboard overrides

### Implementation steps
1. Define feedback ingestion:
   - prediction outcome,
   - task validation result,
   - admin override,
   - pricing outcome,
   - customer satisfaction after maintenance,
   - technician performance.
2. Define adaptation outputs:
   - threshold adjustments,
   - rule confidence updates,
   - staffing recommendations,
   - forecast model parameter tuning.
3. Schedule nightly feedback processing job.

---

## J2. Customer feedback only appears very late and is not tied to maintenance impact
**Severity: MEDIUM**

### What’s missing
Vision’s feedback loop should include customer experience, but Phase 22 delays it and keeps it narrow.

### Modules needing connection
- rc-agent overlay
- `experience_score.rs`
- maintenance events
- feedback loop

### Implementation steps
1. Correlate post-session score with:
   - pod anomalies,
   - recent maintenance actions,
   - FPS/network issues.
2. Use in root cause and ROI analytics.

---

## J3. No “reapply” mechanism after learning
**Severity: HIGH**

### What’s missing
Vision explicitly says refine and reapply.  
v29 lacks explicit config rollout of learned thresholds/rules.

### Modules needing connection
- `feedback_loop.rs`
- `maintenance_engine.rs`
- `dynamic_pricing.rs`
- `business_forecast.rs`
- dashboard approvals

### Implementation steps
1. Create model/rule version table.
2. Nightly generate proposed updates.
3. Require admin approval for high-impact changes.
4. Publish versioned configs.

---

# K. XAI / AI Usage Gaps

## K1. XAI is specified but not guaranteed on every recommendation path
**Severity: HIGH**

### What’s missing
Guiding principle says XAI on all AI decisions.  
But rule-based + scheduler + pricing + forecast + escalation should also produce explainable outputs, not just Ollama diagnosis.

### Modules needing connection
- `maintenance_xai.rs`
- `dynamic_pricing.rs`
- `business_forecast.rs`
- `maintenance_scheduler.rs`
- dashboard

### Implementation steps
1. Standardize:
   - `ExplanationBundle { summary, factors, confidence, source }`
2. Return it from every recommendation API.

---

## K2. AI usage governance missing for cloud fallback and budget enforcement
**Severity: MEDIUM**

### What’s missing
Principle says local first, cloud cap.  
No concrete budget meter/governor is described.

### Modules needing connection
- `rc-sentry-ai/ai_diagnosis.rs`
- OpenRouter client
- dashboard/admin settings

### Implementation steps
1. Add per-day spend ledger.
2. Gate cloud calls by:
   - severity,
   - business value,
   - daily remaining budget.

---

# L. Broken End-to-End Data Flows

## L1. Telemetry → anomaly → task → HR assignment → completion → KPI → feedback is not fully closed
**Severity: CRITICAL**

### Broken point
Starts in telemetry, maybe creates event/task, but HR assignment, completion validation, KPI contribution, and feedback adaptation are not fully specified/wired.

### Modules to connect
- `telemetry_store.rs`
- `maintenance_engine.rs`
- `maintenance_scheduler.rs`
- `hr_store.rs`
- `maintenance_kpi.rs`
- `feedback_loop.rs`

### Implementation steps
Build one canonical workflow:
1. anomaly creates event
2. event creates/updates incident
3. incident/task assigned via HR
4. technician completes task with before/after metrics
5. KPI updates
6. feedback loop reconciles accuracy and effectiveness

---

## L2. Business metrics → forecast → pricing → approval → sales channels → effectiveness feedback is broken
**Severity: CRITICAL**

### Broken point
Pricing recommendations exist conceptually, but no guaranteed push to POS/PWA/kiosk, nor proper backtest/audit.

### Modules to connect
- `business_analytics.rs`
- `business_forecast.rs`
- `dynamic_pricing.rs`
- dashboard
- POS
- PWA
- kiosk
- `cloud_sync.rs`
- feedback loop

### Implementation steps
1. Proposal created
2. Admin approves
3. Effective price version published
4. All channels consume it
5. Revenue/occupancy impact measured
6. Feedback loop updates model

---

## L3. Attendance → payroll → labor cost → business analytics → alerting/optimization is broken
**Severity: CRITICAL**

### Broken point
HR pieces exist, but labor economics are not flowing through to optimization and alerts as in vision.

### Modules to connect
- attendance
- `payroll.rs`
- `business_analytics.rs`
- `alert_engine.rs`
- `optimization_engine.rs`

### Implementation steps
1. Attendance computes actual hours
2. Payroll computes cost by employee/day
3. Business aggregates labor cost/session and labor cost/hour
4. Alert engine monitors variance
5. Optimization adjusts staffing recommendations

---

## L4. Recovery outcomes → cloud history → ROI reports → admin insights is broken
**Severity: HIGH**

### Broken point
Post-recovery logs don’t clearly become cloud historical analytics.

### Modules to connect
- `pod_healer.rs`
- `maintenance_store.rs`
- `cloud_sync.rs`
- cloud analytics/dashboard

### Implementation steps
1. Standard sync payloads
2. Cloud aggregation job
3. Historical reports and ROI dashboard

---

# Likely Stub-Only Areas

These are the modules most likely to be placeholder-level unless proven otherwise, because the plan names them but gives little operational detail.

## S1. `feedback_loop.rs`
**Severity: CRITICAL**
- Likely stub unless there is real nightly processing, scoring, and config update logic.

## S2. `maintenance_xai.rs`
**Severity: HIGH**
- Likely just formatter/helper unless wired into every decision path.

## S3. `snapshot_manager.rs`
**Severity: MEDIUM**
- Mentioned only late; probably not integrated with OTA and maintenance workflows.

## S4. `occupancy_analytics.rs`
**Severity: HIGH**
- Likely dashboard math unless directly consuming session lifecycle and booking data.

## S5. `business_forecast.rs`
**Severity: HIGH**
- Likely forecasting prototype unless fed by complete booking/price/promo/outage history.

## S6. `dynamic_pricing.rs`
**Severity: CRITICAL**
- Stub if it produces recommendations but does not publish effective prices to POS/PWA/kiosk/billing.

## S7. `payroll.rs`
**Severity: HIGH**
- Stub if only computes wages but not linked to attendance, incentives, business metrics, and alerts.

## S8. `maintenance_gate.rs`
**Severity: MEDIUM**
- Likely local guard only, not tied into central incident/recovery orchestration.

---

# Missing Entirely from Vision Coverage

## M1. Utility anomaly engine
**Severity: HIGH**
- Vision requires it; v29 does not concretely implement it.

## M2. Unified alert engine
**Severity: CRITICAL**
- Vision has a dedicated alert engine; v29 spreads alerts across maintenance and WhatsApp.

## M3. Historical analytics product
**Severity: HIGH**
- Storage exists/planned, but not a first-class analytics/reporting capability.

## M4. Staffing optimization engine
**Severity: HIGH**
- Not truly present.

## M5. Load balancing / pod rerouting
**Severity: HIGH**
- Not truly present.

## M6. Revenue + wallet + food unified revenue ledger
**Severity: HIGH**
- Not fully present.

## M7. Incident management domain
**Severity: HIGH**
- Events/tasks exist; incidents do not.

---

# Recommended Priority Order to Close Gaps

## P0 — Must fix first
1. **Mesh orchestrator / canonical workflow**
2. **Unified alert engine**
3. **Self-heal validation loop**
4. **Dynamic pricing publish path to POS/PWA/kiosk**
5. **Attendance → payroll → business analytics linkage**
6. **Maintenance scheduler ↔ HR assignment integration**
7. **Cloud sync contracts for all new domains**

## P1 — Next
8. Incident model
9. Occupancy source-of-truth from billing/session lifecycle
10. Recovery-to-history analytics
11. HR KPI engine
12. Pricing effectiveness audit
13. Feedback loop actual implementation

## P2 — After that
14. Utility anomaly monitoring
15. Incentives/bonus payroll
16. Pod rerouting and customer rescheduling
17. Snapshot/OTA integration
18. Full admin unified overview

---

# Concrete File-Level Actions

## racecontrol
- **Add**
  - `mesh_orchestrator.rs`
  - `data_collector.rs`
  - `alert_engine.rs`
  - `optimization_engine.rs`
  - `incident_store.rs`
  - `hr_kpi.rs`
- **Modify**
  - `main.rs` or router setup: register all APIs and background jobs
  - `telemetry_store.rs`: emit normalized snapshots
  - `maintenance_engine.rs`: action dispatch, validation, escalation persistence
  - `maintenance_scheduler.rs`: skill/availability/cost-aware assignment
  - `business_analytics.rs`: ingest wallet/POS/PWA/cafe/maintenance/payroll
  - `business_forecast.rs`: add bookings/promotions/outage inputs
  - `dynamic_pricing.rs`: publish approved price versions
  - `cloud_sync.rs`: sync DTOs for maintenance/HR/business/KPI/overrides
  - `whatsapp_alerter.rs`: tiered route + ACK support
  - `pod_healer.rs`: report outcomes and validations
  - `billing_fsm.rs`: occupancy events + sellability checks
  - `wallet.rs`: ledger export hooks
  - `cafe.rs`: revenue export hooks
  - `ota_pipeline.rs`: maintenance event hooks + snapshot/validation

## rc-agent
- **Modify**
  - `self_heal.rs`: emit structured recovery outcomes
  - `predictive_maintenance.rs`: attach telemetry quality/status
  - overlay/session UI: post-maintenance customer feedback and downtime notices

## rc-sentry
- **Modify**
  - `maintenance_gate.rs`: central command intake + action result reporting
  - `tier1_fixes.rs`: standardized event/result payloads

## rc-sentry-ai
- **Modify**
  - `attendance/`: expose actual-hours APIs/events
  - `ai_diagnosis.rs`: budget gate, structured XAI, recommendation IDs

## Admin / Next.js
- **Add/Modify**
  - unified overview page
  - incident detail page
  - override capture UI
  - maintenance ROI page
  - staffing optimization page
  - alert ACK/investigate/resolve controls
  - pricing approval + publish status

## POS / Kiosk / PWA
- **Modify**
  - consume effective pricing version
  - respect pod sellability/maintenance hold
  - show alternate pod/time
  - reflect ETA availability

---

# Final Verdict

v29.0 is **strong on module decomposition and phase planning**, but compared to the vision it is still missing the most important thing:

## the system is not yet a true closed-loop operating mesh.

It has many components of the mesh, but the biggest gaps are in:
- **orchestration**
- **cross-domain integration**
- **write-back into live operations**
- **feedback-driven adaptation**
- **customer/ops channel propagation**
- **cloud historical consolidation**

If you want, I can turn this into a **tabular gap matrix** with columns:

`Vision Node | Current v29 Coverage | Gap Type (Missing/Stub/Not Integrated/Broken Flow) | Severity | Files | Exact Fix`

That would be easier to hand directly to engineering.