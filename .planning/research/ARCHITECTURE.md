# Architecture Research

**Domain:** RC Sentry AI Debugger — crash diagnostics as external watchdog in rc-sentry
**Researched:** 2026-03-21
**Confidence:** HIGH (based on direct codebase inspection of rc-sentry/src/main.rs, rc-agent/src/ai_debugger.rs, rc-agent/src/startup_log.rs, rc-common/src/protocol.rs, racecontrol/src/fleet_health.rs)

---

## Existing Architecture Context

Understanding the current system is essential before describing what changes.

**Deployment topology per pod:**

```
Pod (Windows 11, 192.168.31.x)
  rc-agent.exe         port 8090   Axum HTTP + WebSocket client to racecontrol
  rc-sentry.exe        port 8091   Pure std::net TCP, no tokio, no async
```

**rc-sentry current state (v11.0):**
- Single-threaded std::net TCP server with per-connection threads
- 6 endpoints: /ping, /exec, /health, /version, /files, /processes
- No tokio, no async — explicit design choice for minimal deps and crash isolation
- 4-slot concurrency cap via AtomicUsize SlotGuard
- 64KB body truncation, 30s timeouts
- Reads rc-common::exec::run_cmd_sync for /exec
- Runs as a Windows service, independent of rc-agent lifecycle

**rc-agent current state (decomposed in v11.0):**
- `ai_debugger.rs` — 4-tier debug: deterministic fixes, pattern memory, Ollama, cloud Claude
- `startup_log.rs` — writes `C:\RacingPoint\rc-agent-startup.log` per boot; detect_crash_recovery() reads it
- `self_heal.rs` — repairs config, start script, registry key on every boot
- `failure_monitor.rs` — detects consecutive crash events
- `debug-memory.json` — persisted DebugMemory struct (pattern key → fix mapping)

**The fundamental problem:** ai_debugger.rs lives inside rc-agent. When rc-agent crashes, the debugger dies with it. rc-sentry survives the crash and can read logs, run commands, and restart the agent — making it the correct host for post-crash diagnostics.

---

## System Overview: v11.2 Integration

```
+----------------------------------------------+
|  Pod (192.168.31.x)                          |
|                                              |
|  +------------------------------------------+
|  |  rc-sentry.exe  (port 8091)               |
|  |                                            |
|  |  [existing: TCP accept loop, main thread]  |
|  |                                            |
|  |  [NEW: watchdog_thread]                    |
|  |   spawn at startup via std::thread::spawn  |
|  |   |                                        |
|  |   +-> poll GET localhost:8090/health (5s)  |
|  |   |   using std::net::TcpStream            |
|  |   |                                        |
|  |   +-> on crash detected:                   |
|  |       1. read_crash_logs()                 |
|  |          - rc-agent-startup.log            |
|  |          - rc-agent-stderr.log             |
|  |       2. load_debug_memory()               |
|  |          - debug-memory.json               |
|  |       3. Tier 1: run_deterministic_fixes() |
|  |          - kill_zombie_rcagent()           |
|  |          - clean_stale_sockets()           |
|  |          - repair_config()                 |
|  |          - clear_shader_cache()            |
|  |       4. Tier 2: instant_fix() from memory |
|  |       5. Tier 3: query_ollama()            |
|  |          - POST James .27:11434            |
|  |       6. restart_rcagent()                 |
|  |          - run start-rcagent.bat via exec  |
|  |       7. report_to_server()               |
|  |          - HTTP POST .23:8080/api/v1/...   |
|  |          - fire-and-forget (no WS)         |
|  +------------------------------------------+
|                                              |
|  [rc-agent.exe -- may be crashed]            |
|   port 8090  (polled by watchdog)            |
+----------------------------------------------+
         |                        |
         | HTTP (LAN)             | HTTP POST
         v                        v
+----------------+      +----------------------+
| James .27:11434|      | racecontrol :8080    |
| Ollama         |      | (new crash endpoint) |
| qwen3:0.6b     |      | fleet_health.rs      |
+----------------+      +----------------------+
```

---

## Component Responsibilities

| Component | Location | Status | Responsibility |
|-----------|----------|--------|----------------|
| `watchdog_thread` | `rc-sentry/src/main.rs` | NEW | Background std::thread. Polls rc-agent health, drives crash detection FSM, sequences fix tiers, restarts agent |
| `crash_logs.rs` | `rc-sentry/src/crash_logs.rs` | NEW MODULE | Read startup_log, stderr, panic output from known paths. Returns structured CrashContext |
| `debug_memory.rs` | `rc-sentry/src/debug_memory.rs` | NEW MODULE | Load/save/query `debug-memory.json`. Port of DebugMemory from rc-agent — same file path, compatible JSON |
| `tier1_fixes.rs` | `rc-sentry/src/tier1_fixes.rs` | NEW MODULE | Deterministic fixes: kill zombie rc-agent, clean stale sockets, repair config, clear shader cache |
| `ollama.rs` | `rc-sentry/src/ollama.rs` | NEW MODULE | HTTP POST to James .27:11434/api/generate. Blocking std::net call, no tokio |
| `fleet_reporter.rs` | `rc-sentry/src/fleet_reporter.rs` | NEW MODULE | HTTP POST crash diagnostic report to racecontrol. Blocking call, fire-and-forget |
| `rc-agent/ai_debugger.rs` | `rc-agent/src/ai_debugger.rs` | UNCHANGED | Game-crash debugging (process alive). Remove agent-crash paths only if they duplicate sentry |
| `protocol.rs` (rc-common) | `rc-common/src/protocol.rs` | MODIFIED | Add `AgentMessage::SentryCrashReport` variant — sent via HTTP from sentry, not WS |
| `fleet_health.rs` (racecontrol) | `racecontrol/src/fleet_health.rs` | MODIFIED | New HTTP endpoint + store field for sentry crash reports |
| `types.rs` (rc-common) | `rc-common/src/types.rs` | MODIFIED | New `SentryCrashReport` type, `CrashDiagResult` enum |

---

## Recommended Project Structure

New files in rc-sentry:

```
crates/rc-sentry/src/
├── main.rs                    MODIFIED — spawn watchdog_thread at startup
├── crash_logs.rs              NEW — read startup_log + stderr from known paths
├── debug_memory.rs            NEW — DebugMemory load/save/query (sentry-side mirror)
├── tier1_fixes.rs             NEW — deterministic fix functions
├── ollama.rs                  NEW — blocking HTTP to Ollama (std::net, no tokio)
└── fleet_reporter.rs          NEW — blocking HTTP POST to racecontrol
```

Modified files in rc-common:

```
crates/rc-common/src/
├── types.rs                   + SentryCrashReport struct, CrashDiagResult enum
└── protocol.rs                + SentryCrashReport type used by racecontrol endpoint
```

Modified files in racecontrol:

```
crates/racecontrol/src/
├── fleet_health.rs            + FleetHealthStore::last_crash_report field
│                              + POST /api/v1/sentry/crash handler
└── api/routes.rs              + register new POST /api/v1/sentry/crash route
```

No new crate. No new binary. No tokio added to rc-sentry.

---

## Architectural Patterns

### Pattern 1: std::thread Watchdog (not tokio)

**What:** rc-sentry has no tokio runtime — deliberate. The watchdog is a `std::thread::spawn` loop, sleeping 5s between polls. Health checks use `std::net::TcpStream::connect_timeout` + manual HTTP write/read.

**When to use:** This is the only option given the constraint "no new crate dependencies" and the existing no-tokio design. Adding tokio to rc-sentry would require a full Cargo.toml change and binary size increase.

**Trade-offs:**
- Pro: Matches existing rc-sentry threading model perfectly. No runtime coordination.
- Pro: Watchdog thread failure is isolated — does not affect the TCP accept loop or request handling.
- Con: Blocking HTTP calls in the watchdog thread are acceptable (it's the only work that thread does). Not a concern here.

**Key implementation detail:** Use `AtomicBool WATCHDOG_SHUTDOWN` (same pattern as existing `SHUTDOWN_REQUESTED`) so the main loop can signal the watchdog to exit cleanly on Ctrl+C.

```rust
// In main():
let _watchdog = std::thread::Builder::new()
    .name("sentry-watchdog".into())
    .spawn(|| watchdog_loop())
    .expect("watchdog thread");
```

### Pattern 2: Health Poll FSM (hysteresis before crash declaration)

**What:** A simple 3-state FSM — `Healthy`, `Suspect(consecutive_fail_count)`, `Crashed`. Declare crash only after N consecutive poll failures (N=3, giving 15s window). This avoids false positives from brief rc-agent pauses during game launch or shader compilation.

**When to use:** Required. A single missed health check must not trigger diagnostics.

**Trade-offs:**
- 3 failures × 5s = 15s crash declaration latency. Acceptable — self-healing is not real-time.
- State lives in the watchdog thread local variables (not shared atomic) — no contention.

```
[Healthy]
  poll OK -> stay Healthy
  poll FAIL -> transition to Suspect(1)

[Suspect(n)]
  poll OK -> transition to Healthy
  poll FAIL, n < 3 -> Suspect(n+1)
  poll FAIL, n == 3 -> transition to Crashed -> run_diagnostics()

[Crashed]
  After restart + verify OK -> transition to Healthy
  After restart + N retries failed -> escalate (log + report, do not loop forever)
```

### Pattern 3: Crash Log Collection from Known Paths

**What:** rc-sentry reads logs from hardcoded paths that rc-agent writes to. No inter-process communication — pure file reads after rc-agent is confirmed dead.

**Known log paths (from codebase inspection):**
- `C:\RacingPoint\rc-agent-startup.log` — written by `startup_log.rs::write_phase()`
- `C:\RacingPoint\rc-agent-stderr.log` — if start-rcagent.bat redirects stderr (currently it does not — add `2>> rc-agent-stderr.log` to the bat script as part of this milestone)
- `C:\RacingPoint\debug-memory.json` — DebugMemory JSON, written by ai_debugger.rs

**When to use:** Always read all three before running fixes. Logs are small (startup log: ~20 lines per boot). Reading is fast and cheap.

**Trade-offs:**
- Requires bat file change to capture stderr. One-time change, captured in self_heal.rs so it self-repairs.
- startup_log.rs truncates on each new rc-agent startup — sentry must read it while rc-agent is still down.

### Pattern 4: Shared debug-memory.json (sentry reads, agent writes)

**What:** `debug-memory.json` is currently written by rc-agent's `ai_debugger.rs::DebugMemory::save()`. rc-sentry gains a read-only port of `DebugMemory` — loads the same file, runs `instant_fix()` to get a cached fix suggestion. Sentry also writes back after successful fixes (updating `success_count` and `last_seen`).

**When to use:** Tier 2 lookup. After Tier 1 deterministic fixes run and before Ollama query.

**Trade-offs:**
- File-based sharing is safe because rc-agent is crashed (not running) when sentry reads.
- Sentry and agent never write simultaneously — sentry only writes during the crash window.
- JSON format is compatible — sentry uses the same `DebugMemory` struct (copy, not shared dep, since rc-sentry avoids rc-common's tokio feature).

**Concrete implementation:** Copy the `DebugMemory`, `DebugIncident` struct definitions into `debug_memory.rs`. Do not import from rc-agent (cross-crate dependency). The structs are ~50 lines of pure serde — acceptable duplication to avoid pulling in rc-agent's tokio dependency chain.

### Pattern 5: Blocking HTTP for Ollama (std::net, no reqwest)

**What:** Ollama query uses raw `std::net::TcpStream` + hand-written HTTP/1.1 POST, same as rc-sentry's existing `handle_exec` plumbing. No reqwest, no tokio.

**When to use:** Only when Tier 1 and Tier 2 fail. Ollama is at James .27:11434 — LAN hop, low latency, but may be unavailable.

**Trade-offs:**
- 30s timeout on TcpStream covers slow model inference. qwen3:0.6b responds in under 2s typically.
- If James is unreachable (network partition), the function returns `Err` and sentry proceeds to restart without AI suggestion. Never blocks the restart indefinitely.
- Prompt: short crash context (last 5 lines of startup_log + stderr). Same structure as rc-agent's `query_ollama()`.

### Pattern 6: Fleet Report via HTTP (not WebSocket)

**What:** After diagnostics, rc-sentry POSTs a crash report directly to racecontrol's HTTP API. rc-sentry has no WebSocket and no rc-agent state — HTTP is the only option.

**Endpoint:** `POST http://192.168.31.23:8080/api/v1/sentry/crash`

**Payload structure (`SentryCrashReport` in rc-common::types):**
```
pod_id: String
crash_detected_at: String  (RFC3339)
last_startup_log_lines: Vec<String>
stderr_tail: Vec<String>
tier1_fixes_applied: Vec<String>
tier1_success: bool
tier2_instant_fix: Option<String>
tier3_ollama_suggestion: Option<String>
restart_attempted: bool
restart_verified: bool
sentry_version: String
```

**Trade-offs:**
- Fire-and-forget with 10s timeout. If racecontrol is unreachable, log locally and proceed with restart. Never block restart on report delivery.
- No auth required — LAN-only endpoint, same as all other internal HTTP calls in this codebase.

---

## Data Flow

### Normal Operation (rc-agent healthy)

```
watchdog_loop
  every 5s: HTTP GET localhost:8090/health
  response 200 -> reset FSM to Healthy, sleep 5s, repeat
```

### Crash Detection and Response Flow

```
watchdog_loop
  poll FAIL x3 (15s elapsed)
  -> declare Crashed
  -> collect_crash_context():
       read C:\RacingPoint\rc-agent-startup.log   (startup_log.rs output)
       read C:\RacingPoint\rc-agent-stderr.log    (stderr redirect, new)
       parse last_phase, panic_line, error_pattern
  -> load_debug_memory():
       read C:\RacingPoint\debug-memory.json
       -> DebugMemory { incidents: [...] }
  -> Tier 1: run_deterministic_fixes(crash_context):
       kill_zombie_rcagent()    taskkill /F /IM rc-agent.exe
       clean_stale_sockets()    netsh int ip reset or TCPView parse
       repair_config()          re-write rc-agent.toml from embedded template
       clear_shader_cache()     delete C:\Users\...\shader cache dirs
       -> Vec<AutoFixResult>
  -> Tier 2: debug_memory.instant_fix(pattern_key):
       if known pattern -> apply cached suggestion text
  -> Tier 3 (only if Tier 1+2 insufficient):
       query_ollama(crash_context, James .27:11434)
       -> Option<String> (AI suggestion)
  -> restart_rcagent():
       run_cmd_sync("start-rcagent.bat", 30s)
       wait 10s
       poll health again to verify restart
  -> update debug_memory if fix succeeded (write back to debug-memory.json)
  -> fleet_reporter::post_crash_report(SentryCrashReport):
       HTTP POST .23:8080/api/v1/sentry/crash
       10s timeout, fire-and-forget
  -> if restart_verified: transition FSM to Healthy
  -> if not verified after 3 attempts: log + report ESCALATION, stop restart loop
```

### Server-Side Report Ingestion

```
racecontrol receives POST /api/v1/sentry/crash
  -> deserialize SentryCrashReport
  -> FleetHealthStore::record_crash_report(pod_id, report)
     (new field: last_sentry_crash: Option<SentryCrashReport>)
  -> broadcast DashboardEvent::PodCrashDiagnostic to staff kiosk
     (existing WS broadcast infrastructure)
  -> if !restart_verified: WhatsApp alert via whatsapp_alerter.rs
```

---

## Integration Points

### New in rc-common/src/types.rs

```rust
pub struct SentryCrashReport {
    pub pod_id: String,
    pub crash_detected_at: String,
    pub last_startup_log_lines: Vec<String>,
    pub stderr_tail: Vec<String>,
    pub tier1_fixes_applied: Vec<String>,
    pub tier1_success: bool,
    pub tier2_instant_fix: Option<String>,
    pub tier3_ollama_suggestion: Option<String>,
    pub restart_attempted: bool,
    pub restart_verified: bool,
    pub sentry_version: String,
}

pub enum CrashDiagResult {
    FixedAndRestarted,
    RestartedWithoutFix,
    RestartFailed { attempts: u8 },
    EscalationRequired,
}
```

### New HTTP Endpoint in racecontrol

| Endpoint | Method | Auth | Purpose |
|----------|--------|------|---------|
| `/api/v1/sentry/crash` | POST | None (LAN-only) | Receive crash diagnostic report from rc-sentry |

### Modified racecontrol/src/fleet_health.rs

Add to `FleetHealthStore`:
```rust
pub last_sentry_crash: Option<SentryCrashReport>
pub last_sentry_crash_at: Option<DateTime<Utc>>
```

Existing `fleet_health_handler` response already serializes the store — the new fields will appear in `GET /api/v1/fleet/health` automatically.

### rc-agent/src/ai_debugger.rs — NO CHANGE

Game-crash debugging remains in rc-agent. The distinction is clean:
- rc-agent ai_debugger.rs: agent is alive, game crashed (process disappeared)
- rc-sentry watchdog: agent is crashed (health poll fails)

Do not touch ai_debugger.rs in this milestone. The only coordination point is the shared `debug-memory.json` file, which sentry reads via its own `debug_memory.rs` module.

### rc-agent/src/startup_log.rs — NO CHANGE

startup_log.rs already writes to `C:\RacingPoint\rc-agent-startup.log` per the existing implementation. rc-sentry reads this path directly — no API needed.

### start-rcagent.bat — MINOR CHANGE

Add stderr redirect so rc-sentry can read crash output:
```bat
start "" /D C:\RacingPoint rc-agent.exe 2>> C:\RacingPoint\rc-agent-stderr.log
```

self_heal.rs `START_SCRIPT_CONTENT` constant in rc-agent must be updated to match — otherwise self-heal will revert the bat file to the old version without stderr capture.

---

## Build Order

Dependencies determine the sequence. rc-common compiles first (both binaries import it). racecontrol endpoint must be live before rc-sentry tries to POST reports. rc-sentry watchdog can be tested in isolation before racecontrol receives its reports.

### Phase 1: Types Foundation (rc-common)

Add to `rc-common/src/types.rs`:
- `SentryCrashReport` struct
- `CrashDiagResult` enum

**Reason first:** Both racecontrol (for the HTTP endpoint) and rc-sentry (for the fleet reporter) need these types. rc-sentry currently does not depend on rc-common for any types — adding this dependency is acceptable and consistent with the existing `run_cmd_sync` usage.

Verify: `cargo test -p rc-common` passes.

### Phase 2: Sentry Watchdog Core (rc-sentry)

Create in order within `crates/rc-sentry/src/`:

1. `crash_logs.rs` — `collect_crash_context()`, pure file reads, fully testable with temp files
2. `debug_memory.rs` — DebugMemory load/save/query, testable with temp dir
3. `tier1_fixes.rs` — fix functions with `#[cfg(test)]` guards (same discipline as ai_debugger.rs)
4. `ollama.rs` — blocking HTTP POST to Ollama, 30s timeout, graceful fail on unreachable
5. `fleet_reporter.rs` — blocking HTTP POST, 10s timeout, fire-and-forget

Then modify `main.rs`:
- Add `WatchdogState` enum (Healthy, Suspect(u8), Crashed)
- Add `watchdog_loop()` function
- Spawn watchdog thread in `main()` before TCP accept loop
- Add `WATCHDOG_SHUTDOWN` AtomicBool, signal it in `ctrl_handler`

**Reason second:** All new sentry modules are self-contained. `crash_logs.rs` and `debug_memory.rs` can be tested without a running rc-agent. `tier1_fixes.rs` uses `#[cfg(test)]` guards so tests never fire real taskkill. Build and run `cargo test -p rc-sentry` before deploying.

### Phase 3: Server Endpoint (racecontrol)

1. Modify `fleet_health.rs` — add `last_sentry_crash` fields to `FleetHealthStore`
2. Create handler for `POST /api/v1/sentry/crash` (inline in `fleet_health.rs` or separate handler file)
3. Register route in `api/routes.rs`
4. Add WS broadcast of `DashboardEvent::PodCrashDiagnostic` (new variant or reuse existing event type)
5. Add escalation path to `whatsapp_alerter.rs` for `restart_verified = false`

Build and deploy to server .23. Verify endpoint with:
```
curl -X POST http://192.168.31.23:8080/api/v1/sentry/crash \
  -H "Content-Type: application/json" \
  -d '{"pod_id":"pod1","crash_detected_at":"...","tier1_fixes_applied":[],...}'
```

**Reason third:** racecontrol must be live before rc-sentry tries to POST reports. Deploy server first, test endpoint independently.

### Phase 4: rc-agent bat file change

Update `self_heal.rs` `START_SCRIPT_CONTENT` to add `2>> C:\RacingPoint\rc-agent-stderr.log`.
Build rc-agent, deploy to Pod 8, verify stderr log file appears at `C:\RacingPoint\rc-agent-stderr.log`.

**Reason fourth:** This is a simple one-line change with low risk. Deploy after the sentry watchdog is ready so the first crash test can read the stderr log immediately.

### Phase 5: Integration Test on Pod 8 (canary)

Deploy updated rc-sentry to Pod 8. Simulate crash:
```
# Via sentry /exec:
taskkill /F /IM rc-agent.exe
```
Observe:
- Watchdog declares crash within 15s
- Crash logs read from `C:\RacingPoint\`
- Tier 1 fixes run (check sentry stdout/tracing)
- Restart-rcagent.bat invoked
- Health poll confirms rc-agent back up
- `GET http://192.168.31.23:8080/api/v1/fleet/health` shows `last_sentry_crash` populated

Then roll to remaining 7 pods.

---

## Anti-Patterns

### Anti-Pattern 1: Adding tokio to rc-sentry for async watchdog

**What people do:** Import tokio and spawn an `async fn watchdog_loop()` to use `tokio::time::interval` and `reqwest`.

**Why it's wrong:** rc-sentry deliberately avoids tokio. Adding it increases binary size, adds a dep not in the existing Cargo.toml, and violates the constraint "no new crate dependencies." The existing std::thread + AtomicBool pattern handles shutdown cleanly.

**Do this instead:** `std::thread::spawn` + `std::net::TcpStream` for health polls and Ollama calls. Same blocking model already used in every handler in rc-sentry.

### Anti-Pattern 2: Sharing rc-agent's DebugMemory via rc-common

**What people do:** Move `DebugMemory` from `rc-agent/src/ai_debugger.rs` into `rc-common/src/types.rs` so rc-sentry can import it.

**Why it's wrong:** `ai_debugger.rs` imports `tokio::sync::mpsc` and `crate::ffb_controller::FfbController`. Moving DebugMemory to rc-common would pull these into rc-common's public surface. rc-sentry does not use tokio. The structs are 50 lines of pure serde — copy them.

**Do this instead:** Define a minimal `DebugMemory` + `DebugIncident` in `rc-sentry/src/debug_memory.rs` — same JSON shape, compatible with the existing file on disk. rc-sentry reads and writes; rc-agent reads and writes. File locking is not needed (they never run concurrently during the crash window).

### Anti-Pattern 3: Blocking the restart on Ollama response

**What people do:** `query_ollama()` with no timeout, waiting for model inference before restarting rc-agent.

**Why it's wrong:** qwen3:0.6b on James is shared. If James is under load or unreachable, the pod stays down indefinitely. The customer session is interrupted for the duration of the AI query.

**Do this instead:** Hard 30s timeout on the TcpStream. If Ollama does not respond within 30s, proceed to restart with the best available fix from Tiers 1+2. Record the Ollama attempt as `None` in the crash report. The restart never waits on inference.

### Anti-Pattern 4: Restarting rc-agent in a tight loop on repeated crashes

**What people do:** After each failed restart (health still failing), immediately retry with a short sleep.

**Why it's wrong:** If rc-agent crashes on startup due to a config error or missing binary, a tight loop hammers the disk and CPU, potentially making recovery harder. It also masks the real problem from staff.

**Do this instead:** Max 3 restart attempts with 10s, 30s, 60s backoff (matching the `EscalatingBackoff` already in rc-common). After 3 failures, log `ESCALATION` and POST a report to racecontrol with `restart_verified: false`. Stop attempting. Staff sees the alert and intervenes. rc-sentry continues serving its 6 HTTP endpoints — it is not compromised by the failed restarts.

### Anti-Pattern 5: Reading rc-agent logs while rc-agent might still be running

**What people do:** Read `rc-agent-startup.log` and `debug-memory.json` immediately on first poll failure, before crash is confirmed.

**Why it's wrong:** rc-agent writes to both files during its normal startup phase. Reading mid-write produces partial data and false crash patterns.

**Do this instead:** Only read logs after the FSM transitions to `Crashed` (3 consecutive poll failures). At that point, rc-agent has been unresponsive for 15s — either crashed or wedged. File reads are safe.

---

## Scaling Considerations

Fixed 8-pod fleet. Scaling is not a concern. The relevant operational considerations are:

| Concern | Current (8 pods) | Notes |
|---------|-----------------|-------|
| Watchdog thread overhead | 1 thread per pod (runs in rc-sentry) | Sleep 5s between polls — negligible CPU |
| Crash report storage | racecontrol holds `Option<SentryCrashReport>` per pod in memory | Last crash only; no crash history DB needed for v11.2 |
| Ollama contention | qwen3:0.6b on James, single-threaded | Crashes are rare; concurrent pod crashes queuing for Ollama is a non-issue in practice |
| Startup log size | ~20 lines per boot, truncated each boot | Read entirely into memory — never a size concern |
| debug-memory.json size | Capped at 100 incidents in DebugMemory | Max ~20KB on disk |

---

## Sources

- Direct inspection: `crates/rc-sentry/src/main.rs` — complete source, existing endpoint and thread model
- Direct inspection: `crates/rc-agent/src/ai_debugger.rs` — DebugMemory, PodStateSnapshot, 4-tier architecture, fix patterns
- Direct inspection: `crates/rc-agent/src/startup_log.rs` — log paths, write_phase, detect_crash_recovery
- Direct inspection: `crates/rc-agent/src/self_heal.rs` — START_SCRIPT_CONTENT, repair patterns
- Direct inspection: `crates/rc-common/src/protocol.rs` — AgentMessage variants, StartupReport fields
- Direct inspection: `crates/racecontrol/src/fleet_health.rs` — FleetHealthStore, existing crash_recovery field
- Project context: `.planning/PROJECT.md` — v11.2 target features, anti-cheat constraints, existing milestone state
- Confidence: HIGH — all claims based on reading actual source files

---

*Architecture research for: v11.2 RC Sentry AI Debugger — crash diagnostics in rc-sentry*
*Researched: 2026-03-21 IST*
