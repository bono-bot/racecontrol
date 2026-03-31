**INTEGRATION AUDIT FINDINGS - v29.0 Meshed Intelligence**

## P1 CRITICAL ISSUES

### P1-001: WIRING GAP - Maintenance Modules Not Initialized
**File:line**: main.rs:267  
**What's wrong**: `maintenance_engine` module is imported but `spawn_anomaly_scanner` returns an unused `_anomaly_state`. The anomaly detection system isn't properly wired to trigger healing actions.  
**Fix**: Store the anomaly state in AppState and wire it to `self_healing::apply_action`:
```rust
let anomaly_state = racecontrol_crate::maintenance_engine::spawn_anomaly_scanner(telem_pool);
let inner = Arc::get_mut(&mut state).expect("no other Arc refs yet");
inner.anomaly_state = Some(anomaly_state);
```

### P1-002: DATA FLOW BREAK - Extended Telemetry Storage Missing
**File:line**: ws/mod.rs:ExtendedTelemetry handler  
**What's wrong**: `store_extended_telemetry` is called but the function doesn't exist in telemetry_store module. Extended telemetry data is lost.  
**Fix**: Implement the missing storage function in telemetry_store.rs:
```rust
pub fn store_extended_telemetry(state: &Arc<AppState>, pod_id: String, msg: &AgentMessage) {
    if let Some(ref tx) = state.telemetry_writer_tx {
        let _ = tx.send(TelemetryRecord::Extended { pod_id, data: msg.clone() });
    }
}
```

### P1-003: TYPE MISMATCH - Pod Availability Map Not in AppState
**File:line**: self_healing.rs:85  
**What's wrong**: `PodAvailabilityMap` is created but never stored in AppState. API route `pod_availability_handler` can't access pod availability data.  
**Fix**: Add to AppState in state.rs:
```rust
pub pod_availability: Arc<RwLock<HashMap<u8, PodAvailability>>>,
```
Initialize in main.rs after AppState creation:
```rust
let inner = Arc::get_mut(&mut state).expect("no other Arc refs yet");
inner.pod_availability = crate::self_healing::new_availability_map();
```

### P1-004: API ROUTE CONFLICTS - Missing Handler Implementation
**File:line**: api/routes.rs:159  
**What's wrong**: Route `/pods/{id}/availability` registered but `pod_availability_handler` function is missing. All requests to this endpoint will cause compilation failure.  
**Fix**: Implement the handler in routes.rs:
```rust
async fn pod_availability_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<u8>,
) -> impl IntoResponse {
    let availability = state.pod_availability.read().await;
    match availability.get(&id) {
        Some(avail) => Json(json!({ "ok": true, "availability": avail })),
        None => Json(json!({ "ok": false, "error": "Pod not found" })),
    }
}
```

## P2 MODERATE ISSUES

### P2-005: RACE CONDITIONS - Business Aggregator Access
**File:line**: business_aggregator.rs:21  
**What's wrong**: Multiple daily aggregations could run simultaneously on startup vs hourly timer, causing duplicate/inconsistent metrics.  
**Fix**: Add mutex protection in AppState for business aggregation:
```rust
pub business_aggregator_lock: Arc<tokio::sync::Mutex<()>>,
```

### P2-006: STARTUP ORDER - Feedback Tables Before Maintenance Events
**File:line**: main.rs:291  
**What's wrong**: `init_feedback_tables` is called after maintenance events might be created, causing foreign key constraint failures if feedback references maintenance event IDs.  
**Fix**: Move feedback table init right after maintenance table init at line 275:
```rust
// Phase 28: Must come after maintenance tables are ready
if let Err(e) = racecontrol_crate::feedback_loop::init_feedback_tables(&state.db).await {
    tracing::error!("Failed to initialize feedback tables: {e}");
}
```

### P2-007: DB SCHEMA CONFLICTS - Missing Staff CRUD Operations  
**File:line**: api/routes.rs:446  
**What's wrong**: Routes for `update_staff` and `delete_staff` are registered but functions don't exist. Staff management endpoints will fail at runtime.  
**Fix**: Implement missing handlers:
```rust
async fn update_staff(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(update): Json<Value>,
) -> impl IntoResponse {
    // Implementation needed
    Json(json!({ "ok": false, "error": "Not implemented" }))
}

async fn delete_staff(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    // Implementation needed  
    Json(json!({ "ok": false, "error": "Not implemented" }))
}
```

### P2-008: WIRING GAPS - Ollama Health Check Unused
**File:line**: ollama_client.rs:66  
**What's wrong**: `health_check()` function exists but is never called. Ollama failures will happen silently without connectivity validation.  
**Fix**: Add periodic health check in pod_healer or app_health_monitor:
```rust
if !crate::ollama_client::health_check().await {
    tracing::warn!("Ollama service unavailable on James machine");
}
```

## P3 MINOR ISSUES

### P3-009: DATA FLOW GAPS - Pricing Proposals Not Auto-Applied
**File:line**: pricing_bridge.rs:59  
**What's wrong**: `apply_approved_pricing()` exists but is never called. Approved pricing changes won't take effect automatically.  
**Fix**: Add to scheduler or create periodic task:
```rust
// In main.rs spawned tasks section
tokio::spawn(async move {
    let mut interval = tokio::time::interval(Duration::from_secs(300)); // 5 min
    loop {
        interval.tick().await;
        let _ = racecontrol_crate::pricing_bridge::apply_approved_pricing(&pool).await;
    }
});
```

### P3-010: TYPE MISMATCHES - Cloud Sync Maintenance Data Unused
**File:line**: cloud_sync.rs:140  
**What's wrong**: `sync_maintenance_data()` collects data but doesn't actually send it to cloud. The collected events are logged but never transmitted.  
**Fix**: Wire to existing cloud sync pipeline or add HTTP push:
```rust
// Add to existing cloud sync loop
if let Ok(maint_data) = sync_maintenance_data(&pool, &cloud_url).await {
    // Push maint_data via existing relay mechanism
}
```

### P3-011: MISSING SPAWNER - Alert Engine Doesn't Send Alerts  
**File:line**: alert_engine.rs:90  
**What's wrong**: `spawn_alert_checker` generates alerts but the TODO comment shows they're not wired to whatsapp_alerter. Business alerts won't reach staff.  
**Fix**: Complete the integration:
```rust
let _alerts = check_business_alerts(&pool).await;
for alert in alerts {
    if matches!(alert.channel, AlertChannel::WhatsApp | AlertChannel::Both) {
        // Send via existing WhatsApp alerter
    }
}
```

### P3-012: WIRING GAPS - HW Telemetry Interval Not Used in Agent
**File:line**: event_loop.rs:146  
**What's wrong**: `hw_telemetry_interval` is created but the corresponding tick handler sends data that may not be properly stored due to P1-002.  
**Fix**: Depends on P1-002 fix. Ensure storage pipeline is complete before agent sends extended telemetry.

**SUMMARY**: 4 P1 critical integration breaks (compilation/runtime failures), 4 P2 moderate issues (feature gaps), 4 P3 minor issues (incomplete features). Primary focus should be on P1-001 through P1-004 to ensure system stability.