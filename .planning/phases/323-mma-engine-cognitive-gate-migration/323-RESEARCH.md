# Phase 323: MMA Engine & Cognitive Gate Migration — Research

**Researched:** 2026-04-06
**Domain:** Rust module migration — async-to-sync + blocking HTTP, MMA protocol in pure-std binary
**Confidence:** HIGH (all findings from direct source code inspection)

<user_constraints>
## User Constraints (from CONTEXT.md / Phase 322)

### Locked Decisions (inherited from Phase 321/322)
- rc-sentry is pure std::net, no tokio — MMA HTTP calls MUST use a dedicated blocking thread
- reqwest with `blocking` feature is explicitly permitted for MMA (rare, 1/session)
- Strictly sequential phases: 322 must be complete before 323
- OpenRouter key: OPENROUTER_KEY env var or data/openrouter-mma-key.txt — never hardcoded
- Budget: $5/session cap (not rc-agent's $10/day — per-session for sentry context)
- Must work without rc-agent running

### Claude's Discretion
- reqwest version (0.11 vs 0.12) — research current version
- Whether to use native-tls-vendored vs rustls-tls feature for reqwest
- Internal MMA thread trigger mechanism (mpsc from tier engine vs separate HTTP endpoint)
- File path for mma-spend.log
- Whether DiagnosisPlan types go to rc-common or stay in rc-sentry

### Deferred Ideas (OUT OF SCOPE for Phase 323)
- Tier 5 (reputation tracking, model demotion/promotion) in rc-sentry
- Gossip of MMA findings via mesh_client
- Deleting rc-agent MMA engine
- tokio for rc-sentry
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| MIG-04 | rc-sentry initiates MMA audit via OpenRouter, reads COMMS_PSK/OPENROUTER_KEY from env, records consensus in KB — without rc-agent | reqwest blocking in dedicated std::thread. OpenRouter API + rusqlite both synchronous in blocking context. |
| MIG-06 | rc-sentry cognitive gate evaluates diagnosis, produces JSON fix plan (risk + rollback), visible at /gate/last-plan; diagnosis planner covers 5 failure patterns | CgpEngine ports cleanly — strip DiagnosticEvent. DiagnosisPlanner is new static match table. /gate/last-plan follows existing endpoint pattern. |
</phase_requirements>

## Summary

Phase 323 brings Tier 3 (single-model) and Tier 4 (5-model MMA) diagnosis into rc-sentry,
plus a cognitive gate planner that produces structured fix plans. The MMA engine in rc-agent
uses tokio async throughout (reqwest async, Arc<RwLock<BudgetTracker>> with tokio::sync locks).
In rc-sentry the same logic runs in a dedicated std::thread with reqwest blocking feature —
this is the explicitly approved approach per phase constraints.

The cognitive gate (cognitive_gate.rs, 733 lines) is already mostly synchronous — it uses pure
std computations, chrono timestamps, and rusqlite KB lookups. The only adaptation needed is
replacing DiagnosticEvent (which bundles rc-agent-specific FailureMonitorState) with TierDiagnosis
(the output type already in rc-common from Phase 322). The diagnosis planner for the 5 failure
patterns is new code — a static lookup table mapping pattern names to ordered fix sequences.

The budget tracker is simplified for the sentry context: $5/session cap, in-memory AtomicU64
tracking (f64 arithmetic via bit-cast), spend events logged to append-only file, SQLite for
lifetime tracking. No daily reset needed — session-scoped is simpler and matches the success
criteria exactly.

**Primary recommendation:** Add reqwest blocking to rc-sentry Cargo.toml. New modules:
`mma_engine.rs` (port), `mma_budget.rs` (new simplified tracker), extend `cognitive_gate.rs`
with DiagnosisPlanner. Add /mma/status and /gate/last-plan to handle() routing. Extend
mi_debug_state to hold MMA + plan state.

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| reqwest (blocking) | 0.12 | HTTPS calls to OpenRouter API | Approved by constraint; handles TLS without custom rustls setup |
| rusqlite (bundled) | 0.32 | MMA cache + lifetime budget persistence | Already in rc-sentry from Phase 322 |
| serde_json | workspace | Serialize MMA findings, step consensus, plan | Already in rc-sentry |
| chrono | workspace | Timestamps for plan/consensus entries | Already in rc-sentry |
| std::sync::Mutex | std | Budget state, last-plan state shared across threads | No tokio needed |
| std::sync::mpsc | std | MMA trigger channel from tier engine | Established Phase 322 pattern |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| rand | 0.8 | Stratified model selection shuffle | Need for get_model_pool() stratified_select() |

### reqwest Feature Selection
The constraint allows reqwest blocking. Two TLS backend choices:
- `native-tls-vendored` — compiles OpenSSL into the binary, zero system deps. Larger binary (~2MB).
- `rustls-tls` — pure Rust TLS, also zero system deps. Smaller binary.

**Recommendation: `rustls-tls`** — aligns with rc-sentry's "minimal footprint" design goal.
reqwest already uses rustls internally for the async variant when configured this way.

**Installation (rc-sentry Cargo.toml additions):**
```toml
reqwest = { version = "0.12", features = ["blocking", "rustls-tls", "json"], default-features = false }
rand = { version = "0.8", features = ["small_rng"] }
```

Note: rand is needed for stratified_select()'s SliceRandom shuffle. rc-agent already has rand
in its Cargo.toml. rc-sentry does not.

**Version verification:** reqwest 0.12 is current as of research date. Confirmed against rc-agent
Cargo.toml which uses reqwest 0.12 for async context — same version for blocking is compatible.

## Architecture Patterns

### Recommended New Files (rc-sentry)

```
crates/rc-sentry/src/
├── mma_engine.rs          # MMA engine: trigger → 4-step OpenRouter protocol → consensus
├── mma_budget.rs          # MmaBudgetTracker: $5/session cap, spend log, SQLite lifetime
└── cognitive_gate.rs      # CgpEngine (adapted) + DiagnosisPlanner (new)
```

Existing files to extend:
```
crates/rc-sentry/src/
├── mi_tier_engine.rs      # Replace Tier 3/4 stubs with real MMA dispatch
├── mi_debug_state.rs      # Add MMA state + last-plan state
└── main.rs                # Add /mma/status and /gate/last-plan routes
```

New shared types (rc-common):
```
crates/rc-common/src/diagnostic_types.rs
    # Add: PlannedAction, DiagnosisPlan, MmaConsensusEntry
```

### Pattern 1: MMA Engine as Dedicated std::thread + mpsc

**What:** The tier engine sends a TierDiagnosis to the MMA engine via mpsc when Tier 2 fails.
The MMA engine runs in its own blocking thread, calls OpenRouter synchronously, returns result.

**When to use:** Always for Tier 3/4 in rc-sentry.

```rust
// In mi_tier_engine.rs (replacing Tier 3/4 stubs):
fn run_tier3(trigger: &DiagnosticTrigger, mma_tx: &Sender<TierDiagnosis>) -> TierResult {
    // Build a TierDiagnosis for MMA to consume
    let diag = TierDiagnosis { trigger: trigger.clone(), tier: 3, outcome: "pending".into(), ... };
    let _ = mma_tx.send(diag);
    // Return stub — MMA result stored in MMA_STATE asynchronously
    TierResult::Stub { tier: 3 }
}

// In mma_engine.rs:
pub fn spawn(trigger_rx: Receiver<TierDiagnosis>) {
    std::thread::Builder::new()
        .name("sentry-mma-engine".to_string())
        .spawn(move || {
            for diag in trigger_rx {
                let result = run_mma_protocol(&diag);
                // Store result in shared state for /mma/status
                update_mma_state(result);
            }
        })
        .expect("spawn mma engine");
}
```

### Pattern 2: Budget Tracker — AtomicU64 as f64 bit-cast

**What:** f64 doesn't implement atomic ops. The standard pattern is `AtomicU64` + `f64::from_bits`
/`f64::to_bits` for atomic f64 arithmetic. This avoids Mutex for the hot path (can_spend check).

```rust
use std::sync::atomic::{AtomicU64, Ordering};

static SESSION_SPENT: AtomicU64 = AtomicU64::new(0);
const SESSION_CAP: f64 = 5.0;

pub fn session_spent() -> f64 {
    f64::from_bits(SESSION_SPENT.load(Ordering::Relaxed))
}

pub fn can_spend(amount: f64) -> bool {
    session_spent() + amount <= SESSION_CAP
}

pub fn record_spend(model: &str, amount: f64) {
    let prev = f64::from_bits(SESSION_SPENT.fetch_add(amount.to_bits(), Ordering::Relaxed));
    log_spend_event(model, amount, prev + amount);
}

/// Reset session budget (called at MMA session start).
pub fn reset_session() {
    SESSION_SPENT.store(0.0f64.to_bits(), Ordering::Relaxed);
}
```

Note: AtomicU64::fetch_add with `f64::to_bits` only works correctly for monotonically increasing
values where the bit patterns add correctly — which is NOT true for IEEE 754 f64. The correct
approach is to use a `Mutex<f64>` instead:

```rust
use std::sync::Mutex;

static SESSION_SPENT: std::sync::OnceLock<Mutex<f64>> = std::sync::OnceLock::new();

fn spent() -> &'static Mutex<f64> {
    SESSION_SPENT.get_or_init(|| Mutex::new(0.0))
}

pub fn can_spend(amount: f64) -> bool {
    spent().lock().map(|v| *v + amount <= SESSION_CAP).unwrap_or(false)
}

pub fn record_spend(model: &str, amount: f64, cumulative: f64) {
    if let Ok(mut v) = spent().lock() { *v += amount; }
    log_spend_event(model, amount, cumulative);
}
```

**Warning signs:** Using f64::to_bits addition in AtomicU64 gives wrong results because IEEE 754
f64 addition is not the same as u64 addition. Always use Mutex for f64 accumulation.

### Pattern 3: Spend Log — Append-Only File

**What:** Each MMA model call appends a line to `C:\RacingPoint\mma-spend.log`.
Format: `2026-04-06T12:30:00Z | deepseek/deepseek-r1-0528 | $0.43 | session_total=$0.43`

```rust
fn log_spend_event(model: &str, cost: f64, session_total: f64) {
    use std::io::Write;
    let timestamp = chrono::Utc::now().to_rfc3339();
    let line = format!(
        "{} | {} | ${:.4} | session_total=${:.4}\n",
        timestamp, model, cost, session_total
    );
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true).append(true)
        .open(r"C:\RacingPoint\mma-spend.log")
    {
        let _ = f.write_all(line.as_bytes());
    }
}
```

### Pattern 4: OpenRouter call_model() — blocking reqwest

**What:** Port rc-agent's async call_model() to blocking reqwest.

```rust
// Source: rc-agent/src/openrouter.rs — async version adapted for blocking
pub fn call_model_blocking(
    client: &reqwest::blocking::Client,
    api_key: &str,
    model: &ModelConfig,
    prompt: &str,
) -> Option<DiagnosisResult> {
    let body = serde_json::json!({
        "model": model.id,
        "messages": [
            {"role": "system", "content": model.system_prompt},
            {"role": "user", "content": sanitize_mma_prompt(prompt)},
        ],
        "max_tokens": 4000,
        "temperature": 0.1,
    });

    let resp = client
        .post("https://openrouter.ai/api/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .timeout(std::time::Duration::from_secs(60))
        .json(&body)
        .send();

    // Parse response, extract cost, extract JSON diagnosis
    // Return None on error (never .unwrap())
}
```

### Pattern 5: get_api_key() — env var + file fallback

**What:** Read OPENROUTER_KEY from env, fall back to file. Exactly mirrors rc-agent pattern.

```rust
pub fn get_api_key() -> Option<String> {
    // 1. Environment variable (highest priority)
    if let Ok(k) = std::env::var("OPENROUTER_KEY") {
        if !k.is_empty() { return Some(k); }
    }
    // 2. File fallback: C:\RacingPoint\data\openrouter-mma-key.txt
    let path = r"C:\RacingPoint\data\openrouter-mma-key.txt";
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}
```

### Pattern 6: /mma/status endpoint (public — no auth)

**What:** Returns MMA budget state + last run result. Follows existing /health pattern.

```rust
// In handle() match block — public routes:
("GET", "/mma/status") => return handle_mma_status(&mut stream),

fn handle_mma_status(stream: &mut TcpStream) -> Result<(), Box<dyn std::error::Error>> {
    let state = mma_engine::get_status(); // reads from OnceLock<Mutex<MmaStatus>>
    let body = serde_json::to_string(&state).unwrap_or_else(|_| "{}".to_string());
    send_response(stream, 200, &body)
}
```

MmaStatus struct:
```rust
#[derive(Serialize)]
struct MmaStatus {
    session_spent_usd: f64,
    session_cap_usd: f64,
    remaining_usd: f64,
    last_run_trigger: Option<String>,
    last_run_outcome: Option<String>,   // "success" | "budget_exhausted" | "api_unavailable"
    last_run_cost: Option<f64>,
    last_run_at: Option<String>,
    models_called: Vec<String>,
    spend_log_path: &'static str,
}
```

### Pattern 7: /gate/last-plan endpoint (public — no auth)

```rust
("GET", "/gate/last-plan") => return handle_gate_last_plan(&mut stream),

fn handle_gate_last_plan(stream: &mut TcpStream) -> Result<(), Box<dyn std::error::Error>> {
    let plan = cognitive_gate::get_last_plan();
    let body = serde_json::to_string(&plan).unwrap_or_else(|_| "null".to_string());
    send_response(stream, 200, &body)
}
```

### Pattern 8: DiagnosisPlanner — static match on failure pattern

**What:** The 5 required failure patterns mapped to ordered fix sequences.

```rust
pub fn plan_for_trigger(trigger: &DiagnosticTrigger) -> DiagnosisPlan {
    let (trigger_name, actions) = match trigger {
        DiagnosticTrigger::ProcessCrash { process_name } if process_name.contains("rc-agent") => {
            ("rc-agent-crash", vec![
                PlannedAction { step: 1, command: "taskkill /F /IM WerFault.exe".into(),
                    risk_level: "safe".into(), rollback: "none needed".into(),
                    expected_outcome: "WerFault dialog closed".into() },
                PlannedAction { step: 2, command: "del C:\\RacingPoint\\MAINTENANCE_MODE 2>nul & del C:\\RacingPoint\\rcagent-restart-sentinel.txt 2>nul".into(),
                    risk_level: "safe".into(), rollback: "none — sentinel re-created if needed".into(),
                    expected_outcome: "Restart sentinels cleared".into() },
                PlannedAction { step: 3, command: "RCWatchdog triggers restart (no exec needed)".into(),
                    risk_level: "safe".into(), rollback: "schtasks /Run /TN StartRCAgent".into(),
                    expected_outcome: "rc-agent running in Session 1 within 15s".into() },
                PlannedAction { step: 4, command: "curl -s http://127.0.0.1:8090/health".into(),
                    risk_level: "safe".into(), rollback: "escalate to WhatsApp".into(),
                    expected_outcome: "status=ok, build_id present".into() },
            ])
        },
        DiagnosticTrigger::GameLaunchFail | DiagnosticTrigger::GameLaunchTimeout { .. } => {
            ("game-stuck", vec![ /* ... */ ])
        },
        DiagnosticTrigger::SentinelUnexpected { file_name } if file_name.contains("MAINTENANCE") => {
            ("maintenance-mode", vec![ /* ... */ ])
        },
        DiagnosticTrigger::WsDisconnect { .. } => {
            ("ws-disconnect", vec![ /* ... */ ])
        },
        DiagnosticTrigger::DisplayMismatch { .. } => {
            ("blanking-failure", vec![ /* ... */ ])
        },
        _ => ("generic", vec![
            PlannedAction { step: 1, command: "curl -s http://127.0.0.1:8090/health".into(),
                risk_level: "safe".into(), rollback: "none".into(),
                expected_outcome: "health status".into() },
        ])
    };
    DiagnosisPlan {
        trigger_name: trigger_name.to_string(),
        tier: 4,
        actions,
        created_at: chrono::Utc::now().to_rfc3339(),
        confidence: 0.9,
        gate_results: vec![],
    }
}
```

### Anti-Patterns to Avoid

- **AtomicU64 + f64::to_bits for accumulation:** IEEE 754 addition != u64 addition. Use Mutex<f64>.
- **Blocking reqwest on the main accept thread:** NEVER call reqwest::blocking in the handle() thread.
  MMA calls MUST run in the dedicated mma-engine std::thread.
- **Sharing reqwest::blocking::Client across threads via Arc:** reqwest::blocking::Client IS thread-safe
  (it's Arc<ClientRef> internally). Safe to share via Arc<Client>.
- **Storing full StepConsensus in OnceLock without size bound:** consensus JSON can be 50KB+.
  Store only a summary in the status endpoint; full consensus goes to SQLite.
- **Calling mma_engine from tier engine thread directly:** MMA must be async from tier engine's
  perspective — send trigger via mpsc, don't block the tier engine thread for 60s+ MMA calls.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| HTTPS to OpenRouter | TLS over std::net::TcpStream | reqwest blocking | TLS handshake implementation is ~500 lines of rustls config; reqwest handles retries, timeouts, JSON |
| Model selection randomization | Custom PRNG | rand::seq::SliceRandom | SliceRandom::choose() handles the stratified selection already in mma_engine.rs |
| MMA consensus cache | In-memory HashMap | rusqlite (already in rc-sentry) | rusqlite MmaCache from rc-agent is already fully synchronous — copy directly |
| Spend log parsing | JSON or binary format | Append-only text (one line per event) | Text log is sufficient for /mma/status — SQLite stores lifetime total |

**Key insight:** The hardest part is NOT the HTTP calls — reqwest blocking makes that trivial.
The challenge is correctly adapting CgpEngine to accept TierDiagnosis instead of DiagnosticEvent
without breaking the gate logic that depends on pod_state fields.

## Runtime State Inventory

> Phase 323 adds new persistent state but does not rename existing state.

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | `C:\RacingPoint\mesh_kb.db` — add `mma_cache` table (schema from mma_cache.rs, already in rc-agent) | Run CREATE TABLE IF NOT EXISTS on first open — no migration risk |
| Stored data | `C:\RacingPoint\mma-spend.log` — new file | Created on first MMA call; append-only |
| Stored data | Budget lifetime in mesh_kb.db — new `mma_budget` table | CREATE TABLE IF NOT EXISTS |
| Live service config | rc-sentry.toml — no MMA-related config today | Optionally add `[mma] enabled = true` section |
| OS-registered state | None specific to MMA | None |
| Secrets/env vars | `OPENROUTER_KEY` — must be deployed to all 8 pods in start-rcsentry.bat or system env | Add to start-rcsentry.bat as `set OPENROUTER_KEY=...` OR use the file fallback at `C:\RacingPoint\data\openrouter-mma-key.txt` |
| Build artifacts | Existing rc-sentry.exe on all 8 pods | Normal hash-based deploy after build |

**OPENROUTER_KEY deployment decision:** The file fallback (`C:\RacingPoint\data\openrouter-mma-key.txt`)
is safer than embedding in bat file (reduces exposure in process listing). Plan should create
this file on each pod during deploy, or use existing mechanism in rc-agent. The key is already
deployed for rc-agent — rc-sentry can read the same file from the same CWD.

## Common Pitfalls

### Pitfall 1: reqwest::blocking in async context
**What goes wrong:** If reqwest blocking is called from within a tokio async context (even
accidentally), it panics with "Cannot block the current thread from within an async context."
**Why it happens:** rc-sentry has no tokio runtime, so this is NOT a risk in rc-sentry itself.
But if a future phase adds tokio to rc-sentry, the MMA blocking calls would need to move to
spawn_blocking.
**How to avoid:** Keep MMA in its own std::thread. Never call blocking I/O from async code.
**Warning signs:** `thread '<unnamed>' panicked at 'Cannot block the current thread from within an async context'`

### Pitfall 2: reqwest::blocking::Client is not Clone (workaround: Arc)
**What goes wrong:** reqwest::blocking::Client cannot be cloned via #[derive(Clone)] in all
contexts because of internal state.
**Why it happens:** The blocking client holds connection pool state.
**How to avoid:** Create one Client in the MMA thread and reuse it for all calls within that
thread. The client is Send, so it can be stored in thread-local or passed as a parameter.
No need for Arc — the MMA engine is a single dedicated thread.
**Warning signs:** Compiler error about Clone not implemented.

### Pitfall 3: DiagnosticEvent → TierDiagnosis adaptation breaks G5/G9 KB lookup
**What goes wrong:** Gate functions G5 (competing hypotheses) and G9 (retrospective) call
`kb.lookup_all(&normalize_problem_key(&event.trigger), 3)`. TierDiagnosis has the trigger
but normalize_problem_key() is defined in rc-agent/knowledge_base.rs.
**Why it happens:** normalize_problem_key() is a free function, not in rc-common.
**How to avoid:** Move normalize_problem_key() to rc-common/src/diagnostic_types.rs as a
free function. It depends only on DiagnosticTrigger (already in rc-common).
**Warning signs:** `unresolved import crate::knowledge_base::normalize_problem_key`

### Pitfall 4: Mutex poisoning if MMA thread panics
**What goes wrong:** If the MMA engine thread panics while holding SESSION_SPENT.lock(),
the Mutex becomes poisoned. Subsequent lock() calls return Err(PoisonError).
**Why it happens:** Thread panic during OpenRouter HTTP call (network error, parse error).
**How to avoid:** Handle PoisonError on all .lock() calls: `spent().lock().unwrap_or_else(|p| p.into_inner())`.
Better: catch all panics in the MMA thread using std::panic::catch_unwind.
**Warning signs:** All subsequent MMA calls see `can_spend() = false` (returns false on lock error).

### Pitfall 5: OPENROUTER_KEY absent silently disables MMA
**What goes wrong:** MMA returns ApiUnavailable immediately if key is missing. No alert.
**Why it happens:** Key not deployed to pods when sentry binary is deployed.
**How to avoid:** Add a startup check: if OPENROUTER_KEY is absent AND file fallback is absent,
log at ERROR level and set MMA_DISABLED flag. /mma/status reports this condition.
**Warning signs:** /mma/status returns `{"last_run_outcome": "api_unavailable", "reason": "OPENROUTER_KEY not set"}`

### Pitfall 6: reqwest native-tls vs rustls on Windows — Windows SChannel
**What goes wrong:** reqwest's native-tls on Windows uses SChannel (OS TLS). SChannel sometimes
fails certificate verification for OpenRouter's certificate chain on older Windows builds.
**Why it happens:** Missing intermediate certificates in Windows cert store.
**How to avoid:** Use `rustls-tls` feature instead of `native-tls`. rustls bundles its own
CA certificates (via webpki-roots or rustls-native-certs) and does not depend on the Windows cert store.
**Warning signs:** `error sending request: certificate verify failed` or `TLS handshake failed`

### Pitfall 7: normalize_problem_key function location
**What goes wrong:** The G5/G9 cognitive gate functions need normalize_problem_key() but it
currently lives in rc-agent/src/knowledge_base.rs. Copying it to rc-sentry creates duplication.
**How to avoid:** Move it to rc-common/src/diagnostic_types.rs. It's a pure function of DiagnosticTrigger.
The function is already used in rc-sentry's mi_knowledge_base.rs and mi_tier_engine.rs — centralize it.

## Code Examples

Verified patterns from source code inspection:

### MMA get_api_key() — full logic from openrouter.rs
```rust
// Source: rc-agent/src/openrouter.rs (verified pattern to copy)
pub fn get_api_key() -> Option<String> {
    if let Ok(k) = std::env::var("OPENROUTER_KEY") {
        if !k.is_empty() { return Some(k); }
    }
    // File fallback: same dir as binary / CWD
    let paths = [
        r"C:\RacingPoint\data\openrouter-mma-key.txt",
        "data/openrouter-mma-key.txt",
        "openrouter-mma-key.txt",
    ];
    for path in &paths {
        if let Ok(k) = std::fs::read_to_string(path) {
            let k = k.trim().to_string();
            if !k.is_empty() { return Some(k); }
        }
    }
    None
}
```

### MmaBudgetTracker — simplified sentry version
```rust
// New for rc-sentry — session-scoped, $5 cap
use std::sync::{Mutex, OnceLock};

const SESSION_CAP_USD: f64 = 5.0;
const SPEND_LOG_PATH: &str = r"C:\RacingPoint\mma-spend.log";

static SESSION_BUDGET: OnceLock<Mutex<f64>> = OnceLock::new();

fn session_budget() -> &'static Mutex<f64> {
    SESSION_BUDGET.get_or_init(|| Mutex::new(0.0))
}

pub fn can_spend(amount: f64) -> bool {
    session_budget().lock()
        .map(|v| *v + amount <= SESSION_CAP_USD)
        .unwrap_or(false)
}

pub fn record_spend(model: &str, amount: f64) {
    let cumulative = if let Ok(mut v) = session_budget().lock() {
        *v += amount;
        *v
    } else { amount };

    // Append to log
    use std::io::Write;
    let line = format!(
        "{} | {} | ${:.4} | session=${:.4}\n",
        chrono::Utc::now().to_rfc3339(), model, amount, cumulative
    );
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true).append(true).open(SPEND_LOG_PATH)
    { let _ = f.write_all(line.as_bytes()); }
}

pub fn reset_session() {
    if let Ok(mut v) = session_budget().lock() { *v = 0.0; }
}

pub fn get_status() -> MmaStatus {
    let spent = session_budget().lock().map(|v| *v).unwrap_or(0.0);
    MmaStatus {
        session_spent_usd: spent,
        session_cap_usd: SESSION_CAP_USD,
        remaining_usd: (SESSION_CAP_USD - spent).max(0.0),
        spend_log_path: SPEND_LOG_PATH,
        // ... last_run fields from MMA_STATE
    }
}
```

### TierDiagnosis → cognitive gate adaptation
```rust
// Source: rc-sentry will use TierDiagnosis (from rc-common) instead of DiagnosticEvent
// cognitive_gate.rs in rc-sentry — run_phase_a signature change:

pub fn run_phase_a(
    diag: &TierDiagnosis,        // was: event: &DiagnosticEvent
    kb: Option<&KnowledgeBase>,
) -> Result<Vec<CgpGateResult>, CgpError> {
    // G0: use diag.trigger instead of event.trigger
    let g0 = Self::gate_g0_problem_definition(&diag.trigger);
    // G5: use diag.trigger for KB lookup
    let g5 = Self::gate_g5_competing_hypotheses(&diag.trigger, kb);
    // G7: pass diag.tier instead of DiagnosisTier from event
    let tier = match diag.tier {
        3 => DiagnosisTier::SingleModel,
        4 | 5 => DiagnosisTier::MultiModel,
        _ => DiagnosisTier::Deterministic,
    };
    let g7 = Self::gate_g7_tool_verification(&diag.trigger, tier);
    Ok(vec![g0, g5, g7])
}
```

### Finding / FixPlan / StepConsensus structs (copy from mma_engine.rs)
```rust
// Source: rc-agent/src/mma_engine.rs lines 450-525
// These are internal to rc-sentry's mma_engine.rs — do NOT put in rc-common
// (they are large intermediate protocol types, not API types)

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding { pub id: String, pub description: String, pub severity: String,
    pub confidence: f64, pub evidence: Vec<String>, pub assumptions: Vec<String>,
    pub verification_steps: Vec<String> }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixPlan { pub problem_id: String, pub actions: Vec<String>,
    pub fix_type: String, pub risk_analysis: String, pub rollback_strategy: String,
    pub verification_steps: Vec<String>, pub estimated_duration_secs: u64 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Execution { pub problem_id: String, pub implementation: String,
    pub execution_order: u8, pub expected_outcome: String, pub confidence: f64 }
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| MMA only in rc-agent (dies with rc-agent) | MMA in rc-sentry (survives rc-agent death) | Phase 323 | Diagnosis continues even when rc-agent is fully dead |
| Tier 3/4 stubbed in rc-sentry | Tier 3 (single-model) + Tier 4 (MMA) real in rc-sentry | Phase 323 | Real model diagnosis from sentry |
| Budget tracked only in rc-agent | Budget tracked in rc-sentry independently | Phase 323 | $5/session cap enforced at sentry level |
| No fix plan visible externally | /gate/last-plan endpoint | Phase 323 | Operators can query fix plans without access to rc-agent |

**Deprecated/outdated after Phase 323:**
- Tier 3/4 Stub results in mi_tier_engine.rs (replaced with real dispatch)

## Open Questions

1. **Tier 3 (single model) vs direct MMA dispatch**
   - What we know: Success criterion says "initiate an MMA audit" (implies full 4-step)
   - What's unclear: Should Tier 3 (single model) also run in rc-sentry or skip to Tier 4?
   - Recommendation: Implement Tier 3 as a simplified single-model call (cheapest model,
     qwen/qwen3-235b-a22b-2507) that triggers before full MMA. This mirrors rc-agent behavior.

2. **MMA trigger rate limiting**
   - What we know: $5/session cap. "Session" for rc-sentry = time since last reset? Process restart?
   - What's unclear: What constitutes a "session" for rc-sentry (no billing sessions, no game sessions)
   - Recommendation: Session = process lifetime (reset on restart). This matches "1/session" as a
     conservative interpretation. Document in /mma/status response.

3. **normalize_problem_key location**
   - What we know: Used in rc-sentry/mi_knowledge_base.rs already (imported or copied)
   - What's unclear: Is it already in rc-common after Phase 322? If not, must move before Phase 323.
   - Recommendation: Check mi_knowledge_base.rs for how it handles normalize_problem_key.
     If missing, add to rc-common/src/diagnostic_types.rs in Wave 0.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| reqwest (blocking) | OpenRouter HTTP calls | Not in rc-sentry Cargo.toml yet | 0.12 | No fallback — must add |
| rand 0.8 | stratified_select() | Not in rc-sentry Cargo.toml | 0.8 | rand::random (no SliceRandom) — add rand |
| OPENROUTER_KEY env | MMA API calls | Unknown — not deployed to sentry yet | — | File fallback at C:\RacingPoint\data\openrouter-mma-key.txt |
| C:\RacingPoint\mesh_kb.db | MMA cache + budget | Created by rc-agent Phase 322+ | — | rc-sentry creates on first open |
| C:\RacingPoint\data\ | Key file fallback dir | May not exist on pods | — | Create directory in deploy |
| internet access from pods | OpenRouter calls | LAN → internet via venue router | — | Ollama fallback (127.0.0.1:11434) — exists on James only |

**Missing dependencies with no fallback:**
- reqwest blocking must be added to rc-sentry Cargo.toml
- rand must be added to rc-sentry Cargo.toml

**Missing dependencies with fallback:**
- OPENROUTER_KEY: file fallback at C:\RacingPoint\data\openrouter-mma-key.txt
- Internet access from pods: pods have outbound internet but not guaranteed. MMA returns ApiUnavailable gracefully.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (standard Rust) |
| Config file | none — workspace-level |
| Quick run command | `cargo check -p rc-sentry && cargo check -p rc-common` |
| Full suite command | `cargo test -p rc-common && cargo test -p rc-sentry` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| MIG-04 | get_api_key() reads OPENROUTER_KEY from env | unit | `cargo test -p rc-sentry -- mma_engine::get_api_key` | No — Wave 0 |
| MIG-04 | can_spend(4.99) true, can_spend(5.01) false | unit | `cargo test -p rc-sentry -- mma_budget::budget_cap` | No — Wave 0 |
| MIG-04 | record_spend() appends to mma-spend.log | unit | `cargo test -p rc-sentry -- mma_budget::spend_log` | No — Wave 0 |
| MIG-04 | MmaCache::put/get round-trip in-memory | unit | `cargo test -p rc-sentry -- mma_cache` (copy from rc-agent tests) | No — Wave 0 |
| MIG-04 | /mma/status returns JSON with session_spent_usd | integration | manual curl :8091/mma/status | No — Wave 0 |
| MIG-06 | DiagnosisPlanner produces 4+ actions for rc-agent-crash trigger | unit | `cargo test -p rc-sentry -- cognitive_gate::plan_rc_agent_crash` | No — Wave 0 |
| MIG-06 | DiagnosisPlanner covers all 5 required patterns | unit | `cargo test -p rc-sentry -- cognitive_gate::plan_coverage` | No — Wave 0 |
| MIG-06 | /gate/last-plan returns null before any plan | integration | manual curl :8091/gate/last-plan | No — Wave 0 |
| MIG-06 | /gate/last-plan returns valid DiagnosisPlan JSON after trigger | integration | trigger + curl :8091/gate/last-plan | No — Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo check -p rc-sentry && cargo check -p rc-common`
- **Per wave merge:** `cargo test -p rc-common && cargo test -p rc-sentry`
- **Phase gate:** Full test suite green + curl :8091/mma/status + curl :8091/gate/last-plan before /gsd:verify-work

### Wave 0 Gaps
- [ ] `crates/rc-sentry/src/mma_engine.rs` — new module (MIG-04 core)
- [ ] `crates/rc-sentry/src/mma_budget.rs` — new module (MIG-04 budget)
- [ ] `crates/rc-sentry/src/cognitive_gate.rs` — new module (MIG-06)
- [ ] `crates/rc-common/src/diagnostic_types.rs` — add PlannedAction, DiagnosisPlan types
- [ ] `crates/rc-sentry/Cargo.toml` — add reqwest blocking + rand

## Sources

### Primary (HIGH confidence)
- Direct source inspection: `crates/rc-agent/src/mma_engine.rs` (1,891 lines) — full 4-step engine, model roster, structs, step prompts
- Direct source inspection: `crates/rc-agent/src/openrouter.rs` (~600 lines) — get_api_key(), call_model(), FLEET_CONTEXT, 5-model roster
- Direct source inspection: `crates/rc-agent/src/budget_tracker.rs` (~260 lines) — BudgetTracker struct, can_spend(), record_spend(), IST midnight reset
- Direct source inspection: `crates/rc-agent/src/mma_cache.rs` (215 lines) — MmaCache struct, put/get, TTL logic, build_id invalidation
- Direct source inspection: `crates/rc-agent/src/cognitive_gate.rs` (733 lines) — CgpEngine, all gate functions, DiagnosisPlanner absence
- Direct source inspection: `crates/rc-sentry/src/main.rs` — handle() routing, send_response() helpers, public vs protected routes
- Direct source inspection: `crates/rc-sentry/src/mi_tier_engine.rs` — Tier 3/4 stubs to replace
- Direct source inspection: `crates/rc-sentry/src/mi_knowledge_base.rs` — KB_PATH, KnowledgeBase::open()
- Direct source inspection: `crates/rc-sentry/Cargo.toml` — confirmed: no reqwest, no rand
- Direct source inspection: `crates/rc-common/src/diagnostic_types.rs` (Phase 322) — TierDiagnosis, DiagnosticTrigger
- Direct source inspection: ROADMAP.md Phase 323 — success criteria, exact endpoint paths
- Direct source inspection: 322-CONTEXT.md — Deferred items now in scope, architectural decisions

### Secondary (MEDIUM confidence)
- ROADMAP.md constraint "reqwest with blocking feature is acceptable" — verifies reqwest approach
- CLAUDE.md MMA standing rules — confirm OPENROUTER_KEY env var pattern, $5 budget, sanitize_mma_prompt

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all from direct Cargo.toml + source inspection
- Architecture (blocking reqwest thread): HIGH — explicitly approved by constraint, reqwest 0.12 confirmed in rc-agent
- Pitfalls: HIGH — all from direct code patterns (DiagnosticEvent dep, normalize_problem_key location, f64 atomic trap)
- New types (PlannedAction, DiagnosisPlan): HIGH — success criteria names them and describes structure
- DiagnosisPlanner 5 patterns: HIGH — success criteria names all 5 exactly

**Research date:** 2026-04-06
**Valid until:** Stable (Rust source + OpenRouter API stable; only new commits invalidate)
