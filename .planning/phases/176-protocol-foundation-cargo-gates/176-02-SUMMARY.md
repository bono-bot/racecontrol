---
phase: 176-protocol-foundation-cargo-gates
plan: 02
subsystem: infra
tags: [rust, cargo, feature-gates, cfg, conditional-compilation, rc-agent, rc-sentry]

# Dependency graph
requires:
  - phase: 176-protocol-foundation-cargo-gates/176-01
    provides: Phase foundation and project structure for feature gates work
provides:
  - rc-agent optional Cargo features: ai-debugger, process-guard, keyboard-hook, http-client
  - rc-sentry optional Cargo features: watchdog, tier1-fixes, ai-diagnosis
  - Both crates compile with default features (full production build) and --no-default-features (minimal build)
affects: [rc-agent, rc-sentry, deploy, build-system]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Cargo optional features with dep: syntax for optional dependencies"
    - "Pre-compute conditional futures before tokio::join! to avoid macro token count mismatch"
    - "#[cfg] on function parameters for type-polymorphic cfg gates"
    - "Stub struct pattern for cfg-gated types (AiDebuggerConfig stub in config.rs)"
    - "Separate http-client feature from ai-debugger when reqwest is used by multiple subsystems"

key-files:
  created: []
  modified:
    - crates/rc-agent/Cargo.toml
    - crates/rc-agent/src/main.rs
    - crates/rc-agent/src/config.rs
    - crates/rc-agent/src/event_loop.rs
    - crates/rc-agent/src/ws_handler.rs
    - crates/rc-agent/src/failure_monitor.rs
    - crates/rc-agent/src/self_monitor.rs
    - crates/rc-agent/src/self_test.rs
    - crates/rc-agent/src/billing_guard.rs
    - crates/rc-agent/src/kiosk.rs
    - crates/rc-sentry/Cargo.toml
    - crates/rc-sentry/src/main.rs

key-decisions:
  - "Added http-client feature separate from ai-debugger because reqwest is used in 8 rc-agent files beyond ai_debugger.rs — separating them avoids breaking billing/kiosk networking when ai-debugger is disabled"
  - "Pre-compute ollama_fut before tokio::join! in self_test.rs — #[cfg] inside join! causes macro to see variable arg count, mismatching the 22-element tuple destructure"
  - "tier1-fixes = [watchdog] dependency chain because tier1_fixes.rs imports CrashContext from watchdog module"
  - "Guard memory-recording block with all(ai-diagnosis, tier1-fixes) to prevent Vec<()> type error when tier1-fixes is off but ai-diagnosis would access results.iter().filter(|r| r.success)"

patterns-established:
  - "Pre-compute cfg-conditional futures before tokio::join! so macro always sees fixed arg count"
  - "Stub struct for cfg-gated config types: derive Default + serde::Deserialize with #[serde(default)] on all fields"
  - "Separate http-client vs ai-debugger features when a networking library serves multiple subsystems"

requirements-completed: [CF-01, CF-02, CF-04]

# Metrics
duration: 95min
completed: 2026-03-24
---

# Phase 176 Plan 02: Cargo Feature Gates Summary

**Optional Cargo features added to rc-agent (ai-debugger, process-guard, http-client) and rc-sentry (watchdog, tier1-fixes, ai-diagnosis) — both crates compile with default features (full production) and --no-default-features (minimal/bare builds)**

## Performance

- **Duration:** ~95 min (continued from previous session)
- **Started:** 2026-03-24T00:00:00Z (continued)
- **Completed:** 2026-03-24T09:00:00Z IST
- **Tasks:** 2/2
- **Files modified:** 12

## Accomplishments
- rc-agent gains four Cargo features: `ai-debugger`, `process-guard`, `keyboard-hook`, `http-client` — all enabled by default, all can be individually excluded
- rc-sentry gains three features: `watchdog`, `tier1-fixes` (depends on watchdog), `ai-diagnosis` — bare `--no-default-features` build is a lean remote-exec-only HTTP tool
- Both `cargo build -p rc-agent-crate` and `cargo build -p rc-agent-crate --no-default-features` compile with zero errors
- Both `cargo build -p rc-sentry` and `cargo build -p rc-sentry --no-default-features` compile with zero errors
- rc-common: 168 tests pass unaffected

## Task Commits

Each task was committed atomically:

1. **Task 1: Add ai-debugger and process-guard feature gates to rc-agent** - `8b78d116` (feat)
2. **Task 2: Add watchdog, tier1-fixes, ai-diagnosis feature gates to rc-sentry** - `f0ad6492` (feat)

## Files Created/Modified
- `crates/rc-agent/Cargo.toml` - Made reqwest and walkdir optional; added [features] section with 4 features
- `crates/rc-agent/src/main.rs` - Gated mod ai_debugger, mod process_guard; split_brain_probe cfg-conditional type; http-client and process-guard usage gates
- `crates/rc-agent/src/config.rs` - AiDebuggerConfig stub struct for no-ai-debugger builds
- `crates/rc-agent/src/event_loop.rs` - Gated PodStateSnapshot import, AI analysis blocks, execute_ai_action fn and tests, ollama_url/model conditionals, split_brain_probe parameter
- `crates/rc-agent/src/ws_handler.rs` - Gated AI debugger imports and usage, handle_ws_message split_brain_probe parameter
- `crates/rc-agent/src/failure_monitor.rs` - Gated try_auto_fix imports and 3 call sites, build_snapshot fn
- `crates/rc-agent/src/self_monitor.rs` - Gated OLLAMA_CLIENT, ollama_client(), query_ollama(), CLOSE_WAIT consultation block
- `crates/rc-agent/src/self_test.rs` - Gated ollama probe fns; pre-computed ollama_fut outside tokio::join! for fixed arg count
- `crates/rc-agent/src/billing_guard.rs` - Gated http-client reqwest client for orphan billing checks
- `crates/rc-agent/src/kiosk.rs` - Early return in classify_process when ai-debugger off
- `crates/rc-sentry/Cargo.toml` - Added [features]: watchdog, tier1-fixes (deps watchdog), ai-diagnosis; default = all enabled
- `crates/rc-sentry/src/main.rs` - Gated mod declarations, crash-handler thread, tier1 + debug_memory + ollama usage

## Decisions Made
- Added `http-client` feature to rc-agent (deviation from plan): plan assumed reqwest was only needed by `ai_debugger`, but it is also used in `billing_guard.rs`, `kiosk.rs`, `self_test.rs`, `self_monitor.rs`, `main.rs`, `event_loop.rs`, and `ws_handler.rs`. Separating `http-client` from `ai-debugger` allows core networking to stay enabled when only AI features are disabled.
- Pre-compute `ollama_fut` before `tokio::join!` in `self_test.rs`: `#[cfg]` attributes inside the join! macro invocation cause the macro to see a variable number of arguments depending on active features, breaking the 22-element tuple destructure. Moving the conditional outside the macro resolves it cleanly.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Added http-client feature to rc-agent**
- **Found during:** Task 1 (rc-agent feature gates)
- **Issue:** Plan specified `ai-debugger = ["dep:reqwest"]` and `reqwest = { optional = true }`. However, reqwest is used in 8 files beyond `ai_debugger.rs`: `billing_guard.rs`, `kiosk.rs`, `self_test.rs`, `self_monitor.rs`, `main.rs`, `event_loop.rs`, `ws_handler.rs`. Making reqwest optional with only the ai-debugger gate produced 31 compile errors in `--no-default-features` builds from those other files.
- **Fix:** Added `http-client = ["dep:reqwest"]` feature. Default changed to `["ai-debugger", "process-guard", "keyboard-hook", "http-client"]`. Non-AI reqwest usage guarded with `#[cfg(feature = "http-client")]`; AI-specific Ollama usage guarded with `#[cfg(feature = "ai-debugger")]`.
- **Files modified:** `crates/rc-agent/Cargo.toml`, `billing_guard.rs`, `kiosk.rs`, `main.rs`, `event_loop.rs`, `ws_handler.rs`
- **Verification:** `cargo build -p rc-agent-crate --no-default-features` succeeds with zero errors
- **Committed in:** `8b78d116` (Task 1 commit)

**2. [Rule 1 - Bug] Fixed tokio::join! cfg-conditional arg count mismatch in self_test.rs**
- **Found during:** Task 1 (rc-agent feature gates)
- **Issue:** Placing `#[cfg(feature = "ai-debugger")]` directly on one of the 22 timed_probe arguments inside `tokio::join!` caused `error[E0061]: this function takes 1 argument but 0 arguments were supplied` — the macro counted either 21 or 22 futures depending on the active feature, mismatching the 22-element tuple destructure.
- **Fix:** Pre-compute `ollama_fut` as a let binding before the join! macro using `#[cfg]` on the entire binding, then pass `ollama_fut` as a fixed argument inside join!. Macro always sees exactly 22 arguments.
- **Files modified:** `crates/rc-agent/src/self_test.rs`
- **Verification:** `cargo build -p rc-agent-crate --no-default-features` succeeds with zero errors
- **Committed in:** `8b78d116` (Task 1 commit)

---

**Total deviations:** 2 auto-fixed (2 bugs)
**Impact on plan:** Both fixes required for `--no-default-features` correctness. The http-client addition extends the plan's design intent cleanly — same principle, additional scope. No behavior change in default builds.

## Issues Encountered
- Previous session context ran out mid-fix of the self_test.rs tokio::join! issue — resumed and applied the pre-compute pattern immediately

## User Setup Required
None - no external service configuration required. Changes are build-time only; no runtime config or environment variables changed.

## Next Phase Readiness
- rc-agent and rc-sentry both have complete feature gate infrastructure
- Ready for Plan 03 (if exists) or milestone verification
- Default builds produce identical behavior to pre-gate builds — safe to deploy with existing config
- Future per-pod customization: can build minimal binaries by disabling specific features

---
*Phase: 176-protocol-foundation-cargo-gates*
*Completed: 2026-03-24*
