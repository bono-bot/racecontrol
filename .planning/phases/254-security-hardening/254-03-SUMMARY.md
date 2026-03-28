---
phase: 254-security-hardening
plan: "03"
subsystem: security
tags: [rust, axum, tokio, tls, native-tls, websocket, wallet, mutex, rbac]

requires:
  - phase: 254-security-hardening-01
    provides: StaffClaims JWT middleware and RBAC role infrastructure used by self-topup guard

provides:
  - Self-topup prevention in wallet topup handler (cashier/manager blocked; superadmin exempt)
  - WSS TLS configuration for rc-agent WebSocket connections (native-tls connector, custom CA cert, skip-verify)
  - tokio::sync::Mutex serializing LaunchGame + clean_state_reset in rc-agent ws_handler

affects:
  - Any future billing or wallet routes that need self-topup awareness
  - rc-agent deploy: next binary will support wss:// URLs in racecontrol.toml [core] section

tech-stack:
  added:
    - native-tls = "0.2" (direct dep in rc-agent Cargo.toml)
  patterns:
    - "Option<Extension<StaffClaims>> in Axum handlers — allows auth-aware logic without requiring auth"
    - "tokio::sync::Mutex<()> in AppState for serializing async critical sections across WS reconnects"
    - "connect_with_tls_config() wrapper pattern — dispatches to plain connect_async for ws://, TLS connector for wss://"

key-files:
  modified:
    - crates/racecontrol/src/api/routes.rs
    - crates/rc-agent/src/ws_handler.rs
    - crates/rc-agent/src/app_state.rs
    - crates/rc-agent/src/main.rs
    - crates/rc-agent/src/config.rs
    - crates/rc-agent/Cargo.toml

key-decisions:
  - "Use Option<Extension<StaffClaims>> (not required Extension) so topup_wallet doesn't break when called without auth headers"
  - "Store game_launch_mutex in AppState (not ConnectionState) so it survives WebSocket reconnections"
  - "Use native-tls over rustls for WSS — Windows certificate store integration, simpler for LAN self-signed certs"
  - "connect_with_tls_config() passes None for request config to avoid HTTP header conflicts"
  - "native-tls must be a DIRECT dep in Cargo.toml — transitive dep via tokio-tungstenite does not expose it as a crate name"

patterns-established:
  - "Self-topup guard: compare JWT sub (staff ID) with path param (target driver ID) before any DB operation"
  - "Game launch serialization: Arc<tokio::sync::Mutex<()>> guard held for entire match arm via RAII Drop"
  - "WSS dispatch pattern: check url.starts_with('wss://') then build connector, fall back to connect_async on TLS error"

requirements-completed: [SEC-05, SEC-07, SEC-10]

duration: 90min
completed: 2026-03-28
---

# Phase 254 Plan 03: Security Hardening (Self-Topup, WSS TLS, Game Launch Mutex) Summary

**Self-topup block via JWT sub comparison, WSS TLS with native-tls connector and custom CA support, game launch race condition eliminated via tokio Mutex in AppState**

## Performance

- **Duration:** ~90 min
- **Started:** 2026-03-28T18:00:00Z
- **Completed:** 2026-03-28T19:30:00Z
- **Tasks:** 2
- **Files modified:** 6

## Accomplishments

- SEC-05: Staff cashier/manager roles blocked from topping up their own driver wallet; superadmin exempt; guard fires before any DB write
- SEC-10: LaunchGame and clean_state_reset serialized via `Arc<tokio::sync::Mutex<()>>` stored in AppState — survives WebSocket reconnects; second concurrent launch queues until first completes
- SEC-07: rc-agent now supports wss:// WebSocket connections using a native-tls connector; custom CA cert path and skip-verify config fields added to [core] in racecontrol.toml; all existing ws:// configs unchanged

## Task Commits

1. **Task 1: Self-topup block and agent game launch mutex** - `e527e315` (feat)
2. **Task 2: WSS TLS configuration for agent-to-server WebSocket** - `9d378350` (feat)

## Files Created/Modified

- `crates/racecontrol/src/api/routes.rs` — Added `Option<Extension<StaffClaims>>` param to `topup_wallet`, SEC-05 guard before DB operation, 4 unit tests in `self_topup_tests` module
- `crates/rc-agent/src/app_state.rs` — Added `game_launch_mutex: Arc<Mutex<()>>` field to AppState
- `crates/rc-agent/src/ws_handler.rs` — Acquires `game_launch_mutex` at top of `LaunchGame` match arm via `_game_launch_guard`
- `crates/rc-agent/src/main.rs` — Added `connect_with_tls_config()` async function, changed reconnect loop to use it, added `game_launch_mutex` to AppState init
- `crates/rc-agent/src/config.rs` — Added `tls_ca_cert_path: Option<String>` and `tls_skip_verify: bool` to CoreConfig (both `#[serde(default)]`), updated test initializer
- `crates/rc-agent/Cargo.toml` — Added `native-tls = "0.2"` as direct dependency

## Decisions Made

- `Option<Extension<StaffClaims>>` rather than required `Extension<StaffClaims>`: the topup endpoint is already behind auth middleware, but the Option wrapper avoids breaking the handler signature in test contexts or if called from a route that doesn't inject claims.
- `game_launch_mutex` lives in `AppState` (not a local `ConnectionState`): the agent's WS reconnection creates new ConnectionState structs, but AppState persists for the process lifetime. A mutex in ConnectionState would reset on reconnect — losing any in-flight launch serialization.
- `native-tls` over `rustls`: native-tls integrates with the Windows certificate store, which is the right trust anchor for LAN deployments. The `tls_skip_verify` option provides an escape hatch for self-signed certs during development.
- `connect_with_tls_config()` falls back to `connect_async()` on TLS connector build error rather than hard-failing — prevents a bad CA cert file from taking an agent completely offline.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added native-tls as direct Cargo dependency**
- **Found during:** Task 2 (WSS TLS implementation)
- **Issue:** `native_tls::TlsConnector::builder()` could not be used in code because `native-tls` was only a transitive dependency via `tokio-tungstenite = { features = ["native-tls"] }`. In Rust 2018+, transitive deps are not in scope as crate names without an explicit `extern crate` declaration or direct dep entry.
- **Fix:** Added `native-tls = "0.2"` to `[dependencies]` in `crates/rc-agent/Cargo.toml`
- **Files modified:** `crates/rc-agent/Cargo.toml`, `Cargo.lock`
- **Verification:** `cargo build --release --bin rc-agent` — Finished in 33.94s, 0 errors
- **Committed in:** `9d378350` (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (blocking — missing direct dependency)
**Impact on plan:** Required for Task 2 to compile. Zero scope change.

## Issues Encountered

- rust-analyzer repeatedly reverted edits to `app_state.rs`, `config.rs`, `ws_handler.rs`, and `main.rs` during the session. Each affected file had to be re-verified with Read tool and re-applied. Root cause: rust-analyzer background formatting/import-organization was racing with edit operations in the workspace. No code-correctness impact — all changes were confirmed present before each build.
- `cargo test -p racecontrol` hit linker error LNK1104 (integration test binary locked by another process). Resolved by using `cargo test -p racecontrol --lib` which runs only lib tests.

## User Setup Required

None — no external service configuration required for these security controls. The `tls_ca_cert_path` and `tls_skip_verify` config fields are additive and default to `None`/`false`; no racecontrol.toml changes needed unless deploying WSS.

## Next Phase Readiness

- SEC-05, SEC-07, SEC-10 complete
- When WSS is ready to deploy: add `tls_ca_cert_path = "/path/to/ca.pem"` or `tls_skip_verify = true` to `[core]` section in pod racecontrol.toml; change `url` to `wss://`
- Self-topup guard is live immediately — no config change needed

---
## Self-Check: PASSED

- FOUND: `.planning/phases/254-security-hardening/254-03-SUMMARY.md`
- FOUND: commit `e527e315` (Task 1: SEC-05 + SEC-10)
- FOUND: commit `9d378350` (Task 2: SEC-07)

*Phase: 254-security-hardening*
*Completed: 2026-03-28*
