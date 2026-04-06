# Phase 322: MI Core Engine Migration — Research

**Researched:** 2026-04-06
**Domain:** Rust module migration — async-to-sync port, SQLite in pure-std binary, thin HTTP proxy
**Confidence:** HIGH (all findings from direct source code inspection)

<user_constraints>
## User Constraints (from CONTEXT.md / Phase 321)

### Locked Decisions (from Phase 321 CONTEXT.md)
- rc-sentry is pure std::net, no tokio — do NOT add tokio to rc-sentry
- Strictly sequential phases: 321 → 322 → 323 → 324
- Phase 321 monitoring foundation must be live before adding MI logic

### Claude's Discretion
- Feature flag naming for the thin proxy gate
- HTTP endpoint path on rc-sentry for telemetry forwarding (/mi/trigger chosen)
- Whether to use a new rc-common module or expand mesh_types.rs for shared diagnostic types

### Deferred Ideas (OUT OF SCOPE for Phase 322)
- Tier 3/4 model calls in rc-sentry (Phase 323)
- CognitiveGate / DiagnosisPlanner migration (Phase 323)
- mesh_gossip propagation (Phase 324)
- Fleet bus / FleetEvent broadcast in rc-sentry (Phase 324)
- Staff diagnostic bridge in rc-sentry (Phase 323)
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| MIG-01 | rc-sentry runs 5-tier decision tree, produces TierDiagnosis from DiagnosticTrigger — independent of rc-agent | Tier 1+2 logic is pure synchronous, ports cleanly to std::thread. TierDiagnosis is a new type. |
| MIG-02 | rc-sentry diagnostic engine classifies anomalies, fires DiagnosticTrigger via std::sync channel — observable at :8091 /debug | DiagnosticTrigger enum needs to move to rc-common. std::sync::mpsc replaces tokio::sync::mpsc. |
| MIG-03 | rc-sentry knowledge base queries SQLite, returns SolutionRecord even when rc-agent is dead | rusqlite is synchronous — ideal for pure-std model. Same DB path. SolutionRecord = renamed Solution. |
| MIG-05 | rc-agent MI modules replaced with thin proxy forwarding telemetry to rc-sentry :8091 via HTTP — rc-agent still compiles, WS messages unchanged | Feature-gate pattern. DiagnosticLog consumed by WS handler — proxy writes to it after getting TierDiagnosis back. |
</phase_requirements>

## Summary

Phase 322 migrates 4 MI modules (tier_engine, diagnostic_engine, knowledge_base, diagnostic_log)
from rc-agent into rc-sentry. The central challenge is the async-to-sync port: rc-agent's MI
uses tokio throughout (mpsc channels, spawn_blocking, time::interval, watch channels), while
rc-sentry is a pure std binary with thread-per-connection architecture and no tokio runtime.

The good news: the actual computation inside these modules is almost entirely synchronous.
tokio is used for scheduling and channel communication, not for true async I/O. The port to
std threads + std::sync channels is mechanical, not architectural. rusqlite (used by KnowledgeBase)
is already synchronous — it requires no changes.

The thin proxy in rc-agent (MIG-05) is straightforward: rc-agent's diagnostic_engine continues
detecting triggers, but instead of sending them to a local tier_engine, it POSTs them to
rc-sentry :8091/mi/trigger and writes the response into its DiagnosticLog. All downstream
consumers (WS handler, /events/recent endpoint) continue working unchanged.

**Primary recommendation:** Port tier engine Tier 1+2 to a dedicated std::thread. Stub Tier 3+4.
Add rusqlite to rc-sentry. Move DiagnosticTrigger + new TierDiagnosis/SolutionRecord types to
rc-common/src/diagnostic_types.rs. Gate rc-agent MI behind a `mi-engine` Cargo feature.

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| rusqlite | 0.32 (bundled) | SQLite for KnowledgeBase in rc-sentry | Already in rc-agent at same version; bundled = no system SQLite needed |
| sysinfo | 0.33 | Process scan for WerFault detection in diagnostic engine | Already in rc-sentry Cargo.toml |
| std::sync::mpsc | std | Channel from diagnostic engine to tier engine | No tokio in rc-sentry |
| std::thread | std | Dedicated threads for diagnostic + tier engines | No tokio in rc-sentry |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| serde_json | workspace | Serialize DiagnosticTrigger for HTTP proxy POST | Already in both crates |
| chrono | workspace | Timestamps in TierDiagnosis | Already in both crates |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| std::sync::mpsc | crossbeam-channel | crossbeam is faster but adds a dependency; mpsc is sufficient for ~1 event/5 min |
| std::net for proxy HTTP | reqwest | reqwest already in rc-agent (optional feature); std::net avoids adding a new dep but requires manual HTTP formatting; use std::net |

**Installation (rc-sentry Cargo.toml addition):**
```toml
rusqlite = { version = "0.32", features = ["bundled"] }
```

## Architecture Patterns

### Recommended Project Structure (new files)

```
crates/rc-common/src/
└── diagnostic_types.rs     # DiagnosticTrigger, DiagnosticEvent, TierDiagnosis, SolutionRecord

crates/rc-sentry/src/
├── mi_diagnostic_engine.rs  # std::thread polling loop, emits DiagnosticTrigger
├── mi_tier_engine.rs        # std::thread, Tier 1+2 decision tree
├── mi_knowledge_base.rs     # Copy of knowledge_base.rs (rusqlite, synchronous)
└── mi_debug_state.rs        # Shared state: last N triggers + last TierDiagnosis for /debug

crates/rc-agent/src/
└── mi_proxy.rs              # Thin proxy: POST to rc-sentry, write DiagnosticLog
```

Feature gate in rc-agent Cargo.toml:
```toml
[features]
default = ["mi-engine", ...]
mi-engine = []   # When enabled: use local tier_engine. When disabled: use mi_proxy.
```

### Pattern 1: std::thread Channel Pair (replaces tokio::spawn + mpsc)

**What:** Replace `tokio::spawn(async move { ... })` with `std::thread::Builder::new().spawn(move || { ... })`.
Replace `tokio::sync::mpsc::channel` with `std::sync::mpsc::channel`.

**When to use:** All MI engine threads in rc-sentry.

**Example:**
```rust
// Source: rc-sentry/src/main.rs pattern (existing crash-handler thread)
let (trigger_tx, trigger_rx) = std::sync::mpsc::channel::<DiagnosticTrigger>();

std::thread::Builder::new()
    .name("sentry-diagnostic-engine".to_string())
    .spawn(move || {
        tracing::info!(target: "diagnostic-engine", "lifecycle: started");
        loop {
            std::thread::sleep(Duration::from_secs(SCAN_INTERVAL_SECS));
            // emit Periodic trigger
            let _ = trigger_tx.send(DiagnosticTrigger::Periodic);
            // check health, emit HealthCheckFail if needed
        }
    })
    .expect("spawn diagnostic engine");

std::thread::Builder::new()
    .name("sentry-tier-engine".to_string())
    .spawn(move || {
        for trigger in trigger_rx {  // blocks on mpsc::Receiver::iter()
            let diagnosis = run_tier_engine(&trigger, &kb);
            // store in shared state
        }
    })
    .expect("spawn tier engine");
```

### Pattern 2: Shared Debug State via Arc<Mutex<...>>

**What:** rc-sentry's HTTP handler thread and tier engine thread share state via `Arc<Mutex<DebugState>>`.
The debug state holds last N DiagnosticTrigger events and last TierDiagnosis.

**When to use:** Feeding the /debug endpoint from a background engine thread.

**Example:**
```rust
// Source: rc-sentry pattern matches existing EXEC_SLOTS AtomicUsize + SHUTDOWN_REQUESTED AtomicBool
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct MiDebugState {
    recent_triggers: std::collections::VecDeque<DiagnosticTrigger>, // last 20
    last_diagnosis: Option<TierDiagnosis>,
}

// Initialized at startup as OnceLock or passed via Arc clone:
static MI_DEBUG_STATE: OnceLock<Arc<Mutex<MiDebugState>>> = OnceLock::new();
```

### Pattern 3: KnowledgeBase in std context (no changes needed)

**What:** rusqlite::Connection is `!Send` but can be owned by a single thread. The tier engine
thread opens its own KnowledgeBase connection and queries it synchronously.

**When to use:** Tier 2 KB lookup in tier engine thread.

**Example:**
```rust
// Source: rc-agent/src/knowledge_base.rs (verified: already synchronous)
fn run_tier2(trigger: &DiagnosticTrigger) -> TierResult {
    let kb = KnowledgeBase::open(KB_PATH)?;
    let problem_key = normalize_problem_key(trigger);
    match kb.lookup(&problem_key) {
        Ok(Some(solution)) if solution.confidence >= HIGH_CONFIDENCE_THRESHOLD => {
            // apply solution
            TierResult::Fixed { tier: 2, action: solution.fix_action }
        }
        _ => TierResult::NotApplicable { tier: 2 }
    }
}
```

Note: rusqlite::Connection is not Send — the KB must be opened on the tier engine thread and
NOT passed across thread boundaries. Open a fresh connection per-event or keep one per thread.
The existing rc-agent code opens KnowledgeBase::open() per use site (per-event pattern).

### Pattern 4: Thin Proxy in rc-agent (MIG-05)

**What:** When `mi-engine` feature is disabled, rc-agent's diagnostic events go to mi_proxy
instead of tier_engine. mi_proxy does a synchronous HTTP POST to rc-sentry :8091/mi/trigger
using std::net::TcpStream (to avoid adding reqwest to non-optional dependencies).

**When to use:** Phase 322 compile with `--no-default-features --features mi-proxy`.

**Example:**
```rust
// Source: rc-agent thin proxy pattern
pub fn forward_to_sentry(trigger: &DiagnosticTrigger) -> Option<TierDiagnosis> {
    let body = serde_json::to_string(trigger).ok()?;
    let sentry_addr = "127.0.0.1:8091";
    let mut stream = std::net::TcpStream::connect_timeout(
        &sentry_addr.parse().ok()?,
        Duration::from_secs(5),
    ).ok()?;
    // Write raw HTTP POST (no Content-Length trick needed — small body)
    let request = format!(
        "POST /mi/trigger HTTP/1.0\r\nContent-Type: application/json\r\nContent-Length: {}\r\nX-Service-Key: {}\r\n\r\n{}",
        body.len(), service_key(), body
    );
    stream.write_all(request.as_bytes()).ok()?;
    // Read response
    let mut response = String::new();
    let mut reader = std::io::BufReader::new(&stream);
    // skip headers, parse body
    // return TierDiagnosis
}
```

### Anti-Patterns to Avoid

- **Adding tokio to rc-sentry:** rc-sentry's value is its minimal footprint. `No tokio, no async`
  is a stated design goal. Do not add `tokio = { ... }` to rc-sentry/Cargo.toml.
- **Sharing rusqlite::Connection across threads:** rusqlite Connection is !Send. Each thread must
  open its own connection. Open per-event (cheap — SQLite file open is fast).
- **Copying the full tier_engine.rs without adaptation:** The tokio-specific code (spawn_blocking,
  select!, mpsc::Receiver::recv().await) will not compile. Strip all tokio calls first.
- **Moving FailureMonitorState to rc-common:** This type is deeply rc-agent-specific (game_pid,
  billing_active, hid_connected). rc-sentry doesn't have this state — don't leak it.
- **Deleting rc-agent MI modules in Phase 322:** Too risky. Feature-gate them; delete in Phase 323+
  once rc-sentry MI is stable.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| SQLite access | Custom file-based storage | rusqlite (bundled) | Schema migrations, indexing, and transactions already implemented in knowledge_base.rs |
| Process scanning | Win32 API directly | sysinfo 0.33 (already in rc-sentry) | Cross-platform, already a dependency |
| JSON serialization for proxy HTTP | Manual string formatting | serde_json (workspace dep) | Already in both crates; DiagnosticTrigger derives Serialize/Deserialize |

**Key insight:** The hardest part of this migration is NOT finding the right libraries — it is
stripping tokio from well-working code while preserving logic. The risk is in the porting, not
the library choices.

## Runtime State Inventory

> This phase moves engine logic between binaries. No renaming/refactoring of stored data.

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | `C:\RacingPoint\mesh_kb.db` — existing SQLite KB with solutions | No migration — same path, same schema. rc-sentry opens same file. |
| Live service config | rc-sentry.toml — no MI-related config today | Add `mi_enabled = true` section if needed for Phase 323 feature toggle |
| OS-registered state | None (no task scheduler entries specific to MI) | None |
| Secrets/env vars | `RCSENTRY_SERVICE_KEY` — already on pods, used for /exec auth | Reuse for /mi/trigger endpoint |
| Build artifacts | Existing rc-sentry.exe on all 8 pods + POS | Normal deploy cycle — hash-based rename |

**KB path already consistent:** `KB_PATH = r"C:\RacingPoint\mesh_kb.db"` in rc-agent/knowledge_base.rs.
rc-sentry will use the same path. No data migration required.

## Common Pitfalls

### Pitfall 1: tokio::task::spawn_blocking in std context
**What goes wrong:** rc-agent's tier_engine uses `tokio::task::spawn_blocking` for sysinfo calls
(standing rule: no sync ops on async runtime). In rc-sentry's std context there is no async runtime,
so sysinfo can be called directly in the thread without spawn_blocking.
**Why it happens:** Copying tier_engine.rs without adapting the spawn_blocking wrappers.
**How to avoid:** Global search-and-replace: `tokio::task::spawn_blocking(|| { X }).await.unwrap_or(default)` → `X` (direct call, same thread).
**Warning signs:** `error[E0432]: unresolved import tokio::task` or missing #[tokio::main].

### Pitfall 2: async fn in std thread
**What goes wrong:** diagnostic_log.push() and .recent() are declared `async fn` despite having
no actual async work inside them. They will not compile in a non-async context.
**Why it happens:** The async was added for Arc<RwLock<...>> consistency with the rest of rc-agent.
**How to avoid:** In rc-sentry's port of diagnostic_log, drop the async keyword. The RwLock
operations are synchronous — `async fn push` → `fn push`.
**Warning signs:** `'async' is not allowed here` or missing `#[tokio::main]` on main.

### Pitfall 3: tokio::sync::mpsc vs std::sync::mpsc API differences
**What goes wrong:** `tokio::sync::mpsc::Receiver::recv()` is `async fn`. `std::sync::mpsc::Receiver::recv()` is synchronous (blocks). The loop patterns differ:
```rust
// tokio: must await
while let Some(event) = rx.recv().await { ... }
// std: blocks the thread
for trigger in rx { ... }   // uses Iterator on Receiver
```
**How to avoid:** Use the blocking iterator pattern on std Receiver. The tier engine thread
is dedicated — blocking is fine.

### Pitfall 4: rusqlite !Send — cannot share Connection across threads
**What goes wrong:** Attempting to create KnowledgeBase at main() and pass it to the tier engine
thread via Arc<Mutex<KnowledgeBase>> will fail at compile time: `rusqlite::Connection is not Send`.
**Why it happens:** Natural reflex to share expensive resources.
**How to avoid:** Open a fresh KnowledgeBase::open(KB_PATH) at the start of the tier engine
thread. SQLite file open is fast. Alternatively use r2d2 + r2d2-sqlite for a connection pool,
but that adds dependencies — per-thread open is simpler.
**Warning signs:** `error[E0277]: Connection cannot be sent between threads safely`.

### Pitfall 5: /mi/trigger HTTP POST from thin proxy — connection refused when rc-sentry is down
**What goes wrong:** If rc-sentry is down, the thin proxy's HTTP POST fails. This must NOT crash
rc-agent or block the diagnostic loop.
**Why it happens:** TCP connect with no timeout or error handling.
**How to avoid:** Use `TcpStream::connect_timeout(&addr, Duration::from_secs(3))` and return
`None` on error. Log at `debug` level (not warn — expected during deploys). rc-agent continues
running without MI coverage.

### Pitfall 6: Dedup map is per-process — resets on rc-sentry restart
**What goes wrong:** rc-agent's tier engine has a dedup map (T7: collapse same trigger within 5 min).
In rc-sentry, this map is in-memory in the tier engine thread. rc-sentry restart loses the dedup state.
**Why it happens:** rc-sentry restarts more often than rc-agent during deploy.
**How to avoid:** For Phase 322, accept this behavior — duplicate trigger processing after restart
is harmless (idempotent Tier 1 actions). Note it in the plan.

## Code Examples

Verified patterns from source code inspection:

### DiagnosticTrigger enum signature (move to rc-common)
```rust
// Source: crates/rc-agent/src/diagnostic_engine.rs lines 48-115
// Move to: crates/rc-common/src/diagnostic_types.rs
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagnosticTrigger {
    Periodic,
    HealthCheckFail,
    ProcessCrash { process_name: String },
    GameLaunchFail,
    DisplayMismatch { expected_edge_count: u32, actual_edge_count: u32 },
    ErrorSpike { errors_per_min: u64 },
    WsDisconnect { disconnected_secs: u64 },
    WsInstability { reconnects_5m: u64, reconnects_lifetime: u64 },
    SentinelUnexpected { file_name: String },
    ViolationSpike { delta: u64 },
    PreFlightFailed { check_name: String, detail: String },
    // ... POS-specific variants ...
    TaskbarVisible,
    GameMidSessionCrash { exit_code: Option<i32>, session_duration_secs: u64 },
    PostSessionAnalysis { session_quality_pct: u8 },
    PreShiftAudit,
    DeployVerification { new_build_id: String },
    GameLaunchTimeout { elapsed_secs: u64 },
}
```

### New TierDiagnosis type (new, add to rc-common)
```rust
// Source: NEW — composed from DiagnosticLogEntry (rc-agent) + success criteria
// Location: crates/rc-common/src/diagnostic_types.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierDiagnosis {
    pub trigger: DiagnosticTrigger,
    pub tier: u8,
    pub outcome: String,  // "fixed", "failed_to_fix", "not_applicable", "stub"
    pub action: String,
    pub root_cause: String,
    pub fix_type: String,
    pub confidence: f64,
    pub fix_applied: bool,
    pub problem_hash: String,
    pub timestamp: String,  // IST ISO-8601
}
```

### SolutionRecord (rename of Solution for sentry API clarity)
```rust
// Source: crates/rc-agent/src/knowledge_base.rs lines 29-72
// Move Solution → SolutionRecord in rc-common/src/diagnostic_types.rs
// Keep Solution as type alias in rc-agent for backward compat:
//   pub type Solution = rc_common::diagnostic_types::SolutionRecord;
pub type SolutionRecord = Solution;  // or full struct copy
```

### KnowledgeBase::lookup() signature (already synchronous)
```rust
// Source: crates/rc-agent/src/knowledge_base.rs (verified synchronous, no async)
// KB_PATH = r"C:\RacingPoint\mesh_kb.db"
impl KnowledgeBase {
    pub fn open(path: &str) -> anyhow::Result<Self>;
    pub fn lookup(&self, problem_key: &str) -> anyhow::Result<Option<Solution>>;
    pub fn store_solution(&self, solution: &Solution) -> anyhow::Result<()>;
    pub fn solution_count(&self) -> anyhow::Result<i64>;
    // normalize_problem_key, fingerprint_env, compute_problem_hash are free functions
}
```

### tier_engine::spawn() signature (rc-agent — what proxy replaces)
```rust
// Source: crates/rc-agent/src/tier_engine.rs lines 547-565
pub fn spawn(
    event_rx: mpsc::Receiver<DiagnosticEvent>,          // tokio mpsc
    budget: Arc<RwLock<BudgetTracker>>,                 // tokio RwLock
    diag_log: DiagnosticLog,
    staff_rx: mpsc::Receiver<StaffDiagnosticRequest>,   // tokio mpsc
    failure_monitor_rx: tokio::sync::watch::Receiver<FailureMonitorState>,
    fleet_bus_tx: tokio::sync::broadcast::Sender<FleetEvent>,
    ws_msg_tx: mpsc::Sender<rc_common::protocol::AgentMessage>,
    eval_store: Arc<Mutex<ModelEvalStore>>,
)
```

In Phase 322 (mi-engine disabled), this entire call is replaced by wiring diagnostic_event_rx
to mi_proxy::forward_to_sentry() and writing results to diag_log.

### rc-sentry main.rs spawn pattern (existing — follow this model)
```rust
// Source: crates/rc-sentry/src/main.rs lines 171-200
std::thread::Builder::new()
    .name("sentry-crash-handler".to_string())
    .spawn(move || {
        let recovery_logger = RecoveryLogger::new(RECOVERY_LOG_POD);
        // ... blocking loop ...
    })
    .expect("spawn crash handler thread");
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| MI engine only in rc-agent | MI engine in rc-sentry (this phase) | Phase 322 | rc-agent death no longer blinds MI |
| Full Tier 3/4 in every diagnosis | Tier 3/4 stubbed in rc-sentry Phase 322 | Phase 322 | No OpenRouter calls from sentry until Phase 323 |

**Deprecated/outdated after Phase 322:**
- Direct tier_engine::spawn() call in rc-agent main.rs (replaced by mi_proxy when feature flag off)

## Open Questions

1. **rusqlite !Send — per-event open vs OnceLock per-thread**
   - What we know: Connection::open() is fast (microseconds), SQLite file already exists
   - What's unclear: At 1 event/5 min, per-event open is trivially cheap. But Tier 2 tests open KB
     for lookup + optionally write a resolution record. Two opens per event = acceptable.
   - Recommendation: Per-event open. Avoids any thread-local storage complexity.

2. **Feature flag name for the proxy gate**
   - What we know: rc-agent currently has `ai-debugger` and other features in Cargo.toml
   - What's unclear: Whether to call it `mi-engine` (enables local MI) or `mi-proxy` (enables proxy)
   - Recommendation: `mi-engine` as a positive flag. Default = enabled (backward compatible).
     Phase 322 plan can build the proxy and test with `--no-default-features --features mi-proxy`.

3. **DiagnosticEvent vs DiagnosticTrigger for the proxy HTTP POST**
   - What we know: DiagnosticEvent wraps DiagnosticTrigger + FailureMonitorState + timestamp + build_id
   - What's unclear: Should the proxy POST the full DiagnosticEvent or just DiagnosticTrigger?
   - Recommendation: POST DiagnosticTrigger only. FailureMonitorState is not available to rc-sentry
     (and rc-sentry doesn't need it for Tier 1+2 logic). The tier engine in rc-sentry builds its own
     state from sysinfo. Simpler serialization, smaller payload.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| rusqlite (bundled) | KnowledgeBase in rc-sentry | Not in rc-sentry yet | 0.32 (in rc-agent) | No fallback — add to Cargo.toml |
| SQLite DB file | KnowledgeBase open | C:\RacingPoint\mesh_kb.db exists on all pods | Created by rc-agent | rc-sentry creates on first open if missing |
| sysinfo 0.33 | Diagnostic engine process scanning | Already in rc-sentry Cargo.toml | 0.33 | Already available |
| RCSENTRY_SERVICE_KEY env | /mi/trigger auth | Deployed on all pods (Phase 321) | — | No fallback — always required |

**Missing dependencies with no fallback:**
- rusqlite must be added to rc-sentry Cargo.toml before KnowledgeBase can compile there

**Missing dependencies with fallback:**
- mesh_kb.db: rc-sentry creates it on first open via run_migrations() if rc-agent hasn't yet

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (standard Rust) |
| Config file | none — workspace-level |
| Quick run command | `cargo test -p rc-common && cargo test -p rc-sentry` |
| Full suite command | `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p rc-sentry` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| MIG-01 | TierDiagnosis produced from DiagnosticTrigger::Periodic | unit | `cargo test -p rc-sentry -- tier_engine` | No — Wave 0 |
| MIG-01 | Tier 1 deterministic logic (MAINTENANCE_MODE clear) | unit | `cargo test -p rc-sentry -- tier1_deterministic` | No — Wave 0 |
| MIG-02 | DiagnosticTrigger sent via std::sync::mpsc | unit | `cargo test -p rc-sentry -- diagnostic_engine` | No — Wave 0 |
| MIG-02 | /debug endpoint returns last trigger + diagnosis | integration | manual curl :8091/debug | No — Wave 0 |
| MIG-03 | KnowledgeBase::lookup returns SolutionRecord from SQLite | unit | `cargo test -p rc-sentry -- knowledge_base` | No — Wave 0 |
| MIG-05 | rc-agent compiles with mi-engine feature disabled | build | `cargo build -p rc-agent-crate --no-default-features` | N/A — build test |
| MIG-05 | WS messages unchanged when proxy active | integration | manual WS message test | No — Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo check -p rc-sentry && cargo check -p rc-common && cargo check -p rc-agent-crate`
- **Per wave merge:** `cargo test -p rc-common && cargo test -p rc-sentry && cargo build -p rc-agent-crate`
- **Phase gate:** Full test suite green + manual /debug endpoint check + rc-agent WS test before verify

### Wave 0 Gaps
- [ ] `crates/rc-sentry/src/mi_tier_engine.rs` — test module for Tier 1+2 logic
- [ ] `crates/rc-sentry/src/mi_knowledge_base.rs` — test module for SQLite lookup
- [ ] `crates/rc-common/src/diagnostic_types.rs` — new module (needed before other tests)
- [ ] `crates/rc-agent/src/mi_proxy.rs` — new proxy module

## Sources

### Primary (HIGH confidence)
- Direct source inspection: `crates/rc-agent/src/tier_engine.rs` (2,968 lines) — spawn signature, tokio deps
- Direct source inspection: `crates/rc-agent/src/diagnostic_engine.rs` (783 lines) — DiagnosticTrigger enum, spawn signature
- Direct source inspection: `crates/rc-agent/src/knowledge_base.rs` (1,470 lines) — KnowledgeBase struct, rusqlite pattern, KB_PATH
- Direct source inspection: `crates/rc-agent/src/diagnostic_log.rs` (91 lines) — async fn push/recent, RwLock pattern
- Direct source inspection: `crates/rc-agent/src/mesh_gossip.rs` (465 lines) — uses KnowledgeBase + WS types only
- Direct source inspection: `crates/rc-sentry/src/main.rs` — pure-std threading, existing spawn pattern
- Direct source inspection: `crates/rc-sentry/Cargo.toml` — missing rusqlite confirmed
- Direct source inspection: `crates/rc-common/src/lib.rs` + `mesh_types.rs` — existing shared types
- Direct source inspection: `crates/rc-agent/src/main.rs` — entry points for MI modules

### Secondary (MEDIUM confidence)
- ROADMAP.md Phase 322 success criteria — TierDiagnosis, SolutionRecord as named types (confirmed NEW)
- STATE.md migration scope table — line counts confirmed (tier_engine 2,968, etc.)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all from direct Cargo.toml inspection
- Architecture (async-to-sync port): HIGH — all tokio usages found and documented
- Pitfalls: HIGH — all from direct code patterns found in source
- New types (TierDiagnosis, SolutionRecord): HIGH — success criteria names them, source confirms they don't exist yet

**Research date:** 2026-04-06
**Valid until:** Stable (Rust code doesn't drift; only new commits to these files would invalidate)
