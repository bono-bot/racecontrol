# Stack Research

**Domain:** Rust verification framework, observable state machines, boot resilience — v25.0 Debug-First-Time-Right
**Researched:** 2026-03-26 IST
**Confidence:** HIGH for crates already in workspace; MEDIUM for new crate additions (verified via docs.rs and official repos)

---

## Context: The Constraint

This is NOT a greenfield Rust service. The constraint is tight:

- Existing workspace: `tokio 1`, `tracing 0.1`, `tracing-subscriber 0.3`, `axum 0.8`, `serde_json 1`, `anyhow 1`, `thiserror 2`, `sqlx 0.8`
- New crate additions must clear the bar: "no new dep where an existing primitive suffices"
- Pod binaries (rc-agent, rc-sentry) require binary rebuild + fleet deploy for every Cargo.toml change
- The v25.0 goal is observability and correctness patterns, not new runtime infrastructure

**Philosophy for this stack:** most of v25.0 is patterns and macros built on existing deps, not new crates. The few genuine additions are targeted and small.

---

## What Already Exists (Do Not Re-Add)

| Capability | Already Provided By | Used In |
|------------|---------------------|---------|
| Structured logging with spans | `tracing 0.1` (workspace) | All crates |
| Async retry with backoff | `tokio::time::sleep` + custom `EscalatingBackoff` in `rc-common` | self_monitor.rs, ws_handler.rs |
| Phased startup log | `startup_log.rs` (custom, in rc-agent) | main.rs boot sequence |
| Pre-flight check framework | `pre_flight.rs` (custom, in rc-agent) | BillingStarted handler |
| File read/exist checks | `std::fs` | MAINTENANCE_MODE, GRACEFUL_RELAUNCH sentinels |
| Periodic re-fetch | `tokio::time::interval` (done in `821c3031`) | process_guard allowlist 300s |
| State broadcast | `tokio::sync::broadcast` | ws_handler.rs fleet alerts |
| Config file parsing | `toml 0.8` + `serde` (workspace) | config.rs in both crates |

---

## Recommended Stack

### Core Technologies (all already in workspace — zero new deps)

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| `tracing` | 0.1 (workspace) | Chain-of-verification spans, state transition events, silent failure elimination | Already the project logger. `#[instrument]` + manual `info_span!` provide per-step spans for data flow verification. `warn!` / `error!` with structured fields (`cause=`, `value=`) surface silent errors. This is the primary tool for 4 of 7 v25.0 goals. |
| `tokio::time::interval` | tokio 1 (workspace) | Periodic re-fetch loops — configs, allowlists, feature flags fetched at boot | Already used for allowlist 300s re-fetch. Extend the same pattern to all boot-time-fetch resources. No new dep. |
| `tokio::sync::watch` | tokio 1 (workspace) | Observable state values — single writer, many readers, current-value semantics | Prefer `watch` over `broadcast` for state (not events). `watch::Sender<StateEnum>` + `watch::Receiver` gives live state visibility to all subscribers. Any task can call `receiver.changed().await` to react to transitions. |
| `std::fs` / `std::path` | std | Sentinel file polling — MAINTENANCE_MODE, GRACEFUL_RELAUNCH, OTA_DEPLOYING | Already used. For v25.0: centralize all sentinel checks in one `sentinel.rs` module with explicit `warn!` events on creation/deletion. |
| `eprintln!` | std | Pre-logging-init error surfacing — config parse failures, startup crashes | Already used in startup_log.rs ("Never panics -- errors are logged to stderr"). Pattern to enforce: ALL errors before `tracing_subscriber::init()` MUST use `eprintln!`, never `tracing::error!`. |
| `thiserror` | 2 (workspace) | Typed error variants for chain-of-verification failure classification | Already in workspace. Use for `VerificationError` enum with variants per pipeline stage: `InputParseError`, `TransformError`, `DecisionError`, `ActionError`. |

### New Crates — Justified Additions

| Crate | Version | Purpose | Why | Confidence |
|-------|---------|---------|-----|-----------|
| `notify` | 8.2.0 | Filesystem event watcher for sentinel files — MAINTENANCE_MODE writes, config file changes | Windows `ReadDirectoryChangesW` under the hood. Eliminates polling loops for sentinel detection. Use `RecommendedWatcher` + `tokio::sync::mpsc` bridge for async. Sentinel alerting is currently silent — `notify` surfaces the write event immediately. | MEDIUM — verified on docs.rs; Windows support confirmed (windows-sys 0.60 dep, x86_64-pc-windows-msvc listed in supported platforms) |

**Why `notify` is justified over polling:** The current pattern is `loop { sleep(1s); if path.exists() { ... } }`. This either burns CPU or adds 1s latency to sentinel detection. `notify` uses OS-level `ReadDirectoryChangesW` — zero polling, instant detection, single background thread. The file size is minimal (no C++ deps). |

### Patterns Without New Crates (the main workhorses)

These are architectural patterns to implement using existing primitives. They require no Cargo.toml changes.

#### Pattern 1: Chain-of-Verification Trait

```rust
// In rc-common/src/verification.rs (new file, no new deps)
pub trait VerifyStep {
    type Input;
    type Output;
    type Error: std::error::Error;

    fn verify(&self, input: Self::Input) -> Result<Self::Output, Self::Error>;
}

// Usage: each pipeline step wraps its transform in a VerifyStep impl
// The span is created at each step boundary:
//   let span = info_span!("verify_step", step = "config_parse", input = ?raw_toml);
//   let _enter = span.enter();
```

No new dep. `tracing::info_span!` is already available. The trait enforces step-by-step verification by making the boundary explicit.

#### Pattern 2: Observable State via `tokio::sync::watch`

```rust
// In app_state.rs — extend AppState with watch channels
pub state_tx: tokio::sync::watch::Sender<PodState>,
pub state_rx: tokio::sync::watch::Receiver<PodState>,

// On every state transition, log before AND after:
let prev = *self.state_tx.borrow();
self.state_tx.send(new_state).ok();
tracing::warn!(target: "state", prev = ?prev, next = ?new_state, "pod state transition");
```

`tokio::sync::watch` is already in tokio 1. This is purely a usage pattern change — today state changes happen silently; this makes them emit events.

#### Pattern 3: Boot-Resilient Fetch Loop

```rust
// In rc-agent: for ANY resource fetched once at startup
async fn fetch_with_retry<T, F, Fut>(fetch: F, resource: &str) -> T
where F: Fn() -> Fut, Fut: Future<Output = anyhow::Result<T>>, T: Default
{
    let mut attempts = 0u32;
    loop {
        match fetch().await {
            Ok(val) => {
                tracing::info!(target: "boot", resource, attempts, "fetch succeeded");
                return val;
            }
            Err(e) => {
                attempts += 1;
                let delay = Duration::from_secs(2u64.pow(attempts.min(6)));
                tracing::warn!(target: "boot", resource, %e, attempts, delay_secs = delay.as_secs(), "fetch failed, retrying");
                tokio::time::sleep(delay).await;
            }
        }
    }
}
```

No new dep. This is `tokio::time::sleep` + exponential delay. The key is the `tracing::warn!` on every failed attempt — today these failures are silent.

#### Pattern 4: Pre-Init Error Buffer

```rust
// In main.rs, before tracing_subscriber::init():
let mut pre_init_errors: Vec<String> = Vec::new();

// Capture errors during config load, DB init, etc.:
let config = match load_config() {
    Ok(c) => c,
    Err(e) => {
        pre_init_errors.push(format!("config_load FAILED: {e}"));
        eprintln!("[STARTUP ERROR] config_load failed: {e}");
        RcAgentConfig::default()
    }
};

// After tracing init, flush buffer:
for msg in &pre_init_errors {
    tracing::error!(target: "startup", "{}", msg);
}
```

No new dep. Forces pre-logging errors to stderr AND re-emits them after logging is up, so they appear in both the startup log and the structured JSONL log.

#### Pattern 5: Sentinel File Alerting Module

```rust
// In rc-common/src/sentinel.rs (new file, uses notify crate)
// Watch C:\RacingPoint\ for file creates/deletes
// On MAINTENANCE_MODE created: tracing::error! + send WS alert to server
// On OTA_DEPLOYING created: tracing::info! + suppress recovery actions
// On GRACEFUL_RELAUNCH created: tracing::info! + suppress crash counters
```

This is the one case where `notify` is needed — replacing the current silent sentinel file pattern.

### Development / Tooling Additions

| Tool | Purpose | Notes |
|------|---------|-------|
| `cargo test -p rc-common -- verification` | Unit test suite for chain-of-verification steps | Already supported by existing test harness. Add `tests/verification_tests.rs` in rc-common. |
| `tracing-test` crate (optional) | Assert on tracing events in unit tests — verify that `warn!` events fire on silent error paths | Version 0.2. Only add if verification tests need to assert on emitted spans. Otherwise use return type inspection. |

---

## Installation

```toml
# In workspace Cargo.toml — workspace.dependencies
notify = "8.2"   # ONLY new addition

# In crates/rc-agent/Cargo.toml
notify = { workspace = true }

# In crates/racecontrol/Cargo.toml
notify = { workspace = true }
```

```bash
# Verify notify builds on Windows target
cargo check -p rc-agent
```

All other patterns use zero new dependencies.

---

## Alternatives Considered

| Recommended | Alternative | Why Not |
|-------------|-------------|---------|
| `tokio::sync::watch` for state observation | `tokio::sync::broadcast` | `broadcast` drops events if no one is listening; `watch` provides current-value semantics so a late subscriber can always read the current state. For state machines, current-value is correct. |
| Custom `fetch_with_retry` on `tokio::time::sleep` | `tokio-retry` crate (0.3.0) | `tokio-retry` is clean but adds a dep for something that is 15 lines with existing primitives. The custom version also emits `tracing::warn!` on each attempt — the crate does not. The warn is the point. |
| `notify` crate for sentinel watching | Polling loop every 500ms | Polling wakes the CPU every 500ms forever. `notify` is event-driven — zero CPU between events. On Windows, `ReadDirectoryChangesW` is the OS mechanism; `notify` is a thin safe wrapper around it. |
| `tracing::warn!` with structured fields for state transitions | Custom event bus / channel per state type | An event bus adds wiring complexity with no observability benefit over tracing spans. Every `warn!` on a state transition is already captured by the JSONL log subscriber — it's observable by default. |
| `eprintln!` for pre-init errors (+ flush after init) | Panic with error message | Panic aborts before the startup log can record the failure cause. `eprintln!` + default config fallback keeps the process running and records why it fell back. |
| `VerifyStep` trait (custom, no dep) | `validator` crate | `validator` is designed for field-level struct validation (email format, range checks), not pipeline step boundary verification. A simple trait with `Input`/`Output`/`Error` is more expressive for the chain-of-verification pattern. |
| `statig` crate (0.4.1) for pod state machine | Custom enum + match | `statig` is well-designed with `before_transition`/`after_transition` hooks for logging. BUT: the existing pod state machine is not a statig state machine — migrating it would be a breaking change with no immediate reliability benefit. USE statig for NEW state machines in v25.0+, not to rewrite existing ones. |

---

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| OpenTelemetry stack (`opentelemetry`, `axum-tracing-opentelemetry`) | Heavy dep tree, requires collector infrastructure (Jaeger, OTLP endpoint). For 8-pod venue ops with no external telemetry backend, this is massive over-engineering. | `tracing` + `tracing-subscriber` with JSON subscriber (already configured). The JSONL log file IS the telemetry backend. |
| `metrics` / `prometheus` crates | Same over-engineering concern. No Prometheus scraper in the venue. | Structured `tracing::info!` with numeric fields (`count=N`, `duration_ms=N`) achieves the same queryability in the JSONL log. |
| State machine crates for existing state machines | Migrating existing `PodState` enum + match arms to `statig` or `sm` is a rewrite risk with no immediate gain. | Add `tracing::warn!` events to existing `match` arms for state transitions. The observability goal is met without touching state machine structure. |
| `retry` / `again` crates | Both unmaintained or low activity. `tokio-retry` (0.3.0) is fine but overkill for this use case. | Custom 15-line `fetch_with_retry` pattern using `tokio::time::sleep`. |
| `anyhow::Context` for chain verification | `anyhow` context strings are unstructured — good for human readability, not for machine-parseable chain step failures. | `thiserror` enum variants with explicit stage names. `anyhow::Context` for user-facing error messages only. |
| New `tokio::spawn` without lifecycle logs | Silent task death is a root cause category in the 11-bug retrospective. | Every new `tokio::spawn` loop MUST log: (a) start, (b) first item processed, (c) exit reason. Standing rule already exists; the pattern needs enforcement. |

---

## Stack Patterns by Variant

**For chain-of-verification in rc-agent (data pipeline steps):**
- Use `VerifyStep` trait + `info_span!` per step
- Input → Transform → Parse → Decision → Action as 5 typed boundaries
- Each boundary either succeeds (returns typed output) or returns `VerificationError` variant with stage name
- Log the error at `warn!` level with `stage=`, `input=`, `actual=`, `expected=` fields

**For observable state transitions (PodState, BillingState, LockScreenState):**
- Add `watch::Sender<StateEnum>` to `AppState`
- Wrap every `state = NewState` assignment with `tracing::warn!` before the assignment
- Server-side: subscribe to `watch::Receiver` for fleet state visibility

**For boot resilience (allowlist, feature flags, config):**
- Gate the startup loop: try once immediately, fallback to default, then re-fetch every 300s
- Log `warn!` on every failed fetch with the resource name and retry delay
- Log `info!` when re-fetch succeeds, with the number of attempts it took

**For sentinel file observability:**
- Use `notify::RecommendedWatcher` watching `C:\RacingPoint\`
- Filter for `EventKind::Create` on known sentinel file names
- On sentinel detected: `tracing::warn!` + WS alert to server
- Bridge `notify` channel to tokio via `tokio::sync::mpsc` (notify docs pattern)

**For pre-ship verification gates (tooling, not runtime):**
- Domain-matched verification is a PROCESS pattern, not a crate
- Encode as a checklist in the Cause Elimination Process template (bash/markdown)
- No runtime Rust code needed; this is CI/GSD phase tooling

---

## Version Compatibility

| Package | Existing Version | New Addition | Compatibility |
|---------|-----------------|--------------|---------------|
| `tokio` | 1.x (workspace) | n/a | `watch`, `mpsc`, `time::interval` all in tokio 1 |
| `tracing` | 0.1 (workspace) | n/a | `info_span!`, `instrument`, `warn!` with fields all stable |
| `notify` | new: 8.2.0 | 8.2.0 | Windows `windows-sys ^0.60` dep — compatible with Rust 1.70+. Workspace rustc is 1.93.1. No conflict. |
| `thiserror` | 2 (workspace) | n/a | No conflict with new error variants |
| `tracing-test` | optional: 0.2 | only if added | Test-only dev-dependency, zero runtime impact |

---

## Integration With Existing Stack

| Existing Module | v25.0 Addition | Integration Point |
|-----------------|----------------|-------------------|
| `startup_log.rs` | Pre-init error buffer pattern | Add `pre_init_errors` vec before `tracing_subscriber::init()` in `main.rs` |
| `pre_flight.rs` | Chain-of-verification spans | Wrap each of the 3 concurrent checks in `info_span!("pre_flight_check", name = check.name)` |
| `self_monitor.rs` | State transition logging | Add `tracing::warn!` on WS dead detection and CLOSE_WAIT threshold crossings with structured fields |
| `process_guard.rs` | Boot-resilient fetch loop | Already has 300s periodic re-fetch (commit `821c3031`). Add warn logging on each failed fetch. |
| `app_state.rs` | `watch::Sender<PodState>` | Replace bare atomic/mutex state with watch channel for observable transitions |
| `event_loop.rs` | Sentinel file watcher | Spawn `notify` watcher task alongside existing tokio tasks |
| `ws_handler.rs` | State subscription | Subscribe to `watch::Receiver` for state-change-triggered WS messages to server |

---

## Sources

- `docs.rs/notify/latest/notify/` — notify 8.2.0, Windows support (windows-sys 0.60), `RecommendedWatcher` API — MEDIUM confidence
- `docs.rs/tokio-retry/latest/tokio_retry/` — tokio-retry 0.3.0, ExponentialBackoff, strategies — MEDIUM confidence (verified on docs.rs)
- `docs.rs/statig/latest/statig/` — statig 0.4.1, `before_transition`/`after_transition` hooks confirmed — MEDIUM confidence
- `docs.rs/tracing` + tokio-rs/tracing GitHub — `info_span!`, `#[instrument]`, `watch` channel patterns — HIGH confidence (existing dep, used throughout codebase)
- Existing codebase: `startup_log.rs`, `pre_flight.rs`, `self_monitor.rs`, `process_guard.rs` — HIGH confidence (read directly)
- workspace `Cargo.toml` — existing dep versions confirmed — HIGH confidence
- PROJECT.md v25.0 milestone spec — feature requirements — HIGH confidence (read directly)
- CLAUDE.md standing rules (eprintln! for pre-init errors, silent failure root causes) — HIGH confidence (read directly)
- WebSearch: tracing spans for Axum, notify crate Windows support, tokio watch vs broadcast — MEDIUM confidence (corroborated by docs.rs verification)

---

*Stack research for: v25.0 Debug-First-Time-Right — Rust verification framework, observable state machines, boot resilience*
*Researched: 2026-03-26 IST*
