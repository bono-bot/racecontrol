# Architecture Patterns: v31.0 Autonomous Survival System (3-Layer MI Independence)

**Domain:** Self-healing fleet management — Rust/Axum monorepo with Windows pod agents
**Researched:** 2026-03-30
**Confidence:** HIGH — based on direct code inspection of all relevant source files

---

## Current Architecture Baseline

The existing codebase already has significant MI infrastructure. v31.0 adds a survival layer
on top of it, not a replacement. Key existing modules per crate:

### rc-agent (pod, :8090)

Relevant existing modules:
- `diagnostic_engine.rs` — anomaly detection, emits `DiagnosticEvent` via mpsc
- `tier_engine.rs` — 5-tier decision tree, reads DiagnosticEvent, has circuit breaker + budget pre-check
- `knowledge_base.rs` — local SQLite KB per pod
- `openrouter.rs` — OpenRouter API client
- `budget_tracker.rs` — per-node cost tracking
- `mesh_gossip.rs` — gossip broadcast over existing WS
- `self_heal.rs` — startup config/bat self-repair (boot-time only)
- `self_monitor.rs` — self-health monitoring loop
- `failure_monitor.rs` — failure state tracking

### racecontrol (server, :8080)

Relevant existing modules:
- `pod_monitor.rs` — heartbeat detector, pure detection, delegates repair to pod_healer
- `pod_healer.rs` — graduated AI-driven recovery (kill zombies, WoL, AI escalation)
- `fleet_kb.rs` — fleet-wide SQLite KB, gossip storage + promotion pipeline
- `mesh_handler.rs` — processes incoming MeshSolution*/MeshExperiment*/MeshHeartbeat messages
- `maintenance_engine.rs` — predictive maintenance signals

### rc-watchdog (Windows service, pods + james mode)

- `service.rs` — Windows SYSTEM service, polls rc-agent liveness every 5s, restarts via Session 1
- `james_monitor.rs` — James machine mode, monitors 9 services, graduated AI recovery via Ollama
- `reporter.rs` — sends `WatchdogCrashReport` to server via HTTP POST `/pods/{id}/watchdog-crash`

### rc-common (shared lib)

- `mesh_types.rs` — `MeshSolution`, `SolutionStatus`, `FixType`, `DiagnosisTier`
- `verification.rs` — `ColdVerificationChain`, `VerifyStep`
- `recovery.rs` — `RecoveryAction`, `RecoveryAuthority`, `RecoveryDecision`, `RecoveryLogger`
- `watchdog.rs` — `EscalatingBackoff`
- `protocol.rs` — `AgentMessage`, `CoreToAgentMessage`, `DashboardEvent`
- `types.rs` — `WatchdogCrashReport`, `PodInfo`, `PodStatus`

---

## v31.0 Architecture: 3-Layer Survival System

### System Overview

```
┌─────────────────────────────────────────────────────────────────────────┐
│  LAYER 3: EXTERNAL GUARDIAN  (Bono VPS srv1422716.hstgr.cloud)          │
│  ┌─────────────────────────────────────────────────────────────────┐    │
│  │  guardian_daemon (new standalone binary: rc-guardian)           │    │
│  │  - WS subscriber to server /ws/guardian                         │    │
│  │  - Polls server :8080/api/v1/health every 60s (outside LAN)     │    │
│  │  - WhatsApp alert escalation (Evolution API)                     │    │
│  │  - Cross-venue KB sync endpoint                                  │    │
│  └─────────────────────────────────────────────────────────────────┘    │
└───────────────────────────────────┬─────────────────────────────────────┘
                                    │ HTTPS/WS (public internet)
┌───────────────────────────────────▼─────────────────────────────────────┐
│  LAYER 2: SERVER FLEET HEALER  (Server 192.168.31.23:8080)              │
│  ┌────────────────────────────┐  ┌────────────────────────────────────┐ │
│  │  pod_monitor.rs (existing) │  │  survival_coordinator.rs (NEW)     │ │
│  │  - heartbeat detection     │  │  - Layer 1 report ingestion        │ │
│  └────────────┬───────────────┘  │  - cross-pod pattern detection     │ │
│               │ delegates        │  - server self-check loop          │ │
│  ┌────────────▼───────────────┐  │  - guardian push channel           │ │
│  │  pod_healer.rs (extended)  │  └────────────────────────────────────┘ │
│  │  + MMA diagnosis path      │                                         │
│  │  + Layer1Report ingestion  │  ┌────────────────────────────────────┐ │
│  │  + survival_action API     │  │  fleet_kb.rs (existing, extended)  │ │
│  └────────────────────────────┘  │  + survival_events table           │ │
│                                  └────────────────────────────────────┘ │
└──────────────────────┬──────────────────────────────────────────────────┘
                       │ WS + HTTP (LAN)
┌──────────────────────▼──────────────────────────────────────────────────┐
│  LAYER 1: SMART WATCHDOG  (each pod: Windows SYSTEM service)            │
│  ┌─────────────────────────────────────────────────────────────────┐    │
│  │  rc-watchdog/src/service.rs (extended — NOT a new binary)        │    │
│  │                                                                  │    │
│  │  ┌──────────────────────┐   ┌─────────────────────────────────┐ │    │
│  │  │  existing: liveness  │   │  NEW: smart_watchdog.rs module  │ │    │
│  │  │  poll + Session 1    │   │  - binary validation (SHA256)   │ │    │
│  │  │  restart             │   │  - OpenRouter MMA diagnosis     │ │    │
│  │  └──────────────────────┘   │  - HTTP crash report to server  │ │    │
│  │                             │  - survival_state.rs            │ │    │
│  │                             └─────────────────────────────────┘ │    │
│  └─────────────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## New vs Modified: Explicit Breakdown

### New Modules

| Module | Crate | Type | Purpose |
|--------|-------|------|---------|
| `crates/rc-watchdog/src/smart_watchdog.rs` | rc-watchdog | NEW module | OpenRouter client, binary validation, MMA diagnosis inside watchdog |
| `crates/rc-watchdog/src/survival_state.rs` | rc-watchdog | NEW module | Persistent state across watchdog cycles (crash count, last_diagnosis, budget spent) |
| `crates/racecontrol/src/survival_coordinator.rs` | racecontrol | NEW module | Layer 1 report ingestion, cross-pod pattern detection, server self-check loop, guardian push channel |
| `crates/racecontrol/src/api/survival.rs` | racecontrol | NEW module | HTTP endpoints for Layer 1 reports + guardian health queries |
| `crates/rc-common/src/survival_types.rs` | rc-common | NEW module | Shared types: `Layer1Report`, `SurvivalAction`, `GuardianEvent`, `SurvivalState` |
| `crates/rc-guardian/` | NEW CRATE | New standalone binary | External guardian daemon (Bono VPS) — separate deploy target |

### Modified Modules

| Module | Crate | What Changes |
|--------|-------|-------------|
| `crates/rc-watchdog/src/service.rs` | rc-watchdog | Add `smart_watchdog::SmartWatchdogContext` field; after restart, call `smart_watchdog::run_diagnosis()` if crash count >= 2 |
| `crates/rc-watchdog/src/james_monitor.rs` | rc-watchdog | Add `survival_state` persistence; extend Ollama diagnosis to use `smart_watchdog::diagnose_with_openrouter()` as fallback when Ollama fails |
| `crates/racecontrol/src/pod_healer.rs` | racecontrol | Ingest `Layer1Report` from survival_coordinator; Layer 1 diagnosis feeds Tier 2 KB lookup before healer runs its own AI path |
| `crates/racecontrol/src/fleet_kb.rs` | racecontrol | Add `survival_events` table; add `record_survival_event()` and `query_survival_pattern()` functions |
| `crates/rc-common/src/types.rs` | rc-common | Extend `WatchdogCrashReport` with `diagnosis_summary: Option<String>` and `mma_cost: Option<f64>` |
| `crates/rc-common/src/lib.rs` | rc-common | Export `survival_types` module |
| `crates/Cargo.toml` (workspace) | workspace | Add `crates/rc-guardian` to members |

---

## Component Boundaries

### Layer 1: Smart Watchdog (rc-watchdog, pod-side)

**Location:** `crates/rc-watchdog/src/smart_watchdog.rs` — new module within existing crate.

**Why not a new crate:** rc-watchdog is already the Windows service binary on pods. Adding a new crate would require a new binary deployed to all 8 pods. The smart watchdog logic belongs in the same process that already owns restart authority. Adding a module costs zero deploy complexity.

**What it owns:**
- Binary validation: SHA256 check of `rc-agent.exe` against manifest before restart
- Crash count tracking: persisted to `C:\RacingPoint\watchdog-survival.json` (survives watchdog restart)
- MMA diagnosis trigger: after crash_count >= 2 in a 10-minute window, call OpenRouter
- HTTP report to server: POST to `/api/v1/survival/layer1-report` (non-blocking, fire-and-forget)

**What it does NOT own:**
- Fleet-level decisions (belongs to Layer 2)
- Solution storage in fleet KB (belongs to Layer 2)
- External alerting (belongs to Layer 3)

**Interaction with existing code:**

```rust
// service.rs modification — pseudocode, not full implementation
// After restart loop detects crash:
if self.smart_ctx.crash_count_in_window() >= 2 {
    let report = self.smart_ctx.run_diagnosis(&agent_log_tail).await;
    // fire-and-forget — watchdog cannot block the restart loop
    tokio::spawn(async move { report_to_server(report).await; });
}
```

### Layer 2: Server Fleet Healer (racecontrol, server-side)

**Location:** `crates/racecontrol/src/survival_coordinator.rs` — new module within existing crate.

**Why not a new crate:** racecontrol already owns pod_monitor + pod_healer. The survival coordinator reads from the same AppState channels and writes to the same fleet_kb. Splitting would require IPC. No benefit for a single-server deployment.

**What it owns:**
- Layer 1 report ingestion: receives `Layer1Report` from pods via POST `/api/v1/survival/layer1-report`
- Cross-pod pattern detection: "3 pods same crash within 5 minutes = systemic, alert guardian"
- Server self-check loop: every 60s, verify own DB connectivity + WS broker health + guardian reachability
- Guardian push channel: mpsc sender feeding `guardian_event_tx` for Layer 3 notifications

**Interaction with existing pod_healer.rs:**

Layer 1 reports feed into pod_healer's existing graduated recovery tracker. The healer already has a Tier 2 KB lookup path. Layer 1 diagnosis results are injected as synthetic `DiagnosticEvent` entries, allowing the healer's existing tier logic to consume them without architectural changes.

```rust
// survival_coordinator.rs feeds pod_healer via AppState channel
// AppState gets new field: layer1_report_tx: mpsc::Sender<Layer1Report>
// pod_healer spawns a receiver task that converts Layer1Report → DiagnosticEvent
```

**Interaction with existing fleet_kb.rs:**

New `survival_events` table appended to fleet_kb migration. Existing tables untouched. Pattern detection queries over `survival_events` to find cross-pod correlation.

### Layer 3: External Guardian (rc-guardian, Bono VPS)

**Location:** `crates/rc-guardian/` — NEW crate, new binary, deployed to Bono VPS.

**Why a new crate:** This runs on a different machine (Bono VPS, Linux), has different dependencies (no Windows APIs, no rc-watchdog service code), and is a separate deploy target. It cannot be a module of rc-watchdog or racecontrol. A new crate is the correct boundary.

**What it owns:**
- WS connection to server `/ws/guardian` — receives `GuardianEvent` stream
- External health polling: GET `https://racingpoint.cloud/api/v1/health` every 60s (internet path)
- WhatsApp escalation: POST to Evolution API on Bono VPS when events require human response
- Cross-venue KB sync endpoint: future (Phase 2+)

**Deploy target:** pm2 on Bono VPS, same pattern as existing `racecontrol` pm2 service.

### Unified MMA Protocol Placement

**Lives in:** `crates/rc-watchdog/src/smart_watchdog.rs` (pod-side, Layer 1) and
`crates/racecontrol/src/survival_coordinator.rs` (server-side, Layer 2).

**Shared types in:** `crates/rc-common/src/survival_types.rs`

The existing `openrouter.rs` in rc-agent already has a working OpenRouter client. rc-watchdog currently uses Ollama via james_monitor.rs. The smart watchdog needs an OpenRouter client too, but importing rc-agent from rc-watchdog creates a circular dependency (rc-watchdog already imports rc-common, not rc-agent).

**Resolution:** Extract the OpenRouter client logic into `rc-common` as `crates/rc-common/src/openrouter.rs`. Both rc-agent's existing `openrouter.rs` and the new `smart_watchdog.rs` will use this shared implementation. The rc-agent module becomes a thin wrapper.

---

## Data Flow: Diagnosis Event (Watchdog to Server to Guardian)

```
[rc-agent crash detected by rc-watchdog service.rs]
    │
    ▼
[smart_watchdog.rs: increment crash_count in survival_state.rs]
    │
    ├── crash_count < 2: standard restart (existing path), no diagnosis
    │
    ▼ crash_count >= 2 within 10-min window
[smart_watchdog.rs: collect_symptoms()]
    │  - tail last 200 lines of C:\RacingPoint\rc-agent-*.log
    │  - read survival_state.json for crash history
    │  - capture process exit code from Windows event log
    │
    ▼
[smart_watchdog.rs: try_ollama_diagnosis()]
    │  - POST http://192.168.31.27:11434 (James local Ollama)
    │  - timeout: 8s (watchdog cannot block restart loop)
    │  - if timeout/fail: fall through to OpenRouter
    │
    ▼ (on Ollama timeout or failure)
[smart_watchdog.rs: diagnose_with_openrouter()]
    │  - Tier 3: Qwen3 only (Scanner) — cheapest, <$0.05
    │  - Tier 4: all 4 models in parallel — only if Tier 3 inconclusive
    │  - budget check against survival_state.daily_watchdog_spend
    │
    ▼
[smart_watchdog.rs: build Layer1Report]
    │  struct Layer1Report {
    │      pod_id, crash_count, symptoms, diagnosis_summary,
    │      mma_cost, fix_suggestion, fix_type, timestamp
    │  }
    │
    ▼ (fire-and-forget tokio::spawn — MUST NOT block restart)
[reporter.rs: POST http://192.168.31.23:8080/api/v1/survival/layer1-report]
    │  - 5s timeout
    │  - on failure: write to C:\RacingPoint\watchdog-offline-reports.jsonl
    │                (server picks up when back online via poll at reconnect)
    │
    ▼
[racecontrol/api/survival.rs: POST handler]
    │  - validate Layer1Report struct
    │  - write to fleet_kb.survival_events
    │  - send to survival_coordinator via mpsc
    │
    ▼
[survival_coordinator.rs: ingest_layer1_report()]
    │  - check cross-pod correlation: query survival_events WHERE
    │    problem_key = $key AND created_at > (now - 5min) GROUP BY pod_id HAVING count >= 3
    │
    ├── no correlation: inject as DiagnosticEvent into pod_healer channel
    │       pod_healer proceeds with existing Tier 2 KB lookup → Tier 3/4 if needed
    │
    ▼ systemic pattern detected (3+ pods, same issue, 5-min window)
[survival_coordinator.rs: escalate_to_guardian()]
    │  - build GuardianEvent { severity: Critical, pattern: Systemic, ... }
    │  - send via guardian_event_tx mpsc channel
    │
    ▼
[rc-guardian: receive GuardianEvent via /ws/guardian]
    │  - classify severity: Info / Warning / Critical / Emergency
    │
    ├── Info/Warning: log only, no alert
    ├── Critical: WhatsApp alert to Uday + Bono
    ▼ Emergency (server unreachable from external + 3+ pods down)
[rc-guardian: escalate()]
    │  - WhatsApp: immediate alert to Uday
    │  - Attempt comms-link WS message to James
    │  - Record in guardian incident log on Bono VPS
```

### Offline Report Recovery

When the watchdog cannot reach the server (server down, LAN issue):

```
[Layer1Report fails to POST]
    │
    ▼
[smart_watchdog.rs: write to C:\RacingPoint\watchdog-offline-reports.jsonl]
    │  (append-only, one JSON object per line)
    │
    ▼ on next successful server connection (ws_handler.rs or reporter.rs)
[service.rs: flush_offline_reports()]
    │  - read watchdog-offline-reports.jsonl
    │  - POST each to /api/v1/survival/layer1-report
    │  - on success: clear the file
```

---

## New API Endpoints

All endpoints added to `crates/racecontrol/src/api/survival.rs`, registered in `routes.rs`.

| Method | Path | Auth | Purpose |
|--------|------|------|---------|
| POST | `/api/v1/survival/layer1-report` | service key | Receive Layer 1 crash report from watchdog |
| GET | `/api/v1/survival/status` | staff JWT | Current survival state for all pods + server |
| GET | `/api/v1/survival/events` | staff JWT | Paginated survival event log |
| WS | `/ws/guardian` | guardian token | Guardian daemon event stream |

**Auth note:** `/api/v1/survival/layer1-report` uses `X-Service-Key` (same as other pod-to-server endpoints). The guardian WS uses a dedicated `GUARDIAN_TOKEN` env var on Bono VPS — separate from the pod service key, so a compromised pod cannot impersonate the guardian.

---

## Recommended Project Structure Changes

```
crates/
├── rc-common/src/
│   ├── survival_types.rs    # NEW — Layer1Report, SurvivalAction, GuardianEvent, SurvivalState
│   ├── openrouter.rs        # NEW — extracted from rc-agent/src/openrouter.rs (shared client)
│   └── lib.rs               # MODIFIED — export survival_types, openrouter
│
├── rc-watchdog/src/
│   ├── smart_watchdog.rs    # NEW — binary validation, symptom collection, MMA diagnosis
│   ├── survival_state.rs    # NEW — persistent state (crash counts, spend, last diagnosis)
│   ├── service.rs           # MODIFIED — inject SmartWatchdogContext, call diagnosis on threshold
│   ├── james_monitor.rs     # MODIFIED — use openrouter fallback from rc-common
│   └── reporter.rs          # MODIFIED — extend WatchdogCrashReport fields, add flush_offline
│
├── racecontrol/src/
│   ├── survival_coordinator.rs  # NEW — Layer1 ingestion, pattern detection, guardian channel
│   ├── api/survival.rs          # NEW — HTTP endpoints for layer1-report, status, events
│   ├── pod_healer.rs            # MODIFIED — consume Layer1Report via new mpsc channel
│   ├── fleet_kb.rs              # MODIFIED — survival_events table + query_survival_pattern()
│   └── state.rs                 # MODIFIED — add guardian_event_tx, layer1_report_tx to AppState
│
└── rc-guardian/                 # NEW CRATE
    ├── Cargo.toml
    └── src/
        ├── main.rs              # Binary entry point, pm2 target
        ├── ws_client.rs         # WS connection to server /ws/guardian with reconnect
        ├── health_poller.rs     # External health polling (internet path)
        ├── escalation.rs        # WhatsApp + comms-link alert routing
        └── incident_log.rs      # Persistent incident log on Bono VPS
```

---

## Architectural Patterns

### Pattern 1: Module Injection over New Crate

**What:** Add new capability as a module inside an existing crate rather than creating a new binary.

**When to use:** When the new logic needs to run in the same process, shares memory with existing code, and doesn't have conflicting platform requirements.

**Applied here:** `smart_watchdog.rs` is a module in rc-watchdog (not a new crate). `survival_coordinator.rs` is a module in racecontrol. This avoids 2 new binaries that would need separate deploy pipelines to 8 pods + server.

**Trade-offs:** Increases binary size slightly. Tighter coupling — a panic in smart_watchdog could crash the service loop. Mitigate with `tokio::spawn` for all MMA calls (panics stay contained).

### Pattern 2: Fire-and-Forget Diagnosis (Non-Blocking Restart Loop)

**What:** All MMA calls inside rc-watchdog are spawned in a separate tokio task. The restart loop never awaits diagnosis results.

**When to use:** When the primary function (restart rc-agent within seconds) must not be delayed by a secondary function (diagnosis taking up to 30s for 4-model parallel).

**Applied here:** `tokio::spawn(async move { diagnose_and_report(symptoms).await; })` is called immediately after the decision to diagnose. The restart proceeds in parallel.

**Critical constraint:** smart_watchdog.rs cannot use `tokio::runtime::Handle::current()` inside service.rs's std::thread context. The watchdog service.rs uses a `std::thread` loop, not an async runtime. Diagnosis must either be synchronous (blocking reqwest) with a short timeout, OR service.rs must spawn a Tokio runtime for the diagnosis task.

**Resolution:** Add a `tokio::runtime::Builder::new_multi_thread()` runtime inside smart_watchdog's diagnosis path, kept alive for the watchdog process lifetime. This mirrors the pattern in james_monitor.rs.

### Pattern 3: Offline-First Report Queue

**What:** If the report destination is unreachable, persist reports locally to a JSONL file. Flush on next successful connection.

**When to use:** When the reporter (watchdog) may outlive the reportee (server) and reports must not be lost.

**Applied here:** `watchdog-offline-reports.jsonl` on each pod. Prevents survival data loss during server restarts or LAN issues.

**Constraint:** File must be append-only and bounded. Cap at 200 entries (rotate oldest). A server that's been down for days should not receive a flood of stale reports on reconnect.

### Pattern 4: Shared OpenRouter Client in rc-common

**What:** Extract the OpenRouter HTTP client from rc-agent into rc-common so both rc-agent and rc-watchdog can use it without creating a crate dependency from rc-watchdog on rc-agent.

**When to use:** When two crates need the same external API client and neither should depend on the other.

**Applied here:** `rc-common/src/openrouter.rs` contains `OpenRouterClient` struct with `diagnose()` method. Both rc-agent's existing `openrouter.rs` (becomes a re-export or thin wrapper) and the new `smart_watchdog.rs` import from `rc_common::openrouter`.

**Dependency graph result:**
```
rc-guardian → rc-common
rc-watchdog → rc-common
rc-agent    → rc-common
racecontrol → rc-common
```
No circular dependencies. rc-common remains a leaf crate.

---

## Anti-Patterns to Avoid

### Anti-Pattern 1: Diagnosis Blocking the Restart Loop

**What people do:** `await diagnose_with_openrouter(&symptoms).await` directly in the service poll loop before calling restart.

**Why it's wrong:** OpenRouter Tier 4 (4 models parallel) can take 15-30 seconds. rc-agent stays dead during that window. Customer sessions are disrupted. The primary job of the watchdog is to restart fast.

**Do this instead:** `tokio::spawn(...)` or run in a separate thread with `std::thread::spawn`. The restart executes immediately; diagnosis runs in parallel.

### Anti-Pattern 2: New Binary for Layer 1 Smart Watchdog

**What people do:** Create a new `rc-smart-watchdog` crate that replaces rc-watchdog.

**Why it's wrong:** rc-watchdog is a Windows service registered by name `RCWatchdog`. Replacing it requires uninstalling the service on all 8 pods (SSH access needed), re-registering, and updating the `start-rcagent.bat` that references it. The existing binary validation, Session 1 restart logic, and sentry breadcrumb coordination would need to be re-implemented. High deploy risk for no structural benefit.

**Do this instead:** Add `smart_watchdog.rs` as a module inside rc-watchdog. Deploy the updated rc-watchdog binary via the normal pod deploy pipeline. Service name stays `RCWatchdog`.

### Anti-Pattern 3: Guardian Directly Accessing Pod Endpoints

**What people do:** rc-guardian polls each pod directly (curl pod1:8090/health, etc.) from Bono VPS.

**Why it's wrong:** Pod endpoints are LAN-only (192.168.31.x). Bono VPS is on the public internet. This would require Tailscale access from Bono to all pods — fragile and creates a large attack surface. Also, if the server is down but pods are fine, the guardian should know the server is the problem, not the pods.

**Do this instead:** Guardian only talks to the server. The server aggregates pod state and exposes it via `/ws/guardian`. The guardian's job is to observe the server's view of the world from an external vantage point.

### Anti-Pattern 4: Layer 1 Reports Flowing Through WS (Instead of HTTP)

**What people do:** Encode Layer1Report as a new `AgentMessage::Layer1Report` variant and send it over the existing rc-agent WS connection.

**Why it's wrong:** The Layer 1 watchdog is a separate process from rc-agent. The WS connection belongs to rc-agent, not to rc-watchdog. rc-watchdog sends reports because rc-agent is DEAD or crash-looping — there is no WS to use. The report mechanism must be independent of rc-agent's WS connection.

**Do this instead:** rc-watchdog uses its own HTTP client (blocking reqwest, already used in reporter.rs) to POST directly to the server. This path works even when rc-agent is completely down.

---

## Integration Points

### External Services

| Service | Integration Pattern | Notes |
|---------|---------------------|-------|
| OpenRouter API | HTTPS POST from rc-watchdog (new) and rc-agent (existing) | Use shared `rc_common::openrouter::OpenRouterClient`; key from `OPENROUTER_API_KEY` env var |
| Evolution API (WhatsApp) | HTTPS POST from rc-guardian on Bono VPS | Same path as existing `whatsapp_alerter.rs` in racecontrol; rc-guardian gets its own client |
| comms-link WS | WS from rc-guardian for James notification | Use existing comms-link relay at ws://srv1422716.hstgr.cloud:8765 |
| Ollama (James .27) | HTTP from rc-watchdog james_monitor (existing) + smart_watchdog Tier 3 fallback | `http://127.0.0.1:11434` — only accessible from James machine, not pods |

### Internal Boundaries

| Boundary | Communication | Notes |
|----------|---------------|-------|
| rc-watchdog → racecontrol (Layer 1 report) | HTTP POST `/api/v1/survival/layer1-report` | reqwest blocking client, 5s timeout, offline JSONL queue on failure |
| rc-watchdog → racecontrol (crash report, existing) | HTTP POST `/api/v1/pods/{id}/watchdog-crash` | Extended with `diagnosis_summary`, `mma_cost` fields in `WatchdogCrashReport` |
| survival_coordinator → pod_healer | tokio mpsc `Sender<Layer1Report>` in AppState | Layer1Report converted to synthetic DiagnosticEvent in pod_healer |
| survival_coordinator → rc-guardian | WS push `/ws/guardian` | `GuardianEvent` enum with severity classification |
| rc-guardian → server (external poll) | HTTP GET `https://racingpoint.cloud/api/v1/health` | Bono VPS → public internet path; uses Bono's outbound HTTP |
| rc-common (openrouter) ← rc-watchdog + rc-agent | Shared library (crate dependency) | rc-watchdog gains `rc-common` dependency for openrouter module |

---

## Build Order (Phase Dependencies)

Build order is driven by: (1) shared types must compile before consumers, (2) no new crates that block other phases, (3) server endpoints must exist before watchdog reports to them, (4) guardian is independent and can ship last.

```
Phase 1 (Foundation — unblocks all other phases)
    └── rc-common: add survival_types.rs + openrouter.rs extraction
        - SurvivalState, Layer1Report, SurvivalAction, GuardianEvent
        - OpenRouterClient moved from rc-agent/src/openrouter.rs to rc-common
        - rc-agent/src/openrouter.rs becomes thin re-export wrapper
        - Compile-verified: cargo check -p rc-common -p rc-agent
        - No deploy needed yet (lib change only)

Phase 2 (Layer 2 Server — establishes report ingestion endpoint)
    └── racecontrol:
        - survival_coordinator.rs (new module)
        - api/survival.rs (new endpoints)
        - fleet_kb.rs: survival_events migration
        - state.rs: new AppState fields
        - routes.rs: register /api/v1/survival/* + /ws/guardian
        Deploy: server only. Pods don't report yet but endpoints exist.

Phase 3 (Layer 1 Watchdog — pods start reporting)
    └── rc-watchdog:
        - smart_watchdog.rs (new module)
        - survival_state.rs (new module)
        - service.rs: inject SmartWatchdogContext
        - reporter.rs: extend WatchdogCrashReport, add flush_offline
        Deploy: all 8 pods + POS. Uses canary (Pod 8 first).
        Server endpoints from Phase 2 must be live before deploy.

Phase 4 (Layer 2 Integration — healer consumes Layer 1 data)
    └── racecontrol:
        - pod_healer.rs: consume Layer1Report channel
        - survival_coordinator.rs: cross-pod pattern detection active
        Deploy: server only.

Phase 5 (Layer 3 Guardian — external survival view)
    └── rc-guardian (new crate):
        - ws_client.rs, health_poller.rs, escalation.rs, incident_log.rs
        Deploy: Bono VPS (pm2, new process). Zero pod/server changes.
```

**Why this order:**
- Phase 1 first: rc-common is a leaf dep; compiling new shared types before anything uses them prevents "type not found" errors cascade.
- Phase 2 before Phase 3: Pods cannot send reports to an endpoint that doesn't exist. Deploying the watchdog first would cause 8 failed HTTP POSTs per crash event and fill offline JSONL queues unnecessarily.
- Phase 4 after Phase 3: pod_healer integration only adds value once Layer 1 reports are actually arriving.
- Phase 5 last: Guardian is pure consumer, no producer. No other phase depends on it. It can ship incrementally — even a basic "ping server + WhatsApp on failure" guardian provides immediate value.

---

## Scaling Considerations

This is a fixed fleet (8-10 pods, 1-2 servers, 1-2 venues). Not a user-scaling problem.

| Concern | Current Scale (8 pods) | Future Scale (3 venues, 24 pods) |
|---------|------------------------|-----------------------------------|
| Layer 1 HTTP reports | Each watchdog POSTs on crash only — negligible | Same pattern; server can handle 100+ reports/min |
| survival_events table | Low write volume (crash events are rare) | Partition by venue_id when multi-venue ships |
| Guardian WS | 1 persistent WS connection | 1 per venue server; guardian fans out WhatsApp per venue |
| OpenRouter cost (Layer 1) | $0.05-$3.01 per diagnosis, only on crash_count >= 2 | Budget-gated; per-venue budget tracked in survival_state |

---

## Sources

- Direct code inspection: `crates/rc-watchdog/src/service.rs`, `james_monitor.rs`, `reporter.rs`
- Direct code inspection: `crates/rc-agent/src/diagnostic_engine.rs`, `tier_engine.rs`, `openrouter.rs`
- Direct code inspection: `crates/racecontrol/src/pod_healer.rs`, `pod_monitor.rs`, `fleet_kb.rs`, `mesh_handler.rs`
- Direct code inspection: `crates/rc-common/src/mesh_types.rs`, `recovery.rs`, `verification.rs`, `types.rs`
- `.planning/MESHED-INTELLIGENCE.md` — v26.0 MI design spec
- `CLAUDE.md` standing rules — deploy pipeline, Session 1 constraints, WatchdogCrashReport HTTP path

---
*Architecture research for: v31.0 Autonomous Survival System (3-Layer MI Independence)*
*Researched: 2026-03-30*
