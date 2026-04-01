---
phase: 274-whatsapp-escalation
plan: 02
subsystem: server-escalation
tags: [escalation, whatsapp, relay, dedup, fallback]
dependency_graph:
  requires: [274-01]
  provides: [whatsapp-escalation-handler, escalation-dedup, inbox-fallback]
  affects: [racecontrol, ws-handler, state]
tech_stack:
  added: []
  patterns: [dedup-map-with-ttl, relay-with-fallback, tokio-spawn-for-io]
key_files:
  created:
    - crates/racecontrol/src/whatsapp_escalation.rs
  modified:
    - crates/racecontrol/src/lib.rs
    - crates/racecontrol/src/state.rs
    - crates/racecontrol/src/ws/mod.rs
decisions:
  - Used std::sync::Mutex HashMap instead of DashMap (not in workspace, escalations are rare)
  - Reused existing AppState.http_client instead of building a new reqwest::Client
  - WhatsApp routed via Bono relay (localhost:8766), not direct Evolution API
metrics:
  completed: "2026-04-01"
  tasks: 2
  files: 4
---

# Phase 274 Plan 02: Server-Side WhatsApp Escalation Pipeline Summary

Server-side WhatsApp escalation handler with 30-min incident dedup, Bono relay delivery, and comms-link INBOX.md git fallback. Wired into WS handler via AppState.

## What Changed

### Task 1: whatsapp_escalation.rs Module

- Created `WhatsAppEscalation` struct with dedup map, HTTP client, Uday number, relay URL
- `handle_escalation()`: dedup check -> format message -> relay POST -> fallback
- Dedup: std::sync::Mutex<HashMap<String, Instant>> with 30-min TTL, cleanup at >50 entries
- Relay: POST to `http://localhost:8766/relay/exec/run` with `{"command":"whatsapp_send",...}`
- Fallback: append to `comms-link/INBOX.md` + `git add + commit + push` via tokio::process::Command
- IST timestamp via chrono_tz::Asia::Kolkata
- No .unwrap() anywhere
- Commit: `3053fe92`

### Task 2: WS Handler Integration

- Added `whatsapp_escalation` field to AppState (Arc<WhatsAppEscalation>)
- Initialized before http_client is moved into struct (clone order)
- Added `AgentMessage::EscalationRequest(payload)` match arm before catch-all
- Dispatches via `tokio::spawn` (no lock across .await, no WS handler blocking)
- Commit: `3053fe92`

## Decisions Made

1. **std::sync::Mutex over DashMap**: DashMap not in workspace Cargo.toml. Escalations are rare (< 1/hour). Mutex lock is sub-microsecond (no async under lock).
2. **Shared HTTP client**: AppState already builds a reqwest::Client with 30s timeout. Reused via clone (Arc internally) rather than building a separate 15s-timeout client.
3. **Relay routing**: WhatsApp goes through Bono VPS relay per standing rule. Server never calls Evolution API directly.
4. **INBOX.md fallback**: Uses git CLI via tokio::process (not libgit2) for simplicity. Runs only when relay fails.

## Deviations from Plan

None -- plan executed exactly as written.

## Known Stubs

None -- all paths are functional.

## Self-Check: PASSED

- All 4 files (1 created, 3 modified) exist
- Commit 3053fe92 found in git log
- cargo check passes for entire workspace
- EscalationRequest match arm exists in ws/mod.rs
- handle_escalation with dedup + relay + INBOX.md fallback exists
- No .unwrap() in whatsapp_escalation.rs
