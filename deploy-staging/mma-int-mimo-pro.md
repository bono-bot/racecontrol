# Integration Audit Report: v29.0 Meshed Intelligence

## Executive Summary
The audit identified 12 integration issues across the 35-phase codebase, with 3 critical (P1), 5 high (P2), and 4 medium (P3) severity findings. The most significant issues involve missing wiring between new modules and existing infrastructure, data flow breaks in the business intelligence pipeline, and potential race conditions in the telemetry subsystem.

---

## P1 - Critical Findings

### 1. Missing Cloud Sync Integration
**Severity**: P1  
**File**: `cloud_sync.rs:135-176`  
**What's wrong**: The `sync_maintenance_data()` function is defined but never called from the main `spawn()` loop. The cloud sync task only syncs legacy data, leaving v29.0 maintenance, HR, and analytics data stranded on-premises.
**Concrete fix**: In `cloud_sync.rs`'s `spawn()` function, add a call to `sync_maintenance_data()` within the sync interval, after the existing legacy sync operations. Ensure proper error handling and logging.

```rust
// In cloud_sync::spawn() loop:
if let Err(e) = sync_maintenance_data(&state.db, &cloud_url).await {
    tracing::warn!(target: "cloud-sync", error = %e, "Maintenance data sync failed");
}
```

### 2. Telemetry Data Never Consumed by Maintenance Engine
**Severity**: P1  
**File**: `ws/mod.rs:ExtendedTelemetry handler` + `data_collector.rs:1-120`  
**What's wrong**: Extended telemetry is stored via `telemetry_store::store_extended_telemetry()` but never flows to the `maintenance_engine::spawn_anomaly_scanner()` that was initialized in `main.rs`. The anomaly scanner expects data from `telemetry_aggregates` table, but raw telemetry goes to `hardware_telemetry`.
**Concrete fix**: In `data_collector.rs`, modify `check_rul_thresholds()` to query the raw `hardware_telemetry` table when `telemetry_aggregates` is empty. Add a data pipeline that aggregates raw telemetry into `telemetry_aggregates` every 15 minutes.

```rust
// In data_collector.rs: Add aggregation step
async fn aggregate_telemetry(telem_pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT OR REPLACE INTO telemetry_aggregates 
         (pod_id, metric_name, avg_val, min_val, max_val, period_hours, period_start)
         SELECT pod_id, 'gpu_temp_celsius', AVG(gpu_temp_celsius), 
                MIN(gpu_temp_celsius), MAX(gpu_temp_celsius), 
                1, datetime('now', '-1 hour')
         FROM hardware_telemetry 
         WHERE collected_at > datetime('now', '-1 hour')
         GROUP BY pod_id"
    ).execute(telem_pool).await?;
    Ok(())
}
```

### 3. Missing Business Metrics → Alert Engine Data Flow
**Severity**: P1  
**File**: `business_aggregator.rs:1-100` + `alert_engine.rs:1-80`  
**What's wrong**: Business metrics are aggregated hourly but the alert engine checks every 30 minutes. If revenue drops sharply mid-hour, alerts won't trigger until the next aggregation cycle (up to 1 hour delay).
**Concrete fix**: Add a real-time revenue tracking structure in `AppState` that records transactions as they occur. Modify `alert_engine::check_business_alerts()` to check both the DB (hourly) and the in-memory tracker (real-time).

---

## P2 - High Findings

### 4. Missing WebSocket Route for AI Diagnostics
**Severity**: P2  
**File**: `main.rs:Router setup` + `ollama_client.rs:1-60`  
**What's wrong**: The Ollama AI client is implemented but there's no WebSocket or API route to trigger diagnostics from the dashboard or kiosk. The `ws::ai_ws` endpoint exists but its handler doesn't integrate with `ollama_client::diagnose()`.
**Concrete fix**: Add a WebSocket message handler in `ws/ai_ws` that accepts diagnostic requests and calls `ollama_client::diagnose()`. Create an API route `POST /api/v1/ai/diagnose` for REST access.

### 5. Pod Availability Map Not Wired to AppState
**Severity**: P2  
**File**: `self_healing.rs:50-100` + `state.rs` (missing)  
**What's wrong**: `self_healing::new_availability_map()` creates a `PodAvailabilityMap` but there's no evidence it's included in `AppState`. The `pod_availability_handler` API route needs this map to function.
**Concrete fix**: Add `pub pod_availability: PodAvailabilityMap` to `AppState::new()` and initialize it with `self_healing::new_availability_map()`. Ensure it's accessible to handlers.

### 6. Pricing Bridge Not Connected to Billing
**Severity**: P2  
**File**: `pricing_bridge.rs:60-80` + `billing.rs` (missing integration)  
**What's wrong**: `apply_approved_pricing()` updates the proposal status but doesn't push prices to the billing configuration. The TODO comment indicates this is incomplete.
**Concrete fix**: After marking proposals as 'applied', call `billing::update_rate_tiers()` with the new prices. Add a channel from `pricing_bridge` to `billing` for real-time updates.

### 7. Missing Data Retention Job Integration
**Severity**: P2  
**File**: `main.rs:spawn tasks section` + `api/routes.rs:data_retention_job`  
**What's wrong**: The data retention job is spawned via `api::routes::spawn_data_retention_job()` but this function appears to be a route handler, not a spawnable task. The actual implementation may not exist.
**Concrete fix**: Create a dedicated `data_retention::spawn_background_job(state)` function and call it from `main.rs` instead of routing through the API module.

### 8. HR Tables Not Seeded with Default Employees
**Severity**: P2  
**File**: `main.rs:Phase 13-14 init` + `data_collector.rs:staff query`  
**What's wrong**: HR tables are initialized but no default employees are seeded. The `data_collector::collect_venue_snapshot()` queries `employees` table which may be empty, returning `staff_on_duty: 0` even when staff are present.
**Concrete fix**: Add employee seeding in `main.rs` after HR table initialization, similar to pod seeding. At minimum, create a system user for automated tasks.

---

## P3 - Medium Findings

### 9. Inconsistent Timezone Handling
**Severity**: P3  
**File**: `alert_engine.rs:40-50` + `business_aggregator.rs:30-40`  
**What's wrong**: `alert_engine` uses a rough IST conversion `(Utc::now().hour() + 5) % 24` which doesn't account for daylight saving or precise timezone. `business_aggregator` uses UTC dates directly.
**Concrete fix**: Use `chrono_tz` for proper timezone handling. Store venue timezone in config and use it consistently across all modules.

### 10. Telemetry Table Schema Mismatch
**Severity**: P3  
**File**: `api/routes.rs:analytics_telemetry handler` vs `predictive_maintenance.rs:HardwareTelemetry`  
**What's wrong**: The API handler queries `hardware_telemetry` table expecting columns like `gpu_temp_celsius`, `cpu_temp_celsius`, etc. The agent sends these fields in `ExtendedTelemetry`, but the table schema in `telemetry_store` may not match exactly.
**Concrete fix**: Ensure `telemetry_store::init_telemetry_db()` creates the `hardware_telemetry` table with all columns matching the `ExtendedTelemetry` protocol message. Add schema validation on startup.

### 11. Missing Error Aggregator Integration
**Severity**: P3  
**File**: `main.rs:error_aggregator::spawn()` + `alert_engine.rs`  
**What's wrong**: Error patterns are detected by `error_aggregator` but don't feed into the business alert system. Critical error patterns (like repeated payment failures) should trigger business alerts.
**Concrete fix**: Add a channel from `error_aggregator` to `alert_engine` for error-pattern-based business alerts. Define new alert types for operational errors.

### 12. Bono Relay State Clone Race Condition
**Severity**: P3  
**File**: `main.rs:Bono relay section`  
**What's wrong**: `state.clone()` is called before the main router setup to build the relay router. If any mutable state is added to `AppState` later, the relay router will have a stale clone.
**Concrete fix**: Move the Bono relay router creation to after all state initialization, or use `Arc<AppState>` consistently and avoid early cloning.

---

## Startup Order Analysis

**Current initialization sequence**:
1. Config load → 2. Database init → 3. Encryption keys → 4. PII migration → 5. AppState::new() → 6. Telemetry DB → 7. Maintenance tables → 8. Business tables → 9. HR tables → 10. Business aggregator → 11. Feedback tables → 12. Pricing tables → 13. Alert checker → 14. Rating worker → 15. Pod seeding → 16. Feature flags → 17. Background tasks

**Issues identified**:
- Phase 26 (Business aggregator) starts before Phase 30 (Alert checker), but they share dependencies on `daily_business_metrics` table. ✅ Correct order.
- Phase 35 (Data collector) needs `telemetry_db` which is initialized in Phase 251. ✅ Correct order.
- Phase 13-14 (HR tables) are initialized but never seeded with data, causing empty results in Phase 35 queries. ❌ Missing seeding step.

---

## Recommendations

1. **Immediate**: Fix P1 findings, especially the telemetry data flow break (#2) which makes the anomaly detection system non-functional.
2. **This Sprint**: Address P2 findings to ensure all v29.0 modules are properly integrated.
3. **Next Sprint**: Resolve P3 findings for robustness and consistency.
4. **Add Integration Tests**: Create tests that verify data flows from agent → server → DB → API → dashboard.
5. **Document Data Flows**: Create a data flow diagram showing how telemetry, business metrics, and maintenance events move through the system.

---

## Verification Steps for Fixes

For each fix, verify:
1. **Wiring**: Module is called from appropriate lifecycle point
2. **Data Flow**: Data moves from producer to consumer without loss
3. **Race Conditions**: Shared state is properly synchronized
4. **Error Handling**: Failures are logged and don't crash the system
5. **Backward Compatibility**: Existing v28.x functionality remains intact

Total issues found: 12 (3 P1, 5 P2, 4 P3)  
Estimated fix effort: 3-5 developer days for P1/P2 issues