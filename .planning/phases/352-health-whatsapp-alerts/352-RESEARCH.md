# Phase 352: Health + WhatsApp Alerts - Research

**Researched:** 2026-04-10
**Domain:** Rust/Axum health probes, WhatsApp alerting, structured logging, comms-link relay integration
**Confidence:** HIGH

## Summary

Phase 352 extends the existing flat `/api/v1/health` endpoint with per-subsystem probes, wires the Phase 343-02 `alert_incidents` TODO, adds a central dedup layer for subsystem degradation alerts, creates a fallback `/relay/alert` endpoint on comms-link, adds structured JSON logging for admin API requests, and configures rsync of logs to Bono VPS.

The codebase already has MASSIVE alerting infrastructure (10 modules, 4,470 lines) with well-established patterns for dedup, Evolution API dispatch, and incident recording. The key finding is that almost all building blocks already exist -- this phase is primarily **wiring and extension**, not greenfield. The existing `whatsapp_alerter.rs` (375 lines) has `send_whatsapp()`, `record_incident()`, `resolve_incident()`, and `ist_now_string()` that can be reused directly. The `server_diagnostics.rs` already has a DB write probe pattern (INSERT/DELETE on `server_health_probe` table) that can be replicated for the health endpoint.

**Primary recommendation:** Create a new `subsystem_health.rs` module that runs probes on a 10-second interval, maintains state transitions (ok->degraded->ok), uses a central `HashMap<(String,String), Instant>` dedup, and fires WhatsApp alerts via existing `send_whatsapp()`. Extend the existing `/api/v1/health` handler to call into this module for the `subsystems` object. Add `/relay/alert` to comms-link's James relay server (`james/index.js`) as a fallback dispatch path.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- D1: Extend existing `/api/v1/health` endpoint with a `subsystems` object alongside current flat fields (backward-compatible). Each subsystem returns `{ ok: bool, latency_ms: u64, error_code: Option<String>, detail: Option<String> }`.
- D2: Keep direct Evolution API dispatch from racecontrol as PRIMARY. Add comms-link `/relay/alert` as FALLBACK only (when Evolution API unreachable from venue).
- D3: Central dedup layer via `HashMap<(String, String), Instant>` with 10-min window. Same `(subsystem, error_code)` pair within 10 minutes = suppress.
- D4: Wire `alert_incidents` table for history. Table tracks: `id, alert_type, subsystem, severity, message, fired_at, resolved_at, correlation_id`. INSERT on every WhatsApp alert dispatch.
- D5: `tracing` middleware for admin API request logging using tower-http's `TraceLayer`. Logs `{ ts, level, method, route, status, latency_ms, staff_id, err, correlation_id }`.
- D6: Daily rotation via existing tracing rolling appender. Add background task that rsyncs `data/logs/*.jsonl` to Bono VPS `/root/backups/venue-logs/` via SSH.
- D7: Admin dashboard health page DEFERRED to Phase 354 (already partially shipped: 354-01, 354-02).

### Claude's Discretion
- Subsystem probe implementation details (probe order, timeout values, error classification)
- Alert message formatting beyond the `[RP ALERT]`/`[RP RESOLVED]` prefix pattern
- Rsync scheduling (cron vs tokio background task)
- `alert_incidents` table schema evolution (D4 adds columns not in current schema)

### Deferred Ideas (OUT OF SCOPE)
- Alert analytics dashboard (trending, SLA tracking, MTTR per subsystem)
- Centralized dedup across racecontrol + comms-link
- SMS fallback for WhatsApp alerts
- Per-pod alert routing (currently all alerts go to Uday)
- Alert severity semantics standardization across modules
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| OPS-01 | `/api/health` probes admin_db, rc_backend, gateway, static_assets, db_writable, litestream_lag, disk_free -- each returns `{ok, latency_ms, error_code, detail}` | Existing `check_db_health()` in server_diagnostics.rs provides INSERT/DELETE probe pattern. `check_evolution_health()` in routes.rs provides HTTP probe pattern. CONTEXT.md D1 specifies 7 subsystems. |
| OPS-02 | `/settings/health` page renders live per-subsystem tiles with 10s auto-refresh | DEFERRED to Phase 354 per D7. Backend API is Phase 352 scope only. |
| OPS-03 | Degraded subsystem triggers WhatsApp alert via POST to comms-link relay `/relay/alert` on James .27 | D2 makes this FALLBACK only. Primary path is direct Evolution API via existing `send_whatsapp()`. `/relay/alert` is new endpoint on james/index.js relay server (:8766). |
| OPS-04 | Alert dedup -- same subsystem + error_code within 10 minutes = single alert | D3 specifies `HashMap<(String, String), Instant>` in the health monitor task. Existing modules use per-alert cooldown (5min in whatsapp_alerter, 30min in metric_alerts) -- this is a NEW central dedup. |
| OPS-05 | Phase 343 Plan 02 `whatsapp_alerter.rs` TODO wired to the alert path | D4: INSERT into `alert_incidents` on every WhatsApp alert dispatch. Current table schema needs `subsystem`, `severity`, `correlation_id` columns added. |
| OPS-06 | Structured JSON log format for admin API requests | D5: tower-http `TraceLayer` already applied at line 1281 of main.rs. Need to customize with `on_request`/`on_response` callbacks for structured fields. |
| OPS-07 | Admin API logs rotated daily and rsync'd to Bono VPS `/root/backups/venue-logs/` | D6: `RollingFileAppender` already configured for daily rotation (main.rs:582-587, `logs/racecontrol-*.jsonl`). Need rsync background task. Existing `event_archive.rs` has SCP-to-Bono pattern. |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| axum | workspace | HTTP framework, health endpoint | Already in use |
| tower-http | 0.6 | TraceLayer middleware | Already in Cargo.toml with `["cors", "fs", "trace"]` features |
| tracing | workspace | Structured logging | Already the project's logging framework |
| tracing-appender | workspace | Rolling file appender | Already used for daily JSONL rotation (main.rs:577-588) |
| tracing-subscriber | workspace | Log filtering + formatting | Already configured with EnvFilter |
| serde / serde_json | workspace | JSON serialization | Already used everywhere |
| sqlx | workspace | SQLite queries for probes and alert_incidents | Already used for all DB access |
| reqwest | workspace | HTTP client for Evolution API + fallback relay | Already used in whatsapp_alerter.rs |
| chrono / chrono-tz | workspace | IST timestamps | Already used via `ist_now_string()` |
| tokio | workspace | Async runtime, background tasks, timers | Already the async runtime |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| uuid | workspace | Correlation IDs for alerts | Already used in record_incident() |

**No new dependencies required.** Everything needed is already in the workspace.

## Architecture Patterns

### Recommended Project Structure
```
crates/racecontrol/src/
  subsystem_health.rs     # NEW: probe definitions, health monitor task, dedup
  whatsapp_alerter.rs     # EXTEND: add subsystem alert dispatch + alert_incidents wiring
  api/routes.rs           # EXTEND: health() handler calls subsystem probes
  config.rs               # EXTEND: add SubsystemHealthConfig if needed
  db/mod.rs               # EXTEND: ALTER TABLE alert_incidents ADD COLUMN subsystem, severity, correlation_id
  main.rs                 # EXTEND: customize TraceLayer, spawn subsystem_health task
  event_archive.rs        # REFERENCE: SCP-to-Bono pattern to reuse for log rsync

comms-link/james/
  index.js                # EXTEND: add POST /relay/alert handler
```

### Pattern 1: Subsystem Health Probe Module
**What:** A background tokio task that runs all probes every 10 seconds, maintains state transitions, and fires alerts on degradation.
**When to use:** For the new `subsystem_health.rs` module.
**Example:**
```rust
// Pattern from existing server_diagnostics.rs check_db_health()
// and app_health_monitor.rs probe loop

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, RwLock};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, serde::Serialize)]
pub struct SubsystemStatus {
    pub ok: bool,
    pub latency_ms: u64,
    pub error_code: Option<String>,
    pub detail: Option<String>,
}

// Central dedup map: (subsystem, error_code) -> last_alert_time
// 10-minute window per D3
struct DedupMap(HashMap<(String, String), Instant>);

// State stored in LazyLock<RwLock<>> (same pattern as app_health_monitor.rs CURRENT_HEALTH)
static SUBSYSTEM_STATE: LazyLock<RwLock<HashMap<String, SubsystemStatus>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

pub fn get_current_status() -> HashMap<String, SubsystemStatus> {
    SUBSYSTEM_STATE.read().unwrap_or_else(|e| e.into_inner()).clone()
}
```

### Pattern 2: Alert Dispatch with Fallback Chain
**What:** Try direct Evolution API first, fall back to comms-link relay POST.
**When to use:** When sending subsystem degradation alerts.
**Example:**
```rust
// Primary: existing send_whatsapp() (Evolution API direct)
// Fallback: POST to http://192.168.31.27:8766/relay/alert
async fn dispatch_alert(config: &Config, state: &AppState, subsystem: &str, message: &str) {
    // Try direct first
    send_whatsapp(config, message).await;
    
    // Record in alert_incidents (D4)
    record_subsystem_incident(&state.db, subsystem, /* ... */).await;
    
    // Fallback: if Evolution unreachable, try relay
    // (only if direct dispatch returned error -- need to modify send_whatsapp to return Result)
}
```

### Pattern 3: Extending Health Endpoint (Backward-Compatible)
**What:** Add `subsystems` field to existing health JSON without breaking existing consumers.
**When to use:** For the `/api/v1/health` handler modification.
**Example:**
```rust
async fn health(State(state): State<Arc<AppState>>) -> Json<Value> {
    let whatsapp_status = check_evolution_health(&state).await;
    let subsystems = subsystem_health::get_current_status();
    
    // Existing fields preserved (backward-compatible)
    Json(json!({
        "status": "ok",
        "service": "racecontrol",
        "version": env!("CARGO_PKG_VERSION"),
        "build_id": BUILD_ID,
        "whatsapp": whatsapp_status,
        "deploy_context": "...",
        // NEW: per-subsystem probes
        "subsystems": subsystems,
    }))
}
```

### Pattern 4: Comms-Link Relay Alert Endpoint
**What:** Add `POST /relay/alert` to james/index.js relay server.
**When to use:** As fallback WhatsApp dispatch path.
**Example:**
```javascript
// In james/index.js relayServer createServer handler
if (req.method === 'POST' && req.url === '/relay/alert') {
    const payload = await parseBody(req);
    // Validate required fields
    const { source, subsystem, severity, message, timestamp } = payload;
    if (!message) {
        jsonResponse(res, 400, { ok: false, error: 'message required' });
        return;
    }
    // Forward to Bono's AlertManager.sendWhatsApp() via WS
    // OR dispatch directly via sendEvolutionText() from alert-manager.js
    // (James has Evolution API access too)
    jsonResponse(res, 200, { ok: true, dispatched: true });
    return;
}
```

### Anti-Patterns to Avoid
- **Probe lies:** The existing health endpoint returns `"status": "ok"` unconditionally. Per CLAUDE.md standing rule "Probes that lie", every subsystem probe must do an ACTUAL check (write test row, HTTP request, file stat), not just return ok.
- **Lock across await:** Per standing rule, snapshot the subsystem state with `{ let guard = lock.read(); guard.clone() }` before any async work. The `app_health_monitor.rs` uses this pattern correctly.
- **Single-fetch-at-boot:** Per CLAUDE.md "Boot Resilience" rule, the health probe task must be a continuous background loop (10s interval), not a one-time check. Already planned as a tokio::spawn loop.
- **Unwrap in production:** All probe results must use `?`, `.ok()`, or `match`. Probe failure = subsystem reports as degraded, not a panic.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| WhatsApp dispatch | New HTTP client for Evolution API | Existing `send_whatsapp()` in whatsapp_alerter.rs | Already handles config validation, timeout, error logging |
| IST timestamps | Manual UTC offset math | Existing `ist_now_string()` in whatsapp_alerter.rs | Standing rule: never use plain UTC for displayed timestamps |
| DB write probe | Custom SQLite health check | Adapt `check_db_health()` from server_diagnostics.rs | Already validates INSERT/DELETE round-trip with latency tracking |
| Alert incident recording | New table/queries | Existing `record_incident()` / `resolve_incident()` in whatsapp_alerter.rs | Already handles UUID generation, error handling |
| Rolling log files | Custom file rotation | Existing `RollingFileAppender` in main.rs | Already configured for daily rotation |
| SCP to Bono VPS | Custom file transfer | Adapt `transfer_jsonl_to_bono()` from event_archive.rs | Already handles SSH options, SHA256 verification, error handling |
| HTTP request tracing | Custom middleware | tower-http `TraceLayer` (already imported and applied) | Already in the middleware stack at main.rs:1281 |
| Evolution API reachability | Custom health check | Existing `check_evolution_health()` in routes.rs | Already handles timeout, URL validation |

**Key insight:** This phase is 80% wiring existing infrastructure and 20% new code. The biggest risk is accidentally duplicating functionality that already exists in the 4,470 lines of alerting code.

## Common Pitfalls

### Pitfall 1: Breaking Existing Health Consumers
**What goes wrong:** Adding `subsystems` to the health response could break consumers that parse the response with strict schema validation.
**Why it happens:** rc-agent, app_health_monitor, synthetic_monitor, and deploy scripts all call `/api/v1/health`.
**How to avoid:** Keep ALL existing fields unchanged. The `subsystems` field is additive. Test that `jq .status` still works on the response.
**Warning signs:** Any consumer that does `serde_json::from_str::<ExactStruct>()` with `deny_unknown_fields` would fail.

### Pitfall 2: Dedup Map Memory Leak
**What goes wrong:** The `HashMap<(String, String), Instant>` grows unbounded if old entries are never cleaned up.
**Why it happens:** Subsystems can produce many distinct error_codes over time.
**How to avoid:** On each probe cycle, evict entries older than 10 minutes from the dedup map. The existing `app_health_monitor.rs` per-app cooldown uses `Instant::elapsed()` checks which naturally expire.
**Warning signs:** Memory growth over weeks/months.

### Pitfall 3: Probe Timeout Blocking Health Response
**What goes wrong:** The `/api/v1/health` endpoint becomes slow (>5s) because a subsystem probe hangs (e.g., Evolution API DNS timeout).
**Why it happens:** Running probes synchronously in the health handler.
**How to avoid:** Probes run in a BACKGROUND TASK (10s interval) and cache results. The health handler reads cached results -- zero latency. This is exactly the pattern used by `app_health_monitor.rs` (CURRENT_HEALTH static).
**Warning signs:** Health endpoint latency > 1s in monitoring.

### Pitfall 4: alert_incidents Schema Migration
**What goes wrong:** Adding columns to `alert_incidents` via CREATE TABLE IF NOT EXISTS doesn't alter existing tables (per CLAUDE.md standing rule: "DB migrations must cover ALL consumers").
**Why it happens:** Existing venue/cloud databases already have the table from earlier migrations.
**How to avoid:** Use `ALTER TABLE ADD COLUMN IF NOT EXISTS` (or try/catch the ALTER). The current schema has: `id, alert_type, started_at, resolved_at, pod_count, description, created_at`. D4 needs to add: `subsystem`, `severity`, `correlation_id`. Use separate ALTER statements for each column.
**Warning signs:** New columns silently missing on deployed databases.

### Pitfall 5: send_whatsapp() is Fire-and-Forget
**What goes wrong:** The fallback chain (D2: direct -> relay) can't detect if direct dispatch failed because `send_whatsapp()` returns `()`, not `Result`.
**Why it happens:** The function was designed as best-effort (logs warnings, never fails).
**How to avoid:** Either (a) create a new `try_send_whatsapp()` that returns `Result<(), Error>` for the fallback chain, or (b) probe Evolution API health first and choose the dispatch path based on reachability (already checked by `check_evolution_health()`).
**Warning signs:** Alerts silently lost when Evolution API is down.

### Pitfall 6: Rsync Credential Handling on Windows
**What goes wrong:** rsync/SCP from Windows (Git Bash) to Bono VPS requires SSH key access without interactive password prompt.
**Why it happens:** Windows SSH agent may not have the key loaded, or the Tailscale SSH path may differ.
**How to avoid:** Use the same SSH options as `event_archive.rs`: `StrictHostKeyChecking=no`, `BatchMode=yes`, `ConnectTimeout=10`. Use Tailscale IP (100.70.177.44) for reliable routing.
**Warning signs:** rsync hanging on password prompt in background task.

### Pitfall 7: Disk Free Check is Platform-Specific
**What goes wrong:** `statvfs` is Unix-only. On Windows (where racecontrol runs at venue), need Windows-specific disk space API.
**Why it happens:** Rust's `std::fs::available_space` is unstable. Need platform-specific code.
**How to avoid:** Use `sysinfo` crate (already in workspace?) or shell out to `wmic logicaldisk` / `fsutil`. Check if `sysinfo` is already a dependency.
**Warning signs:** Compilation fails on Windows targets.

## Code Examples

### Subsystem Probe Implementation (DB Write)
```rust
// Adapted from server_diagnostics.rs check_db_health()
async fn probe_db_writable(db: &SqlitePool) -> SubsystemStatus {
    let start = Instant::now();
    let ts = chrono::Utc::now().to_rfc3339();
    
    match sqlx::query("INSERT OR REPLACE INTO server_health_probe (id, ts) VALUES (1, ?)")
        .bind(&ts)
        .execute(db)
        .await
    {
        Ok(_) => {
            // Cleanup
            let _ = sqlx::query("DELETE FROM server_health_probe WHERE id = 1")
                .execute(db).await;
            SubsystemStatus {
                ok: true,
                latency_ms: start.elapsed().as_millis() as u64,
                error_code: None,
                detail: None,
            }
        }
        Err(e) => SubsystemStatus {
            ok: false,
            latency_ms: start.elapsed().as_millis() as u64,
            error_code: Some("DB_WRITE_FAILED".to_string()),
            detail: Some(e.to_string()),
        },
    }
}
```

### Cloud Sync Staleness Probe
```rust
// Check last sync time from sync_state table
async fn probe_cloud_sync(db: &SqlitePool, threshold_secs: u64) -> SubsystemStatus {
    let start = Instant::now();
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT MIN(last_synced_at) FROM sync_state WHERE table_name != '_push'"
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten();
    
    match row {
        Some((ts,)) => {
            if let Ok(last_sync) = chrono::NaiveDateTime::parse_from_str(&ts, "%Y-%m-%dT%H:%M:%S%.fZ") {
                let age_secs = (chrono::Utc::now().naive_utc() - last_sync).num_seconds();
                if age_secs > threshold_secs as i64 {
                    SubsystemStatus {
                        ok: false,
                        latency_ms: start.elapsed().as_millis() as u64,
                        error_code: Some("SYNC_STALE".to_string()),
                        detail: Some(format!("Last sync {}s ago (threshold: {}s)", age_secs, threshold_secs)),
                    }
                } else {
                    SubsystemStatus { ok: true, latency_ms: start.elapsed().as_millis() as u64, error_code: None, detail: None }
                }
            } else {
                SubsystemStatus { ok: false, latency_ms: 0, error_code: Some("SYNC_PARSE_ERROR".to_string()), detail: Some(ts) }
            }
        }
        None => SubsystemStatus { ok: false, latency_ms: 0, error_code: Some("NO_SYNC_STATE".to_string()), detail: Some("No sync records found".to_string()) },
    }
}
```

### Comms-Link /relay/alert (JavaScript)
```javascript
// In james/index.js relayServer handler, after existing routes
if (req.method === 'POST' && req.url === '/relay/alert') {
    const payload = await parseBody(req);
    const { source, subsystem, severity, message } = payload;
    if (!message) {
        jsonResponse(res, 400, { ok: false, error: 'message field required' });
        return;
    }
    
    // Import sendEvolutionText from alert-manager.js
    // James has Evolution API config in env/config
    const evoUrl = process.env.EVOLUTION_URL;
    const evoKey = process.env.EVOLUTION_API_KEY;
    const evoInstance = process.env.EVOLUTION_INSTANCE;
    const udayPhone = process.env.UDAY_PHONE;
    
    if (!evoUrl || !evoKey || !evoInstance || !udayPhone) {
        jsonResponse(res, 503, { ok: false, error: 'Evolution API not configured on relay' });
        return;
    }
    
    const result = await sendEvolutionText({
        url: evoUrl, instance: evoInstance, apiKey: evoKey,
        number: udayPhone, text: message
    });
    
    jsonResponse(res, result.ok ? 200 : 502, { ok: result.ok, dispatched: result.ok });
    return;
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Flat health endpoint (`"status": "ok"` always) | Per-subsystem probes with actual checks | Phase 352 | Health endpoint becomes trustworthy |
| No central dedup across alert modules | Central dedup in subsystem_health + existing per-module dedup | Phase 352 | Prevents alert storms |
| No WhatsApp fallback path | Direct Evolution + comms-link relay fallback | Phase 352 | Alerts survive Evolution API outages |
| No structured admin API logging | tower-http TraceLayer with structured JSON fields | Phase 352 | Audit trail for admin actions |

**Deprecated/outdated:**
- The current `"deploy_context"` field in health response is a hardcoded string describing deployed features. It has no programmatic value and should be preserved but not relied upon.

## Open Questions

1. **Disk free check on Windows**
   - What we know: `statvfs` is Unix-only. The `sysinfo` crate can provide disk info cross-platform.
   - What's unclear: Whether `sysinfo` is already in the workspace dependencies. If not, whether to add it or shell out to `wmic`.
   - Recommendation: Check `Cargo.toml` at plan time. If `sysinfo` is available, use it. Otherwise, use `std::process::Command` to run `wmic logicaldisk where "DeviceID='C:'" get FreeSpace /value` and parse the output.

2. **admin_db probe scope**
   - What we know: D1 lists `admin_db` as a subsystem to probe. admin.db is a SEPARATE SQLite file used by the Next.js admin app, not by racecontrol directly.
   - What's unclear: How racecontrol accesses admin.db. Phase 345 added lazy-load with `AdminDbError`. The probe needs to test admin.db accessibility from racecontrol.
   - Recommendation: If racecontrol has an `admin_db` pool in AppState, probe it. If not, use a file-existence + read-permission check on the admin.db path.

3. **send_whatsapp() Return Value for Fallback Chain**
   - What we know: `send_whatsapp()` returns `()` (fire-and-forget). D2 requires fallback to relay when direct fails.
   - What's unclear: Whether to modify the existing function signature (breaking change to 10+ callers) or create a new `try_send_whatsapp()` function.
   - Recommendation: Create `try_send_whatsapp() -> Result<(), AlertError>` that wraps the existing logic with a return value. Use this in the subsystem health module. Leave `send_whatsapp()` unchanged for existing callers.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust compiler | Binary build | Yes | 1.93.1 | -- |
| tower-http `trace` feature | TraceLayer | Yes | 0.6 (Cargo.toml) | -- |
| tracing-appender | Rolling logs | Yes | workspace | -- |
| Evolution API | WhatsApp dispatch | Yes (configured) | -- | comms-link relay (D2) |
| Comms-link relay | Fallback dispatch | Yes | :8766 on James .27 | SSH fallback |
| SSH/SCP to Bono VPS | Log rsync | Yes | Tailscale 100.70.177.44 | -- |
| Node.js | Comms-link relay | Yes | v22.22.0 on James | -- |

**Missing dependencies with no fallback:** None

**Missing dependencies with fallback:** None

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (Rust) + run-all.sh (comms-link) |
| Config file | Cargo.toml (workspace) |
| Quick run command | `cargo test -p racecontrol-crate -- subsystem_health` |
| Full suite command | `cargo test -p rc-common && cargo test -p rc-agent-crate && cargo test -p racecontrol-crate` |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| OPS-01 | Per-subsystem probes return correct status | unit | `cargo test -p racecontrol-crate -- subsystem_health -x` | Wave 0 |
| OPS-03 | WhatsApp alert fires on degradation | unit | `cargo test -p racecontrol-crate -- subsystem_health::tests::alert_fires_on_degradation -x` | Wave 0 |
| OPS-04 | Dedup suppresses duplicate alerts within 10m | unit | `cargo test -p racecontrol-crate -- subsystem_health::tests::dedup_suppresses -x` | Wave 0 |
| OPS-05 | alert_incidents table gets INSERT on alert | unit | `cargo test -p racecontrol-crate -- subsystem_health::tests::incident_recorded -x` | Wave 0 |
| OPS-06 | Structured JSON log output format | unit | `cargo test -p racecontrol-crate -- subsystem_health::tests::log_format -x` | Wave 0 |
| OPS-07 | Log rsync task runs without error | integration | manual -- requires SSH access to Bono VPS | manual-only |

### Sampling Rate
- **Per task commit:** `cargo test -p racecontrol-crate -- subsystem_health -x`
- **Per wave merge:** Full test suite (891+ racecontrol tests)
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/racecontrol/src/subsystem_health.rs` -- tests module with unit tests for probes, dedup, alert dispatch
- [ ] Schema migration test for alert_incidents ALTER TABLE

## Sources

### Primary (HIGH confidence)
- Codebase inspection: `whatsapp_alerter.rs` (375 lines) -- full send_whatsapp, record_incident, resolve_incident API
- Codebase inspection: `app_health_monitor.rs` (887 lines) -- CURRENT_HEALTH LazyLock<RwLock<>> pattern, probe loop, per-app cooldown
- Codebase inspection: `routes.rs:744-817` -- current health endpoint + check_evolution_health
- Codebase inspection: `server_diagnostics.rs:133-164` -- DB write probe pattern (INSERT/DELETE on health table)
- Codebase inspection: `event_archive.rs` -- SCP-to-Bono pattern with SSH options
- Codebase inspection: `main.rs:577-602` -- RollingFileAppender + TraceLayer setup
- Codebase inspection: `james/index.js:720-970` -- relay HTTP server route pattern
- Codebase inspection: `bono/alert-manager.js` -- AlertManager + sendEvolutionText for relay-side dispatch
- Codebase inspection: `config.rs:622-635` -- AlertingConfig struct (enabled, uday_phone, cooldown_secs)
- Codebase inspection: `db/mod.rs:2928-2943` -- alert_incidents table schema (current: id, alert_type, started_at, resolved_at, pod_count, description, created_at)
- Codebase inspection: `notification_outbox.rs` -- durable notification queue with retry + exponential backoff

### Secondary (MEDIUM confidence)
- CONTEXT.md D1-D7 decisions -- user-locked architecture

### Tertiary (LOW confidence)
- None

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all libraries already in workspace, zero new deps
- Architecture: HIGH -- all patterns exist in codebase (app_health_monitor, server_diagnostics, event_archive, whatsapp_alerter)
- Pitfalls: HIGH -- derived from standing rules in CLAUDE.md and codebase patterns

**Research date:** 2026-04-10
**Valid until:** 2026-05-10 (stable -- no external API changes expected)
