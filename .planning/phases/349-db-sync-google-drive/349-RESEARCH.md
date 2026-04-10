# Phase 349: DB Sync via Google Drive — Research

**Researched:** 2026-04-11 IST
**Domain:** Rust/Axum authority guard pattern + subsystem health probes + shell sync scripts
**Confidence:** HIGH (all findings from direct codebase inspection)

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**D1: Venue-Authoritative Table List**
Selected: Inverse of cloud-authoritative. ALL tables are venue-authoritative EXCEPT those in `cloud.authoritative_tables` (currently only `staff_members`). Cloud instance should reject writes to billing, sessions, drivers, laps, game state, etc.

**D2: Read-Replica Guard Pattern**
Selected: Symmetric `venue_authority_guard()` function — same pattern as `cloud_authority_guard()` but inverted logic:
- Runs on CLOUD instance (checks `this_instance_is_cloud()`)
- Rejects writes to tables NOT in `authoritative_tables` (venue-owned data)
- Returns 409 with `{"error": "venue_authoritative", "table": "...", "hint": "This table is managed by the venue instance"}`
- Break-glass: `RC_ALLOW_CLOUD_VENUE_WRITE=1` env var (mirrors `RC_ALLOW_VENUE_STAFF_WRITE`)

**D3: Sync Lag Health Probe**
Selected: File-mtime based probe. Check age of `racecontrol.db` on the cloud instance (last modified by download-db.sh):
- `ok: true` if DB mtime < 600s (10 min, 2 missed 5-min cycles)
- WARN threshold: 300s (5 min, 1 missed cycle)
- CRITICAL threshold: 900s (15 min, 3 missed cycles)
- `detail` field shows human-readable age: `"Last sync 4m 32s ago"`
- CLOUD-ONLY probe

**D4: Admin UI Badge (SYNC-04)**
DEFERRED to Phase 354.

**D5: Monthly Restore Drill (SYNC-07)**
Document procedure in `scripts/db-sync/RESTORE-DRILL.md`. Operational runbook, not code.

**D6: Pause Replication Command (SYNC-08)**
Selected: `touch /tmp/DB_SYNC_PAUSED` sentinel file checked by download-db.sh before downloading. Upload continues. Document in RESTORE-DRILL.md.

### Claude's Discretion
- Guard placement: which specific write endpoints to guard (enumerate from routes.rs)
- Sync probe interval alignment with existing health probe cycle
- sync-status.json format (already defined by shipped scripts — just consume it)

### Deferred Ideas (OUT OF SCOPE)
- Admin dashboard sync status badge — SYNC-04 deferred to Phase 354 (UI Hardening)
- Bi-directional conflict resolution for venue-authoritative tables
- Automated restore drill
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| SYNC-05 | Cloud racecontrol refuses writes to replicated tables (409 with hint) — mirrors Phase 343 venue-side guard | D2: `venue_authority_guard()` symmetric pattern; code template at routes.rs:13016; config helpers in config.rs:293-321 |
| SYNC-06 | `/api/health` includes `litestream_lag_seconds` probe — WARN >300s, CRITICAL >900s | D3: file-mtime probe; subsystem_health.rs extensible pattern; DB path from config `default_db_path()` |
| SYNC-07 | Monthly restore drill documented and executed on a scratch path | D5: `scripts/db-sync/RESTORE-DRILL.md` — download-db.sh covers download logic; new scratch-path variant needed |
| SYNC-08 | Break-glass "pause replication" command documented for maintenance windows | D6: sentinel file `/tmp/DB_SYNC_PAUSED` added to download-db.sh; documented in RESTORE-DRILL.md |
</phase_requirements>

---

## Summary

Phase 349-03 has two distinct deliverables. The first is `venue_authority_guard()` — a Rust function in `routes.rs` symmetric to the existing `cloud_authority_guard()` (Phase 343, lines 13016-13037). This guard runs only on the cloud instance and rejects writes to venue-authoritative tables with a 409 response. The function body is five lines of logic mirroring the existing guard. The only non-trivial question is which write endpoints to apply it to — research shows 30+ write endpoints exist and the scope should be narrowed to operational data tables (billing sessions, drivers, laps, game launches, presets, pricing tiers, kiosk settings).

The second deliverable is a `db_sync_lag` probe added to `subsystem_health.rs`. The existing probe infrastructure uses `tokio::task::spawn_blocking` for I/O operations (see `probe_disk_free`), `LazyLock<RwLock<HashMap>>` for cached state, and `SubsystemStatus` as the return type. The new probe checks the mtime of `racecontrol.db` on the cloud instance using `std::fs::metadata`. This is purely additive — one new async function, one new entry in `tokio::join!`, one new `results.insert()` call.

Supporting deliverables: `RESTORE-DRILL.md` (Markdown runbook) and a one-line addition to `download-db.sh` (sentinel file check).

**Primary recommendation:** Implement in this order: (1) `venue_authority_guard()` + apply to endpoint list, (2) `db_sync_lag` probe, (3) `RESTORE-DRILL.md`, (4) sentinel check in `download-db.sh`. Each is independently testable.

---

## Standard Stack

### Core (all already in Cargo.toml — no new dependencies)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `axum` | existing | HTTP handler returns `(StatusCode, Json<Value>)` | Already used throughout routes.rs |
| `serde_json` | existing | `json!({})` macro for 409 response body | Already used in `cloud_authority_guard` |
| `tokio` | existing | `spawn_blocking` for mtime I/O check | Same pattern as `probe_disk_free` |
| `std::fs::metadata` | stdlib | Get file mtime for sync lag probe | No dep needed — pure stdlib |
| `std::time::SystemTime` | stdlib | Convert mtime to age in seconds | No dep needed |
| `chrono` | existing | Already in Cargo.toml, used in health module | Consistent timestamp formatting |

**No new dependencies are required for this phase.**

---

## Architecture Patterns

### Pattern 1: Symmetric Authority Guard (SYNC-05)

**What:** `venue_authority_guard(state, table)` — exact mirror of `cloud_authority_guard` with inverted condition. Where `cloud_authority_guard` blocks when `!is_cloud()` (venue trying to write cloud-authoritative table), `venue_authority_guard` blocks when `is_cloud()` (cloud trying to write venue-authoritative table).

**The key inversion:** `cloud_authority_guard` returns Some(409) when `!this_instance_is_cloud()`. `venue_authority_guard` returns Some(409) when `this_instance_is_cloud()`.

**Table check inversion:** `cloud_authority_guard` fires when `is_cloud_authoritative_for(table)` is true. `venue_authority_guard` fires when `!is_cloud_authoritative_for(table)` is true (i.e., all tables NOT in the cloud list are venue-authoritative).

**Example (verified from routes.rs:13016-13037):**
```rust
// Source: crates/racecontrol/src/api/routes.rs:13016
fn cloud_authority_guard(state: &AppState, table: &str) -> Option<(StatusCode, Json<Value>)> {
    if !state.config.cloud.is_cloud_authoritative_for(table) {
        return None; // cloud sync not enabled or table not in authoritative list
    }
    if crate::config::this_instance_is_cloud(&state.config) {
        return None; // we ARE the cloud — accept mutations
    }
    if crate::config::allow_venue_staff_write() {
        tracing::warn!("Phase 343: RC_ALLOW_VENUE_STAFF_WRITE override active — allowing venue {table} mutation");
        return None;
    }
    // ... returns 409
}

// NEW: venue_authority_guard mirrors this (inverted conditions)
fn venue_authority_guard(state: &AppState, table: &str) -> Option<(StatusCode, Json<Value>)> {
    if !state.config.cloud.enabled {
        return None; // cloud sync not configured
    }
    if !crate::config::this_instance_is_cloud(&state.config) {
        return None; // we are NOT the cloud — venue can write its own tables
    }
    if state.config.cloud.is_cloud_authoritative_for(table) {
        return None; // cloud IS authoritative for this table — allow cloud write
    }
    if allow_cloud_venue_write() {
        tracing::warn!("Phase 349: RC_ALLOW_CLOUD_VENUE_WRITE override active — allowing cloud {table} write");
        return None;
    }
    Some((
        StatusCode::CONFLICT,
        Json(json!({
            "error": "venue_authoritative",
            "table": table,
            "hint": "This table is managed by the venue instance. Writes must go to the venue racecontrol."
        })),
    ))
}
```

**New config helper needed (mirrors `allow_venue_staff_write` at config.rs:319):**
```rust
pub fn allow_cloud_venue_write() -> bool {
    std::env::var("RC_ALLOW_CLOUD_VENUE_WRITE").as_deref() == Ok("1")
}
```

### Pattern 2: Endpoint Scope for venue_authority_guard

**Enumerated from routes.rs** — write endpoints applicable to venue-authoritative tables:

| Endpoint Function | Table(s) | Apply Guard |
|-------------------|----------|-------------|
| `create_driver` | `drivers` | YES |
| `create_session` | `billing_sessions` | YES |
| `create_pricing_tier` | `pricing_tiers` | YES |
| `update_pricing_tier` | `pricing_tiers` | YES |
| `delete_pricing_tier` | `pricing_tiers` | YES |
| `create_billing_rate` | `billing_rates` | YES |
| `update_billing_rate` | `billing_rates` | YES |
| `delete_billing_rate` | `billing_rates` | YES |
| `update_ac_preset` | `ac_presets` | YES |
| `delete_ac_preset` | `ac_presets` | YES |
| `create_kiosk_experience` | `kiosk_experiences` | YES |
| `update_kiosk_experience` | `kiosk_experiences` | YES |
| `delete_kiosk_experience` | `kiosk_experiences` | YES |
| `update_kiosk_settings` | `kiosk_settings` | YES |
| `create_pricing_rule` | `pricing_rules` | YES |
| `update_pricing_rule` | `pricing_rules` | YES |
| `delete_pricing_rule` | `pricing_rules` | YES |
| `create_coupon` / `update_coupon` / `delete_coupon` | `coupons` | YES |
| `create_staff` / `update_staff` / `delete_staff` | `staff_members` | NO — already guarded by `cloud_authority_guard` |
| `create_debug_incident` / `update_debug_incident` | `debug_incidents` | OPTIONAL (operational log, low risk) |
| `create_hotlap_event` / `update_hotlap_event` | `hotlap_events` | YES |
| `create_championship` / `add_championship_round` | `championships` | YES |

**NOT guarded (read-only or cloud-owned):**
- All GET endpoints — cloud MUST read everything
- `create_staff` / `update_staff` / `delete_staff` — already handled by `cloud_authority_guard`
- `create_deploy_log` — operational logging, cloud-local
- `create_dispute_handler` — operational action, not data replication

**Implementation pattern** (call site, verified from Phase 343 callsites at routes.rs:13122, 13236, 13347):
```rust
async fn create_driver(State(state): State<Arc<AppState>>, ...) -> impl IntoResponse {
    if let Some(rejection) = venue_authority_guard(&state, "drivers") {
        return rejection.into_response();
    }
    // ... existing handler body
}
```

### Pattern 3: DB Sync Lag Probe (SYNC-06)

**What:** New async fn `probe_db_sync_lag()` added to `subsystem_health.rs`. Uses `std::fs::metadata` on the racecontrol.db path to get mtime, computes age in seconds. CLOUD-ONLY — returns ok/skip on venue instance.

**Fit with existing infrastructure (verified from subsystem_health.rs:91-111):**
- Add `probe_db_sync_lag(&state.config)` to the `tokio::join!` call in `run_probes()`
- Add `results.insert("db_sync_lag".to_string(), db_sync_lag)` to the results map
- The probe interval is already 10 seconds — no alignment needed (per D3, WARN/CRITICAL thresholds are minutes-scale)

**DB path source:** `state.config.db_path` (or the `default_db_path()` = `"./data/racecontrol.db"` from config.rs:1205). Use `config.db_path` if available on AppState, otherwise reconstruct from config.

**Verified probe pattern** (matches `probe_disk_free` at subsystem_health.rs:222-288 which also uses `spawn_blocking`):
```rust
/// Probe 8: DB Sync Lag — check age of racecontrol.db mtime (CLOUD-ONLY).
/// Returns ok/skip on venue instance. Checks file mtime last written by download-db.sh.
async fn probe_db_sync_lag(config: &Config) -> SubsystemStatus {
    // CLOUD-ONLY: skip on venue instance
    if !crate::config::this_instance_is_cloud(config) {
        return SubsystemStatus {
            ok: true,
            latency_ms: 0,
            error_code: None,
            detail: Some("venue instance — db_sync_lag probe skipped".to_string()),
        };
    }

    let db_path = config.server.db_path.clone();  // verify field name on Config struct
    match tokio::task::spawn_blocking(move || check_db_sync_lag_sync(&db_path)).await {
        Ok(status) => status,
        Err(e) => SubsystemStatus {
            ok: false,
            latency_ms: 0,
            error_code: Some("DB_SYNC_LAG_CHECK_FAILED".to_string()),
            detail: Some(e.to_string()),
        },
    }
}

fn check_db_sync_lag_sync(db_path: &str) -> SubsystemStatus {
    const WARN_SECS: u64 = 300;
    const CRITICAL_SECS: u64 = 900;

    let path = std::path::Path::new(db_path);
    match std::fs::metadata(path) {
        Ok(meta) => {
            match meta.modified() {
                Ok(mtime) => {
                    let age_secs = mtime.elapsed().unwrap_or(Duration::MAX).as_secs();
                    let detail = Some(format!("Last sync {}m {}s ago", age_secs / 60, age_secs % 60));
                    if age_secs >= CRITICAL_SECS {
                        SubsystemStatus { ok: false, latency_ms: 0,
                            error_code: Some("DB_SYNC_LAG_CRITICAL".to_string()), detail }
                    } else if age_secs >= WARN_SECS {
                        SubsystemStatus { ok: false, latency_ms: 0,
                            error_code: Some("DB_SYNC_LAG_WARN".to_string()), detail }
                    } else {
                        SubsystemStatus { ok: true, latency_ms: 0, error_code: None, detail }
                    }
                }
                Err(e) => SubsystemStatus { ok: false, latency_ms: 0,
                    error_code: Some("DB_SYNC_MTIME_UNAVAILABLE".to_string()),
                    detail: Some(e.to_string()) }
            }
        }
        Err(e) => SubsystemStatus { ok: false, latency_ms: 0,
            error_code: Some("DB_SYNC_FILE_NOT_FOUND".to_string()),
            detail: Some(e.to_string()) }
    }
}
```

**IMPORTANT: verify config field name.** The `Config` struct field for the DB path must be confirmed before implementation. `default_db_path()` at config.rs:1205 returns `"./data/racecontrol.db"`. The field is likely `config.database.path` or `config.server.db_path` — implementer must read Config struct definition at config.rs to find the exact field name before writing the probe.

### Pattern 4: Sentinel Pause (SYNC-08)

**What:** One-line addition to `download-db.sh` at top of the flow, after credential load and before Step 1:

```bash
# SYNC-08: Break-glass pause check
if [ -f "/tmp/DB_SYNC_PAUSED" ]; then
    echo "=== SYNC PAUSED (sentinel /tmp/DB_SYNC_PAUSED exists) — skipping download ==="
    exit 0
fi
```

**Why `/tmp`:** Standard Linux ephemeral path on Bono VPS — cleared on reboot, no cleanup needed. Upload script on James .27 (Windows, PowerShell) does NOT check this sentinel — upload continues, preserving data in Drive.

### Pattern 5: RESTORE-DRILL.md

**Location:** `scripts/db-sync/RESTORE-DRILL.md`

**Required sections:**
1. When to run (monthly drill, disaster recovery)
2. Pause replication first (`touch /tmp/DB_SYNC_PAUSED`)
3. Download to scratch path (variant of `download-db.sh` with `RC_DATA_DIR=/tmp/drill-restore`)
4. Verify integrity (`sqlite3 /tmp/drill-restore/racecontrol.db "SELECT COUNT(*) FROM drivers"`)
5. Restore to production path (manual swap, document expected downtime ~30s)
6. Resume replication (`rm /tmp/DB_SYNC_PAUSED`)
7. Verify post-restore health (`curl http://localhost:8080/api/v1/health`)

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Detecting cloud instance | Custom env parsing | `this_instance_is_cloud()` in config.rs:303 | Already tested, handles loopback heuristic |
| 409 response format | New error struct | `json!({})` macro matching Phase 343 format | Consistency with existing cloud_authority_guard |
| File mtime check | Custom syscall | `std::fs::metadata().modified()` | stdlib, cross-platform, already used in probe_disk_free pattern |
| Blocking I/O in async context | `tokio::fs::metadata` (requires tokio dep bump) | `tokio::task::spawn_blocking` | Already used in probe_disk_free — proven pattern in this codebase |
| Sync status parsing | Re-parse sync-status.json | DB file mtime (D3) | Simpler and more reliable — script crash can't corrupt mtime |

---

## Common Pitfalls

### Pitfall 1: Guard Applied to `create_staff` / `update_staff`
**What goes wrong:** Double-guarding `staff_members` endpoints — `cloud_authority_guard` already protects these on the venue side; if `venue_authority_guard` is also applied, it creates an impossible state where neither venue nor cloud can write staff.
**Why it happens:** Mechanical "apply to all write endpoints" without checking existing guards.
**How to avoid:** Skip `staff_members` table in `venue_authority_guard` call sites. The existing `cloud_authority_guard` already covers this table bidirectionally.
**Warning signs:** Staff PIN change fails on both venue and cloud with 409.

### Pitfall 2: Guard Fires When Cloud Sync Disabled
**What goes wrong:** `venue_authority_guard` returns 409 even when cloud sync is not configured (`cloud.enabled = false`), breaking standalone venue deployments.
**Why it happens:** Forgetting the `if !state.config.cloud.enabled { return None; }` early return.
**How to avoid:** First check in `venue_authority_guard` must be `if !state.config.cloud.enabled { return None; }` — same as `is_cloud_authoritative_for()` which already checks `self.enabled` at config.rs:297.
**Warning signs:** Venue-only deployments start returning 409 on billing/driver endpoints.

### Pitfall 3: DB Path Field Name Wrong in Probe
**What goes wrong:** `probe_db_sync_lag` uses a wrong field path like `config.db_path` when the actual field is `config.database.path` (or vice versa), causing compile error or always "file not found".
**Why it happens:** Config struct has nested sections — the implementer must read the actual Config struct before writing the probe.
**How to avoid:** Before implementing the probe, read `config.rs` Config struct definition and find where `default_db_path()` is used to confirm the field chain.
**Warning signs:** Probe always returns `DB_SYNC_FILE_NOT_FOUND` even on a running cloud instance.

### Pitfall 4: Mtime Probe Returns Stale on First Deploy
**What goes wrong:** The probe immediately returns CRITICAL because the DB file was last written by git checkout or SCP at deploy time, not by `download-db.sh`. The cron hasn't run yet.
**Why it happens:** New cloud deployment — `download-db.sh` has a 5-minute cron interval; DB mtime on a fresh deploy reflects build machine time.
**How to avoid:** Add a 15-minute grace window after racecontrol startup before the probe starts reporting CRITICAL. Alternative: check for the existence of `sync-status.json` — if it exists and is recent, use its timestamp instead of mtime. Decision D3 chose mtime, so the grace window approach is preferred.
**Warning signs:** Probe fires CRITICAL immediately after every deploy.

### Pitfall 5: `mtime.elapsed()` Panics on Clock Skew
**What goes wrong:** `mtime.elapsed().unwrap()` panics if the file mtime is in the future (clock skew between venue and cloud).
**Why it happens:** `SystemTime::elapsed()` returns `Err` when the system time is earlier than the mtime — VM clock drift is common.
**How to avoid:** Use `unwrap_or(Duration::MAX)` as shown in Pattern 3 code example. Or compute `SystemTime::now().duration_since(mtime).unwrap_or(Duration::MAX)`.
**Warning signs:** Sporadic panics in `spawn_blocking` thread caught as `JoinError`.

### Pitfall 6: Venue Guard Logs Flood on Non-Cloud Instance
**What goes wrong:** The guard is applied to high-frequency endpoints and logs a warn on every call on the venue instance (before the early return).
**Why it happens:** Guard is called but `this_instance_is_cloud()` returns false — correct behavior, but if a warn is logged before the early return, it becomes noise.
**How to avoid:** The early returns in `venue_authority_guard` should be silent (no log). Only the 409 rejection path (confirmed cloud write attempt) should log at warn level.

---

## Code Examples

### Complete `venue_authority_guard` (verified against Phase 343 pattern)
```rust
// Source: mirrors crates/racecontrol/src/api/routes.rs:13016
fn venue_authority_guard(state: &AppState, table: &str) -> Option<(StatusCode, Json<Value>)> {
    if !state.config.cloud.enabled {
        return None; // cloud sync not configured — venue writes always allowed
    }
    if !crate::config::this_instance_is_cloud(&state.config) {
        return None; // we are the venue — venue writes always allowed
    }
    if state.config.cloud.is_cloud_authoritative_for(table) {
        return None; // cloud IS authoritative for this table — cloud write allowed
    }
    if crate::config::allow_cloud_venue_write() {
        tracing::warn!(
            "Phase 349: RC_ALLOW_CLOUD_VENUE_WRITE override active — allowing cloud {table} write"
        );
        return None;
    }
    Some((
        StatusCode::CONFLICT,
        Json(json!({
            "error": "venue_authoritative",
            "table": table,
            "hint": "This table is managed by the venue instance. Writes must go to the venue racecontrol.",
            "override_hint": "Emergency: set RC_ALLOW_CLOUD_VENUE_WRITE=1 on the cloud instance and restart."
        })),
    ))
}
```

### Call site pattern (verified from routes.rs:13122 Phase 343 usage)
```rust
async fn create_driver(State(state): State<Arc<AppState>>, Json(body): Json<Value>) -> impl IntoResponse {
    if let Some(rejection) = venue_authority_guard(&state, "drivers") {
        return rejection.into_response();
    }
    // ... existing handler body unchanged
}
```

### New config helper in config.rs (mirrors allow_venue_staff_write at line 319)
```rust
/// Phase 349: Emergency override — allows cloud instance to write venue-authoritative tables.
/// Set RC_ALLOW_CLOUD_VENUE_WRITE=1 on the cloud instance for break-glass scenarios.
pub fn allow_cloud_venue_write() -> bool {
    std::env::var("RC_ALLOW_CLOUD_VENUE_WRITE").as_deref() == Ok("1")
}
```

### subsystem_health.rs run_probes addition
```rust
// In run_probes() — add db_sync_lag to the tokio::join!
let (db_writable, rc_backend, disk_free, cloud_sync, whatsapp_api, fleet_conn, admin_db, db_sync_lag) =
    tokio::join!(
        probe_db_writable(&state.db),
        probe_rc_backend(),
        probe_disk_free(),
        probe_cloud_sync(&state.db),
        probe_whatsapp_api(state),
        probe_fleet_connectivity(state),
        probe_admin_db(),
        probe_db_sync_lag(&state.config),  // Phase 349
    );
// Add to results map:
results.insert("db_sync_lag".to_string(), db_sync_lag);
```

### download-db.sh sentinel check (SYNC-08)
```bash
# SYNC-08: Break-glass pause — add after 'set +a' block, before Step 1
if [ -f "/tmp/DB_SYNC_PAUSED" ]; then
    write_status "paused" "Sync paused by operator sentinel"
    echo "=== SYNC PAUSED (rm /tmp/DB_SYNC_PAUSED to resume) ==="
    exit 0
fi
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Flat `always ok` health endpoint | 7 real subsystem probes (Phase 352) | 2026-04-10 | New probe fits existing extensible pattern |
| No cloud write guard | `cloud_authority_guard` for staff_members (Phase 343) | 2026-04-09 | `venue_authority_guard` is a symmetric extension |
| No DB sync | Google Drive upload/download scripts (Phase 349-01/02) | 2026-04-09 | File mtime now a usable signal for sync lag |

---

## Open Questions

1. **Exact Config field name for db_path**
   - What we know: `default_db_path()` returns `"./data/racecontrol.db"` (config.rs:1205). `Config` struct has nested sub-structs.
   - What's unclear: Is it `config.server.db_path`, `config.database.path`, or `config.db_path`?
   - Recommendation: Implementer reads Config struct definition in config.rs before writing probe. Look for where `default_db_path()` is used as `#[serde(default = "default_db_path")]`.

2. **Whether `run_probes` receives `&AppState` or `Arc<AppState>`**
   - What we know: `probe_db_sync_lag` needs `&Config`. The existing `run_probes` signature (line 91) takes `&AppState`.
   - What's unclear: Whether passing `&state.config` to `spawn_blocking` closure requires a clone (since spawn_blocking requires `'static`).
   - Recommendation: Clone `config.server.db_path` (or whatever the field is) into a `String` before the closure, as done in `check_disk_free_sync` which captures the path by value.

3. **Grace window for new cloud deployments**
   - What we know: D3 chose mtime approach. Fresh deploys will have stale mtime.
   - What's unclear: Whether a startup grace window in the probe or at the probe-task level is preferred.
   - Recommendation: Add a `DB_SYNC_LAG_STARTUP_GRACE_SECS = 900` constant. If `state` startup time (via a global `Instant::now()` at server start) is < 15 min, return ok regardless. Alternatively: probe is cloud-only, and download-db.sh runs every 5 min, so first real sync happens within 5 min of VPS restart. A 15-min grace window in `check_db_sync_lag_sync` (pass startup_instant to the closure) is cleanest.

---

## Environment Availability

Phase 349-03 is pure Rust + one shell script addition. No new external dependencies.

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|---------|
| `std::fs::metadata` | db_sync_lag probe | Built-in | stdlib | — |
| `tokio::task::spawn_blocking` | db_sync_lag probe | Already in Cargo.toml | existing | — |
| `download-db.sh` on Bono VPS | SYNC-08 sentinel | Deployed (Phase 349-02) | current | — |
| `racecontrol.db` path on cloud | db_sync_lag probe | `/root/racingpoint/racecontrol/data/racecontrol.db` | current | Probe returns FILE_NOT_FOUND if missing |

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust `#[cfg(test)]` / `#[test]` inline unit tests |
| Config file | `Cargo.toml` workspace — package `racecontrol-crate` |
| Quick run command | `cargo test -p racecontrol-crate venue_authority` |
| Full suite command | `cargo test -p racecontrol-crate && cargo test -p rc-common` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| SYNC-05 | `venue_authority_guard` returns None when cloud disabled | unit | `cargo test -p racecontrol-crate venue_authority_guard_disabled` | Wave 0 |
| SYNC-05 | `venue_authority_guard` returns None on venue instance | unit | `cargo test -p racecontrol-crate venue_authority_guard_venue` | Wave 0 |
| SYNC-05 | `venue_authority_guard` returns None for cloud-authoritative table | unit | `cargo test -p racecontrol-crate venue_authority_guard_cloud_table` | Wave 0 |
| SYNC-05 | `venue_authority_guard` returns 409 for venue-authoritative table on cloud | unit | `cargo test -p racecontrol-crate venue_authority_guard_409` | Wave 0 |
| SYNC-05 | `allow_cloud_venue_write()` override bypasses guard | unit | `cargo test -p racecontrol-crate allow_cloud_venue_write` | Wave 0 |
| SYNC-06 | `probe_db_sync_lag` returns ok-skip on venue instance | unit | `cargo test -p racecontrol-crate probe_db_sync_lag_venue_skip` | Wave 0 |
| SYNC-06 | `check_db_sync_lag_sync` returns WARN at 300s | unit | `cargo test -p racecontrol-crate db_sync_lag_warn_threshold` | Wave 0 |
| SYNC-06 | `check_db_sync_lag_sync` returns CRITICAL at 900s | unit | `cargo test -p racecontrol-crate db_sync_lag_critical_threshold` | Wave 0 |
| SYNC-06 | `check_db_sync_lag_sync` handles FILE_NOT_FOUND gracefully | unit | `cargo test -p racecontrol-crate db_sync_lag_file_not_found` | Wave 0 |

**Test pattern** — model after existing authority guard tests and subsystem_health tests:

```rust
// Mirrors Phase 343 post_write_verify tests at routes.rs:25334-25377
// and subsystem_health tests at subsystem_health.rs:729-820

#[cfg(test)]
mod tests_349 {
    use super::*;

    #[test]
    fn venue_authority_guard_returns_none_when_cloud_disabled() {
        let mut config = Config::default();
        config.cloud.enabled = false;
        // ... build minimal AppState with this config
        // assert!(venue_authority_guard(&state, "drivers").is_none());
    }

    #[test]
    fn check_db_sync_lag_sync_warn_threshold() {
        // Write a temp file with mtime = 5 minutes ago
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let old_time = std::time::SystemTime::now() - std::time::Duration::from_secs(301);
        filetime::set_file_mtime(tmp.path(), filetime::FileTime::from_system_time(old_time)).unwrap();
        let result = check_db_sync_lag_sync(tmp.path().to_str().unwrap());
        assert!(!result.ok);
        assert_eq!(result.error_code.as_deref(), Some("DB_SYNC_LAG_WARN"));
    }
}
```

NOTE: `filetime` crate is commonly used for setting mtime in tests. Check if it is already in `Cargo.toml` under `[dev-dependencies]`. If not, add `filetime = "0.2"` as a dev dependency. Alternatively, use `std::time::SystemTime::UNIX_EPOCH + Duration::from_secs(...)` and compare elapsed directly in the test.

### Sampling Rate
- **Per task commit:** `cargo test -p racecontrol-crate venue_authority 2>&1 | tail -5`
- **Per wave merge:** `cargo test -p racecontrol-crate && cargo test -p rc-common`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/racecontrol/src/api/routes.rs` (inline `mod tests_349`) — tests for SYNC-05 guard logic (5 tests)
- [ ] `crates/racecontrol/src/subsystem_health.rs` (inline `mod tests`) — tests for SYNC-06 probe (4 tests)
- [ ] `crates/racecontrol/src/config.rs` (inline `mod tests`) — test for `allow_cloud_venue_write()` (1 test)

No new test files needed — all tests are inline in the modules they test (project convention, verified by existing `#[cfg(test)]` blocks in all three files).

---

## Project Constraints (from CLAUDE.md)

These directives apply to Phase 349-03 and the planner must verify compliance:

1. **No `.unwrap()` in production Rust** — use `?`, `.ok()`, or match. Applies to `std::fs::metadata()` and `mtime.elapsed()` calls in the probe.
2. **No fake data** — test must use `tempfile` for real temp DB files, not hardcoded path strings.
3. **Never hold a lock across `.await`** — `probe_db_sync_lag` clones the db_path string before passing to `spawn_blocking` (no locks involved here).
4. **Every `::default()` reviewed** — `Config::default()` used in guard tests is fine since tests only check the guard logic, not real state.
5. **Deploy Manifest Protocol (DMP)** — When this phase ships, the deploy section must list: rust_binary rebuild, cloud redeploy (Bono VPS pm2 restart), shell script update (`download-db.sh` on Bono VPS via SCP). No pod deploy, no frontend rebuild, no venue server binary change (cloud-only feature).
6. **Deploy parity** — the `venue_authority_guard` is cloud-only behavior (early returns on venue), but the binary must be deployed to BOTH venue and cloud. Venue binary just never hits the guard path. Cloud gets the 409 rejection behavior.
7. **Auto-push rule** — git push after commit, LOGBOOK.md update.
8. **ASCII-only scripts** — `RESTORE-DRILL.md` is Markdown, not a script. `download-db.sh` addition is pure ASCII; verify before SCP.

---

## Sources

### Primary (HIGH confidence — direct codebase inspection)
- `crates/racecontrol/src/api/routes.rs:13013-13037` — `cloud_authority_guard()` implementation (Phase 343 template)
- `crates/racecontrol/src/api/routes.rs:13122, 13236, 13347, 13373, 13530` — guard call site pattern
- `crates/racecontrol/src/config.rs:253-321` — `CloudConfig`, `is_cloud_authoritative_for()`, `this_instance_is_cloud()`, `allow_venue_staff_write()`
- `crates/racecontrol/src/subsystem_health.rs:1-550` — full probe infrastructure (7 probes, `spawn_blocking` pattern, `tokio::join!` integration)
- `crates/racecontrol/src/api/routes.rs` (write endpoint enumeration) — 30+ write functions catalogued
- `scripts/db-sync/download-db.sh` — full download script; sentinel insertion point identified
- `.planning/phases/349-db-sync-google-drive/349-CONTEXT.md` — locked decisions D1-D6

### Secondary (MEDIUM confidence)
- `REQUIREMENTS.md` SYNC-05/06/07/08 — requirement text confirmed against decisions
- `STATE.md` — confirmed Phase 349 status: plans 01+02 shipped, 03 pending

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all dependencies verified in Cargo.toml (no new deps needed)
- Architecture: HIGH — guard pattern directly verified against Phase 343 implementation; probe pattern directly verified against Phase 352 implementation
- Pitfalls: HIGH — derived from actual code inspection (config field, guard skip conditions, clock skew pattern)
- Endpoint scope: MEDIUM — enumerated from grep of write functions; handler-to-table mapping inferred from function names (implementer should verify table names in each handler body)

**Research date:** 2026-04-11
**Valid until:** Stable — no external dependencies; valid until routes.rs or subsystem_health.rs undergoes structural refactoring
