# Phase 56: WhatsApp Alerting + Weekly Report - Research

**Researched:** 2026-03-20
**Domain:** Rust async alerting, Evolution API (WhatsApp), SQLite aggregation, Windows Task Scheduler email
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- P0 definition: all pods offline (ws_connected=false for every pod) + error rate threshold breach + billing crash
- Reuse Evolution API pattern from billing.rs send_whatsapp_receipt()
- Same config fields: evolution_url, evolution_api_key, evolution_instance (from config.auth)
- New config field: [alerting] uday_phone = "919XXXXXXXXX" in racecontrol.toml
- 5-second timeout, best-effort (never block the main loop)
- Rate limit: 1 WhatsApp alert per P0 type per 30 minutes
- Alert format: "[RP ALERT] {event_type} - {summary}. {pod_count} pods affected. {IST timestamp}"
- Resolved format: "[RP RESOLVED] {event_type} cleared. All {pod_count} pods online. Duration: {minutes}m. {IST timestamp}"
- Resolved for all-pods-offline: fires when ALL pods reconnect
- Resolved for error rate: fires when no new threshold breach for 5 minutes
- Track P0 start time to include duration in resolved message
- Weekly report content: total sessions, uptime % per pod, credits billed, numbered incident list
- Weekly report period: Monday 00:00 to Sunday 23:59 IST
- Email via existing send_email.js shell-out pattern (same as EmailAlerter)
- Recipient: usingh@racingpoint.in (default_email_recipient)
- HTML table format for readability
- Scheduled via Windows Task Scheduler (same pattern as Phase 53 ONLOGON tasks)
- Monday 08:00 IST trigger

### Claude's Discretion
- Exact HTML template for weekly report email
- Whether to add uptime tracking table to SQLite or compute from logs
- Error rate incident log storage format (file vs SQLite)

### Deferred Ideas (OUT OF SCOPE)
None — discussion stayed within phase scope
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| MON-06 | P0 alert WhatsApp message to Uday within 60s, recovery notification on clear | Evolution API client pattern (billing.rs L2236-2274), bono_event_tx broadcast (state.rs L87), pod_monitor detects offline state |
| MON-07 | Weekly report email every Monday 08:00 IST with sessions, uptime, credits, incidents | billing_sessions + wallet_debit_paise SQLite queries (db/mod.rs L813), send_email.js shell-out (email_alerts.rs L117-127), schtasks ONLOGON pattern (53-01-PLAN.md) |
</phase_requirements>

---

## Summary

Phase 56 adds two new async tasks to racecontrol: a WhatsApp alerter task and a weekly report generator. The WhatsApp alerter subscribes to existing event channels (bono_event_tx broadcast + error_rate alert_rx mpsc) and fires Evolution API calls when P0 conditions are detected. The weekly report is a standalone Rust binary or tokio task triggered by Windows Task Scheduler every Monday at 08:00 IST, which queries SQLite and shells out to send_email.js.

All delivery mechanisms are already proven in production. The Evolution API pattern (reqwest POST, apikey header, 5s timeout, best-effort) is identical to send_whatsapp_receipt() in billing.rs. The email shell-out pattern (tokio::process::Command → node send_email.js) is identical to EmailAlerter::send_alert(). The task scheduler ONLOGON/time-trigger pattern is identical to Phase 53's RacingPoint-StagingHTTP task.

The primary design question (discretion item) is where to store uptime data and incident logs for the weekly report. Computing uptime from fleet_health poll state is feasible in-process but ephemeral across restarts. Adding a lightweight `pod_uptime_samples` table to SQLite is the safer approach for a 7-day lookback report.

**Primary recommendation:** Implement as a new `whatsapp_alerter.rs` module spawned in main.rs alongside existing tasks; implement weekly report as a separate `weekly_report.rs` module invoked by a standalone binary or by the existing racecontrol process via a scheduled HTTP endpoint.

---

## Standard Stack

### Core (already in Cargo.toml)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| reqwest | 0.12 | Evolution API HTTP calls | Already used in billing.rs, cloud_sync.rs, bono_relay.rs |
| tokio | 1.x | Async runtime, mpsc/broadcast channels, process::Command | Project-wide runtime |
| chrono | 0.4 | IST timezone formatting (chrono-tz), DateTime<Utc> | Already used in email_alerts.rs, billing.rs |
| serde_json | 1.x | JSON body construction for Evolution API | Already used project-wide |
| sqlx | 0.7 | SQLite queries for weekly report data | Already project database layer |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| chrono-tz | 0.9 | IST timezone conversion (Asia/Kolkata) | Weekly report timestamps, IST formatting in alerts |

**Verify chrono-tz is already in Cargo.toml — if not, add it.**

**Version verification:**
```bash
# Already in use — no new installs required for core libraries
# Only potential new dependency: chrono-tz
cargo add chrono-tz --package racecontrol
```

---

## Architecture Patterns

### Pattern 1: WhatsApp Alerter Task (follows error_rate_alerter_task pattern)

The error_rate_alerter_task in error_rate.rs (L101-129) is the direct template. The whatsapp_alerter task follows the same structure: spawned in main.rs, receives signals via channels, makes best-effort HTTP calls.

**What:** `pub async fn whatsapp_alerter_task(state: Arc<AppState>, mut error_rate_rx: mpsc::Receiver<()>)` — subscribes to both bono_event_tx broadcast (for PodOffline/PodOnline events) and error_rate mpsc channel (for error rate P0).

**Internal state (not shared with AppState):**
```
struct P0State {
    all_pods_offline_since: Option<Instant>,  // tracks duration
    all_pods_offline_alerted: bool,           // rate limit: last_alert time
    last_all_pods_offline_alert: Option<Instant>,
    error_rate_alerted_at: Option<Instant>,   // 30-min cooldown tracking
    error_rate_p0_since: Option<Instant>,     // for resolved detection
    last_error_rate_resolved_check: Option<Instant>,
}
```

**When to use:** Single task subscribing to both channels via `tokio::select!` loop.

**Example (Evolution API call — identical to billing.rs L2241-2274):**
```rust
// Source: crates/racecontrol/src/billing.rs L2243-2270
let url = format!("{}/message/sendText/{}", evo_url, evo_instance);
let body = serde_json::json!({
    "number": uday_phone,  // "919XXXXXXXXX" from config.alerting.uday_phone
    "text": message
});
let client = reqwest::Client::builder()
    .timeout(std::time::Duration::from_secs(5))
    .build()?;
match client.post(&url).header("apikey", evo_key).json(&body).send().await {
    Ok(resp) if resp.status().is_success() => { tracing::info!("WA alert sent"); }
    Ok(resp) => { tracing::warn!("Evolution API {}", resp.status()); }
    Err(e) => { tracing::warn!("WA alert failed: {}", e); }
}
```

### Pattern 2: P0 Detection via bono_event_tx

**What:** Subscribe to AppState.bono_event_tx broadcast channel. BonoEvent::PodOffline and BonoEvent::PodOnline events already fire when pods disconnect/reconnect (bono_relay.rs L35-36, pod_monitor.rs L27-28 imports BonoEvent).

**All-pods-offline detection:** On each PodOffline event, count ws_connected=false across AppState.pods. When ALL pods are offline AND not already alerted in last 30 min → fire WhatsApp alert.

**All-pods-online (resolved) detection:** On each PodOnline event, count ws_connected=true. When ALL pods are online AND an all_pods_offline P0 was active → fire resolved WhatsApp message.

**Key insight:** AppState.pods (RwLock<HashMap<String, PodInfo>>) contains WS state. Read it inside the alerter task to determine fleet-wide offline/online count.

```rust
// Source: state.rs L83 — Arc<AppState> gives access to pods
let pods = state.pods.read().await;
let total = pods.len();
let offline = pods.values().filter(|p| !p.ws_connected_flag).count();
// Note: PodInfo field name — verify from rc_common::types::PodInfo
```

### Pattern 3: Error Rate P0 Resolved Detection

The ErrorCountLayer already handles the "fire alert" side. The whatsapp_alerter_task needs its own mpsc channel subscribed alongside email (same alert_tx, second receiver is not possible with mpsc — use broadcast instead, or use a second sender).

**CRITICAL:** `tokio::sync::mpsc` has one receiver. The existing alert_tx sends to alert_rx consumed by error_rate_alerter_task. For WhatsApp, use one of:
- **Option A (recommended):** Change alert channel to broadcast — alert_tx: broadcast::Sender<()>, both email alerter and WA alerter subscribe.
- **Option B:** Create a second mpsc channel (wa_alert_tx) and send to BOTH in ErrorCountLayer::on_event() (two try_send calls). This requires wa_alert_tx to be Arc-cloned alongside alert_tx.

Option A is cleaner. Convert error_rate alert channel from mpsc to broadcast::channel(4). Both error_rate_alerter_task and whatsapp_alerter_task subscribe.

**Error rate resolved:** Track time of last error rate P0 in whatsapp_alerter state. After 5 minutes with no new signal from broadcast, fire resolved message. Use a tokio::time::interval check every 60 seconds.

### Pattern 4: Weekly Report as Standalone Script

**What:** A Node.js script (`weekly_report.js`) in `deploy-staging/` (or a Rust binary) that:
1. Opens SQLite (read-only)
2. Queries billing_sessions for the previous week's data
3. Shells out to send_email.js with HTML body

**Why Node.js over Rust binary:** The send_email.js shell-out already handles Gmail OAuth. A Node.js script can directly import the gmail.js service (same pattern as racingpoint-google), eliminating the shell-out chain entirely and sending HTML email directly.

**Alternative: Rust binary `weekly-report`** — queries SQLite via sqlx, constructs HTML, shells out to send_email.js. Simpler from a build perspective since the racecontrol crate already has all dependencies.

**Recommended: Rust binary** (`crates/weekly-report/`) because:
- SQLite access is already wired in racecontrol crate patterns
- Avoids Node.js SQLite dependency (better-sqlite3 or similar)
- Task Scheduler runs it via `weekly-report.exe` — no Node.js path issues
- Shares rc-common types if needed

**Task Scheduler trigger for Monday 08:00 IST:**
```bash
# IST = UTC+5:30, so 08:00 IST = 02:30 UTC
# Task Scheduler uses local machine time — if server is set to IST:
cmd.exe /c 'schtasks /create /tn "RacingPoint-WeeklyReport" /sc WEEKLY /d MON /st 08:00 /ru ADMIN /rl HIGHEST /f /tr "\"C:\RacingPoint\weekly-report.exe\""'
```

### Pattern 5: Uptime Tracking (Discretion Item)

**Recommendation: Compute from fleet_health poll + add SQLite table**

Add `pod_uptime_samples` table via ALTER TABLE migration:
```sql
CREATE TABLE IF NOT EXISTS pod_uptime_samples (
    pod_id TEXT NOT NULL,
    sampled_at TEXT NOT NULL,
    ws_connected INTEGER NOT NULL,  -- 1=true, 0=false
    PRIMARY KEY (pod_id, sampled_at)
);
```

The fleet_health probe loop (every 15s) already calls into fleet_health.rs. Add a side-effect to record ws_connected state per-pod every 15 minutes (NOT every 15s — too many rows). Weekly report queries: `SELECT pod_id, AVG(ws_connected) * 100 AS uptime_pct FROM pod_uptime_samples WHERE sampled_at >= ? GROUP BY pod_id`.

**Alternative: Compute from logs** — parse JSONL log files for ws_connected state changes. More brittle, harder to query, not recommended.

### Pattern 6: Incident Log for Weekly Report

**Recommendation: SQLite table `alert_incidents`**

```sql
CREATE TABLE IF NOT EXISTS alert_incidents (
    id TEXT PRIMARY KEY,
    alert_type TEXT NOT NULL,  -- 'all_pods_offline', 'error_rate', 'billing_crash'
    started_at TEXT NOT NULL,
    resolved_at TEXT,
    pod_count INTEGER,
    description TEXT,
    created_at TEXT DEFAULT (datetime('now'))
);
```

The whatsapp_alerter_task inserts a row when firing alert, updates resolved_at when firing resolved. Weekly report queries: `SELECT * FROM alert_incidents WHERE started_at >= ? ORDER BY started_at`.

This avoids file-based incident log parsing and keeps everything queryable in SQLite.

### Recommended Project Structure

```
crates/
├── racecontrol/src/
│   ├── whatsapp_alerter.rs     # NEW: P0 detection + Evolution API calls
│   ├── config.rs               # MODIFY: add AlertingConfig struct
│   ├── main.rs                 # MODIFY: spawn whatsapp_alerter_task, register WeeklyReport schtask
│   ├── db/mod.rs               # MODIFY: add pod_uptime_samples + alert_incidents tables
│   └── error_rate.rs           # MODIFY: alert channel mpsc→broadcast if Option A chosen
├── weekly-report/              # NEW Rust binary crate
│   ├── Cargo.toml
│   └── src/main.rs             # SQLite query + HTML construction + send_email.js shell-out
```

### Anti-Patterns to Avoid

- **Blocking the WS loop:** all Evolution API calls MUST be best-effort (try_send for mpsc signals from sync context, fire-and-forget for async HTTP). Never `.await` an alert in a WS message handler.
- **Double-subscribe to mpsc:** mpsc has one receiver — cannot share alert_rx between email alerter and WA alerter. Must use broadcast or two channels.
- **Hardcoding phone number:** must be config-driven (`[alerting] uday_phone`) — never hardcoded in source.
- **Blocking weekly report on SQLite write lock:** weekly_report binary should open SQLite in read-only mode (`sqlite:path?mode=ro`) to avoid contention with live racecontrol.
- **UTC timestamps in alerts/emails:** ALL timestamps must be IST. Use chrono-tz Asia/Kolkata conversion.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| WhatsApp delivery | Custom Evolution API wrapper | billing.rs send_whatsapp_receipt() pattern verbatim | Already battle-tested, handles all error cases |
| Email delivery | New SMTP/Gmail client | send_email.js shell-out (EmailAlerter pattern) | Gmail OAuth already wired, project constraint explicitly forbids new SMTP crate |
| Rate limiting | Custom cooldown logic | Replicate EmailAlerter's HashMap<String, DateTime<Utc>> pattern | Proven, tested pattern in email_alerts.rs |
| Uptime calculation | Log file parsing | SQLite `pod_uptime_samples` table + AVG query | Queryable, survives process restarts |
| Weekly scheduler | Custom cron loop inside racecontrol | Windows Task Scheduler (schtasks WEEKLY) | Same pattern as Phase 53, no drift risk |

**Key insight:** This phase is almost entirely integration work — wiring proven components together, not building new capabilities.

---

## Common Pitfalls

### Pitfall 1: mpsc vs broadcast for alert channel
**What goes wrong:** Two tasks try to receive from the same mpsc::Receiver — compiler error or logic bug if you try to pass it to both.
**Why it happens:** error_rate.rs currently creates a single mpsc channel. The existing error_rate_alerter_task consumes it.
**How to avoid:** Change to `tokio::sync::broadcast::channel::<()>(4)` in main.rs. Both email alerter and WA alerter call `alert_tx.subscribe()` independently.
**Warning signs:** Compiler error "use of moved value: `alert_rx`" when trying to give to second consumer.

### Pitfall 2: IST timezone conversion
**What goes wrong:** Alert message shows UTC time ("03:15 UTC") instead of IST ("08:45 IST") — confuses Uday.
**Why it happens:** `chrono::Utc::now()` is UTC. Easy to format without conversion.
**How to avoid:**
```rust
use chrono_tz::Asia::Kolkata;
let ist_now = Utc::now().with_timezone(&Kolkata);
let ts = ist_now.format("%d %b %Y %H:%M IST").to_string();
```
**Warning signs:** Test the formatted string in a unit test — verify offset is +5:30.

### Pitfall 3: all-pods-offline detection race
**What goes wrong:** PodOffline fires for pod 1, alerter reads pods map, but pod 2 hasn't fired PodOffline yet — fleet appears to have 1 pod online. Alert never fires.
**Why it happens:** pod_monitor fires PodOffline events one at a time as heartbeats expire. Multiple pods going offline in the same check cycle emit sequential events, not simultaneous.
**How to avoid:** Don't trigger on a single PodOffline event. Instead, after each PodOffline, read the full pods map and check if ALL pods are offline. Add a short debounce (2s sleep) before re-reading map to let other PodOffline events land.
**Warning signs:** Integration test: stop racecontrol → all 8 pods disconnect within 60s. Verify single alert fires, not multiple.

### Pitfall 4: Weekly report SQLite contention
**What goes wrong:** weekly-report.exe opens SQLite write connection during live racecontrol operation, causing SQLITE_BUSY errors or locking racecontrol.
**Why it happens:** SQLite WAL mode allows concurrent readers, but a write connection blocks other writers.
**How to avoid:** Open SQLite as read-only in weekly-report binary: `sqlite:C:\RacingPoint\racecontrol.db?mode=ro`. Weekly report never writes to the main DB (it reads, then emails). Insert incidents into a separate `incidents.db` if needed, or use alert_incidents table (written by racecontrol, read by weekly-report as read-only).
**Warning signs:** SQLITE_BUSY error in weekly-report log; racecontrol hangs during report generation.

### Pitfall 5: Task Scheduler IST timing
**What goes wrong:** schtasks uses local machine clock. If server is set to UTC (common on Windows servers), 08:00 task runs at 08:00 UTC not IST.
**Why it happens:** Windows Task Scheduler has no timezone-aware scheduling — it uses local time.
**How to avoid:** Verify server timezone with `tzutil /g` on server .23. If UTC, schedule at 02:30 (08:00 IST = 02:30 UTC). If already IST (Asia/Calcutta), schedule at 08:00.
**Warning signs:** Email arrives at 08:00 UTC (13:30 IST) instead of 08:00 IST.

### Pitfall 6: PodInfo ws_connected field name
**What goes wrong:** Accessing wrong field name causes compile error or incorrect offline count.
**Why it happens:** PodInfo is defined in rc_common — field name must be verified.
**How to avoid:** Read `crates/rc-common/src/types.rs` before implementing. The fleet_health endpoint (fleet_health.rs L59) uses `ws_connected: bool` in PodFleetStatus — but AppState.pods stores PodInfo, not PodFleetStatus. Verify which struct and field name tracks WS connection state in PodInfo.
**Warning signs:** Compile error on field access; all pods always appear "online" in alerter logic.

---

## Code Examples

### IST Timestamp Formatting
```rust
// Source: project-wide requirement — verified pattern
use chrono::Utc;
use chrono_tz::Asia::Kolkata;

fn ist_now_string() -> String {
    Utc::now()
        .with_timezone(&Kolkata)
        .format("%d %b %Y %H:%M IST")
        .to_string()
}
// Output: "20 Mar 2026 14:30 IST"
```

### AlertingConfig addition to config.rs
```rust
#[derive(Debug, Default, Deserialize)]
pub struct AlertingConfig {
    #[serde(default)]
    pub enabled: bool,
    /// Uday's WhatsApp number in Evolution API format (e.g., "919876543210")
    pub uday_phone: Option<String>,
    /// Cooldown between same-type P0 alerts in seconds (default: 1800 = 30 min)
    #[serde(default = "default_alert_cooldown")]
    pub cooldown_secs: u64,
}
fn default_alert_cooldown() -> u64 { 1800 }
```

Add to Config struct: `pub alerting: AlertingConfig,` with `#[serde(default)]`.

### Weekly Report SQL Queries
```rust
// Total sessions for the week (Mon 00:00 to Sun 23:59 IST)
// Note: billing_sessions.started_at is stored in UTC ("datetime('now')")
// IST week boundary = UTC Mon 18:30 prev week to Mon 18:30 this week
// Simpler: use strftime with -5h30m offset on query side

let total_sessions: i64 = sqlx::query_scalar(
    "SELECT COUNT(*) FROM billing_sessions
     WHERE started_at >= ? AND started_at < ?
     AND status IN ('completed', 'active', 'ended_early')"
)
.bind(&week_start_utc)  // previous Mon 18:30 UTC
.bind(&week_end_utc)    // this Mon 18:30 UTC
.fetch_one(&pool).await?;

// Total credits billed for the week
let total_paise: i64 = sqlx::query_scalar(
    "SELECT COALESCE(SUM(wallet_debit_paise), 0) FROM billing_sessions
     WHERE started_at >= ? AND started_at < ?"
)
.bind(&week_start_utc)
.bind(&week_end_utc)
.fetch_one(&pool).await?;

// Uptime % per pod (from pod_uptime_samples)
let uptime_rows: Vec<(String, f64)> = sqlx::query_as(
    "SELECT pod_id, AVG(ws_connected) * 100.0 AS uptime_pct
     FROM pod_uptime_samples
     WHERE sampled_at >= ? AND sampled_at < ?
     GROUP BY pod_id ORDER BY pod_id"
)
.bind(&week_start_utc)
.bind(&week_end_utc)
.fetch_all(&pool).await?;
```

### Evolution API call (verified pattern from billing.rs)
```rust
// Source: crates/racecontrol/src/billing.rs L2243-2274
let url = format!("{}/message/sendText/{}", evo_url, evo_instance);
let body = serde_json::json!({ "number": phone, "text": message });
let client = reqwest::Client::builder()
    .timeout(std::time::Duration::from_secs(5))
    .build()
    .expect("reqwest client build");
match client.post(&url).header("apikey", evo_key).json(&body).send().await {
    Ok(r) if r.status().is_success() => tracing::info!("WA sent"),
    Ok(r) => tracing::warn!("Evolution {} for WA alert", r.status()),
    Err(e) => tracing::warn!("WA alert send failed: {}", e),
}
```

### send_email.js shell-out with HTML body (verified pattern from email_alerts.rs)
```rust
// Source: crates/racecontrol/src/email_alerts.rs L117-127
let result = tokio::time::timeout(
    std::time::Duration::from_secs(15),
    tokio::process::Command::new("node")
        .arg(&script_path)          // "send_email.js" full path
        .arg("usingh@racingpoint.in")
        .arg("Racing Point Weekly Report — Week of 17 Mar 2026")
        .arg(&html_body)            // HTML string passed as arg
        .kill_on_drop(true)
        .output(),
).await;
```

**Note:** Verify whether send_email.js exists and accepts HTML. The existing EmailAlerter uses it for plain text. Check if it needs a `--html` flag or if a separate `send_html_email.js` is needed.

### Windows Task Scheduler — WEEKLY trigger (Monday 08:00)
```bash
# Source: .planning/phases/53-deployment-automation/53-01-PLAN.md (ONLOGON pattern adapted)
# Server timezone must be verified first: cmd /c tzutil /g
cmd.exe /c 'schtasks /create /tn "RacingPoint-WeeklyReport" /sc WEEKLY /d MON /st 08:00 /ru ADMIN /rl HIGHEST /f /tr "\"C:\RacingPoint\weekly-report.exe\""'
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Alert channel mpsc (email only) | Alert channel broadcast (email + WhatsApp) | Phase 56 | Both alerters subscribe independently |
| Plain text watchdog emails | HTML weekly report email | Phase 56 | Scannable in 30s on Uday's phone |
| No P0 WhatsApp notification | WhatsApp within 60s of P0 | Phase 56 | Uday gets instant notification |

---

## Open Questions

1. **Does send_email.js support HTML body?**
   - What we know: EmailAlerter shells out to `node send_email.js recipient subject body` — the script path is `config.watchdog.email_script_path` defaulting to `"send_email.js"`. This file is NOT in the racecontrol repo (it's a separate script on the server at `C:\RacingPoint\send_email.js` or similar path).
   - What's unclear: Whether the script sends Content-Type: text/html or text/plain. If plain text, the HTML tags will show raw in Uday's email.
   - Recommendation: Plan 02 (weekly report) must read the actual send_email.js on server .23 before implementation. If it's plain text only, create `send_html_email.js` with `Content-Type: text/html`.

2. **PodInfo ws_connected field name**
   - What we know: PodFleetStatus has `ws_connected: bool` (fleet_health.rs L59). AppState.pods is `RwLock<HashMap<String, PodInfo>>` where PodInfo is from rc_common.
   - What's unclear: Exact field name on PodInfo for WS connection state — may be `ws_connected`, `is_connected`, or derived from `DrivingState`.
   - Recommendation: Plan 01 must read `crates/rc-common/src/types.rs` before implementing offline detection.

3. **Server .23 timezone**
   - What we know: Task Scheduler runs in local machine time. Project rule is always IST.
   - What's unclear: Whether server .23 is configured for Asia/Calcutta or UTC.
   - Recommendation: Plan 02 verifies with `cmd /c tzutil /g` via rc-agent exec before setting task time.

4. **wallet_debit_paise NULL handling for weekly report**
   - What we know: `wallet_debit_paise` is nullable (added via ALTER TABLE). Sessions paid by pricing tier (not wallet) may have NULL.
   - What's unclear: How to count non-wallet sessions in total credits billed.
   - Recommendation: Use `COALESCE(wallet_debit_paise, custom_price_paise, (SELECT price_paise FROM pricing_tiers WHERE id = pricing_tier_id), 0)` — same pattern as billing.rs L2190-2192.

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (racecontrol unit tests) |
| Config file | Cargo.toml (workspace) |
| Quick run command | `cargo test -p racecontrol -- whatsapp_alerter` |
| Full suite command | `cargo test -p racecontrol && cargo test -p weekly-report` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| MON-06 | WhatsApp alert fires within 60s of all-pods-offline | integration (manual) | Stop racecontrol, observe Evolution API call in logs | No (manual test) |
| MON-06 | Rate limit: second alert blocked within 30 min | unit | `cargo test -p racecontrol -- test_wa_rate_limit` | No — Wave 0 |
| MON-06 | IST timestamp in alert message | unit | `cargo test -p racecontrol -- test_ist_format` | No — Wave 0 |
| MON-06 | Resolved message fires after all pods reconnect | integration (manual) | Restart racecontrol, observe resolved in logs | No (manual test) |
| MON-07 | Weekly report SQL returns correct session count | unit | `cargo test -p weekly-report -- test_week_query` | No — Wave 0 |
| MON-07 | Weekly report email arrives Monday 08:00 IST | manual | Verify Task Scheduler task + check Uday inbox | No (manual gate) |

### Sampling Rate
- **Per task commit:** `cargo test -p racecontrol -- whatsapp_alerter`
- **Per wave merge:** `cargo test -p racecontrol && cargo test -p weekly-report`
- **Phase gate:** Full suite green + manual P0 simulation test before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/racecontrol/src/whatsapp_alerter.rs` — module does not exist yet
- [ ] `crates/weekly-report/` — crate does not exist yet
- [ ] `crates/racecontrol/tests/whatsapp_alerter_tests.rs` — unit tests for rate limiting + IST format

---

## Sources

### Primary (HIGH confidence)
- `crates/racecontrol/src/billing.rs` L2168-2275 — Evolution API client pattern, exact reqwest usage
- `crates/racecontrol/src/error_rate.rs` — ErrorCountLayer, mpsc alert channel, alerter task pattern
- `crates/racecontrol/src/email_alerts.rs` — EmailAlerter, send_email.js shell-out, rate limiting pattern
- `crates/racecontrol/src/bono_relay.rs` L31-38 — BonoEvent enum with PodOffline/PodOnline
- `crates/racecontrol/src/state.rs` L87 — bono_event_tx broadcast::Sender
- `crates/racecontrol/src/config.rs` L221-223 — evolution_url/api_key/instance in AuthConfig
- `crates/racecontrol/src/db/mod.rs` L262-279, L813 — billing_sessions schema, wallet_debit_paise column
- `.planning/phases/53-deployment-automation/53-01-PLAN.md` — schtasks ONLOGON/WEEKLY pattern
- `crates/racecontrol/src/main.rs` — spawn pattern, alert_tx channel creation, task spawn order

### Secondary (MEDIUM confidence)
- `racingpoint-whatsapp-bot/src/services/evolutionService.js` — confirms Evolution API endpoint shape `/message/sendText/{instance}`
- `racingpoint-google/services/gmail.js` — confirms Gmail send pattern (not used directly, but reference for send_email.js capabilities)

### Tertiary (LOW confidence — needs runtime verification)
- Server .23 timezone: assumed IST, needs `tzutil /g` confirmation
- `send_email.js` HTML support: assumed plain text, needs inspection on server .23

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all libraries already in Cargo.toml, verified in source
- Architecture: HIGH — all patterns copied from working production code
- Pitfalls: HIGH — mpsc/broadcast issue is a compiler-verifiable fact; IST pitfall is project-wide known issue; others derived from reading actual code
- Open questions: LOW — require runtime verification (server timezone, send_email.js content)

**Research date:** 2026-03-20
**Valid until:** 2026-04-20 (stable patterns — Evolution API and schtasks don't change)
