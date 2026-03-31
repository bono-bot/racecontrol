## INTEGRATION AUDIT REPORT — v29.0 Meshed Intelligence

### EXECUTIVE SUMMARY
15 integration defects identified across 7 categories. Critical wiring gaps in anomaly→self-healing→feedback chain. Business alerting disconnected from notification channels. Multiple DB schema conflicts between init functions.

---

### P1 FINDINGS (Critical, breaks core functionality)

1. **SEVERITY: P1**  
   **FILE: main.rs:240-242**  
   **ISSUE:** Anomaly-scanner spawned but never wired to self-healing actions or feedback loop.  
   **CODE:** `let _anomaly_state = racecontrol_crate::maintenance_engine::spawn_anomaly_scanner(telem_pool);`  
   **FIX:** Store anomaly_state in AppState, wire to `self_healing::recommend_action()` and `feedback_loop::record_outcome()`.

2. **SEVERITY: P1**  
   **FILE: alert_engine.rs:98-103**  
   **ISSUE:** Business alerts generated but never delivered via WhatsApp or dashboard. TODO comment indicates missing wiring.  
   **CODE:** `// TODO: wire alerts to whatsapp_alerter for WhatsApp channel`  
   **FIX:** In `check_business_alerts()`, filter by AlertChannel and call appropriate notifier: WhatsApp via `whatsapp_alerter::send_alert()`, Dashboard via `state.dashboard_tx`.

3. **SEVERITY: P1**  
   **FILE: main.rs:133-137**  
   **ISSUE:** Pod availability map initialized in self_healing.rs but never added to AppState or used by kiosk/pricing/dashboard APIs.  
   **CODE:** `self_healing::new_availability_map()` created but not stored.  
   **FIX:** Add `availability_map: PodAvailabilityMap` field to AppState, initialize in AppState::new(), update in maintenance endpoints.

4. **SEVERITY: P1**  
   **FILE: data_collector.rs:108**  
   **ISSUE:** RUL threshold queries `telemetry_aggregates` table but Phase 5 anomaly-scanner creates `hardware_telemetry`. Schema mismatch.  
   **CODE:** `FROM telemetry_aggregates WHERE metric_name IN ('disk_smart_health_pct', 'gpu_temp_celsius')`  
   **FIX:** Use `hardware_telemetry` table or create `telemetry_aggregates` via maintenance_engine aggregates.

---

### P2 FINDINGS (Major, partial failure)

5. **SEVERITY: P2**  
   **FILE: self_healing.rs:30-50**  
   **ISSUE:** Healing actions are enum variants but no executor to run them (RestartPod, ClearDiskSpace, etc.).  
   **FIX:** Add `execute_healing_action(state: &AppState, action: HealingAction) -> HealingOutcome` that calls existing pod_healer, process_guard, or server_ops functions.

6. **SEVERITY: P2**  
   **FILE: feedback_loop.rs:73-75**  
   **ISSUE:** `record_outcome()` needs `predicted_value` → `actual_value` comparison but no data source for actual values (anomaly detection outcomes).  
   **FIX:** Add callback from maintenance_engine anomaly validation: `on_anomaly_resolved(event_id, actual_severity, resolution_time)` → calculate lead_time_hours.

7. **SEVERITY: P2**  
   **FILE: main.rs:124-127**  
   **ISSUE:** Pricing bridge initialized but no connection to dynamic_pricing module or billing config push. Prices never applied.  
   **FIX:** Add call to `pricing_bridge::apply_approved_pricing(&state.db)` in billing::refresh_rate_tiers() after fetching from DB.

8. **SEVERITY: P2**  
   **FILE: ws/mod.rs:34-44**  
   **ISSUE:** Extended telemetry stored via `telemetry_store::store_extended_telemetry()` but doesn't invoke anomaly scanner or RUL checks.  
   **FIX:** After storage, emit channel message to anomaly scanner: `state.anomaly_tx.send((pod_id, telemetry_data))`.

9. **SEVERITY: P2**  
   **FILE: ollama_client.rs:80-82**  
   **ISSUE:** `health_check()` pings `/api/tags` but Ollama might be running without model loaded. Diagnosis calls will fail.  
   **FIX:** Check specific model existence: `POST /api/show` with model name, verify `"response": "success"`.

10. **SEVERITY: P2**  
    **FILE: api/routes.rs:1453-1456**  
    **ISSUE:** `/pods/{id}/availability` route added but handler `pod_availability_handler` not implemented. Returns 404.  
    **FIX:** Implement handler querying `self_healing::PodAvailabilityMap` from AppState.

11. **SEVERITY: P2**  
    **FILE: cloud_sync.rs:138-165**  
    **ISSUE:** `sync_maintenance_data()` collects data but doesn't push to cloud (no HTTP call). Returns Ok(()) without transmission.  
    **FIX:** Call existing cloud sync HTTP client with JSON payload containing recent_events, active_staff, revenue_today.

---

### P3 FINDINGS (Minor, inconsistent behavior)

12. **SEVERITY: P3**  
    **FILE: business_aggregator.rs:26-29**  
    **ISSUE:** Aggregator queries `DATE(ended_at)` but billing_sessions.ended_at is TEXT RFC3339. SQLite DATE() works but may be locale-dependent.  
    **FIX:** Use `ended_at LIKE '2024-%'` or store ended_at_date as separate DATE column.

13. **SEVERITY: P3**  
    **FILE: data_collector.rs:72-74**  
    **ISSUE:** `collect_venue_snapshot()` placeholder values for pod counts (hard-coded 8). Should query fleet_health or pod_monitor.  
    **FIX:** Add `fleet_health::online_pod_count()` and `pod_monitor::degraded_count()` functions.

14. **SEVERITY: P3**  
    **FILE: predictive_maintenance.rs:495-497**  
    **ISSUE:** Collectors return `Option<T>` but `HardwareTelemetry` fields are `Option<T>`. No error aggregation if multiple collectors fail.  
    **FIX:** Add `failed_collectors: Vec<String>` field to telemetry, log which collectors failed each cycle.

15. **SEVERITY: P3**  
    **FILE: main.rs:119-121 vs 124-127**  
    **ISSUE:** Two init functions potentially creating overlapping tables: `init_business_tables()` (phase 11) vs `init_hr_tables()` (phase 13-14). No dependency ordering.  
    **FIX:** Add comment or assert that HR tables include employee_attendance which business tables don't touch.

---

### INTEGRATION GAP SUMMARY

**MISSING WIRES:**
1. Anomaly → Self-healing → Feedback loop
2. Business alerts → WhatsApp/Dashboard
3. Pod availability map → Kiosk API routes
4. Pricing proposals → Billing config refreshes

**DATA FLOW BREAKS:**
1. Telemetry → Anomaly scanner (stored but not processed)
2. Cloud sync collects but doesn't transmit
3. RUL queries wrong table name

**SCHEMA CONFLICTS:**
1. `telemetry_aggregates` vs `hardware_telemetry`
2. DATE() on RFC3339 timestamps
3. Business vs HR table overlap risk

---

### RECOMMENDED PATCH BUNDLE

```rust
// 1. Add to AppState (state.rs)
pub struct AppState {
    // ... existing fields
    pub availability_map: self_healing::PodAvailabilityMap,
    pub anomaly_tx: tokio::sync::mpsc::Sender<(String, JsonValue)>,
    pub feedback_db: SqlitePool, // same as main db
}

// 2. Wire in main.rs after init functions
let (anomaly_tx, anomaly_rx) = tokio::sync::mpsc::channel(100);
let anomaly_state = maintenance_engine::spawn_anomaly_scanner_with_feedback(
    telem_pool, 
    state.feedback_db.clone(),
    anomaly_rx
);
state.anomaly_tx = anomaly_tx;

// 3. Connect alerts to whatsapp
async fn dispatch_business_alert(
    state: &AppState,
    alert: &BusinessAlert,
) {
    match alert.channel {
        AlertChannel::WhatsApp => {
            whatsapp_alerter::send_alert(state, &alert.message).await;
        }
        AlertChannel::Dashboard => {
            let _ = state.dashboard_tx.send(DashboardEvent::BusinessAlert(alert.clone()));
        }
        AlertChannel::Both => {
            // both
        }
    }
}
```

**TOTAL DEFECTS:** 15 (P1:4, P2:7, P3:4)  
**CRITICAL CHAIN:** Telemetry→Anomaly→Self-healing→Feedback→Monitoring→Alerts→Notification requires 7 connection points, 4 are missing.