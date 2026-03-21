---
phase: 117-alerts-notifications
plan: 01
subsystem: api
tags: [websocket, axum, broadcast, alerts, serde]

requires:
  - phase: 113-face-recognition
    provides: RecognitionResult broadcast channel and types
provides:
  - AlertEvent enum (Recognized + UnknownPerson variants) with JSON serialization
  - WebSocket endpoint at /ws/alerts for real-time dashboard notifications
  - Alert engine subscribing to recognition broadcast and fanning out to WS clients
  - AlertsConfig with enabled, unknown_rate_limit_secs, face_crop_dir, face_crop_quality
affects: [117-02-toast, 117-03-unknown-person, dashboard]

tech-stack:
  added: [axum ws feature]
  patterns: [broadcast fan-out to WebSocket clients, serde tagged enum for JSON discriminator]

key-files:
  created:
    - crates/rc-sentry-ai/src/alerts/mod.rs
    - crates/rc-sentry-ai/src/alerts/types.rs
    - crates/rc-sentry-ai/src/alerts/engine.rs
    - crates/rc-sentry-ai/src/alerts/ws.rs
  modified:
    - crates/rc-sentry-ai/src/config.rs
    - crates/rc-sentry-ai/src/main.rs
    - crates/rc-sentry-ai/Cargo.toml

key-decisions:
  - "Used serde tagged enum (#[serde(tag = type)]) for JSON discriminator on AlertEvent"
  - "Alert broadcast created regardless of config.alerts.enabled so WS endpoint always works"

patterns-established:
  - "Broadcast fan-out: engine subscribes to recognition_tx, converts to AlertEvent, sends on alert_tx; WS clients subscribe to alert_tx"
  - "WebSocket state pattern: AlertWsState holds broadcast Sender, each client subscribes on upgrade"

requirements-completed: [ALRT-01]

duration: 2min
completed: 2026-03-21
---

# Phase 117 Plan 01: Alert WebSocket Infrastructure Summary

**AlertEvent type system with tagged JSON serialization and WebSocket /ws/alerts endpoint broadcasting recognized-person events to dashboard clients via tokio broadcast fan-out**

## Performance

- **Duration:** 2 min
- **Started:** 2026-03-21T18:21:42Z
- **Completed:** 2026-03-21T18:23:58Z
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments
- AlertEvent enum with Recognized and UnknownPerson variants, From<RecognitionResult> conversion
- WebSocket endpoint at /ws/alerts with broadcast fan-out to all connected dashboard clients
- Alert engine subscribing to recognition broadcast and forwarding as AlertEvent messages
- AlertsConfig added to rc-sentry-ai config schema with defaults

## Task Commits

Each task was committed atomically:

1. **Task 1: Alert types, config, and engine module** - `0373cbe` (feat)
2. **Task 2: WebSocket endpoint and main.rs wiring** - `bc00953` (feat)

## Files Created/Modified
- `crates/rc-sentry-ai/src/alerts/mod.rs` - Module declarations (types, ws, engine)
- `crates/rc-sentry-ai/src/alerts/types.rs` - AlertEvent enum with Recognized + UnknownPerson variants
- `crates/rc-sentry-ai/src/alerts/engine.rs` - Engine subscribing to recognition broadcast, forwarding to alert_tx
- `crates/rc-sentry-ai/src/alerts/ws.rs` - WebSocket upgrade handler with broadcast fan-out
- `crates/rc-sentry-ai/src/config.rs` - Added AlertsConfig struct with defaults
- `crates/rc-sentry-ai/src/main.rs` - Alert broadcast channel, engine spawn, WS router merge
- `crates/rc-sentry-ai/Cargo.toml` - Added axum ws feature

## Decisions Made
- Used serde tagged enum `#[serde(tag = "type", rename_all = "snake_case")]` for clean JSON discriminator
- Alert broadcast channel created regardless of `config.alerts.enabled` so WS endpoint always accepts connections (just won't receive events if engine disabled)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Enabled axum ws feature in Cargo.toml**
- **Found during:** Task 2 (WebSocket endpoint)
- **Issue:** axum 0.7 does not include WebSocket support by default; `axum::extract::ws` was gated behind `ws` feature
- **Fix:** Changed `axum = "0.7"` to `axum = { version = "0.7", features = ["ws"] }` in Cargo.toml
- **Files modified:** crates/rc-sentry-ai/Cargo.toml
- **Verification:** cargo check passes clean
- **Committed in:** bc00953 (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Required for WebSocket compilation. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Alert broadcast infrastructure ready for Plan 02 (toast notifications) and Plan 03 (unknown person detection)
- Dashboard clients can connect to ws://james:8096/ws/alerts to receive real-time attendance events

---
*Phase: 117-alerts-notifications*
*Completed: 2026-03-21*
