---
phase: 321-external-monitoring-alert-chain
plan: 02
subsystem: rc-sentry
tags: [whatsapp, alerting, evolution-api, mon-04, direct-alert]
dependency_graph:
  requires: []
  provides: [AlertConfig, send_whatsapp_alert]
  affects: [tier1_fixes, sentry_config]
tech_stack:
  added: []
  patterns: [std-net-tcpstream-http, dns-resolution-to-socket-addrs, serde-default-backward-compat]
key_files:
  created: []
  modified:
    - crates/rc-sentry/src/sentry_config.rs
    - crates/rc-sentry/src/tier1_fixes.rs
decisions:
  - "Extracted build_whatsapp_alert_request() as testable helper since send_whatsapp_alert() reads from OnceLock config"
  - "Used to_socket_addrs() instead of parse::<SocketAddr>() for DNS hostname support"
metrics:
  duration_seconds: 409
  completed: "2026-04-06T16:47:00Z"
  tasks_completed: 2
  tasks_total: 2
  tests_added: 8
  files_modified: 2
---

# Phase 321 Plan 02: Direct WhatsApp Alert via Evolution API Summary

AlertConfig struct with 6 fields and send_whatsapp_alert() using pure std::net TcpStream to Evolution API, bypassing server relay for MON-04 dual-path alerting.

## Tasks Completed

### Task 1: Add AlertConfig to sentry_config.rs
**Commit:** `cf905275`

- Added `AlertConfig` struct with 6 fields: enabled, whatsapp_url, whatsapp_instance, whatsapp_api_key, whatsapp_number, comms_psk
- Custom `Debug` impl redacts `whatsapp_api_key` and `comms_psk` (follows MeshConfig pattern)
- `Default` impl: all disabled/empty
- Added `alert_config: AlertConfig` field to `SentryConfig` with `#[serde(default)]` for backward compatibility
- 4 unit tests: defaults, full deserialization, missing-section deserialization, secret redaction

### Task 2: Add send_whatsapp_alert() to tier1_fixes.rs
**Commit:** `5e00cabe`

- `build_whatsapp_alert_request()` helper: extracted request formatting logic for testability
- `send_whatsapp_alert()`: pure std::net TcpStream, 5s timeout, DNS via `to_socket_addrs()`
- Wired into `escalate_to_whatsapp()` as dual path (server relay + direct Evolution API)
- Wired into `enter_maintenance_mode()` for MAINTENANCE_MODE entry alerts
- 4 unit tests: request formatting, disabled no-op, empty URL no-op, URL scheme stripping

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Extracted build_whatsapp_alert_request() helper for testability**
- **Found during:** Task 2
- **Issue:** send_whatsapp_alert() reads from sentry_config::load() which uses OnceLock -- tests cannot inject config
- **Fix:** Extracted HTTP request building into a separate function that takes parameters directly, making it independently testable
- **Files modified:** crates/rc-sentry/src/tier1_fixes.rs

## Verification Results

- `cargo check -p rc-sentry`: PASS (compiles cleanly)
- `cargo test -p rc-sentry -- alert_config whatsapp`: 8/8 tests pass
- No `.unwrap()` in production code (only in test code)
- AlertConfig struct: 6 fields confirmed
- send_whatsapp_alert: dual path wiring confirmed (escalate_to_whatsapp line 678, enter_maintenance_mode line 859)

## Known Stubs

None -- all functionality is fully implemented.

## Self-Check: PASSED
