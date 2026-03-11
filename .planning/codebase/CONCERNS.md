# RaceControl Codebase — Technical Concerns & Debt

**Last Updated:** 2026-03-11
**Scope:** rc-agent, rc-core, rc-common, pod-agent, PWA/Web frontends

---

## Critical Issues

### 1. Hardcoded Terminal Secret in Frontend ⚠️ SECURITY

**Severity:** CRITICAL
**File:** `/pwa/src/lib/api.ts` (lines 602, 612)
**Issue:** The fallback terminal secret `"rp-terminal-2026"` is hardcoded in the TypeScript source:

```typescript
headers: session ? { "x-terminal-session": session }
  : { "x-terminal-secret": "rp-terminal-2026" },
```

**Risk:**
- Anyone with access to the client bundle can extract the secret
- Acts as a backdoor to the terminal API if session-based auth fails
- Violates principle of least privilege (customers can access terminal)

**Mitigation Needed:**
- Remove hardcoded secret from frontend entirely
- Session-based auth must always be present before using terminal
- Consider API-proxying terminal commands through authenticated backend endpoint
- Rotate this secret in production configuration

---

### 2. Pod-Agent Has No Authentication ⚠️ SECURITY

**Severity:** CRITICAL
**Files:** `/pod-agent/src/main.rs` (entire service)
**Issue:** Pod-agent on port 8090 accepts commands from any source on the LAN (192.168.31.x):
- `/exec` — runs arbitrary shell commands
- `/write` — writes files to pod filesystem
- `/input` — sends keyboard/mouse input
- `/screenshot` — captures screen
- No auth token validation, no mutual TLS, no IP whitelist in code

**Risk:**
- Any pod on the local network can compromise any other pod
- Malicious actor with network access can execute code as SYSTEM
- No audit trail of who issued commands
- Complete system takeover possible

**Mitigation Needed:**
- Implement shared-secret or HMAC authentication in pod-agent
- Add token validation to all endpoints (`/exec`, `/write`, `/input`, `/screenshot`)
- Require signed requests from rc-core (HMAC-SHA256 or similar)
- Log all commands with source IP + timestamp
- Consider mTLS between rc-core and pod-agents
- Add IP whitelist validation (only rc-core server IP)

**Memory Note:** Standing rules mention pod-agent has no auth, flagged as known issue.

---

### 3. Default JWT Secret in Production ⚠️ SECURITY

**Severity:** CRITICAL
**File:** `/crates/rc-core/src/main.rs` (line 94-96), `/crates/rc-core/src/config.rs` (line 310)
**Issue:** Default JWT secret is `"racingpoint-jwt-change-me-in-production"`:

```rust
fn default_jwt_secret() -> String { "racingpoint-jwt-change-me-in-production".to_string() }

if config.auth.jwt_secret == "racingpoint-jwt-change-me-in-production" {
    tracing::warn!("Using default JWT secret! Set auth.jwt_secret in racecontrol.toml for production.");
}
```

**Risk:**
- If production deployment forgets to override in `racecontrol.toml`, any attacker can forge JWTs
- Warning is only logged, not enforced
- No validation that the changed secret is sufficiently strong

**Mitigation Needed:**
- Make JWT secret required (panic if default is used in production mode)
- Reject weak secrets (< 32 bytes, no entropy checks)
- Load from environment variable or secure config vault, never defaults in code
- Consider using the `secrecy` crate to prevent secret leaks in logs/panics

---

## High-Priority Issues

### 4. Insecure Terminal Secret Storage ⚠️ SECURITY

**Severity:** HIGH
**Files:**
- `/crates/rc-core/src/config.rs` (line 62)
- `/crates/rc-core/src/remote_terminal.rs` (line 47)
- `/crates/rc-core/src/cloud_sync.rs` (lines 84-85, 404-405)

**Issue:** Terminal secret is stored in plaintext TOML config file and transmitted in HTTP headers:

```rust
pub terminal_secret: Option<String>,  // From racecontrol.toml
```

And sent as:
```rust
.header("x-terminal-secret", secret)
```

**Risk:**
- Config file visible to anyone with file system access
- Secret transmitted in HTTP headers (could be logged, cached, or intercepted if not HTTPS)
- No encryption at rest
- Cloud sync transmits terminal secret on every request

**Mitigation Needed:**
- Load terminal secret from environment variable only (not TOML)
- Use `secrecy::Secret<String>` type to prevent accidental logging
- Only use HTTPS for cloud sync and terminal requests
- Never log the secret value (redact in logs)
- Consider session-based tokens instead of long-lived secrets

---

### 5. Camera Credentials in Code (Historical) ⚠️ SECURITY

**Severity:** HIGH
**Reference:** Memory note states "Camera credentials (Admin@123) in code" marked as DEPRECATED
**Note:** This appears to be a legacy issue flagged in standing rules. Verify if camera RTSP credentials are still hardcoded in:
- Camera access code (if exists, not found in main search)
- RTSP endpoint connections

**Action:**
- Search for any remaining hardcoded camera credentials
- Move to environment variables or secure storage

---

### 6. SQL Migration Lacks Version Control ⚠️ DATA INTEGRITY

**Severity:** HIGH
**File:** `/crates/rc-core/src/db/mod.rs` (inline SQL)
**Issue:** All database migrations are inline SQL strings in a single `migrate()` function:

```rust
async fn migrate(pool: &SqlitePool) -> anyhow::Result<()> {
    // 400+ lines of raw SQL CREATE TABLE / ALTER TABLE statements
}
```

**Problems:**
- No versioning — can't track which migrations have been applied
- `CREATE TABLE IF NOT EXISTS` works first time but won't handle schema changes
- No rollback mechanism
- Difficult to debug migration failures
- Can't add new columns with defaults safely without data loss
- Multiple concurrent instances might try to migrate simultaneously

**Risk:**
- Breaking schema changes in production without downtime protection
- No way to confirm all instances have same schema
- Lost audit trail of schema evolution

**Mitigation Needed:**
- Implement proper migration versioning (sqlx::migrate! or liquibase-style)
- Use numbered migration files (001_initial.sql, 002_add_column.sql, etc.)
- Add a `schema_version` table to track applied migrations
- Implement mutex to prevent concurrent migrations
- Test migrations against real data before production deploy

---

### 7. Timestamp Normalization in Cloud Sync ⚠️ DATA CONSISTENCY

**Severity:** HIGH
**File:** `/crates/rc-core/src/cloud_sync.rs` (lines 16-26)
**Issue:** ISO timestamps are normalized for comparison:

```rust
fn normalize_timestamp(ts: &str) -> String {
    ts.replace('T', " ")
        .split('+')
        .next()
        .unwrap_or("1970-01-01 00:00:00")
        .trim_end_matches('Z')
        .to_string()
}
```

**Problems:**
- `unwrap_or()` silently defaults to epoch if parsing fails (silently corrupts sync)
- Loses timezone information in conversion
- Character-based string replacement is fragile
- No validation that timestamp format is correct

**Risk:**
- Updated records become "invisible" if sync timestamp comparison breaks
- Data inconsistency between local and cloud instances
- Silent failures (no error, just wrong data)

**Mitigation Needed:**
- Use proper datetime parsing (chrono::DateTime)
- Return error instead of fallback default
- Add validation that converted timestamp is reasonable (not epoch)
- Log conversion failures for debugging
- Unit test with various timestamp formats

---

### 8. Pod-Agent Watchdog May Miss rc-agent Crashes ⚠️ RELIABILITY

**Severity:** HIGH
**File:** `/pod-agent/src/main.rs` (lines 135-140)
**Issue:** Watchdog loop checks every 30 seconds, but rc-agent could crash between checks:

```rust
tokio::spawn(async {
    tokio::time::sleep(Duration::from_secs(WATCHDOG_INTERVAL_SECS)).await;  // 30s
    loop {
        watchdog_ensure_running("rc-agent.exe").await;
        tokio::time::sleep(Duration::from_secs(WATCHDOG_INTERVAL_SECS)).await;
    }
});
```

**Risk:**
- Up to 30 seconds of downtime before detection
- Customer session lost during gap
- Billing timer continues running while agent is dead
- Wheelbase input ignored during downtime

**Context:** Memory notes that watchdog is active on pods, but 30s interval may be too large for venue operations.

**Mitigation Needed:**
- Reduce watchdog interval to 5-10 seconds
- Implement heartbeat-based detection instead of polling
- Consider file modification monitoring (rc-agent executable timestamp)
- Add dead-letter queue if rc-agent crashes repeatedly

---

## Medium-Priority Issues

### 9. No Error Recovery for Failed Cloud Sync ⚠️ RELIABILITY

**Severity:** MEDIUM
**File:** `/crates/rc-core/src/cloud_sync.rs` (line 59-62)
**Issue:** Cloud sync errors are logged but silently skipped:

```rust
loop {
    interval.tick().await;
    if let Err(e) = sync_once(&state, &api_url).await {
        tracing::error!("Cloud sync failed: {}", e);  // Log and continue
    }
}
```

**Problems:**
- Failed sync is not retried or escalated
- No exponential backoff for transient failures
- Missing customers, pricing, or driver data on local copy if cloud is temporarily down
- No alerting to operations team

**Risk:**
- Local and cloud drift indefinitely
- Stale pricing or driver data served to customers
- No visibility into sync health

**Mitigation Needed:**
- Implement retry with exponential backoff (3 attempts, 1s/5s/30s delays)
- Track consecutive failures and alert after 3+ failures
- Broadcast sync status via dashboard WebSocket
- Store last successful sync timestamp and warn if gap > 1 hour

---

### 10. Pod-Monitor and Pod-Healer Lack Coordination ⚠️ RELIABILITY

**Severity:** MEDIUM
**Files:**
- `/crates/rc-core/src/pod_monitor.rs` (Tier 2)
- `/crates/rc-core/src/pod_healer.rs` (Tier 3)

**Issue:** Two independent recovery loops may step on each other:

```rust
// pod_monitor.rs: tries to restart rc-agent via pod-agent
// pod_healer.rs: tries deep diagnostics, also kills/restarts processes
```

**Problems:**
- No coordination between Tier 2 and Tier 3
- Both may attempt same recovery simultaneously (race condition)
- Recovery state not shared (pod_monitor has `PodRecoveryState`, pod_healer has separate logic)
- No deduplication of recovery attempts

**Risk:**
- Conflicting commands sent to same pod
- Recovery state inconsistency
- Unnecessary restarts if both start same fix
- Customer confusion if pod restarts unexpectedly during recovery

**Mitigation Needed:**
- Unify recovery state in AppState (single source of truth)
- Implement recovery attempt locking (only one recovery per pod at a time)
- Pod_monitor marks pod as "recovery_in_progress", pod_healer checks this state
- Add recovery history tracking (last 5 attempts with outcomes)

---

### 11. Billing Timer May Drift if Pod Goes Offline ⚠️ CORRECTNESS

**Severity:** MEDIUM
**File:** `/crates/rc-core/src/billing.rs` (lines 150+)
**Issue:** Billing timer tracks `offline_since` but continues counting:

```rust
pub struct BillingTimer {
    pub driving_seconds: u32,
    pub offline_since: Option<DateTime<Utc>>,  // Tracks when pod went offline
    // But driving_seconds still increments!
}
```

**Problems:**
- If pod is offline for 10 minutes, but `offline_since` is set, timer still counts down
- After pod comes back online, customer gets less time than they paid for
- Memory notes mention "offline auto-end" logic, but unclear if billing is paused or not

**Risk:**
- Billing disputes if customer's session abruptly ends while pod is recovering
- Loss of revenue if timers are paused and not properly resumed
- No clear state machine (is it paused? counting? suspended?)

**Mitigation Needed:**
- Clarify billing semantics when pod goes offline:
  - Option A: Pause billing while offline
  - Option B: Force-end billing after N seconds offline (with refund)
- Implement explicit state (Active, Paused, Suspended, Ended)
- Ensure driving_seconds doesn't increment if paused
- Add detailed logging of state transitions for debugging

---

### 12. Weak Validation in POST Endpoints ⚠️ SECURITY

**Severity:** MEDIUM
**File:** `/crates/rc-core/src/api/routes.rs` (multiple handlers)
**Issue:** POST endpoints lack comprehensive input validation:

```rust
// Example: billing/start endpoint takes pod_id, driver_id, tier_id
// No validation that:
// - pod_id actually exists
// - driver_id is authorized for this pod
// - tier_id is active and not deleted
```

**Problems:**
- No check for deleted/archived entities
- No ownership validation (can a driver bill another driver?)
- No rate limiting on booking or billing start
- Enum/category values not validated against whitelist

**Risk:**
- API can create billing for non-existent pods
- Potential to bill wrong customer
- Malformed requests crash handlers silently
- DoS by spamming with invalid IDs

**Mitigation Needed:**
- Add input validation middleware (deserialize + custom validator)
- Check entity existence before processing
- Validate ownership and authorization
- Implement rate limiting on all POST/PUT endpoints
- Return 400 (Bad Request) with clear error messages, not 500

---

### 13. AI Debugger Fallback Chain Incomplete ⚠️ RELIABILITY

**Severity:** MEDIUM
**File:** `/crates/rc-agent/src/ai_debugger.rs` (lines 78-124)
**Issue:** If Ollama fails and no Anthropic key, analysis is silently skipped:

```rust
if let Some(api_key) = &config.anthropic_api_key {
    // Try Anthropic
} else {
    tracing::warn!("No Anthropic API key configured and Ollama failed");
}
```

**Problems:**
- No local fallback (hard-coded rules, decision tree)
- Crashes are reported but not actioned without AI
- No visibility into why analysis failed (silent warn)
- AI suggestions are optional, so pod health isn't assessed

**Risk:**
- Pod crashes go undiagnosed and unhealed
- Recurring issues not detected
- Customer experience degrades (pod offline, no auto-fix)

**Mitigation Needed:**
- Implement deterministic fallback rules (hardcoded heuristics) when both AI systems fail
- Log specific reason AI was unavailable (network, timeout, no key, etc.)
- Broadcast error to dashboard
- Consider caching previous successful analyses for similar crashes

---

## Low-Priority Issues

### 14. Deprecated Orange Color in Lock Screen ⚠️ BRAND CONSISTENCY

**Severity:** LOW
**File:** `/crates/rc-agent/src/lock_screen.rs` (line ~18)
**Reference:** Memory notes state "OLD orange #FF4400 is DEPRECATED"
**Issue:** Lock screen may still use deprecated color if not updated:

```html
<div style="color:#ff4444;...">Invalid PIN — try again</div>
```

**Note:** The color above appears to be #ff4444 (red), not #FF4400 (orange). Verify no other references to deprecated orange.

**Mitigation:**
- Search codebase for #FF4400 or `#FF4400` patterns
- Ensure all UI uses official Racing Red (#E10600) or approved palette

---

### 15. Hardcoded Subnet in Prompts ⚠️ MAINTAINABILITY

**Severity:** LOW
**Files:**
- `/crates/rc-agent/src/ai_debugger.rs` (line 144)
- `/crates/rc-core/src/ai.rs` (multiple locations)

**Issue:** Network topology (subnet 192.168.31.x) hardcoded in AI prompt templates:

```rust
- 8 pods on subnet 192.168.31.x, server at .51:8080\n\
```

**Problems:**
- Prompt is tightly coupled to venue topology
- If network changes, prompt must be updated
- Not configurable, requires code change + recompile

**Risk:**
- Low — only affects AI context, not functional correctness

**Mitigation Needed:**
- Load subnet info from config
- Template AI prompts with venue network info at runtime
- Store topology in AppState instead of hardcoded

---

### 16. WebSocket Parse Errors Silently Ignored ⚠️ DEBUGGABILITY

**Severity:** LOW
**File:** `/web/src/hooks/useWebSocket.ts` (line 209)
**Issue:** WebSocket message parsing errors are logged but not visible to user:

```typescript
} catch (e) {
  console.warn("[RaceControl] Parse error:", e);
}
```

**Problems:**
- Developer tools only, not visible in UI
- No retry logic if message is malformed
- Metrics don't track parse error frequency

**Risk:**
- Low — doesn't affect critical operations

**Mitigation Needed:**
- Broadcast parse errors to dashboard
- Count and alert if error rate > 5% of messages
- Consider validating schema on both ends (serde for Rust)

---

### 17. Billing Session Split Logic Untested ⚠️ CORRECTNESS

**Severity:** LOW
**File:** `/crates/rc-core/src/billing.rs` (billing split feature)
**Issue:** Billing split feature (multiple sub-sessions) added but no unit tests visible:

```rust
pub split_count: u32,
pub split_duration_minutes: Option<u32>,
pub current_split_number: u32,
```

**Problems:**
- Feature is complex (state transitions, refunds, continuation)
- No tests confirm correct behavior
- Edge cases not covered (e.g., split during refund)

**Risk:**
- Low — feature appears to have happy path working
- Medium risk if splits are commonly used (not yet, per memory notes)

**Mitigation Needed:**
- Add unit tests for split billing scenarios:
  - Create 60-minute session, split into 3x20min
  - Pause and resume between splits
  - Refund a split session
  - End session mid-split
- Document split billing state machine

---

### 18. No Liveness Check for DB Pool ⚠️ RELIABILITY

**Severity:** LOW
**File:** `/crates/rc-core/src/db/mod.rs` (line 11-14)
**Issue:** SQLite pool initialized but no health check:

```rust
let url = format!("sqlite:{}?mode=rwc", db_path);
let pool = SqlitePoolOptions::new()
    .max_connections(5)
    .connect(&url)
    .await?;
```

**Problems:**
- If DB is corrupted, only discovered when first query runs
- No startup validation that DB is readable/writable
- No periodic health checks

**Risk:**
- Low — SQLite is local, unlikely to fail unexpectedly
- Could delay issue detection if DB file is inaccessible

**Mitigation Needed:**
- Add simple `SELECT 1` health check during startup
- Periodically ping DB (every 5 minutes)
- Alert if health check fails (broadcast to dashboard)

---

### 19. AC Server Health Check May Be Too Lenient ⚠️ RELIABILITY

**Severity:** LOW
**File:** `/crates/rc-core/src/ac_server.rs` (health check logic)
**Issue:** AC server health check runs every 5 seconds but criteria unclear

**Problems:**
- Health check implementation not reviewed (file not fully examined)
- May not catch process hung (responding but frozen)
- No heartbeat validation from AC server itself

**Risk:**
- Low — AC server crash is detected eventually

**Mitigation Needed:**
- Verify health check includes:
  - TCP port 8081 responds
  - Process memory usage is reasonable
  - Response time < 2 seconds
  - Lap data updates within last 30 seconds

---

### 20. Missing Metrics and Observability ⚠️ OPERATIONS

**Severity:** LOW
**File:** Entire codebase
**Issue:** No metrics/instrumentation for:
- API response times
- Billing timer accuracy
- Pod recovery success rates
- Cloud sync latency
- Database query times

**Problems:**
- Can't answer: "Which API endpoint is slow?"
- No visibility into pod healer effectiveness
- Billing accuracy can't be verified post-hoc

**Risk:**
- Low — system appears functional
- Medium risk as venue scales (bottlenecks invisible)

**Mitigation Needed:**
- Integrate Prometheus or similar metrics library
- Track key metrics:
  - `rc_core_api_request_duration_seconds` (histogram)
  - `rc_core_billing_timer_accuracy_percent` (gauge)
  - `rc_core_cloud_sync_latency_seconds` (histogram)
  - `rc_agent_crash_count` (counter)
- Expose `/metrics` endpoint
- Visualize in Grafana or similar

---

## Recommendations by Priority

### Immediate (This Week)
1. **Add pod-agent authentication** (Critical) — Shared secret HMAC validation on all endpoints
2. **Remove hardcoded terminal secret from frontend** (Critical) — Use session-only auth
3. **Make JWT secret required** (Critical) — Panic if default used
4. **Coordinate pod-monitor and pod-healer** (High) — Implement recovery state locking

### Short Term (This Month)
5. Implement SQL migration versioning
6. Add cloud sync retry + exponential backoff
7. Reduce pod-agent watchdog interval to 5-10 seconds
8. Clarify and test billing offline/pause semantics
9. Add input validation middleware for POST endpoints

### Medium Term (Q2 2026)
10. Implement metrics/observability (Prometheus)
11. Add billing split unit tests
12. Move camera credentials to environment
13. Implement pod recovery attempt history
14. Refactor cloud sync timestamp handling with proper datetime parsing

### Nice-to-Have
15. Deterministic fallback rules for AI debugger
16. Webhook alerts for error patterns
17. Database health check on startup
18. Schema migration locking for concurrent instances

---

## Known Technical Debt Tracked in Standing Rules

- **Edge stacking fix (80ec001):** Fixed by killing both msedge.exe + msedgewebview2.exe
- **Static CRT build:** Eliminates vcruntime140.dll dependency (✓ implemented)
- **Force-close game on billing end:** TODO in standing rules, not yet implemented
- **USB mass storage lockdown:** TODO via Group Policy
- **Regenerate GitHub PAT:** Done (renewed Mar 9, 2026)

---

## Summary by Category

| Category | Critical | High | Medium | Low |
|----------|----------|------|--------|-----|
| **Security** | 3 | 2 | 1 | 0 |
| **Reliability** | 0 | 2 | 3 | 3 |
| **Data Integrity** | 0 | 1 | 1 | 0 |
| **Correctness** | 0 | 0 | 1 | 1 |
| **Maintainability** | 0 | 0 | 0 | 2 |
| **Operations** | 0 | 0 | 0 | 1 |

**Total Concerns: 20**

---

## Testing Gaps

- **Billing split scenarios:** No unit tests visible
- **Pod recovery coordination:** No integration tests
- **Cloud sync timestamp handling:** Needs regression tests
- **AI debugger fallback chain:** Manual testing only
- **API input validation:** Ad-hoc, no comprehensive coverage

Recommend running `cargo test -p rc-core -p rc-agent -p rc-common` and reviewing test output.

---

## Final Notes

This codebase is functionally solid for a venue management system, with good async/tokio patterns and proper error handling in most places. However, security hardening around pod-agent auth and terminal secrets is critical before production deployment. The data consistency issues (cloud sync, billing offline) need clarification and testing to ensure correctness at scale.

The AI-driven auto-fix system is innovative but requires fallback mechanisms when both Ollama and Anthropic are unavailable.
