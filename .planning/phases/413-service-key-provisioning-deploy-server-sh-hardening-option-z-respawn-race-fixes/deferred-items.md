# Phase 413 — Deferred Items (Out of Scope)

Items discovered during plan execution that are unrelated to Phase 413's scope.
Logged here for future triage per the scope-boundary rule (only fix issues
DIRECTLY caused by the current plan's changes).

## 2026-04-18 — Plan 04

### rc-sentry-ai release linker failure (LNK4286)

**Discovered by:** Plan 04 Task 4 workspace build verification.
**Command:** `cargo build --release --workspace`
**Status:** Pre-existing. Baseline verified via `git stash && cargo build
--release --workspace` showing identical error on clean HEAD.
**Error shape:**
```
error: linking with `link.exe` failed: exit code: 1120
LINK : warning LNK4098: defaultlib 'MSVCRT' conflicts with use of other libs
LINK : warning LNK4286: symbol '_invalid_parameter_noinfo' defined in
  'libucrt.lib(invalid_parameter.obj)' is imported by
  'libort_sys-b5c7e272924805c4.rlib(...)' [repeated x N]
```
**Root cause (hypothesis):** `ort` + `ort-sys` (ONNX Runtime bindings) +
DirectML pull in libucrt, which conflicts with the static-CRT workspace
setting (`+crt-static` in `.cargo/config.toml`) used by the pod binaries.
`rc-sentry-ai` runs on James .27 (not on pods), so static CRT may not be
needed for this specific crate.
**Candidate fixes (requires investigation):**
1. Remove `+crt-static` from rc-sentry-ai's build profile (per-crate override
   via `package.metadata.rustflags` or a dedicated `.cargo/config.toml` in
   that crate dir).
2. Pin `ort` to a version that plays nicely with static CRT.
3. Switch to `/NODEFAULTLIB:MSVCRT` for rc-sentry-ai.
**Scope:** Separate side-task — not blocking Phase 413 work. Plan 04 builds
`rc-agent` + `racecontrol` cleanly (both `--bin` targets), which are the
only binaries Phase 413 touches.

### `--no-default-features` pre-existing 33 errors

**Discovered by:** Plan 04 Task 4 workspace compile-check.
**Command:** `cargo check --no-default-features -p rc-agent-crate --bin rc-agent`
**Status:** Pre-existing. Baseline verified via `git stash && cargo check
--no-default-features` showing identical 33 errors.
**Error shape:** `error[E0433]: failed to resolve: use of unresolved module
or unlinked crate \`reqwest\`` at:
- `crates/rc-agent/src/openrouter.rs:404`
- `crates/rc-agent/src/mma_engine.rs` (8 locations)
- `crates/rc-agent/src/tier_engine.rs` (several locations)
**Root cause:** `mma_engine`, `tier_engine`, and `openrouter` use `reqwest`
directly without wrapping in `#[cfg(feature = "http-client")]`. Until the
`http-client` feature becomes universally default (it IS in production), the
no-default-features variant is a broken CI shape.
**Candidate fix:** Feature-gate all `reqwest::` usages in those three
modules, OR elevate `reqwest` to a non-optional workspace dep. Low priority
because production never builds with `--no-default-features`.
**Scope:** Plan 04 does NOT worsen this count in its touched files (remote_ops,
ai_debugger, ws_handler, main.rs, app_state, event_loop all have their
feature-gates in place). Deferred as pre-existing tech debt.

## 2026-04-18 — Plan 10

### 2 billing-integration-test failures in `integration.rs` (pre-existing)

**Discovered by:** Plan 10 Task 1 T2 full-suite run.
**Command:** `cargo test -p rc-common -p rc-agent-crate -p racecontrol-crate`
**Failures:**
- `test_billing_rates_delete_excludes_from_cost` — expected 180000 paise baseline 90-min cost, got 135000 (integration.rs:3679)
- `test_financial_e2e_tiered_pricing_integer_math` — expected 75000 for 30-min standard tier, got 70000 (integration.rs:3894)
**Status:** Pre-existing. `git log 36f6d2a0..HEAD -- crates/racecontrol/tests/integration.rs` returns zero commits — the test file has not been modified since `36f6d2a0` (Phase 367-05), which predates Phase 413 by ~2 weeks. Phase 413's commits touch `network_source.rs`, `mesh_intelligence.rs`, `routes.rs`, `mesh_key_cache.rs`, `ai_debugger.rs`, `remote_ops.rs`, `ws_handler.rs`, `csv_lap_fallback.rs`, `main.rs`, `app_state.rs`, `event_loop.rs`, `deploy-server.sh` — zero billing-code changes.
**Root cause (hypothesis):** Drift in the tiered-pricing engine between the baseline the integration tests were written against and the current runtime computation. MEMORY.md documents per-minute tiered pricing landing in commits `290f16ca` + `f4de983d` with a billing migration — possibly the test fixtures' expected paise values weren't updated when the pricing floor/rounding changed, or the migration swapped rate semantics.
**Impact on Phase 413:** None. All Phase 413-specific test suites pass (mesh_key_cache 11/11, remote_ops 19/19 incl. 7 service-key, phase413_tests 7/7 on racecontrol, network_source 21/21, rc-common 252/252). The mesh-service-key HTTP route + rc-agent cache code path is fully exercised and green.
**Candidate fix:** Audit the two tests against the production pricing engine output. Either (a) update the tests' expected paise values if the runtime is correct, or (b) find and fix the regression if the runtime is wrong. Side-task; assign to whoever owns the pricing-engine backlog.
**Scope:** Non-blocking for Plan 11 deploy gate — Plan 11 only touches mesh-service-key code path and deploy-server.sh, neither of which interacts with the billing engine under test.

