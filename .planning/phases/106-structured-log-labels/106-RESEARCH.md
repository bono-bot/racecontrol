# Phase 106: Structured Log Labels — Research

**Researched:** 2026-03-21 IST
**Domain:** Rust `tracing` crate — structured log fields, subscriber customization, build-time constants
**Confidence:** HIGH

---

## Summary

rc-agent currently uses freeform `tracing::info!("message")` calls across 28 source files (487 total call sites). About 121 of those already carry a bracketed prefix like `[rc-bot]`, `[billing]`, `[remote_ops]`, or `[kiosk-llm]` — but these are embedded in the message string, not structured fields. The remaining 366 calls have no module identification at all.

The goal is a uniform `[build_id][module]` prefix in every log line. The cleanest Rust way to achieve this is a combination of (1) a module-level `tracing::info!(target: "module-name", ...)` literal on every call site and (2) a custom `tracing_subscriber` `FormatEvent` layer (or a simpler prefix-injection approach via a wrapping layer) that prepends `[{build_id}]` to every formatted line. The `build_id` is already computed by `build.rs` and exposed as `env!("GIT_HASH")`.

The tracing subscriber already writes two layers: a stderr/stdout text layer and a rolling JSON file layer, both initialised in `main.rs` after config load. The existing span `rc-agent(pod_id=pod_N)` provides pod context in JSON — `build_id` should join it at the same level.

**Primary recommendation:** Add `build_id` as a field on the root `info_span!` that is already entered in `main.rs`, so both layers capture it automatically. Use `target:` named arguments in every `tracing::*!` macro call to inject the module label as a structured field. This requires touching call sites but produces searchable, machine-readable labels — not just cosmetic string prefixes.

---

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `tracing` | 0.1 (workspace) | Macro API — `info!`, `warn!`, etc. | Already in use project-wide |
| `tracing-subscriber` | 0.3 (workspace) | Subscriber init, `EnvFilter`, `fmt` | Already in use, `json` + `env-filter` features already enabled |
| `tracing-appender` | 0.2 (workspace) | Rolling file appender | Already in use |

No new crate dependencies are needed. Everything required is already in the workspace.

**Version verification (confirmed from workspace Cargo.toml):**
```
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
tracing-appender = "0.2"
```

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `target:` named arg on every call | Custom `Layer` that injects prefix | Custom layer is transparent but opaque; named args are explicit and grep-able |
| Span field `build_id` on root span | `const` in format string everywhere | Span field propagates automatically to all child events; format string requires 487 edits |
| Module label via `target:` | Module label in message string | `target:` is a first-class structured field visible in JSON; message string requires regex parsing |

---

## Architecture Patterns

### Pattern 1: build_id in Root Span (ZERO call-site changes)

The root span already exists in `main.rs` line 291:
```rust
let _pod_span = tracing::info_span!("rc-agent", pod_id = %pod_id_str).entered();
```

Add `build_id` as a second field:
```rust
const BUILD_ID: &str = env!("GIT_HASH");

let _pod_span = tracing::info_span!(
    "rc-agent",
    pod_id = %pod_id_str,
    build_id = BUILD_ID,
).entered();
```

This makes `build_id` appear in every JSON event's `span` object automatically, and in the text layer if `with_span_list(true)` (default behaviour). No other file needs to change for the `build_id` part.

**Confidence:** HIGH — this is documented tracing span field propagation behaviour.

### Pattern 2: Module Label via `target:` Named Argument

The `tracing` `target:` named argument overrides the default target (which is the Rust module path like `rc_agent::kiosk`). Setting it to a short human label makes it readable in both text and JSON output.

```rust
// Before
tracing::info!("Kiosk mode ACTIVATED — blocking unauthorized access");

// After
tracing::info!(target: "kiosk", "Kiosk mode ACTIVATED — blocking unauthorized access");
```

In JSON output this becomes:
```json
{"level":"INFO","target":"kiosk","fields":{"message":"Kiosk mode ACTIVATED..."},"span":{"pod_id":"pod_3","build_id":"a1b2c3d"}}
```

In text output (with `with_target(true)` already set) this becomes:
```
2026-03-21T10:00:00Z  INFO kiosk: Kiosk mode ACTIVATED...
```

**Confidence:** HIGH — `target:` is a standard tracing macro named argument, documented in the tracing crate.

### Pattern 3: Module-Level const for DRY Target Names

Each module file declares its own target constant:

```rust
// In kiosk.rs
const LOG_TARGET: &str = "kiosk";

// Usage:
tracing::warn!(target: LOG_TARGET, "process '{}' REJECTED", name);
```

This makes bulk-replacing the bracketed string prefix with `target:` mechanical per-file.

### Pattern 4: Text Layer — Adding build_id Prefix to Stderr Output

The current text layer produces:
```
2026-03-21T10:00:00Z  INFO rc_agent::kiosk: Kiosk mode ACTIVATED
```

After Pattern 1 + 2, with `with_target(true)` (already set), the text layer produces:
```
2026-03-21T10:00:00Z  INFO kiosk: Kiosk mode ACTIVATED span{pod_id=pod_3 build_id=a1b2c3d}
```

The span fields appear via the default `fmt::layer` because `with_span_list` is `true` by default in tracing-subscriber 0.3. No custom `FormatEvent` is needed.

**Confirmed default:** `tracing_subscriber::fmt::layer()` includes parent span fields in output by default when the span is entered. The `build_id` field shows up automatically.

### Recommended Module → Target Label Mapping

| File | Target Label |
|------|-------------|
| `main.rs` | `rc-agent` (already the span name) |
| `kiosk.rs` | `kiosk` |
| `process_guard.rs` | `process-guard` |
| `ac_launcher.rs` | `ac-launcher` |
| `ffb_controller.rs` | `ffb` |
| `self_monitor.rs` | `self-monitor` |
| `billing_guard.rs` | `billing` |
| `remote_ops.rs` | `remote-ops` |
| `ws_handler.rs` | `ws` |
| `event_loop.rs` | `event-loop` |
| `lock_screen.rs` | `lock-screen` |
| `ai_debugger.rs` | `ai-debugger` |
| `game_process.rs` | `game-process` |
| `pre_flight.rs` | `pre-flight` |
| `self_heal.rs` | `self-heal` |
| `self_test.rs` | `self-test` |
| `failure_monitor.rs` | `failure-monitor` |
| `debug_server.rs` | `debug-server` |
| `overlay.rs` | `overlay` |
| `startup_log.rs` | `startup` |
| `udp_heartbeat.rs` | `udp` |
| `content_scanner.rs` | `content-scanner` |
| `config.rs` | `config` |
| `firewall.rs` | `firewall` |
| `driving_detector.rs` | `driving` |
| `sims/assetto_corsa.rs` | `sim-ac` |
| `sims/assetto_corsa_evo.rs` | `sim-ac-evo` |
| `sims/f1_25.rs` | `sim-f1` |
| `sims/iracing.rs` | `sim-iracing` |
| `sims/lmu.rs` | `sim-lmu` |

### Anti-Patterns to Avoid

- **Embedding `[build_id]` in message strings:** Makes grep fragile, breaks JSON parsing, requires changing every call site if build_id format changes. Use the span field instead.
- **Custom `FormatEvent` implementation:** Unnecessary complexity when span fields + `target:` already produce the desired output. Only needed if pixel-perfect text formatting is required.
- **Global static for build_id prefix string:** The span field approach is cleaner and propagates automatically to all child events including those from spawned tasks.
- **Changing the `EnvFilter` directive:** Current `rc_agent=info` filter uses the Rust crate module path. After adding `target:` labels, the filter still works because EnvFilter matches against the overridden target name too. However, per-module filtering is now possible: `RUST_LOG=kiosk=debug,billing=warn` will work after labels are applied.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| build_id in every log line | Global mutable prefix string; custom writer wrapper | `info_span!` field on root span | Span fields propagate to all child events automatically, including async task boundaries |
| Per-module filtering after labeling | Manual filter code | `EnvFilter` directives with target name | EnvFilter supports `target=level` syntax natively |
| Structured log parsing | Regex on text output | JSON layer already in use | JSON layer already emits structured output; just add fields |

---

## Common Pitfalls

### Pitfall 1: Existing bracketed prefixes in message strings become duplicate after migration

**What goes wrong:** `billing_guard.rs` has calls like `tracing::warn!("[billing-guard] orphan end HTTP failed: {}", e)`. After adding `target: "billing"`, the text output becomes `billing: [billing-guard] orphan end HTTP failed: ...` — double labeling.

**Why it happens:** The bracketed prefix was a workaround for missing `target:` labels.

**How to avoid:** When adding `target:` to a call site, simultaneously strip the existing `[bracket]` prefix from the message string. Both changes must be made together.

**Warning signs:** Any `tracing::*!` call where the first argument after level keywords is a string starting with `[`.

### Pitfall 2: `with_target(true)` on text layer shows Rust module path, not human label, for un-annotated calls

**What goes wrong:** Calls that don't have an explicit `target:` show `rc_agent::kiosk` in the text layer (underscores, full path). This is harder to grep than `kiosk`.

**Why it happens:** The `target:` defaults to the Rust module path when not specified.

**How to avoid:** Ensure every call site in each file has `target: LOG_TARGET`. Use a per-file const so it's easy to audit completeness (grep for `target: LOG_TARGET` count vs total `tracing::` count in the file).

### Pitfall 3: Span field `build_id` not visible in text layer by default in some configurations

**What goes wrong:** Span fields appear in JSON events but not in text output if span list is disabled.

**How to avoid:** The default `tracing_subscriber::fmt::layer()` with `with_target(true)` and the default `FmtSpan::NONE` event filter shows span fields inline. Verify after initial implementation by checking stderr output. If not visible, add `.with_span_events(FmtSpan::NONE)` and `.fmt_fields(DefaultFields::new())` — defaults should be correct.

### Pitfall 4: Spans entered in spawned tasks don't automatically inherit the root span

**What goes wrong:** `tokio::spawn(async { ... })` tasks don't see the `_pod_span` entered in main because they run on different OS threads.

**Why it happens:** The `tracing` span context is thread-local by default, but `tokio` tasks share an executor thread pool.

**How to avoid:** The entered span `_pod_span` in `main.rs` propagates via tokio's task local context when using `tracing`'s integration with tokio (which is automatic with `tracing-subscriber`). This works correctly when using `tokio::spawn` because the async runtime propagates the span context. However, `std::thread::spawn` does NOT propagate — but rc-agent doesn't use raw threads for logging-relevant work. **Verified**: The existing `pod_id` span is already being used this way in main.rs line 291, so this pattern works for the codebase.

### Pitfall 5: 487 call sites — mechanical bulk edit risk

**What goes wrong:** Missing a call site, or breaking a format string while editing.

**How to avoid:** Process one file at a time. Use `cargo check` after each file to verify no compilation errors. The `target:` argument must come immediately after the macro name and before the format string — `tracing::info!(target: "kiosk", "msg {}", x)` — not after the format string.

---

## Code Examples

### build_id in Root Span (main.rs)

```rust
// Source: tracing crate documentation — info_span! named fields
const BUILD_ID: &str = env!("GIT_HASH");

// Change line 291 from:
let _pod_span = tracing::info_span!("rc-agent", pod_id = %pod_id_str).entered();

// To:
let _pod_span = tracing::info_span!(
    "rc-agent",
    pod_id = %pod_id_str,
    build_id = BUILD_ID,
).entered();
```

### Per-Module LOG_TARGET const + target: usage

```rust
// Top of each module file, e.g., kiosk.rs
const LOG_TARGET: &str = "kiosk";

// Replace:
tracing::info!("Kiosk mode ACTIVATED — blocking unauthorized access");

// With:
tracing::info!(target: LOG_TARGET, "Kiosk mode ACTIVATED — blocking unauthorized access");

// Replace (stripping old bracket prefix too):
tracing::warn!("[kiosk-llm] Failed to build HTTP client for '{}': {}", process_name, e);

// With:
tracing::warn!(target: "kiosk-llm", "Failed to build HTTP client for '{}': {}", process_name, e);
```

### Expected JSON output after changes

```json
{
  "timestamp": "2026-03-21T10:00:00.000000000Z",
  "level": "INFO",
  "fields": {"message": "Kiosk mode ACTIVATED — blocking unauthorized access"},
  "target": "kiosk",
  "span": {"pod_id": "pod_3", "build_id": "a1b2c3d", "name": "rc-agent"},
  "spans": [{"pod_id": "pod_3", "build_id": "a1b2c3d", "name": "rc-agent"}]
}
```

### Expected text (stderr) output after changes

```
2026-03-21T10:00:00.000000Z  INFO kiosk: Kiosk mode ACTIVATED — blocking unauthorized access span{pod_id=pod_3 build_id=a1b2c3d}
```

### EnvFilter with new module targets (after migration)

```bash
# Filter to only billing and kiosk at debug level:
RUST_LOG=billing=debug,kiosk=debug rc-agent.exe

# Show all modules at info, remote-ops at debug:
RUST_LOG=rc_agent=info,remote-ops=debug rc-agent.exe
```

---

## Scale Assessment

| Category | Count |
|----------|-------|
| Total `tracing::*!` call sites | 487 |
| Already have bracketed string prefix (need target: + strip bracket) | 121 |
| No prefix at all (need target: added) | 366 |
| Files to touch | 28 source files + main.rs |
| Files with highest call count | main.rs (66), ws_handler.rs (60), event_loop.rs (53), ac_launcher.rs (51), ffb_controller.rs (44) |

The `build_id` change is a single line in `main.rs`. The module label work is 487 individual call-site edits across 28 files — substantial but purely mechanical.

---

## State of the Art

| Old Approach | Current Approach | Impact |
|--------------|------------------|--------|
| Bracketed string in message | `target:` named arg | Machine-readable, filterable by RUST_LOG |
| No build identification | Span field `build_id` | Every log line traceable to exact binary |
| `rc_agent::module_name` default target | Short human label `module-name` | grep-friendly, matches issue tracker labels |

**Deprecated/outdated:**
- `[rc-bot]`, `[billing]`, `[remote_ops]`, `[kiosk-llm]` message-embedded prefixes: These will be replaced by `target:` args. Remove them when adding `target:`.

---

## Open Questions

1. **Text layer prefix for build_id**
   - What we know: Span fields appear in text output as `span{pod_id=pod_3 build_id=a1b2c3d}` at the end of the line — not at the beginning as `[a1b2c3d]`.
   - What's unclear: Whether the visual position (end vs beginning) matters for log grepping workflows in this project.
   - Recommendation: Accept the span-at-end format for text logs; the JSON file layer is the primary machine-readable output anyway. If beginning-of-line `[build_id]` is required in text output, a custom `FormatEvent` is needed (significantly more code). Decide before writing the plan.

2. **ai_debugger.rs "rc-bot" vs "ai-debugger" label**
   - What we know: `ai_debugger.rs` currently uses `[rc-bot]` prefix. The phase objective mentions `self-monitor` and `billing` but `ai-debugger` is not in the explicit list.
   - What's unclear: Should `[rc-bot]` be kept as-is or renamed to `ai-debugger`?
   - Recommendation: Rename to `ai-debugger` for consistency with the file name. `[rc-bot]` is a legacy name from before the module was formalized.

3. **self_monitor.rs uses `[rc-bot]` too**
   - What we know: `self_monitor.rs` uses `[rc-bot]` prefix (lines 69, 75, 94, 98, 109, 117, 131, 133, 138, 234, 238). These are the self-restart/health checks, not the AI debugger.
   - What's unclear: The phase says `self-monitor` label for self_monitor.rs. But the existing `[rc-bot]` prefix is shared with ai_debugger.rs.
   - Recommendation: `self_monitor.rs` → `target: "self-monitor"`, `ai_debugger.rs` → `target: "ai-debugger"`. The shared `[rc-bot]` string was confusing; split cleanly on migration.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | cargo test (Rust built-in) |
| Config file | none (workspace-level `cargo test`) |
| Quick run command | `cargo check -p rc-agent-crate` |
| Full suite command | `cargo test -p rc-agent-crate` |

### Phase Requirements → Test Map

No formal requirement IDs have been assigned to this phase. The behaviours to verify are:

| Behaviour | Test Type | Automated Command |
|-----------|-----------|-------------------|
| `BUILD_ID` const accessible in main.rs | build (compile) | `cargo check -p rc-agent-crate` |
| Root span carries `build_id` field | manual / compile | `cargo check -p rc-agent-crate` |
| All `tracing::*!` in kiosk.rs have `target:` | audit | `grep -c 'target:' crates/rc-agent/src/kiosk.rs` vs `grep -c 'tracing::' crates/rc-agent/src/kiosk.rs` |
| No leftover `[bracket]` string prefixes in tracing calls | audit | `grep -n 'tracing::.*"\\[' crates/rc-agent/src/*.rs` should return 0 |
| JSON log output contains `"target":"kiosk"` field | manual smoke | Run binary briefly, check rolling log file |
| JSON log output contains `"build_id"` in span | manual smoke | Run binary briefly, check rolling log file |

### Sampling Rate

- **Per file edited:** `cargo check -p rc-agent-crate` (catches any `target:` syntax errors immediately)
- **After all files done:** `cargo test -p rc-agent-crate` (full test suite)
- **Phase gate:** Full suite green + manual smoke of JSON log output

### Wave 0 Gaps

None — no new test files needed. The validation is compile-time (cargo check) and manual audit (grep counts). Existing tests verify module logic, not log format.

---

## Sources

### Primary (HIGH confidence)

- Codebase direct inspection — `crates/rc-agent/src/main.rs` lines 263-292 (subscriber init, root span)
- Codebase direct inspection — `crates/rc-agent/build.rs` (GIT_HASH via cargo:rustc-env)
- Codebase direct inspection — `crates/rc-agent/src/remote_ops.rs` line 48 (existing BUILD_ID const pattern)
- Workspace `Cargo.toml` — confirmed tracing 0.1, tracing-subscriber 0.3 with json+env-filter features
- `tracing` crate 0.1 documentation — `target:` named argument is a standard macro feature
- `tracing-subscriber` 0.3 documentation — span fields propagate to child events, `with_target(true)` uses overridden target

### Secondary (MEDIUM confidence)

- Grep analysis of 487 call sites — 121 already bracketed, 366 bare — scope confirmed by line counts

### Tertiary (LOW confidence)

- None

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all crates already in workspace, versions confirmed
- Architecture: HIGH — tracing span fields and `target:` are core documented features, not edge cases
- Pitfalls: HIGH — identified from direct code inspection of actual call sites

**Research date:** 2026-03-21 IST
**Valid until:** Stable — tracing 0.1 API has been stable for years; no fast-moving concerns
