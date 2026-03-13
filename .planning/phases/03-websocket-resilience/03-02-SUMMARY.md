---
phase: 03-websocket-resilience
plan: "02"
subsystem: rc-agent
tags: [websocket, reconnect, backoff, ping-pong, rc-agent]
dependency_graph:
  requires: [03-01]
  provides: [fast-then-backoff-reconnect, agent-ping-handler]
  affects: [crates/rc-agent/src/main.rs]
tech_stack:
  added: []
  patterns: [pure-function-with-tests, attempt-counter-replacing-delay-var]
key_files:
  created: []
  modified:
    - crates/rc-agent/src/main.rs
decisions:
  - reconnect_attempt resets to 0 on successful connect_async (not on Register send -- register failure continues the loop which increments on the next delay)
  - Exponent formula uses (attempt - 2).min(5) to produce 2s/4s/8s/16s/30s starting at attempt 3
  - Lock screen disconnect behavior (lines 1246-1259) left EXACTLY as-is per locked decisions
  - Ping handler sends Pong via ws_tx (the send half of the split stream) not ws_sender
  - No WS-level Pong handler added -- tungstenite auto-queues pong replies per RFC 6455
metrics:
  duration: "5 min"
  completed_date: "2026-03-13"
  tasks_completed: 2
  files_modified: 1
---

# Phase 3 Plan 02: rc-agent Fast-Then-Backoff Reconnect + Ping Handler

**One-liner:** First 3 reconnect attempts at 1s each for brief CPU spike blips, then exponential backoff 2s→4s→8s→16s→30s cap; agent responds to app-level Ping with Pong for round-trip measurement.

## Tasks Completed

| # | Task | Commit | Files |
|---|------|--------|-------|
| 1 | Extract reconnect_delay_for_attempt pure function with tests | fc2dc8e | crates/rc-agent/src/main.rs |
| 2 | Wire fast-then-backoff into reconnection loop + add Ping handler | 7cbb591 | crates/rc-agent/src/main.rs |

## What Was Built

### Task 1: reconnect_delay_for_attempt (TDD)

Pure function at module level with 3 unit tests covering:
- Fast retries: attempts 0-2 return 1s each
- Exponential backoff: attempt 3→2s, 4→4s, 5→8s, 6→16s
- Cap: attempt 7+ returns 30s (no overflow)

### Task 2: Reconnection Loop + Ping Handler

**Reconnection loop:** Replaced `reconnect_delay` Duration variable with `reconnect_attempt: u32` counter. All 4 delay sites (connect error, timeout, register failure, post-disconnect) now use `reconnect_delay_for_attempt(reconnect_attempt)` with `reconnect_attempt += 1`. Counter resets to 0 only on successful `connect_async`.

**Ping handler:** Added `CoreToAgentMessage::Ping { id }` match arm before the `_ => {}` catch-all. Responds with `AgentMessage::Pong { id }` via ws_tx. If send fails, breaks the event loop (connection lost → reconnect).

**Unchanged:** Lock screen behavior on disconnect (blank screen if no billing, "Disconnected" if billing active). No WS-level Pong handling (tungstenite auto-handles per RFC 6455).

## Decisions Made

- Exponent formula `(attempt - 2).min(5)` — plan's `(attempt - 3).min(4)` produced off-by-one (attempt 3 gave 1s instead of 2s)
- reconnect_attempt resets on successful TCP connect, not on Register send — register failure continues the loop which will increment on next delay
- All 4 delay sites updated consistently (connect error, timeout, register failure, post-event-loop disconnect)

## Verification Results

- [x] `cargo test -p rc-agent` — 47 tests pass (including 3 new reconnect_delay tests)
- [x] `cargo build -p rc-agent` — compiles cleanly
- [x] Lock screen disconnect behavior unchanged (verified by code review)
- [x] No `Message::Pong` send in rc-agent (only app-level Pong via AgentMessage)

## Deviations from Plan

- Fixed exponent formula: `(attempt - 2).min(5)` instead of plan's `(attempt - 3).min(4)` to produce correct 2s starting at attempt 3

## Self-Check: PASSED

All files confirmed present. All commits confirmed in git log.
