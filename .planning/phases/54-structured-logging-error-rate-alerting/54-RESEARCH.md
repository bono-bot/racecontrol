# Phase 54: Structured Logging + Error Rate Alerting - Research

**Researched:** 2026-03-20
**Domain:** tracing-subscriber JSON logging + custom tracing Layer for error rate counting
**Confidence:** HIGH

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Switch both racecontrol and rc-agent from text to JSON format via `tracing_subscriber::fmt::layer().json()`
- Fields: timestamp, level, message, target (module path), span context
- rc-agent logs include `pod_id` field (from config) for fleet-wide log aggregation
- racecontrol keeps `rolling::daily` — format change only
- rc-agent switches from `rolling::never` to `rolling::daily`
- Log file naming: `racecontrol-YYYY-MM-DD.jsonl` and `rc-agent-YYYY-MM-DD.jsonl`
- stdout layer stays as human-readable text (JSON for files only)
- Error rate threshold: 5 errors in 1 minute triggers email alert
- Configurable via `racecontrol.toml`: `error_rate_threshold` and `error_rate_window_secs`
- Only `tracing::error!()` counts (not warn)
- Use existing `state.email_alerter` with `should_send` rate limiting
- Recipients: james@racingpoint.in and usingh@racingpoint.in
- 30 days log retention, startup cleanup (delete old `.jsonl` files), no background task
- Use existing tracing-subscriber + tracing-appender stack, no new logging libraries
- Email via existing `send_email.js` shell-out pattern (no SMTP crate)

### Claude's Discretion
All implementation decisions delegated to Claude. Sensible defaults listed above.

### Deferred Ideas (OUT OF SCOPE)
- Prometheus /metrics endpoint (MON-08)
- Structured log search via MCP
- Log aggregation across pods (Netdata Phase 55 may cover)
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| MON-01 | racecontrol emits structured JSON logs via tracing-subscriber with daily file rotation | tracing-subscriber `json` feature flag + rolling::daily already in use |
| MON-02 | rc-agent emits structured JSON logs via tracing-subscriber with daily file rotation | Same feature flag + switch from rolling::never to rolling::daily |
| MON-03 | racecontrol triggers email alert when error rate exceeds N errors in M minutes | Custom `tracing::Layer` counting ERROR events, drives existing email_alerter |
</phase_requirements>

---

## Summary

Both crates use `tracing-subscriber 0.3` with the `env-filter` feature only. The `json` feature is **not** currently enabled in the workspace `Cargo.toml`. Adding `"json"` to the workspace dependency features unlocks `.json()` on the fmt layer builder. This is the only Cargo change needed for both crates — the feature lives in the workspace definition and both crates inherit it.

For MON-03, the standard approach is a custom `tracing::Layer` struct that holds `Arc<Mutex<ErrorRateCounter>>` and increments on `on_event` when the event level is ERROR. The counter checks whether N errors occurred within the sliding window and pushes an alert via the existing `email_alerter` mechanism. The counter runs entirely in-process — no external process, no new thread, no new dependency.

The rolling file naming detail requires attention: `tracing_appender::rolling::daily` generates filenames like `racecontrol.log.2026-03-20`. Getting `.jsonl` extension requires either a custom appender or accepting the `.jsonl.2026-03-20` form. The correct approach is to name the base file without extension and instead name it `racecontrol-` with `tracing_appender::rolling::daily(log_dir, "racecontrol-.jsonl")` — `rolling::daily` appends the date suffix, so the result is `racecontrol-.jsonl.2026-03-20` which is not ideal. The practical solution is to name it `racecontrol` and accept files like `racecontrol.2026-03-20` (no extension), which `jq` handles fine, OR use a custom `RollingFileAppender` builder with `tracing_appender::rolling::RollingFileAppender::builder()` available since tracing-appender 0.2.x.

**Primary recommendation:** Add `"json"` feature to workspace `tracing-subscriber`, implement a custom `ErrorCountLayer`, and use `tracing_appender::rolling::RollingFileAppender::builder()` for full filename control.

---

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| tracing-subscriber | 0.3 (workspace) | JSON fmt layer + env-filter | Already in both crates |
| tracing-appender | 0.2 (workspace) | Rolling file writer, non-blocking | Already in both crates |
| tracing | 0.1 (workspace) | Instrumentation macros | Already in both crates |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| std::sync::Arc + Mutex | stdlib | Shared mutable error counter | MON-03 counter state inside custom Layer |
| chrono | 0.4 (workspace) | Sliding window timestamp arithmetic | Already in both crates |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Custom ErrorCountLayer | tracing-error crate | tracing-error focuses on span capture, not rate counting — overkill |
| RollingFileAppender builder | Custom Writer impl | Builder API is simpler and fully supported since 0.2.x |

**Feature flag change (workspace Cargo.toml):**
```toml
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
```

This single change propagates to both `racecontrol` and `rc-agent` automatically since both reference the workspace dependency.

---

## Architecture Patterns

### Recommended File Layout Changes
```
crates/
├── racecontrol/src/
│   ├── main.rs              # tracing setup changed, log cleanup on startup
│   ├── error_rate.rs        # NEW: ErrorCountLayer + ErrorRateConfig
│   └── config.rs            # add error_rate_threshold, error_rate_window_secs
└── rc-agent/src/
    └── main.rs              # tracing setup changed (rolling::daily + json + pod_id field)
```

### Pattern 1: Enabling JSON Layer on File Writer Only

The stdout layer stays as text; only the file layer uses `.json()`.

```rust
// Source: tracing-subscriber 0.3 docs — fmt::Layer builder
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

let file_appender = tracing_appender::rolling::daily(log_dir, "racecontrol");
let (non_blocking_file, _guard) = tracing_appender::non_blocking(file_appender);

tracing_subscriber::registry()
    .with(env_filter)
    // stdout: human-readable text, unchanged
    .with(tracing_subscriber::fmt::layer().with_target(true))
    // file: JSON, no ANSI
    .with(
        tracing_subscriber::fmt::layer()
            .json()
            .with_target(true)
            .with_ansi(false)
            .with_writer(non_blocking_file),
    )
    .init();
```

Note: `rolling::daily(dir, "racecontrol")` produces files named `racecontrol.2026-03-20`. The `.jsonl` extension is not achievable with the default daily appender without the builder API. See Pattern 3 for the builder approach.

### Pattern 2: Injecting pod_id as a Constant Field (rc-agent)

`tracing_subscriber::fmt::layer().json()` does not have a built-in "add constant field" method. The standard approach is to wrap the subscriber with a custom layer or use `tracing::Span` in the main function. The simplest approach for rc-agent: wrap inside a `tracing::info_span!` that covers `main()`, which will inject the span fields into every JSON log line.

```rust
// rc-agent/src/main.rs — at entry point, after config is loaded
let pod_id = format!("pod_{}", config.pod.number);
let _pod_span = tracing::info_span!("rc-agent", pod_id = %pod_id).entered();
// All log events below this point inherit pod_id in their span context
```

Alternative: implement a custom `Layer` that inserts the field via `on_event`. But the span approach requires zero new code and works natively with the JSON formatter's span recording.

### Pattern 3: Rolling File Appender Builder (for .jsonl filename control)

`tracing_appender::rolling::RollingFileAppender::builder()` is available in tracing-appender 0.2.x:

```rust
// Source: tracing-appender 0.2 docs — RollingFileAppender::builder()
use tracing_appender::rolling::{RollingFileAppender, Rotation};

let file_appender = RollingFileAppender::builder()
    .rotation(Rotation::DAILY)
    .filename_prefix("racecontrol-")
    .filename_suffix("jsonl")
    .build(log_dir)
    .expect("failed to build rolling file appender");
```

This produces filenames like `racecontrol-2026-03-20.jsonl`. This is the correct approach for the naming requirement in the decisions.

**CRITICAL:** Verify `RollingFileAppender::builder()` exists in the version currently locked in Cargo.lock. If it does not (older 0.2.x), fall back to using `rolling::daily(dir, "racecontrol-.jsonl")` which produces `racecontrol-.jsonl.2026-03-20` — still parseable by `jq` but less clean.

### Pattern 4: Custom ErrorCountLayer for MON-03

A custom `tracing::Layer` that counts ERROR events within a sliding window:

```rust
// crates/racecontrol/src/error_rate.rs
use std::sync::{Arc, Mutex};
use std::collections::VecDeque;
use std::time::{Instant, Duration};
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;

pub struct ErrorCountLayer {
    inner: Arc<Mutex<ErrorRateState>>,
}

struct ErrorRateState {
    timestamps: VecDeque<Instant>,
    threshold: usize,
    window: Duration,
    last_alerted: Option<Instant>,
}

impl<S: Subscriber> Layer<S> for ErrorCountLayer {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        if *event.metadata().level() == tracing::Level::ERROR {
            let mut state = self.inner.lock().unwrap();
            let now = Instant::now();
            // Evict timestamps outside window
            let cutoff = now - state.window;
            state.timestamps.retain(|&t| t > cutoff);
            state.timestamps.push_back(now);
            // Check threshold
            if state.timestamps.len() >= state.threshold {
                // Signal alert needed — cannot call async here
                // Use a channel to send to the alerter task
            }
        }
    }
}
```

**Key constraint:** `on_event` is synchronous. The `email_alerter` in AppState is async (`send_alert` is async fn). The bridge must be an `Arc<tokio::sync::Mutex<...>>` with a try-lock, OR a `tokio::sync::mpsc::Sender<()>` that signals an alerter task. The mpsc approach is cleanest:

```rust
// ErrorCountLayer holds: Arc<Mutex<timestamps>>, mpsc::Sender<()>
// A spawned task holds mpsc::Receiver<()> and calls email_alerter.send_alert
// This avoids async-in-sync issues entirely
```

The threshold check should also check `last_alerted` to avoid re-firing within the same window period. The alerter task adds its own cooldown via `should_send`.

### Pattern 5: Startup Log Cleanup (30-day retention)

```rust
// At startup, before tracing is initialized
fn cleanup_old_logs(log_dir: &std::path::Path) {
    let cutoff = std::time::SystemTime::now()
        - std::time::Duration::from_secs(30 * 24 * 3600);
    if let Ok(entries) = std::fs::read_dir(log_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "jsonl")
                || path.to_str().map_or(false, |s| s.contains(".jsonl"))
            {
                if let Ok(meta) = entry.metadata() {
                    if let Ok(modified) = meta.modified() {
                        if modified < cutoff {
                            let _ = std::fs::remove_file(&path);
                        }
                    }
                }
            }
        }
    }
}
```

### Anti-Patterns to Avoid

- **Calling email_alerter inside on_event:** `on_event` is sync; `send_alert` is async. Wrapping in `tokio::runtime::Handle::current().block_on()` inside a tracing callback will deadlock if the executor is busy. Use mpsc channel instead.
- **Two email_alerter recipients as two separate calls:** The existing `EmailAlerter` takes a single `recipient`. For MON-03 sending to both james@ and usingh@, either create two alerter instances or extend `send_alert` to accept multiple recipients. Do NOT call `send_alert` twice with a shared rate limiter — the second call will be rate-limited by the first.
- **Applying .json() to the stdout layer:** stdout should remain text for readability during live tailing. JSON on stdout breaks `tracing`-based test output too.
- **Missing `_guard` lifetime:** `tracing_appender::non_blocking` returns a `(Writer, WorkerGuard)`. If `_guard` is dropped immediately, the background writer thread exits and file logs stop. Must hold guard for the process lifetime (assign to a `_guard` variable in `main`).

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| JSON serialization of log events | Custom serde serializer | `tracing_subscriber::fmt::layer().json()` | Handles all tracing span/event fields correctly, including nested spans |
| Non-blocking file writes | Custom thread + channel | `tracing_appender::non_blocking()` | Handles backpressure, shutdown, and the worker guard lifecycle |
| Rolling file rotation | `std::fs` + date check in log write | `tracing_appender::rolling::daily()` | Thread-safe, handles midnight rotation atomically |
| Rate limiting on error alerts | Custom timestamp tracking | Reuse existing `EmailAlerter::should_send()` | Already proven, tested, has per-pod and venue-wide cooldowns |

---

## Common Pitfalls

### Pitfall 1: WorkerGuard Dropped Too Early
**What goes wrong:** File logs silently stop after init; only stdout logs appear.
**Why it happens:** `tracing_appender::non_blocking` returns `(Writer, WorkerGuard)`. If guard is dropped (e.g., assigned to `_` instead of a named variable), the worker thread exits immediately.
**How to avoid:** Assign to `let _file_guard = ...;` (with underscore prefix to suppress unused warning, but NOT bare `_`). Keep in scope for all of `main`.
**Warning signs:** Binary runs fine but no log file is created or file is empty.

### Pitfall 2: JSON Feature Not Enabled
**What goes wrong:** Compiler error: `no method named json found for struct Layer`.
**Why it happens:** `.json()` on `tracing_subscriber::fmt::Layer` requires the `json` feature flag.
**How to avoid:** Change workspace Cargo.toml to `features = ["env-filter", "json"]`. Both crates inherit from workspace — one change covers both.
**Warning signs:** Immediate compile error, easy to diagnose.

### Pitfall 3: Async in Sync tracing Layer Callback
**What goes wrong:** Deadlock or panic when calling async email code inside `on_event`.
**Why it happens:** `on_event` is called synchronously from within the tracing dispatch path, which may be called from inside a tokio task holding executor resources.
**How to avoid:** Use `mpsc::Sender::try_send()` (non-blocking, non-async) inside `on_event`. Move all async work to a separate spawned task.
**Warning signs:** Process hangs under error load; timeout-related panics in test output.

### Pitfall 4: Both EmailAlerter Recipients Not Receiving
**What goes wrong:** Only one of james@/usingh@ gets the alert.
**Why it happens:** `EmailAlerter::send_alert` takes one recipient. The `should_send` rate limiter fires after the first call, blocking the second.
**How to avoid:** The `send_email.js` script accepts multiple comma-separated recipients OR the `EmailAlerter` can be initialized with a comma-joined recipient string. Check the send_email.js signature before deciding. Alternatively, extend `EmailAlerter` to support a `Vec<String>` of recipients and send one email to multiple addresses (single `node` invocation).
**Warning signs:** Recipient consistently gets no alerts during testing.

### Pitfall 5: RollingFileAppender Builder API Availability
**What goes wrong:** `RollingFileAppender::builder()` does not exist at compile time.
**Why it happens:** The builder API was added in a later patch of tracing-appender 0.2. The workspace locks `^0.2` which resolves to the latest 0.2.x in Cargo.lock, but the exact version matters.
**How to avoid:** Check Cargo.lock for the actual resolved tracing-appender version before relying on the builder. As of 0.2.3+, the builder exists. If older, fall back to the prefix/suffix approach with `daily(dir, "racecontrol")` and accept the `racecontrol.2026-03-20` naming (no extension).
**Warning signs:** Compile error about missing `builder` associated function.

### Pitfall 6: send_email.js Argument Escaping with Multiline Alert Body
**What goes wrong:** The error rate alert email body is truncated or shell-interpreted.
**Why it happens:** `tokio::process::Command::new("node").arg(body)` passes body as a single OS argument — no shell quoting needed. This is safe. But the existing `EmailAlerter::send_alert` signature passes subject and body as separate `.arg()` calls, which is already correct.
**How to avoid:** Use the existing `send_alert` method unchanged. Do not build a shell command string.

---

## Code Examples

Verified patterns from official sources and codebase inspection:

### Current racecontrol tracing setup (from main.rs lines 259-279)
```rust
// CURRENT — text format, needs json() added to file layer
let file_appender = tracing_appender::rolling::daily(log_dir, "racecontrol.log");
let (non_blocking_file, _guard) = tracing_appender::non_blocking(file_appender);
tracing_subscriber::registry()
    .with(env_filter)
    .with(tracing_subscriber::fmt::layer().with_target(false))
    .with(
        tracing_subscriber::fmt::layer()
            .with_target(false)
            .with_ansi(false)
            .with_writer(non_blocking_file),
    )
    .init();
```

### Current rc-agent tracing setup (from main.rs lines 444-467)
```rust
// CURRENT — rolling::never single file, needs daily + json
let file_appender = tracing_appender::rolling::never(&log_dir, "rc-agent.log");
let (non_blocking_file, _file_guard) = tracing_appender::non_blocking(file_appender);
let stdout_layer = tracing_subscriber::fmt::layer();
let file_layer = tracing_subscriber::fmt::layer()
    .with_writer(non_blocking_file)
    .with_ansi(false);
tracing_subscriber::registry()
    .with(env_filter)
    .with(stdout_layer)
    .with(file_layer)
    .init();
```

### Target racecontrol setup (MON-01 + MON-03)
```rust
// Target: JSON file layer + ErrorCountLayer
let file_appender = RollingFileAppender::builder()
    .rotation(Rotation::DAILY)
    .filename_prefix("racecontrol-")
    .filename_suffix("jsonl")
    .build(log_dir)
    .expect("rolling file appender");
let (non_blocking_file, _file_guard) = tracing_appender::non_blocking(file_appender);

let (alert_tx, alert_rx) = tokio::sync::mpsc::channel::<()>(4);
let error_count_layer = ErrorCountLayer::new(
    config.monitoring.error_rate_threshold,
    config.monitoring.error_rate_window_secs,
    alert_tx,
);

tracing_subscriber::registry()
    .with(env_filter)
    .with(tracing_subscriber::fmt::layer().with_target(true))    // stdout text
    .with(
        tracing_subscriber::fmt::layer()
            .json()
            .with_target(true)
            .with_ansi(false)
            .with_writer(non_blocking_file),
    )
    .with(error_count_layer)
    .init();

// Spawn alerter task that receives from alert_rx and calls email_alerter
tokio::spawn(error_rate_alerter_task(state.clone(), alert_rx));
```

### Target rc-agent setup (MON-02)
```rust
// After config is loaded (pod_id is known)
let pod_id = format!("pod_{}", config.pod.number);
let file_appender = RollingFileAppender::builder()
    .rotation(Rotation::DAILY)
    .filename_prefix("rc-agent-")
    .filename_suffix("jsonl")
    .build(&log_dir)
    .expect("rolling file appender");
let (non_blocking_file, _file_guard) = tracing_appender::non_blocking(file_appender);

tracing_subscriber::registry()
    .with(env_filter)
    .with(tracing_subscriber::fmt::layer().with_target(true))    // stdout text
    .with(
        tracing_subscriber::fmt::layer()
            .json()
            .with_target(true)
            .with_ansi(false)
            .with_writer(non_blocking_file),
    )
    .init();

// Enter pod span — all logs below carry pod_id in span context
let _pod_span = tracing::info_span!("rc-agent", pod_id = %pod_id).entered();
```

**Challenge:** `tracing_subscriber` must be initialized BEFORE config is loaded in rc-agent (current code initializes tracing early, before config). The pod_id is not available at tracing init time. Options:
1. Initialize tracing twice (first without file layer, reload after config) — messy
2. Initialize tracing with file layer but use an empty/placeholder pod_id in the span, update span after config load — span fields are static once entered
3. Initialize tracing without the file layer first (stdout only), then add file layer after config is loaded — not easily supported; `registry().init()` can only be called once
4. **Recommended:** Move tracing init after config load in rc-agent. The current code initializes tracing before the mutex check and before config. Restructure to: mutex check → config load → tracing init → rest of startup. This is a straightforward reorder.

Check rc-agent main.rs: the mutex check is at line ~430, tracing init is at lines ~444-467. Config load happens later (around line ~490+). Reordering to: mutex → config → tracing init is clean and fixes the pod_id injection cleanly.

### EmailAlerter recipient — how to send to two people
```rust
// send_email.js signature check needed. If it supports comma-separated:
// EmailAlerter::new("james@racingpoint.in,usingh@racingpoint.in", ...)
// If not, extend send_alert to accept Vec<&str> recipients
// OR: create two EmailAlerter instances (not ideal — independent rate limits)
```

The existing `format_alert_body` and `send_alert` already work. For error-rate alerts, a new method or new EmailAlerter field for secondary recipient is the cleanest extension. Do not duplicate the rate-limiting logic.

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Text log files | JSON log files via `.json()` feature | This phase | Enables `jq` queries on incidents |
| Single rolling file (no rotation for rc-agent) | Daily rotation on both | This phase | Prevents unbounded log growth on pods |
| No structured error counting | Custom Layer with sliding window | This phase | First automated error rate detection |

**Already current:**
- `tracing_appender::non_blocking` — correct pattern, already in use
- `rolling::daily` in racecontrol — already in use, keep it

---

## Open Questions

1. **RollingFileAppender::builder() availability**
   - What we know: workspace locks `tracing-appender = "0.2"`; builder was added in 0.2.3+
   - What's unclear: exact version in Cargo.lock without running `cargo update`
   - Recommendation: planner should instruct implementer to check `Cargo.lock` for tracing-appender version. If < 0.2.3, use `rolling::daily(dir, "racecontrol")` and accept `racecontrol.YYYY-MM-DD` filenames. If >= 0.2.3, use the builder.

2. **send_email.js multi-recipient support**
   - What we know: `EmailAlerter::send_alert` calls `node send_email.js <recipient> <subject> <body>` with one recipient
   - What's unclear: whether `send_email.js` itself supports comma-separated recipients
   - Recommendation: planner should note that implementer must check `send_email.js` (it is in the repo at the server config path). If it does not support multiple recipients, extend `EmailAlerter` with a `secondary_recipient: Option<String>` field that results in a second `node` invocation, bypassing rate-limit check for the CC.

3. **rc-agent init order**
   - What we know: tracing is initialized before config load in current rc-agent main.rs
   - What's unclear: whether early tracing (before config) logs anything critical that would be lost if we move init later
   - Recommendation: The early tracing logs are minor startup messages (crash recovery, early lock screen init). They can be printed with `eprintln!` or lost without impact. Move tracing init after config load to enable pod_id injection cleanly.

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo-nextest + `cargo test` |
| Config file | `nextest.toml` (repo root) |
| Quick run command | `cargo test -p racecontrol-crate error_rate` |
| Full suite command | `cargo test -p racecontrol-crate && cargo test -p rc-agent-crate` |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| MON-01 | racecontrol JSON file layer emits valid JSON | unit | `cargo test -p racecontrol-crate test_json_log` | No — Wave 0 |
| MON-02 | rc-agent daily rolling file created | unit | `cargo test -p rc-agent-crate test_rolling_log` | No — Wave 0 |
| MON-03 | ErrorCountLayer fires alert after N errors in window | unit | `cargo test -p racecontrol-crate error_rate` | No — Wave 0 |
| MON-03 | ErrorCountLayer does NOT fire before threshold | unit | `cargo test -p racecontrol-crate error_rate_below_threshold` | No — Wave 0 |
| MON-03 | Sliding window evicts stale timestamps | unit | `cargo test -p racecontrol-crate error_rate_window_eviction` | No — Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p racecontrol-crate -p rc-agent-crate 2>&1 | tail -5`
- **Per wave merge:** `cargo test -p racecontrol-crate && cargo test -p rc-agent-crate`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/racecontrol/src/error_rate.rs` — ErrorCountLayer + unit tests covering MON-03
- [ ] JSON log layer test helper (use `tracing_test` or a test subscriber) — covers MON-01

---

## Sources

### Primary (HIGH confidence)
- Codebase: `crates/racecontrol/src/main.rs` lines 258-279 — exact current tracing setup
- Codebase: `crates/rc-agent/src/main.rs` lines 442-467 — exact current tracing setup
- Codebase: `Cargo.toml` (workspace) — tracing-subscriber currently has `["env-filter"]` only, no `json` feature
- Codebase: `crates/racecontrol/src/email_alerts.rs` — EmailAlerter implementation, should_send pattern, send_alert signature
- Codebase: `crates/racecontrol/src/config.rs` — existing watchdog config pattern for email fields (model for new monitoring config section)

### Secondary (MEDIUM confidence)
- tracing-subscriber 0.3 docs: `fmt::Layer::json()` requires `json` feature — standard documented feature flag
- tracing-appender 0.2 changelog: `RollingFileAppender::builder()` — added in 0.2.x but exact version uncertain

### Tertiary (LOW confidence)
- Tracing Layer pattern for custom event handling — standard pattern documented in tracing-subscriber 0.3 book, not independently verified against current Cargo.lock version

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — confirmed from Cargo.toml files directly
- Architecture: HIGH for tracing layer setup, MEDIUM for builder API availability
- Pitfalls: HIGH — all derived from reading actual code

**Research date:** 2026-03-20 IST
**Valid until:** 2026-04-20 (tracing ecosystem is stable)
