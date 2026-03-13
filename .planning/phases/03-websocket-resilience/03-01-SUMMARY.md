---
phase: 03-websocket-resilience
plan: "01"
subsystem: protocol-ws
tags: [websocket, keepalive, ping-pong, latency, tokio, rc-core, rc-common]
dependency_graph:
  requires: []
  provides: [ws-keepalive-ping, app-level-ping-pong, round-trip-measurement]
  affects: [crates/rc-common/src/protocol.rs, crates/rc-core/src/ws/mod.rs]
tech_stack:
  added: []
  patterns: [tokio-select-interval, arc-mutex-shared-state, atomic-counter]
key_files:
  created: []
  modified:
    - crates/rc-common/src/protocol.rs
    - crates/rc-core/src/ws/mod.rs
decisions:
  - pending_ping uses Arc<Mutex<Option<(u64, Instant)>>> shared between send_task and receive loop -- only one outstanding measurement at a time
  - MissedTickBehavior::Skip on both intervals -- skip rather than burst pings if channel is busy
  - First tick consumed immediately after interval creation to avoid sending ping at t=0
  - No pong timeout added -- existing is_closed() check in pod_monitor handles dead connections
  - No manual WS Pong frame send -- tungstenite auto-queues pong replies (RFC 6455)
metrics:
  duration: "4 min"
  completed_date: "2026-03-13"
  tasks_completed: 2
  files_modified: 2
---

# Phase 3 Plan 01: WS Keepalive Ping + App-Level Round-Trip Measurement

**One-liner:** 15s WS-level keepalive pings prevent TCP idle timeout during game launch CPU spikes; 30s app-level Ping/Pong measures round-trip latency with 200ms warning threshold.

## Tasks Completed

| # | Task | Commit | Files |
|---|------|--------|-------|
| 1 | Add Ping/Pong protocol variants with roundtrip serde tests | 5640c76 | crates/rc-common/src/protocol.rs |
| 2 | Rewrite send_task with WS keepalive ping + app-level round-trip measurement | c2fb701 | crates/rc-core/src/ws/mod.rs |

## What Was Built

### Task 1: Ping/Pong Protocol Variants (CONN-01, PERF-03)

Added two new variants to the protocol enums in `crates/rc-common/src/protocol.rs`:
- `CoreToAgentMessage::Ping { id: u64 }` — server sends to agent for round-trip measurement
- `AgentMessage::Pong { id: u64 }` — agent responds with same id

Two roundtrip serde tests verify JSON serialization with correct tag names ("ping"/"pong") and id preservation. All existing protocol tests unaffected.

### Task 2: send_task Rewrite with tokio::select! (CONN-01, PERF-03)

Rewrote rc-core's `send_task` in `crates/rc-core/src/ws/mod.rs` from a simple `while let` loop to a `tokio::select!` with 3 arms:

1. **cmd_rx.recv()** — forwards CoreToAgentMessage as JSON (existing behavior preserved)
2. **ping_interval (15s)** — sends WS-level `Message::Ping` frame to keep TCP alive during shader compilation spikes
3. **measure_interval (30s)** — sends `CoreToAgentMessage::Ping { id }` for app-level round-trip measurement

Round-trip measurement uses `Arc<tokio::sync::Mutex<Option<(u64, Instant)>>>` shared between send_task (writes timestamp on send) and receive loop (reads/clears on `AgentMessage::Pong`). Logs `tracing::warn!` when round-trip exceeds 200ms, `tracing::debug!` otherwise.

## Decisions Made

- One outstanding measurement at a time (Option, not HashMap) — simpler, sufficient for 30s interval
- `MissedTickBehavior::Skip` — if the channel is busy during a CPU spike, skip that ping tick rather than bursting pings afterward
- First tick consumed immediately after interval creation — prevents sending ping at t=0 before agent is fully registered
- No pong timeout — per RESEARCH.md discretion decision, existing `is_closed()` check in pod_monitor handles dead connections
- No manual `Message::Pong` send — tungstenite auto-queues pong replies per RFC 6455

## Verification Results

- [x] `cargo test -p rc-common` — Ping/Pong serde roundtrip tests pass
- [x] `cargo test -p rc-core` — 83 unit + 13 integration tests pass
- [x] send_task has exactly 3 select! arms (cmd_rx, ping_interval, measure_interval)
- [x] No `Message::Pong` send anywhere in rc-core (auto-handled by tungstenite)

## Deviations from Plan

None — plan executed as written.

## Self-Check: PASSED

All files confirmed present. All commits confirmed in git log.
