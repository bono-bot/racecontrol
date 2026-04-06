# Phase 322: MI Core Engine Migration — Context

Generated: 2026-04-06

## Domain Boundary

rc-sentry runs the tier engine, diagnostic engine, knowledge base, and telemetry proxy
independently of rc-agent. rc-agent becomes a thin proxy that forwards telemetry to
rc-sentry :8091 — all existing WS messages continue working.

## Existing Assets (from codebase scout)

| Asset | Location | Lines | Reusable? |
|-------|----------|-------|-----------|
| DiagnosticTrigger enum (24 variants) | `rc-agent/src/diagnostic_engine.rs` | 783 | COPY to rc-common |
| DiagnosticEvent struct | `rc-agent/src/diagnostic_engine.rs` | inline | COPY to rc-common |
| KnowledgeBase struct + migrations | `rc-agent/src/knowledge_base.rs` | 1,470 | COPY to rc-sentry |
| Solution / HardenedRule / Experiment structs | `rc-agent/src/knowledge_base.rs` | inline | COPY to rc-common |
| diagnostic_log ring buffer | `rc-agent/src/diagnostic_log.rs` | 91 | COPY to rc-sentry (std port) |
| mesh_gossip WS message handlers | `rc-agent/src/mesh_gossip.rs` | 465 | DEFERRED (Phase 324) |
| tier_engine (5-tier decision tree) | `rc-agent/src/tier_engine.rs` | 2,968 | CORE of this phase |
| DiagnosisTier / FixType / SolutionStatus | `rc-common/src/mesh_types.rs` | existing | ALREADY shared |
| KB path constant | `rc-agent/src/knowledge_base.rs` | line 25 | REUSE (same path) |
| rc-sentry pure-std threading model | `rc-sentry/src/main.rs` | 3,952 | NO TOKIO — constraint |

## Key Finding: rc-sentry is Pure std, No Tokio

rc-sentry's `main.rs` opens with: "No tokio, no async — pure std::net for minimal binary size
and zero shared deps." The entire HTTP server is a blocking thread-per-connection model.

This is the most important architectural constraint for Phase 322:
- diagnostic_engine.spawn() uses tokio::spawn + tokio::time::interval + tokio::select!
- tier_engine.spawn() uses tokio::spawn + tokio::sync::mpsc/watch/RwLock/broadcast
- diagnostic_log.push() and .recent() are async functions (despite no actual async operation inside)
- KnowledgeBase is synchronous (rusqlite is blocking) — this is the easy part

## Decisions

### MIG-01: Tier Engine in rc-sentry — Threading Model

**Decision:** Port tier engine to std::thread + std::sync::mpsc (no tokio). The engine runs in
a dedicated thread, receives DiagnosticTrigger events via std::sync::mpsc::Receiver, and
produces TierDiagnosis structs stored in a shared std::sync::Mutex<Vec<TierDiagnosis>>.

**What we copy vs rewrite:**
- COPY: Tier 1 deterministic logic (sentinel clear, WerFault kill, process restart checks) — this
  is pure synchronous code that runs fine in std threads. Wrap sysinfo calls directly (no spawn_blocking
  needed in std context).
- COPY: Tier 2 KB lookup (rusqlite is synchronous — ideal for std model).
- STUB: Tier 3/4 (single-model + MMA) — Phase 323 handles these. For Phase 322, Tier 3/4 return
  TierResult::Stub immediately with a note.
- DROP: tokio::sync types — replace with std::sync equivalents.
- DROP: staff diagnostic bridge (no WS connection in rc-sentry in this phase).
- DROP: fleet_bus FleetEvent broadcast — rc-sentry has no fleet bus yet.
- DROP: ws_msg_tx (gossip) — Phase 324.
- DROP: eval_store / model_eval_store — Phase 323.

**New types needed in rc-common:**
```
TierDiagnosis { trigger: DiagnosticTrigger, tier: u8, outcome: String, action: String,
                root_cause: String, fix_type: String, confidence: f64, fix_applied: bool,
                problem_hash: String, timestamp: String }
```

**Why:** The success criterion says "produces a TierDiagnosis" — this is a new named type that
summarizes a complete tier engine run. It is the equivalent of what DiagnosticLogEntry captures
in rc-agent, but scoped to what rc-sentry needs to expose via /debug.

### MIG-02: Diagnostic Engine in rc-sentry — Trigger Detection

**Decision:** Port diagnostic engine to std::thread polling loop using std::time::Duration +
std::thread::sleep. Emit DiagnosticTrigger events (not DiagnosticEvent — sentry has no
FailureMonitorState or HeartbeatStatus) via std::sync::mpsc::Sender.

**What we detect in Phase 322 (subset):**
- Periodic (5-min scan): always emit
- HealthCheckFail: HTTP probe to :8090/health (already done in watchdog.rs — wire up)
- ProcessCrash: sysinfo scan for WerFault/WerReport (std thread, no spawn_blocking)
- SentinelUnexpected: fs::read_dir on C:\RacingPoint\ for unexpected sentinels

**What we skip in Phase 322** (requires rc-agent state not available in sentry):
- GameLaunchFail, DisplayMismatch, ErrorSpike, WsInstability — these need FailureMonitorState
- These remain in rc-agent's diagnostic_engine. Phase 324 can wire them via telemetry proxy.

**Channel type:** std::sync::mpsc::channel::<DiagnosticTrigger>() — NOTE: success criteria says
"no tokio dependency" for the channel. DiagnosticTrigger (not DiagnosticEvent) is sufficient
because rc-sentry has no FailureMonitorState to bundle.

**Observable via /debug:** Add a `/debug` endpoint to rc-sentry that returns last N
DiagnosticTrigger events received + last TierDiagnosis produced.

### MIG-03: Knowledge Base in rc-sentry

**Decision:** Copy KnowledgeBase struct directly into rc-sentry as a new module. Same SQLite
path (`C:\RacingPoint\mesh_kb.db`) — both rc-agent and rc-sentry read/write the same DB file.
rusqlite has no write-ahead locking conflicts in this use case (rc-sentry is read-heavy in
Phase 322; write conflicts with rc-agent are rare and SQLite handles them via busy timeout).

**New type — SolutionRecord:** The success criteria says "return a SolutionRecord". This is an alias
or renamed copy of the existing `Solution` struct. We expose it via rc-common so both the proxy
(rc-agent) and the real engine (rc-sentry) can refer to the same type.

**rusqlite in rc-sentry Cargo.toml:** Add `rusqlite = { version = "0.32", features = ["bundled"] }`.
No feature gate — KB is always available in Phase 322+.

**Why same path:** The existing KB has solutions from rc-agent's diagnosis history. rc-sentry should
inherit this knowledge immediately without any migration. The file is already at C:\RacingPoint\
on each pod.

### MIG-05: Thin Proxy in rc-agent

**Decision:** rc-agent's MI modules (tier_engine, diagnostic_engine, knowledge_base, mesh_gossip,
diagnostic_log) are NOT deleted — they are compile-gated behind a feature flag `mi-engine`.
When `mi-engine` is enabled (default remains true for now), rc-agent runs full MI as before.
When disabled, rc-agent uses a thin proxy.

**Why feature gate instead of delete:** Preserves rollback path. Phase 322 deploys both rc-sentry
(with new MI) and rc-agent (with thin proxy). If rc-sentry MI has bugs, disabling the proxy feature
instantly restores full rc-agent MI.

**Thin proxy behavior:**
- Receives DiagnosticTrigger events from its own diagnostic_engine (unchanged)
- POSTs them to rc-sentry :8091/mi/trigger via HTTP (std::net or reqwest — use std::net to
  avoid new dependency)
- rc-sentry receives, runs tier engine, returns TierDiagnosis JSON
- rc-agent logs the result in its DiagnosticLog ring buffer (unchanged consumer)
- All existing WS messages (StaffDiagnosticResult, DiagnosticResult) continue working because
  the WS handler reads from DiagnosticLog which the proxy populates

**HTTP endpoint on rc-sentry:** POST /mi/trigger — accepts DiagnosticTrigger JSON, returns
TierDiagnosis JSON. Protected by X-Service-Key header (existing auth pattern).

**No existing WS messages break:** The WS handler in rc-agent still reads from DiagnosticLog.
The proxy writes to DiagnosticLog after getting TierDiagnosis back from sentry. Net effect: same
data flows, different engine location.

## Types Moving to rc-common

| Type | Current Location | Move To | Reason |
|------|-----------------|---------|--------|
| DiagnosticTrigger | rc-agent/diagnostic_engine.rs | rc-common/diagnostic_types.rs | Both proxy (rc-agent) and engine (rc-sentry) need it for HTTP serialization |
| DiagnosticEvent | rc-agent/diagnostic_engine.rs | rc-common/diagnostic_types.rs | Proxy sends, sentry receives |
| TierDiagnosis | NEW | rc-common/diagnostic_types.rs | Engine produces, proxy consumes, /debug exposes |
| SolutionRecord | rc-agent/knowledge_base.rs (as Solution) | rc-common/diagnostic_types.rs | New alias/rename for sentry API clarity |

Types that STAY in rc-agent (not shared):
- FailureMonitorState — rc-agent-specific pod state
- StaffDiagnosticRequest/Result — rc-agent WS concern
- MmaDiagnosis — Phase 323 concern
- BudgetTracker — Phase 323 concern

## Scope Guard

Phase 322 covers: diagnostic engine (subset), tier engine (Tier 1+2 only), knowledge base, thin proxy.

Phase 322 does NOT cover:
- Tier 3/4 (MMA, single-model) — Phase 323
- CognitiveGate / DiagnosisPlanner — Phase 323
- mesh_gossip propagation — Phase 324
- Fleet bus / FleetEvent broadcast in rc-sentry — Phase 324
- Staff diagnostic bridge in rc-sentry — Phase 323

## Canonical References

- `crates/rc-agent/src/tier_engine.rs` — 5-tier decision tree (source of truth for logic)
- `crates/rc-agent/src/diagnostic_engine.rs` — trigger detection patterns
- `crates/rc-agent/src/knowledge_base.rs` — SQLite schema + query patterns
- `crates/rc-sentry/src/main.rs` — pure-std threading model to follow
- `crates/rc-sentry/src/watchdog.rs` — existing health poll (reuse for HealthCheckFail trigger)
- `crates/rc-common/src/mesh_types.rs` — DiagnosisTier, FixType, SolutionStatus (already shared)
- `crates/rc-sentry/Cargo.toml` — add rusqlite dependency

## Deferred Ideas

- Tier 3/4 (OpenRouter model calls) in rc-sentry — Phase 323
- Gossip propagation via rc-sentry — Phase 324
- Staff diagnostic HTTP endpoint on rc-sentry — Phase 323
- Delete rc-agent MI modules entirely — after Phase 323 ships and is stable
- tokio feature for rc-sentry — explicitly NOT planned (pure std is a design goal)
