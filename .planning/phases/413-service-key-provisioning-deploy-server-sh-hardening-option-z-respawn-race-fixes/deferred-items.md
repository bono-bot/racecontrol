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

