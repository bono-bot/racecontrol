# Phase 323: MMA Engine & Cognitive Gate Migration — Context

Generated: 2026-04-06

## Domain Boundary

rc-sentry gains Tier 3 (single-model) and Tier 4/5 (multi-model MMA) diagnosis via OpenRouter,
plus a cognitive gate planner that produces structured fix plans. Both operate independently of
rc-agent. rc-agent's MMA engine and cognitive gate are de-prioritized behind the feature gate
established in Phase 322 — they remain functional for rollback but are no longer the primary path.

## Existing Assets (from codebase scout)

| Asset | Location | Lines | Reusable? |
|-------|----------|-------|-----------|
| MMA engine (4-step protocol) | `rc-agent/src/mma_engine.rs` | 1,891 | COPY key structs + logic — strip tokio/reqwest |
| openrouter HTTP client | `rc-agent/src/openrouter.rs` | ~600 | COPY get_api_key(), call_model() — rewrite HTTP as std::net or reqwest blocking |
| BudgetTracker (daily + monthly) | `rc-agent/src/budget_tracker.rs` | ~260 | COPY struct + can_spend/record_spend — needs SQLite persistence in sentry context |
| MmaCache (consensus cache) | `rc-agent/src/mma_cache.rs` | 215 | COPY directly — pure rusqlite, already synchronous |
| CgpEngine (cognitive gate) | `rc-agent/src/cognitive_gate.rs` | 733 | COPY — already mostly synchronous; strip DiagnosticEvent dependency |
| TierResult / TierDiagnosis | `rc-sentry/src/mi_tier_engine.rs` | existing | REUSE — already defined from Phase 322 |
| KnowledgeBase | `rc-sentry/src/mi_knowledge_base.rs` | existing | REUSE — from Phase 322, same SQLite path |
| MI debug state | `rc-sentry/src/mi_debug_state.rs` | existing | EXTEND — add MMA state + gate state |
| mi_tier_engine.rs Tier 3/4 stubs | `rc-sentry/src/mi_tier_engine.rs` | existing | REPLACE stubs with real logic |
| HTTP routing in handle() | `rc-sentry/src/main.rs` | existing | EXTEND — add /mma/status and /gate/last-plan |
| model roster (6 domains × 10 models) | `rc-agent/src/mma_engine.rs` | lines 250-325 | COPY verbatim |
| Finding / FixPlan / Execution structs | `rc-agent/src/mma_engine.rs` | lines 450-525 | COPY to rc-sentry |
| StepConsensus struct | `rc-agent/src/mma_engine.rs` | lines 483-525 | COPY to rc-sentry |
| 4 step system prompts | `rc-agent/src/mma_engine.rs` | lines 527-586 | COPY verbatim |
| FLEET_CONTEXT string | `rc-agent/src/openrouter.rs` | lines 47-76 | COPY verbatim |
| CognitiveGate fix planner helper fns | `rc-agent/src/cognitive_gate.rs` | lines 450-733 | COPY + adapt |

## Key Finding: reqwest blocking feature is acceptable for MMA

rc-sentry is pure std, no tokio. The constraint says:
"OR: use reqwest with blocking feature — this is acceptable since MMA calls are rare (1/session) and blocking is fine in a dedicated thread."

Phase 322 research confirms: the existing rc-sentry Cargo.toml does not have reqwest. We need to add it.
The MMA thread will run as a dedicated std::thread — blocking HTTP calls are fine there.

reqwest blocking is simpler than hand-rolling TLS over std::net::TcpStream. OpenRouter is HTTPS —
rustls/native-tls would be needed for std::net. reqwest with blocking feature handles all of this.

## Key Finding: BudgetTracker needs to be simpler in rc-sentry context

rc-agent's BudgetTracker uses Arc<RwLock<BudgetTracker>> where the RwLock is tokio::sync::RwLock
(MMA calls it with .await). In rc-sentry we use std::sync::Mutex instead. The budget tracker also
depends on budget_tracker_store.rs (a separate SQLite persistence layer) — we can inline this into
a simpler struct for the sentry context.

The success criterion says "enforces the $5/session cap" (not $10/day like rc-agent uses). The
per-session budget is simpler: track spent amount in an AtomicF64 equivalent, reset when a new
MMA session starts. Log to a local file at C:\RacingPoint\mma-spend.log.

## Key Finding: CgpEngine dependency on DiagnosticEvent needs removal

cognitive_gate.rs imports DiagnosticEvent from rc-agent's diagnostic_engine.rs. DiagnosticEvent
contains FailureMonitorState (pod-specific state: game_pid, billing_active, hid_connected) which
rc-sentry doesn't have.

Solution: In rc-sentry's cognitive gate, replace DiagnosticEvent with TierDiagnosis (already in
rc-common). The gate functions that use event.pod_state can use defaults or skip those fields.
The core gate logic (G0, G5, G7, G1, G2, G4, G8, G9) works with just the DiagnosticTrigger.

## Key Finding: DiagnosisPlanner for 5 common failure patterns is new code

rc-agent's cognitive_gate.rs is a gate validator (pass/fail checks). The success criterion adds
a diagnosis planner — a new component that takes a failure pattern name and produces a structured
fix sequence as a JSON array of actions with risk + rollback. This does not exist in rc-agent.

The 5 patterns required:
1. rc-agent crash: kill WerFault → clear sentinels → restart via RCWatchdog → verify spawn
2. game stuck: kill acs.exe/acs_x86.exe → clear game.pid → verify no billing orphan
3. MAINTENANCE_MODE: check TTL → clear sentinel → restart rc-agent → verify spawn_verified
4. WS disconnect: check server reachable → clear CLOSE_WAIT sockets → trigger reconnect
5. blanking failure: check edge_process_count → kill existing Edge → relaunch blank screen URL

This is a lookup table (pattern_key → Vec<Action>) backed by a static match expression in Rust.
Not an AI call — deterministic fix sequences.

## Decisions

### MIG-04: MMA Engine in rc-sentry

**Decision:** Port MMA engine to rc-sentry as a new `mma_engine.rs` module. The engine runs in
a dedicated std::thread. OpenRouter HTTP calls use reqwest with the blocking feature (add
`reqwest = { version = "0.12", features = ["blocking", "json"], default-features = false }` to
rc-sentry Cargo.toml). TLS: reqwest's native-tls-vendored feature gives HTTPS without system deps.

**Budget tracking:** New `MmaBudgetTracker` in rc-sentry (not a copy of rc-agent's — simpler).
Tracks: session_spent (f64, resets per-session), lifetime_spent (f64, persistent via SQLite).
Hard cap: $5.00/session. Logs every spend event to `C:\RacingPoint\mma-spend.log` (append,
one line per call: `timestamp | model | cost | cumulative_session`).

**OpenRouter key:** Read from `OPENROUTER_KEY` env var. Fallback: read from
`data/openrouter-mma-key.txt` (relative to rc-sentry CWD, which is `C:\RacingPoint\`). Same
priority as rc-agent — env first, then file.

**Consensus stored in KB:** On MMA completion, the consensus JSON is written to `mesh_kb.db`
via the existing mi_knowledge_base module (add a `store_mma_consensus` method). This satisfies
"record the consensus finding in the knowledge base."

**Trigger mechanism:** MMA is triggered by the tier engine when Tier 2 KB lookup fails to find
a high-confidence solution and the trigger is serious enough (HealthCheckFail, ProcessCrash,
GameLaunchFail — not Periodic). The tier engine sends a `TierDiagnosis` to the MMA engine via
a std::sync::mpsc::Sender<TierDiagnosis>. MMA engine thread blocks on Receiver.

**Why reqwest blocking over std::net + rustls:** MMA calls are rare (1/session max). reqwest
blocking saves ~200 lines of raw HTTP/TLS implementation. The blocking call runs in a dedicated
thread so there is no concern about blocking the main accept loop.

### MIG-06: Cognitive Gate & Diagnosis Planner in rc-sentry

**Decision:** Port CgpEngine to rc-sentry as `cognitive_gate.rs`. The struct methods that
accept DiagnosticEvent are adapted to accept TierDiagnosis instead. The gate logic that
references pod_state fields (game_pid, billing_active) is replaced with defaults that are
conservative (assume the worst = safer fix plans).

**Diagnosis planner:** New `DiagnosisPlanner` struct in `cognitive_gate.rs`. Static match on
failure pattern name → Vec<PlannedAction>. PlannedAction has: step (u8), command (String),
risk_level (safe/caution/dangerous), rollback (String), expected_outcome (String).

**Last plan storage:** After cognitive gate runs and produces a fix plan, the plan is stored
in a `OnceLock<Mutex<Option<DiagnosisPlan>>>` or similar in-process state. The HTTP handler
at `/gate/last-plan` reads from this state.

**DiagnosisPlan type (new, to rc-common):**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedAction {
    pub step: u8,
    pub command: String,
    pub risk_level: String,    // "safe" | "caution" | "dangerous"
    pub rollback: String,
    pub expected_outcome: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosisPlan {
    pub trigger_name: String,
    pub tier: u8,
    pub actions: Vec<PlannedAction>,
    pub created_at: String,     // IST ISO-8601
    pub confidence: f64,
    pub gate_results: Vec<serde_json::Value>,
}
```

## Types Moving to rc-common (Phase 323 additions)

| Type | Current Location | Move To | Reason |
|------|-----------------|---------|--------|
| PlannedAction | NEW | rc-common/src/diagnostic_types.rs | Exposed via /gate/last-plan, needs Serialize |
| DiagnosisPlan | NEW | rc-common/src/diagnostic_types.rs | Same reason |
| MmaConsensusEntry | NEW | rc-common/src/diagnostic_types.rs | Stored in KB, queried via /mma/status |

Types that STAY in rc-sentry (not shared):
- MmaBudgetTracker — sentry-specific, simpler than rc-agent's BudgetTracker
- Finding / FixPlan / Execution — internal MMA step types, not exposed to consumers
- StepConsensus — internal MMA protocol type

## Scope Guard

Phase 323 covers: MMA engine (Tier 3/4 in rc-sentry), budget tracker, cognitive gate planner,
diagnosis planner for 5 failure patterns, /mma/status endpoint, /gate/last-plan endpoint.

Phase 323 does NOT cover:
- Deleting rc-agent MMA engine — keep for rollback (Phase 324 eval)
- Gossip propagation of MMA findings to other pods — Phase 324
- Fleet bus / FleetEvent broadcast in rc-sentry — Phase 324
- Staff diagnostic HTTP bridge in rc-sentry — Phase 324
- Tier 5 (full multi-model with reputation tracking) in rc-sentry — out of scope

## Canonical References

- `crates/rc-agent/src/mma_engine.rs` — 4-step engine, model roster, step prompts, structs
- `crates/rc-agent/src/openrouter.rs` — get_api_key(), call_model(), FLEET_CONTEXT, model configs
- `crates/rc-agent/src/budget_tracker.rs` — BudgetTracker struct, can_spend(), record_spend()
- `crates/rc-agent/src/mma_cache.rs` — MmaCache struct (already synchronous, pure rusqlite)
- `crates/rc-agent/src/cognitive_gate.rs` — CgpEngine, gate functions, helper fns
- `crates/rc-sentry/src/main.rs` — handle() routing, HTTP response helpers, service_key() auth
- `crates/rc-sentry/src/mi_tier_engine.rs` — Tier 3/4 stubs to replace, TierResult enum
- `crates/rc-sentry/src/mi_knowledge_base.rs` — KB path, open(), SolutionRecord
- `crates/rc-sentry/src/mi_debug_state.rs` — Shared MI state pattern to extend
- `crates/rc-sentry/Cargo.toml` — add reqwest blocking feature

## Deferred Ideas

- Tier 5 (full reputation tracking, model demotion/promotion) in rc-sentry — Phase 324
- Gossip of MMA findings via mesh_client — Phase 324
- Deleting rc-agent MMA engine behind feature flag — after Phase 323 proven stable
- tokio for rc-sentry — explicitly NOT planned (pure std + blocking thread for MMA)
- OpenRouter key rotation / 401 auto-recovery — existing logic in rc-agent, copy later
